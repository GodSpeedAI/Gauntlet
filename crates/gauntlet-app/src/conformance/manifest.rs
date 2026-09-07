//! The strict `gauntlet.conformance-manifest@v1` codec (task 27, B1;
//! charter decision 2): the authored input the sweep adjudicates from,
//! parsed like the repo's other zero-dependency readers (the
//! `baseline_task@v1` precedent) — a hand-rolled parser over a
//! DOCUMENTED SUBSET where strictness is the feature.
//!
//! # The grammar
//!
//! ```text
//! schema: gauntlet.conformance-manifest@v1
//! units:
//!   - id: <AK-NNN>
//!     kind: <evidence kind wire token>
//!     frozen_locator: <verbatim from the ruler>
//!     evidence:
//!       execute: {package: <allowlisted>, target: <cargo test target>}
//!       # execute: {package: <allowlisted>, lib: true, filter: <token>}
//!       # execute: {program: </absolute/path>}          # fixture arm
//!       # artifact: {path: <repo-relative>, predicate: <named>}
//!       # none: {reason: <comma-free text>}
//!     bounded: <true|false>
//!     status_hint: <none|configured_but_untested|proposed|unknown>
//!     basis: <one line>
//!     divergence: <none|reference-candidates YYYY-MM-DD|unrecorded>
//! ```
//!
//! Accepted, and nothing else:
//!
//! - `schema` exactly once at the head, equal to
//!   [`MANIFEST_SCHEMA_TOKEN`]; `units:` exactly once; then block
//!   entries, each introduced by `- id:` at two spaces;
//! - each entry: exactly the eight keys above, each exactly once, in
//!   any order, all required; the `evidence:` key opens a sub-map whose
//!   EXACTLY ONE arm follows at six spaces as a flow map of plain
//!   `key: value` pairs — a value containing a comma is refused
//!   (authorship discipline: reasons stay comma-free);
//! - the execute arms validate through the closed vocabulary
//!   ([`super::commands`]); `file`/`metric` kinds may carry ONLY
//!   `artifact`/`none` arms (the charter's "OR, for kind file/metric"
//!   shape — an artifact-locator unit's evidence is its artifact, or an
//!   honest none);
//! - artifact paths are repo-relative (no leading `/`, no `..`, no
//!   whitespace) and their predicate is a name [`super::predicates`]
//!   implements;
//! - `kind` parses through the domain `EvidenceKind` wire vocabulary;
//!   `status_hint` and `divergence` are validated vocabularies.
//!
//! Refused, each a typed error carrying the 1-based line: unknown,
//! duplicate, or missing keys; more than one arm or no arm; tabs in
//! indentation; comments; wrong indentation; every vocabulary
//! violation above.

use core::fmt;

use gauntlet_domain::EvidenceKind;

use super::commands::{CommandError, SweepCommand, validate_nextest, validate_program};
use super::predicates;

/// The only schema token this codec admits.
pub const MANIFEST_SCHEMA_TOKEN: &str = "gauntlet.conformance-manifest@v1";

/// The validated non-PASS status-hint vocabulary (charter decision 5:
/// `configured_but_untested|proposed|unknown`; `None` carries no hint
/// and is valid only where the entry expects to PASS — an unforeseen
/// INCONCLUSIVE maps to `unknown` at assembly, disclosed there).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StatusHint {
    None,
    ConfiguredButUntested,
    Proposed,
    Unknown,
}

impl StatusHint {
    /// The wire token.
    pub fn wire(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::ConfiguredButUntested => "configured_but_untested",
            Self::Proposed => "proposed",
            Self::Unknown => "unknown",
        }
    }

    /// Strict parse; anything else is the caller's typed error.
    pub fn parse(token: &str) -> Option<Self> {
        match token {
            "none" => Some(Self::None),
            "configured_but_untested" => Some(Self::ConfiguredButUntested),
            "proposed" => Some(Self::Proposed),
            "unknown" => Some(Self::Unknown),
            _ => None,
        }
    }
}

/// One manifest entry's evidence arm — exactly one of the four.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EvidenceArm {
    Execute(SweepCommand),
    Artifact { path: String, predicate: String },
    None { reason: String },
}

/// One parsed manifest entry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ManifestEntry {
    pub id: String,
    /// The wire token of the unit's `evidence_required` kind.
    pub kind: String,
    pub frozen_locator: String,
    pub evidence: EvidenceArm,
    pub bounded: bool,
    pub status_hint: StatusHint,
    pub basis: String,
    /// `none`, `reference-candidates <date>`, or `unrecorded` (a real
    /// divergence whose dated entry is owed — the task-27 transition
    /// spelling, disclosed in the report's `_meta`).
    pub divergence: String,
}

/// Why the manifest refused. Typed, carrying the 1-based line where the
/// defect was detected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ManifestError {
    Schema { line: usize, found: String },
    UnitsHeader { line: usize, text: String },
    UnknownKey { line: usize, key: String },
    DuplicateKey { line: usize, key: &'static str },
    MissingKey { key: &'static str },
    MalformedLine { line: usize, text: String },
    TabIndentation { line: usize },
    Comment { line: usize },
    BadEvidenceArm { line: usize, detail: String },
    BadKind { line: usize, found: String },
    BadStatusHint { line: usize, found: String },
    BadDivergence { line: usize, found: String },
    BadBool { line: usize, found: String },
    BadArtifactPath { line: usize, path: String },
    UnknownPredicate { line: usize, predicate: String },
    Command { line: usize, cause: CommandError },
    EmptyEntryList,
}

impl fmt::Display for ManifestError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Schema { line, found } => write!(
                f,
                "line {line}: the schema token must be {MANIFEST_SCHEMA_TOKEN:?}, found \
                 {found:?}"
            ),
            Self::UnitsHeader { line, text } => {
                write!(f, "line {line}: {text:?} is not the units header")
            }
            Self::UnknownKey { line, key } => write!(
                f,
                "line {line}: unknown key {key:?}; the frozen entry key set is id, kind, \
                 frozen_locator, evidence, bounded, status_hint, basis, divergence"
            ),
            Self::DuplicateKey { line, key } => {
                write!(f, "line {line}: key {key:?} appears twice in one entry")
            }
            Self::MissingKey { key } => {
                write!(f, "an entry is missing its required key {key:?}")
            }
            Self::MalformedLine { line, text } => {
                write!(
                    f,
                    "line {line}: {text:?} is not a line of the documented shape"
                )
            }
            Self::TabIndentation { line } => {
                write!(f, "line {line}: tab in indentation (spaces only)")
            }
            Self::Comment { line } => {
                write!(
                    f,
                    "line {line}: comments are refused — a tolerated comment hides drift"
                )
            }
            Self::BadEvidenceArm { line, detail } => {
                write!(f, "line {line}: the evidence arm is invalid: {detail}")
            }
            Self::BadKind { line, found } => {
                write!(
                    f,
                    "line {line}: kind {found:?} is not an evidence kind wire token"
                )
            }
            Self::BadStatusHint { line, found } => write!(
                f,
                "line {line}: status_hint {found:?} is not one of none, \
                 configured_but_untested, proposed, unknown"
            ),
            Self::BadDivergence { line, found } => write!(
                f,
                "line {line}: divergence {found:?} is not none, unrecorded, or \
                 reference-candidates <YYYY-MM-DD>"
            ),
            Self::BadBool { line, found } => {
                write!(
                    f,
                    "line {line}: bounded must be true or false, found {found:?}"
                )
            }
            Self::BadArtifactPath { line, path } => write!(
                f,
                "line {line}: artifact path {path:?} must be repo-relative (no leading \
                 slash, no parent traversal, no whitespace)"
            ),
            Self::UnknownPredicate { line, predicate } => write!(
                f,
                "line {line}: predicate {predicate:?} is not one of the named predicates"
            ),
            Self::Command { line, cause } => {
                write!(
                    f,
                    "line {line}: the execute arm violates the closed vocabulary: {cause}"
                )
            }
            Self::EmptyEntryList => {
                write!(f, "the manifest carries no entries")
            }
        }
    }
}

impl std::error::Error for ManifestError {}

/// The eight entry keys (any order accepted; this list drives the
/// missing-key check).
const ENTRY_KEYS: &[&str] = &[
    "id",
    "kind",
    "frozen_locator",
    "evidence",
    "bounded",
    "status_hint",
    "basis",
    "divergence",
];

/// One entry under assembly.
#[derive(Default)]
struct EntryDraft {
    id: Option<String>,
    kind: Option<String>,
    frozen_locator: Option<String>,
    evidence: Option<EvidenceArm>,
    evidence_seen: bool,
    bounded: Option<bool>,
    status_hint: Option<StatusHint>,
    basis: Option<String>,
    divergence: Option<String>,
    seen: Vec<&'static str>,
}

impl EntryDraft {
    fn set(&mut self, key: &'static str, line: usize) -> Result<(), ManifestError> {
        if self.seen.contains(&key) {
            return Err(ManifestError::DuplicateKey { line, key });
        }
        self.seen.push(key);
        Ok(())
    }
}

/// A `key: value` line split on the FIRST colon; the value keeps any
/// later colons verbatim. `None` for empty sides or a missing colon.
fn split_field(line: &str) -> Option<(&str, &str)> {
    let (key, value) = line.split_once(':')?;
    let key = key.trim();
    let value = value.trim();
    if key.is_empty() || value.is_empty() {
        return None;
    }
    Some((key, value))
}

/// A flow map `{k: v, k: v}` → its pairs (values are plain, comma-free).
fn flow_map(line: usize, value: &str) -> Result<Vec<(String, String)>, ManifestError> {
    let malformed = |detail: String| ManifestError::BadEvidenceArm { line, detail };
    let Some(body) = value
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
    else {
        return Err(malformed(format!(
            "expected a {{...}} flow map, got {value:?}"
        )));
    };
    if body.trim().is_empty() {
        return Err(malformed("the flow map is empty".to_owned()));
    }
    let mut pairs = Vec::new();
    for part in body.split(',') {
        let (key, sub_value) = part
            .trim()
            .split_once(':')
            .ok_or_else(|| malformed(format!("flow entry {part:?} is not a `key: value` pair")))?;
        let key = key.trim();
        let sub_value = sub_value.trim();
        if key.is_empty() || sub_value.is_empty() {
            return Err(malformed(format!(
                "flow entry {part:?} carries an empty side"
            )));
        }
        if pairs
            .iter()
            .any(|(known, _): &(String, String)| known == key)
        {
            return Err(malformed(format!("flow key {key:?} appears twice")));
        }
        pairs.push((key.to_owned(), sub_value.to_owned()));
    }
    Ok(pairs)
}

fn artifact_path_ok(path: &str) -> bool {
    // The component-wise guard (the controller's N4 precedent): only
    // Normal components — a parent traversal, root, or prefix is not a
    // repo-relative artifact path, whatever its spelling.
    !path.starts_with('/')
        && std::path::Path::new(path)
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
        && !path.contains(char::is_whitespace)
        && !path.is_empty()
}

fn divergence_ok(value: &str) -> bool {
    if value == "none" || value == "unrecorded" {
        return true;
    }
    let Some(date) = value.strip_prefix("reference-candidates ") else {
        return false;
    };
    let bytes = date.as_bytes();
    if bytes.len() != 10
        || !bytes[0..4].iter().all(u8::is_ascii_digit)
        || bytes[4] != b'-'
        || !bytes[5..7].iter().all(u8::is_ascii_digit)
        || bytes[7] != b'-'
        || !bytes[8..10].iter().all(u8::is_ascii_digit)
    {
        return false;
    }
    // Calendar-plausible bounds: a citation is a date, not digits.
    let month: u32 = date[5..7].parse().unwrap_or(0);
    let day: u32 = date[8..10].parse().unwrap_or(0);
    (1..=12).contains(&month) && (1..=31).contains(&day)
}

/// Parses one evidence arm line (`execute:`, `artifact:`, or `none:` at
/// six spaces) into its arm, against the caller's package and feature
/// vocabularies.
fn parse_arm(
    line: usize,
    key: &str,
    value: &str,
    allowed: &[&str],
    allowed_features: &[&str],
) -> Result<EvidenceArm, ManifestError> {
    let pairs = flow_map(line, value)?;
    let lookup = |name: &str| pairs.iter().find(|(key, _)| key == name);
    match key {
        "execute" => {
            if let Some((_, program)) = lookup("program") {
                let extra = pairs.len() > 1;
                if extra {
                    return Err(ManifestError::BadEvidenceArm {
                        line,
                        detail: "the program arm carries exactly one key".to_owned(),
                    });
                }
                return validate_program(program)
                    .map(EvidenceArm::Execute)
                    .map_err(|cause| ManifestError::Command { line, cause });
            }
            let Some((_, package)) = lookup("package") else {
                return Err(ManifestError::BadEvidenceArm {
                    line,
                    detail: "an execute arm names neither package nor program".to_owned(),
                });
            };
            let target = lookup("target").map(|(_, value)| value.as_str());
            let lib = match lookup("lib") {
                Some((_, value)) if value == "true" => true,
                Some((_, other)) => {
                    return Err(ManifestError::BadEvidenceArm {
                        line,
                        detail: format!("lib must be true, found {other:?}"),
                    });
                }
                None => false,
            };
            let filter = lookup("filter").map(|(_, value)| value.as_str());
            // `features: [a, b]` — an inline flow sequence of feature
            // edges, validated against the caller's vocabulary (COR-2:
            // a feature-gated target under the default config compiles
            // ZERO tests, so the config is part of the command).
            let features = match lookup("features") {
                Some((_, raw)) => {
                    let Some(body) = raw
                        .strip_prefix('[')
                        .and_then(|rest| rest.strip_suffix(']'))
                    else {
                        return Err(ManifestError::BadEvidenceArm {
                            line,
                            detail: format!("features must be a [a, b] sequence, got {raw:?}"),
                        });
                    };
                    let mut edges = Vec::new();
                    for entry in body.split(',') {
                        let entry = entry.trim();
                        if entry.is_empty() {
                            return Err(ManifestError::BadEvidenceArm {
                                line,
                                detail: "the features sequence carries an empty entry".to_owned(),
                            });
                        }
                        edges.push(entry.to_owned());
                    }
                    edges
                }
                None => Vec::new(),
            };
            let known_keys = ["package", "target", "lib", "filter", "features"];
            for (flow_key, _) in &pairs {
                if !known_keys.contains(&flow_key.as_str()) {
                    return Err(ManifestError::BadEvidenceArm {
                        line,
                        detail: format!("unknown execute key {flow_key:?}"),
                    });
                }
            }
            validate_nextest(
                package,
                target,
                lib,
                filter,
                &features,
                allowed,
                allowed_features,
            )
            .map(EvidenceArm::Execute)
            .map_err(|cause| ManifestError::Command { line, cause })
        }
        "artifact" => {
            let Some((_, path)) = lookup("path") else {
                return Err(ManifestError::BadEvidenceArm {
                    line,
                    detail: "the artifact arm requires a path".to_owned(),
                });
            };
            let Some((_, predicate)) = lookup("predicate") else {
                return Err(ManifestError::BadEvidenceArm {
                    line,
                    detail: "the artifact arm requires a predicate".to_owned(),
                });
            };
            if pairs.len() != 2 {
                return Err(ManifestError::BadEvidenceArm {
                    line,
                    detail: "the artifact arm carries exactly path and predicate".to_owned(),
                });
            }
            if !artifact_path_ok(path) {
                return Err(ManifestError::BadArtifactPath {
                    line,
                    path: path.clone(),
                });
            }
            if !predicates::is_known(predicate) {
                return Err(ManifestError::UnknownPredicate {
                    line,
                    predicate: predicate.clone(),
                });
            }
            Ok(EvidenceArm::Artifact {
                path: path.clone(),
                predicate: predicate.clone(),
            })
        }
        "none" => {
            let Some((_, reason)) = lookup("reason") else {
                return Err(ManifestError::BadEvidenceArm {
                    line,
                    detail: "the none arm requires a reason".to_owned(),
                });
            };
            if pairs.len() != 1 {
                return Err(ManifestError::BadEvidenceArm {
                    line,
                    detail: "the none arm carries exactly a reason".to_owned(),
                });
            }
            if reason.len() < 4 {
                return Err(ManifestError::BadEvidenceArm {
                    line,
                    detail: format!("the reason {reason:?} is too thin to be honest"),
                });
            }
            Ok(EvidenceArm::None {
                reason: reason.clone(),
            })
        }
        other => Err(ManifestError::BadEvidenceArm {
            line,
            detail: format!("unknown arm key {other:?}; the arms are execute, artifact, none"),
        }),
    }
}

/// Finishes one entry: every key present, then the kind-based arm rule.
fn close_entry(
    draft: &mut Option<EntryDraft>,
    entries: &mut Vec<ManifestEntry>,
) -> Result<(), ManifestError> {
    let Some(entry) = draft.take() else {
        return Ok(());
    };
    for key in ENTRY_KEYS {
        if !entry.seen.contains(key) {
            return Err(ManifestError::MissingKey { key });
        }
    }
    let Some(evidence) = entry.evidence.clone() else {
        return Err(ManifestError::BadEvidenceArm {
            line: 0,
            detail: "the evidence key carries no arm".to_owned(),
        });
    };
    // Every required key is present (checked above); the None arms are
    // unreachable, and this unpack is the presence invariant made local.
    let built = ManifestEntry {
        id: entry.id.clone().expect("checked present"),
        kind: entry.kind.clone().expect("checked present"),
        frozen_locator: entry.frozen_locator.clone().expect("checked present"),
        evidence,
        bounded: entry.bounded.expect("checked present"),
        status_hint: entry.status_hint.expect("checked present"),
        basis: entry.basis.clone().expect("checked present"),
        divergence: entry.divergence.clone().expect("checked present"),
    };
    // The charter's shape rule: `file`/`metric` kinds never carry an
    // execute arm — their evidence is an artifact or an honest none.
    if (built.kind == "file" || built.kind == "metric")
        && matches!(built.evidence, EvidenceArm::Execute(_))
    {
        return Err(ManifestError::BadEvidenceArm {
            line: 0,
            detail: format!(
                "kind {} admits artifact or none arms only — an artifact-locator unit's \
                 evidence is its artifact",
                built.kind
            ),
        });
    }
    entries.push(built);
    Ok(())
}

/// Parses the manifest against the APPLICATION's default package and
/// feature vocabularies (the core workspace packages and their feature
/// edges). Strict: every violation above is a typed error naming its
/// line; there is no permissive fallback anywhere.
pub fn parse_manifest(text: &str) -> Result<Vec<ManifestEntry>, ManifestError> {
    parse_manifest_with(
        text,
        super::commands::WORKSPACE_PACKAGES,
        super::commands::WORKSPACE_FEATURES,
    )
}

/// Parses the manifest against CALLER-supplied package and feature
/// vocabularies — the composition root's seam for extending them with
/// the adapters it wires (adapter identifiers never appear in
/// application source; AK-002's own denylist enforces that here).
pub fn parse_manifest_with(
    text: &str,
    allowed: &[&str],
    allowed_features: &[&str],
) -> Result<Vec<ManifestEntry>, ManifestError> {
    let mut schema_seen = false;
    let mut units_seen = false;
    let mut entries: Vec<ManifestEntry> = Vec::new();
    let mut draft: Option<EntryDraft> = None;

    for (index, raw_line) in text.split('\n').enumerate() {
        let line_no = index + 1;
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.is_empty() {
            continue;
        }
        if line.trim_start().starts_with('#') {
            return Err(ManifestError::Comment { line: line_no });
        }
        let indent = line.len() - line.trim_start().len();
        if line[..indent].contains('\t') {
            return Err(ManifestError::TabIndentation { line: line_no });
        }
        let body = line.trim_start();

        if indent == 0 {
            if body == "units:" {
                if units_seen {
                    return Err(ManifestError::DuplicateKey {
                        line: line_no,
                        key: "units",
                    });
                }
                units_seen = true;
                continue;
            }
            let Some((key, value)) = split_field(body) else {
                return Err(ManifestError::MalformedLine {
                    line: line_no,
                    text: line.to_owned(),
                });
            };
            match key {
                "schema" => {
                    if schema_seen {
                        return Err(ManifestError::DuplicateKey {
                            line: line_no,
                            key: "schema",
                        });
                    }
                    if value != MANIFEST_SCHEMA_TOKEN {
                        return Err(ManifestError::Schema {
                            line: line_no,
                            found: value.to_owned(),
                        });
                    }
                    schema_seen = true;
                }
                other => {
                    return Err(ManifestError::UnknownKey {
                        line: line_no,
                        key: other.to_owned(),
                    });
                }
            }
            continue;
        }

        if let Some(rest) = body.strip_prefix("- ") {
            if indent != 2 || !units_seen {
                return Err(ManifestError::MalformedLine {
                    line: line_no,
                    text: line.to_owned(),
                });
            }
            close_entry(&mut draft, &mut entries)?;
            let Some((key, value)) = split_field(rest) else {
                return Err(ManifestError::MalformedLine {
                    line: line_no,
                    text: line.to_owned(),
                });
            };
            if key != "id" {
                return Err(ManifestError::MalformedLine {
                    line: line_no,
                    text: line.to_owned(),
                });
            }
            let mut fresh = EntryDraft::default();
            fresh.set("id", line_no)?;
            fresh.id = Some(value.to_owned());
            draft = Some(fresh);
            continue;
        }

        if indent == 6 {
            // The evidence arm under this entry's `evidence:` marker.
            let Some(entry) = draft.as_mut() else {
                return Err(ManifestError::MalformedLine {
                    line: line_no,
                    text: line.to_owned(),
                });
            };
            if !entry.evidence_seen || entry.evidence.is_some() {
                return Err(ManifestError::MalformedLine {
                    line: line_no,
                    text: line.to_owned(),
                });
            }
            let Some((key, value)) = split_field(body) else {
                return Err(ManifestError::MalformedLine {
                    line: line_no,
                    text: line.to_owned(),
                });
            };
            entry.evidence = Some(parse_arm(line_no, key, value, allowed, allowed_features)?);
            continue;
        }

        if indent != 4 {
            return Err(ManifestError::MalformedLine {
                line: line_no,
                text: line.to_owned(),
            });
        }
        let Some(entry) = draft.as_mut() else {
            return Err(ManifestError::MalformedLine {
                line: line_no,
                text: line.to_owned(),
            });
        };
        let Some((key, value)) = split_field(body) else {
            // `evidence:` legitimately carries no value; any other bare
            // key is drift.
            if body.trim_end() == "evidence:" {
                entry.set("evidence", line_no)?;
                entry.evidence_seen = true;
                continue;
            }
            return Err(ManifestError::MalformedLine {
                line: line_no,
                text: line.to_owned(),
            });
        };
        let Some(known) = ENTRY_KEYS.iter().find(|candidate| **candidate == key) else {
            return Err(ManifestError::UnknownKey {
                line: line_no,
                key: key.to_owned(),
            });
        };
        entry.set(known, line_no)?;
        let known: &str = known;
        match known {
            "id" => entry.id = Some(value.to_owned()),
            "kind" => {
                if EvidenceKind::parse(value).is_err() {
                    return Err(ManifestError::BadKind {
                        line: line_no,
                        found: value.to_owned(),
                    });
                }
                entry.kind = Some(value.to_owned());
            }
            "frozen_locator" => entry.frozen_locator = Some(value.to_owned()),
            "evidence" => {
                return Err(ManifestError::MalformedLine {
                    line: line_no,
                    text: line.to_owned(),
                });
            }
            "bounded" => {
                let bounded = match value {
                    "true" => true,
                    "false" => false,
                    other => {
                        return Err(ManifestError::BadBool {
                            line: line_no,
                            found: other.to_owned(),
                        });
                    }
                };
                entry.bounded = Some(bounded);
            }
            "status_hint" => {
                let hint =
                    StatusHint::parse(value).ok_or_else(|| ManifestError::BadStatusHint {
                        line: line_no,
                        found: value.to_owned(),
                    })?;
                entry.status_hint = Some(hint);
            }
            "basis" => entry.basis = Some(value.to_owned()),
            "divergence" => {
                if !divergence_ok(value) {
                    return Err(ManifestError::BadDivergence {
                        line: line_no,
                        found: value.to_owned(),
                    });
                }
                entry.divergence = Some(value.to_owned());
            }
            _ => unreachable!("the key set was checked above"),
        }
    }
    close_entry(&mut draft, &mut entries)?;

    if !schema_seen {
        return Err(ManifestError::Schema {
            line: 0,
            found: "(missing)".to_owned(),
        });
    }
    if !units_seen {
        return Err(ManifestError::UnitsHeader {
            line: 0,
            text: "(missing)".to_owned(),
        });
    }
    if entries.is_empty() {
        return Err(ManifestError::EmptyEntryList);
    }
    Ok(entries)
}
