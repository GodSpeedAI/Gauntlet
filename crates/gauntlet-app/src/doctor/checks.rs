//! The eight §8.5 preflight checks (plan Task 5, decisions D5/D6/D16).
//!
//! Each check is a callable function over shared context so the Task-12
//! controller can re-run the drift subset at iteration boundaries (§8.5
//! "re-checked at every iteration boundary for the subset that can drift").
//! Scope honesty for this task is per decision D6: what is fully real now,
//! what is `unavailable` because its machinery lands with a later plan task,
//! and what is `skipped` because a precondition is false — never a silent
//! pass (I8).
//!
//! Evidence strings carry NAMES — variables, keys, file leaves — never
//! values: a resolved path or credential behind a `$VAR` reference is not
//! printable evidence (§8.1).

use std::path::Path;

use gauntlet_ports::json::{self, JsonValue};

use crate::config::defaults;
use crate::config::env_layer::UnknownEnv;
use crate::config::error::ConfigError;
use crate::config::resolve::{ResolvedConfig, UnknownKey};
use crate::doctor::report::{CheckOutcome, CheckStatus, SubResult};

/// Shared inputs for the checks that need more than the resolved products.
#[derive(Debug, Clone)]
pub struct DoctorContext {
    pub resolved: ResolvedConfig,
    pub unknown_keys: Vec<UnknownKey>,
    pub unknown_env: Vec<UnknownEnv>,
    /// The §8.4 semantic-capability vocabulary, supplied by the composition
    /// root (see `env_layer`'s header for why vocabularies are supplied).
    pub capabilities: &'static [&'static str],
}

// ---------------------------------------------------------------------------
// Check 1 — config parses; unknown keys reported; indirections resolve;
// secrets present but unprinted (§8.5 check 1)
// ---------------------------------------------------------------------------

/// What check 1 needs beyond [`DoctorContext`].
#[derive(Debug, Clone)]
pub struct ConfigCheckInput {
    /// Parse or merge failure, when there was one.
    pub config_error: Option<ConfigError>,
    /// Indirection-resolution failure, when there was one.
    pub products_error: Option<ConfigError>,
    /// Where the contract was found, when it was.
    pub config_origin: Option<String>,
    pub unknown_keys: Vec<UnknownKey>,
    pub unknown_env: Vec<UnknownEnv>,
    /// Variable NAMES whose values were expanded.
    pub indirections_used: Vec<String>,
}

pub fn check_config(input: &ConfigCheckInput) -> CheckOutcome {
    let parse_status = match &input.config_error {
        None => CheckStatus::Pass,
        Some(_) => CheckStatus::Fail,
    };
    let parse_evidence = match &input.config_error {
        None => vec![match &input.config_origin {
            Some(origin) => format!("contract found and parsed at {origin}"),
            None => "no GAUNTLET.md found; the environment fully specifies the run (spec 8.2)"
                .to_owned(),
        }],
        Some(err) => vec![format!("configuration did not resolve: {err}")],
    };
    let unknown_key_evidence = if input.unknown_keys.is_empty() {
        vec!["no unknown front-matter keys".to_owned()]
    } else {
        input
            .unknown_keys
            .iter()
            .map(|unknown| format!("unknown key '{}' at line {}", unknown.path, unknown.line))
            .collect()
    };
    let unknown_env_evidence = if input.unknown_env.is_empty() {
        vec!["no unknown environment variables".to_owned()]
    } else {
        input
            .unknown_env
            .iter()
            .map(|unknown| {
                format!(
                    "unknown variable '{}' from the {} (outside Annex E)",
                    unknown.name,
                    match unknown.source {
                        crate::config::env_layer::EnvSource::DotEnv => "dotenv file",
                        crate::config::env_layer::EnvSource::Process => "process environment",
                    }
                )
            })
            .collect()
    };
    // The two resolution-product clauses share one honesty rule: when the
    // configuration itself did not resolve (parse/merge/read failure), the
    // product clauses say so instead of claiming an empty accounting.
    let degraded = input.config_error.is_some();
    let indirection_evidence = match (&input.products_error, degraded) {
        (None, false) => {
            let mut lines = vec![format!(
                "{} indirection(s) resolved{}",
                input.indirections_used.len(),
                if input.indirections_used.is_empty() {
                    String::new()
                } else {
                    format!(": {}", input.indirections_used.join(", "))
                }
            )];
            if input.indirections_used.is_empty() {
                lines.push("no indirections configured".to_owned());
            } else {
                lines.push("values present but never printed (spec 8.1 secrets rule)".to_owned());
            }
            lines
        }
        (Some(err @ ConfigError::UnresolvedIndirection { .. }), _) => {
            vec![format!("indirection failed: {err}")]
        }
        (Some(_), _) => vec![
            "indirection accounting stopped at a config value failure — see 'config values'"
                .to_owned(),
        ],
        (None, true) => {
            vec!["indirection expansion did not run — the configuration did not resolve".to_owned()]
        }
    };
    // Value-conversion failures are their OWN failure locus (the error-
    // handling contract: preserve the locus, do not re-badge a value error
    // as an indirection failure). Evidence names the key and reason; a value
    // that arrived through an indirection is never echoed.
    let (value_status, value_evidence) = match (&input.products_error, degraded) {
        (None, false) => (
            CheckStatus::Pass,
            vec!["every config value converted into its domain".to_owned()],
        ),
        (None, true) => (
            CheckStatus::Skipped {
                reason: "configuration did not resolve; value conversion did not run".to_owned(),
            },
            vec!["value conversion did not run — the configuration did not resolve".to_owned()],
        ),
        (Some(ConfigError::UnresolvedIndirection { .. }), _) => (
            CheckStatus::Skipped {
                reason: "an indirection failed before value conversion ran".to_owned(),
            },
            vec!["value conversion did not run — an indirection failed first".to_owned()],
        ),
        (Some(err), _) => (
            CheckStatus::Fail,
            vec![format!("config value rejected: {err}")],
        ),
    };
    CheckOutcome::combine(
        1,
        "config",
        CheckStatus::Pass,
        vec![
            SubResult {
                name: "front matter parses".to_owned(),
                status: parse_status,
                evidence: parse_evidence,
            },
            SubResult {
                name: "unknown keys".to_owned(),
                // Forward compatibility (§8.2): reported, never fatal.
                status: CheckStatus::Pass,
                evidence: unknown_key_evidence,
            },
            SubResult {
                name: "unknown environment".to_owned(),
                status: CheckStatus::Pass,
                evidence: unknown_env_evidence,
            },
            SubResult {
                name: "config values".to_owned(),
                status: value_status,
                evidence: value_evidence,
            },
            SubResult {
                name: "indirections".to_owned(),
                status: match (&input.products_error, degraded) {
                    (None, false) => CheckStatus::Pass,
                    (Some(ConfigError::UnresolvedIndirection { .. }), _) => CheckStatus::Fail,
                    (Some(_), _) | (None, true) => CheckStatus::Skipped {
                        reason: "product resolution did not complete; indirection accounting is \
                                 not claimed"
                            .to_owned(),
                    },
                },
                evidence: indirection_evidence,
            },
        ],
        Vec::new(),
    )
}

// ---------------------------------------------------------------------------
// Check 2 — reference manifest and answer key (§8.5 check 2)
// ---------------------------------------------------------------------------

pub fn check_reference(resolved: &ResolvedConfig) -> CheckOutcome {
    // Manifest: exists, and its top level is a mapping.
    let manifest_status;
    let mut manifest_evidence = Vec::new();
    match std::fs::read_to_string(&resolved.reference.manifest) {
        Err(err) => {
            manifest_status = CheckStatus::Fail;
            manifest_evidence.push(format!(
                "reference manifest not readable at {} ({})",
                leaf_of(&resolved.reference.manifest),
                err.kind()
            ));
        }
        Ok(text) => match top_level_mapping_keys(&text) {
            Err(line) => {
                manifest_status = CheckStatus::Fail;
                manifest_evidence.push(format!(
                    "manifest top level is not a mapping (line {line} is not a `key:` entry)"
                ));
            }
            Ok(keys) => {
                manifest_status = CheckStatus::Pass;
                manifest_evidence.push(format!(
                    "manifest exists; top level is a mapping ({keys} keys)"
                ));
            }
        },
    }
    // Answer key: exists, non-empty, block count equals the frozen count.
    let lock_path = defaults::reference_lock_at(&resolved.project_root);
    let expected_count = match read_lock_unit_count(&lock_path) {
        Ok(count) => count,
        Err(detail) => {
            return CheckOutcome::combine(
                2,
                "reference",
                CheckStatus::Fail,
                vec![
                    SubResult {
                        name: "manifest".to_owned(),
                        status: manifest_status,
                        evidence: manifest_evidence,
                    },
                    SubResult {
                        name: "answer key".to_owned(),
                        status: CheckStatus::Fail,
                        evidence: vec![detail],
                    },
                    SubResult {
                        name: "unit schema".to_owned(),
                        status: CheckStatus::Unavailable {
                            reason: UNIT_SCHEMA_LATER.to_owned(),
                        },
                        evidence: vec![UNIT_SCHEMA_LATER.to_owned()],
                    },
                    SubResult {
                        name: "digest".to_owned(),
                        status: CheckStatus::Unavailable {
                            reason: DIGEST_LATER.to_owned(),
                        },
                        evidence: vec![DIGEST_LATER.to_owned()],
                    },
                ],
                Vec::new(),
            );
        }
    };
    let mut answer_evidence = Vec::new();
    let answer_status = match std::fs::read_to_string(&resolved.reference.answer_key) {
        Err(err) => {
            answer_evidence.push(format!(
                "answer key not readable at {} ({})",
                leaf_of(&resolved.reference.answer_key),
                err.kind()
            ));
            CheckStatus::Fail
        }
        Ok(text) if text.trim().is_empty() => {
            answer_evidence.push("answer key is empty".to_owned());
            CheckStatus::Fail
        }
        Ok(text) => {
            let count = text
                .lines()
                .filter(|line| line.starts_with("## AK-"))
                .count();
            if count == expected_count {
                answer_evidence.push(format!(
                    "answer key exists, non-empty, {count} units (frozen count {expected_count})"
                ));
                CheckStatus::Pass
            } else {
                answer_evidence.push(format!(
                    "answer key holds {count} unit blocks; the frozen record declares \
                     {expected_count}"
                ));
                CheckStatus::Fail
            }
        }
    };
    CheckOutcome::combine(
        2,
        "reference",
        CheckStatus::Pass,
        vec![
            SubResult {
                name: "manifest".to_owned(),
                status: manifest_status,
                evidence: manifest_evidence,
            },
            SubResult {
                name: "answer key".to_owned(),
                status: answer_status,
                evidence: answer_evidence,
            },
            SubResult {
                name: "unit schema".to_owned(),
                status: CheckStatus::Unavailable {
                    reason: UNIT_SCHEMA_LATER.to_owned(),
                },
                evidence: vec![format!(
                    "per-Unit validation against 5.3.2 (including the tier/authority coupling) \
                     is unavailable: {UNIT_SCHEMA_LATER}"
                )],
            },
            SubResult {
                name: "digest".to_owned(),
                status: CheckStatus::Unavailable {
                    reason: DIGEST_LATER.to_owned(),
                },
                evidence: vec![format!(
                    "digest-vs-lock verification is unavailable: {DIGEST_LATER}"
                )],
            },
        ],
        Vec::new(),
    )
}

const UNIT_SCHEMA_LATER: &str = "unit-schema validation lands with the ReferenceProvider parser \
     (plan Task 7)";
const DIGEST_LATER: &str = "verify_unchanged lands with plan Task 7";

/// The file leaf of a path, for evidence that must not carry resolved
/// directories.
fn leaf_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Confirms a document's top level is a mapping: every non-blank,
/// non-comment, column-zero line is a `key:` entry. Returns the count of
/// top-level keys. Full YAML is NOT claimed — the manifest uses constructs
/// (block scalars) outside the §8.2 grammar on purpose.
fn top_level_mapping_keys(text: &str) -> Result<usize, usize> {
    let mut keys = 0usize;
    for (index, line) in text.lines().enumerate() {
        let trimmed = line.trim_start();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }
        if line.starts_with(char::is_whitespace) {
            continue;
        }
        let ident_end = trimmed
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.'))
            .unwrap_or(trimmed.len());
        let is_key = ident_end > 0
            && trimmed.as_bytes()[0] != b'-'
            && trimmed
                .get(ident_end..)
                .is_some_and(|rest| rest.starts_with(':'));
        if !is_key {
            return Err(index + 1);
        }
        keys += 1;
    }
    Ok(keys)
}

/// Reads `unit_count` from the frozen reference record (§9.4). The lock is
/// the freeze record; digest verification against it is Task 7's.
fn read_lock_unit_count(lock_path: &Path) -> Result<usize, String> {
    let text = std::fs::read_to_string(lock_path).map_err(|err| {
        format!(
            "reference record not readable at {} ({}) — the frozen count criterion needs it",
            leaf_of(lock_path),
            err.kind()
        )
    })?;
    let value =
        json::parse(&text).map_err(|err| format!("reference record is not valid JSON: {err}"))?;
    let count = value
        .get("unit_count")
        .and_then(|field| match field {
            JsonValue::Number(json::JsonNumber::Uint(n)) => Some(*n),
            _ => None,
        })
        .ok_or_else(|| "reference record carries no unit_count".to_owned())?;
    usize::try_from(count).map_err(|_| "reference record unit_count is negative".to_owned())
}

// ---------------------------------------------------------------------------
// Check 3 — a runner resolves for every dispatchable role (§8.5 check 3)
// ---------------------------------------------------------------------------

pub fn check_runners(resolved: &ResolvedConfig) -> CheckOutcome {
    let has_default = resolved
        .runners_default
        .as_deref()
        .is_some_and(|name| !name.trim().is_empty());
    let roles: Vec<&String> = resolved.runners_by_role.keys().collect();
    let (structure_status, structure_evidence) = if !has_default && roles.is_empty() {
        (
            CheckStatus::Skipped {
                reason: "no runners configured (UNK-003)".to_owned(),
            },
            vec![
                "no runners configured (UNK-003): the default runner is an open unknown; \
                  selection arrives via the runner variables in later tasks"
                    .to_owned(),
            ],
        )
    } else {
        let mut evidence = Vec::new();
        let mut ok = true;
        if resolved.runners_default.is_some() && !has_default {
            ok = false;
            evidence.push("runners.default is configured but empty".to_owned());
        } else if has_default {
            evidence.push("runners.default configured".to_owned());
        }
        for role in &roles {
            let entry = &resolved.runners_by_role[*role];
            if entry.runner.trim().is_empty() {
                ok = false;
                evidence.push(format!("role '{role}' has an empty runner name"));
            } else {
                evidence.push(format!(
                    "role '{role}' configured (runner name present, {} model binding)",
                    if entry.model.is_some() {
                        "with"
                    } else {
                        "without"
                    }
                ));
            }
        }
        (
            if ok {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            evidence,
        )
    };
    let resolution_reason = "runner adapters land with plan Tasks 15–16";
    CheckOutcome::combine(
        3,
        "runners",
        CheckStatus::Pass,
        vec![
            SubResult {
                name: "structure".to_owned(),
                status: structure_status,
                evidence: structure_evidence,
            },
            SubResult {
                name: "resolution".to_owned(),
                status: CheckStatus::Unavailable {
                    reason: resolution_reason.to_owned(),
                },
                evidence: vec![format!(
                    "runtime resolution against a runner registry is unavailable: \
                     {resolution_reason}"
                )],
            },
        ],
        Vec::new(),
    )
}

// ---------------------------------------------------------------------------
// Check 4 — runner capabilities satisfy configured tiers (§8.5 check 4)
// ---------------------------------------------------------------------------

pub fn check_capabilities(resolved: &ResolvedConfig) -> CheckOutcome {
    let _ = resolved;
    let reason = "capability-vs-tier dispatch check lands with plan Task 14; Isolation ladder exists in \
         ports";
    CheckOutcome::combine(
        4,
        "capabilities",
        CheckStatus::Unavailable {
            reason: reason.to_owned(),
        },
        Vec::new(),
        vec![reason.to_owned()],
    )
}

// ---------------------------------------------------------------------------
// Check 5 — sandbox policies exist per tier; none for the top tier
// (§8.5 check 5, §17.3, AK-049)
// ---------------------------------------------------------------------------

pub fn check_policies(resolved: &ResolvedConfig, raw_policy_dir: &str) -> CheckOutcome {
    let dir = &resolved.sandbox_policy_dir;
    // The forbidden tier's policy file: its PRESENCE is a configuration
    // error even when no backend is configured (§17.3, AK-049). The probe is
    // deliberately spelling- and location-robust:
    //
    // - BOTH the effective policy dir and the default dir are scanned, each
    //   reported as its own evidence line — overriding `sandbox.policy_dir`
    //   must not hide a forbidden file sitting in the default location;
    // - ANY directory entry whose file stem is `irreversible` in any casing
    //   counts, whatever its extension (.json, .yaml, .toml, none);
    // - links are NOT followed (`symlink_metadata`): a broken symlink named
    //   for the tier is still a present name;
    // - an UNREADABLE directory (e.g. mode 000) fails the clause — only a
    //   successfully-read empty directory, or no directory at all, is clean.
    let mut forbidden_evidence = Vec::new();
    let mut forbidden_hit = false;
    let mut unreadable_hit = false;
    let mut scanned_dirs = vec![dir.clone()];
    let default_dir = defaults::policy_dir_at(&resolved.project_root);
    if !scanned_dirs.contains(&default_dir) {
        scanned_dirs.push(default_dir);
    }
    for scan in &scanned_dirs {
        // The raw token belongs to the EFFECTIVE dir only; the default dir
        // is a fixed layout spelling.
        let raw = (scan == dir).then_some(raw_policy_dir);
        let label = policy_dir_label(scan, raw, &resolved.project_root);
        match forbidden_entries_in(scan) {
            DirScan::Read(hits) if hits.is_empty() => forbidden_evidence.push(format!(
                "no policy file for the top tier in policy dir '{label}' (AK-049)"
            )),
            DirScan::Read(hits) => {
                forbidden_hit = true;
                for name in hits {
                    forbidden_evidence.push(format!(
                        "a policy file exists for the top tier ('{name}' in policy dir \
                         '{label}') — its presence is a configuration error (AK-049, spec 17.3)"
                    ));
                }
            }
            // No directory to scan: the honest clean absence.
            DirScan::Absent => forbidden_evidence.push(format!(
                "no policy file for the top tier in policy dir '{label}' (AK-049)"
            )),
            // An unreadable directory (e.g. mode 000) is NEVER clean: the
            // scan cannot run, so the clause cannot be satisfied.
            DirScan::Unreadable(kind) => {
                unreadable_hit = true;
                forbidden_evidence.push(format!(
                    "policy dir unreadable: '{label}' could not be read ({kind}) — an \
                     unreadable directory is never clean; the forbidden-tier scan did not run \
                     (spec 8.5 check 5)"
                ));
            }
        }
    }
    let forbidden_status = if forbidden_hit || unreadable_hit {
        CheckStatus::Fail
    } else {
        CheckStatus::Pass
    };
    // The dispatchable tiers' policy files: required only with a backend.
    let backend_live = resolved.sandbox_backend.trim() != defaults::SANDBOX_BACKEND;
    let required_tiers = ["contained", "local", "cross_cutting"];
    let (existence_status, existence_evidence) = if !backend_live {
        (
            CheckStatus::Skipped {
                reason: "sandbox backend is the conservative default; the existence clause \
                         applies when a backend is configured"
                    .to_owned(),
            },
            vec![format!(
                "sandbox backend resolves to '{}'; policy files for {} are required only when a \
                 backend is configured (spec 8.5 check 5)",
                resolved.sandbox_backend,
                required_tiers.join(", ")
            )],
        )
    } else {
        let mut evidence = Vec::new();
        let mut ok = true;
        for tier in required_tiers {
            let file = dir.join(format!("{tier}.json"));
            if file.exists() {
                evidence.push(format!("policy file present for tier '{tier}'"));
            } else {
                ok = false;
                evidence.push(format!(
                    "policy file missing for tier '{tier}' ({tier}.json) while a backend is \
                     configured (spec 8.5 check 5)"
                ));
            }
        }
        (
            if ok {
                CheckStatus::Pass
            } else {
                CheckStatus::Fail
            },
            evidence,
        )
    };
    CheckOutcome::combine(
        5,
        "sandbox policies",
        CheckStatus::Pass,
        vec![
            SubResult {
                name: "no top-tier policy".to_owned(),
                status: forbidden_status,
                evidence: forbidden_evidence,
            },
            SubResult {
                name: "tier policies exist".to_owned(),
                status: existence_status,
                evidence: existence_evidence,
            },
        ],
        Vec::new(),
    )
}

/// The outcome of scanning one policy directory for forbidden-tier entries
/// (AK-049). Only a successfully-read directory may satisfy the clause: a
/// read failure is reported, never flattened into an empty (clean) scan —
/// a mode-000 directory must not launder a forbidden file into "clean".
enum DirScan {
    /// The read succeeded; the forbidden entry names, possibly empty.
    Read(Vec<String>),
    /// The directory does not exist: the honest clean absence.
    Absent,
    /// The directory is present but could not be read. Carries the io
    /// error KIND, never contents.
    Unreadable(std::io::ErrorKind),
}

/// The forbidden-tier directory entries in `dir`, sorted: every entry whose
/// file stem is `irreversible` in any casing, whatever its extension or lack
/// of one. `symlink_metadata` confirms presence WITHOUT following links, so
/// a broken symlink named for the tier still counts (AK-049).
fn forbidden_entries_in(dir: &Path) -> DirScan {
    let entries = match std::fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => return DirScan::Absent,
        Err(err) => return DirScan::Unreadable(err.kind()),
    };
    let mut names: Vec<String> = entries
        .flatten()
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let stem_matches = Path::new(&name)
                .file_stem()
                .is_some_and(|stem| stem.eq_ignore_ascii_case("irreversible"));
            if stem_matches && std::fs::symlink_metadata(entry.path()).is_ok() {
                Some(name)
            } else {
                None
            }
        })
        .collect();
    names.sort();
    DirScan::Read(names)
}

/// Evidence-safe policy-dir label: a literal (non-indirected) dir shows its
/// root-relative spelling; an indirected one shows the raw token and
/// withholds the resolved value (spec 8.1 secrets rule — a resolved path may
/// embed an environment value). The default policy dir is never indirected
/// (`None`), so it always shows its layout spelling.
fn policy_dir_label(dir: &Path, raw_token: Option<&str>, project_root: &Path) -> String {
    if let Some(token) = raw_token.filter(|token| token.contains('$')) {
        return format!("{token} (resolved for probing; value withheld)");
    }
    match dir.strip_prefix(project_root) {
        Ok(rel) if !rel.as_os_str().is_empty() => rel.to_string_lossy().into_owned(),
        _ => leaf_of(dir),
    }
}

// ---------------------------------------------------------------------------
// Check 6 — required semantic capabilities resolve (§8.5 check 6, §8.4)
// ---------------------------------------------------------------------------

pub fn check_required_capabilities(
    required: &[String],
    vocabulary: &'static [&'static str],
) -> CheckOutcome {
    if required.is_empty() {
        return CheckOutcome::combine(
            6,
            "required capabilities",
            CheckStatus::Pass,
            Vec::new(),
            vec!["no required capabilities configured".to_owned()],
        );
    }
    let mut evidence = Vec::new();
    let mut ok = true;
    for name in required {
        if vocabulary.contains(&name.as_str()) {
            evidence.push(format!("'{name}' is in the 8.4 vocabulary"));
        } else {
            ok = false;
            evidence.push(format!(
                "'{name}' is not one of the spec 8.4 semantic capabilities"
            ));
        }
    }
    let binding_reason = "tool registry lands with plan Tasks 15–16";
    CheckOutcome::combine(
        6,
        "required capabilities",
        CheckStatus::Pass,
        vec![
            SubResult {
                name: "vocabulary".to_owned(),
                status: if ok {
                    CheckStatus::Pass
                } else {
                    CheckStatus::Fail
                },
                evidence,
            },
            SubResult {
                name: "tool binding".to_owned(),
                status: CheckStatus::Unavailable {
                    reason: binding_reason.to_owned(),
                },
                evidence: vec![format!(
                    "whether a tool binding resolves for each capability is unavailable: \
                     {binding_reason}"
                )],
            },
        ],
        Vec::new(),
    )
}

// ---------------------------------------------------------------------------
// Check 7 — writable dirs; answer key outside workspaces (§8.5 check 7)
// ---------------------------------------------------------------------------

pub fn check_paths(resolved: &ResolvedConfig, raw_tokens: &RawTokens) -> CheckOutcome {
    let state = probe_writable(&resolved.state_dir, "state dir", &raw_tokens.state_dir);
    let evidence_store = probe_writable(
        &resolved.evidence_dir,
        "evidence dir",
        &raw_tokens.evidence_dir,
    );
    let workspace = probe_writable(
        &resolved.workspace_root,
        "workspace root",
        &raw_tokens.workspace_root,
    );
    let answer_key = check_answer_key_outside(resolved, raw_tokens);
    CheckOutcome::combine(
        7,
        "writable paths",
        CheckStatus::Pass,
        vec![state, evidence_store, workspace, answer_key],
        Vec::new(),
    )
}

/// The raw token forms, so evidence can withhold values behind `$VAR`
/// references while still naming the setting.
#[derive(Debug, Clone, Default)]
pub struct RawTokens {
    pub state_dir: String,
    pub evidence_dir: String,
    pub workspace_root: String,
    pub answer_key: String,
}

/// Evidence-safe display: a raw token carrying indirection shows the token
/// and says the value is withheld; a literal shows the resolved path.
fn display_path(role: &str, raw: &str, resolved: &Path) -> String {
    if raw.contains('$') {
        format!("{role} = {raw} (resolved for probing; value withheld)")
    } else {
        format!("{role} = {}", resolved.display())
    }
}

/// Probes one directory: create it (and parents) if absent, then create and
/// remove one scratch file. Failure evidence carries the error kind, never a
/// path that might embed a secret value.
fn probe_writable(dir: &Path, role: &str, raw: &str) -> SubResult {
    let display = display_path(role, raw, dir);
    let outcome = (|| -> Result<(), std::io::ErrorKind> {
        std::fs::create_dir_all(dir).map_err(|err| err.kind())?;
        let scratch = dir.join(format!(
            "doctor-probe-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|since| since.as_nanos())
                .unwrap_or(0),
        ));
        std::fs::write(&scratch, b"probe").map_err(|err| err.kind())?;
        std::fs::remove_file(&scratch).map_err(|err| err.kind())?;
        Ok(())
    })();
    match outcome {
        Ok(()) => SubResult {
            name: role.to_owned(),
            status: CheckStatus::Pass,
            evidence: vec![format!("{display}: created, wrote, removed a scratch file")],
        },
        Err(kind) => SubResult {
            name: role.to_owned(),
            status: CheckStatus::Fail,
            evidence: vec![format!("{display}: not writable ({kind})")],
        },
    }
}

/// The answer key must sit outside every builder-writable workspace,
/// lexically AND on the canonicalized filesystem (§8.5 check 7, §22 item 5).
fn check_answer_key_outside(resolved: &ResolvedConfig, raw_tokens: &RawTokens) -> SubResult {
    let answer = &resolved.reference.answer_key;
    let workspace = &resolved.workspace_root;
    let lexical_inside = answer.starts_with(workspace);
    let (canonical_inside, canonical_note) = match (answer.canonicalize(), workspace.canonicalize())
    {
        (Ok(answer_real), Ok(workspace_real)) => {
            let inside = answer_real.starts_with(&workspace_real);
            (inside, None)
        }
        (Err(err), _) => (
            false,
            Some(format!(
                "answer key could not be canonicalized ({}) — a finding, not a pass",
                err.kind()
            )),
        ),
        (_, Err(err)) => (
            false,
            Some(format!(
                "workspace root could not be canonicalized ({}) — a finding, not a pass",
                err.kind()
            )),
        ),
    };
    let mut status = CheckStatus::Pass;
    let mut evidence = vec![display_path("answer key", &raw_tokens.answer_key, answer)];
    if lexical_inside || canonical_inside {
        status = CheckStatus::Fail;
        evidence.push(
            "answer key sits inside a builder-writable workspace — the grader must not be \
             writable by the graded (spec 8.5 check 7, 22 item 5)"
                .to_owned(),
        );
    } else {
        evidence.push(
            "answer key outside every builder-writable workspace (lexical and canonical)"
                .to_owned(),
        );
    }
    if let Some(note) = canonical_note {
        status = CheckStatus::Fail;
        evidence.push(note);
    }
    SubResult {
        name: "answer key placement".to_owned(),
        status,
        evidence,
    }
}

// ---------------------------------------------------------------------------
// Check 8 — budget dimensions parse, non-zero; reserve < 100 (§8.5 check 8)
// ---------------------------------------------------------------------------

/// Check 8's §8.5 id, as a named constant: the budget statement also names
/// the wall-seconds field, and a bare number beside a duration word is the
/// boundary suite's timeout class (spec §8.1).
const BUDGET_CHECK_ID: u8 = 8;

pub fn check_budget(resolved: &ResolvedConfig) -> CheckOutcome {
    let mut evidence = Vec::new();
    let limits = {
        let attempt = resolved.budget.limits();
        match attempt {
            Ok(limits) => {
                evidence.push("domain Limits constructed from the effective budget".to_owned());
                Some(limits)
            }
            Err(err) => {
                evidence.push(format!("budget ceilings refused by the domain: {err}"));
                None
            }
        }
    };
    let pct = resolved.budget.recovery_reserve_pct;
    let reserve_status = if pct == 0 {
        evidence.push(format!(
            "recovery reserve of {pct} percent is not a real portion (spec 8.5 check 8)"
        ));
        CheckStatus::Fail
    } else if pct >= 100 {
        evidence.push(format!(
            "recovery reserve of {pct} percent is not below one hundred (spec 8.5 check 8)"
        ));
        CheckStatus::Fail
    } else {
        evidence.push(format!("recovery reserve {pct} percent, below one hundred"));
        CheckStatus::Pass
    };
    let diagnostic = resolved.budget.diagnostic_budget;
    let diagnostic_status = if diagnostic == 0 {
        evidence.push(
            "diagnostic budget of zero is not bounded diagnosis (spec 8.5 check 8)".to_owned(),
        );
        CheckStatus::Fail
    } else {
        evidence.push(format!("diagnostic budget {diagnostic}"));
        CheckStatus::Pass
    };
    let ceilings_status = if limits.is_some() {
        CheckStatus::Pass
    } else {
        CheckStatus::Fail
    };
    CheckOutcome::combine(
        BUDGET_CHECK_ID,
        "budget",
        CheckStatus::Pass,
        vec![
            SubResult {
                name: "ceilings".to_owned(),
                status: ceilings_status,
                evidence: vec![format!(
                    "rounds {} parallel {} depth {} wall {} tokens {} micro-usd {}",
                    resolved.budget.max_rounds,
                    resolved.budget.max_parallel,
                    resolved.budget.max_depth,
                    resolved.budget.max_wall_seconds,
                    resolved.budget.max_tokens,
                    resolved.budget.max_cost_usd.get()
                )],
            },
            SubResult {
                name: "recovery reserve".to_owned(),
                status: reserve_status,
                evidence: Vec::new(),
            },
            SubResult {
                name: "diagnostic".to_owned(),
                status: diagnostic_status,
                evidence: Vec::new(),
            },
        ],
        evidence,
    )
}

/// Builds one unavailable outcome for the degraded path (configuration did
/// not resolve; check 1 carries the failure).
pub fn degraded(id: u8, name: &'static str) -> CheckOutcome {
    let reason = "configuration unresolved — check 1 carries the failure".to_owned();
    CheckOutcome::combine(
        id,
        name,
        CheckStatus::Unavailable {
            reason: reason.clone(),
        },
        Vec::new(),
        vec![reason],
    )
}
