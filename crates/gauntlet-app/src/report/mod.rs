//! The plan Task 20 step-3 report generator (spec §26): the final report
//! "distinguishes **implemented and proven · implemented but provisional ·
//! configured but untested · proposed · blocked · unknown**. Provisional
//! capability is never reported as complete."
//!
//! The service is a PURE READ SIDE over the durable run records. It writes
//! nothing, it composes only existing [`StateStore`] reads (`load_run`,
//! `resume`, `settlements`, `verification_records`), and every category it
//! populates is decided by records — unit statuses projected by the latest
//! durable checkpoint, settlement rows, and verification records — never by
//! a string literal claiming capability. The module does not know what code
//! exists; it reports what the records say.
//!
//! ## Population rules (each category, with its deciding record)
//!
//! - `implemented-and-proven` — a durable settlement row for the unit AND a
//!   durable verification record whose ref is the settlement's and whose
//!   outcome is PASS. The pair is the proof; either half alone is not.
//! - `provisional` — recorded attempt without recorded proof: a settlement
//!   row with no durable PASS record carrying its ref (missing evidence is
//!   not passing evidence); a FAIL or INCONCLUSIVE verification record; a
//!   PASS record no settlement commits; a checkpoint projection of
//!   `in_progress`; or a checkpoint projection of `settled` with no
//!   settlement row (the projection's claim never outruns the durable row).
//! - `blocked` — the latest durable checkpoint projects the unit `blocked`
//!   (units never dispatched because of budget or governance land here).
//! - `configured-but-untested` — the checkpoint names the unit `open`:
//!   configured into the run, never attempted on record.
//! - `proposed` — capability-scope only: the caller's manifest declares the
//!   capability not wired. The report never upgrades the declaration.
//! - `unknown` — the caller's manifest binds a unit that NO durable record
//!   of the run names: unattempted, nothing is known. No category is ever
//!   populated for a unit or capability without either a durable record or
//!   an explicit manifest entry; an all-empty store with an empty manifest
//!   populates nothing.
//!
//! A unit the checkpoint projects `out_of_scope` that no record ever
//! attempted is surfaced separately (the six categories are the vocabulary;
//! out-of-scope is not one of them). If records show the unit was attempted
//! or settled anyway, the records decide — the projection never hides
//! activity.
//!
//! Precedence, when records disagree (deterministic, documented): the
//! authority-minted settlement+PASS pair outranks a stale projection (a
//! checkpoint is written at iteration boundaries and can lag the durable
//! settlement row); a `blocked` projection outranks unproven attempt
//! records (the run's actual recorded status decides where an
//! INCONCLUSIVE-blocked unit lands); everything else falls through the
//! rules above in order.
//!
//! ## Capability scope
//!
//! Capability-scope rows (e.g. which §20.1 commands are wired) come from the
//! CALLER-SUPPLIED [`CapabilityManifestEntry`] list — the CLI owns declaring
//! it from its real command table. The manifest entry's `wired` flag is the
//! caller's structural fact about its own surface; a wired entry with no
//! bound unit or no run records is reported `configured-but-untested`, and
//! its proof can only come from the bound unit's durable records.
//!
//! ## Determinism
//!
//! The machine rendering ([`RunReport::to_json`]) is a function of the
//! records alone: rows are sorted (units by id, capabilities by key), the
//! category object always carries the six tokens in report order, and no
//! timestamp is minted — every value is moved from a record. Two builds
//! over identical records are byte-identical whatever order the store
//! handed the rows over in. The human summary ([`RunReport::human_summary`])
//! renders from the same struct.

use core::fmt;

use gauntlet_domain::{Outcome, RunId, SettlementRecord, TerminalState, UnitId, UnitStatus};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::json::JsonValue;
use gauntlet_ports::state_store::{StateStore, StateStoreError};
use gauntlet_ports::verification_record::VerificationRecordDoc;

/// The six §26 categories, in report order. The tokens are the plan Task 20
/// step-3 spellings, exact-match.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum StatusCategory {
    /// A durable settlement row AND the durable PASS verification record it
    /// names — proof, from records.
    ImplementedAndProven,
    /// Recorded work without recorded proof. Never reported as complete.
    Provisional,
    /// Configured into the surface (or the run) with no attempt on record.
    ConfiguredButUntested,
    /// The caller's manifest declares the capability not wired.
    Proposed,
    /// The run's durable projection records the unit as blocked.
    Blocked,
    /// No durable record names the unit: nothing is known.
    Unknown,
}

impl StatusCategory {
    /// Every category, in report order (the order the renderings use).
    pub const ALL: &'static [Self] = &[
        Self::ImplementedAndProven,
        Self::Provisional,
        Self::ConfiguredButUntested,
        Self::Proposed,
        Self::Blocked,
        Self::Unknown,
    ];

    /// The exact category token (plan Task 20 step 3). Note: spec §26 and
    /// AK-067 phrase this category "implemented but provisional"; the plan's
    /// spelling is the one rendered here.
    pub fn token(self) -> &'static str {
        match self {
            Self::ImplementedAndProven => "implemented-and-proven",
            Self::Provisional => "provisional",
            Self::ConfiguredButUntested => "configured-but-untested",
            Self::Proposed => "proposed",
            Self::Blocked => "blocked",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for StatusCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// Report failures — typed (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum ReportError {
    /// A store read failed; the detail carries the store's own locus.
    Store { detail: String },
    /// A manifest entry carried an empty capability key.
    ManifestKeyEmpty,
    /// Two manifest entries claim the same capability key — the report would
    /// be ambiguous, and ambiguity is not a report.
    DuplicateManifestKey { key: String },
}

impl fmt::Display for ReportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store { detail } => write!(f, "the report's store read failed: {detail}"),
            Self::ManifestKeyEmpty => {
                write!(f, "a capability manifest entry has an empty key")
            }
            Self::DuplicateManifestKey { key } => {
                write!(f, "the capability manifest names {key:?} twice")
            }
        }
    }
}

impl std::error::Error for ReportError {}

impl From<StateStoreError> for ReportError {
    fn from(error: StateStoreError) -> Self {
        Self::Store {
            detail: error.to_string(),
        }
    }
}

/// One capability-scope row the CALLER supplies (the CLI owns declaring the
/// manifest from its real command table — the report module invents no
/// capability). `wired` is the caller's structural fact about its own
/// surface; `evidence_unit`, when given, binds the run-scope unit whose
/// durable records may prove the capability.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityManifestEntry {
    /// The capability key as the surface names it (e.g. a §20.1 command).
    pub key: String,
    /// Whether the caller's real table wires the capability.
    pub wired: bool,
    /// The run-scope unit bound as the capability's evidence, when one is.
    pub evidence_unit: Option<UnitId>,
}

impl CapabilityManifestEntry {
    /// Validates the key is present; the rest is the caller's declaration.
    pub fn new(
        key: String,
        wired: bool,
        evidence_unit: Option<UnitId>,
    ) -> Result<Self, ReportError> {
        if key.trim().is_empty() {
            return Err(ReportError::ManifestKeyEmpty);
        }
        Ok(Self {
            key,
            wired,
            evidence_unit,
        })
    }
}

/// One run-scope row: a unit named by durable records, its category, and
/// the record facts that decided it (citations, never claims).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnitRow {
    pub unit: UnitId,
    pub category: StatusCategory,
    pub basis: String,
}

/// One capability-scope row: the caller's manifest key, its category, and
/// the basis — the caller declaration plus, when bound, the unit records.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReportCapabilityRow {
    pub key: String,
    pub wired: bool,
    pub category: StatusCategory,
    pub basis: String,
}

/// The typed run report. Built only through [`build_report`]; both
/// renderings read this one struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunReport {
    pub run_id: RunId,
    /// Whether the store holds a durable identity record for the run.
    pub run_present: bool,
    /// The run's recorded terminal state, when it ended.
    pub terminal_state: Option<TerminalState>,
    /// Every unit the durable records name, sorted by unit id.
    pub units: Vec<UnitRow>,
    /// Every caller manifest entry, sorted by key.
    pub capabilities: Vec<RunReportCapabilityRow>,
    /// Units the checkpoint projects `out_of_scope` that no record ever
    /// attempted — surfaced, not silently dropped, and not one of the six
    /// categories.
    pub out_of_scope: Vec<UnitId>,
}

impl RunReport {
    /// How many units sit in one category.
    pub fn count(&self, category: StatusCategory) -> usize {
        self.units
            .iter()
            .filter(|row| row.category == category)
            .count()
    }

    /// The canonical machine rendering: fixed field order, the six category
    /// tokens always present in report order, rows sorted. A function of the
    /// records alone — no clock is read, so identical records render
    /// byte-identically.
    pub fn to_json(&self) -> String {
        let categories = StatusCategory::ALL
            .iter()
            .map(|category| {
                let units: Vec<JsonValue> = self
                    .units
                    .iter()
                    .filter(|row| row.category == *category)
                    .map(|row| JsonValue::Str(row.unit.as_str().to_owned()))
                    .collect();
                ((*category).token().to_owned(), JsonValue::Array(units))
            })
            .collect();
        let units = self
            .units
            .iter()
            .map(|row| {
                JsonValue::Object(vec![
                    (
                        "unit".to_owned(),
                        JsonValue::Str(row.unit.as_str().to_owned()),
                    ),
                    (
                        "category".to_owned(),
                        JsonValue::Str(row.category.token().to_owned()),
                    ),
                    ("basis".to_owned(), JsonValue::Str(row.basis.clone())),
                ])
            })
            .collect();
        let capabilities = self
            .capabilities
            .iter()
            .map(|row| {
                JsonValue::Object(vec![
                    ("key".to_owned(), JsonValue::Str(row.key.clone())),
                    ("wired".to_owned(), JsonValue::Bool(row.wired)),
                    (
                        "category".to_owned(),
                        JsonValue::Str(row.category.token().to_owned()),
                    ),
                    ("basis".to_owned(), JsonValue::Str(row.basis.clone())),
                ])
            })
            .collect();
        let out_of_scope = self
            .out_of_scope
            .iter()
            .map(|unit| JsonValue::Str(unit.as_str().to_owned()))
            .collect();
        let document = JsonValue::Object(vec![
            (
                "run_id".to_owned(),
                JsonValue::Str(self.run_id.as_str().to_owned()),
            ),
            ("run_present".to_owned(), JsonValue::Bool(self.run_present)),
            (
                "terminal_state".to_owned(),
                match &self.terminal_state {
                    Some(state) => JsonValue::Str(state.wire()),
                    None => JsonValue::Null,
                },
            ),
            ("categories".to_owned(), JsonValue::Object(categories)),
            ("units".to_owned(), JsonValue::Array(units)),
            ("capabilities".to_owned(), JsonValue::Array(capabilities)),
            ("out_of_scope".to_owned(), JsonValue::Array(out_of_scope)),
        ]);
        document.to_json_text()
    }

    /// The concise human summary, from the same struct: a counts line
    /// (always carrying all six tokens, in report order), one line per
    /// populated unit category, and one line for capability rows when the
    /// caller supplied a manifest.
    pub fn human_summary(&self) -> String {
        let mut lines = Vec::new();
        let counts = StatusCategory::ALL
            .iter()
            .map(|category| format!("{} {}", category.token(), self.count(*category)))
            .collect::<Vec<_>>()
            .join(", ");
        let mut first = format!("run {}: {}", self.run_id.as_str(), counts);
        first.push_str(&format!("; out_of_scope {}", self.out_of_scope.len()));
        if let Some(state) = &self.terminal_state {
            first.push_str(&format!("; terminal {}", state.wire()));
        }
        if !self.run_present {
            first.push_str("; no run record in the store");
        }
        lines.push(first);
        for category in StatusCategory::ALL {
            let units: Vec<&str> = self
                .units
                .iter()
                .filter(|row| row.category == *category)
                .map(|row| row.unit.as_str())
                .collect();
            if !units.is_empty() {
                lines.push(format!("{}: {}", category.token(), units.join(", ")));
            }
        }
        if !self.capabilities.is_empty() {
            let rows = self
                .capabilities
                .iter()
                .map(|row| format!("{}={}", row.key, row.category.token()))
                .collect::<Vec<_>>()
                .join(", ");
            lines.push(format!("capabilities: {rows}"));
        }
        lines.join("\n")
    }
}

/// Builds the run report from durable records. Reads only — the store is
/// never written. An all-empty store yields a report with every category
/// empty (`run_present == false`), never an error: the report says what the
/// records say, and empty records say nothing.
pub fn build_report(
    store: &dyn StateStore,
    run_id: &RunId,
    manifest: &[CapabilityManifestEntry],
    deadline: Deadline,
) -> Result<RunReport, ReportError> {
    for (index, entry) in manifest.iter().enumerate() {
        if entry.key.trim().is_empty() {
            return Err(ReportError::ManifestKeyEmpty);
        }
        if manifest[index + 1..]
            .iter()
            .any(|other| other.key == entry.key)
        {
            return Err(ReportError::DuplicateManifestKey {
                key: entry.key.clone(),
            });
        }
    }

    let run = store.load_run(run_id, deadline)?;
    let checkpoint = store.resume(run_id, deadline)?;
    // Typed reads only; a store that does not implement the
    // settlement/verification reads fails closed here (COR-7), which is the
    // report failing loudly, never under-informing the operator.
    // Same run-scope discipline as the verification records below: rows of
    // another run are not this run's records and never enter the report.
    let settlements: Vec<SettlementRecord> = store
        .settlements(run_id, deadline)?
        .into_iter()
        .filter(|row| row.run_id() == run_id)
        .collect();
    let verifications: Vec<VerificationRecordDoc> = store
        .verification_records(run_id, deadline)?
        .into_iter()
        .filter(|record| record.run_id() == run_id)
        .collect();

    // The unit universe: every id the durable records name. Sorted — the
    // report is a function of the records, not of the store's hand-over
    // order.
    let mut ids: Vec<UnitId> = Vec::new();
    if let Some(checkpoint) = &checkpoint {
        for (unit, _) in &checkpoint.unit_states {
            if !ids.contains(unit) {
                ids.push(unit.clone());
            }
        }
    }
    for row in &settlements {
        if !ids.contains(row.unit_id()) {
            ids.push(row.unit_id().clone());
        }
    }
    for record in &verifications {
        if !ids.contains(record.unit_id()) {
            ids.push(record.unit_id().clone());
        }
    }
    ids.sort();

    let mut units = Vec::new();
    let mut out_of_scope = Vec::new();
    for unit in &ids {
        let status = checkpoint.as_ref().and_then(|checkpoint| {
            checkpoint
                .unit_states
                .iter()
                .find(|(candidate, _)| candidate == unit)
                .map(|(_, status)| *status)
        });
        let settlement = settlements.iter().find(|row| row.unit_id() == unit);
        let records: Vec<&VerificationRecordDoc> = verifications
            .iter()
            .filter(|record| record.unit_id() == unit)
            .collect();
        let (category, basis) = classify_unit(status, settlement, &records);
        // Out-of-scope units that were never attempted are surfaced
        // separately; recorded activity keeps the classified category (the
        // projection never hides activity).
        if status == Some(UnitStatus::OutOfScope)
            && category == StatusCategory::ConfiguredButUntested
        {
            out_of_scope.push(unit.clone());
            continue;
        }
        units.push(UnitRow {
            unit: unit.clone(),
            category,
            basis,
        });
    }

    let mut capabilities = Vec::new();
    for entry in manifest {
        let (category, basis) = classify_capability(entry, &units, run_id);
        capabilities.push(RunReportCapabilityRow {
            key: entry.key.clone(),
            wired: entry.wired,
            category,
            basis,
        });
    }
    capabilities.sort_by(|left, right| left.key.cmp(&right.key));

    Ok(RunReport {
        run_id: run_id.clone(),
        run_present: run.is_some(),
        terminal_state: run.as_ref().and_then(|run| run.terminal_state()),
        units,
        capabilities,
        out_of_scope,
    })
}

/// The run-scope classification for one unit, from its records. See the
/// module docs for the rules and their precedence.
fn classify_unit(
    status: Option<UnitStatus>,
    settlement: Option<&SettlementRecord>,
    records: &[&VerificationRecordDoc],
) -> (StatusCategory, String) {
    // 1. The proof pair: settlement row + the durable PASS record it names.
    if let Some(row) = settlement {
        let proven = records.iter().any(|record| {
            record.verification() == row.verification() && record.outcome() == Outcome::Pass
        });
        if proven {
            return (
                StatusCategory::ImplementedAndProven,
                format!(
                    "settlement row names verification {} with a durable PASS record at evidence {}",
                    row.verification().as_str(),
                    row.evidence().as_str()
                ),
            );
        }
    }
    // 2. The run's actual recorded decision: blocked. An INCONCLUSIVE-blocked
    // unit lands here — the status the run recorded decides.
    if status == Some(UnitStatus::Blocked) {
        let mut basis = "the latest checkpoint projects blocked".to_owned();
        if let Some(row) = settlement {
            basis.push_str(&format!(
                "; a settlement row names verification {} without a durable PASS record, which cannot upgrade the recorded status",
                row.verification().as_str()
            ));
        }
        return (StatusCategory::Blocked, basis);
    }
    // 3. Recorded attempt without recorded proof.
    let mut reasons: Vec<String> = Vec::new();
    if let Some(row) = settlement {
        reasons.push(format!(
            "settlement row names verification {} but no durable PASS record carries that ref",
            row.verification().as_str()
        ));
    }
    for record in records {
        match record.outcome() {
            Outcome::Pass => reasons.push(format!(
                "verification record {} is PASS but no settlement record commits it",
                record.verification().as_str()
            )),
            other => reasons.push(format!(
                "verification record {} is {other}; an unproven outcome never settles",
                record.verification().as_str()
            )),
        }
    }
    match status {
        Some(UnitStatus::InProgress) => {
            reasons.push("the latest checkpoint projects in_progress".to_owned());
        }
        Some(UnitStatus::Settled) => {
            reasons.push(
                "the latest checkpoint projects settled but no settlement record exists".to_owned(),
            );
        }
        _ => {}
    }
    if !reasons.is_empty() {
        return (StatusCategory::Provisional, reasons.join("; "));
    }
    // 4. Named by the run, never attempted on record.
    (
        StatusCategory::ConfiguredButUntested,
        "the latest checkpoint names the unit open — configured into the run, never attempted on record"
            .to_owned(),
    )
}

/// The capability-scope classification: the caller's declaration crossed
/// with the bound unit's run-scope records (when one is bound). A not-wired
/// declaration is never upgraded.
fn classify_capability(
    entry: &CapabilityManifestEntry,
    units: &[UnitRow],
    run_id: &RunId,
) -> (StatusCategory, String) {
    if !entry.wired {
        return (
            StatusCategory::Proposed,
            "the caller's manifest declares the capability not wired; the report claims nothing"
                .to_owned(),
        );
    }
    match &entry.evidence_unit {
        None => (
            StatusCategory::ConfiguredButUntested,
            "wired per the caller's manifest; the manifest binds no run unit as its evidence"
                .to_owned(),
        ),
        Some(unit) => match units.iter().find(|row| &row.unit == unit) {
            Some(row) => (
                row.category,
                format!(
                    "wired per the caller's manifest; bound unit {} — {}",
                    unit.as_str(),
                    row.basis
                ),
            ),
            None => (
                StatusCategory::Unknown,
                format!(
                    "wired per the caller's manifest, but no durable record in run {} names unit {} — unattempted, nothing is known",
                    run_id.as_str(),
                    unit.as_str()
                ),
            ),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The category tokens are the plan's exact spellings, in report order.
    #[test]
    fn the_six_category_tokens_are_exact() {
        let tokens: Vec<&str> = StatusCategory::ALL
            .iter()
            .map(|category| category.token())
            .collect();
        assert_eq!(
            tokens,
            vec![
                "implemented-and-proven",
                "provisional",
                "configured-but-untested",
                "proposed",
                "blocked",
                "unknown",
            ]
        );
    }

    /// A manifest entry's key is required; an empty one is typed, never a
    /// silent row.
    #[test]
    fn an_empty_manifest_key_is_refused() {
        assert!(matches!(
            CapabilityManifestEntry::new(String::new(), true, None),
            Err(ReportError::ManifestKeyEmpty)
        ));
        assert!(matches!(
            CapabilityManifestEntry::new("   ".to_owned(), true, None),
            Err(ReportError::ManifestKeyEmpty)
        ));
    }

    /// Store failures surface typed, carrying the store's locus.
    #[test]
    fn a_store_failure_is_typed() {
        let error = ReportError::from(StateStoreError::ReadFailed {
            detail: "disk gone".to_owned(),
        });
        assert!(
            matches!(error, ReportError::Store { .. }) && error.to_string().contains("disk gone"),
            "{error}"
        );
    }
}
