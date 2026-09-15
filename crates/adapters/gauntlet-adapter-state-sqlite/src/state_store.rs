//! The `StateStore` adapter over the embedded database (spec §6.2, §18).
//!
//! Everything durable about a run lives here: its identity (`runs`), its
//! §18.2 checkpoints, its §16.3 settlement rows, and the remaining §18.1
//! record kinds (`records`). All writes are `INSERT` — there is no UPDATE or
//! DELETE anywhere in this crate; idempotency comes from keys, never from
//! rewriting history.
//!
//! `load_run` returns the authoritative `runs` row when there is one. A run
//! known only through the port's own writes (events and checkpoints) gets a
//! documented projection: the port's durable records carry the run id and
//! frozen reference digest but not the full `RunDefinition`, so the fields
//! they never carried come back as explicit placeholders. Recording the run
//! ([`SqliteStateStore::record_run`], the composition root's job) is what
//! makes the record authoritative.
//!
//! One recorded port gap: no production caller of `Budget::charge` exists
//! yet (the controller lands with Task 12's later slices), so every
//! `Checkpoint` value that can exist today carries zero spend, and `resume`
//! returns exactly that. The six counters stay durable in the database and
//! the export; the domain's reconstruction path (`Spent::from_counters` +
//! `Budget::restore`, Task 12) makes a non-zero resume representable, but
//! admitting it belongs to the resume composition that can read the run's
//! durable ceilings — until then a non-zero value is refused loudly rather
//! than silently zeroed.
//!
//! Port methods carry a `Deadline` (spec §6 preamble); like the event log,
//! this adapter runs single bounded statements with no retry loop and no
//! clock of its own, so the bound is noted and not consulted.

use std::path::Path;
use std::sync::Mutex;

use gauntlet_domain::{
    Authority, BlastRadius, Budget, CorrectionDescriptor, CorrectionSelection, CorrectionShape,
    DecisionStep, DiagnosticAllowance, Digest, EpochSeconds, EventLogOffset, EvidenceLocator,
    EvidenceRef, InstrumentName, IterationNo, Limits, MicroUsd, ReferenceId, ReferenceVersion,
    RejectedAlternative, RejectionClass, RejectionReason, RoleInvocation, Run, RunDefinition,
    RunId, SelectionDimension, SettlementRecord, Spent, UnitId, VerificationRef,
};
use gauntlet_ports::comparison_record::ComparisonRecordDoc;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::json::{self, JsonNumber, JsonValue};
use gauntlet_ports::producer_authority::AdmittedSettlement;
use gauntlet_ports::state_store::{
    Checkpoint, ObservationBinding, RunEvent, RunEventKind, RunLifecycleFacts, SettlementCommit,
    StateStore, StateStoreError,
};
use gauntlet_ports::verification_record::VerificationRecordDoc;
use rusqlite::Connection;

use crate::encode;
use crate::event_log::{append_to_log, open_connection, read_entries};

/// Maps an [`gauntlet_ports::event_log::EventLogError`] onto the store's
/// vocabulary WITHOUT losing the failure class: `Damaged` (a log row that
/// does not decode into the domain event types) is stored state that does
/// not decode — exactly what [`StateStoreError::Corrupt`] names — with the
/// damage detail preserved; anything else stays a read failure.
fn map_log_error(err: gauntlet_ports::event_log::EventLogError) -> StateStoreError {
    match err {
        gauntlet_ports::event_log::EventLogError::Damaged { detail } => StateStoreError::Corrupt {
            detail: format!("the event log is damaged: {detail}"),
        },
        other => StateStoreError::ReadFailed {
            detail: other.to_string(),
        },
    }
}

/// Placeholder fields for the checkpoint-only projection of [`load_run`]:
/// names the limitation instead of inventing data silently.
const PROJECTION_REFERENCE_ID: &str = "checkpoint-only";
/// The projection's reference version: unknown, so the smallest legal value.
const PROJECTION_REFERENCE_VERSION: u32 = 1;
/// The projection's fingerprint placeholder: a valid digest of nothing.
const PROJECTION_FINGERPRINT_HEX: &str = "0000000000000000";
/// The projection's intent: says what it is, never a fabricated purpose.
const PROJECTION_INTENT: &str = "run identity not yet recorded; projected from a checkpoint";

/// One §18.1 durable record kind carried by the `records` table (everything
/// except `runs`, `settlements`, and the log itself). Typed (AGENTS.md:
/// every fixed-value-set field is a type); the SQL CHECK pins the same list.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecordKind {
    Iteration,
    RoleInvocation,
    ReferenceVersion,
    Observation,
    Comparison,
    VerificationRecord,
    Residual,
    Payment,
    Failure,
    RecoveryEvent,
    CandidateRevision,
    PromotionDecision,
    Escalation,
    /// The controller's §15.1 selection record (plan Task 12, AK-037).
    /// Beyond the §18.1 minimum list by exactly this one kind; the 0008
    /// migration widened the records CHECK for it (a recorded act).
    CorrectionSelection,
}

impl RecordKind {
    /// The wire token stored in `records.record_type`.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Iteration => "iteration",
            Self::RoleInvocation => "role_invocation",
            Self::ReferenceVersion => "reference_version",
            Self::Observation => "observation",
            Self::Comparison => "comparison",
            Self::VerificationRecord => "verification_record",
            Self::Residual => "residual",
            Self::Payment => "payment",
            Self::Failure => "failure",
            Self::RecoveryEvent => "recovery_event",
            Self::CandidateRevision => "candidate_revision",
            Self::PromotionDecision => "promotion_decision",
            Self::Escalation => "escalation",
            Self::CorrectionSelection => "correction_selection",
        }
    }
}

impl core::fmt::Display for RecordKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Canonical durable representation of the exact admitted domain aggregate.
/// Keeping this here prevents application code from depending on SQLite's raw
/// records table while preserving all §5.3.5 fields in one immutable row.
fn encode_role_invocation(invocation: &RoleInvocation) -> String {
    let optional = |value: Option<String>| value.map_or(JsonValue::Null, JsonValue::Str);
    JsonValue::Object(vec![
        ("id".to_owned(), JsonValue::Str(invocation.id().to_string())),
        (
            "run_id".to_owned(),
            JsonValue::Str(invocation.run_id().to_string()),
        ),
        (
            "iteration_no".to_owned(),
            uint(u64::from(invocation.iteration_no().get())),
        ),
        (
            "role".to_owned(),
            JsonValue::Str(invocation.role().to_string()),
        ),
        (
            "runner".to_owned(),
            JsonValue::Str(invocation.runner().to_string()),
        ),
        (
            "model".to_owned(),
            optional(invocation.model().map(ToString::to_string)),
        ),
        (
            "permissions".to_owned(),
            JsonValue::Str(invocation.permissions().to_string()),
        ),
        (
            "context_envelope_digest".to_owned(),
            JsonValue::Str(invocation.context_envelope_digest().to_string()),
        ),
        (
            "workspace_ref".to_owned(),
            JsonValue::Str(invocation.workspace_ref().to_string()),
        ),
        (
            "tier".to_owned(),
            JsonValue::Str(invocation.tier().to_string()),
        ),
        (
            "sandbox_policy_ref".to_owned(),
            optional(invocation.sandbox_policy_ref().map(ToString::to_string)),
        ),
        (
            "result_contract_path".to_owned(),
            JsonValue::Str(invocation.result_contract_path().to_string()),
        ),
        (
            "state".to_owned(),
            JsonValue::Str(invocation.state().to_string()),
        ),
        (
            "admitted_at".to_owned(),
            invocation
                .admitted_at()
                .map_or(JsonValue::Null, |at| uint(at.get())),
        ),
        (
            "session_ref".to_owned(),
            optional(invocation.session_ref().map(ToString::to_string)),
        ),
    ])
    .to_json_text()
}

fn encode_comparison(document: &ComparisonRecordDoc, evidence: &EvidenceRef) -> String {
    // The document came from a strict parser; reparse before nesting so a
    // broken implementation cannot smuggle non-JSON through a typed method.
    let document =
        json::parse(&document.to_json()).expect("ComparisonRecordDoc serializes valid JSON");
    JsonValue::Object(vec![
        ("document".to_owned(), document),
        (
            "evidence_ref".to_owned(),
            JsonValue::Str(evidence.to_string()),
        ),
    ])
    .to_json_text()
}

// ---------------------------------------------------------------------------
// Correction selections (spec §15.1 step 4, AK-037; plan Task 12)
// ---------------------------------------------------------------------------

/// The budget-facts object shared by the descriptor and the meter-refusal
/// reason.
fn encode_budget_facts(facts: &gauntlet_domain::BudgetFacts) -> JsonValue {
    JsonValue::Object(vec![
        (
            "requested_rounds".to_owned(),
            uint(u64::from(facts.requested_rounds)),
        ),
        (
            "requested_wall_seconds".to_owned(),
            uint(facts.requested_wall_seconds),
        ),
        ("requested_tokens".to_owned(), uint(facts.requested_tokens)),
        (
            "requested_cost_usd".to_owned(),
            uint(facts.requested_cost_usd.get()),
        ),
        (
            "remaining_rounds".to_owned(),
            uint(u64::from(facts.remaining_rounds)),
        ),
        (
            "remaining_wall_seconds".to_owned(),
            uint(facts.remaining_wall_seconds),
        ),
        ("remaining_tokens".to_owned(), uint(facts.remaining_tokens)),
        (
            "remaining_cost_usd".to_owned(),
            uint(facts.remaining_cost_usd.get()),
        ),
    ])
}

fn encode_descriptor(descriptor: &CorrectionDescriptor) -> JsonValue {
    JsonValue::Object(vec![
        (
            "summary".to_owned(),
            JsonValue::Str(descriptor.summary().to_owned()),
        ),
        (
            "shape".to_owned(),
            JsonValue::Str(descriptor.shape().to_string()),
        ),
        (
            "unit_refs".to_owned(),
            JsonValue::Array(
                descriptor
                    .unit_refs()
                    .iter()
                    .map(|unit| JsonValue::Str(unit.as_str().to_owned()))
                    .collect(),
            ),
        ),
        (
            "estimated".to_owned(),
            encode_budget_facts(&descriptor.estimated()),
        ),
    ])
}

fn encode_rejection_reason(reason: &RejectionReason) -> JsonValue {
    match reason {
        RejectionReason::AuthorityAboveCeiling {
            required,
            exercisable,
        } => JsonValue::Object(vec![
            (
                "kind".to_owned(),
                JsonValue::Str("authority_above_ceiling".to_owned()),
            ),
            ("required".to_owned(), JsonValue::Str(required.to_string())),
            (
                "exercisable".to_owned(),
                JsonValue::Str(exercisable.to_string()),
            ),
        ]),
        RejectionReason::TierNotDispatchable { tier } => JsonValue::Object(vec![
            (
                "kind".to_owned(),
                JsonValue::Str("tier_not_dispatchable".to_owned()),
            ),
            ("tier".to_owned(), JsonValue::Str(tier.to_string())),
        ]),
        RejectionReason::MeterRefusedReserve { facts } => JsonValue::Object(vec![
            (
                "kind".to_owned(),
                JsonValue::Str("meter_refused_reserve".to_owned()),
            ),
            ("facts".to_owned(), encode_budget_facts(facts)),
        ]),
        RejectionReason::IrreversibleTier { tier } => JsonValue::Object(vec![
            (
                "kind".to_owned(),
                JsonValue::Str("irreversible_tier".to_owned()),
            ),
            ("tier".to_owned(), JsonValue::Str(tier.to_string())),
        ]),
        RejectionReason::LowerPriority { decided_by } => JsonValue::Object(vec![
            (
                "kind".to_owned(),
                JsonValue::Str("lower_priority".to_owned()),
            ),
            (
                "decided_by".to_owned(),
                JsonValue::Str(decided_by.name().to_owned()),
            ),
        ]),
        RejectionReason::SpeculativeWhenBoundedAvailable => JsonValue::Object(vec![(
            "kind".to_owned(),
            JsonValue::Str("speculative_when_bounded_available".to_owned()),
        )]),
    }
}

/// Canonical durable representation of one controller correction selection.
/// The payload decodes back through the domain constructors only — a row
/// this adapter did not write, or a damaged one, is `Corrupt`, never a
/// partially-populated selection.
fn encode_correction_selection(selection: &CorrectionSelection) -> String {
    let rejected = selection
        .rejected()
        .iter()
        .map(|alternative| {
            JsonValue::Object(vec![
                (
                    "correction".to_owned(),
                    encode_descriptor(alternative.correction()),
                ),
                (
                    "class".to_owned(),
                    JsonValue::Str(alternative.class().to_string()),
                ),
                (
                    "reason".to_owned(),
                    encode_rejection_reason(alternative.reason()),
                ),
            ])
        })
        .collect();
    let trace = selection
        .trace()
        .iter()
        .map(|step| {
            JsonValue::Object(vec![
                (
                    "dimension".to_owned(),
                    JsonValue::Str(step.dimension().name().to_owned()),
                ),
                ("note".to_owned(), JsonValue::Str(step.note().to_owned())),
            ])
        })
        .collect();
    JsonValue::Object(vec![
        (
            "run_id".to_owned(),
            JsonValue::Str(selection.run_id().to_string()),
        ),
        (
            "iteration_no".to_owned(),
            uint(u64::from(selection.iteration_no().get())),
        ),
        ("chosen".to_owned(), encode_descriptor(selection.chosen())),
        ("rejected".to_owned(), JsonValue::Array(rejected)),
        ("trace".to_owned(), JsonValue::Array(trace)),
    ])
    .to_json_text()
}

/// Decode-side helpers: every field routes through a domain constructor or
/// typed parse; anything else is corrupt.
fn corrupt_selection(detail: String) -> StateStoreError {
    StateStoreError::Corrupt {
        detail: format!("a stored correction selection does not decode: {detail}"),
    }
}

fn decode_uint(value: Option<&JsonValue>, field: &str) -> Result<u64, StateStoreError> {
    match value {
        Some(JsonValue::Number(JsonNumber::Uint(n))) => Ok(*n),
        _ => Err(corrupt_selection(format!("{field} is not a whole number"))),
    }
}

fn decode_str<'a>(value: Option<&'a JsonValue>, field: &str) -> Result<&'a str, StateStoreError> {
    match value {
        Some(JsonValue::Str(text)) => Ok(text),
        _ => Err(corrupt_selection(format!("{field} is not a string"))),
    }
}

fn decode_budget_facts(
    value: Option<&JsonValue>,
) -> Result<gauntlet_domain::BudgetFacts, StateStoreError> {
    let Some(JsonValue::Object(_)) = value else {
        return Err(corrupt_selection(
            "estimated budget facts are not an object".to_owned(),
        ));
    };
    let narrow_u32 = |field: &str| -> Result<u32, StateStoreError> {
        u32::try_from(decode_uint(value.unwrap().get(field), field)?)
            .map_err(|_| corrupt_selection(format!("{field} exceeds the u32 space")))
    };
    Ok(gauntlet_domain::BudgetFacts {
        requested_rounds: narrow_u32("requested_rounds")?,
        requested_wall_seconds: decode_uint(
            value.unwrap().get("requested_wall_seconds"),
            "requested_wall_seconds",
        )?,
        requested_tokens: decode_uint(value.unwrap().get("requested_tokens"), "requested_tokens")?,
        requested_cost_usd: MicroUsd::new(decode_uint(
            value.unwrap().get("requested_cost_usd"),
            "requested_cost_usd",
        )?),
        remaining_rounds: narrow_u32("remaining_rounds")?,
        remaining_wall_seconds: decode_uint(
            value.unwrap().get("remaining_wall_seconds"),
            "remaining_wall_seconds",
        )?,
        remaining_tokens: decode_uint(value.unwrap().get("remaining_tokens"), "remaining_tokens")?,
        remaining_cost_usd: MicroUsd::new(decode_uint(
            value.unwrap().get("remaining_cost_usd"),
            "remaining_cost_usd",
        )?),
    })
}

fn decode_descriptor(value: Option<&JsonValue>) -> Result<CorrectionDescriptor, StateStoreError> {
    let Some(JsonValue::Object(_)) = value else {
        return Err(corrupt_selection(
            "a correction descriptor is not an object".to_owned(),
        ));
    };
    let object = value.unwrap();
    let summary = decode_str(object.get("summary"), "descriptor.summary")?.to_owned();
    let shape = match decode_str(object.get("shape"), "descriptor.shape")? {
        "bounded" => CorrectionShape::Bounded,
        "speculative" => CorrectionShape::Speculative,
        other => {
            return Err(corrupt_selection(format!(
                "unknown correction shape token {other:?}"
            )));
        }
    };
    let unit_refs = match object.get("unit_refs") {
        Some(JsonValue::Array(items)) => items
            .iter()
            .map(|item| match item {
                JsonValue::Str(raw) => {
                    UnitId::new(raw).map_err(|err| corrupt_selection(err.to_string()))
                }
                _ => Err(corrupt_selection(
                    "descriptor.unit_refs holds a non-string".to_owned(),
                )),
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => {
            return Err(corrupt_selection(
                "descriptor.unit_refs is not an array".to_owned(),
            ));
        }
    };
    CorrectionDescriptor::new(
        summary,
        shape,
        unit_refs,
        decode_budget_facts(object.get("estimated"))?,
    )
    .map_err(|err| corrupt_selection(err.to_string()))
}

fn decode_rejection_reason(value: Option<&JsonValue>) -> Result<RejectionReason, StateStoreError> {
    let Some(JsonValue::Object(_)) = value else {
        return Err(corrupt_selection(
            "a rejection reason is not an object".to_owned(),
        ));
    };
    let object = value.unwrap();
    let tier = |field: &str| -> Result<BlastRadius, StateStoreError> {
        BlastRadius::parse(decode_str(object.get(field), field)?)
            .map_err(|err| corrupt_selection(err.to_string()))
    };
    match decode_str(object.get("kind"), "reason.kind")? {
        "authority_above_ceiling" => Ok(RejectionReason::AuthorityAboveCeiling {
            required: Authority::parse(decode_str(object.get("required"), "reason.required")?)
                .map_err(|err| corrupt_selection(err.to_string()))?,
            exercisable: Authority::parse(decode_str(
                object.get("exercisable"),
                "reason.exercisable",
            )?)
            .map_err(|err| corrupt_selection(err.to_string()))?,
        }),
        "tier_not_dispatchable" => Ok(RejectionReason::TierNotDispatchable {
            tier: tier("tier")?,
        }),
        "meter_refused_reserve" => Ok(RejectionReason::MeterRefusedReserve {
            facts: decode_budget_facts(object.get("facts"))?,
        }),
        "irreversible_tier" => Ok(RejectionReason::IrreversibleTier {
            tier: tier("tier")?,
        }),
        "lower_priority" => {
            let decided_by = match decode_str(object.get("decided_by"), "reason.decided_by")? {
                "impact" => SelectionDimension::Impact,
                "dependency order" => SelectionDimension::DependencyOrder,
                "confidence" => SelectionDimension::Confidence,
                "payment" => SelectionDimension::Payment,
                "reversibility" => SelectionDimension::Reversibility,
                "risk" => SelectionDimension::Risk,
                "governance status" => SelectionDimension::GovernanceStatus,
                "settlement accessibility" => SelectionDimension::SettlementAccessibility,
                "recovery cost" => SelectionDimension::RecoveryCost,
                "expected information gain" => SelectionDimension::ExpectedInformationGain,
                other => {
                    return Err(corrupt_selection(format!(
                        "unknown 15.1 dimension {other:?}"
                    )));
                }
            };
            Ok(RejectionReason::LowerPriority { decided_by })
        }
        "speculative_when_bounded_available" => {
            Ok(RejectionReason::SpeculativeWhenBoundedAvailable)
        }
        other => Err(corrupt_selection(format!(
            "unknown rejection reason kind {other:?}"
        ))),
    }
}

fn decode_correction_selection(payload: &str) -> Result<CorrectionSelection, StateStoreError> {
    let parsed = json::parse(payload)
        .map_err(|err| corrupt_selection(format!("the payload is not valid JSON: {err}")))?;
    if !matches!(parsed, JsonValue::Object(_)) {
        return Err(corrupt_selection("the payload is not an object".to_owned()));
    }
    let get = |field: &str| parsed.get(field);
    let run_id = RunId::new(decode_str(get("run_id"), "run_id")?)
        .map_err(|err| corrupt_selection(err.to_string()))?;
    let iteration = decode_uint(get("iteration_no"), "iteration_no")?;
    let iteration_no =
        IterationNo::new(u32::try_from(iteration).map_err(|_| {
            corrupt_selection(format!("iteration {iteration} exceeds the u32 space"))
        })?)
        .map_err(|err| corrupt_selection(err.to_string()))?;
    let chosen = decode_descriptor(get("chosen"))?;
    let rejected = match get("rejected") {
        Some(JsonValue::Array(items)) => items
            .iter()
            .map(|item| {
                let JsonValue::Object(_) = item else {
                    return Err(corrupt_selection(
                        "a rejected alternative is not an object".to_owned(),
                    ));
                };
                let class = match decode_str(item.get("class"), "alternative.class")? {
                    "authority" => RejectionClass::Authority,
                    "tier" => RejectionClass::Tier,
                    "budget" => RejectionClass::Budget,
                    "irreversibility" => RejectionClass::Irreversibility,
                    "not selected" => RejectionClass::NotSelected,
                    other => {
                        return Err(corrupt_selection(format!(
                            "unknown rejection class {other:?}"
                        )));
                    }
                };
                Ok(RejectedAlternative::new(
                    decode_descriptor(item.get("correction"))?,
                    class,
                    decode_rejection_reason(item.get("reason"))?,
                ))
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => {
            return Err(corrupt_selection(
                "rejected alternatives are not an array".to_owned(),
            ));
        }
    };
    let trace = match get("trace") {
        Some(JsonValue::Array(items)) => items
            .iter()
            .map(|item| {
                let JsonValue::Object(_) = item else {
                    return Err(corrupt_selection(
                        "a trace step is not an object".to_owned(),
                    ));
                };
                let dimension = match decode_str(item.get("dimension"), "trace.dimension")? {
                    "impact" => SelectionDimension::Impact,
                    "dependency order" => SelectionDimension::DependencyOrder,
                    "confidence" => SelectionDimension::Confidence,
                    "payment" => SelectionDimension::Payment,
                    "reversibility" => SelectionDimension::Reversibility,
                    "risk" => SelectionDimension::Risk,
                    "governance status" => SelectionDimension::GovernanceStatus,
                    "settlement accessibility" => SelectionDimension::SettlementAccessibility,
                    "recovery cost" => SelectionDimension::RecoveryCost,
                    "expected information gain" => SelectionDimension::ExpectedInformationGain,
                    other => {
                        return Err(corrupt_selection(format!(
                            "unknown 15.1 dimension {other:?}"
                        )));
                    }
                };
                Ok(DecisionStep::new(
                    dimension,
                    decode_str(item.get("note"), "trace.note")?.to_owned(),
                ))
            })
            .collect::<Result<Vec<_>, _>>()?,
        _ => return Err(corrupt_selection("the trace is not an array".to_owned())),
    };
    CorrectionSelection::new(run_id, iteration_no, chosen, rejected, trace)
        .map_err(|err| corrupt_selection(err.to_string()))
}

/// A durable run-state store in one embedded database file.
pub struct SqliteStateStore {
    conn: Mutex<Connection>,
}

impl SqliteStateStore {
    /// Opens (creating if needed) the store at `path`, applying any
    /// migrations not yet recorded.
    pub fn open(path: &Path) -> Result<Self, StateStoreError> {
        let conn =
            open_connection(path).map_err(|detail| StateStoreError::WriteFailed { detail })?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// An in-memory store, for tests that never need a second process.
    pub fn open_in_memory() -> Result<Self, StateStoreError> {
        let conn = open_connection(Path::new(":memory:"))
            .map_err(|detail| StateStoreError::WriteFailed { detail })?;
        Ok(Self {
            conn: Mutex::new(conn),
        })
    }

    /// Records the authoritative run identity. Idempotent by `run_id`: a
    /// second recording of the SAME identity is a no-op, and a recording
    /// that disagrees with the stored identity on ANY column — the frozen
    /// reference (spec §5.3.3) or any other identity field — is a typed
    /// error naming every differing field and both values.
    pub fn record_run(&self, run: &Run, _deadline: Deadline) -> Result<(), StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let budget_text = encode::encode_budget_ceiling(run.budget());
        let parent = run.parent_run().map(|id| id.to_string());
        let insert = conn.execute(
            "INSERT INTO runs (
                run_id, created_at, reference_id, reference_version, reference_digest,
                reference_evidence_ref, intent, config_fingerprint, environment_fingerprint,
                parent_run, budget
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11)",
            (
                run.id().to_string(),
                run.created_at().get(),
                run.reference_version().reference_id().as_str(),
                u64::from(run.reference_version().version()),
                run.reference_version().digest().to_string(),
                run.reference_version().evidence_ref().as_str(),
                run.intent(),
                run.config_fingerprint().to_string(),
                run.environment_fingerprint().to_string(),
                parent,
                budget_text,
            ),
        );
        match insert {
            Ok(_) => Ok(()),
            Err(rusqlite::Error::SqliteFailure(ffi, _))
                if ffi.code == rusqlite::ErrorCode::ConstraintViolation =>
            {
                // The run id already exists. Idempotency requires the FULL
                // stored identity to agree — not just the digest: a same
                // digest with a different intent/budget/fingerprint is a
                // disagreement that must be surfaced, never silently
                // absorbed as a no-op.
                let stored = conn
                    .query_row(
                        "SELECT created_at, reference_id, reference_version, reference_digest, \
                         reference_evidence_ref, intent, config_fingerprint, \
                         environment_fingerprint, parent_run, budget FROM runs WHERE run_id = ?1",
                        [run.id().to_string()],
                        |row| {
                            Ok((
                                row.get::<_, i64>(0)?,
                                row.get::<_, String>(1)?,
                                row.get::<_, i64>(2)?,
                                row.get::<_, String>(3)?,
                                row.get::<_, Option<String>>(4)?,
                                row.get::<_, String>(5)?,
                                row.get::<_, String>(6)?,
                                row.get::<_, String>(7)?,
                                row.get::<_, Option<String>>(8)?,
                                row.get::<_, String>(9)?,
                            ))
                        },
                    )
                    .map_err(|err| StateStoreError::ReadFailed {
                        detail: format!("reading the stored run identity failed: {err}"),
                    })?;
                let differences = identity_differences(&stored, run)?;
                if differences.is_empty() {
                    Ok(())
                } else {
                    Err(StateStoreError::WriteFailed {
                        detail: format!(
                            "run {} is already recorded and the offered identity disagrees on: \
                             {}; a run never changes its frozen reference or its recorded \
                             identity (spec 5.3.3)",
                            run.id(),
                            differences.join("; ")
                        ),
                    })
                }
            }
            Err(err) => Err(StateStoreError::WriteFailed {
                detail: format!("recording run {} failed: {err}", run.id()),
            }),
        }
    }

    /// §16.3 step 2 on its own: writes the settlement row for a settlement
    /// event that is ALREADY durable at `offset`. Idempotent by
    /// `(run_id, unit_id, verification_ref)` — applying the same event twice
    /// adds one row and returns 0 the second time. This is the half the
    /// crash-recovery path replays; the offset (and, since plan Task 13,
    /// the event's stored timestamp) is cross-validated against the durable
    /// log before the row lands. The atomic commit sequence itself lives on
    /// the port (`StateStore::commit_settlement`) and consumes an
    /// authority-minted [`AdmittedSettlement`], never a caller-attested
    /// payload.
    ///
    /// Test-gated since the Task-13 correction round (early probe C1's
    /// aggravator): B1 recorded this inherent method as "test-facing", and
    /// the correction makes that structural — a caller-built
    /// [`SettlementCommit`] has no production route to a settlement row
    /// anymore. Production recovery reaches the row through the port's
    /// §18.3 replay (`complete_pending_settlements`), which rebuilds the
    /// commit from the durable event itself.
    #[cfg(test)]
    pub fn apply_settlement_row(
        &self,
        commit: SettlementCommit,
        offset: EventLogOffset,
        _deadline: Deadline,
    ) -> Result<usize, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        insert_settlement(&conn, &commit, offset)
    }

    /// §16.3 step 1 ALONE over an EARNED admission (Task 13 COR3,
    /// confirmation round 1 F-1): appends the durable settlement event
    /// WITHOUT applying the settlement row. Both generic append surfaces
    /// (the trait's `append_event` and the `EventLog` port over this same
    /// file) refuse the settlement kind since COR3, so this and
    /// `commit_settlement` are the only two routes a settlement event can
    /// enter the log — and both consume the same authority-minted
    /// [`AdmittedSettlement`], so every §16.1 condition was validated
    /// against durable state when the event was minted.
    ///
    /// This half-step exists for the crash-recovery instrument: the §16.3
    /// crash window (event durable, row missing) is a real process abort
    /// BETWEEN the two steps, which no single packaged call can inject —
    /// `crash_helper`'s `settle` script aborts here, then the §18.3 replay
    /// proves recovery. Production code completes both steps atomically
    /// through [`SqliteStateStore::commit_settlement`] (via the trait's
    /// [`StateStore::commit_settlement`]).
    pub fn append_settlement_event(
        &self,
        admitted: &AdmittedSettlement,
        _deadline: Deadline,
    ) -> Result<EventLogOffset, StateStoreError> {
        let commit = admitted.commit();
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let event = RunEvent {
            run_id: commit.run_id.clone(),
            kind: RunEventKind::SettlementCommitted {
                unit_id: commit.unit_id.clone(),
                evidence: commit.evidence.clone(),
                verification: commit.verification.clone(),
                reference_digest: commit.reference_digest.clone(),
            },
            at: commit.at,
        };
        append_to_log(&conn, &event).map_err(|err| StateStoreError::WriteFailed {
            detail: err.to_string(),
        })
    }

    /// [`SqliteStateStore::apply_settlement_row`] over a held connection.
    fn apply_settlement_row_with(
        &self,
        conn: &Connection,
        commit: &SettlementCommit,
        offset: EventLogOffset,
    ) -> Result<usize, StateStoreError> {
        insert_settlement(conn, commit, offset)
    }

    /// Appends one §18.1 durable record. The payload MUST be valid JSON —
    /// model output is untrusted input; it is parsed here, before admission,
    /// and rejected as a typed error otherwise.
    ///
    /// Appends one §18.1 durable record — the GENERIC kinds only.
    ///
    /// COR2-4 (critic round 1, F-5): `RecordKind::VerificationRecord` is
    /// REFUSED here for every caller — the durable PASS substrate is
    /// earned through the ports' `VerificationRecordAuthority` (COR-1),
    /// and a public inherent write of that kind let any concrete-type
    /// holder forge the record the settlement admission validates
    /// against. The trait's own `record_verification` uses the private
    /// raw helper.
    pub fn append_record(
        &self,
        run_id: &RunId,
        kind: RecordKind,
        payload: &str,
        at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<u64, StateStoreError> {
        if kind == RecordKind::VerificationRecord {
            return Err(StateStoreError::WriteFailed {
                detail: "verification records are earned through the ports' \
                     VerificationRecordAuthority (COR-1/COR2-4) — the generic record \
                     append refuses the settlement substrate's kind for every caller"
                    .to_owned(),
            });
        }
        self.append_record_raw(run_id, kind, payload, at, deadline)
    }

    /// The un-gated write the trait's own earned surfaces use.
    fn append_record_raw(
        &self,
        run_id: &RunId,
        kind: RecordKind,
        payload: &str,
        at: EpochSeconds,
        _deadline: Deadline,
    ) -> Result<u64, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        json::parse(payload).map_err(|err| StateStoreError::WriteFailed {
            detail: format!("the {kind} payload is not valid JSON: {err}"),
        })?;
        conn.execute(
            "INSERT INTO records (run_id, record_type, payload, at) VALUES (?1, ?2, ?3, ?4)",
            (run_id.to_string(), kind.as_str(), payload, at.get()),
        )
        .map_err(|err| StateStoreError::WriteFailed {
            detail: format!("appending a {kind} record for run {run_id} failed: {err}"),
        })?;
        Ok(u64::try_from(conn.last_insert_rowid()).unwrap_or(0))
    }

    /// Loads the authoritative runs row, when there is one.
    fn load_recorded_run(
        conn: &Connection,
        run_id: &RunId,
    ) -> Result<Option<Run>, StateStoreError> {
        let row = conn.query_row(
            "SELECT created_at, reference_id, reference_version, reference_digest, \
             reference_evidence_ref, intent, config_fingerprint, environment_fingerprint, \
             parent_run, budget FROM runs WHERE run_id = ?1",
            [run_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, Option<String>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, String>(7)?,
                    row.get::<_, Option<String>>(8)?,
                    row.get::<_, String>(9)?,
                ))
            },
        );
        let (
            created_at,
            reference_id,
            reference_version,
            reference_digest,
            reference_evidence_ref,
            intent,
            config_fingerprint,
            environment_fingerprint,
            parent_run,
            budget_text,
        ) = match row {
            Ok(stored) => stored,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(err) => {
                return Err(StateStoreError::ReadFailed {
                    detail: format!("reading run {run_id} failed: {err}"),
                });
            }
        };
        if created_at < 0 {
            return Err(StateStoreError::Corrupt {
                detail: format!(
                    "run {run_id} holds a created_at of {created_at} — negative; this adapter \
                     never writes one, so the row was damaged or tampered out of band"
                ),
            });
        }
        let ceiling = encode::decode_budget_ceiling(&budget_text).map_err(|err| {
            StateStoreError::Corrupt {
                detail: format!("run {run_id} holds an undecodable budget: {err}"),
            }
        })?;
        let budget = Budget::new(
            ceiling.limits,
            ceiling.recovery_reserve_pct,
            ceiling.diagnostic,
        )
        .map_err(|err| StateStoreError::Corrupt {
            detail: format!("run {run_id} holds an unusable budget: {err}"),
        })?;
        let run = Run::new(RunDefinition {
            id: run_id.clone(),
            created_at: EpochSeconds::new(created_at.max(0) as u64),
            reference_version: ReferenceVersion::with_evidence(
                ReferenceId::new(&reference_id).map_err(|err| StateStoreError::Corrupt {
                    detail: format!("run {run_id} holds an invalid reference id: {err}"),
                })?,
                u32::try_from(reference_version).unwrap_or(0),
                Digest::parse(&reference_digest).map_err(|err| StateStoreError::Corrupt {
                    detail: format!("run {run_id} holds an invalid digest: {err}"),
                })?,
                EvidenceRef::new(&reference_evidence_ref.ok_or_else(|| StateStoreError::Unavailable {
                    detail: format!(
                        "run {run_id} was written before frozen reference evidence was persisted; \
                         migrate/recreate it from its original frozen evidence, never synthesize a binding"
                    ),
                })?).map_err(|err| StateStoreError::Corrupt {
                    detail: format!("run {run_id} holds an invalid frozen reference evidence ref: {err}"),
                })?,
            ),
            intent,
            config_fingerprint: Digest::parse(&config_fingerprint).map_err(|err| {
                StateStoreError::Corrupt {
                    detail: format!("run {run_id} holds an invalid config fingerprint: {err}"),
                }
            })?,
            environment_fingerprint: Digest::parse(&environment_fingerprint).map_err(|err| {
                StateStoreError::Corrupt {
                    detail: format!("run {run_id} holds an invalid environment fingerprint: {err}"),
                }
            })?,
            budget,
            parent_run: parent_run
                .map(|id| {
                    RunId::new(&id).map_err(|err| StateStoreError::Corrupt {
                        detail: format!("run {run_id} holds an invalid parent run id: {err}"),
                    })
                })
                .transpose()?,
        })
        .map_err(|err| StateStoreError::Corrupt {
            detail: format!("run {run_id} does not reconstruct: {err}"),
        })?;
        Ok(Some(run))
    }

    /// The checkpoint-only projection (see the module docs): true run id and
    /// frozen digest, placeholders for everything the port's durable records
    /// never carried.
    fn project_run_from_checkpoint(
        conn: &Connection,
        run_id: &RunId,
        digest: &Digest,
    ) -> Result<Run, StateStoreError> {
        let created_at: Option<i64> = conn
            .query_row(
                "SELECT MIN(at) FROM event_log WHERE run_id = ?1",
                [run_id.to_string()],
                |row| row.get(0),
            )
            .map_err(|err| StateStoreError::ReadFailed {
                detail: format!("reading the first event of run {run_id} failed: {err}"),
            })?;
        if created_at.is_some_and(|value| value < 0) {
            return Err(StateStoreError::Corrupt {
                detail: format!(
                    "run {run_id} holds an event timestamp of {} — negative; this adapter \
                     never writes one, so the row was damaged or tampered out of band",
                    created_at.unwrap_or_default()
                ),
            });
        }
        Run::new(RunDefinition {
            id: run_id.clone(),
            created_at: EpochSeconds::new(created_at.unwrap_or(0).max(0) as u64),
            reference_version: ReferenceVersion::new(
                ReferenceId::new(PROJECTION_REFERENCE_ID).map_err(|err| {
                    StateStoreError::Corrupt {
                        detail: format!("the projection reference id is invalid: {err}"),
                    }
                })?,
                PROJECTION_REFERENCE_VERSION,
                digest.clone(),
            ),
            intent: PROJECTION_INTENT.to_owned(),
            config_fingerprint: projection_fingerprint()?,
            environment_fingerprint: projection_fingerprint()?,
            // Minimal legal ceilings: a projection cannot spend; the
            // authority is the runs row, recorded by the composition root.
            budget: projection_budget()?,
            parent_run: None,
        })
        .map_err(|err| StateStoreError::Corrupt {
            detail: format!("run {run_id} does not project: {err}"),
        })
    }
}

fn projection_fingerprint() -> Result<Digest, StateStoreError> {
    Digest::new("sha256", PROJECTION_FINGERPRINT_HEX).map_err(|err| StateStoreError::Corrupt {
        detail: format!("the projection fingerprint is invalid: {err}"),
    })
}

fn projection_budget() -> Result<Budget, StateStoreError> {
    let limits =
        Limits::new(1, 1, 1, 1, 1, MicroUsd::new(1)).map_err(|err| StateStoreError::Corrupt {
            detail: format!("the projection budget ceilings are unusable: {err}"),
        })?;
    Budget::new(limits, 1, DiagnosticAllowance::new(1, MicroUsd::new(1))).map_err(|err| {
        StateStoreError::Corrupt {
            detail: format!("the projection budget is unusable: {err}"),
        }
    })
}

/// A zero `Spent` — the only value any `Checkpoint` can carry today (see
/// the module docs). Since the domain's value-carrying constructor landed
/// (Task 12), this is the plain zero value rather than a budget-construction
/// dance; non-zero water-marks are still refused below until resume composes
/// `Budget::restore` against the run's durable ceilings (B3's controller
/// composition — the equivalence is recorded in the Task-12 report).
fn zero_spent() -> Spent {
    Spent::from_counters(0, 0, 0, MicroUsd::new(0), 0, MicroUsd::new(0))
}

/// §16.3 step 2: the idempotent keyed insert, returning how many rows it
/// added. Replaying an event twice inserts once — the key is the spec's own.
/// Before the insert, the claimed `offset` is cross-validated against the
/// durable log (see [`verify_settlement_event_at`]): a settlement row may
/// never name an event it did not come from, and `settled_at` is the
/// DURABLE event's stored timestamp (plan Task 13, A3) — never the commit's
/// caller-supplied construction `at`.
fn insert_settlement(
    conn: &Connection,
    commit: &SettlementCommit,
    offset: EventLogOffset,
) -> Result<usize, StateStoreError> {
    let settled_at = verify_settlement_event_at(conn, commit, offset)?;
    let inserted = conn
        .execute(
            "INSERT OR IGNORE INTO settlements (
            run_id, unit_id, verification_ref, evidence_ref, reference_digest,
            event_log_offset, settled_at
        ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                commit.run_id.to_string(),
                commit.unit_id.as_str(),
                commit.verification.as_str(),
                commit.evidence.as_str(),
                commit.reference_digest.to_string(),
                offset.get(),
                settled_at.get(),
            ),
        )
        .map_err(|err| StateStoreError::WriteFailed {
            detail: format!(
                "writing the settlement row for unit {} of run {} failed: {err}",
                commit.unit_id, commit.run_id
            ),
        })?;
    if inserted == 0 {
        // COR2-5 (critic round 1, F-6): the keyed insert was ignored —
        // either the idempotent re-run of the SAME facts (fine) or a
        // same-key event whose facts DIVERGE from the durable row. The
        // silent OR-IGNORE swallowed the divergence; it is now a typed
        // integrity refusal naming both sides, never a quiet no-op.
        let (stored_evidence, stored_digest, stored_offset): (String, String, i64) = conn
            .query_row(
                "SELECT evidence_ref, reference_digest, event_log_offset FROM settlements \
                 WHERE run_id = ?1 AND unit_id = ?2 AND verification_ref = ?3",
                (
                    commit.run_id.to_string(),
                    commit.unit_id.as_str(),
                    commit.verification.as_str(),
                ),
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .map_err(|err| StateStoreError::ReadFailed {
                detail: format!(
                    "reading back the existing settlement row for unit {} of run {} failed: {err}",
                    commit.unit_id, commit.run_id
                ),
            })?;
        // The offset is deliberately NOT compared: a second same-key
        // event at a new offset with identical facts is the idempotent
        // duplicate the §16.3 design permits; divergence is about the
        // settlement's FACTS (evidence, digest).
        let _ = stored_offset;
        if stored_evidence != commit.evidence.as_str()
            || stored_digest != commit.reference_digest.to_string()
        {
            return Err(StateStoreError::Corrupt {
                detail: format!(
                    "the settlement key (run {}, unit {}, verification {}) is already \
                     durable with DIVERGENT facts — stored: evidence {}, digest {}, offset \
                     {}; offered: evidence {}, digest {}, offset {}. A same-key settlement \
                     with different facts is an integrity violation, not an idempotent replay",
                    commit.run_id,
                    commit.unit_id,
                    commit.verification,
                    stored_evidence,
                    stored_digest,
                    stored_offset,
                    commit.evidence,
                    commit.reference_digest,
                    offset.get()
                ),
            });
        }
    }
    Ok(inserted)
}

/// Cross-validates a settlement row's claimed offset against the log before
/// the row lands: the event at `offset` must exist, belong to the commit's
/// run, be a `settlement_committed`, carry exactly the commit's unit,
/// verification, evidence, and reference digest (spec §5.3.6: a
/// `SettlementRecord` "references the event-log offset that made it
/// durable" — its OWN offset, not an offset the caller attests), and —
/// since plan Task 13 (A3) — the commit's constructed timestamp must AGREE
/// with the timestamp stored at that offset. Returns the durable event's
/// stored timestamp, which is what the row's `settled_at` takes. A missing
/// row or any mismatch is a typed refusal naming both what was claimed and
/// what is there; the replay path passes trivially because it hands over
/// the log's own offsets and the event's own timestamp.
fn verify_settlement_event_at(
    conn: &Connection,
    commit: &SettlementCommit,
    offset: EventLogOffset,
) -> Result<EpochSeconds, StateStoreError> {
    let stored = conn.query_row(
        "SELECT run_id, kind, payload, at FROM event_log WHERE offset = ?1",
        [offset.get()],
        |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        },
    );
    let (stored_run, stored_kind, stored_payload, stored_at) = match stored {
        Ok(columns) => columns,
        Err(rusqlite::Error::QueryReturnedNoRows) => {
            return Err(StateStoreError::WriteFailed {
                detail: format!(
                    "the settlement row for unit {} of run {} claims offset {}, but the event \
                     log has no row there — a settlement row must name the durable settlement \
                     event that made it real (spec 5.3.6)",
                    commit.unit_id,
                    commit.run_id,
                    offset.get()
                ),
            });
        }
        Err(err) => {
            return Err(StateStoreError::WriteFailed {
                detail: format!(
                    "reading the event at offset {} to validate the settlement row for unit {} \
                     of run {} failed: {err}",
                    offset.get(),
                    commit.unit_id,
                    commit.run_id
                ),
            });
        }
    };
    if stored_at < 0 {
        return Err(StateStoreError::WriteFailed {
            detail: format!(
                "the settlement row for unit {} of run {} claims offset {}, but the event there \
                 holds a negative timestamp ({stored_at}) — this adapter never writes one, so \
                 the row was damaged or tampered out of band",
                commit.unit_id,
                commit.run_id,
                offset.get()
            ),
        });
    }
    let stored_at = EpochSeconds::new(u64::try_from(stored_at).unwrap_or(0));
    let event = match encode::decode_event(
        RunId::new(&stored_run).map_err(|err| StateStoreError::WriteFailed {
            detail: format!(
                "the event at offset {} holds a run id that does not decode ({stored_run:?}): \
                 {err}",
                offset.get()
            ),
        })?,
        stored_at,
        &stored_kind,
        &stored_payload,
    ) {
        Ok(event) => event,
        Err(err) => {
            return Err(StateStoreError::WriteFailed {
                detail: format!(
                    "the settlement row for unit {} of run {} claims offset {}, but the event \
                     there does not decode: {err}",
                    commit.unit_id,
                    commit.run_id,
                    offset.get()
                ),
            });
        }
    };
    let RunEventKind::SettlementCommitted {
        unit_id,
        evidence,
        verification,
        reference_digest,
    } = event.kind
    else {
        return Err(StateStoreError::WriteFailed {
            detail: format!(
                "the settlement row for unit {} of run {} claims offset {}, but the event log \
                 row there is a different event (kind {stored_kind:?}, run {stored_run}) — a \
                 settlement row must name its own durable settlement event (spec 5.3.6)",
                commit.unit_id,
                commit.run_id,
                offset.get()
            ),
        });
    };
    if event.run_id != commit.run_id
        || unit_id != commit.unit_id
        || evidence != commit.evidence
        || verification != commit.verification
        || reference_digest != commit.reference_digest
    {
        return Err(StateStoreError::WriteFailed {
            detail: format!(
                "the settlement row for unit {} of run {} claims offset {}, but the event there \
                 is the settlement of unit {unit_id} of run {} (verification {}, evidence {}) — \
                 the claimed fields and the durable event disagree (spec 5.3.6)",
                commit.unit_id,
                commit.run_id,
                offset.get(),
                event.run_id,
                verification.as_str(),
                evidence.as_str()
            ),
        });
    }
    // The A3 seam, checked LAST so the stronger refusals above stay
    // authoritative: with every field agreeing, a constructed timestamp
    // that disagrees with the stored one is the one remaining caller
    // attestation, and it is refused.
    if stored_at != commit.at {
        return Err(StateStoreError::WriteFailed {
            detail: format!(
                "the settlement row for unit {} of run {} claims offset {} with constructed \
                 timestamp {}, but the durable event there stores {} — the commit's `at` \
                 constructs the event only; settled_at is the event's stored timestamp and a \
                 disagreeing construction is refused (spec 5.3.6)",
                commit.unit_id,
                commit.run_id,
                offset.get(),
                commit.at.get(),
                stored_at.get()
            ),
        });
    }
    Ok(stored_at)
}

/// The settlement rows of one run, ordered by `(unit_id, verification_ref)`.
fn settlements_of(
    conn: &Connection,
    run_id: &RunId,
) -> Result<Vec<SettlementRecord>, StateStoreError> {
    let mut statement = conn
        .prepare(
            "SELECT unit_id, verification_ref, evidence_ref, event_log_offset \
             FROM settlements WHERE run_id = ?1 ORDER BY unit_id ASC, verification_ref ASC",
        )
        .map_err(|err| StateStoreError::ReadFailed {
            detail: format!("preparing the settlement query failed: {err}"),
        })?;
    let rows = statement
        .query_map([run_id.to_string()], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
                row.get::<_, i64>(3)?,
            ))
        })
        .map_err(|err| StateStoreError::ReadFailed {
            detail: format!("reading settlements failed: {err}"),
        })?;
    let mut out = Vec::new();
    for row in rows {
        let (unit_id, verification, evidence, offset) =
            row.map_err(|err| StateStoreError::Corrupt {
                detail: format!("a settlement row does not decode: {err}"),
            })?;
        out.push(SettlementRecord::new(
            run_id.clone(),
            UnitId::new(&unit_id).map_err(|err| StateStoreError::Corrupt {
                detail: format!("a stored unit id does not decode: {err}"),
            })?,
            EvidenceRef::new(&evidence).map_err(|err| StateStoreError::Corrupt {
                detail: format!("a stored evidence ref does not decode: {err}"),
            })?,
            VerificationRef::new(&verification).map_err(|err| StateStoreError::Corrupt {
                detail: format!("a stored verification ref does not decode: {err}"),
            })?,
            // A3's decode leg (plan Task 13, the Task-12 N3 checkpoint
            // discipline mirrored): the offset is the §5.3.6 durable-anchor
            // reference — a stored value no u64 can represent is CORRUPT,
            // never a silent 0 that would point the record at whatever
            // event happens to sit at the log's start.
            EventLogOffset::new(u64::try_from(offset).map_err(|_| StateStoreError::Corrupt {
                detail: format!(
                    "a settlement row of run {run_id} (unit {unit_id}) holds a \
                     non-representable event-log offset ({offset})"
                ),
            })?),
        ));
    }
    Ok(out)
}

fn uint(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber::Uint(value))
}

/// Rebuilds one durable [`gauntlet_ports::state_store::EvidenceProductionBinding`]
/// from its stored columns. Strict: any column the domain constructors
/// refuse is `Corrupt`, never a silently degraded record (spec §21.2).
/// The column-for-column signature is the row's own shape (clippy 1.98
/// began counting it over the threshold; the repo's `mint`-style allow).
#[allow(clippy::type_complexity, clippy::too_many_arguments)]
fn production_from_parts(
    run_id: &RunId,
    evidence: &EvidenceRef,
    producer_name: &str,
    producer_version: &str,
    locator: &str,
    iteration: i64,
    produced_at: i64,
    offset: i64,
    content: &str,
) -> Result<gauntlet_ports::state_store::EvidenceProductionBinding, StateStoreError> {
    let corrupt = |detail: String| StateStoreError::Corrupt {
        detail: format!(
            "a stored evidence-production record of run {run_id} does not decode: {detail}"
        ),
    };
    let name = InstrumentName::new(producer_name).map_err(|err| corrupt(err.to_string()))?;
    let identity =
        gauntlet_ports::observer::InstrumentIdentity::new(name, producer_version.to_owned());
    let locator = EvidenceLocator::new(locator).map_err(|err| corrupt(err.to_string()))?;
    let iteration = u32::try_from(iteration)
        .map_err(|_| corrupt(format!("iteration {iteration} exceeds the u32 space")))?;
    let iteration = IterationNo::new(iteration).map_err(|err| corrupt(err.to_string()))?;
    let produced_at = u64::try_from(produced_at)
        .map_err(|_| corrupt(format!("produced_at {produced_at} is negative")))?;
    let offset = u64::try_from(offset)
        .map_err(|_| corrupt(format!("event-log offset {offset} is negative")))?;
    // The content digest rides as its Display form (`algo:hex`). An empty
    // digest is a row written before migration 0007 — Corrupt, never a
    // production record with no content binding (spec §21.2).
    let (content_algo, content_hex) = content.split_once(':').ok_or_else(|| {
        corrupt(format!(
            "the content digest {content:?} is not in `algo:hex` form (a pre-0007 row has no \
             content binding)"
        ))
    })?;
    let content = gauntlet_domain::Digest::new(content_algo, content_hex)
        .map_err(|err| corrupt(err.to_string()))?;
    Ok(gauntlet_ports::state_store::EvidenceProductionBinding::new(
        run_id.clone(),
        gauntlet_ports::state_store::EvidenceProduction::new(
            evidence.clone(),
            content,
            identity,
            locator,
            iteration,
            EpochSeconds::new(produced_at),
        ),
        EventLogOffset::new(offset),
    ))
}

/// The immutable columns that define a stored run. The nullable evidence
/// field identifies legacy rows written before migration 0004; it is never
/// converted into a fabricated reference during rehydration.
type StoredRunIdentity = (
    i64,
    String,
    i64,
    String,
    Option<String>,
    String,
    String,
    String,
    Option<String>,
    String,
);

/// Every identity column on which a stored `runs` row and a re-recorded
/// [`Run`] disagree, as "field (stored X, offered Y)" entries — empty when
/// the recording is idempotent. The budget is compared field-by-field after
/// decoding (an undecodable stored budget is `Corrupt`, not a silent
/// mismatch).
fn identity_differences(
    stored: &StoredRunIdentity,
    run: &Run,
) -> Result<Vec<String>, StateStoreError> {
    let (
        created_at,
        reference_id,
        reference_version,
        reference_digest,
        reference_evidence_ref,
        intent,
        config_fingerprint,
        environment_fingerprint,
        parent_run,
        budget_text,
    ) = stored;
    let mut differences: Vec<String> = Vec::new();
    if u64::try_from(*created_at) != Ok(run.created_at().get()) {
        differences.push(format!(
            "created_at (stored {created_at}, offered {})",
            run.created_at().get()
        ));
    }
    if reference_id != run.reference_version().reference_id().as_str() {
        differences.push(format!(
            "reference_id (stored {reference_id:?}, offered {:?})",
            run.reference_version().reference_id().as_str()
        ));
    }
    if *reference_version != i64::from(run.reference_version().version()) {
        differences.push(format!(
            "reference_version (stored {reference_version}, offered {})",
            run.reference_version().version()
        ));
    }
    if reference_digest != &run.reference_version().digest().to_string() {
        differences.push(format!(
            "reference_digest (stored {reference_digest:?}, offered {:?})",
            run.reference_version().digest().to_string()
        ));
    }
    let offered_evidence = run.reference_version().evidence_ref().as_str();
    if reference_evidence_ref.as_deref() != Some(offered_evidence) {
        differences.push(format!(
            "frozen reference evidence (stored {reference_evidence_ref:?}, offered {offered_evidence:?})"
        ));
    }
    if intent != run.intent() {
        differences.push(format!(
            "intent (stored {intent:?}, offered {:?})",
            run.intent()
        ));
    }
    if config_fingerprint != &run.config_fingerprint().to_string() {
        differences.push(format!(
            "config_fingerprint (stored {config_fingerprint:?}, offered {:?})",
            run.config_fingerprint().to_string()
        ));
    }
    if environment_fingerprint != &run.environment_fingerprint().to_string() {
        differences.push(format!(
            "environment_fingerprint (stored {environment_fingerprint:?}, offered {:?})",
            run.environment_fingerprint().to_string()
        ));
    }
    let offered_parent = run.parent_run().map(|id| id.to_string());
    if parent_run != &offered_parent {
        differences.push(format!(
            "parent_run (stored {parent_run:?}, offered {offered_parent:?})"
        ));
    }
    let stored_budget =
        encode::decode_budget_ceiling(budget_text).map_err(|err| StateStoreError::Corrupt {
            detail: format!("run {} holds an undecodable budget: {err}", run.id()),
        })?;
    let offered = run.budget();
    let stored_limits = &stored_budget.limits;
    let offered_limits = offered.limits();
    let budget_fields = [
        (
            "recovery_reserve_pct",
            u64::from(stored_budget.recovery_reserve_pct),
            u64::from(offered.recovery_reserve_pct()),
        ),
        (
            "rounds",
            u64::from(stored_limits.max_rounds()),
            u64::from(offered_limits.max_rounds()),
        ),
        (
            "parallel",
            u64::from(stored_limits.max_parallel()),
            u64::from(offered_limits.max_parallel()),
        ),
        (
            "depth",
            u64::from(stored_limits.max_depth()),
            u64::from(offered_limits.max_depth()),
        ),
        (
            "wall_seconds",
            stored_limits.max_wall_seconds(),
            offered_limits.max_wall_seconds(),
        ),
        (
            "tokens",
            stored_limits.max_tokens(),
            offered_limits.max_tokens(),
        ),
        (
            "cost_usd",
            stored_limits.max_cost_usd().get(),
            offered_limits.max_cost_usd().get(),
        ),
        (
            "diagnostic.tokens",
            stored_budget.diagnostic.tokens(),
            offered.diagnostic_allowance().tokens(),
        ),
        (
            "diagnostic.cost_usd",
            stored_budget.diagnostic.cost_usd().get(),
            offered.diagnostic_allowance().cost_usd().get(),
        ),
    ];
    for (field, was, now) in budget_fields {
        if was != now {
            differences.push(format!("budget.{field} (stored {was}, offered {now})"));
        }
    }
    Ok(differences)
}

impl SqliteStateStore {
    /// Every durable row for one `(run, evidence ref)`, ordered by their
    /// event-log anchor (oldest — earliest iteration — first). Since 0011
    /// a ref may carry more than one row (one per iteration it was
    /// genuinely re-produced in); `.last()` is the freshest.
    fn production_rows_for_ref(
        &self,
        run_id: &RunId,
        evidence: &EvidenceRef,
        _deadline: Deadline,
    ) -> Result<Vec<gauntlet_ports::state_store::EvidenceProductionBinding>, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let mut statement = conn
            .prepare(
                "SELECT producer_name, producer_version, locator, iteration_no, produced_at, \
                 event_log_offset, content_digest FROM evidence_productions WHERE run_id = ?1 \
                 AND evidence_ref = ?2 ORDER BY event_log_offset ASC",
            )
            .map_err(|err| StateStoreError::ReadFailed {
                detail: format!(
                    "preparing the production-rows query for evidence {evidence} in run \
                     {run_id} failed: {err}"
                ),
            })?;
        let rows = statement
            .query_map((run_id.to_string(), evidence.as_str()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .map_err(|err| StateStoreError::ReadFailed {
                detail: format!(
                    "reading the production rows for evidence {evidence} in run {run_id} \
                     failed: {err}"
                ),
            })?;
        let mut bindings = Vec::new();
        for row in rows {
            let (name, version, locator, iteration, produced_at, offset, content) =
                row.map_err(|err| StateStoreError::ReadFailed {
                    detail: format!(
                        "stepping the production rows for evidence {evidence} in run {run_id} \
                         failed: {err}",
                    ),
                })?;
            bindings.push(production_from_parts(
                run_id,
                evidence,
                &name,
                &version,
                &locator,
                iteration,
                produced_at,
                offset,
                &content,
            )?);
        }
        Ok(bindings)
    }
}

impl StateStore for SqliteStateStore {
    /// The trait half of the adapter's identity recording: the inherent
    /// method (idempotent by run id, disagreeing re-record refused with
    /// every differing field named) is the implementation; the port method
    /// exists so the Task-12 controller composition can make a run durable
    /// through `Arc<dyn StateStore>` like every other durable write.
    fn record_run(&self, run: &Run, deadline: Deadline) -> Result<(), StateStoreError> {
        Self::record_run(self, run, deadline)
    }

    fn append_event(
        &self,
        event: RunEvent,
        _deadline: Deadline,
    ) -> Result<EventLogOffset, StateStoreError> {
        // COR3 (confirmation round 1, F-1): the settlement kind is
        // authority-minted — earned only through `commit_settlement`
        // consuming an `AdmittedSettlement` (and the recovery instrument's
        // earned half-step). The §18.3 replay mints a durable settlement
        // row from the event alone, so a caller-built event here forged
        // the exact record every settlement consumer reads. The COR2-4
        // `append_record` kind gate, mirrored at the event layer.
        if let RunEventKind::SettlementCommitted { unit_id, .. } = &event.kind {
            return Err(StateStoreError::SettlementEventNotEarned {
                run_id: event.run_id.clone(),
                unit_id: unit_id.clone(),
            });
        }
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        append_to_log(&conn, &event).map_err(|err| StateStoreError::WriteFailed {
            detail: err.to_string(),
        })
    }

    fn load_run(
        &self,
        run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Option<Run>, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        if let Some(run) = Self::load_recorded_run(&conn, run_id)? {
            return Ok(Some(run));
        }
        let latest: Option<(String, i64)> = match conn.query_row(
            "SELECT reference_digest, seq FROM checkpoints WHERE run_id = ?1 \
             ORDER BY iteration_no DESC, seq DESC LIMIT 1",
            [run_id.to_string()],
            |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
        ) {
            Ok(pair) => Some(pair),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(err) => {
                return Err(StateStoreError::ReadFailed {
                    detail: format!("reading the latest checkpoint of run {run_id} failed: {err}"),
                });
            }
        };
        match latest {
            Some((digest_text, _)) => {
                let digest =
                    Digest::parse(&digest_text).map_err(|err| StateStoreError::Corrupt {
                        detail: format!(
                            "run {run_id} holds a checkpoint digest that does not decode: {err}"
                        ),
                    })?;
                Ok(Some(Self::project_run_from_checkpoint(
                    &conn, run_id, &digest,
                )?))
            }
            None => Ok(None),
        }
    }

    /// The latest recorded run (greatest ULID first — ULID order is time
    /// order; `created_at` breaks nothing but is not the authority).
    fn latest_run(&self, _deadline: Deadline) -> Result<Option<RunId>, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let latest: Option<String> = match conn.query_row(
            "SELECT run_id FROM runs ORDER BY run_id DESC LIMIT 1",
            [],
            |row| row.get(0),
        ) {
            Ok(run_id) => Some(run_id),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(err) => {
                return Err(StateStoreError::ReadFailed {
                    detail: format!("reading the latest recorded run failed: {err}"),
                });
            }
        };
        latest
            .map(|run_id| {
                RunId::new(&run_id).map_err(|err| StateStoreError::Corrupt {
                    detail: format!("the runs table holds a run id that does not decode: {err}"),
                })
            })
            .transpose()
    }

    fn checkpoint(
        &self,
        checkpoint: Checkpoint,
        _deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        conn.execute(
            "INSERT INTO checkpoints (
                run_id, iteration_no, reference_digest, unit_states, budget_spent,
                in_flight, event_log_offset
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            (
                checkpoint.run_id.to_string(),
                u64::from(checkpoint.iteration_no.get()),
                checkpoint.reference_digest.to_string(),
                encode::encode_unit_states(&checkpoint.unit_states),
                encode::encode_spent(&checkpoint.budget_spent),
                encode::encode_in_flight(&checkpoint.in_flight),
                checkpoint.event_log_offset.get(),
            ),
        )
        .map_err(|err| StateStoreError::WriteFailed {
            detail: format!(
                "writing the iteration-{} checkpoint of run {} failed: {err}",
                checkpoint.iteration_no, checkpoint.run_id
            ),
        })?;
        Ok(())
    }

    fn resume(
        &self,
        run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Option<Checkpoint>, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let stored = match conn.query_row(
            "SELECT seq, iteration_no, reference_digest, unit_states, budget_spent, in_flight, \
             event_log_offset FROM checkpoints WHERE run_id = ?1 \
             ORDER BY iteration_no DESC, seq DESC LIMIT 1",
            [run_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        ) {
            Ok(row) => row,
            Err(rusqlite::Error::QueryReturnedNoRows) => return Ok(None),
            Err(err) => {
                return Err(StateStoreError::ReadFailed {
                    detail: format!("resuming run {run_id} failed: {err}"),
                });
            }
        };
        let (seq, iteration_no, digest_text, unit_states, budget_spent, in_flight, offset) = stored;
        let unit_states =
            encode::decode_unit_states(&unit_states).map_err(|err| StateStoreError::Corrupt {
                detail: format!("run {run_id} checkpoint {seq} holds bad unit states: {err}"),
            })?;
        let spent =
            encode::decode_spent(&budget_spent).map_err(|err| StateStoreError::Corrupt {
                detail: format!("run {run_id} checkpoint {seq} holds a bad budget: {err}"),
            })?;
        let in_flight =
            encode::decode_in_flight(&in_flight).map_err(|err| StateStoreError::Corrupt {
                detail: format!("run {run_id} checkpoint {seq} holds bad in-flight work: {err}"),
            })?;
        let budget_spent = {
            // The ceilings that admit a non-zero water-mark live on the
            // DURABLE RUN's own budget row (loaded through the same
            // connection; a checkpoint-projected run has none and keeps
            // the loud zero-only refusal).
            let ceiling = Self::load_recorded_run(&conn, run_id)?.map(|run| run.budget().clone());
            spent_or_restore(spent, ceiling.as_ref(), run_id, seq)?
        };
        Ok(Some(Checkpoint {
            run_id: run_id.clone(),
            reference_digest: Digest::parse(&digest_text).map_err(|err| {
                StateStoreError::Corrupt {
                    detail: format!("run {run_id} checkpoint {seq} holds a bad digest: {err}"),
                }
            })?,
            iteration_no: gauntlet_domain::IterationNo::new(
                u32::try_from(iteration_no).unwrap_or(0),
            )
            .map_err(|err| StateStoreError::Corrupt {
                detail: format!("run {run_id} checkpoint {seq} holds a bad iteration: {err}"),
            })?,
            unit_states,
            budget_spent,
            in_flight,
            // N3 (Task-12 correction, checkpoint leg only): the offset is
            // the §18.3 replay anchor — a stored value no u64 water-mark
            // can represent is CORRUPT, never a silent 0 (the constant-
            // zero defect C8.1 removed from the write path must not
            // return through the read path).
            event_log_offset: EventLogOffset::new(u64::try_from(offset).map_err(|_| {
                StateStoreError::Corrupt {
                    detail: format!(
                        "run {run_id} checkpoint {seq} holds a non-representable event-log \
                         offset ({offset})"
                    ),
                }
            })?),
        }))
    }

    fn record_role_invocation(
        &self,
        invocation: &RoleInvocation,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.append_record(
            invocation.run_id(),
            RecordKind::RoleInvocation,
            &encode_role_invocation(invocation),
            invocation
                .admitted_at()
                .unwrap_or(gauntlet_domain::EpochSeconds::new(0)),
            deadline,
        )
        .map(|_| ())
    }

    /// The controller's §15.1 selection record (AK-037): one append-only
    /// `records` row per recorded selection, like the role-invocation
    /// precedent. W4 (correction task26-dcritic-apparatus-repair): the `at`
    /// column now carries the caller's own real clock reading — the
    /// aggregate itself still carries no timestamp field (the column IS the
    /// fact), so record order (the append's `seq`) remains the ordering
    /// fact, but the row's own instant is honest.
    fn record_correction_selection(
        &self,
        selection: &CorrectionSelection,
        at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.append_record(
            selection.run_id(),
            RecordKind::CorrectionSelection,
            &encode_correction_selection(selection),
            at,
            deadline,
        )
        .map(|_| ())
    }

    fn correction_selections(
        &self,
        run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Vec<CorrectionSelection>, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let mut statement = conn
            .prepare(
                "SELECT payload FROM records WHERE run_id = ?1 AND record_type = 'correction_selection' \
                 ORDER BY seq ASC",
            )
            .map_err(|err| StateStoreError::ReadFailed {
                detail: format!("preparing the correction-selection query failed: {err}"),
            })?;
        let rows = statement
            .query_map([run_id.to_string()], |row| row.get::<_, String>(0))
            .map_err(|err| StateStoreError::ReadFailed {
                detail: format!("reading correction selections of run {run_id} failed: {err}"),
            })?;
        let mut selections = Vec::new();
        for row in rows {
            let payload = row.map_err(|err| StateStoreError::ReadFailed {
                detail: format!("stepping correction selections of run {run_id} failed: {err}"),
            })?;
            // Every row decodes through the domain constructors or names
            // itself Corrupt — never a degraded selection.
            selections.push(decode_correction_selection(&payload)?);
        }
        Ok(selections)
    }

    /// The §5.3.6 verification decision, durably: the typed Annex B.5
    /// document under the run it names, `at` the document's own
    /// `produced_at` provenance fact. One `records` row per call, like the
    /// role-invocation and correction-selection precedents. The argument is
    /// the recording authority's minted value (Task 13 correction round 1,
    /// COR-1) — a raw caller-built document has no route here.
    fn record_verification(
        &self,
        record: &gauntlet_ports::producer_authority::AdmittedVerificationRecord,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        let record = record.record();
        self.append_record_raw(
            record.run_id(),
            RecordKind::VerificationRecord,
            &record.to_json(),
            record.produced_at(),
            deadline,
        )
        .map(|_| ())
    }

    /// The run's durable verification records in record order. Strict
    /// decode through the Annex B.5 codec — a payload that does not decode
    /// is `Corrupt`, never a degraded record (the correction-selection
    /// precedent).
    fn verification_records(
        &self,
        run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Vec<VerificationRecordDoc>, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let mut statement = conn
            .prepare(
                "SELECT payload FROM records WHERE run_id = ?1 AND record_type = \
                 'verification_record' ORDER BY seq ASC",
            )
            .map_err(|err| StateStoreError::ReadFailed {
                detail: format!("preparing the verification-record query failed: {err}"),
            })?;
        let rows = statement
            .query_map([run_id.to_string()], |row| row.get::<_, String>(0))
            .map_err(|err| StateStoreError::ReadFailed {
                detail: format!("reading verification records of run {run_id} failed: {err}"),
            })?;
        let mut records = Vec::new();
        for row in rows {
            let payload = row.map_err(|err| StateStoreError::ReadFailed {
                detail: format!("stepping verification records of run {run_id} failed: {err}"),
            })?;
            records.push(VerificationRecordDoc::decode(&payload).map_err(|err| {
                StateStoreError::Corrupt {
                    detail: format!(
                        "a stored verification record of run {run_id} does not decode: {err}"
                    ),
                }
            })?);
        }
        Ok(records)
    }

    /// The run's lifecycle facts (corrections round 2, F-R4): one ordered
    /// scan of the run's durable event log, projected onto the three
    /// kinds the §26 report classifies from — the unit-attributed
    /// dispatches, the invocation terminals, and the run's own
    /// append-only terminals. The entries decode through the SAME strict
    /// codec every reader uses (`read_entries`), so a damaged event is a
    /// typed `Corrupt`, never a silent skip: the report classifies from
    /// records, and a damaged record is not a record.
    fn run_lifecycle_facts(
        &self,
        run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<RunLifecycleFacts, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let entries = read_entries(&conn, EventLogOffset::new(0)).map_err(map_log_error)?;
        let mut facts = RunLifecycleFacts::default();
        for entry in entries {
            if entry.event.run_id != *run_id {
                continue;
            }
            match entry.event.kind {
                RunEventKind::UnitDispatch {
                    unit,
                    invocation_id,
                } => facts.unit_dispatches.push((unit, invocation_id)),
                RunEventKind::InvocationTerminal {
                    invocation_id,
                    state,
                } => facts.invocation_terminals.push((invocation_id, state)),
                RunEventKind::RunTerminal { state } => facts.run_terminals.push(state),
                _ => {}
            }
        }
        Ok(facts)
    }

    /// The §16.3 atomic commit sequence behind the earned admission (the
    /// port docs carry the full contract; the authority/store division of
    /// labor is recorded there and in the Task-13 report). Step 1 appends
    /// the durable event; step 2 writes the cross-validated, keyed row.
    fn commit_settlement(
        &self,
        admitted: &AdmittedSettlement,
        _deadline: Deadline,
    ) -> Result<EventLogOffset, StateStoreError> {
        let commit = admitted.commit().clone();
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let event = RunEvent {
            run_id: commit.run_id.clone(),
            kind: RunEventKind::SettlementCommitted {
                unit_id: commit.unit_id.clone(),
                evidence: commit.evidence.clone(),
                verification: commit.verification.clone(),
                reference_digest: commit.reference_digest.clone(),
            },
            at: commit.at,
        };
        // Step 1 — durable before anything else happens (§16.3).
        let offset = append_to_log(&conn, &event).map_err(|err| StateStoreError::WriteFailed {
            detail: err.to_string(),
        })?;
        // Step 2 — idempotent keyed insert. If THIS step fails, the event
        // from step 1 is already durable, and the error must say so: a
        // caller reading a bare failure as "nothing happened" would re-run
        // the unit and append a duplicate settlement event. The error names
        // the durable offset, the recovery that completes it, and why
        // re-running is wrong.
        self.apply_settlement_row_with(&conn, &commit, offset)
            .map_err(|err| StateStoreError::WriteFailed {
                detail: format!(
                    "{err}; the settlement event for unit {} of run {} IS durable at offset {} \
                     — complete_pending_settlements will complete this settlement on replay; \
                     re-running the unit is wrong (it would append a duplicate settlement \
                     event)",
                    commit.unit_id,
                    commit.run_id,
                    offset.get()
                ),
            })?;
        Ok(offset)
    }

    /// The run's durable settlement rows, ordered by `(unit_id,
    /// verification_ref)` — the order the export pins.
    fn settlements(
        &self,
        run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Vec<SettlementRecord>, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        settlements_of(&conn, run_id)
    }

    /// §18.3 resume step 1 (plan Task 13 B1 moves this to the port; the
    /// anchor is the run's latest checkpoint offset — B2 composes
    /// `resume` → checkpoint → here). Replays the run's settlement events
    /// AT OR AFTER the anchor — events below it are the checkpoint's own
    /// certification and are not re-scanned — and completes any half-applied
    /// settlement, idempotent by `(run_id, unit_id, verification_ref)`
    /// (§16.3). Returns how many settlement rows this call added; a second
    /// call over the same range adds none.
    fn complete_pending_settlements(
        &self,
        run_id: &RunId,
        anchor: EventLogOffset,
        _deadline: Deadline,
    ) -> Result<usize, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let entries = read_entries(&conn, anchor).map_err(map_log_error)?;
        let mut applied = 0;
        for entry in entries {
            let event = entry.event;
            if event.run_id != *run_id {
                continue;
            }
            let RunEventKind::SettlementCommitted {
                unit_id,
                evidence,
                verification,
                reference_digest,
            } = event.kind
            else {
                continue;
            };
            let added = insert_settlement(
                &conn,
                &SettlementCommit {
                    run_id: event.run_id.clone(),
                    unit_id,
                    evidence,
                    verification,
                    reference_digest,
                    at: event.at,
                },
                entry.offset,
            )?;
            applied += added;
        }
        Ok(applied)
    }

    fn bind_observation_bundle(
        &self,
        run_id: &RunId,
        iteration_no: IterationNo,
        binding: &gauntlet_ports::producer_authority::AdmittedObservationBinding,
        _deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        let payload = binding.binding();
        let evidence = payload.evidence();
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let existing: Option<(String, String)> = match conn.query_row(
            "SELECT evidence_ref, observation_digest FROM observation_bindings \
             WHERE run_id = ?1 AND iteration_no = ?2",
            (run_id.to_string(), u64::from(iteration_no.get())),
            |row| Ok((row.get(0)?, row.get(1)?)),
        ) {
            Ok(found) => Some(found),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(err) => {
                return Err(StateStoreError::ReadFailed {
                    detail: format!(
                        "reading the observation binding of run {run_id} iteration {} failed: \
                         {err}",
                        iteration_no.get()
                    ),
                });
            }
        };
        // Write-once. Re-binding the SAME ref is idempotent; re-binding a
        // different one is a substitution attempt and is refused, never
        // absorbed.
        if let Some((found_ref, found_digest)) = existing {
            if found_ref == evidence.as_str() && found_digest == payload.digest().to_string() {
                return Ok(());
            }
            return Err(StateStoreError::WriteFailed {
                detail: format!(
                    "run {run_id} iteration {} is already bound to observation evidence \
                     {found_ref} ({found_digest}); rebinding it to {evidence} ({}) is refused",
                    iteration_no.get(),
                    payload.digest()
                ),
            });
        }
        conn.execute(
            "INSERT INTO observation_bindings \
             (run_id, iteration_no, evidence_ref, observation_digest) \
             VALUES (?1, ?2, ?3, ?4)",
            (
                run_id.to_string(),
                u64::from(iteration_no.get()),
                evidence.as_str(),
                payload.digest().to_string(),
            ),
        )
        .map_err(|err| StateStoreError::WriteFailed {
            detail: format!(
                "binding observation evidence {evidence} to run {run_id} iteration {} failed: \
                 {err}",
                iteration_no.get()
            ),
        })?;
        Ok(())
    }

    fn observation_binding(
        &self,
        run_id: &RunId,
        iteration_no: IterationNo,
        _deadline: Deadline,
    ) -> Result<Option<ObservationBinding>, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let found: Option<(String, String)> = match conn.query_row(
            "SELECT evidence_ref, observation_digest FROM observation_bindings \
             WHERE run_id = ?1 AND iteration_no = ?2",
            (run_id.to_string(), u64::from(iteration_no.get())),
            |row| Ok((row.get(0)?, row.get(1)?)),
        ) {
            Ok(found) => Some(found),
            Err(rusqlite::Error::QueryReturnedNoRows) => None,
            Err(err) => {
                return Err(StateStoreError::ReadFailed {
                    detail: format!(
                        "reading the observation binding of run {run_id} iteration {} failed: \
                         {err}",
                        iteration_no.get()
                    ),
                });
            }
        };
        found
            .map(|(reference, digest)| {
                let corrupt = |error: String| StateStoreError::Corrupt {
                    detail: format!(
                        "the stored observation binding of run {run_id} iteration {} does not \
                         decode: {error}",
                        iteration_no.get()
                    ),
                };
                let reference =
                    EvidenceRef::new(&reference).map_err(|error| corrupt(error.to_string()))?;
                let digest = Digest::parse(&digest).map_err(|error| corrupt(error.to_string()))?;
                Ok(ObservationBinding::new(reference, digest))
            })
            .transpose()
    }

    fn record_evidence_production(
        &self,
        run_id: &RunId,
        production: &gauntlet_ports::producer_authority::AdmittedEvidenceProduction,
        deadline: Deadline,
    ) -> Result<gauntlet_ports::state_store::EvidenceProductionBinding, StateStoreError> {
        // The payload is authority-minted (plan Task 12): its content digest
        // was derived from the store's own bytes by the ports authority, so
        // this method receives producer FACTS only — never a caller-chosen
        // digest.
        let production = production.production();
        // task26 (append-only correction): the row's key is now (run,
        // evidence ref, ITERATION) — every governed re-production earns
        // its own row, never overwriting or collapsing onto a prior
        // iteration's. Two prior facts stay exactly write-once: the SAME
        // iteration re-recording the SAME facts is idempotent (no new
        // event); the SAME iteration with DIFFERING facts is refused. The
        // new fact this migration exists for: a LATER iteration is a NEW
        // row — §14.3 condition 3 ("produced in this run at or after the
        // transform it certifies") names a fact about WHEN the instrument
        // made bytes, and a later iteration's governed re-production of
        // identical bytes is a real, separate instance of that fact, not
        // a duplicate of the earlier one. A declared iteration at or
        // before an EXISTING row for this ref is refused as stale/replayed
        // (recorded facts never move backward); a producer/locator/content
        // mismatch against any existing row for this ref is refused as a
        // genuine conflict (a work product's identity does not change
        // across iterations under one evidence ref).
        let existing_rows =
            self.production_rows_for_ref(run_id, production.evidence(), deadline)?;
        if let Some(same_iteration) = existing_rows
            .iter()
            .find(|row| row.production().iteration_no() == production.iteration_no())
        {
            if same_iteration.production() == production {
                return Ok(same_iteration.clone());
            }
            return Err(StateStoreError::WriteFailed {
                detail: format!(
                    "run {run_id} already recorded evidence {} produced by {} at {} iteration {} \
                     (anchor offset {}); recording it again with different facts at the SAME \
                     iteration is refused",
                    production.evidence(),
                    production.producer().name(),
                    production.locator(),
                    production.iteration_no().get(),
                    same_iteration.event_log_offset().get(),
                ),
            });
        }
        if let Some(freshest) = existing_rows.last() {
            let same_work_product = freshest.production().content() == production.content()
                && freshest.production().producer() == production.producer()
                && freshest.production().locator() == production.locator();
            if !same_work_product {
                return Err(StateStoreError::WriteFailed {
                    detail: format!(
                        "run {run_id} already recorded evidence {} produced by {} at {} (anchor \
                         offset {}); recording it again with a different producer/locator/content \
                         is refused",
                        production.evidence(),
                        freshest.production().producer().name(),
                        freshest.production().locator(),
                        freshest.event_log_offset().get(),
                    ),
                });
            }
            if production.iteration_no().get() <= freshest.production().iteration_no().get() {
                return Err(StateStoreError::WriteFailed {
                    detail: format!(
                        "run {run_id} already recorded evidence {} at iteration {} (anchor offset \
                         {}); a re-production declared at iteration {} — at or before the durable \
                         row's own — is refused as stale or replayed",
                        production.evidence(),
                        freshest.production().iteration_no().get(),
                        freshest.event_log_offset().get(),
                        production.iteration_no().get(),
                    ),
                });
            }
        }
        // The ordering anchor: minted by THIS append, inside the store, from
        // a value no caller of this method supplies (spec §14.3 condition 3).
        let event = RunEvent {
            run_id: run_id.clone(),
            kind: RunEventKind::EvidenceProduced {
                evidence: production.evidence().clone(),
                locator: production.locator().clone(),
            },
            at: production.produced_at(),
        };
        let offset = self.append_event(event, deadline)?;
        let binding = gauntlet_ports::state_store::EvidenceProductionBinding::new(
            run_id.clone(),
            production.clone(),
            offset,
        );
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        // If THIS insert fails the event above is already durable; the row
        // is the authority and its absence is visible to every read, so the
        // error names the durable offset rather than pretending nothing
        // happened (the same crash-window discipline as settlement step 2).
        conn.execute(
            "INSERT INTO evidence_productions (
                run_id, evidence_ref, producer_name, producer_version, locator, iteration_no,
                produced_at, event_log_offset, content_digest
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            (
                run_id.to_string(),
                production.evidence().as_str(),
                production.producer().name().as_str(),
                production.producer().version(),
                production.locator().as_str(),
                u64::from(production.iteration_no().get()),
                production.produced_at().get(),
                offset.get(),
                production.content().to_string(),
            ),
        )
        .map_err(|err| StateStoreError::WriteFailed {
            detail: format!(
                "recording the production of evidence {} in run {run_id} at iteration {} failed: \
                 {err}; the EvidenceProduced event IS durable at offset {} — replay recovery \
                 should complete the row before re-recording",
                production.evidence(),
                production.iteration_no().get(),
                offset.get(),
            ),
        })?;
        Ok(binding)
    }

    fn evidence_production(
        &self,
        run_id: &RunId,
        evidence: &EvidenceRef,
        deadline: Deadline,
    ) -> Result<Option<gauntlet_ports::state_store::EvidenceProductionBinding>, StateStoreError>
    {
        // The FRESHEST row for this ref (task26: a ref may now carry one
        // row per iteration it was genuinely re-produced in) — every
        // caller of this singular read wants "what stands for this ref
        // right now", which is its latest anchor, exactly like
        // `evidence_productions_at`'s own freshest-last ordering.
        Ok(self
            .production_rows_for_ref(run_id, evidence, deadline)?
            .into_iter()
            .next_back())
    }

    fn evidence_productions_at(
        &self,
        run_id: &RunId,
        locator: &gauntlet_domain::EvidenceLocator,
        _deadline: Deadline,
    ) -> Result<Vec<gauntlet_ports::state_store::EvidenceProductionBinding>, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let mut statement = conn
            .prepare(
                "SELECT evidence_ref, producer_name, producer_version, iteration_no, produced_at, \
                 event_log_offset, content_digest FROM evidence_productions WHERE run_id = ?1 AND \
                 locator = ?2 ORDER BY event_log_offset ASC",
            )
            .map_err(|err| StateStoreError::ReadFailed {
                detail: format!(
                    "preparing the locator production query for run {run_id} failed: {err}"
                ),
            })?;
        let rows = statement
            .query_map((run_id.to_string(), locator.as_str()), |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })
            .map_err(|err| StateStoreError::ReadFailed {
                detail: format!(
                    "reading the productions at locator {locator} of run {run_id} failed: {err}"
                ),
            })?;
        let mut bindings = Vec::new();
        for row in rows {
            let (reference, name, version, iteration, produced_at, offset, content) =
                row.map_err(|err| StateStoreError::ReadFailed {
                    detail: format!(
                        "stepping the productions at locator {locator} of run {run_id} failed: \
                         {err}",
                    ),
                })?;
            let evidence =
                EvidenceRef::new(&reference).map_err(|err| StateStoreError::Corrupt {
                    detail: format!(
                        "a production record of run {run_id} holds an invalid evidence ref: {err}"
                    ),
                })?;
            bindings.push(production_from_parts(
                run_id,
                &evidence,
                &name,
                &version,
                locator.as_str(),
                iteration,
                produced_at,
                offset,
                &content,
            )?);
        }
        Ok(bindings)
    }

    fn accept_comparison(
        &self,
        admitted: &gauntlet_ports::comparator::AdmittedComparison,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        let spec = admitted.spec();
        let document = admitted.document();
        let evidence = admitted.evidence();
        if document.run_id() != &spec.run_id
            || document.iteration_no() != spec.iteration_no
            || document.envelope_digest() != spec.context_envelope.digest()
        {
            return Err(StateStoreError::WriteFailed {
                detail: "comparison does not match its admitted invocation tuple".to_owned(),
            });
        }
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let run = Self::load_recorded_run(&conn, &spec.run_id)?.ok_or_else(|| {
            StateStoreError::Unavailable {
                detail: format!(
                    "cannot accept comparison: durable run {} does not exist",
                    spec.run_id
                ),
            }
        })?;
        if document.reference_digest() != run.reference_version().digest() {
            return Err(StateStoreError::WriteFailed {
                detail: format!(
                    "comparison reference digest {} differs from durable run {} frozen digest {}",
                    document.reference_digest(),
                    spec.run_id,
                    run.reference_version().digest()
                ),
            });
        }
        let invocation_present: i64 = conn.query_row(
            "SELECT COUNT(*) FROM records WHERE run_id = ?1 AND record_type = 'role_invocation' AND json_extract(payload, '$.id') = ?2 AND json_extract(payload, '$.iteration_no') = ?3 AND json_extract(payload, '$.context_envelope_digest') = ?4",
            (spec.run_id.to_string(), spec.invocation_id.to_string(), u64::from(spec.iteration_no.get()), spec.context_envelope.digest().to_string()),
            |row| row.get(0),
        ).map_err(|error| StateStoreError::ReadFailed { detail: format!("checking durable admitted invocation failed: {error}") })?;
        if invocation_present != 1 {
            return Err(StateStoreError::Unavailable {
                detail: format!(
                    "cannot accept comparison: admitted invocation {} is absent or ambiguous",
                    spec.invocation_id
                ),
            });
        }
        drop(conn);
        self.append_record(
            &spec.run_id,
            RecordKind::Comparison,
            &encode_comparison(document, evidence),
            gauntlet_domain::EpochSeconds::new(0),
            deadline,
        )
        .map(|_| ())
    }

    /// Completes the `EvidenceProduced` crash window (plan Task 13, A5 —
    /// the window Task 11's error string names): the event is durable, the
    /// row is not. The row's missing facts arrive authority-MINTED (the
    /// same mint `record_evidence_production` takes); the completed row
    /// binds to the ORPHAN EVENT'S offset with `produced_at` from the
    /// event's stored timestamp, and NOTHING is appended. See the port docs
    /// for the full contract.
    fn complete_pending_evidence_production(
        &self,
        run_id: &RunId,
        anchor: EventLogOffset,
        production: &gauntlet_ports::producer_authority::AdmittedEvidenceProduction,
        deadline: Deadline,
    ) -> Result<Option<gauntlet_ports::state_store::EvidenceProductionBinding>, StateStoreError>
    {
        let payload = production.production();
        // Already applied → nothing to complete (idempotent on the
        // production identity). Checked before the lock is taken: this call
        // takes its own.
        if self
            .evidence_production(run_id, payload.evidence(), deadline)?
            .is_some()
        {
            return Ok(None);
        }
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let entries = read_entries(&conn, anchor).map_err(map_log_error)?;
        let orphan = entries.into_iter().find(|entry| {
            entry.event.run_id == *run_id
                && matches!(
                    &entry.event.kind,
                    RunEventKind::EvidenceProduced { evidence, .. } if evidence == payload.evidence()
                )
        });
        let orphan = match orphan {
            Some(entry) => entry,
            // No orphan event at or after the anchor: nothing is pending.
            None => return Ok(None),
        };
        let RunEventKind::EvidenceProduced { locator, .. } = &orphan.event.kind else {
            return Err(StateStoreError::ReadFailed {
                detail: format!(
                    "the EvidenceProduced scan matched a non-production event at offset {}",
                    orphan.offset.get()
                ),
            });
        };
        if locator != payload.locator() {
            return Err(StateStoreError::WriteFailed {
                detail: format!(
                    "the durable EvidenceProduced event for {} of run {run_id} (offset {}) names \
                     locator {locator}, but the minted production declares {} — the orphan event \
                     belongs to a different production of the same ref and no row is fabricated \
                     over it",
                    payload.evidence(),
                    orphan.offset.get(),
                    payload.locator()
                ),
            });
        }
        // The completed row: every fact from the mint, the ordering anchor
        // and the timestamp from the DURABLE event (the A3 discipline,
        // mirrored — the re-mint's clock is not a durable fact).
        let completed = gauntlet_ports::state_store::EvidenceProduction::new(
            payload.evidence().clone(),
            payload.content().clone(),
            payload.producer().clone(),
            payload.locator().clone(),
            payload.iteration_no(),
            orphan.event.at,
        );
        let binding = gauntlet_ports::state_store::EvidenceProductionBinding::new(
            run_id.clone(),
            completed.clone(),
            orphan.offset,
        );
        // If THIS insert fails, a concurrent record may have landed the row
        // (the check-then-insert window above); the constraint refusal says
        // so — never a silent second row.
        conn.execute(
            "INSERT INTO evidence_productions (
                run_id, evidence_ref, producer_name, producer_version, locator, iteration_no,
                produced_at, event_log_offset, content_digest
            ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)",
            (
                run_id.to_string(),
                completed.evidence().as_str(),
                completed.producer().name().as_str(),
                completed.producer().version(),
                completed.locator().as_str(),
                u64::from(completed.iteration_no().get()),
                completed.produced_at().get(),
                orphan.offset.get(),
                completed.content().to_string(),
            ),
        )
        .map_err(|err| StateStoreError::WriteFailed {
            detail: format!(
                "completing the production of evidence {} in run {run_id} against the durable \
                 EvidenceProduced event at offset {} failed: {err}",
                completed.evidence(),
                orphan.offset.get()
            ),
        })?;
        Ok(Some(binding))
    }

    /// COR-R3-1 (falsifier round 3, F-R3-1): the orphan-event read the
    /// §18.3 resume-completion recovery needs to re-mint a missing row at
    /// the ref the produce step actually recorded — the same replay scan
    /// `complete_pending_evidence_production` performs, surfaced as facts
    /// instead of a keyed completion.
    fn pending_evidence_production_events(
        &self,
        run_id: &RunId,
        anchor: EventLogOffset,
        locator: &gauntlet_domain::EvidenceLocator,
        _deadline: Deadline,
    ) -> Result<Vec<gauntlet_ports::state_store::OrphanEvidenceProduction>, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let entries = read_entries(&conn, anchor).map_err(map_log_error)?;
        let mut orphans = Vec::new();
        for entry in entries {
            if entry.event.run_id != *run_id {
                continue;
            }
            let RunEventKind::EvidenceProduced {
                evidence,
                locator: event_locator,
            } = &entry.event.kind
            else {
                continue;
            };
            if event_locator != locator {
                continue;
            }
            orphans.push(gauntlet_ports::state_store::OrphanEvidenceProduction::new(
                evidence.clone(),
                entry.offset,
                entry.event.at,
            ));
        }
        Ok(orphans)
    }

    /// Deterministic export (spec §6.2: "Export is deterministic"): stable
    /// segments and explicit `ORDER BY` everywhere — the output is a pure
    /// function of durable state, never of query timing. Timestamps are the
    /// stored ones; none is generated here.
    fn export_jsonl(&self, run_id: &RunId, _deadline: Deadline) -> Result<String, StateStoreError> {
        let conn = self
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        let mut lines: Vec<String> = Vec::new();
        // Segment 1 — the run identity, when recorded.
        if let Some(run) = Self::load_recorded_run(&conn, run_id)? {
            let budget =
                json::parse(&encode::encode_budget_ceiling(run.budget())).map_err(|err| {
                    StateStoreError::ReadFailed {
                        detail: format!("the run's budget does not re-parse: {err}"),
                    }
                })?;
            lines.push(
                JsonValue::Object(vec![
                    ("record".to_owned(), JsonValue::Str("run".to_owned())),
                    ("run_id".to_owned(), JsonValue::Str(run.id().to_string())),
                    ("created_at".to_owned(), uint(run.created_at().get())),
                    (
                        "reference_id".to_owned(),
                        JsonValue::Str(run.reference_version().reference_id().as_str().to_owned()),
                    ),
                    (
                        "reference_version".to_owned(),
                        uint(u64::from(run.reference_version().version())),
                    ),
                    (
                        "reference_digest".to_owned(),
                        JsonValue::Str(run.reference_version().digest().to_string()),
                    ),
                    (
                        "reference_evidence_ref".to_owned(),
                        JsonValue::Str(run.reference_version().evidence_ref().as_str().to_owned()),
                    ),
                    ("intent".to_owned(), JsonValue::Str(run.intent().to_owned())),
                    (
                        "config_fingerprint".to_owned(),
                        JsonValue::Str(run.config_fingerprint().to_string()),
                    ),
                    (
                        "environment_fingerprint".to_owned(),
                        JsonValue::Str(run.environment_fingerprint().to_string()),
                    ),
                    (
                        "parent_run".to_owned(),
                        match run.parent_run() {
                            Some(parent) => JsonValue::Str(parent.to_string()),
                            None => JsonValue::Null,
                        },
                    ),
                    ("budget".to_owned(), budget),
                ])
                .to_json_text(),
            );
        }
        // Segment 2 — events, in offset order (the append order).
        let entries = read_entries(&conn, EventLogOffset::new(0)).map_err(map_log_error)?;
        for entry in entries {
            if entry.event.run_id != *run_id {
                continue;
            }
            let (kind, payload) = encode::encode_event(&entry.event.kind);
            let body = json::parse(&payload).map_err(|err| StateStoreError::ReadFailed {
                detail: format!("an event payload does not re-parse: {err}"),
            })?;
            lines.push(
                JsonValue::Object(vec![
                    ("record".to_owned(), JsonValue::Str("event".to_owned())),
                    ("offset".to_owned(), uint(entry.offset.get())),
                    ("at".to_owned(), uint(entry.event.at.get())),
                    ("kind".to_owned(), JsonValue::Str(kind)),
                    ("body".to_owned(), body),
                ])
                .to_json_text(),
            );
        }
        // Segment 3 — checkpoints, ordered by (iteration, write order).
        let mut statement = conn
            .prepare(
                "SELECT seq, iteration_no, reference_digest, unit_states, budget_spent, \
                 in_flight, event_log_offset FROM checkpoints WHERE run_id = ?1 \
                 ORDER BY iteration_no ASC, seq ASC",
            )
            .map_err(|err| StateStoreError::ReadFailed {
                detail: format!("preparing the checkpoint export failed: {err}"),
            })?;
        let checkpoint_rows = statement
            .query_map([run_id.to_string()], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            })
            .map_err(|err| StateStoreError::ReadFailed {
                detail: format!("reading checkpoints for export failed: {err}"),
            })?;
        for row in checkpoint_rows {
            let (seq, iteration_no, digest, unit_states, budget_spent, in_flight, offset) = row
                .map_err(|err| StateStoreError::ReadFailed {
                    detail: format!("a checkpoint row does not read: {err}"),
                })?;
            let unit_body =
                json::parse(&unit_states).map_err(|err| StateStoreError::ReadFailed {
                    detail: format!("checkpoint {seq} unit states do not re-parse: {err}"),
                })?;
            let spent_body =
                json::parse(&budget_spent).map_err(|err| StateStoreError::ReadFailed {
                    detail: format!("checkpoint {seq} budget does not re-parse: {err}"),
                })?;
            let flight_body =
                json::parse(&in_flight).map_err(|err| StateStoreError::ReadFailed {
                    detail: format!("checkpoint {seq} in-flight list does not re-parse: {err}"),
                })?;
            lines.push(
                JsonValue::Object(vec![
                    ("record".to_owned(), JsonValue::Str("checkpoint".to_owned())),
                    ("seq".to_owned(), uint(u64::try_from(seq).unwrap_or(0))),
                    (
                        "iteration_no".to_owned(),
                        uint(u64::try_from(iteration_no).unwrap_or(0)),
                    ),
                    (
                        "event_log_offset".to_owned(),
                        uint(u64::try_from(offset).unwrap_or(0)),
                    ),
                    ("reference_digest".to_owned(), JsonValue::Str(digest)),
                    ("unit_states".to_owned(), unit_body),
                    ("budget_spent".to_owned(), spent_body),
                    ("in_flight".to_owned(), flight_body),
                ])
                .to_json_text(),
            );
        }
        // Segment 4 — settlements, ordered by (unit, verification).
        for record in settlements_of(&conn, run_id)? {
            lines.push(
                JsonValue::Object(vec![
                    ("record".to_owned(), JsonValue::Str("settlement".to_owned())),
                    (
                        "unit_id".to_owned(),
                        JsonValue::Str(record.unit_id().to_string()),
                    ),
                    (
                        "verification".to_owned(),
                        JsonValue::Str(record.verification().to_string()),
                    ),
                    (
                        "evidence".to_owned(),
                        JsonValue::Str(record.evidence().to_string()),
                    ),
                    (
                        "event_log_offset".to_owned(),
                        uint(record.event_log_offset().get()),
                    ),
                ])
                .to_json_text(),
            );
        }
        // Segment 5 — the remaining §18.1 record kinds, ordered by
        // (kind, seq).
        let mut statement = conn
            .prepare(
                "SELECT seq, record_type, payload, at FROM records WHERE run_id = ?1 \
                 ORDER BY record_type ASC, seq ASC",
            )
            .map_err(|err| StateStoreError::ReadFailed {
                detail: format!("preparing the records export failed: {err}"),
            })?;
        let record_rows = statement
            .query_map([run_id.to_string()], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            })
            .map_err(|err| StateStoreError::ReadFailed {
                detail: format!("reading records for export failed: {err}"),
            })?;
        for row in record_rows {
            let (seq, record_type, payload, at) =
                row.map_err(|err| StateStoreError::ReadFailed {
                    detail: format!("a record row does not read: {err}"),
                })?;
            if at < 0 {
                return Err(StateStoreError::Corrupt {
                    detail: format!(
                        "record seq {seq} of run {run_id} holds an at of {at} — negative; this \
                         adapter never writes one, so the row was damaged or tampered out of band"
                    ),
                });
            }
            let body = json::parse(&payload).map_err(|err| StateStoreError::ReadFailed {
                detail: format!("record {seq} does not re-parse: {err}"),
            })?;
            lines.push(
                JsonValue::Object(vec![
                    ("record".to_owned(), JsonValue::Str(record_type)),
                    ("seq".to_owned(), uint(u64::try_from(seq).unwrap_or(0))),
                    ("at".to_owned(), uint(at as u64)),
                    ("payload".to_owned(), body),
                ])
                .to_json_text(),
            );
        }
        Ok(lines.join("\n"))
    }
}

/// Resume fidelity for `budget_spent`: the six counters are durable, and
/// admission is the DURABLE RUN BUDGET's own restore gate — the domain
/// `Budget::restore` path landed for exactly this composition (plan Task
/// 12, B1's deferred item: the controller composition is what first
/// produces charged checkpoints). A water-mark no legal charge sequence
/// can leave refuses with the domain's typed `RestoreUnreachable` cause,
/// never a silent zeroing; the zero water-mark round-trips exactly. A run
/// with NO recorded budget (the checkpoint-projected shape) still has no
/// ceilings in scope to admit against, so it keeps the loud zero-only
/// refusal.
fn spent_or_restore(
    numbers: encode::SpentNumbers,
    ceiling: Option<&Budget>,
    run_id: &RunId,
    seq: i64,
) -> Result<Spent, StateStoreError> {
    let zero = numbers.rounds == 0
        && numbers.wall_seconds == 0
        && numbers.tokens == 0
        && numbers.cost_usd == 0
        && numbers.diagnostic_tokens == 0
        && numbers.diagnostic_cost_usd == 0;
    if zero {
        return Ok(zero_spent());
    }
    let Some(budget) = ceiling else {
        return Err(StateStoreError::Corrupt {
            detail: format!(
                "run {run_id} checkpoint {seq} stores non-zero spend (tokens {}, micro-usd {}) \
                 with no recorded run budget to admit it against — refusing rather than \
                 silently zeroing",
                numbers.tokens, numbers.cost_usd
            ),
        });
    };
    let spent = Spent::from_counters(
        u32::try_from(numbers.rounds).unwrap_or(u32::MAX),
        numbers.wall_seconds,
        numbers.tokens,
        MicroUsd::new(numbers.cost_usd),
        numbers.diagnostic_tokens,
        MicroUsd::new(numbers.diagnostic_cost_usd),
    );
    let _ = Budget::restore(
        *budget.limits(),
        budget.recovery_reserve_pct(),
        *budget.diagnostic_allowance(),
        spent,
    )
    .map_err(|cause| StateStoreError::Corrupt {
        detail: format!(
            "run {run_id} checkpoint {seq} stores a spend water-mark the durable run budget \
             does not admit: {cause}",
        ),
    })?;
    Ok(spent)
}

#[cfg(test)]
mod tests {
    use super::*;
    use gauntlet_domain::{
        DiagnosticAllowance, InvocationId, IterationNo, ResultContractPath, UnitStatus,
    };
    use gauntlet_ports::state_store::Checkpoint;

    const RUN: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const INVOCATION: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAW";
    const DIGEST_HEX: &str = "c4d3e2f1a0b1c2d3";

    fn run_id() -> RunId {
        RunId::new(RUN).expect("valid run id")
    }

    fn bound() -> Deadline {
        Deadline::at_epoch(EpochSeconds::new(1_700_000_060))
    }

    fn digest() -> Digest {
        Digest::new("sha256", DIGEST_HEX).expect("valid digest")
    }

    /// COR3 (confirmation round 1, F-1): the INTERNAL append path for the
    /// implementation's own tests. The public `append_event` refuses
    /// `SettlementCommitted` — that gate is the correction — but the
    /// replay, anchor, cross-validation, and golden-export fixtures model
    /// durable states a REAL crash between §16.3 steps 1 and 2 leaves:
    /// states the public API can no longer create on purpose, which
    /// recovery must still handle. Same internal write the trait's own
    /// `commit_settlement` uses (`append_to_log`), over the held
    /// connection.
    fn append_settlement_event_unchecked(
        store: &SqliteStateStore,
        event: RunEvent,
    ) -> Result<EventLogOffset, StateStoreError> {
        let conn = store
            .conn
            .lock()
            .expect("the store connection is not poisoned");
        append_to_log(&conn, &event).map_err(|err| StateStoreError::WriteFailed {
            detail: err.to_string(),
        })
    }

    fn run() -> Run {
        run_with_evidence(EvidenceRef::new("evd-task6-frozen-reference").expect("valid evidence"))
    }

    fn run_with_evidence(evidence_ref: EvidenceRef) -> Run {
        Run::new(RunDefinition {
            id: run_id(),
            created_at: EpochSeconds::new(1_700_000_000),
            reference_version: ReferenceVersion::with_evidence(
                ReferenceId::new("ref-task6").expect("valid reference id"),
                3,
                digest(),
                evidence_ref,
            ),
            intent: "settle the task".to_owned(),
            config_fingerprint: Digest::new("sha256", "a1b2c3d4e5f6a7b8").expect("valid digest"),
            environment_fingerprint: Digest::new("sha256", "e5f6a7b8c4d3e2f1")
                .expect("valid digest"),
            budget: Budget::new(
                Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("legal ceilings"),
                20,
                DiagnosticAllowance::new(100, MicroUsd::new(50)),
            )
            .expect("legal budget"),
            parent_run: None,
        })
        .expect("legal run")
    }

    fn checkpoint(iteration: u32, offset: u64, status: UnitStatus, in_flight: bool) -> Checkpoint {
        Checkpoint {
            run_id: run_id(),
            reference_digest: digest(),
            iteration_no: IterationNo::new(iteration).expect("1-based"),
            unit_states: vec![(UnitId::new("AK-900").expect("valid unit id"), status)],
            budget_spent: *run().budget().spent(),
            in_flight: if in_flight {
                vec![(
                    InvocationId::new(INVOCATION).expect("valid invocation id"),
                    ResultContractPath::new("result-contract.json").expect("valid path"),
                )]
            } else {
                Vec::new()
            },
            event_log_offset: EventLogOffset::new(offset),
        }
    }

    /// N3 (Task-12 correction, checkpoint leg ONLY): §18.3 step 1 makes the
    /// checkpoint's `event_log_offset` the anchor replay starts from, so a
    /// tampered or negative stored offset silently reading `0` re-introduces
    /// the constant-zero defect through the READ path — exactly what C8.1
    /// removed from the write path. A non-representable stored offset is
    /// CORRUPT, never a silent zero.
    #[test]
    fn a_tampered_checkpoint_offset_is_corrupt_never_a_silent_zero() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        store.record_run(&run(), bound()).expect("run records");
        store
            .checkpoint(checkpoint(1, 3, UnitStatus::Open, false), bound())
            .expect("checkpoint writes");
        // Tamper the durable row directly: a negative integer cannot
        // round-trip a u64 event-log water-mark.
        store
            .conn
            .lock()
            .expect("the store connection is not poisoned")
            .execute("UPDATE checkpoints SET event_log_offset = -1", [])
            .expect("the tamper applies");
        match store.resume(&run_id(), bound()) {
            Err(StateStoreError::Corrupt { detail }) => {
                assert!(
                    detail.contains("event-log offset") && detail.contains("-1"),
                    "the refusal names the locus and the stored value: {detail}"
                );
            }
            Ok(Some(checkpoint)) => panic!(
                "a non-representable stored offset is corrupt, never a silent zero (got {})",
                checkpoint.event_log_offset.get()
            ),
            Ok(None) => panic!("the checkpoint exists; the read cannot pretend otherwise"),
            Err(other) => panic!("the refusal is Corrupt, got {other}"),
        }
    }

    /// The golden export: the exact bytes a known history exports as, pinned
    /// so any ordering or shape drift is a diff, not a vibe. Note the two
    /// checkpoints are written OUT of iteration order — the export must
    /// order by iteration, not by write.
    #[test]
    fn export_is_the_pinned_golden_and_byte_identical() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        store.record_run(&run(), bound()).expect("run records");
        let admitted = RunEvent {
            run_id: run_id(),
            kind: RunEventKind::InvocationAdmitted {
                invocation_id: InvocationId::new(INVOCATION).expect("valid invocation id"),
            },
            at: EpochSeconds::new(1_700_000_100),
        };
        store
            .append_event(admitted, bound())
            .expect("event appends");
        let settled = RunEvent {
            run_id: run_id(),
            kind: RunEventKind::SettlementCommitted {
                unit_id: UnitId::new("AK-900").expect("valid unit id"),
                evidence: EvidenceRef::new("evd-000900").expect("valid ref"),
                verification: VerificationRef::new("vrf-000900").expect("valid ref"),
                reference_digest: digest(),
            },
            at: EpochSeconds::new(1_700_000_101),
        };
        // COR3: the settlement kind reaches the log through the internal
        // path — the golden bytes pin the CRASH-WINDOW history (event
        // durable, row not yet), not a public append.
        append_settlement_event_unchecked(&store, settled).expect("event appends");
        store
            .complete_pending_settlements(&run_id(), EventLogOffset::new(0), bound())
            .expect("replay applies the settlement");
        store
            .checkpoint(checkpoint(2, 2, UnitStatus::InProgress, true), bound())
            .expect("checkpoint writes");
        store
            .checkpoint(checkpoint(1, 1, UnitStatus::Open, false), bound())
            .expect("checkpoint writes");
        store
            .append_record(
                &run_id(),
                RecordKind::Observation,
                r#"{"facet":"filesystem"}"#,
                EpochSeconds::new(1_700_000_103),
                bound(),
            )
            .expect("record appends");

        let export = store.export_jsonl(&run_id(), bound()).expect("exports");
        let again = store
            .export_jsonl(&run_id(), bound())
            .expect("exports again");
        assert_eq!(export, again, "two exports must be byte-identical");

        let expected = [
            r#"{"record":"run","run_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV","created_at":1700000000,"reference_id":"ref-task6","reference_version":3,"reference_digest":"sha256:c4d3e2f1a0b1c2d3","reference_evidence_ref":"evd-task6-frozen-reference","intent":"settle the task","config_fingerprint":"sha256:a1b2c3d4e5f6a7b8","environment_fingerprint":"sha256:e5f6a7b8c4d3e2f1","parent_run":null,"budget":{"recovery_reserve_pct":20,"limits":{"rounds":8,"parallel":2,"depth":2,"wall_seconds":3600,"tokens":1000,"cost_usd":500},"diagnostic":{"tokens":100,"cost_usd":50}}}"#,
            r#"{"record":"event","offset":1,"at":1700000100,"kind":"invocation_admitted","body":{"kind":"invocation_admitted","invocation_id":"01ARZ3NDEKTSV4RRFFQ69G5FAW"}}"#,
            r#"{"record":"event","offset":2,"at":1700000101,"kind":"settlement_committed","body":{"kind":"settlement_committed","unit_id":"AK-900","evidence":"evd-000900","verification":"vrf-000900","reference_digest":"sha256:c4d3e2f1a0b1c2d3"}}"#,
            r#"{"record":"checkpoint","seq":2,"iteration_no":1,"event_log_offset":1,"reference_digest":"sha256:c4d3e2f1a0b1c2d3","unit_states":[["AK-900","open"]],"budget_spent":{"rounds":0,"wall_seconds":0,"tokens":0,"cost_usd":0,"diagnostic_tokens":0,"diagnostic_cost_usd":0},"in_flight":[]}"#,
            r#"{"record":"checkpoint","seq":1,"iteration_no":2,"event_log_offset":2,"reference_digest":"sha256:c4d3e2f1a0b1c2d3","unit_states":[["AK-900","in_progress"]],"budget_spent":{"rounds":0,"wall_seconds":0,"tokens":0,"cost_usd":0,"diagnostic_tokens":0,"diagnostic_cost_usd":0},"in_flight":[["01ARZ3NDEKTSV4RRFFQ69G5FAW","result-contract.json"]]}"#,
            r#"{"record":"settlement","unit_id":"AK-900","verification":"vrf-000900","evidence":"evd-000900","event_log_offset":2}"#,
            r#"{"record":"observation","seq":1,"at":1700000103,"payload":{"facet":"filesystem"}}"#,
        ]
        .join("\n");
        assert_eq!(export, expected, "the export must be the pinned golden");
    }

    /// A run recorded twice with the same identity is a no-op; with a
    /// different frozen reference it is a typed refusal (spec §5.3.3).
    #[test]
    fn recording_a_moved_reference_is_refused() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        store.record_run(&run(), bound()).expect("run records");
        store
            .record_run(&run(), bound())
            .expect("same identity is idempotent");
        let mut moved = RunDefinition {
            id: run_id(),
            created_at: EpochSeconds::new(1_700_000_000),
            reference_version: ReferenceVersion::new(
                ReferenceId::new("ref-task6").expect("valid reference id"),
                4,
                Digest::new("sha256", "0000000000000000").expect("valid digest"),
            ),
            intent: "settle the task".to_owned(),
            config_fingerprint: Digest::new("sha256", "a1b2c3d4e5f6a7b8").expect("valid digest"),
            environment_fingerprint: Digest::new("sha256", "e5f6a7b8c4d3e2f1")
                .expect("valid digest"),
            budget: run().budget().clone(),
            parent_run: None,
        };
        moved.intent = "settle the task differently".to_owned();
        let moved = Run::new(moved).expect("legal run");
        let error = store
            .record_run(&moved, bound())
            .expect_err("a run never moves its frozen reference");
        assert!(
            error.to_string().contains("frozen reference"),
            "the refusal names the invariant: {error}"
        );
    }

    /// A legacy runs row predating migration 0004 has no immutable evidence
    /// binding. Rehydration is explicitly unavailable; no synthetic ref may
    /// stand in for evidence that was never persisted.
    #[test]
    fn legacy_run_without_frozen_evidence_is_unavailable_not_rebound() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        let run = run();
        store
            .conn
            .lock()
            .expect("connection lock")
            .execute(
                "INSERT INTO runs (run_id, created_at, reference_id, reference_version, \
                 reference_digest, intent, config_fingerprint, environment_fingerprint, \
                 parent_run, budget) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)",
                (
                    run.id().to_string(),
                    run.created_at().get(),
                    run.reference_version().reference_id().as_str(),
                    u64::from(run.reference_version().version()),
                    run.reference_version().digest().to_string(),
                    run.intent(),
                    run.config_fingerprint().to_string(),
                    run.environment_fingerprint().to_string(),
                    run.parent_run().map(ToString::to_string),
                    encode::encode_budget_ceiling(run.budget()),
                ),
            )
            .expect("a legacy-shaped row inserts");

        assert!(matches!(
            store.load_run(&run_id(), bound()),
            Err(StateStoreError::Unavailable { .. })
        ));
    }

    /// A conflicting immutable evidence ref is a run-identity change, even
    /// when every former identity field agrees. The state store must reject it
    /// rather than silently retaining or overwriting either binding.
    #[test]
    fn recording_a_wrong_frozen_evidence_binding_is_refused() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        store.record_run(&run(), bound()).expect("run records");
        let wrong = run_with_evidence(
            EvidenceRef::new("evd-task6-wrong-frozen-reference").expect("valid evidence"),
        );
        let error = store
            .record_run(&wrong, bound())
            .expect_err("a run cannot exchange frozen evidence");
        assert!(matches!(error, StateStoreError::WriteFailed { .. }));
        assert!(
            error.to_string().contains("frozen reference evidence"),
            "the error identifies the immutable binding: {error}"
        );
    }

    /// Resume returns the LATEST checkpoint exactly (spec §18), and a fresh
    /// run resumes `None`.
    #[test]
    fn resume_returns_the_latest_checkpoint_exactly() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        assert_eq!(
            store.resume(&run_id(), bound()).expect("resume works"),
            None,
            "a fresh run has no checkpoint"
        );
        let first = checkpoint(1, 1, UnitStatus::Open, true);
        let second = checkpoint(2, 4, UnitStatus::InProgress, false);
        store.checkpoint(first, bound()).expect("writes");
        store.checkpoint(second.clone(), bound()).expect("writes");
        assert_eq!(
            store.resume(&run_id(), bound()).expect("resume works"),
            Some(second),
            "the latest checkpoint comes back exactly"
        );
    }

    /// Task 12 B4 (the B1-deferred composition, completed): a checkpoint
    /// written by the CONTROLLER carries charged spend (§18.2's
    /// `budget_spent`), and the owner-write authorities read their durable
    /// anchor through `resume` — so a non-zero water-mark must round-trip
    /// when (and only when) the DURABLE RUN BUDGET's own `Budget::restore`
    /// gate admits it. Reachable: exactly back. Unreachable (inside the
    /// recovery reserve): the domain's typed cause, never a silent zero.
    #[test]
    fn resume_admits_charged_checkpoints_through_the_durable_run_budget() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        store
            .record_run(&run(), bound())
            .expect("the run is durable");
        // Reachable: one round and a moderate token spend against the
        // fixture budget (rounds 8 at a fifth in reserve; tokens 1000).
        let reachable = Checkpoint {
            budget_spent: Spent::from_counters(1, 60, 100, MicroUsd::new(10), 0, MicroUsd::new(0)),
            ..checkpoint(3, 9, UnitStatus::InProgress, false)
        };
        store
            .checkpoint(reachable.clone(), bound())
            .expect("writes");
        assert_eq!(
            store.resume(&run_id(), bound()).expect("resume works"),
            Some(reachable),
            "a reachable charged water-mark round-trips exactly"
        );
        // Unreachable: tokens past the forward ceiling (four fifths of
        // 1000) — the restore gate's own refusal, naming the dimension.
        let unreachable = Checkpoint {
            budget_spent: Spent::from_counters(1, 0, 900, MicroUsd::new(0), 0, MicroUsd::new(0)),
            ..checkpoint(4, 12, UnitStatus::Open, false)
        };
        store.checkpoint(unreachable, bound()).expect("writes");
        match store.resume(&run_id(), bound()) {
            Err(StateStoreError::Corrupt { detail }) => {
                assert!(
                    detail.contains("does not admit"),
                    "the refusal names the restore gate: {detail}"
                );
            }
            other => panic!("an unreachable water-mark refuses loudly, got {other:?}"),
        }
    }

    /// The projected-run shape (checkpoints, no recorded identity) has no
    /// durable budget to admit a charged water-mark against: the loud
    /// zero-only refusal stands — never a silent zeroing.
    #[test]
    fn a_projected_run_still_refuses_charged_checkpoints_loudly() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        let charged = Checkpoint {
            budget_spent: Spent::from_counters(1, 0, 100, MicroUsd::new(0), 0, MicroUsd::new(0)),
            ..checkpoint(1, 1, UnitStatus::Open, false)
        };
        store.checkpoint(charged, bound()).expect("writes");
        match store.resume(&run_id(), bound()) {
            Err(StateStoreError::Corrupt { detail }) => {
                assert!(
                    detail.contains("no recorded run budget"),
                    "the refusal names the missing ceilings: {detail}"
                );
            }
            other => panic!("a projected run keeps the zero-only refusal, got {other:?}"),
        }
    }

    /// A payload that is not JSON is refused at the door (§21.2: model
    /// output is untrusted input — parse, validate, then admit).
    #[test]
    fn non_json_record_payloads_are_refused() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        let error = store
            .append_record(
                &run_id(),
                RecordKind::Observation,
                "not json at all",
                EpochSeconds::new(1),
                bound(),
            )
            .expect_err("garbage must not be admitted");
        assert!(
            error.to_string().contains("not valid JSON"),
            "the refusal names why: {error}"
        );
    }

    /// W7: the store names its latest run (greatest ULID) for the
    /// operator surface's context derivation; an empty store is an
    /// honest absence, never an error.
    #[test]
    fn latest_run_returns_the_greatest_recorded_run_id() {
        use gauntlet_ports::state_store::StateStore as _;
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        assert_eq!(
            store.latest_run(bound()).expect("reads"),
            None,
            "an empty store names no run"
        );
        store.record_run(&run(), bound()).expect("records");
        assert_eq!(
            store.latest_run(bound()).expect("reads"),
            Some(run_id()),
            "the only run is the latest"
        );
        // A later run sorts first regardless of recording order (ULIDs
        // sort lexicographically; 01B follows 01A).
        let later_id = RunId::new("01BSZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id");
        let later = Run::new(RunDefinition {
            id: later_id.clone(),
            created_at: EpochSeconds::new(1_700_000_100),
            reference_version: ReferenceVersion::with_evidence(
                ReferenceId::new("ref-task6").expect("valid reference id"),
                3,
                digest(),
                EvidenceRef::new("evd-task6-frozen-reference").expect("valid evidence"),
            ),
            intent: "settle the task".to_owned(),
            config_fingerprint: Digest::new("sha256", "a1b2c3d4e5f6a7b8").expect("valid digest"),
            environment_fingerprint: Digest::new("sha256", "e5f6a7b8c4d3e2f1")
                .expect("valid digest"),
            budget: Budget::new(
                Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("legal ceilings"),
                20,
                DiagnosticAllowance::new(100, MicroUsd::new(50)),
            )
            .expect("legal budget"),
            parent_run: None,
        })
        .expect("legal run");
        store.record_run(&later, bound()).expect("records");
        assert_eq!(
            store.latest_run(bound()).expect("reads"),
            Some(later_id),
            "the greatest ULID wins regardless of recording order"
        );
    }

    /// The runs row is authoritative when present; a checkpoint-only run
    /// loads with its true id and frozen digest (the projection).
    #[test]
    fn load_run_prefers_the_recorded_identity() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        store
            .checkpoint(checkpoint(1, 0, UnitStatus::Open, false), bound())
            .expect("writes");
        let projected = store
            .load_run(&run_id(), bound())
            .expect("loads")
            .expect("a checkpointed run loads");
        assert_eq!(projected.id(), &run_id());
        assert_eq!(projected.reference_version().digest(), &digest());
        assert_eq!(
            projected.intent(),
            PROJECTION_INTENT,
            "the projection names itself instead of inventing an intent"
        );
        store.record_run(&run(), bound()).expect("records");
        let recorded = store
            .load_run(&run_id(), bound())
            .expect("loads")
            .expect("the recorded run loads");
        assert_eq!(recorded.intent(), "settle the task");
        assert_eq!(recorded.reference_version().version(), 3);
    }

    /// The PASS verification document the Task-13 settlement admission    /// validates against: this run, the commit's unit, the commit's
    /// evidence.
    fn pass_record() -> gauntlet_ports::verification_record::VerificationRecordDoc {
        gauntlet_ports::verification_record::VerificationRecordDoc::new(
            gauntlet_ports::verification_record::VerificationContent {
                // Deliberately the committed settlement's own ref (COR-3:
                // the admission requires the durable record's ref to equal
                // the commit's).
                verification: VerificationRef::new("vrf-000900").expect("valid ref"),
                run_id: run_id(),
                iteration_no: IterationNo::new(1).expect("1-based"),
                unit_id: UnitId::new("AK-900").expect("valid unit id"),
                claim: "the packaged unit's claim".to_owned(),
                requirement: gauntlet_ports::verification_record::RequirementDoc::new(
                    gauntlet_domain::EvidenceKind::TestReport,
                    EvidenceLocator::new("packaged-report.json").expect("valid locator"),
                    gauntlet_domain::Producer::Instrument,
                ),
                instrument: gauntlet_ports::verification_record::VerifyingInstrument::new(
                    "packaged-verifier".to_owned(),
                    "1".to_owned(),
                    true,
                ),
                evidence: EvidenceRef::new("evd-000900").expect("valid ref"),
                produced_at: EpochSeconds::new(1_700_000_100),
                outcome: gauntlet_domain::Outcome::Pass,
                class: None,
                detail: "the fixture PASS record".to_owned(),
            },
        )
        .expect("fixture content is well formed")
    }

    /// Makes the durable substrate the admission validates against (durable
    /// run + PASS verification record naming the commit's unit and evidence)
    /// and mints the admission — the governed path since plan Task 13.
    fn earn_settlement(
        store: &std::sync::Arc<SqliteStateStore>,
    ) -> gauntlet_ports::producer_authority::AdmittedSettlement {
        store.record_run(&run(), bound()).expect("the run records");
        gauntlet_ports::producer_authority::VerificationRecordAuthority::new(store.clone())
            .record(&pass_record(), bound())
            .expect("the PASS record lands");
        gauntlet_ports::producer_authority::SettlementCommitAuthority::new(store.clone())
            .admit(commit_payload(), bound())
            .expect("the honest settlement earns its admission")
    }

    fn commit_payload() -> SettlementCommit {
        SettlementCommit {
            run_id: run_id(),
            unit_id: UnitId::new("AK-900").expect("valid unit id"),
            evidence: EvidenceRef::new("evd-000900").expect("valid ref"),
            verification: VerificationRef::new("vrf-000900").expect("valid ref"),
            reference_digest: digest(),
            at: EpochSeconds::new(1_700_000_101),
        }
    }

    /// §18.3 anchors replay at the checkpoint offset (plan Task 13, A2):
    /// events BELOW the anchor are the checkpoint's own certification — a
    /// checkpoint stands at an offset only after everything at or below it
    /// was applied — and are NOT re-scanned for completion. The half-applied
    /// window at or after the anchor is completed exactly once; a whole-log
    /// replay (anchor 0, the no-checkpoint fallback) remains available and
    /// is what repairs anything a mis-anchored history left below.
    #[test]
    fn anchored_replay_skips_events_below_the_anchor_and_completes_the_rest() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        let settlement = |unit: &str, at: u64| RunEvent {
            run_id: run_id(),
            kind: RunEventKind::SettlementCommitted {
                unit_id: UnitId::new(unit).expect("valid unit id"),
                evidence: EvidenceRef::new("evd-000900").expect("valid ref"),
                verification: VerificationRef::new("vrf-000900").expect("valid ref"),
                reference_digest: digest(),
            },
            at: EpochSeconds::new(at),
        };
        // Offset 1: an APPLIED settlement (the pre-anchor history a
        // checkpoint honestly certifies). COR3: crash-window fixtures use
        // the internal path — the public append refuses the kind.
        append_settlement_event_unchecked(&store, settlement("AK-000", 1_700_000_101))
            .expect("the applied settlement event appends");
        assert_eq!(
            store
                .complete_pending_settlements(&run_id(), EventLogOffset::new(0), bound())
                .expect("the whole-log replay applies it"),
            1
        );
        // Offset 2: a below-anchor event whose row never landed (a window a
        // later checkpoint should not have certified, but the store's
        // contract is the §18.3 anchor, not checkpoint forensics). Offset 3:
        // the post-anchor half-applied window resume exists to complete.
        append_settlement_event_unchecked(&store, settlement("AK-001", 1_700_000_102))
            .expect("the below-anchor event appends");
        append_settlement_event_unchecked(&store, settlement("AK-002", 1_700_000_103))
            .expect("the post-anchor event appends");
        // The §18.3 anchor: replay starts at the checkpoint offset (here 3,
        // standing for a checkpoint written after offset 3).
        assert_eq!(
            store
                .complete_pending_settlements(&run_id(), EventLogOffset::new(3), bound())
                .expect("the anchored replay runs"),
            1,
            "exactly the post-anchor half-applied settlement is completed; the \
             below-anchor event is not re-scanned"
        );
        let rows = store.settlements(&run_id(), bound()).expect("reads");
        let units: Vec<&str> = rows.iter().map(|row| row.unit_id().as_str()).collect();
        assert_eq!(
            units,
            ["AK-000", "AK-002"],
            "the below-anchor row is absent"
        );
        // The second anchored call adds nothing (keyed idempotency).
        assert_eq!(
            store
                .complete_pending_settlements(&run_id(), EventLogOffset::new(3), bound())
                .expect("the anchored replay runs"),
            0
        );
        // The whole-log replay stays available and repairs the below-anchor
        // window — the superset behavior the crash-recovery tests pin.
        assert_eq!(
            store
                .complete_pending_settlements(&run_id(), EventLogOffset::new(0), bound())
                .expect("the whole-log replay runs"),
            1,
            "the whole-log fallback completes what the anchored scan skipped"
        );
        assert_eq!(
            store.settlements(&run_id(), bound()).expect("reads").len(),
            3
        );
    }

    /// A3 (plan Task 13): `settled_at` is EVENT-DERIVED. The commit's
    /// caller-supplied `at` constructs the event only; the row takes the
    /// durable event's STORED timestamp, and a commit whose constructed
    /// timestamp disagrees with what is stored at the offset is refused —
    /// the previously unchecked seam between the caller's clock claim and
    /// the durable history.
    #[test]
    fn a_commit_timestamp_disagreeing_with_the_durable_event_is_refused() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        let at_of_event = EpochSeconds::new(1_700_000_101);
        // COR3: the crash-window fixture lands through the internal path.
        let offset = append_settlement_event_unchecked(
            &store,
            RunEvent {
                run_id: run_id(),
                kind: RunEventKind::SettlementCommitted {
                    unit_id: UnitId::new("AK-900").expect("valid unit id"),
                    evidence: EvidenceRef::new("evd-000900").expect("valid ref"),
                    verification: VerificationRef::new("vrf-000900").expect("valid ref"),
                    reference_digest: digest(),
                },
                at: at_of_event,
            },
        )
        .expect("the settlement event appends durably at 1_700_000_101");
        // The disagreeing commit: same fields, a DIFFERENT `at`.
        let mut disagreeing = commit_payload();
        disagreeing.at = EpochSeconds::new(1_700_000_999);
        match store.apply_settlement_row(disagreeing, offset, bound()) {
            Err(StateStoreError::WriteFailed { detail }) => {
                assert!(
                    detail.contains("timestamp") && detail.contains("1700000999"),
                    "the refusal names the disagreement and both instants: {detail}"
                );
            }
            other => panic!(
                "a commit timestamp that disagrees with the durable event must be refused, \
                 got {other:?}"
            ),
        }
        assert!(
            store
                .settlements(&run_id(), bound())
                .expect("reads")
                .is_empty(),
            "the refused settlement row must not exist"
        );
        // The agreeing commit applies, and the row's settled_at IS the
        // event's stored timestamp.
        let agreeing = commit_payload();
        assert_eq!(agreeing.at, at_of_event);
        assert_eq!(
            store
                .apply_settlement_row(agreeing, offset, bound())
                .expect("the agreeing row applies"),
            1
        );
        let settled_at: i64 = store
            .conn
            .lock()
            .expect("the store connection is not poisoned")
            .query_row(
                "SELECT settled_at FROM settlements WHERE unit_id = 'AK-900'",
                [],
                |row| row.get(0),
            )
            .expect("the settled_at column reads");
        assert_eq!(
            u64::try_from(settled_at),
            Ok(at_of_event.get()),
            "settled_at is the durable event's stored timestamp"
        );
    }

    // -----------------------------------------------------------------------
    // COR-1 (Task 13 correction round 1): the three §16.3-step-2-alone
    // cross-validation legs moved IN-CRATE from tests/tamper_evidence.rs —
    // the raw row-apply primitive is `#[cfg(test)]` since the correction
    // (the early probe's C1 aggravator closed structurally), and in-crate
    // tests are its established visibility. Assertions verbatim.
    // -----------------------------------------------------------------------

    fn admitted_event(at: u64) -> RunEvent {
        RunEvent {
            run_id: run_id(),
            kind: RunEventKind::InvocationAdmitted {
                invocation_id: InvocationId::new(INVOCATION).expect("valid invocation id"),
            },
            at: EpochSeconds::new(at),
        }
    }

    fn settlement_event(at: u64) -> RunEvent {
        RunEvent {
            run_id: run_id(),
            kind: RunEventKind::SettlementCommitted {
                unit_id: UnitId::new("AK-900").expect("valid unit id"),
                evidence: EvidenceRef::new("evd-000900").expect("valid ref"),
                verification: VerificationRef::new("vrf-000900").expect("valid ref"),
                reference_digest: digest(),
            },
            at: EpochSeconds::new(at),
        }
    }

    /// `apply_settlement_row` with an offset that holds a DIFFERENT event is a
    /// typed refusal naming what was claimed and what is there; a settlement
    /// row must name its own durable settlement event (spec §5.3.6).
    #[test]
    fn a_settlement_row_pointing_at_a_foreign_event_is_refused() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        let offset = store
            .append_event(admitted_event(1_700_000_000), bound())
            .expect("the admitted event appends at offset 1");
        assert_eq!(offset.get(), 1);
        match store.apply_settlement_row(commit_payload(), offset, bound()) {
            Err(StateStoreError::WriteFailed { detail }) => {
                assert!(
                    detail.contains("offset 1") && detail.contains("invocation_admitted"),
                    "the refusal names the claimed offset and the event actually there: {detail}"
                );
            }
            other => panic!("a foreign offset must be a typed refusal, got {other:?}"),
        }
        // Nothing landed: the row does not exist.
        assert!(
            store
                .settlements(&run_id(), bound())
                .expect("reads")
                .is_empty(),
            "the refused settlement row must not exist"
        );
    }

    /// `apply_settlement_row` with an offset BEYOND the log is a typed refusal
    /// naming the missing row.
    #[test]
    fn a_settlement_row_pointing_past_the_log_is_refused() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        match store.apply_settlement_row(commit_payload(), EventLogOffset::new(99), bound()) {
            Err(StateStoreError::WriteFailed { detail }) => {
                assert!(
                    detail.contains("99") && detail.contains("no row"),
                    "the refusal names the missing offset: {detail}"
                );
            }
            other => panic!("a missing offset must be a typed refusal, got {other:?}"),
        }
        assert!(
            store
                .settlements(&run_id(), bound())
                .expect("reads")
                .is_empty(),
            "the refused settlement row must not exist"
        );
    }

    /// The happy path still works: applying a row at the offset of ITS OWN
    /// settlement event lands exactly once.
    #[test]
    fn a_settlement_row_at_its_own_event_offset_lands() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        let offset = append_settlement_event_unchecked(&store, settlement_event(1_700_000_101))
            .expect("the settlement event appends");
        let added = store
            .apply_settlement_row(commit_payload(), offset, bound())
            .expect("the row applies at its own event's offset");
        assert_eq!(added, 1);
        let rows = store.settlements(&run_id(), bound()).expect("reads");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].event_log_offset(), offset);
    }

    /// A5 (plan Task 13): the `EvidenceProduced` crash window — event
    /// durable, row missing — is completed exactly once, bound to the
    /// ORPHAN event's offset (no second event appended), with `produced_at`
    /// from the event's own stored timestamp; an already-applied production
    /// completes nothing; a mint whose locator disagrees with the orphan
    /// event is refused (the orphan belongs to a different production).
    #[test]
    fn a_half_applied_evidence_production_is_completed_exactly_once() {
        let store = std::sync::Arc::new(SqliteStateStore::open_in_memory().expect("store opens"));
        store.record_run(&run(), bound()).expect("run records");
        let evidence = std::sync::Arc::new(SwappableEvidence::default());
        evidence.insert("evd-orphan-0001", b"the orphaned production bytes");
        // The durable half alone: the event exists, the row never landed
        // (the crash window Task 11's error string names).
        let orphan_at = EpochSeconds::new(1_700_000_201);
        let orphan_offset = store
            .append_event(
                RunEvent {
                    run_id: run_id(),
                    kind: RunEventKind::EvidenceProduced {
                        evidence: EvidenceRef::new("evd-orphan-0001").expect("valid ref"),
                        locator: EvidenceLocator::new("tests/proofs/x").expect("valid locator"),
                    },
                    at: orphan_at,
                },
                bound(),
            )
            .expect("the EvidenceProduced event is durable");
        // The durable iteration anchor the mint validates against.
        store
            .checkpoint(
                checkpoint(2, orphan_offset.get(), UnitStatus::Open, false),
                bound(),
            )
            .expect("the anchor checkpoint lands");
        // The mint: governed reads, derived digest, NO write.
        let minted = gauntlet_ports::producer_authority::EvidenceProductionAuthority::new(
            store.clone(),
            evidence.clone(),
        )
        .mint(
            &run_id(),
            IterationNo::new(2).expect("1-based"),
            &EvidenceRef::new("evd-orphan-0001").expect("valid ref"),
            &gauntlet_ports::observer::InstrumentIdentity::new(
                InstrumentName::new("gauntlet-verify-test-report").expect("valid instrument"),
                "1".to_owned(),
            ),
            &EvidenceLocator::new("tests/proofs/x").expect("valid locator"),
            EpochSeconds::new(1_700_000_400),
            bound(),
        )
        .expect("the mint earns over the honest bytes");
        // Nothing was recorded by the mint alone.
        assert!(
            store
                .evidence_production(
                    &run_id(),
                    &EvidenceRef::new("evd-orphan-0001").unwrap(),
                    bound(),
                )
                .expect("reads")
                .is_none(),
            "the mint writes nothing"
        );
        // A locator-disagreeing mint is refused WHILE THE ROW IS MISSING
        // (once applied, completion is a no-op): the orphan event belongs
        // to a different production of the same ref.
        let foreign = gauntlet_ports::producer_authority::EvidenceProductionAuthority::new(
            store.clone(),
            evidence.clone(),
        )
        .mint(
            &run_id(),
            IterationNo::new(2).expect("1-based"),
            &EvidenceRef::new("evd-orphan-0001").expect("valid ref"),
            &gauntlet_ports::observer::InstrumentIdentity::new(
                InstrumentName::new("gauntlet-verify-test-report").expect("valid instrument"),
                "1".to_owned(),
            ),
            &EvidenceLocator::new("tests/proofs/other").expect("valid locator"),
            EpochSeconds::new(1_700_000_400),
            bound(),
        )
        .expect("the mint earns over the same bytes");
        assert!(matches!(
            store.complete_pending_evidence_production(
                &run_id(),
                EventLogOffset::new(0),
                &foreign,
                bound(),
            ),
            Err(StateStoreError::WriteFailed { .. })
        ));
        // Completion binds the row to the orphan event — exactly once.
        let completed = store
            .complete_pending_evidence_production(
                &run_id(),
                EventLogOffset::new(0),
                &minted,
                bound(),
            )
            .expect("completion runs")
            .expect("the half-applied production completes");
        assert_eq!(
            completed.event_log_offset(),
            orphan_offset,
            "the row names the durable event that made it real — no second event"
        );
        assert_eq!(
            completed.production().produced_at(),
            orphan_at,
            "produced_at is the event's stored timestamp, not the re-mint's clock"
        );
        // No new event was appended by completion either.
        {
            let conn = store.conn.lock().expect("connection lock");
            let entries = read_entries(&conn, EventLogOffset::new(0)).expect("replays");
            assert_eq!(
                entries.len(),
                1,
                "completion appends nothing; the orphan event remains the only one"
            );
        }
        // The durable row reads back exactly as completed.
        assert_eq!(
            store
                .evidence_production(
                    &run_id(),
                    &EvidenceRef::new("evd-orphan-0001").unwrap(),
                    bound(),
                )
                .expect("reads")
                .expect("the completed row stands"),
            completed
        );
        // Second completion: already applied → nothing.
        assert!(
            store
                .complete_pending_evidence_production(
                    &run_id(),
                    EventLogOffset::new(0),
                    &minted,
                    bound(),
                )
                .expect("completion runs")
                .is_none(),
            "an already-applied production completes nothing"
        );
    }

    /// A fully-applied production (the normal earn path) has no window:
    /// completion over it adds nothing and returns `None`.
    #[test]
    fn a_fully_applied_evidence_production_completes_nothing() {
        let store = std::sync::Arc::new(SqliteStateStore::open_in_memory().expect("store opens"));
        store.record_run(&run(), bound()).expect("run records");
        let evidence = std::sync::Arc::new(SwappableEvidence::default());
        let binding = earn_production(
            &store,
            &evidence,
            "evd-complete-0001",
            "gauntlet-verify-test-report",
            "tests/proofs/x",
            1,
            b"the fully applied bytes",
        );
        // Re-mint the SAME facts (idempotent governed reads).
        let minted = gauntlet_ports::producer_authority::EvidenceProductionAuthority::new(
            store.clone(),
            evidence.clone(),
        )
        .mint(
            &run_id(),
            IterationNo::new(1).expect("1-based"),
            &EvidenceRef::new("evd-complete-0001").expect("valid ref"),
            &gauntlet_ports::observer::InstrumentIdentity::new(
                InstrumentName::new("gauntlet-verify-test-report").expect("valid instrument"),
                "1".to_owned(),
            ),
            &EvidenceLocator::new("tests/proofs/x").expect("valid locator"),
            EpochSeconds::new(1_700_000_200),
            bound(),
        )
        .expect("the same facts re-mint");
        assert_eq!(minted.production(), binding.production());
        assert!(
            store
                .complete_pending_evidence_production(
                    &run_id(),
                    EventLogOffset::new(0),
                    &minted,
                    bound(),
                )
                .expect("completion runs")
                .is_none(),
            "an already-applied production completes nothing"
        );
    }

    /// The PACKAGED §16.3 primitive, reached the only way it can be reached
    /// since plan Task 13 — through an EARNED admission (the correction
    /// round's reopening finding gave this method zero callers; Task 13
    /// closes the ungated shape and keeps the packaged path pinned). Pins
    /// the whole contract at the API that ships: the returned offset is the
    /// settlement row's offset; the event at that offset is exactly the
    /// commit's own settlement event; replay afterwards adds nothing; a
    /// second ADMISSION of the committed key is refused (re-committing
    /// would append a duplicate event — the store's own error calls that
    /// wrong); and a raw duplicate EVENT for the same key leaves the keyed
    /// row count at one.
    #[test]
    fn commit_settlement_is_the_spec_sequence_and_idempotent_by_key() {
        use crate::event_log::read_entries;
        let store = std::sync::Arc::new(SqliteStateStore::open_in_memory().expect("store opens"));
        let admitted = earn_settlement(&store);
        let commit = admitted.commit().clone();
        let offset = store
            .commit_settlement(&admitted, bound())
            .expect("the packaged commit sequence runs");

        // The settlement row references exactly the returned offset.
        let rows = store.settlements(&run_id(), bound()).expect("reads");
        assert_eq!(rows.len(), 1, "exactly one settlement row");
        assert_eq!(
            rows[0].event_log_offset(),
            offset,
            "the row names the offset the commit returned"
        );

        // The event at that offset decodes to SettlementCommitted with
        // exactly the commit's fields.
        {
            let conn = store
                .conn
                .lock()
                .expect("the store connection is not poisoned");
            let entries = read_entries(&conn, EventLogOffset::new(0)).expect("the log replays");
            assert_eq!(entries.len(), 1, "one event, at the returned offset");
            assert_eq!(entries[0].offset, offset);
            assert_eq!(entries[0].event.run_id, commit.run_id);
            assert_eq!(entries[0].event.at, commit.at);
            match &entries[0].event.kind {
                RunEventKind::SettlementCommitted {
                    unit_id,
                    evidence,
                    verification,
                    reference_digest,
                } => {
                    assert_eq!(unit_id, &commit.unit_id);
                    assert_eq!(evidence, &commit.evidence);
                    assert_eq!(verification, &commit.verification);
                    assert_eq!(reference_digest, &commit.reference_digest);
                }
                other => panic!("the event must be a settlement, got {other:?}"),
            }
        }

        // Nothing is half-applied: replay adds zero.
        assert_eq!(
            store
                .complete_pending_settlements(&run_id(), EventLogOffset::new(0), bound())
                .expect("replay works"),
            0,
            "a completed commit needs no replay"
        );

        // A second admission of the committed key is refused at the gate
        // (re-committing would append a duplicate settlement event; the
        // settled work stands).
        assert!(matches!(
            gauntlet_ports::producer_authority::SettlementCommitAuthority::new(store.clone())
                .admit(commit, bound()),
            Err(
                gauntlet_ports::producer_authority::SettlementAdmissionError::AlreadyCommitted { .. }
            )
        ));

        // And the keyed row still names one settlement even if a duplicate
        // EVENT for the same key lands raw: the log is append-only, the row
        // is keyed, and replay of the duplicate adds nothing.
        append_settlement_event_unchecked(
            &store,
            RunEvent {
                run_id: run_id(),
                kind: RunEventKind::SettlementCommitted {
                    unit_id: UnitId::new("AK-900").expect("valid unit id"),
                    evidence: EvidenceRef::new("evd-000900").expect("valid ref"),
                    verification: VerificationRef::new("vrf-000900").expect("valid ref"),
                    reference_digest: digest(),
                },
                at: EpochSeconds::new(1_700_000_101),
            },
        )
        .expect("the raw duplicate event appends");
        assert_eq!(
            store.settlements(&run_id(), bound()).expect("reads").len(),
            1,
            "the (run, unit, verification) key still names one row"
        );
        assert_eq!(
            store
                .complete_pending_settlements(&run_id(), EventLogOffset::new(0), bound())
                .expect("replay works"),
            0,
            "replay after the duplicate-key event adds nothing"
        );
    }

    // -----------------------------------------------------------------------
    // Evidence-production bindings (plan Task 11; spec 14.3) — earned
    // through the ports authority since Task 12's capability gate
    // -----------------------------------------------------------------------

    /// A test evidence store with REAL content semantics plus one power a
    /// real store does not have: the bytes at a ref can be swapped after
    /// the fact (`replace`). That swap is exactly the after-the-fact tamper
    /// the authority's derived-from-bytes digest must catch — the adapter's
    /// tests exercise it through the ports authority the way production
    /// does, since direct construction of a production record is no longer
    /// representable (plan Task 12).
    #[derive(Default)]
    struct SwappableEvidence {
        objects: std::sync::Mutex<Vec<(EvidenceRef, Vec<u8>)>>,
    }

    impl SwappableEvidence {
        fn insert(&self, name: &str, bytes: &[u8]) -> EvidenceRef {
            let reference = EvidenceRef::new(name).expect("valid evidence ref");
            self.objects
                .lock()
                .expect("not poisoned")
                .push((reference.clone(), bytes.to_vec()));
            reference
        }

        fn replace(&self, name: &str, bytes: &[u8]) {
            let reference = EvidenceRef::new(name).expect("valid evidence ref");
            let mut objects = self.objects.lock().expect("not poisoned");
            if let Some(entry) = objects.iter_mut().find(|(found, _)| *found == reference) {
                entry.1 = bytes.to_vec();
            }
        }
    }

    impl gauntlet_ports::evidence_store::EvidenceStore for SwappableEvidence {
        fn put(
            &self,
            bytes: &[u8],
            _: gauntlet_domain::EvidenceKind,
            _: gauntlet_ports::deadline::Deadline,
        ) -> Result<EvidenceRef, gauntlet_ports::evidence_store::EvidenceStoreError> {
            let index = self.objects.lock().expect("not poisoned").len();
            Ok(self.insert(&format!("evd-adapter-test-{index:04}"), bytes))
        }
        fn get(
            &self,
            reference: &EvidenceRef,
            _: gauntlet_ports::deadline::Deadline,
        ) -> Result<Option<Vec<u8>>, gauntlet_ports::evidence_store::EvidenceStoreError> {
            Ok(self
                .objects
                .lock()
                .expect("not poisoned")
                .iter()
                .find(|(found, _)| found == reference)
                .map(|(_, bytes)| bytes.clone()))
        }
        fn pin(
            &self,
            _: &RunId,
            _: &EvidenceRef,
            _: gauntlet_ports::deadline::Deadline,
        ) -> Result<(), gauntlet_ports::evidence_store::EvidenceStoreError> {
            Ok(())
        }
    }

    /// The evidence owner's EARNED write (plan Task 12): the bytes are
    /// placed at a named ref, the durable anchor stands at the declared
    /// iteration (a checkpoint, the way the controller does), and the ports
    /// authority derives the content digest from the bytes and mints the
    /// record this store accepts. There is no direct construction path left.
    fn earn_production(
        store: &std::sync::Arc<SqliteStateStore>,
        evidence: &std::sync::Arc<SwappableEvidence>,
        evidence_name: &str,
        producer: &str,
        locator: &str,
        iteration: u32,
        bytes: &[u8],
    ) -> gauntlet_ports::state_store::EvidenceProductionBinding {
        evidence.insert(evidence_name, bytes);
        store
            .checkpoint(checkpoint(iteration, 0, UnitStatus::Open, false), bound())
            .expect("the durable anchor checkpoint lands");
        gauntlet_ports::producer_authority::EvidenceProductionAuthority::new(
            store.clone(),
            evidence.clone(),
        )
        .record(
            &run_id(),
            IterationNo::new(iteration).expect("1-based"),
            &EvidenceRef::new(evidence_name).expect("valid evidence ref"),
            &gauntlet_ports::observer::InstrumentIdentity::new(
                InstrumentName::new(producer).expect("valid instrument name"),
                "1".to_owned(),
            ),
            &EvidenceLocator::new(locator).expect("valid locator"),
            EpochSeconds::new(1_700_000_200),
            bound(),
        )
        .expect("the authority mints and the store records")
    }

    /// A re-earn attempt over the SAME ref through the same authority —
    /// used by the write-once tests to reach the store's refusal with an
    /// honestly-minted conflicting record.
    fn earn_production_result(
        store: &std::sync::Arc<SqliteStateStore>,
        evidence: &std::sync::Arc<SwappableEvidence>,
        evidence_name: &str,
        producer: &str,
        locator: &str,
        iteration: u32,
    ) -> Result<
        gauntlet_ports::state_store::EvidenceProductionBinding,
        gauntlet_ports::producer_authority::ProducerAuthorityError,
    > {
        gauntlet_ports::producer_authority::EvidenceProductionAuthority::new(
            store.clone(),
            evidence.clone(),
        )
        .record(
            &run_id(),
            IterationNo::new(iteration).expect("1-based"),
            &EvidenceRef::new(evidence_name).expect("valid evidence ref"),
            &gauntlet_ports::observer::InstrumentIdentity::new(
                InstrumentName::new(producer).expect("valid instrument name"),
                "1".to_owned(),
            ),
            &EvidenceLocator::new(locator).expect("valid locator"),
            EpochSeconds::new(1_700_000_200),
            bound(),
        )
    }

    /// A valid `sha256:…`-shaped digest of the given bytes through the
    /// ports' own derivation rule (the digest the authority derives).
    fn content_digest(bytes: &[u8]) -> gauntlet_domain::Digest {
        gauntlet_ports::producer_authority::evidence_content_digest(bytes)
    }

    #[test]
    fn evidence_production_records_are_write_once_and_idempotent() {
        let store = std::sync::Arc::new(SqliteStateStore::open_in_memory().expect("store opens"));
        store.record_run(&run(), bound()).expect("run records");
        let evidence = std::sync::Arc::new(SwappableEvidence::default());
        let bound_record = earn_production(
            &store,
            &evidence,
            "evd-report-0001",
            "gauntlet-verify-test-report",
            "tests/proofs/x",
            1,
            b"the report bytes",
        );
        // The anchor is server-minted: a real offset, not a caller value.
        assert!(bound_record.event_log_offset().get() > 0);
        // Re-recording the SAME facts (same ref, same bytes, same digest) is
        // idempotent: the ORIGINAL binding (original anchor) returns.
        let again = earn_production_result(
            &store,
            &evidence,
            "evd-report-0001",
            "gauntlet-verify-test-report",
            "tests/proofs/x",
            1,
        )
        .expect("identical re-record is idempotent");
        assert_eq!(again, bound_record);
        // A CONFLICTING record for the same ref (different producer) is
        // minted honestly and refused by the STORE, never absorbed.
        assert!(matches!(
            earn_production_result(
                &store,
                &evidence,
                "evd-report-0001",
                "gauntlet-observe-test",
                "tests/proofs/x",
                1,
            ),
            Err(
                gauntlet_ports::producer_authority::ProducerAuthorityError::State(
                    StateStoreError::WriteFailed { .. }
                )
            )
        ));
        // And the stored record is still the original.
        let stored = store
            .evidence_production(
                &run_id(),
                &EvidenceRef::new("evd-report-0001").unwrap(),
                bound(),
            )
            .expect("the lookup succeeds")
            .expect("the original record stands");
        assert_eq!(stored, bound_record);
    }

    #[test]
    fn the_production_content_digest_is_durable_and_write_once() {
        // Correction round 1 (B-M4) under the Task-12 gate: the production
        // record carries the digest DERIVED from the bytes the store serves;
        // it persists, round-trips through the strict decoder, and a record
        // minted over swapped bytes (a different derived digest for the same
        // ref) is refused.
        let store = std::sync::Arc::new(SqliteStateStore::open_in_memory().expect("store opens"));
        store.record_run(&run(), bound()).expect("run records");
        let evidence = std::sync::Arc::new(SwappableEvidence::default());
        let bound_record = earn_production(
            &store,
            &evidence,
            "evd-content-0001",
            "gauntlet-verify-test-report",
            "tests/proofs/x",
            2,
            b"the original content bytes",
        );
        assert_eq!(
            bound_record.production().content(),
            &content_digest(b"the original content bytes")
        );
        let stored = store
            .evidence_production(
                &run_id(),
                &EvidenceRef::new("evd-content-0001").unwrap(),
                bound(),
            )
            .expect("the lookup succeeds")
            .expect("the record stands");
        assert_eq!(stored, bound_record, "the content digest round-trips");
        // The tamper: the bytes at the SAME ref are swapped. The authority
        // derives the digest of the SWAPPED bytes, so the minted record
        // names a different content for the same ref — and the store's
        // write-once rule refuses it. The doctored digest cannot ride.
        evidence.replace("evd-content-0001", b"the doctored content bytes");
        assert!(matches!(
            earn_production_result(
                &store,
                &evidence,
                "evd-content-0001",
                "gauntlet-verify-test-report",
                "tests/proofs/x",
                2,
            ),
            Err(
                gauntlet_ports::producer_authority::ProducerAuthorityError::State(
                    StateStoreError::WriteFailed { .. }
                )
            )
        ));
        // And the durable record still names the ORIGINAL content.
        assert_eq!(
            store
                .evidence_production(
                    &run_id(),
                    &EvidenceRef::new("evd-content-0001").unwrap(),
                    bound()
                )
                .expect("the lookup succeeds")
                .expect("the record stands")
                .production()
                .content(),
            &content_digest(b"the original content bytes")
        );
    }

    #[test]
    fn productions_at_a_locator_are_ordered_by_their_anchors() {
        let store = std::sync::Arc::new(SqliteStateStore::open_in_memory().expect("store opens"));
        store.record_run(&run(), bound()).expect("run records");
        let evidence = std::sync::Arc::new(SwappableEvidence::default());
        // A prior loop pass's production (iteration 1), then the run crosses
        // to iteration 3 and produces again — the anchor ordering is by
        // event-log offset, not by iteration number.
        let first = earn_production(
            &store,
            &evidence,
            "evd-report-old",
            "gauntlet-verify-test-report",
            "tests/proofs/x",
            1,
            b"older report bytes",
        );
        let second = earn_production(
            &store,
            &evidence,
            "evd-report-new",
            "gauntlet-verify-test-report",
            "tests/proofs/x",
            3,
            b"newer report bytes",
        );
        assert!(
            second.event_log_offset().get() > first.event_log_offset().get(),
            "the second production's anchor is minted after the first"
        );
        let locator = EvidenceLocator::new("tests/proofs/x").expect("valid locator");
        let found = store
            .evidence_productions_at(&run_id(), &locator, bound())
            .expect("the locator query succeeds");
        assert_eq!(found.len(), 2, "both refs recorded at one locator stand");
        assert_eq!(found[0], first, "anchor order, oldest first");
        assert_eq!(found[1], second);
        // A locator with no productions is an empty vector, never an error.
        let absent = EvidenceLocator::new("tests/proofs/none").expect("valid locator");
        assert!(
            store
                .evidence_productions_at(&run_id(), &absent, bound())
                .expect("the locator query succeeds")
                .is_empty()
        );
        // Lookups are run-keyed: another run sees none of this run's records.
        let other = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAB").expect("valid run id");
        assert!(
            store
                .evidence_production(
                    &other,
                    &EvidenceRef::new("evd-report-new").unwrap(),
                    bound()
                )
                .expect("the lookup succeeds")
                .is_none()
        );
    }

    // -----------------------------------------------------------------------
    // Correction selections (plan Task 12, spec 15.1 step 4, AK-037)
    // -----------------------------------------------------------------------

    fn selection() -> CorrectionSelection {
        let facts = gauntlet_domain::BudgetFacts {
            requested_rounds: 1,
            requested_wall_seconds: 60,
            requested_tokens: 400,
            requested_cost_usd: MicroUsd::new(5),
            remaining_rounds: 6,
            remaining_wall_seconds: 3000,
            remaining_tokens: 400,
            remaining_cost_usd: MicroUsd::new(395),
        };
        let descriptor = |summary: &str, shape| {
            CorrectionDescriptor::new(
                summary.to_owned(),
                shape,
                vec![UnitId::new("AK-914").expect("valid unit id")],
                facts,
            )
            .expect("complete descriptor")
        };
        CorrectionSelection::new(
            run_id(),
            IterationNo::new(3).expect("1-based"),
            descriptor("bound the parser branch", CorrectionShape::Bounded),
            vec![
                RejectedAlternative::new(
                    descriptor("rewrite everything", CorrectionShape::Speculative),
                    RejectionClass::Budget,
                    RejectionReason::MeterRefusedReserve { facts },
                ),
                RejectedAlternative::new(
                    descriptor("neighboring tweak", CorrectionShape::Bounded),
                    RejectionClass::Authority,
                    RejectionReason::AuthorityAboveCeiling {
                        required: Authority::Human,
                        exercisable: Authority::LeadAgent,
                    },
                ),
            ],
            gauntlet_domain::SelectionDimension::ALL
                .iter()
                .map(|dimension| {
                    DecisionStep::new(*dimension, format!("adapter-test fact: {dimension}"))
                })
                .collect(),
        )
        .expect("the selection satisfies the aggregate")
    }

    /// The durable round trip (AK-037): the chosen correction, both
    /// rejected alternatives with class and typed reason, and the full
    /// trace read back exactly, in record order; a damaged row is Corrupt.
    #[test]
    fn correction_selections_round_trip_with_alternatives_and_reasons() {
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        let chosen = selection();
        // A second, later selection of the same run: record order holds.
        let later = selection();
        store
            .record_correction_selection(&chosen, EpochSeconds::new(0), bound())
            .expect("the selection records");
        store
            .record_correction_selection(&later, EpochSeconds::new(0), bound())
            .expect("the second selection records");
        let read = store
            .correction_selections(&run_id(), bound())
            .expect("reads back");
        assert_eq!(read.len(), 2, "append order, both rows");
        assert_eq!(read[0], chosen, "the exact aggregate returns");
        assert_eq!(read[0].rejected().len(), 2);
        assert_eq!(read[0].rejected()[0].class(), RejectionClass::Budget);
        assert!(matches!(
            read[0].rejected()[0].reason(),
            RejectionReason::MeterRefusedReserve { .. }
        ));
        assert_eq!(read[1], later);
        // Run-keyed: another run reads nothing.
        let other = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAB").expect("valid run id");
        assert!(
            store
                .correction_selections(&other, bound())
                .expect("reads")
                .is_empty()
        );
        // A damaged payload names itself Corrupt, never degrades.
        store
            .append_record(
                &run_id(),
                RecordKind::CorrectionSelection,
                r#"{"run_id":"01ARZ3NDEKTSV4RRFFQ69G5FAV"}"#,
                EpochSeconds::new(0),
                bound(),
            )
            .expect("the damaged row inserts (the write side is generic JSON)");
        assert!(matches!(
            store.correction_selections(&run_id(), bound()),
            Err(StateStoreError::Corrupt { .. })
        ));
        // The migration actually widened the CHECK: the new kind inserts.
        // (A pre-0008 database would refuse it — the 0008 rebuild is what
        // admits this row; pinned again by the migration version test.)
        let count: i64 = store
            .conn
            .lock()
            .expect("connection lock")
            .query_row(
                "SELECT COUNT(*) FROM records WHERE record_type = 'correction_selection'",
                [],
                |row| row.get(0),
            )
            .expect("counts");
        assert_eq!(count, 3);
    }

    /// W4 (correction task26-dcritic-apparatus-repair): a selection
    /// recorded at clock instant T stores T on its own `records` row —
    /// never a fixed `EpochSeconds::new(0)` regardless of when it was
    /// really recorded.
    #[test]
    fn a_correction_selection_stores_the_callers_real_clock_instant() {
        const RECORDED_AT: u64 = 1_700_000_555;
        let store = SqliteStateStore::open_in_memory().expect("store opens");
        store
            .record_correction_selection(&selection(), EpochSeconds::new(RECORDED_AT), bound())
            .expect("the selection records");
        let at: i64 = store
            .conn
            .lock()
            .expect("connection lock")
            .query_row(
                "SELECT at FROM records WHERE record_type = 'correction_selection'",
                [],
                |row| row.get(0),
            )
            .expect("reads the row's own at column");
        assert_eq!(
            at as u64, RECORDED_AT,
            "the row's at column carries the caller's real clock instant, not a fixed zero"
        );
    }

    #[test]
    fn the_production_event_replays_through_the_event_log() {
        let store = std::sync::Arc::new(SqliteStateStore::open_in_memory().expect("store opens"));
        store.record_run(&run(), bound()).expect("run records");
        let evidence = std::sync::Arc::new(SwappableEvidence::default());
        let binding = earn_production(
            &store,
            &evidence,
            "evd-report-0001",
            "gauntlet-verify-test-report",
            "tests/proofs/x",
            2,
            b"the replayed report bytes",
        );
        let entries = read_entries(
            &store.conn.lock().expect("connection lock"),
            EventLogOffset::new(0),
        )
        .expect("the log replays");
        let produced = entries
            .iter()
            .find(|entry| matches!(entry.event.kind, RunEventKind::EvidenceProduced { .. }));
        let produced = produced.expect("the EvidenceProduced event is durable");
        assert_eq!(
            produced.event.kind,
            RunEventKind::EvidenceProduced {
                evidence: EvidenceRef::new("evd-report-0001").expect("valid ref"),
                locator: EvidenceLocator::new("tests/proofs/x").expect("valid locator"),
            }
        );
        // The binding's anchor names exactly that event's offset — the
        // ordering fact and the durable history are one fact, not two.
        assert_eq!(binding.event_log_offset(), produced.offset);
    }
}
