//! `gauntlet doctor` — the §8.5 validation and preflight, assembled.
//!
//! Spec §8.5: "REQUIRED checks before any run starts, and re-checked at every
//! iteration boundary for the subset that can drift … Output is a
//! machine-readable report at `$GAUNTLET_REPORT_DIR/doctor.json` plus a human
//! summary. Exit non-zero on any REQUIRED failure."
//!
//! [`run_doctor`] resolves the configuration through all four layers (§8.1),
//! runs the eight checks ([`checks`]), and produces a [`DoctorReport`] whose
//! JSON ([`report`]) is written by [`write_report`]. Individual checks stay
//! callable (decision D16) for the Task-12 controller's drift re-checks.

pub mod checks;
pub mod report;

use std::path::{Path, PathBuf};

use gauntlet_domain::Digest;

use crate::config::defaults;
use crate::config::env_layer::EnvLayer;
use crate::config::error::ConfigError;
use crate::config::fingerprint::config_fingerprint;
use crate::config::gauntlet_md;
use crate::config::resolve::{
    FlagOverrides, ResolveInput, discover_config_path, discover_project_root, resolve,
};
use crate::doctor::checks::RawTokens;

pub use report::{CheckOutcome, CheckStatus, DoctorReport, SubResult};

/// One doctor invocation's inputs. The two vocabularies (Annex E
/// environment names, §8.4 capability names) are supplied by the
/// composition root — see `config::env_layer`'s header for why the
/// application layer does not spell them.
#[derive(Debug, Clone)]
pub struct DoctorInput {
    /// `--config <path>`.
    pub config_override: Option<PathBuf>,
    /// Annex E environment-variable names.
    pub recognized_env: &'static [&'static str],
    /// §8.4 semantic capability names.
    pub capabilities: &'static [&'static str],
    /// Test injection for the process environment; `None` reads the host.
    pub env_pairs: Option<Vec<(String, String)>>,
    /// Test injection for the dotenv text; `None` reads the file at the
    /// project root.
    pub dotenv_text: Option<String>,
}

impl DoctorInput {
    pub fn new(
        recognized_env: &'static [&'static str],
        capabilities: &'static [&'static str],
    ) -> Self {
        Self {
            config_override: None,
            recognized_env,
            capabilities,
            env_pairs: None,
            dotenv_text: None,
        }
    }
}

/// The product of a doctor run: every check's outcome, the fingerprint, and
/// the provenance the human summary needs.
struct DoctorRun {
    report: DoctorReport,
    /// Where doctor.json should be written (the resolved report dir when the
    /// configuration resolved, the conservative fallback otherwise).
    report_dir: PathBuf,
}

/// Runs the eight §8.5 checks end to end.
pub fn run_doctor(input: &DoctorInput) -> DoctorReport {
    let run = assemble(input);
    run.report
}

/// Runs doctor AND writes the machine report. The written path is returned;
/// a write failure is the caller's to surface (the CLI exits non-zero).
pub fn run_doctor_and_write(input: &DoctorInput) -> std::io::Result<(DoctorReport, PathBuf)> {
    let run = assemble(input);
    let path = write_report(&run.report, &run.report_dir)?;
    Ok((run.report, path))
}

fn assemble(input: &DoctorInput) -> DoctorRun {
    // Discovery first: the project root may itself come from the
    // environment, so read the process environment once without a dotenv
    // underneath it, then fold the project root's dotenv in.
    let env_for_root = match &input.env_pairs {
        Some(pairs) => EnvLayer::from_parts("", pairs, input.recognized_env),
        None => EnvLayer::from_parts("", &host_env(), input.recognized_env),
    };
    let project_root = discover_project_root(&env_for_root);
    let dotenv = match &input.dotenv_text {
        Some(text) => text.clone(),
        None => dotenv_at(&project_root),
    };
    let pairs = match &input.env_pairs {
        Some(pairs) => pairs.clone(),
        None => host_env(),
    };
    let env = EnvLayer::from_parts(&dotenv, &pairs, input.recognized_env);
    let config_path = discover_config_path(&project_root, input.config_override.as_deref(), &env);
    // Reading the contract distinguishes four states (not-found is the only
    // honest absence, and only for the DISCOVERED location; anything else
    // the read can fail on is a typed failure, never silently "no
    // GAUNTLET.md"):
    //   Ok(text)                       → parse it below;
    //   Err(NotFound), discovered path → no contract, the environment path;
    //   Err(NotFound), --config path   → typed failure naming the operator's
    //                                    path — an explicit override must not
    //                                    vanish into defaults;
    //   Err(other io failure)          → check 1 failure naming the class.
    let (config_text, read_failure) = match std::fs::read_to_string(&config_path) {
        Ok(text) => (Some(text), None),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => {
            if input.config_override.is_some() {
                (
                    None,
                    Some(ConfigError::ExplicitConfigMissing {
                        path: config_path.to_string_lossy().into_owned(),
                    }),
                )
            } else {
                (None, None)
            }
        }
        Err(err) => (
            None,
            Some(ConfigError::ContractUnreadable {
                leaf: config_path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_default(),
                kind: err.kind(),
            }),
        ),
    };
    let config_origin = config_text
        .as_ref()
        .map(|_| config_path.to_string_lossy().into_owned());

    // Parse, merge, and expand — each failure lands in check 1 and degrades
    // the checks that need products.
    let front_matter = config_text.as_deref().map(gauntlet_md::parse);
    let (config_error, effective, unknown_keys) = match (&read_failure, &front_matter) {
        (Some(err), _) => (
            Some(err.clone()),
            crate::config::resolve::EffectiveConfig::from_defaults(),
            Vec::new(),
        ),
        (None, None) => (
            None,
            crate::config::resolve::EffectiveConfig::from_defaults(),
            Vec::new(),
        ),
        (None, Some(Err(err))) => (
            Some(ConfigError::FrontMatter(err.clone())),
            crate::config::resolve::EffectiveConfig::from_defaults(),
            Vec::new(),
        ),
        (None, Some(Ok(fm))) => {
            let resolved_input = ResolveInput {
                project_root: project_root.clone(),
                front_matter: Some(fm.clone()),
                env: &env,
                flags: FlagOverrides::new(),
            };
            match resolve(&resolved_input) {
                Err(err) => (
                    Some(err),
                    crate::config::resolve::EffectiveConfig::from_defaults(),
                    Vec::new(),
                ),
                Ok(resolved) => (None, resolved.effective, resolved.unknown_keys),
            }
        }
    };
    let (products_error, products) = match &config_error {
        Some(_) => (None, None),
        None => match effective.resolve_against(&env, &project_root) {
            Ok(products) => (None, Some(products)),
            Err(err) => (Some(err), None),
        },
    };
    let fingerprint = config_fingerprint(&effective);

    let check_1 = checks::check_config(&checks::ConfigCheckInput {
        config_error: config_error.clone(),
        products_error: products_error.clone(),
        config_origin: config_origin.clone(),
        unknown_keys: unknown_keys.clone(),
        unknown_env: env.unknown().to_vec(),
        indirections_used: products
            .as_ref()
            .map(|resolved| resolved.indirections_used.clone())
            .unwrap_or_default(),
    });

    let mut all_checks = vec![check_1];
    match &products {
        None => {
            for (id, name) in CHECK_NAMES.iter().enumerate().skip(1) {
                all_checks.push(checks::degraded(id as u8 + 1, name));
            }
        }
        Some(resolved) => {
            let raw = raw_tokens(&effective);
            all_checks.push(checks::check_reference(resolved));
            all_checks.push(checks::check_runners(resolved));
            all_checks.push(checks::check_capabilities(resolved));
            all_checks.push(checks::check_policies(
                resolved,
                &effective.sandbox_policy_dir,
            ));
            all_checks.push(checks::check_required_capabilities(
                &resolved.mcp_required,
                input.capabilities,
            ));
            all_checks.push(checks::check_paths(resolved, &raw));
            all_checks.push(checks::check_budget(resolved));
        }
    }

    let report_dir = products
        .as_ref()
        .map(|resolved| resolved.report_dir.clone())
        .unwrap_or_else(|| defaults::report_dir_at(&project_root));
    DoctorRun {
        report: DoctorReport {
            checks: all_checks,
            config_fingerprint: fingerprint,
            generated_at: unix_now(),
            config_origin,
        },
        report_dir,
    }
}

const CHECK_NAMES: [&str; 8] = [
    "config",
    "reference",
    "runners",
    "capabilities",
    "sandbox policies",
    "required capabilities",
    "writable paths",
    "budget",
];

fn raw_tokens(effective: &crate::config::resolve::EffectiveConfig) -> RawTokens {
    RawTokens {
        state_dir: effective.state_dir.clone(),
        evidence_dir: effective.evidence_dir.clone(),
        workspace_root: effective.workspace_root.clone(),
        answer_key: effective.reference.answer_key.clone(),
    }
}

fn host_env() -> Vec<(String, String)> {
    std::env::vars_os()
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            )
        })
        .collect()
}

fn dotenv_at(project_root: &Path) -> String {
    std::fs::read_to_string(project_root.join(defaults::ENV_FILE_NAME)).unwrap_or_default()
}

fn unix_now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs())
        .unwrap_or(0)
}

/// Writes the canonical report JSON under `dir`, creating the directory.
pub fn write_report(report: &DoctorReport, dir: &Path) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let path = dir.join(defaults::DOCTOR_REPORT_NAME);
    std::fs::write(&path, report.to_json())?;
    Ok(path)
}

/// The report's fingerprint accessor, for callers composing run records.
pub fn fingerprint_of(report: &DoctorReport) -> Digest {
    report.config_fingerprint.clone()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::doctor::report::CheckStatus;

    const RECOGNIZED: &[&str] = &[
        "GAUNTLET_PROJECT_ROOT",
        "GAUNTLET_CONFIG_DIR",
        "GAUNTLET_REPORT_DIR",
        "GAUNTLET_MAX_ROUNDS",
        "GAUNTLET_MCP_REQUIRED",
        "GAUNTLET_SANDBOX_POLICY_DIR",
        "GAUNTLET_STATE_DIR",
        "GAUNTLET_ARTIFACT_DIR",
        "TASK5_CANARY",
    ];
    const CAPABILITIES: &[&str] = &["code_nav", "schema", "versioning", "web", "policy"];

    /// A scratch project: lock, manifest, answer key, policies dir, all in
    /// a temp root driven through the environment so no test touches the
    /// real repository. Since Task 7 closed check 2's honest-unavailables,
    /// the scratch key is a REAL answer-key shape (the strict parser must
    /// admit it) and the scratch lock is a strict freeze record carrying
    /// the key's true normalized digest.
    struct Scratch {
        root: PathBuf,
    }

    /// Two well-formed AK blocks — the same shape the strict parser's own
    /// unit tests admit.
    fn scratch_answer_key() -> String {
        [
            "## AK-001",
            "claim: Scratch claim one is checkable.",
            "verify_by: test",
            "blast_radius: local",
            "authority: critic",
            "evidence_required: {kind: test_report, locator: scratch-0001, producer: instrument}",
            "",
            "## AK-002",
            "claim: Scratch claim two is checkable.",
            "verify_by: test",
            "blast_radius: local",
            "authority: critic",
            "evidence_required: {kind: test_report, locator: scratch-0002, producer: instrument}",
            "",
        ]
        .join("\n")
    }

    impl Scratch {
        fn new() -> Self {
            // pid + atomic counter — the repository Scratch idiom (Task 7
            // alignment; the earlier nanos spelling is retired).
            static SCRATCH_SEQ: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
            let root = std::env::temp_dir().join(format!(
                "gauntlet-doctor-{}-{}",
                std::process::id(),
                SCRATCH_SEQ.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            ));
            std::fs::remove_dir_all(&root).ok();
            std::fs::create_dir_all(root.join("policies")).expect("scratch creates");
            std::fs::create_dir_all(root.join(".gauntlet")).expect("runtime dir");
            let ak = scratch_answer_key();
            std::fs::write(root.join("answer-key.md"), &ak).expect("answer key");
            std::fs::write(root.join("reference.yaml"), scratch_manifest()).expect("manifest");
            std::fs::write(
                root.join(".gauntlet").join("reference.lock"),
                lock_text(ak.as_bytes(), scratch_manifest().as_bytes(), 2),
            )
            .expect("lock");
            Self { root }
        }

        fn input(&self, doc: Option<&str>) -> DoctorInput {
            if let Some(doc) = doc {
                std::fs::write(self.root.join("GAUNTLET.md"), doc).expect("contract");
            }
            let mut input = DoctorInput::new(RECOGNIZED, CAPABILITIES);
            input.env_pairs = Some(vec![(
                "GAUNTLET_PROJECT_ROOT".to_owned(),
                self.root.to_string_lossy().into_owned(),
            )]);
            input
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.root);
        }
    }

    /// The scratch manifest — shallow on purpose: check 2's manifest clause
    /// validates top-level mapping shape (deep `parse_manifest` validation
    /// is a recorded deferral), and the digest clause hashes these exact
    /// bytes against the lock's manifest half.
    fn scratch_manifest() -> String {
        "reference:\n  id: scratch\n  version: 1\n".to_owned()
    }

    /// A strict, frozen-shaped lock for the scratch: the true digests of the
    /// given key and manifest bytes and a chosen `unit_count`.
    fn lock_text(key_bytes: &[u8], manifest_bytes: &[u8], count: u64) -> String {
        let digest =
            crate::reference::normalize::digest_of(key_bytes).expect("the scratch key digests");
        let manifest_digest = crate::reference::normalize::digest_of(manifest_bytes)
            .expect("the scratch manifest digests");
        format!(
            "{{\"answer_key_path\": \"answer-key.md\", \"answer_key_digest\": \"{digest}\", \
              \"manifest_path\": \"reference.yaml\", \"manifest_digest\": \"{manifest_digest}\", \
              \"algo\": \"sha256\", \
              \"frozen_at\": \"2026-08-18T00:00:00Z\", \"unit_count\": {count}}}"
        )
    }

    fn healthy_doc() -> String {
        let mut doc = String::from("---\nreference:\n");
        doc.push_str("  answer_key: answer-key.md\n");
        doc.push_str("  manifest: reference.yaml\n");
        doc.push_str("sandbox:\n  policy_dir: policies\n---\n");
        doc
    }

    fn status_of(report: &DoctorReport, id: u8) -> &CheckStatus {
        &report
            .checks
            .iter()
            .find(|check| check.id == id)
            .expect("check present")
            .status
    }

    #[test]
    fn a_healthy_scratch_project_passes_every_real_check() {
        let scratch = Scratch::new();
        let report = run_doctor(&scratch.input(Some(&healthy_doc())));
        // Fully-real checks pass. Since Task 7 closed check 2's two
        // honest-unavailables (unit schema, digest-vs-lock), check 2 is a
        // fully-real check too: all four of its clauses run and pass. The
        // checks still carrying later-task machinery roll up to honestly
        // unavailable (their real sub-clauses each show pass).
        for id in [1, 2, 5, 6, 7, 8] {
            assert_eq!(
                *status_of(&report, id),
                CheckStatus::Pass,
                "check {id} evidence: {:?}",
                report.checks[id_to_index(id)].sub
            );
        }
        for id in [3, 4] {
            assert!(
                matches!(status_of(&report, id), CheckStatus::Unavailable { .. }),
                "check {id} carries its later-task reason: {:?}",
                report.checks[id_to_index(id)].sub
            );
        }
        // Check 2's four clauses all pass on the healthy scratch: manifest,
        // answer key, unit schema (2/2 well-formed), digest-vs-lock.
        let reference = &report.checks[id_to_index(2)];
        assert_eq!(reference.sub.len(), 4);
        for clause in &reference.sub {
            assert_eq!(
                clause.status,
                CheckStatus::Pass,
                "{}: {:?}",
                clause.name,
                clause.evidence
            );
        }
        assert!(report.exit_ok(), "{}", report.human_summary());
        // All eight checks always appear.
        assert_eq!(report.checks.len(), 8);
    }

    fn id_to_index(id: u8) -> usize {
        usize::from(id) - 1
    }

    #[test]
    fn a_present_top_tier_policy_fails_check_five_and_the_run() {
        let scratch = Scratch::new();
        let report = run_doctor(&scratch.input(Some(&healthy_doc())));
        assert!(report.exit_ok());
        std::fs::write(
            scratch.root.join("policies").join("irreversible.json"),
            "{}",
        )
        .expect("forbidden policy");
        let report = run_doctor(&scratch.input(Some(&healthy_doc())));
        assert_eq!(status_of(&report, 5), &CheckStatus::Fail);
        let failed = report.required_failure().expect("non-zero exit");
        assert_eq!(failed.id, 5);
        let text = report.human_summary();
        assert!(text.contains("AK-049"), "{text}");
        let json = report.to_json();
        assert!(json.contains("AK-049"), "{json}");
        // Removing it restores health.
        std::fs::remove_file(scratch.root.join("policies").join("irreversible.json"))
            .expect("removed");
        let report = run_doctor(&scratch.input(Some(&healthy_doc())));
        assert!(report.exit_ok(), "{}", report.human_summary());
    }

    #[test]
    fn a_bom_prefixed_contract_fails_check_one_citing_the_byte_order_mark() {
        let scratch = Scratch::new();
        let doc = format!("\u{feff}{}", healthy_doc());
        let report = run_doctor(&scratch.input(Some(&doc)));
        assert_eq!(status_of(&report, 1), &CheckStatus::Fail);
        assert!(!report.exit_ok());
        // The BOM reason reaches BOTH surfaces: the human summary and the
        // machine report.
        let human = report.human_summary();
        assert!(human.contains("byte-order mark"), "{human}");
        let json = report.to_json();
        assert!(json.contains("byte-order mark"), "{json}");
        // The rest degrades honestly; all eight checks appear.
        assert_eq!(report.checks.len(), 8);
    }

    #[test]
    fn a_non_utf8_contract_fails_check_one_with_the_failure_class() {
        let scratch = Scratch::new();
        std::fs::write(
            scratch.root.join("GAUNTLET.md"),
            b"\xff\xfe---\nbudget:\n  max_rounds: 5\n---\n",
        )
        .expect("invalid-encoding contract");
        let report = run_doctor(&scratch.input(None));
        assert_eq!(status_of(&report, 1), &CheckStatus::Fail);
        assert!(!report.exit_ok());
        let text = format!("{}{}", report.human_summary(), report.to_json());
        assert!(text.contains("unreadable"), "{text}");
        assert!(text.contains("invalid data"), "{text}");
        // Never the "not found" evidence: the file exists.
        assert!(!text.contains("no GAUNTLET.md found"), "{text}");
    }

    #[cfg(unix)]
    #[test]
    fn a_permission_denied_contract_fails_check_one_not_as_not_found() {
        let scratch = Scratch::new();
        let path = scratch.root.join("GAUNTLET.md");
        std::fs::write(&path, healthy_doc()).expect("contract");
        let enforce = |mode: u32| {
            let mut perms = std::fs::metadata(&path).expect("meta").permissions();
            use std::os::unix::fs::PermissionsExt;
            perms.set_mode(mode);
            std::fs::set_permissions(&path, perms).expect("chmod");
        };
        enforce(0o000);
        // Platform honesty: when the runner ignores mode bits (root), the
        // typed path cannot be exercised through a real read.
        let enforced = std::fs::read_to_string(&path).is_err();
        let report = run_doctor(&scratch.input(None));
        enforce(0o644);
        if !enforced {
            eprintln!("skipped assertions: mode bits not enforced on this runner");
            return;
        }
        assert_eq!(status_of(&report, 1), &CheckStatus::Fail);
        assert!(!report.exit_ok());
        let text = format!("{}{}", report.human_summary(), report.to_json());
        assert!(text.contains("permission denied"), "{text}");
        assert!(!text.contains("no GAUNTLET.md found"), "{text}");
    }

    #[test]
    fn a_missing_contract_stays_the_honest_no_contract_pass() {
        let scratch = Scratch::new();
        let report = run_doctor(&scratch.input(None));
        // Not-found keeps the honest absence wording in check 1 (the run may
        // still fail elsewhere — the default reference paths point at files
        // this scratch does not carry).
        assert_eq!(status_of(&report, 1), &CheckStatus::Pass);
        let check_1 = &report.checks[0];
        let joined = check_1
            .sub
            .iter()
            .map(|clause| clause.evidence.join(" | "))
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(joined.contains("no GAUNTLET.md found"), "{joined}");
        assert!(!joined.contains("unreadable"), "{joined}");
    }

    #[test]
    fn an_explicit_config_override_to_a_missing_path_fails_check_one_naming_it() {
        let scratch = Scratch::new();
        // No GAUNTLET.md anywhere in the scratch: the only contract this run
        // could use is the operator-named one, and it does not exist.
        let mut input = scratch.input(None);
        input.config_override = Some(scratch.root.join("operator-named-missing.md"));
        let report = run_doctor(&input);
        assert_eq!(status_of(&report, 1), &CheckStatus::Fail);
        assert!(!report.exit_ok());
        // The operator's path reaches BOTH surfaces as the typed failure it
        // is — never the honest-absence wording that belongs to discovery.
        let text = format!("{}{}", report.human_summary(), report.to_json());
        assert!(text.contains("operator-named-missing.md"), "{text}");
        assert!(text.contains("explicitly requested config"), "{text}");
        assert!(!text.contains("no GAUNTLET.md found"), "{text}");
        assert_eq!(report.config_origin, None);
        // The override must not vanish into a defaults-fueled run: the
        // failure degraded the products, so every later check reports
        // honestly-unavailable instead of running green on defaults.
        for check in &report.checks {
            if check.id > 1 {
                assert!(
                    matches!(check.status, CheckStatus::Unavailable { .. }),
                    "check {} should degrade, not run on the defaults fallback",
                    check.id
                );
            }
        }
        // Contrast, same scratch: the DISCOVERED missing contract keeps the
        // honest no-contract pass, and its checks DO run on the defaults
        // products — the two absences stay distinguishable states.
        let discovered = run_doctor(&scratch.input(None));
        assert_eq!(status_of(&discovered, 1), &CheckStatus::Pass);
        let joined = discovered.checks[0]
            .sub
            .iter()
            .map(|clause| clause.evidence.join(" | "))
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(joined.contains("no GAUNTLET.md found"), "{joined}");
        assert!(
            !matches!(
                discovered.checks[id_to_index(5)].status,
                CheckStatus::Unavailable { .. }
            ),
            "the discovered absence runs its checks; only the override degrades"
        );
    }

    #[test]
    fn duplicate_flow_mapping_keys_fail_the_run_at_parse() {
        let scratch = Scratch::new();
        let doc = "---\nrunners:\n  by_role:\n    builder: { runner: aaa, runner: bbb }\n\
                    sandbox:\n  policy_dir: policies\n---\n";
        let report = run_doctor(&scratch.input(Some(doc)));
        assert_eq!(status_of(&report, 1), &CheckStatus::Fail);
        assert!(!report.exit_ok());
        let text = format!("{}{}", report.human_summary(), report.to_json());
        assert!(text.contains("duplicate key 'runner'"), "{text}");
    }

    #[test]
    fn forbidden_tier_policies_under_any_spelling_or_link_fail_check_five() {
        let scratch = Scratch::new();
        for name in ["irreversible.yaml", "IRREVERSIBLE.JSON", "irreversible"] {
            std::fs::write(scratch.root.join("policies").join(name), "{}").expect("placed");
            let report = run_doctor(&scratch.input(Some(&healthy_doc())));
            assert_eq!(
                status_of(&report, 5),
                &CheckStatus::Fail,
                "spelling '{name}' must be the forbidden policy"
            );
            assert!(report.to_json().contains("AK-049"));
            std::fs::remove_file(scratch.root.join("policies").join(name)).expect("removed");
        }
        // A broken symlink named for the tier is still a present directory
        // entry: links are not followed.
        #[cfg(unix)]
        {
            let link = scratch.root.join("policies").join("irreversible.json");
            std::os::unix::fs::symlink("no-such-target", &link).expect("dangling symlink");
            let report = run_doctor(&scratch.input(Some(&healthy_doc())));
            assert_eq!(status_of(&report, 5), &CheckStatus::Fail);
            std::fs::remove_file(&link).expect("removed");
        }
        // Clean again: green.
        let report = run_doctor(&scratch.input(Some(&healthy_doc())));
        assert!(report.exit_ok(), "{}", report.human_summary());
    }

    #[cfg(unix)]
    #[test]
    fn an_unreadable_policy_dir_fails_check_five_and_recovers_when_readable() {
        let scratch = Scratch::new();
        let policies = scratch.root.join("policies");
        let enforce = |mode: u32| {
            use std::os::unix::fs::PermissionsExt;
            let mut perms = std::fs::metadata(&policies).expect("meta").permissions();
            perms.set_mode(mode);
            std::fs::set_permissions(&policies, perms).expect("chmod");
        };
        enforce(0o000);
        // Platform honesty: when the runner ignores mode bits (root), the
        // unreadable path cannot be exercised through a real read_dir.
        let enforced = std::fs::read_dir(&policies).is_err();
        let report = run_doctor(&scratch.input(Some(&healthy_doc())));
        // Restore BEFORE any assertion can bail, so the scratch's cleanup
        // keeps its ability to remove the directory.
        enforce(0o755);
        if !enforced {
            eprintln!("skipped assertions: mode bits not enforced on this runner");
            return;
        }
        // An unreadable policy dir is NEVER clean: check 5 fails and takes
        // the run with it, with the unreadable reason on both surfaces.
        assert_eq!(status_of(&report, 5), &CheckStatus::Fail);
        assert!(!report.exit_ok());
        let failed = report.required_failure().expect("non-zero exit");
        assert_eq!(failed.id, 5);
        let text = format!("{}{}", report.human_summary(), report.to_json());
        assert!(text.contains("policy dir unreadable"), "{text}");
        assert!(text.contains("permission denied"), "{text}");
        assert!(text.contains("never clean"), "{text}");
        // Readability restored: the now-readable EMPTY dir is honestly clean
        // and the run goes green again — recovery, not a stuck failure.
        let recovered = run_doctor(&scratch.input(Some(&healthy_doc())));
        assert_eq!(status_of(&recovered, 5), &CheckStatus::Pass);
        assert!(recovered.exit_ok(), "{}", recovered.human_summary());
    }

    #[test]
    fn a_forbidden_policy_in_the_default_dir_fails_even_when_policy_dir_is_overridden() {
        let scratch = Scratch::new();
        let doc = "---\nreference:\n  answer_key: answer-key.md\n  manifest: reference.yaml\n\
                    sandbox:\n  policy_dir: policies2\n---\n";
        std::fs::create_dir_all(scratch.root.join("policies2")).expect("overridden dir");
        std::fs::create_dir_all(scratch.root.join(".gauntlet").join("policies"))
            .expect("default policies dir");
        std::fs::write(
            scratch
                .root
                .join(".gauntlet")
                .join("policies")
                .join("irreversible.json"),
            "{}",
        )
        .expect("forbidden policy in the default dir");
        let report = run_doctor(&scratch.input(Some(doc)));
        assert_eq!(status_of(&report, 5), &CheckStatus::Fail);
        assert!(!report.exit_ok());
        // Both scanned dirs appear as their own evidence lines: the clean
        // overridden dir and the offending default dir.
        let json = report.to_json();
        assert!(json.contains("policies2"), "{json}");
        assert!(json.contains(".gauntlet"), "{json}");
        assert!(json.contains("AK-049"), "{json}");
        // Removing it restores health.
        std::fs::remove_file(
            scratch
                .root
                .join(".gauntlet")
                .join("policies")
                .join("irreversible.json"),
        )
        .expect("removed");
        let report = run_doctor(&scratch.input(Some(doc)));
        assert!(report.exit_ok(), "{}", report.human_summary());
    }

    #[test]
    fn a_malformed_money_value_fails_under_config_values_not_indirections() {
        let scratch = Scratch::new();
        let doc = "---\nreference:\n  answer_key: answer-key.md\n  manifest: reference.yaml\n\
                    sandbox:\n  policy_dir: policies\nbudget:\n  max_cost_usd: 1e3\n---\n";
        let report = run_doctor(&scratch.input(Some(doc)));
        assert_eq!(status_of(&report, 1), &CheckStatus::Fail);
        assert!(!report.exit_ok());
        let check_1 = &report.checks[0];
        let values = check_1
            .sub
            .iter()
            .find(|clause| clause.name == "config values")
            .expect("config values clause");
        assert_eq!(values.status, CheckStatus::Fail);
        assert!(
            values.evidence.join("; ").contains("budget.max_cost_usd"),
            "{:?}",
            values.evidence
        );
        let indirections = check_1
            .sub
            .iter()
            .find(|clause| clause.name == "indirections")
            .expect("indirections clause");
        assert_ne!(indirections.status, CheckStatus::Fail);
        // The old wrong locus is gone: no "indirection failed" re-badging.
        let text = format!("{}{}", report.human_summary(), report.to_json());
        assert!(!text.contains("indirection failed"), "{text}");
    }

    #[test]
    fn dotenv_inline_comments_no_longer_corrupt_directory_names() {
        let scratch = Scratch::new();
        let mut input = scratch.input(Some(&healthy_doc()));
        input.dotenv_text = Some(format!(
            "GAUNTLET_STATE_DIR={} # inline note\n",
            scratch.root.join("state").display()
        ));
        let report = run_doctor(&input);
        assert!(report.exit_ok(), "{}", report.human_summary());
        // The clean directory is the one doctor probed and created; no
        // directory named after the comment text exists.
        assert!(scratch.root.join("state").is_dir());
        let weird: Vec<String> = std::fs::read_dir(&scratch.root)
            .expect("scratch root readable")
            .flatten()
            .map(|entry| entry.file_name().to_string_lossy().into_owned())
            .filter(|name| name.contains('#'))
            .collect();
        assert!(
            weird.is_empty(),
            "comment text leaked into names: {weird:?}"
        );
    }

    #[test]
    fn a_live_backend_requires_tier_policy_files() {
        let scratch = Scratch::new();
        let doc = "---\nreference:\n  answer_key: answer-key.md\n  manifest: reference.yaml\n\
                    sandbox:\n  backend: wrapped\n  policy_dir: policies\n---\n";
        let report = run_doctor(&scratch.input(Some(doc)));
        // backend live: the existence clause runs; policy files are absent.
        assert_eq!(status_of(&report, 5), &CheckStatus::Fail);
        let names = ["contained", "local", "cross_cutting"];
        for tier in names {
            std::fs::write(
                scratch.root.join("policies").join(format!("{tier}.json")),
                "{}",
            )
            .expect("policy file");
        }
        let report = run_doctor(&scratch.input(Some(doc)));
        assert_eq!(status_of(&report, 5), &CheckStatus::Pass);
    }

    #[test]
    fn an_answer_key_inside_the_workspace_fails_check_seven() {
        let scratch = Scratch::new();
        let doc = "---\nreference:\n  answer_key: answer-key.md\n  manifest: reference.yaml\n\
                    sandbox:\n  policy_dir: policies\nworkspace:\n  root: .\n---\n";
        let report = run_doctor(&scratch.input(Some(doc)));
        assert_eq!(status_of(&report, 7), &CheckStatus::Fail);
        assert!(!report.exit_ok());
    }

    #[test]
    fn a_wrong_unit_count_fails_check_two() {
        let scratch = Scratch::new();
        // Two blocks in the file; lie in the frozen record. The lock's
        // digest stays TRUE, so the failure locus is exactly the count.
        let key_bytes = std::fs::read(scratch.root.join("answer-key.md")).expect("key reads");
        std::fs::write(
            scratch.root.join(".gauntlet").join("reference.lock"),
            lock_text(&key_bytes, scratch_manifest().as_bytes(), 5),
        )
        .expect("lock placed");
        let report = run_doctor(&scratch.input(Some(&healthy_doc())));
        assert_eq!(status_of(&report, 2), &CheckStatus::Fail);
        let check = &report.checks[id_to_index(2)];
        let count_clause = check
            .sub
            .iter()
            .find(|clause| clause.name == "answer key")
            .expect("count clause");
        assert_eq!(count_clause.status, CheckStatus::Fail);
        let digest_clause = check
            .sub
            .iter()
            .find(|clause| clause.name == "digest")
            .expect("digest clause");
        assert_eq!(
            digest_clause.status,
            CheckStatus::Pass,
            "the count lie must not smear into the digest clause: {:?}",
            digest_clause.evidence
        );
    }

    /// Task 7's digest-vs-lock clause: same block count, well-formed units,
    /// DIFFERENT content — only the recomputed digest can catch it. The
    /// failure is loud and reference-class on both surfaces.
    #[test]
    fn a_moved_answer_key_fails_check_two_as_a_reference_class_failure() {
        let scratch = Scratch::new();
        let original =
            std::fs::read_to_string(scratch.root.join("answer-key.md")).expect("key reads");
        let moved = original.replace(
            "claim: Scratch claim one is checkable.",
            "claim: The builder quietly edited claim one.",
        );
        std::fs::write(scratch.root.join("answer-key.md"), moved).expect("moved key");
        let report = run_doctor(&scratch.input(Some(&healthy_doc())));
        assert_eq!(status_of(&report, 2), &CheckStatus::Fail);
        let check = &report.checks[id_to_index(2)];
        // The locus is the digest clause alone: shape-level clauses pass.
        for name in ["answer key", "unit schema"] {
            let clause = check
                .sub
                .iter()
                .find(|clause| clause.name == name)
                .expect("clause");
            assert_eq!(
                clause.status,
                CheckStatus::Pass,
                "{name}: {:?}",
                clause.evidence
            );
        }
        let digest_clause = check
            .sub
            .iter()
            .find(|clause| clause.name == "digest")
            .expect("digest clause");
        assert_eq!(digest_clause.status, CheckStatus::Fail);
        // The loud reference-class wording reaches BOTH surfaces.
        let human = report.human_summary();
        let json = report.to_json();
        for text in [&human, &json] {
            assert!(text.contains("REFERENCE MOVED"), "{text}");
            assert!(text.contains("frozen record"), "{text}");
        }
        assert!(!report.exit_ok());
    }

    /// The manifest half of digest-vs-lock (critic-A M3 / critic-B F2): a
    /// rewrite of the ruler's narrative half — keeping every shape-level
    /// clause green — is caught ONLY by the recomputed manifest digest, and
    /// is a reference-class failure on both surfaces.
    #[test]
    fn a_moved_manifest_fails_check_two_as_a_reference_class_failure() {
        let scratch = Scratch::new();
        std::fs::write(
            scratch.root.join("reference.yaml"),
            "reference:\n  id: scratch\n  version: 999\n",
        )
        .expect("moved manifest");
        let report = run_doctor(&scratch.input(Some(&healthy_doc())));
        assert_eq!(status_of(&report, 2), &CheckStatus::Fail);
        let check = &report.checks[id_to_index(2)];
        // The locus is the digest clause alone: the moved manifest keeps its
        // top-level mapping shape and the answer key is untouched.
        for name in ["manifest", "answer key", "unit schema"] {
            let clause = check
                .sub
                .iter()
                .find(|clause| clause.name == name)
                .expect("clause");
            assert_eq!(
                clause.status,
                CheckStatus::Pass,
                "{name}: {:?}",
                clause.evidence
            );
        }
        let digest_clause = check
            .sub
            .iter()
            .find(|clause| clause.name == "digest")
            .expect("digest clause");
        assert_eq!(digest_clause.status, CheckStatus::Fail);
        let evidence = digest_clause.evidence.join("\n");
        assert!(evidence.contains("REFERENCE MOVED"), "{evidence}");
        assert!(evidence.contains("manifest"), "{evidence}");
        assert!(
            evidence.contains("answer-key half"),
            "the untouched key half still reports its match: {evidence}"
        );
        assert!(!report.exit_ok());
    }

    /// The strict lock read: an unknown field in the freeze record is a
    /// finding naming the field — drift, never silently ignored.
    #[test]
    fn an_unknown_field_in_the_reference_record_is_a_finding() {
        let scratch = Scratch::new();
        let key_bytes = std::fs::read(scratch.root.join("answer-key.md")).expect("key reads");
        let drifted = lock_text(&key_bytes, scratch_manifest().as_bytes(), 2)
            .replace('}', ", \"revision\": 2}");
        std::fs::write(
            scratch.root.join(".gauntlet").join("reference.lock"),
            drifted,
        )
        .expect("lock placed");
        let report = run_doctor(&scratch.input(Some(&healthy_doc())));
        assert_eq!(status_of(&report, 2), &CheckStatus::Fail);
        let text = format!("{}{}", report.human_summary(), report.to_json());
        assert!(text.contains("unknown field"), "{text}");
        assert!(text.contains("revision"), "{text}");
    }

    /// Task 7's unit-schema clause: one malformed block fails THAT block
    /// (per-block isolation), the clause reports the block-level locus, and
    /// the failure takes the run with it.
    #[test]
    fn a_malformed_block_fails_the_unit_schema_clause_with_its_locus() {
        let scratch = Scratch::new();
        let original =
            std::fs::read_to_string(scratch.root.join("answer-key.md")).expect("key reads");
        let broken = original.replace(
            "claim: Scratch claim one is checkable.",
            "claim: Scratch claim one is checkable.\nphantom_field: 1",
        );
        std::fs::write(scratch.root.join("answer-key.md"), broken).expect("broken key");
        let report = run_doctor(&scratch.input(Some(&healthy_doc())));
        assert_eq!(status_of(&report, 2), &CheckStatus::Fail);
        assert!(!report.exit_ok());
        let check = &report.checks[id_to_index(2)];
        let schema_clause = check
            .sub
            .iter()
            .find(|clause| clause.name == "unit schema")
            .expect("schema clause");
        assert_eq!(schema_clause.status, CheckStatus::Fail);
        let evidence = schema_clause.evidence.join("; ");
        assert!(
            evidence.contains("phantom_field"),
            "the block-level locus is carried: {evidence}"
        );
        assert!(
            evidence.contains("AK-001"),
            "the failing block is named: {evidence}"
        );
        // The needle is built, not spelled: a bare `1/2` string literal is a
        // path-literal shape to the boundary scanner (AK-016).
        let well_formed_fraction = format!("{}/{}", 1, 2);
        assert!(
            evidence.contains(&well_formed_fraction),
            "the well-formed count is reported: {evidence}"
        );
    }

    #[test]
    fn an_unparsable_contract_fails_check_one_and_degrades_the_rest() {
        let scratch = Scratch::new();
        let doc = "---\nbudget:\n\tmax_rounds: 5\n---\n";
        let report = run_doctor(&scratch.input(Some(doc)));
        assert_eq!(status_of(&report, 1), &CheckStatus::Fail);
        assert!(!report.exit_ok());
        // Every later check is honestly unavailable, all eight appear.
        assert_eq!(report.checks.len(), 8);
        for check in &report.checks {
            if check.id > 1 {
                assert!(
                    matches!(check.status, CheckStatus::Unavailable { .. }),
                    "check {} should be honestly unavailable",
                    check.id
                );
            }
        }
    }

    #[test]
    fn unknown_capabilities_fail_and_unknown_names_are_reported() {
        let scratch = Scratch::new();
        let doc = "---\nreference:\n  answer_key: answer-key.md\n  manifest: reference.yaml\n\
                    sandbox:\n  policy_dir: policies\nmcp:\n---\n";
        let mut input = scratch.input(Some(doc));
        let pairs = vec![
            (
                "GAUNTLET_PROJECT_ROOT".to_owned(),
                scratch.root.to_string_lossy().into_owned(),
            ),
            (
                "GAUNTLET_MCP_REQUIRED".to_owned(),
                "code_nav, no_such_cap".to_owned(),
            ),
            ("GAUNTLET_TYPO_VAR".to_owned(), "x".to_owned()),
        ];
        input.env_pairs = Some(pairs);
        let report = run_doctor(&input);
        assert_eq!(status_of(&report, 6), &CheckStatus::Fail);
        let check_1 = &report.checks[0];
        let joined = check_1
            .sub
            .iter()
            .map(|clause| clause.evidence.join(" | "))
            .collect::<Vec<_>>()
            .join(" | ");
        assert!(joined.contains("GAUNTLET_TYPO_VAR"), "{joined}");
        assert!(joined.contains("unknown key 'mcp'"), "{joined}");
        assert!(!report.exit_ok());
    }

    #[test]
    fn budget_violations_fail_check_eight_through_the_domain() {
        let scratch = Scratch::new();
        let doc = "---\nreference:\n  answer_key: answer-key.md\n  manifest: reference.yaml\n\
                    sandbox:\n  policy_dir: policies\nbudget:\n  max_rounds: 0\n  \
                    recovery_reserve_pct: 100\n---\n";
        let report = run_doctor(&scratch.input(Some(doc)));
        assert_eq!(status_of(&report, 8), &CheckStatus::Fail);
        let mut joined = report.checks[7].evidence.join("; ");
        for clause in &report.checks[7].sub {
            joined.push_str("; ");
            joined.push_str(&clause.evidence.join("; "));
        }
        assert!(joined.contains("zero"), "{joined}");
        assert!(joined.contains("one hundred"), "{joined}");
    }

    #[test]
    fn the_canary_secret_never_reaches_the_report_or_the_summary() {
        let scratch = Scratch::new();
        // The contract references the canary through indirection; the
        // environment carries the value.
        let doc = "---\nreference:\n  answer_key: answer-key.md\n  manifest: reference.yaml\n  \
                    map: map-$TASK5_CANARY.md\nsandbox:\n  policy_dir: policies\n---\n";
        let mut input = scratch.input(Some(doc));
        input.env_pairs = Some(vec![
            (
                "GAUNTLET_PROJECT_ROOT".to_owned(),
                scratch.root.to_string_lossy().into_owned(),
            ),
            ("TASK5_CANARY".to_owned(), "canary-value-xyz".to_owned()),
        ]);
        let (report, path) = run_doctor_and_write(&input).expect("writes");
        let json_text = std::fs::read_to_string(&path).expect("report read");
        let human = report.human_summary();
        for text in [&json_text, &human] {
            assert!(!text.contains("canary-value-xyz"), "{text}");
        }
        // The indirection NAME appears in check 1 evidence; the value never.
        assert!(json_text.contains("TASK5_CANARY"), "{json_text}");
        // The JSON is canonical: sorted keys, two-space indent.
        assert!(json_text.starts_with("{\n  \"checks\""), "{json_text}");
    }

    #[test]
    fn generated_at_is_present_and_the_fingerprint_is_the_domain_shape() {
        let scratch = Scratch::new();
        let report = run_doctor(&scratch.input(Some(&healthy_doc())));
        assert!(report.generated_at > 0);
        let text = report.config_fingerprint.to_string();
        assert!(text.starts_with("sha256:"));
        assert_eq!(text.len(), "sha256:".len() + 64);
    }
}
