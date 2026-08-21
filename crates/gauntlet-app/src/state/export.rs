//! Deterministic run export (spec §6.2: "Export is deterministic"; §18.1:
//! "Deterministic export to portable JSON/JSONL is REQUIRED regardless of
//! backend").
//!
//! The adapter's `export_jsonl` already orders its queries; this service
//! makes determinism a property of the APPLICATION contract rather than of
//! any one adapter: it parses every line the store produced (strictly —
//! model-shaped output is untrusted input, spec §21.2), validates it against
//! the canonical record shape, and re-emits the records in the canonical
//! total order with canonical field order. Two exports of the same history
//! are byte-identical whatever order the store handed the lines over in.
//!
//! The canonical total order:
//!
//! 1. `event` records, by offset (the append order — the audit trail);
//! 2. `checkpoint` records, by (iteration, write sequence);
//! 3. `settlement` records, by (unit, verification);
//! 4. the remaining §18.1 record kinds, by (kind, sequence).
//!
//! A `run` record, when present, leads. Two records carrying the same
//! canonical key are a typed error, not a stable sort's silent choice.
//! No timestamp is generated here: `at` fields are the stored ones, moved,
//! never minted.

use core::fmt;

use gauntlet_domain::RunId;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::json::{JsonNumber, JsonValue};
use gauntlet_ports::state_store::{StateStore, StateStoreError};

/// The §18.1 record kinds the `records` stream carries — everything except
/// the dedicated `run`, `event`, `checkpoint`, and `settlement` shapes.
const GENERIC_KINDS: &[&str] = &[
    "iteration",
    "role_invocation",
    "reference_version",
    "observation",
    "comparison",
    "verification_record",
    "residual",
    "payment",
    "failure",
    "recovery_event",
    "candidate_revision",
    "promotion_decision",
    "escalation",
];

/// Export failures — typed, carrying the 1-based line that caused them.
#[derive(Debug, Clone, PartialEq)]
pub enum ExportError {
    /// The store's own export failed.
    Store { detail: String },
    /// A line is not a JSON object.
    NotAnObject { line: usize },
    /// A line is not JSON at all.
    MalformedJson { line: usize, detail: String },
    /// The `record` tag is not part of the canonical shape.
    UnknownRecord { line: usize, record: String },
    /// A field outside the canonical shape for the record's kind.
    UnknownField { line: usize, field: String },
    /// A canonical field is absent.
    MissingField { line: usize, field: &'static str },
    /// A canonical field carries the wrong JSON shape.
    FieldType {
        line: usize,
        field: &'static str,
        expected: &'static str,
    },
    /// Two records claim the same canonical key — the export would be
    /// ambiguous, and ambiguity is not determinism.
    DuplicateKey { earlier: usize, later: usize },
}

impl fmt::Display for ExportError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Store { detail } => write!(f, "the store's export failed: {detail}"),
            Self::NotAnObject { line } => {
                write!(f, "line {line} is not a JSON object")
            }
            Self::MalformedJson { line, detail } => {
                write!(f, "line {line} is not valid JSON: {detail}")
            }
            Self::UnknownRecord { line, record } => write!(
                f,
                "line {line} names record kind {record:?}, which is not part of the canonical \
                 export shape"
            ),
            Self::UnknownField { line, field } => write!(
                f,
                "line {line} carries field {field:?}, which the record's canonical shape does \
                 not have"
            ),
            Self::MissingField { line, field } => {
                write!(f, "line {line} is missing the canonical field {field:?}")
            }
            Self::FieldType {
                line,
                field,
                expected,
            } => write!(f, "line {line} field {field:?} is not {expected}"),
            Self::DuplicateKey { earlier, later } => write!(
                f,
                "lines {earlier} and {later} carry the same canonical key — an ambiguous export \
                 is not a deterministic one"
            ),
        }
    }
}

impl std::error::Error for ExportError {}

impl From<StateStoreError> for ExportError {
    fn from(error: StateStoreError) -> Self {
        Self::Store {
            detail: error.to_string(),
        }
    }
}

/// Where a record sits in the canonical total order.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
enum Rank {
    Run,
    Event,
    Checkpoint,
    Settlement,
    Generic,
}

/// A record's position in the canonical total order: rank first, then the
/// kind (generic records order by kind), then the kind's own key parts —
/// ids as text (lexicographic is correct for ids), numbers as numbers
/// (numeric is correct for offsets, iterations, and sequences).
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord, Debug)]
struct CanonicalKey {
    rank: Rank,
    kind: String,
    text_key: String,
    number_key: u64,
    sequence_key: u64,
}

/// One admitted record: where it came from, its canonical key, and its
/// rebuilt canonical form.
struct Admitted {
    source_line: usize,
    key: CanonicalKey,
    line: JsonValue,
}

/// The canonical field order of each record kind (the shape every adapter's
/// export must produce, and the order this service re-emits).
fn canonical_fields(record: &str) -> &'static [&'static str] {
    match record {
        "run" => &[
            "record",
            "run_id",
            "created_at",
            "reference_id",
            "reference_version",
            "reference_digest",
            "reference_evidence_ref",
            "intent",
            "config_fingerprint",
            "environment_fingerprint",
            "parent_run",
            "budget",
        ],
        "event" => &["record", "offset", "at", "kind", "body"],
        "checkpoint" => &[
            "record",
            "seq",
            "iteration_no",
            "event_log_offset",
            "reference_digest",
            "unit_states",
            "budget_spent",
            "in_flight",
        ],
        "settlement" => &[
            "record",
            "unit_id",
            "verification",
            "evidence",
            "event_log_offset",
        ],
        _ => &["record", "seq", "at", "payload"],
    }
}

/// Exports one run canonically: parse strictly, validate against the
/// canonical shape, order by the canonical total order, emit with canonical
/// field order. Same durable history, byte-identical output — regardless of
/// the adapter underneath or the order it produced lines in.
pub fn export_run(
    store: &dyn StateStore,
    run_id: &RunId,
    deadline: Deadline,
) -> Result<String, ExportError> {
    let raw = store.export_jsonl(run_id, deadline)?;
    if raw.is_empty() {
        return Ok(String::new());
    }
    let mut admitted: Vec<Admitted> = Vec::new();
    for (index, line) in raw.split('\n').enumerate() {
        let number = index + 1;
        let parsed =
            gauntlet_ports::json::parse(line).map_err(|err| ExportError::MalformedJson {
                line: number,
                detail: err.to_string(),
            })?;
        let JsonValue::Object(entries) = parsed else {
            return Err(ExportError::NotAnObject { line: number });
        };
        let record = string_field(&entries, number, "record")?;
        let rank = match record.as_str() {
            "run" => Rank::Run,
            "event" => Rank::Event,
            "checkpoint" => Rank::Checkpoint,
            "settlement" => Rank::Settlement,
            other if GENERIC_KINDS.contains(&other) => Rank::Generic,
            other => {
                return Err(ExportError::UnknownRecord {
                    line: number,
                    record: other.to_owned(),
                });
            }
        };
        // Unknown fields are refused mechanically: the canonical shape is
        // the contract, and a field beside it is drift, not data.
        let fields = canonical_fields(&record);
        for (key, _) in &entries {
            if !fields.contains(&key.as_str()) {
                return Err(ExportError::UnknownField {
                    line: number,
                    field: key.clone(),
                });
            }
        }
        for field in fields {
            if !entries.iter().any(|(key, _)| key == field) {
                return Err(ExportError::MissingField {
                    line: number,
                    field,
                });
            }
        }
        let (text_key, number_key, sequence_key) = match rank {
            Rank::Run => (string_field(&entries, number, "run_id")?, 0, 0),
            Rank::Event => {
                // `at` is not part of the ordering key, but its SHAPE is
                // still the contract: a mis-shaped timestamp is drift, not
                // data, and passes through nothing.
                uint_field(&entries, number, "at")?;
                (String::new(), uint_field(&entries, number, "offset")?, 0)
            }
            Rank::Checkpoint => (
                String::new(),
                uint_field(&entries, number, "iteration_no")?,
                uint_field(&entries, number, "seq")?,
            ),
            Rank::Settlement => (
                format!(
                    "{}\u{1f}{}",
                    string_field(&entries, number, "unit_id")?,
                    string_field(&entries, number, "verification")?
                ),
                0,
                0,
            ),
            Rank::Generic => {
                uint_field(&entries, number, "at")?;
                (String::new(), uint_field(&entries, number, "seq")?, 0)
            }
        };
        let rebuilt = JsonValue::Object(
            fields
                .iter()
                .map(|field| {
                    let value = entries
                        .iter()
                        .find(|(key, _)| key == field)
                        .map(|(_, value)| value.clone())
                        .expect("presence was checked above");
                    ((*field).to_owned(), value)
                })
                .collect(),
        );
        admitted.push(Admitted {
            source_line: number,
            key: CanonicalKey {
                rank,
                kind: record.clone(),
                text_key,
                number_key,
                sequence_key,
            },
            line: rebuilt,
        });
    }
    // Duplicate canonical keys would make the output order ambiguous —
    // refuse rather than let a stable sort decide silently.
    for (index, current) in admitted.iter().enumerate() {
        if let Some(later) = admitted[index + 1..]
            .iter()
            .find(|other| other.key == current.key)
        {
            return Err(ExportError::DuplicateKey {
                earlier: current.source_line,
                later: later.source_line,
            });
        }
    }
    admitted.sort_by(|left, right| left.key.cmp(&right.key));
    Ok(admitted
        .iter()
        .map(|record| record.line.to_json_text())
        .collect::<Vec<_>>()
        .join("\n"))
}

/// A required string field of a record object.
fn string_field(
    entries: &[(String, JsonValue)],
    line: usize,
    field: &'static str,
) -> Result<String, ExportError> {
    match entries.iter().find(|(key, _)| key == field) {
        Some((_, JsonValue::Str(value))) => Ok(value.clone()),
        Some(_) => Err(ExportError::FieldType {
            line,
            field,
            expected: "a string",
        }),
        None => Err(ExportError::MissingField { line, field }),
    }
}

/// A required unsigned-integer field of a record object.
fn uint_field(
    entries: &[(String, JsonValue)],
    line: usize,
    field: &'static str,
) -> Result<u64, ExportError> {
    match entries.iter().find(|(key, _)| key == field) {
        Some((_, JsonValue::Number(JsonNumber::Uint(value)))) => Ok(*value),
        Some(_) => Err(ExportError::FieldType {
            line,
            field,
            expected: "an unsigned integer",
        }),
        None => Err(ExportError::MissingField { line, field }),
    }
}

#[cfg(test)]
pub(crate) mod stubs {
    //! Port stubs for the export tests: a scripted store whose export
    //! lines can be handed over in any order the test chooses.

    use core::cell::Cell;

    use gauntlet_domain::{EventLogOffset, Run, RunId};
    use gauntlet_ports::deadline::Deadline;
    use gauntlet_ports::state_store::{Checkpoint, RunEvent, StateStore, StateStoreError};

    /// A store that replays canned export lines, flipping between forward
    /// and reversed hand-over order on successive calls — adapters differ;
    /// the export contract must not.
    pub(crate) struct FlipFlopStore {
        lines: Vec<String>,
        reversed: Cell<bool>,
    }

    impl FlipFlopStore {
        pub(crate) fn new(lines: Vec<String>) -> Self {
            Self {
                lines,
                reversed: Cell::new(false),
            }
        }
    }

    impl StateStore for FlipFlopStore {
        fn append_event(
            &self,
            _event: RunEvent,
            _deadline: Deadline,
        ) -> Result<EventLogOffset, StateStoreError> {
            Err(StateStoreError::WriteFailed {
                detail: "the stub is read-only".to_owned(),
            })
        }

        fn load_run(
            &self,
            _run_id: &RunId,
            _deadline: Deadline,
        ) -> Result<Option<Run>, StateStoreError> {
            Ok(None)
        }

        fn checkpoint(
            &self,
            _checkpoint: Checkpoint,
            _deadline: Deadline,
        ) -> Result<(), StateStoreError> {
            Err(StateStoreError::WriteFailed {
                detail: "the stub is read-only".to_owned(),
            })
        }

        fn resume(
            &self,
            _run_id: &RunId,
            _deadline: Deadline,
        ) -> Result<Option<Checkpoint>, StateStoreError> {
            Ok(None)
        }

        fn export_jsonl(
            &self,
            _run_id: &RunId,
            _deadline: Deadline,
        ) -> Result<String, StateStoreError> {
            let mut lines = self.lines.clone();
            if self.reversed.get() {
                lines.reverse();
            }
            self.reversed.set(!self.reversed.get());
            Ok(lines.join("\n"))
        }
    }
}

/// The gate filter `state::export::deterministic` depends on this exact
/// module path existing (plan Task 6).
#[cfg(test)]
mod deterministic {
    use super::stubs::FlipFlopStore;
    use super::*;
    use gauntlet_domain::EpochSeconds;

    /// A fixed point far from any clock: exports must never read one.
    const MOMENT_A: u64 = 1_700_000_001;
    const MOMENT_B: u64 = 1_700_000_002;
    const OFF_A: u64 = 1;
    const OFF_B: u64 = 2;
    const SEQ_A: u64 = 1;
    const SEQ_B: u64 = 2;

    fn run_id() -> RunId {
        RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("a valid run id")
    }

    fn bound() -> Deadline {
        Deadline::at_epoch(EpochSeconds::new(MOMENT_B))
    }

    /// A history whose hand-over order (reversed on the second export) is
    /// NOT its canonical order: events 2 before 1, the later checkpoint
    /// before the earlier one, and the settlement between them.
    fn history() -> Vec<String> {
        let zeros = concat!(
            r#""budget_spent":{"rounds":0,"#,
            r#""wall_seconds":0,"tokens":0,"cost_usd":0,"diagnostic_tokens":0,"diagnostic_cost_usd":0}"#
        );
        vec![
            format!(
                r#"{{"record":"observation","seq":{SEQ_B},"at":{MOMENT_B},"payload":{{"facet":"filesystem"}}}}"#
            ),
            format!(
                r#"{{"record":"checkpoint","seq":{SEQ_A},"iteration_no":{OFF_B},"event_log_offset":{OFF_B},"reference_digest":"sha256:c4d3e2f1a0b1c2d3","unit_states":[["AK-014","in_progress"]],{zeros},"in_flight":[]}}"#
            ),
            format!(
                r#"{{"record":"settlement","unit_id":"AK-014","verification":"vrf-000042","evidence":"evd-000042","event_log_offset":{OFF_B}}}"#
            ),
            format!(
                r#"{{"record":"event","offset":{OFF_B},"at":{MOMENT_B},"kind":"settlement_committed","body":{{"kind":"settlement_committed","unit_id":"AK-014","evidence":"evd-000042","verification":"vrf-000042","reference_digest":"sha256:c4d3e2f1a0b1c2d3"}}}}"#
            ),
            format!(
                r#"{{"record":"checkpoint","seq":{SEQ_B},"iteration_no":{OFF_A},"event_log_offset":{OFF_A},"reference_digest":"sha256:c4d3e2f1a0b1c2d3","unit_states":[["AK-014","open"]],{zeros},"in_flight":[]}}"#
            ),
            format!(
                r#"{{"record":"event","offset":{OFF_A},"at":{MOMENT_A},"kind":"invocation_admitted","body":{{"kind":"invocation_admitted","invocation_id":"01ARZ3NDEKTSV4RRFFQ69G5FAW"}}}}"#
            ),
        ]
    }

    /// The canonical form of [`history`]: events by offset, checkpoints by
    /// iteration, the settlement, then the generic record.
    fn canonical() -> String {
        let zeros = concat!(
            r#""budget_spent":{"rounds":0,"#,
            r#""wall_seconds":0,"tokens":0,"cost_usd":0,"diagnostic_tokens":0,"diagnostic_cost_usd":0}"#
        );
        [
            format!(
                r#"{{"record":"event","offset":{OFF_A},"at":{MOMENT_A},"kind":"invocation_admitted","body":{{"kind":"invocation_admitted","invocation_id":"01ARZ3NDEKTSV4RRFFQ69G5FAW"}}}}"#
            ),
            format!(
                r#"{{"record":"event","offset":{OFF_B},"at":{MOMENT_B},"kind":"settlement_committed","body":{{"kind":"settlement_committed","unit_id":"AK-014","evidence":"evd-000042","verification":"vrf-000042","reference_digest":"sha256:c4d3e2f1a0b1c2d3"}}}}"#
            ),
            format!(
                r#"{{"record":"checkpoint","seq":{SEQ_B},"iteration_no":{OFF_A},"event_log_offset":{OFF_A},"reference_digest":"sha256:c4d3e2f1a0b1c2d3","unit_states":[["AK-014","open"]],{zeros},"in_flight":[]}}"#
            ),
            format!(
                r#"{{"record":"checkpoint","seq":{SEQ_A},"iteration_no":{OFF_B},"event_log_offset":{OFF_B},"reference_digest":"sha256:c4d3e2f1a0b1c2d3","unit_states":[["AK-014","in_progress"]],{zeros},"in_flight":[]}}"#
            ),
            format!(
                r#"{{"record":"settlement","unit_id":"AK-014","verification":"vrf-000042","evidence":"evd-000042","event_log_offset":{OFF_B}}}"#
            ),
            format!(
                r#"{{"record":"observation","seq":{SEQ_B},"at":{MOMENT_B},"payload":{{"facet":"filesystem"}}}}"#
            ),
        ]
        .join("\n")
    }

    /// The determinism gate: the same history exports byte-identically even
    /// when the store hands the lines over in a different order each time,
    /// and the output is the canonical order, not the arrival order.
    #[test]
    fn export_is_byte_identical_and_canonical_under_reordered_input() {
        let store = FlipFlopStore::new(history());
        let first = export_run(&store, &run_id(), bound()).expect("exports");
        let second = export_run(&store, &run_id(), bound()).expect("exports again");
        assert_eq!(
            first, second,
            "the same history must export byte-identically every time"
        );
        assert_eq!(
            first,
            canonical(),
            "the export is the canonical order, not the store's hand-over order"
        );
    }

    #[test]
    fn an_empty_history_exports_empty() {
        let store = FlipFlopStore::new(Vec::new());
        assert_eq!(
            export_run(&store, &run_id(), bound()).expect("exports"),
            String::new()
        );
    }

    #[test]
    fn malformed_lines_are_typed_errors() {
        let store = FlipFlopStore::new(vec!["{not json".to_owned()]);
        let error = export_run(&store, &run_id(), bound()).expect_err("garbage is refused");
        assert!(
            matches!(error, ExportError::MalformedJson { line: 1, .. }),
            "{error}"
        );
    }

    #[test]
    fn unknown_record_kinds_are_typed_errors() {
        let store = FlipFlopStore::new(vec![
            r#"{"record":"hologram","seq":1,"at":1,"payload":{}}"#.to_owned(),
        ]);
        let error = export_run(&store, &run_id(), bound()).expect_err("drift is refused");
        assert!(
            matches!(error, ExportError::UnknownRecord { ref record, .. } if record == "hologram"),
            "{error}"
        );
    }

    #[test]
    fn fields_outside_the_canonical_shape_are_refused() {
        let store = FlipFlopStore::new(vec![
            r#"{"record":"event","offset":1,"at":1,"kind":"checkpoint_written","body":{},"extra":true}"#
                .to_owned(),
        ]);
        let error = export_run(&store, &run_id(), bound()).expect_err("drift is refused");
        assert!(
            matches!(error, ExportError::UnknownField { ref field, .. } if field == "extra"),
            "{error}"
        );
    }

    #[test]
    fn duplicate_canonical_keys_are_refused() {
        let line = r#"{"record":"event","offset":1,"at":1,"kind":"checkpoint_written","body":{}}"#;
        let store = FlipFlopStore::new(vec![line.to_owned(), line.to_owned()]);
        let error = export_run(&store, &run_id(), bound()).expect_err("ambiguity is refused");
        assert!(matches!(error, ExportError::DuplicateKey { .. }), "{error}");
    }

    #[test]
    fn mis_shaped_at_fields_are_typed_errors() {
        // Event `at` — presence was checked, but the shape was not; a
        // mis-shaped value must be a FieldType error, not a passthrough.
        let store = FlipFlopStore::new(vec![
            r#"{"record":"event","offset":1,"at":"soon","kind":"checkpoint_written","body":{}}"#
                .to_owned(),
        ]);
        let error = export_run(&store, &run_id(), bound()).expect_err("drift is refused");
        assert!(
            matches!(error, ExportError::FieldType { field: "at", .. }),
            "{error}"
        );
        // Generic records' `at` — same rule.
        let store = FlipFlopStore::new(vec![
            r#"{"record":"observation","seq":1,"at":"soon","payload":{}}"#.to_owned(),
        ]);
        let error = export_run(&store, &run_id(), bound()).expect_err("drift is refused");
        assert!(
            matches!(error, ExportError::FieldType { field: "at", .. }),
            "{error}"
        );
    }

    #[test]
    fn missing_canonical_fields_are_typed_errors() {
        let store = FlipFlopStore::new(vec![
            r#"{"record":"settlement","unit_id":"AK-014","verification":"vrf-000042"}"#.to_owned(),
        ]);
        let error = export_run(&store, &run_id(), bound()).expect_err("drift is refused");
        assert!(
            matches!(error, ExportError::MissingField { field, .. } if field == "evidence"),
            "{error}"
        );
    }
}
