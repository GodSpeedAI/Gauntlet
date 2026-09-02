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

use gauntlet_domain::Digest;
use gauntlet_ports::agent_runner::Isolation;
use gauntlet_ports::json::{self, JsonValue};

use crate::config::defaults;
use crate::config::env_layer::UnknownEnv;
use crate::config::error::ConfigError;
use crate::config::resolve::{ResolvedConfig, UnknownKey};
use crate::doctor::report::{CheckOutcome, CheckStatus, SubResult};
use crate::govern::posture::dispatchable_posture;
use crate::reference::manifest::parse_manifest;
use crate::reference::normalize::digest_of;
use crate::reference::parse::parse_answer_key;

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
    // Manifest: exists, its top level is a mapping, and it deep-parses
    // under the STRICT frozen-shape reader (`reference::manifest`) — the
    // same parser the provider path uses at admission (N1: a shape-only
    // clause is vacuous; `Ok(0)` top-level keys and a manifest the strict
    // reader refuses both passed it). The raw bytes are read ONCE here and
    // shared with the digest clause, which recomputes the manifest's
    // normalized digest against the freeze record's other half.
    let manifest_bytes = std::fs::read(&resolved.reference.manifest);
    let manifest_status;
    let mut manifest_evidence = Vec::new();
    match &manifest_bytes {
        Err(err) => {
            manifest_status = CheckStatus::Fail;
            manifest_evidence.push(format!(
                "reference manifest not readable at {} ({})",
                leaf_of(&resolved.reference.manifest),
                err.kind()
            ));
        }
        Ok(bytes) => match std::str::from_utf8(bytes) {
            Err(err) => {
                manifest_status = CheckStatus::Fail;
                manifest_evidence.push(format!(
                    "reference manifest is not valid UTF-8 (first invalid byte at {})",
                    err.valid_up_to()
                ));
            }
            Ok(text) => match top_level_mapping_keys(text) {
                Err(line) => {
                    manifest_status = CheckStatus::Fail;
                    manifest_evidence.push(format!(
                        "manifest top level is not a mapping (line {line} is not a `key:` entry)"
                    ));
                }
                Ok(0) => {
                    // N1's vacuity: every non-blank line indented (or a
                    // comment-only body) yields zero top-level keys, and a
                    // mapping with zero top-level keys is not a mapping.
                    manifest_status = CheckStatus::Fail;
                    manifest_evidence.push(
                        "manifest top level carries no mapping entries — an all-indented \
                         (or comment-only) document is not a mapping; zero keys is not a pass"
                            .to_owned(),
                    );
                }
                Ok(keys) => match parse_manifest(text) {
                    Err(err) => {
                        manifest_status = CheckStatus::Fail;
                        manifest_evidence.push(format!(
                            "manifest fails the strict frozen-shape parse — reference-class \
                             failure: {err}"
                        ));
                    }
                    Ok(definition) => {
                        manifest_status = CheckStatus::Pass;
                        manifest_evidence.push(format!(
                            "manifest exists; top level is a mapping ({keys} keys); deep-parses \
                             under the frozen reference shape (id {}, version {})",
                            definition.id.as_str(),
                            definition.version
                        ));
                    }
                },
            },
        },
    }
    // The frozen reference record (§9.4), read STRICTLY (plan Task 7): an
    // unknown field in the lock is a finding, never silently ignored — the
    // freeze record is as frozen as the key.
    let lock_path = defaults::reference_lock_at(&resolved.project_root);
    let lock = read_reference_lock(&lock_path);
    // The answer key's raw bytes, read once: the count clause, the schema
    // clause, and the digest clause all consume the same read.
    let key_bytes = std::fs::read(&resolved.reference.answer_key);
    // Answer key: exists, non-empty, block count equals the frozen count.
    let mut answer_evidence = Vec::new();
    let answer_status = match (&key_bytes, &lock) {
        (Err(err), _) => {
            answer_evidence.push(format!(
                "answer key not readable at {} ({})",
                leaf_of(&resolved.reference.answer_key),
                err.kind()
            ));
            CheckStatus::Fail
        }
        (Ok(bytes), _) if bytes.iter().all(|byte| byte.is_ascii_whitespace()) => {
            answer_evidence.push("answer key is empty".to_owned());
            CheckStatus::Fail
        }
        (Ok(_), Err(finding)) => {
            answer_evidence.push(finding.clone());
            CheckStatus::Fail
        }
        (Ok(bytes), Ok(lock)) => {
            let count = bytes
                .split(|byte| *byte == b'\n')
                .filter(|line| line.starts_with(b"## AK-"))
                .count();
            if count == lock.unit_count {
                answer_evidence.push(format!(
                    "answer key exists, non-empty, {count} units (frozen count {})",
                    lock.unit_count
                ));
                CheckStatus::Pass
            } else {
                answer_evidence.push(format!(
                    "answer key holds {count} unit blocks; the frozen record declares {}",
                    lock.unit_count
                ));
                CheckStatus::Fail
            }
        }
    };
    // Unit schema (plan Task 7): the configured answer key parsed by the
    // strict per-block parser — every Unit well-formed under the §5.3.2
    // schema (tier/authority coupling included), or the per-block failures
    // with their loci as evidence. This is the compensating control for the
    // provider's documented admission decision (`reference/provider.rs`'s
    // header): structural strictness lives at admission, and this is the
    // admission callsite that reports block-level defects loudly.
    let (schema_status, schema_evidence) = match &key_bytes {
        Err(err) => (
            CheckStatus::Fail,
            vec![format!(
                "cannot validate the unit schema: the answer key is not readable ({})",
                err.kind()
            )],
        ),
        Ok(bytes) => match std::str::from_utf8(bytes) {
            Err(err) => (
                CheckStatus::Fail,
                vec![format!(
                    "the answer key is not valid UTF-8 (first invalid byte at {}) — the \
                     unit schema cannot be validated",
                    err.valid_up_to()
                )],
            ),
            Ok(text) => {
                let parsed = parse_answer_key(text);
                let total = parsed.units.len() + parsed.failures.len();
                if total == 0 {
                    (
                        CheckStatus::Fail,
                        vec![
                            "the answer key holds no AK-NNN blocks at all; there is no unit \
                             schema to validate"
                                .to_owned(),
                        ],
                    )
                } else if parsed.failures.is_empty() {
                    (
                        CheckStatus::Pass,
                        vec![format!(
                            "{total}/{total} units well-formed under the 5.3.2 schema \
                             (tier/authority coupling included)"
                        )],
                    )
                } else {
                    let mut evidence = vec![format!(
                        "{}/{} units well-formed; per-block failures:",
                        parsed.units.len(),
                        total
                    )];
                    evidence.extend(parsed.failures.iter().map(|failure| failure.to_string()));
                    (CheckStatus::Fail, evidence)
                }
            }
        },
    };
    // Digest-vs-lock (plan Task 7): recompute the normalized digests of BOTH
    // ruler halves — the answer key and the manifest — and compare each with
    // the frozen record. Either mismatch means the ruler moved (I1) — a
    // reference-class failure, loud on both surfaces. The manifest half
    // closes critic-A M3 / critic-B F2: the record's manifest_digest was
    // previously declared, whitelisted, and never verified, leaving the
    // narrative half of the ruler with no integrity check anywhere.
    let (digest_status, digest_evidence) = match (&lock, &key_bytes, &manifest_bytes) {
        (Err(finding), _, _) => (
            CheckStatus::Fail,
            vec![format!("digest-vs-lock verification cannot run: {finding}")],
        ),
        (Ok(_), Err(err), _) => (
            CheckStatus::Fail,
            vec![format!(
                "cannot recompute the answer-key digest: the key is not readable ({})",
                err.kind()
            )],
        ),
        (Ok(_), Ok(_), Err(err)) => (
            CheckStatus::Fail,
            vec![format!(
                "cannot recompute the manifest digest: the manifest is not readable ({})",
                err.kind()
            )],
        ),
        (Ok(lock), Ok(key_bytes), Ok(manifest_bytes)) => {
            let mut status = CheckStatus::Pass;
            let mut evidence = Vec::new();
            match digest_of(key_bytes) {
                Err(err) => {
                    status = CheckStatus::Fail;
                    evidence.push(format!(
                        "the answer key does not normalize and digest: {err}"
                    ));
                }
                Ok(actual) if actual == lock.answer_key_digest => {
                    evidence.push(format!(
                        "recomputed normalized digest {actual} matches the frozen record \
                         (answer-key half, digest-vs-lock)"
                    ));
                }
                Ok(actual) => {
                    status = CheckStatus::Fail;
                    evidence.push(format!(
                        "REFERENCE MOVED: the answer key's normalized digest is {actual} but the \
                         frozen record declares {} — reference-class failure (I1): the ruler does \
                         not match its freeze record",
                        lock.answer_key_digest
                    ));
                }
            }
            match digest_of(manifest_bytes) {
                Err(err) => {
                    status = CheckStatus::Fail;
                    evidence.push(format!("the manifest does not normalize and digest: {err}"));
                }
                Ok(actual) if actual == lock.manifest_digest => {
                    evidence.push(format!(
                        "recomputed normalized digest {actual} matches the frozen record \
                         (manifest half, digest-vs-lock)"
                    ));
                }
                Ok(actual) => {
                    status = CheckStatus::Fail;
                    evidence.push(format!(
                        "REFERENCE MOVED: the manifest's normalized digest is {actual} but the \
                         frozen record declares {} — reference-class failure (I1): the ruler's \
                         narrative half does not match its freeze record",
                        lock.manifest_digest
                    ));
                }
            }
            (status, evidence)
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
                status: schema_status,
                evidence: schema_evidence,
            },
            SubResult {
                name: "digest".to_owned(),
                status: digest_status,
                evidence: digest_evidence,
            },
        ],
        Vec::new(),
    )
}

/// The file leaf of a path, for evidence that must not carry resolved
/// directories.
fn leaf_of(path: &Path) -> String {
    path.file_name()
        .map(|name| name.to_string_lossy().into_owned())
        .unwrap_or_else(|| path.to_string_lossy().into_owned())
}

/// Confirms a document's top level is a mapping: every non-blank,
/// non-comment, column-zero line is a `key:` entry. Returns the count of
/// top-level keys; the caller refuses a zero count (N1: zero keys is the
/// vacuous shape an all-indented document produces, and a mapping with no
/// top-level keys is not a mapping). Full YAML is NOT claimed — the deep
/// shape is `parse_manifest`'s business, and the manifest uses constructs
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

/// The frozen lock's known field names (§9.4 freeze record, as frozen by
/// plan Task 1). The lock is as frozen as the key: a field outside this set
/// is drift, and drift is a finding — never silently ignored.
const LOCK_FIELDS: &[&str] = &[
    "answer_key_path",
    "answer_key_digest",
    "manifest_path",
    "manifest_digest",
    "algo",
    "frozen_at",
    "spec_source",
    "spec_annex",
    "unit_count",
    "digest_tool",
    "note",
];

/// What check 2 consumes from the frozen reference record: the digests the
/// answer key AND the manifest must still hash to, and the unit count the
/// key declares.
struct FrozenLock {
    answer_key_digest: Digest,
    manifest_digest: Digest,
    unit_count: usize,
}

/// Strict read of `.gauntlet/reference.lock` (plan Task 7): hand-rolled over
/// the ports JSON parser. Unknown fields are findings; `answer_key_digest`
/// must be present and parse as a domain `Digest`; `unit_count` must be a
/// non-negative integer; a present `algo` must name the defaults-table
/// digest algorithm. Every failure names its locus.
fn read_reference_lock(lock_path: &Path) -> Result<FrozenLock, String> {
    let text = std::fs::read_to_string(lock_path).map_err(|err| {
        format!(
            "reference record not readable at {} ({})",
            leaf_of(lock_path),
            err.kind()
        )
    })?;
    let value =
        json::parse(&text).map_err(|err| format!("reference record is not valid JSON: {err}"))?;
    let unknown: Vec<String> = value
        .keys()
        .into_iter()
        .filter(|key| !LOCK_FIELDS.contains(&key.as_str()))
        .collect();
    if !unknown.is_empty() {
        return Err(format!(
            "reference record carries unknown field(s) {} — the freeze record is frozen; \
             drift is a finding",
            unknown.join(", ")
        ));
    }
    let digest_text = match value.get("answer_key_digest") {
        Some(JsonValue::Str(text)) => text.clone(),
        Some(other) => {
            return Err(format!(
                "reference record answer_key_digest is not a string: {other:?}"
            ));
        }
        None => return Err("reference record carries no answer_key_digest".to_owned()),
    };
    let answer_key_digest = Digest::parse(&digest_text)
        .map_err(|err| format!("reference record answer_key_digest is not a digest: {err}"))?;
    // The manifest half of the freeze record is as required as the key half
    // (critic-A M3 / critic-B F2): a lock that drops it is drift, and a
    // declared-but-unverified digest is decorative.
    let manifest_text = match value.get("manifest_digest") {
        Some(JsonValue::Str(text)) => text.clone(),
        Some(other) => {
            return Err(format!(
                "reference record manifest_digest is not a string: {other:?}"
            ));
        }
        None => return Err("reference record carries no manifest_digest".to_owned()),
    };
    let manifest_digest = Digest::parse(&manifest_text)
        .map_err(|err| format!("reference record manifest_digest is not a digest: {err}"))?;
    let count = value
        .get("unit_count")
        .and_then(|field| match field {
            JsonValue::Number(json::JsonNumber::Uint(count)) => Some(*count),
            _ => None,
        })
        .ok_or_else(|| "reference record carries no unit_count".to_owned())?;
    let unit_count =
        usize::try_from(count).map_err(|_| "reference record unit_count is negative".to_owned())?;
    if let Some(algo) = value.get("algo") {
        match algo {
            JsonValue::Str(token) if token == defaults::DIGEST_ALGO => {}
            other => {
                return Err(format!(
                    "reference record algo must be '{}': {other:?}",
                    defaults::DIGEST_ALGO
                ));
            }
        }
    }
    Ok(FrozenLock {
        answer_key_digest,
        manifest_digest,
        unit_count,
    })
}

// ---------------------------------------------------------------------------
// Check 3 — a runner resolves for every dispatchable role (§8.5 check 3)
// ---------------------------------------------------------------------------

/// A composition-root MEASUREMENT of the configured default runner (plan
/// Task 15, §8.5 check 3; correction RES-D7): the mirror of
/// [`BackendDescriptor`]. The app layer cannot probe a harness binary
/// itself — the adapter lives under `crates/adapters/**` (§24.3) — so the
/// measurement arrives from the composition root, which owns the
/// adapters. WHAT was measured is named by [`RunnerDescriptor::kind`] —
/// plan Task 15's version-range probe or plan Task 16's configuration
/// probe — and the evidence text renders that dimension and nothing more
/// (correction RES-D6: a configuration-only probe must never claim a
/// version measurement).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunnerDescriptor {
    /// The runner token the descriptor measured (must equal the resolved
    /// `runners.default`, or the descriptor describes a different runner).
    pub token: String,
    /// What the measurement WAS — the typed dimension (RES-D6). A
    /// descriptor naming the configuration dimension claims version
    /// compatibility for nothing, and vice versa.
    pub kind: RunnerProbeKind,
    /// Whether the probe RESOLVED: for [`RunnerProbeKind::VersionRange`],
    /// the binary was found, its version parsed, and the version sits
    /// inside the supported range injected by the composition root from
    /// the app defaults module; for [`RunnerProbeKind::Configuration`],
    /// the adapter's typed configuration probe resolved (the command
    /// template parses and every placeholder is known).
    pub measured: bool,
    /// Why measurement failed, when it did — the probe's typed locus.
    pub note: Option<String>,
}

/// What a [`RunnerDescriptor`]'s measurement actually was (correction
/// RES-D6, plan Task 16): the typed what-was-measured dimension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerProbeKind {
    /// Plan Task 15: the real binary was located and its version probed
    /// against the injected supported range. The only dimension that may
    /// claim version compatibility.
    VersionRange,
    /// Plan Task 16: the adapter's typed configuration probe resolved —
    /// the command template parses and every placeholder is known. No
    /// binary is touched and no version is claimed.
    Configuration,
}

impl RunnerProbeKind {
    /// The evidence-text phrase for a CLEAN measurement of this dimension.
    pub fn clean_phrase(self, token: &str) -> String {
        match self {
            Self::VersionRange => format!(
                "runner '{token}' resolves: the harness API-surface version probe measured \
                 clean (version compatibility only — isolation is declared per invocation at \
                 dispatch)"
            ),
            Self::Configuration => format!(
                "runner '{token}' resolves: the typed configuration probe measured clean \
                 (command template parses and every placeholder is known — configuration \
                 only, no binary probed and no version claimed)"
            ),
        }
    }

    /// The evidence-text phrase for a FAILED measurement of this
    /// dimension.
    pub fn failed_phrase(self, token: &str, note: Option<&str>) -> String {
        let note = note.map(|note| format!(": {note}")).unwrap_or_default();
        match self {
            Self::VersionRange => format!(
                "runner_probe_failed: runner '{token}' could not be probed{note} — a harness \
                 whose API-surface version does not resolve is a configuration failure (spec \
                 8.5 check 3, 21.1)"
            ),
            Self::Configuration => format!(
                "runner_probe_failed: runner '{token}' could not be probed{note} — a command \
                 template that does not parse (unknown placeholder, unbalanced quoting, or a \
                 missing value) is a configuration failure (spec 8.5 check 3, 21.1)"
            ),
        }
    }

    /// The evidence-text phrase for the capability check's measured half.
    pub fn capability_phrase(self, token: &str) -> String {
        match self {
            Self::VersionRange => format!(
                "runner '{token}' probed: harness API-surface version compatibility measured \
                 (that dimension only)"
            ),
            Self::Configuration => format!(
                "runner '{token}' probed: runner configuration measured (that dimension only \
                 — no binary probed and no version claimed)"
            ),
        }
    }
}

pub fn check_runners(resolved: &ResolvedConfig, runner: Option<&RunnerDescriptor>) -> CheckOutcome {
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
    // The resolution half (plan Task 15, correction RES-D7): when the
    // configuration names a default runner AND the composition root
    // measured it, the check reports the MEASURED probe result — resolved
    // or failed with its locus — instead of the blanket unavailability.
    // A failed measurement is a configuration failure (non-zero exit
    // class), never an unavailable-inside-pass. With no measurement (no
    // hook, no adapter fronting the token, or no runner configured), the
    // honest unavailability stands.
    let has_named_runner = has_default;
    let resolution = match (has_named_runner, runner) {
        (true, Some(descriptor))
            if descriptor.token != resolved.runners_default.as_deref().unwrap_or("") =>
        {
            (
                CheckStatus::Fail,
                vec![format!(
                    "runner_resolution_mismatch: the measured descriptor names runner '{}' \
                     while the configuration resolves '{}' — the measurement describes a \
                     different runner",
                    descriptor.token,
                    resolved.runners_default.as_deref().unwrap_or("")
                )],
            )
        }
        (true, Some(descriptor)) if !descriptor.measured => (
            CheckStatus::Fail,
            vec![descriptor.kind.failed_phrase(
                resolved.runners_default.as_deref().unwrap_or(""),
                descriptor.note.as_deref(),
            )],
        ),
        (true, Some(descriptor)) => (
            CheckStatus::Pass,
            vec![descriptor.kind.clean_phrase(&descriptor.token)],
        ),
        (true, None) => {
            // COR-D8 (Task 20 RES-09): token-neutral and present-tense —
            // the composition fronting no MEASUREMENT for this token is
            // the honest locus (e.g. the test-division mock token); the
            // stale "adapters land with plan Tasks 15–16" named a past
            // state and mislocated the fact.
            let reason = "no measurement fronted for this runner token";
            (
                CheckStatus::Unavailable {
                    reason: reason.to_owned(),
                },
                vec![format!(
                    "runtime resolution against a runner registry is unavailable: \
                     {reason} — the composition root measured no descriptor for the \
                     configured runner"
                )],
            )
        }
        (false, _) => {
            let reason = "no runner configured (UNK-003)";
            (
                CheckStatus::Unavailable {
                    reason: reason.to_owned(),
                },
                vec![format!(
                    "runtime resolution against a runner registry is unavailable: {reason}"
                )],
            )
        }
    };
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
                status: resolution.0,
                evidence: resolution.1,
            },
        ],
        Vec::new(),
    )
}

// ---------------------------------------------------------------------------
// Check 4 — runner capabilities satisfy configured tiers (§8.5 check 4)
// ---------------------------------------------------------------------------

/// A composition-root MEASUREMENT of the configured sandbox backend (plan
/// Task 14): the achieved isolation rung, and whether it was actually
/// measured (an adapter fronting the backend validated a policy and
/// reported the rung) rather than declared. The app layer cannot map
/// backend tokens to rungs itself — vendor tokens are adapter knowledge
/// (§24.3) — so the measurement arrives from the composition root, which
/// owns the adapters.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BackendDescriptor {
    /// The backend token the descriptor measured (must equal the resolved
    /// `sandbox.backend`, or the descriptor describes a different backend).
    pub token: String,
    /// The isolation rung the backend actually achieved under validation.
    pub isolation: Isolation,
    /// Whether the rung was MEASURED: a backend whose probe failed is a
    /// capability mismatch in its own right (§8.5 check 4's failure
    /// class), not an unavailable check.
    pub measured: bool,
    /// Why measurement failed, when it did.
    pub note: Option<String>,
}

pub fn check_capabilities(
    resolved: &ResolvedConfig,
    descriptor: Option<&BackendDescriptor>,
    runner: Option<&RunnerDescriptor>,
) -> CheckOutcome {
    let backend_live = resolved.sandbox_backend.trim() != defaults::SANDBOX_BACKEND;
    // The §17.3 tier→rung table is the GATE's, read through
    // govern::posture. The doctor previously wrote its own copy, so the two
    // could drift: mutating the gate's table left every doctor test green,
    // and the doctor would then report a posture the gate does not enforce.
    // There is one table now, and this is a read of it.
    let tiers = dispatchable_posture(backend_live);
    // Subresult 1 — the tier rungs the configuration implies vs the
    // backend's measured capability. Under the conservative posture the
    // required rungs are trivially satisfied (contained requires none) and
    // the honest evidence names the posture: above-contained dispatch
    // refuses at the governance gate with an escalation, which is the
    // §8.6-specified behavior, not a doctor failure.
    let (rung_status, rung_evidence) = if !backend_live {
        (
            CheckStatus::Pass,
            vec![
                format!(
                    "sandbox backend resolves to '{}' (the 8.6 conservative default): only the \
                     contained tier is dispatchable",
                    resolved.sandbox_backend
                ),
                "above-contained units refuse dispatch at the governance gate with an escalation \
                 ticket (spec 8.6; plan Task 14) — runtime governance, not a config error"
                    .to_owned(),
            ],
        )
    } else {
        match descriptor {
            None => (
                CheckStatus::Fail,
                vec![format!(
                    "sandbox_capability_mismatch: backend '{}' is configured but the composition \
                     root measured no descriptor for it — a backend nothing provides cannot \
                     cover the {} dispatchable tier(s) (spec 8.5 check 4)",
                    resolved.sandbox_backend,
                    tiers.len()
                )],
            ),
            Some(descriptor) if descriptor.token != resolved.sandbox_backend => (
                CheckStatus::Fail,
                vec![format!(
                    "sandbox_capability_mismatch: the measured descriptor names backend '{}' \
                     while the configuration resolves '{}' — the measurement describes a \
                     different backend",
                    descriptor.token, resolved.sandbox_backend
                )],
            ),
            Some(descriptor) if !descriptor.measured => (
                CheckStatus::Fail,
                vec![format!(
                    "sandbox_capability_mismatch: backend '{}' could not be measured{} — an \
                     unusable backend covers no tier (spec 8.5 check 4, 21.3)",
                    resolved.sandbox_backend,
                    descriptor
                        .note
                        .as_ref()
                        .map(|note| format!(": {note}"))
                        .unwrap_or_default()
                )],
            ),
            Some(descriptor) => {
                let mut evidence = Vec::new();
                let mut ok = true;
                for (tier, required) in &tiers {
                    if descriptor.isolation < *required {
                        ok = false;
                        evidence.push(format!(
                            "sandbox_capability_mismatch: backend '{}' achieves isolation {}, \
                             below the {} tier's required rung {} (spec 17.3)",
                            resolved.sandbox_backend, descriptor.isolation, tier, required
                        ));
                    } else {
                        evidence.push(format!(
                            "tier {tier} requires rung {required}; backend achieves {}",
                            descriptor.isolation
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
            }
        }
    };
    // Subresult 2 — the RUNNER half of §8.5 check 4: fed from the same
    // measured descriptor as check 3's resolution half (plan Task 15,
    // RES-D7). A measured-but-failed probe is a FAIL here too — never an
    // unavailable-inside-pass. What was measured is the harness
    // API-surface version compatibility ONLY; the per-invocation
    // isolation-vs-tier enforcement remains the dispatch gate's (§17.3),
    // and the evidence says exactly that.
    let (runner_status, runner_evidence) = match runner {
        Some(descriptor)
            if resolved
                .runners_default
                .as_deref()
                .is_some_and(|name| !name.trim().is_empty()) =>
        {
            if descriptor.token != resolved.runners_default.as_deref().unwrap_or("") {
                (
                    CheckStatus::Fail,
                    vec![format!(
                        "runner_resolution_mismatch: the measured descriptor names runner '{}' \
                         while the configuration resolves '{}'",
                        descriptor.token,
                        resolved.runners_default.as_deref().unwrap_or("")
                    )],
                )
            } else if !descriptor.measured {
                (
                    CheckStatus::Fail,
                    vec![format!(
                        "runner_probe_failed: runner '{}' could not be probed{} — a runner \
                         nothing can probe covers no dispatchable role (spec 8.5 check 4)",
                        descriptor.token,
                        descriptor
                            .note
                            .as_ref()
                            .map(|note| format!(": {note}"))
                            .unwrap_or_default()
                    )],
                )
            } else {
                (
                    CheckStatus::Pass,
                    vec![
                        descriptor.kind.capability_phrase(&descriptor.token),
                        "runner isolation-vs-tier enforcement happens per invocation at the \
                         dispatch gate (spec 17.3); the doctor measures no rung for runners"
                            .to_owned(),
                    ],
                )
            }
        }
        _ => {
            let runner_reason = "runner capabilities resolve with plan Tasks 15-16; the dispatch \
                 gate enforces per-invocation isolation in the meantime";
            (
                CheckStatus::Unavailable {
                    reason: runner_reason.to_owned(),
                },
                vec![format!(
                    "runtime runner-capability resolution is unavailable: {runner_reason}"
                )],
            )
        }
    };
    CheckOutcome::combine(
        4,
        "capabilities",
        CheckStatus::Pass,
        vec![
            SubResult {
                name: "tier rungs".to_owned(),
                status: rung_status,
                evidence: rung_evidence,
            },
            SubResult {
                name: "runner capabilities".to_owned(),
                status: runner_status,
                evidence: runner_evidence,
            },
        ],
        Vec::new(),
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
