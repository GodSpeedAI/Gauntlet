//! Report assembly (charter decision 5): the frozen
//! `gauntlet.conformance@v1` document, built through the repo's own JSON
//! machinery ([`gauntlet_ports::json`]) — zero new dependencies.
//!
//! The `status_category` mapping is mechanical: PASS+unbounded ⇒
//! `implemented_and_proven`; PASS+bounded ⇒
//! `implemented_but_provisional`; INCONCLUSIVE ⇒ the entry's
//! `status_hint` (a `none` hint maps to `unknown`, disclosed — an
//! unforeseen inconclusive state is unknown, never a guess); FAIL ⇒
//! `blocked`.
//!
//! The two plan-gate python asserts hold BY CONSTRUCTION here:
//! outcomes are the three-state vocabulary, and PASS without a
//! non-empty, repo-relative, on-disk `evidence_ref` is REFUSED
//! (the invalid state is unrepresentable, and the refusal is tested).

use std::path::Path;

use gauntlet_ports::json::{JsonNumber, JsonValue};

/// The report schema token.
pub const REPORT_SCHEMA_TOKEN: &str = "gauntlet.conformance@v1";

/// The `_meta` disclosures the report always carries (the sweep's
/// honesty notes — decision 6's raw-tool rationale among them).
pub const DISCLOSURES: &[&str] = &[
    "command vocabulary: closed STRUCTURAL set — an allowlisted package with a cargo test \
     target or the lib selector plus at most one plain filter, or a bare absolute \
     executable with no arguments; there is no command string to inject and no shell \
     anywhere (the structural arm is the recorded task-27 deviation from the charter's \
     string example, forced by the frozen AK-016 literal boundary; see the task-27 B1 \
     report)",
    "per-target nextest invocations have no justfile equivalent (the recipes are \
     package/workspace scoped) — the raw tool under the repository's configured \
     parallelism is the honest spelling, the scripts-layer mutation precedent",
    "all cargo invocations run under the repository compile-lock discipline with the \
     lock acquisition bounded and the compiler cache daemon stopped inside the same \
     locked command; configured parallelism is never raised; the lock layer's own \
     outcomes (a lock that stayed busy; a wrapper that could not execute its command) \
     are environmental INCONCLUSIVE — never FAIL — with the wrapper remapping an \
     inner command's own reserved exit code so the lock-level pair stays unambiguous",
    "an INCONCLUSIVE unit whose entry carries no status_hint maps to the category unknown",
    "divergence citations marked unrecorded are real frozen-locator divergences whose \
     dated reference-candidates entry is owed (reported in the task-27 B1 report; B3 is \
     the single writer of reference-candidates.md)",
    "AK-070 is adjudicated from the recorded task-24 settlement artifact (dated, pinned), \
     not a fresh mutation re-run — the recorded-artifact posture, bounded",
    "AK-067's artifact lands with wave 2 (B3); until then it is honestly INCONCLUSIVE \
     (no_artifact)",
];

/// The report header facts the caller resolves (time, tree commit,
/// digests).
pub struct ReportHeader {
    pub generated_at: String,
    pub tree_commit: String,
    pub answer_key_path: String,
    pub answer_key_sha256: String,
    pub manifest_path: String,
    pub manifest_sha256: String,
    pub commands_executed: usize,
}

/// One unit's adjudicated disposition — the report row. Built by the
/// adjudicator; [`assemble_report`] validates every row's shape.
pub struct UnitDisposition {
    pub id: String,
    pub outcome: String,
    pub status_category: String,
    pub evidence_kind: String,
    pub frozen_locator: String,
    pub real_locator: String,
    pub divergence: String,
    pub evidence_ref: String,
    pub basis: String,
    pub reason: String,
}

/// Why assembly refused. The PASS-without-evidence invariant and the
/// vocabulary gates are assembly-level so they hold for EVERY caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AssemblyRefusal {
    PassWithoutEvidence { id: String, evidence_ref: String },
    OutcomeVocabulary { id: String, outcome: String },
    CategoryVocabulary { id: String, status_category: String },
    ReasonRequired { id: String, outcome: String },
    EmptyBasis { id: String },
}

impl core::fmt::Display for AssemblyRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::PassWithoutEvidence { id, evidence_ref } => write!(
                f,
                "{id}: PASS requires a non-empty repo-relative on-disk evidence_ref, got \
                 {evidence_ref:?}"
            ),
            Self::OutcomeVocabulary { id, outcome } => {
                write!(
                    f,
                    "{id}: {outcome:?} is not one of PASS, INCONCLUSIVE, FAIL"
                )
            }
            Self::CategoryVocabulary {
                id,
                status_category,
            } => write!(
                f,
                "{id}: {status_category:?} is not one of the six status categories"
            ),
            Self::ReasonRequired { id, outcome } => {
                write!(
                    f,
                    "{id}: a non-PASS outcome ({outcome}) requires a non-empty reason"
                )
            }
            Self::EmptyBasis { id } => write!(f, "{id}: the basis line is empty"),
        }
    }
}

impl std::error::Error for AssemblyRefusal {}

const CATEGORIES: &[&str] = &[
    "implemented_and_proven",
    "implemented_but_provisional",
    "configured_but_untested",
    "proposed",
    "blocked",
    "unknown",
];

/// A repo-relative, non-escaping evidence reference: the shape PASS
/// requires before the on-disk check.
fn reference_shape_ok(reference: &str) -> bool {
    // Component-wise (the N4 precedent): only Normal components — a
    // parent traversal is not a repo-relative evidence reference.
    !reference.is_empty()
        && !reference.starts_with('/')
        && std::path::Path::new(reference)
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
        && !reference.contains(char::is_whitespace)
}

/// Assembles the frozen report, refusing any invalid disposition. PASS
/// rows must cite a non-empty, repo-relative `evidence_ref` that either
/// is among the evidence files this sweep writes or exists on disk
/// under `workspace_root`.
pub fn assemble_report(
    header: &ReportHeader,
    dispositions: &[UnitDisposition],
    written_evidence: &[String],
    workspace_root: &Path,
) -> Result<JsonValue, AssemblyRefusal> {
    let mut rows = Vec::with_capacity(dispositions.len());
    let mut pass = 0usize;
    let mut inconclusive = 0usize;
    let mut fail = 0usize;
    for unit in dispositions {
        match unit.outcome.as_str() {
            "PASS" => pass += 1,
            "INCONCLUSIVE" => inconclusive += 1,
            "FAIL" => fail += 1,
            other => {
                return Err(AssemblyRefusal::OutcomeVocabulary {
                    id: unit.id.clone(),
                    outcome: other.to_owned(),
                });
            }
        }
        if !CATEGORIES.contains(&unit.status_category.as_str()) {
            return Err(AssemblyRefusal::CategoryVocabulary {
                id: unit.id.clone(),
                status_category: unit.status_category.clone(),
            });
        }
        if unit.outcome != "PASS" && unit.reason.trim().is_empty() {
            return Err(AssemblyRefusal::ReasonRequired {
                id: unit.id.clone(),
                outcome: unit.outcome.clone(),
            });
        }
        if unit.basis.trim().is_empty() {
            return Err(AssemblyRefusal::EmptyBasis {
                id: unit.id.clone(),
            });
        }
        if unit.outcome == "PASS" {
            if !reference_shape_ok(&unit.evidence_ref) {
                return Err(AssemblyRefusal::PassWithoutEvidence {
                    id: unit.id.clone(),
                    evidence_ref: unit.evidence_ref.clone(),
                });
            }
            let on_disk = written_evidence.contains(&unit.evidence_ref)
                || workspace_root.join(&unit.evidence_ref).is_file();
            if !on_disk {
                return Err(AssemblyRefusal::PassWithoutEvidence {
                    id: unit.id.clone(),
                    evidence_ref: unit.evidence_ref.clone(),
                });
            }
        }
        rows.push(JsonValue::Object(vec![
            ("id".to_owned(), JsonValue::Str(unit.id.clone())),
            ("outcome".to_owned(), JsonValue::Str(unit.outcome.clone())),
            (
                "status_category".to_owned(),
                JsonValue::Str(unit.status_category.clone()),
            ),
            (
                "evidence_kind".to_owned(),
                JsonValue::Str(unit.evidence_kind.clone()),
            ),
            (
                "frozen_locator".to_owned(),
                JsonValue::Str(unit.frozen_locator.clone()),
            ),
            (
                "real_locator".to_owned(),
                JsonValue::Str(unit.real_locator.clone()),
            ),
            (
                "divergence".to_owned(),
                JsonValue::Str(unit.divergence.clone()),
            ),
            (
                "evidence_ref".to_owned(),
                JsonValue::Str(unit.evidence_ref.clone()),
            ),
            ("basis".to_owned(), JsonValue::Str(unit.basis.clone())),
            ("reason".to_owned(), JsonValue::Str(unit.reason.clone())),
        ]));
    }
    let count = |value: usize| JsonValue::Number(JsonNumber::Uint(value as u64));
    Ok(JsonValue::Object(vec![
        (
            "schema".to_owned(),
            JsonValue::Str(REPORT_SCHEMA_TOKEN.to_owned()),
        ),
        (
            "generated_at".to_owned(),
            JsonValue::Str(header.generated_at.clone()),
        ),
        (
            "tree".to_owned(),
            JsonValue::Object(vec![(
                "commit".to_owned(),
                JsonValue::Str(header.tree_commit.clone()),
            )]),
        ),
        (
            "answer_key".to_owned(),
            JsonValue::Object(vec![
                (
                    "path".to_owned(),
                    JsonValue::Str(header.answer_key_path.clone()),
                ),
                (
                    "sha256".to_owned(),
                    JsonValue::Str(header.answer_key_sha256.clone()),
                ),
            ]),
        ),
        (
            "manifest".to_owned(),
            JsonValue::Object(vec![
                (
                    "path".to_owned(),
                    JsonValue::Str(header.manifest_path.clone()),
                ),
                (
                    "sha256".to_owned(),
                    JsonValue::Str(header.manifest_sha256.clone()),
                ),
            ]),
        ),
        ("units".to_owned(), JsonValue::Array(rows)),
        (
            "summary".to_owned(),
            JsonValue::Object(vec![
                ("total".to_owned(), count(dispositions.len())),
                ("pass".to_owned(), count(pass)),
                ("inconclusive".to_owned(), count(inconclusive)),
                ("fail".to_owned(), count(fail)),
            ]),
        ),
        (
            "_meta".to_owned(),
            JsonValue::Object(vec![
                (
                    "tool".to_owned(),
                    JsonValue::Str("gauntlet verify --all (gauntlet.conformance sweep)".to_owned()),
                ),
                (
                    "commands_executed".to_owned(),
                    count(header.commands_executed),
                ),
                (
                    "disclosures".to_owned(),
                    JsonValue::Array(
                        DISCLOSURES
                            .iter()
                            .map(|text| JsonValue::Str((*text).to_owned()))
                            .collect(),
                    ),
                ),
            ]),
        ),
    ]))
}
