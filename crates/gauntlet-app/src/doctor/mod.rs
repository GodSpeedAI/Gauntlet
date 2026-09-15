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
/// application layer does not spell them. The backend measurement hook
/// (plan Task 14) is supplied the same way: the composition root owns the
/// adapters, so only it can MEASURE the configured sandbox backend's
/// achieved isolation rung.
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
    /// Measures the configured backend token (§8.5 check 4, plan Task 14):
    /// given the resolved `sandbox.backend` token, `Some(descriptor)` is a
    /// measurement, `None` means no adapter fronts that token on this
    /// composition. Consulted only when a real backend is configured —
    /// under the §8.6 conservative default the tier posture needs no
    /// measurement.
    pub backend_measure: Option<fn(&str) -> Option<checks::BackendDescriptor>>,
    /// Measures the configured DEFAULT runner (§8.5 check 3, plan Task
    /// 15): given the resolved `runners.default` token, `Some(descriptor)`
    /// is a measurement (harness API-surface version compatibility),
    /// `None` means no adapter fronts that token on this composition.
    /// The composition root owns the adapters, so only it can measure.
    pub runner_measure: Option<fn(&str) -> Option<checks::RunnerDescriptor>>,
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
            backend_measure: None,
            runner_measure: None,
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
            // §8.5 check 3 (plan Task 15): the runner measurement is
            // consulted only when a default runner is actually named —
            // with none configured the honest unavailability stands.
            let named_runner = resolved
                .runners_default
                .as_deref()
                .filter(|name| !name.trim().is_empty());
            let runner_descriptor = named_runner.and_then(|token| {
                let measure = input.runner_measure.as_ref()?;
                measure(token)
            });
            all_checks.push(checks::check_runners(resolved, runner_descriptor.as_ref()));
            // §8.5 check 4 (plan Task 14): the backend measurement is
            // consulted only when a real backend is configured — the
            // conservative default needs none.
            let backend_live = resolved.sandbox_backend.trim() != defaults::SANDBOX_BACKEND;
            let descriptor = if backend_live {
                input
                    .backend_measure
                    .and_then(|measure| measure(&resolved.sandbox_backend))
            } else {
                None
            };
            all_checks.push(checks::check_capabilities(
                resolved,
                descriptor.as_ref(),
                runner_descriptor.as_ref(),
            ));
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

    /// The scratch manifest — a minimal FROZEN-SHAPE manifest that
    /// deep-parses under the strict reader (N1 promoted the fixture: the
    /// clause now runs `parse_manifest`, so the fixture must satisfy it).
    /// The digest clause hashes these exact bytes against the lock's
    /// manifest half.
    fn scratch_manifest() -> String {
        [
            "reference:",
            "  id: scratch-ref-01",
            "  version: 1",
            "  purpose: >",
            "    A scratch reference narrative for the doctor suite.",
            "  desired_state: >",
            "    The scratch destination is reached.",
            "  settlement:",
            "    success_criteria:",
            "      - the scratch destination is reached",
            "    minimum_reliability: >",
            "      spec 19.2 default: >= 1 settlements before promotion.",
            "    terminal_conditions: [settled, aborted]",
            "",
        ]
        .join("\n")
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
        // W7: placement is judged against the RUNTIME workspace root
        // (the state dir's workspaces child), not the configured
        // `workspace.root` — which this doc deliberately points
        // elsewhere (the scratch root) to prove it no longer governs.
        // The state dir is the scratch root, and the key copy inside
        // its workspaces child carries identical bytes (the lock's
        // digest still matches).
        std::fs::create_dir_all(scratch.root.join("workspaces")).expect("runtime root");
        std::fs::copy(
            scratch.root.join("answer-key.md"),
            scratch.root.join("workspaces").join("answer-key.md"),
        )
        .expect("key copied into the runtime root");
        let doc = "---\nreference:\n  answer_key: workspaces/answer-key.md\n  manifest: \
                   reference.yaml\nsandbox:\n  policy_dir: policies\nworkspace:\n  root: \
                   .\nstate:\n  dir: .\n---\n";
        let report = run_doctor(&scratch.input(Some(doc)));
        assert_eq!(status_of(&report, 7), &CheckStatus::Fail);
        assert!(!report.exit_ok());
    }

    /// The OTHER half of check 7's placement clause (Task 13 B3, N4),
    /// re-anchored by W7 onto the runtime workspace root: the answer key
    /// that is lexically OUTSIDE the state dir's workspaces child but
    /// canonically INSIDE it — an alias symlink pointing into the
    /// runtime root. A lexical-only implementation passes this
    /// configuration; the canonicalized half is what refuses it.
    #[test]
    fn an_answer_key_inside_the_workspace_only_canonically_still_fails_check_seven() {
        #[cfg(unix)]
        {
            let scratch = Scratch::new();
            // The real workspace dir lives inside the runtime root (the
            // state dir is the scratch root); `alias` is a symlink to
            // it, so the aliased key path is lexically outside the
            // runtime root but canonicalizes inside it. The key bytes
            // are the scratch key, so the lock's digest still matches
            // wherever the clause reads. The configured
            // `workspace.root` points at the real dir on purpose — it
            // no longer governs placement.
            let real_ws = scratch.root.join("workspaces").join("real-ws");
            std::fs::create_dir_all(&real_ws).expect("workspace dir");
            std::fs::copy(
                scratch.root.join("answer-key.md"),
                real_ws.join("answer-key.md"),
            )
            .expect("key copied behind the alias");
            std::os::unix::fs::symlink(
                std::path::Path::new("workspaces").join("real-ws"),
                scratch.root.join("alias"),
            )
            .expect("alias symlink");
            let doc = "---\nreference:\n  answer_key: alias/answer-key.md\n  manifest: \
                       reference.yaml\nsandbox:\n  policy_dir: policies\nworkspace:\n  root: \
                       real-ws\nstate:\n  dir: .\n---\n";
            let report = run_doctor(&scratch.input(Some(doc)));
            assert_eq!(
                status_of(&report, 7),
                &CheckStatus::Fail,
                "the canonicalized half must refuse the aliased placement: {}",
                report.human_summary()
            );
            assert!(!report.exit_ok());
            let placement = &report.checks[id_to_index(7)]
                .sub
                .iter()
                .find(|clause| clause.name == "answer key placement")
                .expect("placement clause");
            assert_eq!(placement.status, CheckStatus::Fail);
            assert!(
                placement
                    .evidence
                    .iter()
                    .any(|line| line.contains("inside")),
                "the refusal names the inside-workspace finding: {placement:?}"
            );
        }
        #[cfg(not(unix))]
        {
            // The alias channel is a filesystem symlink; the lexical leg
            // is pinned by the test above on every platform.
        }
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
    /// rewrite of the ruler's narrative half — one that still deep-parses,
    /// keeping every shape-level clause green — is caught ONLY by the
    /// recomputed manifest digest, and is a reference-class failure on both
    /// surfaces. (N1 note: the rewrite keeps the frozen shape precisely so
    /// the digest clause stays the only possible locus.)
    #[test]
    fn a_moved_manifest_fails_check_two_as_a_reference_class_failure() {
        let scratch = Scratch::new();
        let moved = scratch_manifest().replace(
            "A scratch reference narrative",
            "A quietly rewritten narrative",
        );
        assert_ne!(&moved, &scratch_manifest(), "the rewrite must move bytes");
        std::fs::write(scratch.root.join("reference.yaml"), moved).expect("moved manifest");
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

    /// N1, vacuity leg: a manifest whose every line is indented carries NO
    /// top-level mapping entry — zero keys is not a mapping, so the manifest
    /// clause must fail. The lock is rewritten TRUE for the doctored bytes,
    /// so the manifest clause is the only possible locus.
    #[test]
    fn an_all_indented_manifest_is_not_a_mapping_and_fails_check_two() {
        let scratch = Scratch::new();
        let doctored = "  reference:\n    id: scratch-ref-01\n    version: 1\n";
        std::fs::write(scratch.root.join("reference.yaml"), doctored).expect("doctored manifest");
        let key_bytes = std::fs::read(scratch.root.join("answer-key.md")).expect("key reads");
        std::fs::write(
            scratch.root.join(".gauntlet").join("reference.lock"),
            lock_text(&key_bytes, doctored.as_bytes(), 2),
        )
        .expect("lock placed");
        let report = run_doctor(&scratch.input(Some(&healthy_doc())));
        assert_eq!(
            status_of(&report, 2),
            &CheckStatus::Fail,
            "an all-indented manifest is not a mapping: {}",
            report.human_summary()
        );
        let check = &report.checks[id_to_index(2)];
        let manifest_clause = check
            .sub
            .iter()
            .find(|clause| clause.name == "manifest")
            .expect("manifest clause");
        assert_eq!(
            manifest_clause.status,
            CheckStatus::Fail,
            "zero top-level keys is a vacuous pass, never a pass: {:?}",
            manifest_clause.evidence
        );
        let digest_clause = check
            .sub
            .iter()
            .find(|clause| clause.name == "digest")
            .expect("digest clause");
        assert_eq!(
            digest_clause.status,
            CheckStatus::Pass,
            "the lock was rewritten true for the doctored bytes, so the \
             manifest clause is the only locus: {:?}",
            digest_clause.evidence
        );
    }

    /// N1, strict-parse leg: a manifest that keeps its top-level mapping
    /// shape but cannot deep-parse under the frozen manifest shape (the
    /// pre-promotion scratch shape: id and version only) fails check 2 as a
    /// reference-class failure carrying the typed parse locus.
    #[test]
    fn a_manifest_that_cannot_deep_parse_fails_check_two_as_a_reference_class_failure() {
        let scratch = Scratch::new();
        let shallow = "reference:\n  id: scratch-ref-01\n  version: 1\n";
        std::fs::write(scratch.root.join("reference.yaml"), shallow).expect("shallow manifest");
        let key_bytes = std::fs::read(scratch.root.join("answer-key.md")).expect("key reads");
        std::fs::write(
            scratch.root.join(".gauntlet").join("reference.lock"),
            lock_text(&key_bytes, shallow.as_bytes(), 2),
        )
        .expect("lock placed");
        let report = run_doctor(&scratch.input(Some(&healthy_doc())));
        assert_eq!(status_of(&report, 2), &CheckStatus::Fail);
        assert!(!report.exit_ok());
        let check = &report.checks[id_to_index(2)];
        let manifest_clause = check
            .sub
            .iter()
            .find(|clause| clause.name == "manifest")
            .expect("manifest clause");
        assert_eq!(
            manifest_clause.status,
            CheckStatus::Fail,
            "a shape-only clause admits a manifest the strict reader refuses: {:?}",
            manifest_clause.evidence
        );
        let evidence = manifest_clause.evidence.join("\n");
        assert!(
            evidence.contains("reference-class"),
            "the failure is reference-class: {evidence}"
        );
        // The typed ManifestError names its locus: the first missing
        // required key of the frozen shape.
        assert!(
            evidence.contains("purpose"),
            "the typed parse error names the missing required key: {evidence}"
        );
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
    // ------------------------------------------------------------------
    // Check 4 (plan Task 14): capability-vs-tier, measured by the
    // composition root when a real backend is configured.
    // ------------------------------------------------------------------

    /// A measured process-rung descriptor for whatever token is asked.
    fn measured_process(token: &str) -> Option<checks::BackendDescriptor> {
        Some(checks::BackendDescriptor {
            token: token.to_owned(),
            isolation: gauntlet_ports::agent_runner::Isolation::Process,
            measured: true,
            note: None,
        })
    }

    /// Writes the three dispatchable tier policy artifacts into the
    /// scratch (check 5 requires them once a backend is configured).
    fn tier_policy_artifacts(scratch: &Scratch) {
        for tier in ["contained", "local", "cross_cutting"] {
            std::fs::write(
                scratch.root.join("policies").join(format!("{tier}.json")),
                "{\"reference\":\"tier-{tier}-v1\",\"tier\":\"{tier}\",\"backend\":\"test-rung\",\"network\":\"deny_by_default\",\"writable_roots\":[\"$WORKSPACE_ROOT\"],\"denied_paths\":[]}",
            )
            .expect("tier policy artifact");
        }
    }

    #[test]
    fn check_four_passes_the_conservative_posture_and_names_the_dispatch_note() {
        let scratch = Scratch::new();
        let report = run_doctor(&scratch.input(Some(&healthy_doc())));
        // The runner half is honestly unavailable (Tasks 15-16), so the
        // check's own status is the unavailable convention check 3 uses;
        // the TIER-RUNG half is the Task-14 fact under test and it passes.
        assert!(matches!(
            status_of(&report, 4),
            CheckStatus::Unavailable { .. }
        ));
        let check = &report.checks[3];
        let tier_rungs = check
            .sub
            .iter()
            .find(|sub| sub.name == "tier rungs")
            .expect("the tier-rung subresult exists");
        assert_eq!(tier_rungs.status, CheckStatus::Pass);
        let joined = tier_rungs.evidence.join(" ");
        assert!(
            joined.contains("only the contained tier is dispatchable"),
            "{joined}"
        );
        assert!(
            joined.contains("refuse dispatch at the governance gate"),
            "{joined}"
        );
        assert!(report.exit_ok(), "{}", report.human_summary());
    }

    #[test]
    fn check_four_fails_a_configured_backend_no_adapter_fronts() {
        let scratch = Scratch::new();
        tier_policy_artifacts(&scratch);
        let mut input = scratch.input(Some(&healthy_doc()));
        input.env_pairs = Some(vec![
            (
                "GAUNTLET_PROJECT_ROOT".to_owned(),
                scratch.root.to_string_lossy().into_owned(),
            ),
            (
                "GAUNTLET_SANDBOX_BACKEND".to_owned(),
                "unfronted-rung".to_owned(),
            ),
        ]);
        input.backend_measure = Some(|_| None);
        let report = run_doctor(&input);
        assert_eq!(status_of(&report, 4), &CheckStatus::Fail);
        let failed = report.required_failure().expect("non-zero exit");
        assert_eq!(failed.id, 4);
        let text = report.human_summary();
        assert!(text.contains("sandbox_capability_mismatch"), "{text}");
    }

    #[test]
    fn check_four_fails_an_unmeasured_or_mistokened_descriptor() {
        let scratch = Scratch::new();
        tier_policy_artifacts(&scratch);
        // Unmeasured: the probe failed — an unusable backend covers no
        // tier.
        let mut input = scratch.input(Some(&healthy_doc()));
        input.env_pairs = Some(vec![
            (
                "GAUNTLET_PROJECT_ROOT".to_owned(),
                scratch.root.to_string_lossy().into_owned(),
            ),
            (
                "GAUNTLET_SANDBOX_BACKEND".to_owned(),
                "probe-broken-rung".to_owned(),
            ),
        ]);
        input.backend_measure = Some(|token| {
            Some(checks::BackendDescriptor {
                token: token.to_owned(),
                isolation: gauntlet_ports::agent_runner::Isolation::None,
                measured: false,
                note: Some("the probe could not run the backend".to_owned()),
            })
        });
        let report = run_doctor(&input);
        assert_eq!(status_of(&report, 4), &CheckStatus::Fail);
        assert!(report.human_summary().contains("could not be measured"));
        // A descriptor naming a DIFFERENT backend than the configuration.
        let mut input = scratch.input(Some(&healthy_doc()));
        input.env_pairs = Some(vec![
            (
                "GAUNTLET_PROJECT_ROOT".to_owned(),
                scratch.root.to_string_lossy().into_owned(),
            ),
            (
                "GAUNTLET_SANDBOX_BACKEND".to_owned(),
                "configured-rung".to_owned(),
            ),
        ]);
        input.backend_measure = Some(|_| {
            Some(checks::BackendDescriptor {
                token: "a-different-rung".to_owned(),
                isolation: gauntlet_ports::agent_runner::Isolation::Vm,
                measured: true,
                note: None,
            })
        });
        let report = run_doctor(&input);
        assert_eq!(status_of(&report, 4), &CheckStatus::Fail);
        assert!(report.human_summary().contains("describes a"));
    }

    #[test]
    fn check_four_covers_the_dispatchable_tiers_with_the_measured_rung() {
        let scratch = Scratch::new();
        tier_policy_artifacts(&scratch);
        let mut input = scratch.input(Some(&healthy_doc()));
        input.env_pairs = Some(vec![
            (
                "GAUNTLET_PROJECT_ROOT".to_owned(),
                scratch.root.to_string_lossy().into_owned(),
            ),
            (
                "GAUNTLET_SANDBOX_BACKEND".to_owned(),
                "measured-rung".to_owned(),
            ),
        ]);
        input.backend_measure = Some(measured_process);
        let report = run_doctor(&input);
        let check = &report.checks[3];
        let tier_rungs = check
            .sub
            .iter()
            .find(|sub| sub.name == "tier rungs")
            .expect("the tier-rung subresult exists");
        assert_eq!(tier_rungs.status, CheckStatus::Pass);
        let joined = tier_rungs.evidence.join(" ");
        for tier in ["contained", "local", "cross_cutting"] {
            assert!(joined.contains(tier), "{joined}");
        }
        assert!(joined.contains("process"), "{joined}");
        // A measured NONE rung cannot cover the above-contained tiers.
        input.backend_measure = Some(|token| {
            Some(checks::BackendDescriptor {
                token: token.to_owned(),
                isolation: gauntlet_ports::agent_runner::Isolation::None,
                measured: true,
                note: None,
            })
        });
        let report = run_doctor(&input);
        assert_eq!(status_of(&report, 4), &CheckStatus::Fail);
        assert!(report.human_summary().contains("below the"));
    }

    // ------------------------------------------------------------------
    // Check 3 resolution (plan Task 15, correction RES-D7): the
    // measured-descriptor pattern for the configured default runner.
    // ------------------------------------------------------------------

    /// A healthy doc that also names a default runner. The token is an
    /// opaque name: this layer cannot spell an adapter identifier (the
    /// denylist), and the check is token-agnostic — the descriptor
    /// echoes whatever token the hook is asked about.
    fn runner_doc() -> String {
        let mut doc = String::from("---\nreference:\n");
        doc.push_str("  answer_key: answer-key.md\n");
        doc.push_str("  manifest: reference.yaml\n");
        doc.push_str("sandbox:\n  policy_dir: policies\n");
        doc.push_str("runners:\n  default: probed-runner\n");
        doc.push_str("---\n");
        doc
    }

    /// A measured-clean descriptor for whatever token is asked.
    fn measured_runner(token: &str) -> Option<checks::RunnerDescriptor> {
        Some(checks::RunnerDescriptor {
            token: token.to_owned(),
            kind: checks::RunnerProbeKind::VersionRange,
            measured: true,
            note: None,
        })
    }

    #[test]
    fn check_three_reports_a_measured_runner_instead_of_the_blanket_unavailable() {
        let scratch = Scratch::new();
        let mut input = scratch.input(Some(&runner_doc()));
        input.runner_measure = Some(measured_runner);
        let report = run_doctor(&input);
        // The resolution half is now MEASURED: the check rolls up to a
        // full pass, not an unavailable.
        assert_eq!(status_of(&report, 3), &CheckStatus::Pass);
        let check = &report.checks[2];
        let resolution = check
            .sub
            .iter()
            .find(|sub| sub.name == "resolution")
            .expect("the resolution subresult exists");
        assert_eq!(resolution.status, CheckStatus::Pass);
        let joined = resolution.evidence.join(" ");
        assert!(joined.contains("probed-runner"), "{joined}");
        assert!(joined.contains("version compatibility"), "{joined}");
        assert!(report.exit_ok(), "{}", report.human_summary());
        // Check 4's runner half is fed from the same measurement: pass,
        // with the measured dimension named and no rung claimed.
        let check_four = &report.checks[3];
        let runner_half = check_four
            .sub
            .iter()
            .find(|sub| sub.name == "runner capabilities")
            .expect("the runner subresult exists");
        assert_eq!(runner_half.status, CheckStatus::Pass);
        let joined = runner_half.evidence.join(" ");
        assert!(joined.contains("version compatibility"), "{joined}");
        assert!(joined.contains("dispatch gate"), "{joined}");
    }

    #[test]
    fn check_three_fails_a_failed_measured_probe_and_so_does_check_four() {
        let scratch = Scratch::new();
        let mut input = scratch.input(Some(&runner_doc()));
        input.runner_measure = Some(|token| {
            Some(checks::RunnerDescriptor {
                token: token.to_owned(),
                kind: checks::RunnerProbeKind::VersionRange,
                measured: false,
                note: Some("runner binary not found at no-such-binary".to_owned()),
            })
        });
        let report = run_doctor(&input);
        // Check 3: a failed measurement is a configuration failure with
        // the probe's locus — never an unavailable-inside-pass.
        assert_eq!(status_of(&report, 3), &CheckStatus::Fail);
        let failed = report.required_failure().expect("non-zero exit");
        assert_eq!(failed.id, 3);
        let text = report.human_summary();
        assert!(text.contains("runner_probe_failed"), "{text}");
        assert!(text.contains("not found"), "{text}");
        // Check 4's runner half fails on the same measurement.
        assert_eq!(status_of(&report, 4), &CheckStatus::Fail);
        assert!(report.human_summary().contains("runner_probe_failed"));
    }

    #[test]
    fn check_three_fails_a_mistokened_descriptor() {
        let scratch = Scratch::new();
        let mut input = scratch.input(Some(&runner_doc()));
        input.runner_measure = Some(|_| {
            Some(checks::RunnerDescriptor {
                token: "a-different-runner".to_owned(),
                kind: checks::RunnerProbeKind::VersionRange,
                measured: true,
                note: None,
            })
        });
        let report = run_doctor(&input);
        assert_eq!(status_of(&report, 3), &CheckStatus::Fail);
        let text = report.human_summary();
        assert!(text.contains("runner_resolution_mismatch"), "{text}");
        assert!(text.contains("different runner"), "{text}");
    }

    #[test]
    fn check_three_stays_unavailable_without_a_measurement_hook() {
        let scratch = Scratch::new();
        // A runner is named, but the composition supplies no measurement
        // hook: the honest unavailability stands (nothing was measured).
        let report = run_doctor(&scratch.input(Some(&runner_doc())));
        assert!(matches!(
            status_of(&report, 3),
            CheckStatus::Unavailable { .. }
        ));
        let check = &report.checks[2];
        let resolution = check
            .sub
            .iter()
            .find(|sub| sub.name == "resolution")
            .expect("the resolution subresult exists");
        let joined = resolution.evidence.join(" ");
        assert!(joined.contains("measured no descriptor"), "{joined}");
        // And with no runner configured at all, the historical reason
        // stands.
        let report = run_doctor(&scratch.input(Some(&healthy_doc())));
        assert!(matches!(
            status_of(&report, 3),
            CheckStatus::Unavailable { .. }
        ));
    }
}
