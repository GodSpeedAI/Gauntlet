//! `RevisionLedger` — spec §19.3/§19.4's durable candidate-revision store
//! (plan Task 23, charter D2). The §19.4 gate promotes REVISIONS from
//! evaluation evidence; this port holds those revisions. It is the
//! capability ledger's house discipline over a different record:
//! transition-derived decision state (NO mutable decision column), typed
//! per-condition refusals, and insufficient-evidence-refuses.
//!
//! The port durably holds three append-only record classes:
//!
//! - **Proposals** ([`RevisionLedger::admit_proposal`]): the admitted
//!   `candidate_revision@v1` declaration plus its admission provenance.
//!   Idempotent on the declared [`RevisionId`] — the same proposal
//!   re-admitted with agreeing facts is a no-op; a re-admission that
//!   DISAGREES is the typed [`RevisionLedgerError::ProposalConflict`],
//!   never a silent overwrite (the `record_run`/observation-identity
//!   precedent).
//! - **Decision transitions** ([`RevisionLedger::apply_decision`]): the
//!   §19.4 gate's durable verdicts — promoted-with-decision-record /
//!   rejected-with-reasons / provisional-with-reasons — as first-class
//!   [`RevisionDecisionRecord`]s. The CURRENT decision is DERIVED from the
//!   latest applied record; there is deliberately no set-decision method.
//!   A decision past a final state (promoted, rejected) is refused — the
//!   domain record's write-once floor, re-derived here.
//! - **Evaluation records** ([`RevisionLedger::append_evaluation`]) and
//!   **restore records** ([`RevisionLedger::record_restore`]): the
//!   isolated-evaluation evidence a verdict rests on, and the append-only
//!   rollback/restore history. A promoted revision that was later
//!   restored reads as promoted-BUT-ROLLED-BACK
//!   ([`RevisionEntry::rolled_back`]) — never silently active.
//!
//! Every method takes a [`Deadline`] (spec §6 preamble). JSON codecs are
//! hand-rolled over [`crate::json`] (the workspace has no serde); decoding
//! is strict, and a stored payload that does not decode is
//! [`RevisionLedgerError::Corrupt`], never a degraded record.

use core::fmt;

use gauntlet_domain::{Digest, EpochSeconds, EvidenceRef, RevisionId, RunId};

use crate::deadline::Deadline;
use crate::json::{JsonError, JsonNumber, JsonValue};

// The shared contract suite lives in `crate::contract::revision_ledger`;
// this module is the port itself.

// ---------------------------------------------------------------------------
// The proposal record
// ---------------------------------------------------------------------------

/// One admitted candidate-revision proposal (spec §5.3.8/§19.3): the nine
/// declared fields, decoded from the strict `candidate_revision@v1`
/// payload at admission, plus the admission provenance (the instant and
/// the run whose evidence seeded the proposal, when the admission was
/// dispatched from one).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionProposal {
    revision: RevisionId,
    /// The affected component (§19.3 `affected component`; the domain
    /// record's `target` — an open vocabulary).
    target: String,
    claimed_problem: String,
    supporting_evidence: Vec<EvidenceRef>,
    predicted_improvement: String,
    predicted_regression_risk: Vec<String>,
    diff_ref: EvidenceRef,
    /// §19.3's `test` declaration — the evaluation plan.
    evaluation_plan: String,
    /// §19.3's `held-out evaluation` declaration — the plan.
    held_out_plan: String,
    rollback_ref: EvidenceRef,
    admitted_at: EpochSeconds,
    /// The run whose history seeded the proposal, when the admission was
    /// dispatched from one (`None` for an operator-supplied document).
    source_run: Option<RunId>,
    detail: String,
}

impl RevisionProposal {
    /// Assembles a proposal. Every declaration field is required (the
    /// codec enforced them at decode; this constructor is the ledger's
    /// typed door, used by the app admission service and the tests).
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        revision: RevisionId,
        target: String,
        claimed_problem: String,
        supporting_evidence: Vec<EvidenceRef>,
        predicted_improvement: String,
        predicted_regression_risk: Vec<String>,
        diff_ref: EvidenceRef,
        evaluation_plan: String,
        held_out_plan: String,
        rollback_ref: EvidenceRef,
        admitted_at: EpochSeconds,
        source_run: Option<RunId>,
        detail: String,
    ) -> Self {
        Self {
            revision,
            target,
            claimed_problem,
            supporting_evidence,
            predicted_improvement,
            predicted_regression_risk,
            diff_ref,
            evaluation_plan,
            held_out_plan,
            rollback_ref,
            admitted_at,
            source_run,
            detail,
        }
    }

    pub fn revision(&self) -> &RevisionId {
        &self.revision
    }

    pub fn target(&self) -> &str {
        &self.target
    }

    pub fn claimed_problem(&self) -> &str {
        &self.claimed_problem
    }

    pub fn supporting_evidence(&self) -> &[EvidenceRef] {
        &self.supporting_evidence
    }

    pub fn predicted_improvement(&self) -> &str {
        &self.predicted_improvement
    }

    pub fn predicted_regression_risk(&self) -> &[String] {
        &self.predicted_regression_risk
    }

    pub fn diff_ref(&self) -> &EvidenceRef {
        &self.diff_ref
    }

    pub fn evaluation_plan(&self) -> &str {
        &self.evaluation_plan
    }

    pub fn held_out_plan(&self) -> &str {
        &self.held_out_plan
    }

    pub fn rollback_ref(&self) -> &EvidenceRef {
        &self.rollback_ref
    }

    pub fn admitted_at(&self) -> EpochSeconds {
        self.admitted_at
    }

    pub fn source_run(&self) -> Option<&RunId> {
        self.source_run.as_ref()
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    /// Whether two proposals agree on every DECLARED FACT (COR-1, critic
    /// M1: the nine §19.3 payload fields plus the provenance run and the
    /// admission's own detail). The ADMISSION INSTANT deliberately does
    /// not participate: it is admission provenance stamped by the moving
    /// clock, not a declared fact — a replayed document mints the same
    /// content-derived id at a later instant and must be the idempotent
    /// no-op, never a false "different declared facts" conflict.
    pub fn facts_agree(&self, other: &RevisionProposal) -> bool {
        self.target == other.target
            && self.claimed_problem == other.claimed_problem
            && self.supporting_evidence == other.supporting_evidence
            && self.predicted_improvement == other.predicted_improvement
            && self.predicted_regression_risk == other.predicted_regression_risk
            && self.diff_ref == other.diff_ref
            && self.evaluation_plan == other.evaluation_plan
            && self.held_out_plan == other.held_out_plan
            && self.rollback_ref == other.rollback_ref
            && self.source_run == other.source_run
            && self.detail == other.detail
    }

    pub fn to_json(&self) -> String {
        JsonValue::Object(vec![
            (
                "revision".to_owned(),
                JsonValue::Str(self.revision.as_str().to_owned()),
            ),
            ("target".to_owned(), JsonValue::Str(self.target.clone())),
            (
                "claimed_problem".to_owned(),
                JsonValue::Str(self.claimed_problem.clone()),
            ),
            (
                "supporting_evidence".to_owned(),
                refs_json(&self.supporting_evidence),
            ),
            (
                "predicted_improvement".to_owned(),
                JsonValue::Str(self.predicted_improvement.clone()),
            ),
            (
                "predicted_regression_risk".to_owned(),
                JsonValue::Array(
                    self.predicted_regression_risk
                        .iter()
                        .map(|risk| JsonValue::Str(risk.clone()))
                        .collect(),
                ),
            ),
            (
                "diff_ref".to_owned(),
                JsonValue::Str(self.diff_ref.as_str().to_owned()),
            ),
            (
                "evaluation_plan".to_owned(),
                JsonValue::Str(self.evaluation_plan.clone()),
            ),
            (
                "held_out_plan".to_owned(),
                JsonValue::Str(self.held_out_plan.clone()),
            ),
            (
                "rollback_ref".to_owned(),
                JsonValue::Str(self.rollback_ref.as_str().to_owned()),
            ),
            (
                "admitted_at".to_owned(),
                JsonValue::Number(JsonNumber::Uint(self.admitted_at.get())),
            ),
            (
                "source_run".to_owned(),
                self.source_run.as_ref().map_or(JsonValue::Null, |run| {
                    JsonValue::Str(run.as_str().to_owned())
                }),
            ),
            ("detail".to_owned(), JsonValue::Str(self.detail.clone())),
        ])
        .to_json_text()
    }

    /// Strict decode of the canonical bytes: unknown keys rejected; a
    /// payload that does not decode is `Corrupt`, never a degraded record.
    pub fn decode(text: &str) -> Result<Self, RevisionLedgerError> {
        const FIELDS: &[&str] = &[
            "revision",
            "target",
            "claimed_problem",
            "supporting_evidence",
            "predicted_improvement",
            "predicted_regression_risk",
            "diff_ref",
            "evaluation_plan",
            "held_out_plan",
            "rollback_ref",
            "admitted_at",
            "source_run",
            "detail",
        ];
        let document = crate::json::parse(text).map_err(|error| RevisionLedgerError::Corrupt {
            detail: format!("a proposal payload is not valid JSON: {error}"),
        })?;
        reject_unknown(&document, FIELDS)?;
        let revision = RevisionId::new(string_field(&document, "revision")?)
            .map_err(corrupt_field("revision"))?;
        let admitted_at = uint_field(&document, "admitted_at")?;
        let source_run = match document.get("source_run") {
            None | Some(JsonValue::Null) => None,
            Some(JsonValue::Str(raw)) => {
                Some(RunId::new(raw).map_err(corrupt_field("source_run"))?)
            }
            Some(_) => {
                return Err(RevisionLedgerError::Corrupt {
                    detail: "the ledger field source_run is null or a run id".to_owned(),
                });
            }
        };
        Ok(Self {
            revision,
            target: string_field(&document, "target")?.to_owned(),
            claimed_problem: string_field(&document, "claimed_problem")?.to_owned(),
            supporting_evidence: refs_field(&document, "supporting_evidence")?,
            predicted_improvement: string_field(&document, "predicted_improvement")?.to_owned(),
            predicted_regression_risk: strings_field(&document, "predicted_regression_risk")?,
            diff_ref: ref_field(&document, "diff_ref")?,
            evaluation_plan: string_field(&document, "evaluation_plan")?.to_owned(),
            held_out_plan: string_field(&document, "held_out_plan")?.to_owned(),
            rollback_ref: ref_field(&document, "rollback_ref")?,
            admitted_at: EpochSeconds::new(admitted_at),
            source_run,
            detail: string_field(&document, "detail")?.to_owned(),
        })
    }
}

// ---------------------------------------------------------------------------
// Run metrics — the typed figures the harness derives from durable facts
// ---------------------------------------------------------------------------

/// One settlement a run committed, with the facts the gate's conditions
/// read: the unit, the verifying instrument's §14.2 model-rung flag, and
/// the settlement's own occurrence instant (its verification record's
/// `produced_at`), in the order the store reads them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettlementFact {
    pub unit: String,
    pub instrument_deterministic: Option<bool>,
    pub occurred_at: Option<EpochSeconds>,
}

impl SettlementFact {
    fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("unit".to_owned(), JsonValue::Str(self.unit.clone())),
            (
                "instrument_deterministic".to_owned(),
                self.instrument_deterministic
                    .map_or(JsonValue::Null, JsonValue::Bool),
            ),
            (
                "occurred_at".to_owned(),
                self.occurred_at.map_or(JsonValue::Null, |instant| {
                    JsonValue::Number(JsonNumber::Uint(instant.get()))
                }),
            ),
        ])
    }

    fn from_json(value: &JsonValue) -> Result<Self, RevisionLedgerError> {
        let JsonValue::Object(_) = value else {
            return Err(RevisionLedgerError::Corrupt {
                detail: "a settlement fact is an object".to_owned(),
            });
        };
        let unit = string_field(value, "unit")?.to_owned();
        let instrument_deterministic = match value.get("instrument_deterministic") {
            None | Some(JsonValue::Null) => None,
            Some(JsonValue::Bool(flag)) => Some(*flag),
            Some(_) => {
                return Err(RevisionLedgerError::Corrupt {
                    detail: "the ledger field instrument_deterministic is null or a boolean"
                        .to_owned(),
                });
            }
        };
        let occurred_at = match value.get("occurred_at") {
            None | Some(JsonValue::Null) => None,
            Some(JsonValue::Number(JsonNumber::Uint(raw))) => Some(EpochSeconds::new(*raw)),
            Some(_) => {
                return Err(RevisionLedgerError::Corrupt {
                    detail: "the ledger field occurred_at is null or an instant".to_owned(),
                });
            }
        };
        Ok(Self {
            unit,
            instrument_deterministic,
            occurred_at,
        })
    }
}

/// The typed metrics of ONE run, derived by the evaluation harness from
/// that run's DURABLE facts (settled units, iterations, payment,
/// per-settlement verifying instruments, the dispatched-unit mix) with
/// the SAME function for both arms. Nothing here is claimed — every field
/// is a store read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunMetrics {
    pub iterations: u64,
    /// The latest durable checkpoint's token counter (the payment fact).
    pub payment_tokens: u64,
    /// The latest durable checkpoint's cost counter, micro-dollars.
    pub payment_micro_usd: u64,
    /// The settlements the run committed, in store order.
    pub settlements: Vec<SettlementFact>,
    /// The units the run dispatched (attempted), sorted and deduplicated
    /// — the governance condition's durable comparison figure. Tier and
    /// authority are per-unit properties frozen in the reference both
    /// arms share, so equality of the dispatched-unit mix IS equality of
    /// the tier/authority mix over facts the store can read.
    pub dispatched_units: Vec<String>,
    /// Whether the run reached a settled terminal state.
    pub settled_terminal: bool,
}

impl RunMetrics {
    pub fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            (
                "iterations".to_owned(),
                JsonValue::Number(JsonNumber::Uint(self.iterations)),
            ),
            (
                "payment_tokens".to_owned(),
                JsonValue::Number(JsonNumber::Uint(self.payment_tokens)),
            ),
            (
                "payment_micro_usd".to_owned(),
                JsonValue::Number(JsonNumber::Uint(self.payment_micro_usd)),
            ),
            (
                "settlements".to_owned(),
                JsonValue::Array(
                    self.settlements
                        .iter()
                        .map(SettlementFact::to_json)
                        .collect(),
                ),
            ),
            (
                "dispatched_units".to_owned(),
                JsonValue::Array(
                    self.dispatched_units
                        .iter()
                        .map(|token| JsonValue::Str(token.clone()))
                        .collect(),
                ),
            ),
            (
                "settled_terminal".to_owned(),
                JsonValue::Bool(self.settled_terminal),
            ),
        ])
    }

    pub fn decode(value: &JsonValue) -> Result<Self, RevisionLedgerError> {
        const FIELDS: &[&str] = &[
            "iterations",
            "payment_tokens",
            "payment_micro_usd",
            "settlements",
            "dispatched_units",
            "settled_terminal",
        ];
        let JsonValue::Object(_) = value else {
            return Err(RevisionLedgerError::Corrupt {
                detail: "run metrics are an object".to_owned(),
            });
        };
        reject_unknown(value, FIELDS)?;
        let settlements = match value.get("settlements") {
            Some(JsonValue::Array(items)) => items
                .iter()
                .map(SettlementFact::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            _ => {
                return Err(RevisionLedgerError::Corrupt {
                    detail: "the ledger field settlements is an array".to_owned(),
                });
            }
        };
        let dispatched_units = strings_in(value, "dispatched_units")?;
        Ok(Self {
            iterations: uint_in(value, "iterations")?,
            payment_tokens: uint_in(value, "payment_tokens")?,
            payment_micro_usd: uint_in(value, "payment_micro_usd")?,
            settlements,
            dispatched_units,
            settled_terminal: match value.get("settled_terminal") {
                Some(JsonValue::Bool(flag)) => *flag,
                _ => {
                    return Err(RevisionLedgerError::Corrupt {
                        detail: "the ledger field settled_terminal is a boolean".to_owned(),
                    });
                }
            },
        })
    }
}

// ---------------------------------------------------------------------------
// The evaluation record
// ---------------------------------------------------------------------------

/// One isolated evaluation (charter D4, the `evaluate` piece): the four
/// REAL composed runs — control and treatment over the held-out set AND
/// the regression set — with all four arms' typed metrics and ALL FOUR
/// arms' frozen reference digests (COR-2, R1: the regression arms'
/// rulers are recorded, never implicit), all derived from durable facts.
/// Idempotent on
/// the run-set identity: the same four runs re-appended agree is a no-op;
/// a disagreeing re-append is [`RevisionLedgerError::EvaluationConflict`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvaluationRecord {
    pub revision: RevisionId,
    /// The target-metric token the evaluation declared (§19.4 condition
    /// 7's typed vocabulary — the operator names it at evaluation time,
    /// and the gate judges exactly this metric).
    pub metric: String,
    /// The protected-capability set the evaluation declared (§19.4
    /// condition 6) — recorded at evaluation time, so the gate adjudicates
    /// exactly the set the operator named when the arms ran.
    pub protected: Vec<String>,
    pub held_out_control: RunId,
    pub held_out_treatment: RunId,
    pub regression_control: RunId,
    pub regression_treatment: RunId,
    /// The treatment arms' VERIFIED parent runs (spec §5.3.3; COR-1,
    /// critic M2): the harness checked each treatment run's DURABLE record
    /// (`load_run(...).parent_run`) against the declared control parent
    /// BEFORE appending this record, and the verified fact is what the
    /// record carries. An isolated evaluation whose parent cannot be
    /// verified never becomes a record at all — so a record's presence
    /// certifies its isolation.
    pub held_out_treatment_parent: RunId,
    pub regression_treatment_parent: RunId,
    pub control_reference_digest: Digest,
    pub treatment_reference_digest: Digest,
    /// The REGRESSION arms' frozen reference digests (COR-2, R1): the
    /// regression task set is an operator-named project with its OWN
    /// frozen reference, and the falsifier evaluated with the held-out
    /// arms freezing one ruler while the regression arms froze a MOVED
    /// one — the record carried no field that could even notice. All
    /// FOUR arms' rulers are durable, and the gate's reference-integrity
    /// condition checks all four against the current frozen digest.
    pub regression_control_reference_digest: Digest,
    pub regression_treatment_reference_digest: Digest,
    pub control_held_out_metrics: RunMetrics,
    pub treatment_held_out_metrics: RunMetrics,
    pub control_regression_metrics: RunMetrics,
    pub treatment_regression_metrics: RunMetrics,
    pub at: EpochSeconds,
}

impl EvaluationRecord {
    /// The declared identity: the four run ids. The same set re-appended
    /// with agreeing facts is the same evaluation.
    pub fn identity(&self) -> (String, String, String, String) {
        (
            self.held_out_control.as_str().to_owned(),
            self.held_out_treatment.as_str().to_owned(),
            self.regression_control.as_str().to_owned(),
            self.regression_treatment.as_str().to_owned(),
        )
    }

    /// Whether two evaluation records agree on every DECLARED fact
    /// (COR-2, R4): the FOUR arms' reference digests, the four arms'
    /// metrics, the verified isolation parents, the record instant, and
    /// the OPERATOR'S DECLARATIONS — the target metric and the protected
    /// capability set. The falsifier's probe C re-appended the same
    /// run-set with a different declared metric and an emptier protected
    /// set and was swallowed as the idempotent no-op, keeping the ORIGINAL
    /// declaration: the declarations the gate adjudicates are part of the
    /// agreement, so a disagreeing re-append is the typed
    /// [`RevisionLedgerError::EvaluationConflict`], the stored record
    /// standing.
    pub fn facts_agree(&self, other: &EvaluationRecord) -> bool {
        self.metric == other.metric
            && self.protected == other.protected
            && self.control_reference_digest == other.control_reference_digest
            && self.treatment_reference_digest == other.treatment_reference_digest
            && self.regression_control_reference_digest == other.regression_control_reference_digest
            && self.regression_treatment_reference_digest
                == other.regression_treatment_reference_digest
            && self.control_held_out_metrics == other.control_held_out_metrics
            && self.treatment_held_out_metrics == other.treatment_held_out_metrics
            && self.control_regression_metrics == other.control_regression_metrics
            && self.treatment_regression_metrics == other.treatment_regression_metrics
            && self.held_out_treatment_parent == other.held_out_treatment_parent
            && self.regression_treatment_parent == other.regression_treatment_parent
            && self.at == other.at
    }

    pub fn to_json(&self) -> String {
        JsonValue::Object(vec![
            (
                "revision".to_owned(),
                JsonValue::Str(self.revision.as_str().to_owned()),
            ),
            ("metric".to_owned(), JsonValue::Str(self.metric.clone())),
            (
                "protected".to_owned(),
                JsonValue::Array(
                    self.protected
                        .iter()
                        .map(|id| JsonValue::Str(id.clone()))
                        .collect(),
                ),
            ),
            (
                "held_out_control".to_owned(),
                JsonValue::Str(self.held_out_control.as_str().to_owned()),
            ),
            (
                "held_out_treatment".to_owned(),
                JsonValue::Str(self.held_out_treatment.as_str().to_owned()),
            ),
            (
                "regression_control".to_owned(),
                JsonValue::Str(self.regression_control.as_str().to_owned()),
            ),
            (
                "regression_treatment".to_owned(),
                JsonValue::Str(self.regression_treatment.as_str().to_owned()),
            ),
            (
                "held_out_treatment_parent".to_owned(),
                JsonValue::Str(self.held_out_treatment_parent.as_str().to_owned()),
            ),
            (
                "regression_treatment_parent".to_owned(),
                JsonValue::Str(self.regression_treatment_parent.as_str().to_owned()),
            ),
            (
                "control_reference_digest".to_owned(),
                JsonValue::Str(self.control_reference_digest.to_string()),
            ),
            (
                "treatment_reference_digest".to_owned(),
                JsonValue::Str(self.treatment_reference_digest.to_string()),
            ),
            (
                "regression_control_reference_digest".to_owned(),
                JsonValue::Str(self.regression_control_reference_digest.to_string()),
            ),
            (
                "regression_treatment_reference_digest".to_owned(),
                JsonValue::Str(self.regression_treatment_reference_digest.to_string()),
            ),
            (
                "control_held_out_metrics".to_owned(),
                self.control_held_out_metrics.to_json(),
            ),
            (
                "treatment_held_out_metrics".to_owned(),
                self.treatment_held_out_metrics.to_json(),
            ),
            (
                "control_regression_metrics".to_owned(),
                self.control_regression_metrics.to_json(),
            ),
            (
                "treatment_regression_metrics".to_owned(),
                self.treatment_regression_metrics.to_json(),
            ),
            (
                "at".to_owned(),
                JsonValue::Number(JsonNumber::Uint(self.at.get())),
            ),
        ])
        .to_json_text()
    }

    pub fn decode(text: &str) -> Result<Self, RevisionLedgerError> {
        const FIELDS: &[&str] = &[
            "revision",
            "metric",
            "protected",
            "held_out_control",
            "held_out_treatment",
            "regression_control",
            "regression_treatment",
            "held_out_treatment_parent",
            "regression_treatment_parent",
            "control_reference_digest",
            "treatment_reference_digest",
            "regression_control_reference_digest",
            "regression_treatment_reference_digest",
            "control_held_out_metrics",
            "treatment_held_out_metrics",
            "control_regression_metrics",
            "treatment_regression_metrics",
            "at",
        ];
        let document = crate::json::parse(text).map_err(|error| RevisionLedgerError::Corrupt {
            detail: format!("an evaluation payload is not valid JSON: {error}"),
        })?;
        reject_unknown(&document, FIELDS)?;
        let metrics = |name: &str| -> Result<RunMetrics, RevisionLedgerError> {
            let value = document
                .get(name)
                .ok_or_else(|| RevisionLedgerError::Corrupt {
                    detail: format!("the ledger field {name:?} is absent"),
                })?;
            RunMetrics::decode(value)
        };
        Ok(Self {
            revision: RevisionId::new(string_field(&document, "revision")?)
                .map_err(corrupt_field("revision"))?,
            metric: string_field(&document, "metric")?.to_owned(),
            protected: strings_field(&document, "protected")?,
            held_out_control: RunId::new(string_field(&document, "held_out_control")?)
                .map_err(corrupt_field("held_out_control"))?,
            held_out_treatment: RunId::new(string_field(&document, "held_out_treatment")?)
                .map_err(corrupt_field("held_out_treatment"))?,
            regression_control: RunId::new(string_field(&document, "regression_control")?)
                .map_err(corrupt_field("regression_control"))?,
            regression_treatment: RunId::new(string_field(&document, "regression_treatment")?)
                .map_err(corrupt_field("regression_treatment"))?,
            held_out_treatment_parent: RunId::new(string_field(
                &document,
                "held_out_treatment_parent",
            )?)
            .map_err(corrupt_field("held_out_treatment_parent"))?,
            regression_treatment_parent: RunId::new(string_field(
                &document,
                "regression_treatment_parent",
            )?)
            .map_err(corrupt_field("regression_treatment_parent"))?,
            control_reference_digest: Digest::parse(string_field(
                &document,
                "control_reference_digest",
            )?)
            .map_err(corrupt_field("control_reference_digest"))?,
            treatment_reference_digest: Digest::parse(string_field(
                &document,
                "treatment_reference_digest",
            )?)
            .map_err(corrupt_field("treatment_reference_digest"))?,
            regression_control_reference_digest: Digest::parse(string_field(
                &document,
                "regression_control_reference_digest",
            )?)
            .map_err(corrupt_field("regression_control_reference_digest"))?,
            regression_treatment_reference_digest: Digest::parse(string_field(
                &document,
                "regression_treatment_reference_digest",
            )?)
            .map_err(corrupt_field("regression_treatment_reference_digest"))?,
            control_held_out_metrics: metrics("control_held_out_metrics")?,
            treatment_held_out_metrics: metrics("treatment_held_out_metrics")?,
            control_regression_metrics: metrics("control_regression_metrics")?,
            treatment_regression_metrics: metrics("treatment_regression_metrics")?,
            at: EpochSeconds::new(uint_field(&document, "at")?),
        })
    }
}

// ---------------------------------------------------------------------------
// Gate refusals — the §19.4 typed per-condition vocabulary
// ---------------------------------------------------------------------------

/// One §19.4 condition refusal, typed (charter D4, the `gate` piece). The
/// eight conditions refuse by name; the evidence gaps (missing evaluation,
/// failed/absent runs, underivable metrics) refuse too — insufficient
/// evidence is NEVER a negative result, so the gaps never read as
/// `Rejected`; they adjudicate `Provisional`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateRefusal {
    /// No evaluation record on the ledger for this revision.
    MissingEvaluation,
    /// An evaluation's runs are absent or did not reach a settled terminal
    /// state — the arms produced no adjudicable evidence.
    EvaluationIncomplete { detail: String },
    /// A run's typed metrics could not be derived from its durable facts.
    MetricsUnderivable { run: String, detail: String },
    /// Condition 1: the treatment arm's dispatched tier/role mix differs
    /// from control's, or the declared target is governance-bearing.
    GovernanceAltered { detail: String },
    /// Condition 2: an evaluation arm does not carry the current frozen
    /// reference digest — a moved ruler never adjudicates.
    ReferenceMoved { detail: String },
    /// Condition 3 — AK-056's refusal. The candidate's configuration delta
    /// removes, disables, or downgrades a verification-bearing key, or the
    /// treatment settlements' instrument series is weaker than control's.
    SettlementAccessWeakened { detail: String },
    /// Condition 4: the rollback declaration is absent or does not
    /// resolve. A revision without one cannot be adjudicated.
    RollbackInvalid { detail: String },
    /// Condition 5: on the regression set, a unit control settled did not
    /// settle under treatment, or settled with a weaker instrument.
    RegressionSuiteFailed { detail: String },
    /// Condition 6: a declared protected capability reads inferior in the
    /// treatment arm.
    ProtectedCapabilityInferior { capability: String, detail: String },
    /// Condition 7: the target metric's improvement does not meet the
    /// declared margin. Figures are fixed-point per 10 000 (lower is
    /// better).
    MarginNotMet {
        metric: String,
        control_figure: u64,
        treatment_figure: u64,
        margin_bps: u64,
    },
    /// Condition 8: treatment payment per settlement rose above control's.
    PaymentIncreased {
        control_figure: u64,
        treatment_figure: u64,
    },
}

impl GateRefusal {
    /// Every refusal's wire token (the per-condition vocabulary the
    /// durable reasons record).
    pub fn token(&self) -> &'static str {
        match self {
            Self::MissingEvaluation => "missing_evaluation",
            Self::EvaluationIncomplete { .. } => "evaluation_incomplete",
            Self::MetricsUnderivable { .. } => "metrics_underivable",
            Self::GovernanceAltered { .. } => "governance_altered",
            Self::ReferenceMoved { .. } => "reference_moved",
            Self::SettlementAccessWeakened { .. } => "settlement_access_weakened",
            Self::RollbackInvalid { .. } => "rollback_invalid",
            Self::RegressionSuiteFailed { .. } => "regression_suite_failed",
            Self::ProtectedCapabilityInferior { .. } => "protected_capability_inferior",
            Self::MarginNotMet { .. } => "margin_not_met",
            Self::PaymentIncreased { .. } => "payment_increased",
        }
    }

    pub fn to_json(&self) -> JsonValue {
        let mut object = vec![(
            "refusal".to_owned(),
            JsonValue::Str(self.token().to_owned()),
        )];
        match self {
            Self::EvaluationIncomplete { detail }
            | Self::GovernanceAltered { detail }
            | Self::ReferenceMoved { detail }
            | Self::SettlementAccessWeakened { detail }
            | Self::RollbackInvalid { detail }
            | Self::RegressionSuiteFailed { detail } => {
                object.push(("detail".to_owned(), JsonValue::Str(detail.clone())));
            }
            Self::MetricsUnderivable { run, detail } => {
                object.push(("run".to_owned(), JsonValue::Str(run.clone())));
                object.push(("detail".to_owned(), JsonValue::Str(detail.clone())));
            }
            Self::ProtectedCapabilityInferior { capability, detail } => {
                object.push(("capability".to_owned(), JsonValue::Str(capability.clone())));
                object.push(("detail".to_owned(), JsonValue::Str(detail.clone())));
            }
            Self::MarginNotMet {
                metric,
                control_figure,
                treatment_figure,
                margin_bps,
            } => {
                object.push(("metric".to_owned(), JsonValue::Str(metric.clone())));
                object.push((
                    "control_figure".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(*control_figure)),
                ));
                object.push((
                    "treatment_figure".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(*treatment_figure)),
                ));
                object.push((
                    "margin_bps".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(*margin_bps)),
                ));
            }
            Self::PaymentIncreased {
                control_figure,
                treatment_figure,
            } => {
                object.push((
                    "control_figure".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(*control_figure)),
                ));
                object.push((
                    "treatment_figure".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(*treatment_figure)),
                ));
            }
            Self::MissingEvaluation => {}
        }
        JsonValue::Object(object)
    }

    pub fn from_json(value: &JsonValue) -> Result<Self, RevisionLedgerError> {
        let JsonValue::Object(_) = value else {
            return Err(RevisionLedgerError::Corrupt {
                detail: "a gate refusal is an object".to_owned(),
            });
        };
        let token = string_field(value, "refusal")?;
        let detail_of = |name: &str| -> Result<String, RevisionLedgerError> {
            match value.get(name) {
                Some(JsonValue::Str(text)) => Ok(text.clone()),
                _ => Err(RevisionLedgerError::Corrupt {
                    detail: format!("the {token} refusal carries no {name}"),
                }),
            }
        };
        let uint_of = |name: &str| -> Result<u64, RevisionLedgerError> {
            match value.get(name) {
                Some(JsonValue::Number(JsonNumber::Uint(n))) => Ok(*n),
                _ => Err(RevisionLedgerError::Corrupt {
                    detail: format!("the {token} refusal carries no {name} figure"),
                }),
            }
        };
        match token {
            "missing_evaluation" => Ok(Self::MissingEvaluation),
            "evaluation_incomplete" => Ok(Self::EvaluationIncomplete {
                detail: detail_of("detail")?,
            }),
            "metrics_underivable" => Ok(Self::MetricsUnderivable {
                run: detail_of("run")?,
                detail: detail_of("detail")?,
            }),
            "governance_altered" => Ok(Self::GovernanceAltered {
                detail: detail_of("detail")?,
            }),
            "reference_moved" => Ok(Self::ReferenceMoved {
                detail: detail_of("detail")?,
            }),
            "settlement_access_weakened" => Ok(Self::SettlementAccessWeakened {
                detail: detail_of("detail")?,
            }),
            "rollback_invalid" => Ok(Self::RollbackInvalid {
                detail: detail_of("detail")?,
            }),
            "regression_suite_failed" => Ok(Self::RegressionSuiteFailed {
                detail: detail_of("detail")?,
            }),
            "protected_capability_inferior" => Ok(Self::ProtectedCapabilityInferior {
                capability: detail_of("capability")?,
                detail: detail_of("detail")?,
            }),
            "margin_not_met" => Ok(Self::MarginNotMet {
                metric: detail_of("metric")?,
                control_figure: uint_of("control_figure")?,
                treatment_figure: uint_of("treatment_figure")?,
                margin_bps: uint_of("margin_bps")?,
            }),
            "payment_increased" => Ok(Self::PaymentIncreased {
                control_figure: uint_of("control_figure")?,
                treatment_figure: uint_of("treatment_figure")?,
            }),
            other => Err(RevisionLedgerError::Corrupt {
                detail: format!("gate refusal token {other:?} is outside the typed vocabulary"),
            }),
        }
    }
}

impl fmt::Display for GateRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingEvaluation => write!(
                f,
                "missing evaluation: no isolated-evaluation record is on the ledger — \
                 insufficient evidence is provisional, never promoted (spec 19.4)"
            ),
            Self::EvaluationIncomplete { detail } => {
                write!(f, "evaluation incomplete: {detail}")
            }
            Self::MetricsUnderivable { run, detail } => {
                write!(f, "metrics underivable for run {run}: {detail}")
            }
            Self::GovernanceAltered { detail } => {
                write!(f, "governance altered: {detail} (spec 19.4, condition 1)")
            }
            Self::ReferenceMoved { detail } => {
                write!(f, "reference moved: {detail} (spec 19.4, condition 2)")
            }
            Self::SettlementAccessWeakened { detail } => write!(
                f,
                "settlement access weakened: {detail} — a candidate faster by weakening \
                 verification is a regression, not an improvement (spec 19.4, condition 3)"
            ),
            Self::RollbackInvalid { detail } => {
                write!(f, "rollback invalid: {detail} (spec 19.4, condition 4)")
            }
            Self::RegressionSuiteFailed { detail } => {
                write!(
                    f,
                    "regression suite failed: {detail} (spec 19.4, condition 5)"
                )
            }
            Self::ProtectedCapabilityInferior { capability, detail } => write!(
                f,
                "protected capability {capability} inferior: {detail} (spec 19.4, condition 6)"
            ),
            Self::MarginNotMet {
                metric,
                control_figure,
                treatment_figure,
                margin_bps,
            } => write!(
                f,
                "margin not met: {metric} control {control_figure} vs treatment \
                 {treatment_figure} (per 10 000) does not beat the declared margin of \
                 {margin_bps} basis points (spec 19.4, condition 7)"
            ),
            Self::PaymentIncreased {
                control_figure,
                treatment_figure,
            } => write!(
                f,
                "payment increased: control {control_figure} vs treatment {treatment_figure} \
                 per 10 000 (spec 19.4, condition 8)"
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// The gate's decision record
// ---------------------------------------------------------------------------

/// The eight-condition satisfaction record a PROMOTED decision carries —
/// every figure the conditions produced, durable (charter D4).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionPromotionEvidence {
    pub metric: String,
    pub control_figure: u64,
    pub treatment_figure: u64,
    pub margin_bps: u64,
    pub control_payment_figure: u64,
    pub treatment_payment_figure: u64,
    pub unit_mix_preserved: bool,
    pub instrument_series_non_weaker: bool,
    pub control_reference_digest: String,
    pub treatment_reference_digest: String,
    pub regression_units_control_settled: usize,
    pub regression_units_treatment_settled: usize,
    pub protected_capabilities: Vec<String>,
}

impl RevisionPromotionEvidence {
    pub fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("metric".to_owned(), JsonValue::Str(self.metric.clone())),
            (
                "control_figure".to_owned(),
                JsonValue::Number(JsonNumber::Uint(self.control_figure)),
            ),
            (
                "treatment_figure".to_owned(),
                JsonValue::Number(JsonNumber::Uint(self.treatment_figure)),
            ),
            (
                "margin_bps".to_owned(),
                JsonValue::Number(JsonNumber::Uint(self.margin_bps)),
            ),
            (
                "control_payment_figure".to_owned(),
                JsonValue::Number(JsonNumber::Uint(self.control_payment_figure)),
            ),
            (
                "treatment_payment_figure".to_owned(),
                JsonValue::Number(JsonNumber::Uint(self.treatment_payment_figure)),
            ),
            (
                "unit_mix_preserved".to_owned(),
                JsonValue::Bool(self.unit_mix_preserved),
            ),
            (
                "instrument_series_non_weaker".to_owned(),
                JsonValue::Bool(self.instrument_series_non_weaker),
            ),
            (
                "control_reference_digest".to_owned(),
                JsonValue::Str(self.control_reference_digest.clone()),
            ),
            (
                "treatment_reference_digest".to_owned(),
                JsonValue::Str(self.treatment_reference_digest.clone()),
            ),
            (
                "regression_units_control_settled".to_owned(),
                JsonValue::Number(JsonNumber::Uint(
                    self.regression_units_control_settled as u64,
                )),
            ),
            (
                "regression_units_treatment_settled".to_owned(),
                JsonValue::Number(JsonNumber::Uint(
                    self.regression_units_treatment_settled as u64,
                )),
            ),
            (
                "protected_capabilities".to_owned(),
                JsonValue::Array(
                    self.protected_capabilities
                        .iter()
                        .map(|id| JsonValue::Str(id.clone()))
                        .collect(),
                ),
            ),
        ])
    }

    pub fn from_json(value: &JsonValue) -> Result<Self, RevisionLedgerError> {
        // "kind" is the promoted decision object's own tag (the verdict
        // envelope's discriminator) — accepted here and ignored: the
        // evidence fields are the record.
        const FIELDS: &[&str] = &[
            "kind",
            "metric",
            "control_figure",
            "treatment_figure",
            "margin_bps",
            "control_payment_figure",
            "treatment_payment_figure",
            "unit_mix_preserved",
            "instrument_series_non_weaker",
            "control_reference_digest",
            "treatment_reference_digest",
            "regression_units_control_settled",
            "regression_units_treatment_settled",
            "protected_capabilities",
        ];
        let JsonValue::Object(_) = value else {
            return Err(RevisionLedgerError::Corrupt {
                detail: "the promotion evidence is an object".to_owned(),
            });
        };
        reject_unknown(value, FIELDS)?;
        let uint_of = |name: &str| -> Result<u64, RevisionLedgerError> {
            match value.get(name) {
                Some(JsonValue::Number(JsonNumber::Uint(n))) => Ok(*n),
                _ => Err(RevisionLedgerError::Corrupt {
                    detail: format!("the promotion evidence carries no {name} figure"),
                }),
            }
        };
        let bool_of = |name: &str| -> Result<bool, RevisionLedgerError> {
            match value.get(name) {
                Some(JsonValue::Bool(flag)) => Ok(*flag),
                _ => Err(RevisionLedgerError::Corrupt {
                    detail: format!("the promotion evidence carries no {name} flag"),
                }),
            }
        };
        Ok(Self {
            metric: string_field(value, "metric")?.to_owned(),
            control_figure: uint_of("control_figure")?,
            treatment_figure: uint_of("treatment_figure")?,
            margin_bps: uint_of("margin_bps")?,
            control_payment_figure: uint_of("control_payment_figure")?,
            treatment_payment_figure: uint_of("treatment_payment_figure")?,
            unit_mix_preserved: bool_of("unit_mix_preserved")?,
            instrument_series_non_weaker: bool_of("instrument_series_non_weaker")?,
            control_reference_digest: string_field(value, "control_reference_digest")?.to_owned(),
            treatment_reference_digest: string_field(value, "treatment_reference_digest")?
                .to_owned(),
            regression_units_control_settled: usize::try_from(uint_of(
                "regression_units_control_settled",
            )?)
            .map_err(corrupt_field("regression_units_control_settled"))?,
            regression_units_treatment_settled: usize::try_from(uint_of(
                "regression_units_treatment_settled",
            )?)
            .map_err(corrupt_field("regression_units_treatment_settled"))?,
            protected_capabilities: strings_in(value, "protected_capabilities")?,
        })
    }
}

/// The §19.4 gate's verdict over one revision — the payload of a decision
/// record. Composed only through [`GateDecision::new`], which enforces
/// the honest shapes: a promotion carries the eight-condition evidence; a
/// rejection carries the failed conditions' reasons AND the rollback ref
/// (the domain floor requires it); a provisional carries the evidence
/// gaps' reasons — never empty, never a promotion.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateDecision {
    Promoted(RevisionPromotionEvidence),
    Rejected {
        reasons: Vec<GateRefusal>,
        rollback_ref: EvidenceRef,
    },
    Provisional {
        reasons: Vec<GateRefusal>,
    },
}

/// Why a verdict refused to compose — the honest-shape refusals.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GateDecisionError {
    /// A `Rejected` or `Provisional` verdict carried an EMPTY reason set:
    /// a decision record with no reasons in it is one the honest gate can
    /// never produce (every condition is evaluated; every evidence gap
    /// refuses).
    EmptyReasons,
}

impl fmt::Display for GateDecisionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyReasons => write!(
                f,
                "a rejection or a provisional carried zero reasons — every §19.4 condition \
                 is evaluated and every evidence gap refuses, so the honest gate can never \
                 produce an empty reason set"
            ),
        }
    }
}

impl std::error::Error for GateDecisionError {}

impl GateDecision {
    /// Composes a `Rejected` verdict: the failed conditions' reasons (all
    /// of them — every condition is evaluated) plus the rollback ref the
    /// rejection is safe because of.
    pub fn rejected(
        reasons: Vec<GateRefusal>,
        rollback_ref: EvidenceRef,
    ) -> Result<Self, GateDecisionError> {
        if reasons.is_empty() {
            return Err(GateDecisionError::EmptyReasons);
        }
        Ok(Self::Rejected {
            reasons,
            rollback_ref,
        })
    }

    /// Composes a `Provisional` verdict: the evidence gaps' reasons. Never
    /// empty — and never a rejection (insufficient evidence is not a
    /// negative result, §19.4).
    pub fn provisional(reasons: Vec<GateRefusal>) -> Result<Self, GateDecisionError> {
        if reasons.is_empty() {
            return Err(GateDecisionError::EmptyReasons);
        }
        Ok(Self::Provisional { reasons })
    }

    /// Composes the `Promoted` verdict with its eight-condition evidence.
    pub fn promoted(evidence: RevisionPromotionEvidence) -> Self {
        Self::Promoted(evidence)
    }

    fn to_json(&self) -> JsonValue {
        match self {
            Self::Promoted(evidence) => {
                let mut object = vec![("kind".to_owned(), JsonValue::Str("promoted".to_owned()))];
                if let JsonValue::Object(entries) = evidence.to_json() {
                    object.extend(entries);
                }
                JsonValue::Object(object)
            }
            Self::Rejected {
                reasons,
                rollback_ref,
            } => JsonValue::Object(vec![
                ("kind".to_owned(), JsonValue::Str("rejected".to_owned())),
                (
                    "reasons".to_owned(),
                    JsonValue::Array(reasons.iter().map(GateRefusal::to_json).collect()),
                ),
                (
                    "rollback_ref".to_owned(),
                    JsonValue::Str(rollback_ref.as_str().to_owned()),
                ),
            ]),
            Self::Provisional { reasons } => JsonValue::Object(vec![
                ("kind".to_owned(), JsonValue::Str("provisional".to_owned())),
                (
                    "reasons".to_owned(),
                    JsonValue::Array(reasons.iter().map(GateRefusal::to_json).collect()),
                ),
            ]),
        }
    }

    fn from_json(value: &JsonValue) -> Result<Self, RevisionLedgerError> {
        let JsonValue::Object(_) = value else {
            return Err(RevisionLedgerError::Corrupt {
                detail: "a gate decision is an object".to_owned(),
            });
        };
        let reasons_of = |name: &str| -> Result<Vec<GateRefusal>, RevisionLedgerError> {
            let Some(JsonValue::Array(items)) = value.get(name) else {
                return Err(RevisionLedgerError::Corrupt {
                    detail: "the gate decision carries no reasons array".to_owned(),
                });
            };
            let mut reasons = Vec::with_capacity(items.len());
            for item in items {
                reasons.push(GateRefusal::from_json(item)?);
            }
            Ok(reasons)
        };
        match string_field(value, "kind")? {
            "promoted" => Ok(Self::Promoted(RevisionPromotionEvidence::from_json(value)?)),
            "rejected" => Ok(Self::Rejected {
                reasons: reasons_of("reasons")?,
                rollback_ref: ref_in(value, "rollback_ref")?,
            }),
            "provisional" => Ok(Self::Provisional {
                reasons: reasons_of("reasons")?,
            }),
            other => Err(RevisionLedgerError::Corrupt {
                detail: format!("gate decision kind {other:?} is outside the vocabulary"),
            }),
        }
    }
}

/// The derived decision state of a revision — the domain
/// `RevisionDecision` vocabulary plus the ledger's `pending` floor.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionKind {
    /// Admitted, never adjudicated.
    Pending,
    /// Adjudicated; evidence was insufficient — NOT promoted, NOT rejected.
    Provisional,
    /// Every §19.4 condition held.
    Promoted,
    /// At least one condition FAILED.
    Rejected,
}

impl DecisionKind {
    /// The wire token, exact-match parsed.
    pub fn parse(input: &str) -> Result<Self, RevisionLedgerError> {
        match input {
            "pending" => Ok(Self::Pending),
            "provisional" => Ok(Self::Provisional),
            "promoted" => Ok(Self::Promoted),
            "rejected" => Ok(Self::Rejected),
            other => Err(RevisionLedgerError::Corrupt {
                detail: format!("decision kind token {other:?} is outside the vocabulary"),
            }),
        }
    }
}

impl fmt::Display for DecisionKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Pending => "pending",
            Self::Provisional => "provisional",
            Self::Promoted => "promoted",
            Self::Rejected => "rejected",
        })
    }
}

/// Why a decision record refused to compose.
///
/// COR-1 (critic m1): the former `ReasonMismatch` variant is removed — its
/// check was a dead computation (every verdict shape "carried"), and it
/// misdocumented the design. The REAL stale-claim gate is the applying
/// ledger's [`RevisionLedgerError::StaleDecision`], which validates the
/// record's `claimed` state against the derived decision at apply time.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecisionRecordError {
    /// The move is outside the state machine: a decision past a FINAL
    /// state (`promoted`, `rejected`) can never be applied — the domain
    /// record's write-once floor.
    FinalState { from: DecisionKind },
}

impl fmt::Display for DecisionRecordError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::FinalState { from } => write!(
                f,
                "the revision's decision is write-once past a final state: {from} can never \
                 be re-adjudicated (spec 5.3.8)"
            ),
        }
    }
}

impl std::error::Error for DecisionRecordError {}

/// One first-class decision transition (charter D2) — the ledger's ONLY
/// decision writer. `claimed` is the caller's claim about the DERIVED
/// current decision; the applying ledger validates the claim. A record
/// past a final state refuses to compose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionDecisionRecord {
    revision: RevisionId,
    claimed: DecisionKind,
    decision: GateDecision,
    at: EpochSeconds,
}

impl RevisionDecisionRecord {
    /// Composes a decision record. Legal from `pending` and
    /// `provisional` (a re-adjudication of a provisional re-records its
    /// reasons or concludes — it can never lower a final state); illegal
    /// from `promoted` or `rejected` (write-once past a final state).
    /// The `claimed` state is validated where it is CHECKED — by the
    /// applying ledger against the derived decision
    /// ([`RevisionLedgerError::StaleDecision`]) — not here.
    pub fn new(
        revision: RevisionId,
        claimed: DecisionKind,
        decision: GateDecision,
        at: EpochSeconds,
    ) -> Result<Self, DecisionRecordError> {
        if matches!(claimed, DecisionKind::Promoted | DecisionKind::Rejected) {
            return Err(DecisionRecordError::FinalState { from: claimed });
        }
        Ok(Self {
            revision,
            claimed,
            decision,
            at,
        })
    }

    pub fn revision(&self) -> &RevisionId {
        &self.revision
    }

    pub fn claimed(&self) -> DecisionKind {
        self.claimed
    }

    pub fn decision(&self) -> &GateDecision {
        &self.decision
    }

    pub fn at(&self) -> EpochSeconds {
        self.at
    }

    /// The decision kind this record WRITES (the derived state's new
    /// value).
    pub fn writes(&self) -> DecisionKind {
        match &self.decision {
            GateDecision::Promoted(_) => DecisionKind::Promoted,
            GateDecision::Rejected { .. } => DecisionKind::Rejected,
            GateDecision::Provisional { .. } => DecisionKind::Provisional,
        }
    }

    pub fn to_json(&self) -> String {
        JsonValue::Object(vec![
            (
                "revision".to_owned(),
                JsonValue::Str(self.revision.as_str().to_owned()),
            ),
            (
                "claimed".to_owned(),
                JsonValue::Str(self.claimed.to_string()),
            ),
            ("decision".to_owned(), self.decision.to_json()),
            (
                "at".to_owned(),
                JsonValue::Number(JsonNumber::Uint(self.at.get())),
            ),
        ])
        .to_json_text()
    }

    pub fn decode(text: &str) -> Result<Self, RevisionLedgerError> {
        const FIELDS: &[&str] = &["revision", "claimed", "decision", "at"];
        let document = crate::json::parse(text).map_err(|error| RevisionLedgerError::Corrupt {
            detail: format!("a decision payload is not valid JSON: {error}"),
        })?;
        reject_unknown(&document, FIELDS)?;
        let revision = RevisionId::new(string_field(&document, "revision")?)
            .map_err(corrupt_field("revision"))?;
        let claimed = DecisionKind::parse(string_field(&document, "claimed")?)
            .map_err(corrupt_field("claimed"))?;
        let decision = document
            .get("decision")
            .ok_or_else(|| RevisionLedgerError::Corrupt {
                detail: "the ledger field decision is absent".to_owned(),
            })?;
        let decision = GateDecision::from_json(decision)?;
        let at = EpochSeconds::new(uint_field(&document, "at")?);
        // The decoded record re-validates its own shape: a payload past a
        // final state is corruption, never a decision.
        Self::new(revision, claimed, decision, at).map_err(|error| RevisionLedgerError::Corrupt {
            detail: format!("the stored decision violates its own state machine: {error}"),
        })
    }
}

/// One append-only rollback/restore record (charter D4, the `rollback`
/// piece): the promoted revision's restore transition. `claimed` is the
/// caller's claim that the revision is currently promoted-and-not-yet-
/// rolled-back; the applying ledger validates it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RestoreRecord {
    revision: RevisionId,
    claimed: DecisionKind,
    detail: String,
    at: EpochSeconds,
}

impl RestoreRecord {
    /// Composes a restore record carrying its CLAIMED prior state. The
    /// record type does not gate the claim — the applying LEDGER
    /// validates it against the derived state (only a promoted,
    /// unrestored revision restores), so a never-promoted claim composes
    /// and is refused where it is checked.
    pub fn new(
        revision: RevisionId,
        claimed: DecisionKind,
        detail: String,
        at: EpochSeconds,
    ) -> Self {
        Self {
            revision,
            claimed,
            detail,
            at,
        }
    }

    pub fn revision(&self) -> &RevisionId {
        &self.revision
    }

    pub fn claimed(&self) -> DecisionKind {
        self.claimed
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    pub fn at(&self) -> EpochSeconds {
        self.at
    }

    pub fn to_json(&self) -> String {
        JsonValue::Object(vec![
            (
                "revision".to_owned(),
                JsonValue::Str(self.revision.as_str().to_owned()),
            ),
            (
                "claimed".to_owned(),
                JsonValue::Str(self.claimed.to_string()),
            ),
            ("detail".to_owned(), JsonValue::Str(self.detail.clone())),
            (
                "at".to_owned(),
                JsonValue::Number(JsonNumber::Uint(self.at.get())),
            ),
        ])
        .to_json_text()
    }

    pub fn decode(text: &str) -> Result<Self, RevisionLedgerError> {
        const FIELDS: &[&str] = &["revision", "claimed", "detail", "at"];
        let document = crate::json::parse(text).map_err(|error| RevisionLedgerError::Corrupt {
            detail: format!("a restore payload is not valid JSON: {error}"),
        })?;
        reject_unknown(&document, FIELDS)?;
        let revision = RevisionId::new(string_field(&document, "revision")?)
            .map_err(corrupt_field("revision"))?;
        let claimed = DecisionKind::parse(string_field(&document, "claimed")?)
            .map_err(corrupt_field("claimed"))?;
        let detail = string_field(&document, "detail")?.to_owned();
        let at = EpochSeconds::new(uint_field(&document, "at")?);
        Ok(Self::new(revision, claimed, detail, at))
    }
}

// ---------------------------------------------------------------------------
// The derived register entry and append receipts
// ---------------------------------------------------------------------------

/// A revision's DERIVED entry: the current decision is the latest applied
/// decision record's kind (never a separately mutable column), and
/// `rolled_back` is the EXISTENCE of a restore record — a promoted
/// revision that was restored reads as promoted-but-rolled-back, never
/// silently active.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RevisionEntry {
    pub revision: RevisionId,
    pub decision: Option<DecisionKind>,
    pub rolled_back: bool,
    pub evaluations: usize,
    pub admitted_at: EpochSeconds,
    pub last_decision_at: Option<EpochSeconds>,
}

/// The receipt of one proposal admission: whether the revision was
/// already present with agreeing facts (the idempotent no-op).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProposalAdmission {
    pub idempotent: bool,
}

/// The receipt of one evaluation append.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EvaluationAppend {
    pub sequence: u64,
    pub idempotent: bool,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// `RevisionLedger` failures — typed (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum RevisionLedgerError {
    /// A durable append failed. Nothing was recorded.
    AppendFailed { detail: String },
    /// A read failed.
    ReadFailed { detail: String },
    /// Stored state exists but does not decode into the typed records.
    Corrupt { detail: String },
    /// The same revision was re-admitted with DISAGREEING facts — the
    /// stored proposal stands.
    ProposalConflict { revision: RevisionId },
    /// The same evaluation run-set was re-appended with DISAGREEING
    /// facts — the stored record stands.
    EvaluationConflict { revision: RevisionId },
    /// The revision has no proposal on record.
    UnknownRevision { revision: RevisionId },
    /// A decision record declared a `claimed` state that disagrees with
    /// the derived decision. Nothing was written.
    StaleDecision {
        revision: RevisionId,
        current: DecisionKind,
    },
    /// A restore record declared a claim that disagrees with the derived
    /// state (the revision is not a promoted, unrestored revision).
    /// Nothing was written.
    StaleRestore {
        revision: RevisionId,
        current: DecisionKind,
        rolled_back: bool,
    },
}

impl fmt::Display for RevisionLedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AppendFailed { detail } => write!(f, "ledger append failed: {detail}"),
            Self::ReadFailed { detail } => write!(f, "ledger read failed: {detail}"),
            Self::Corrupt { detail } => write!(f, "the revision ledger is damaged: {detail}"),
            Self::ProposalConflict { revision } => write!(
                f,
                "revision {revision} is already admitted with different declared facts — \
                 the stored proposal stands (the record_run precedent)"
            ),
            Self::EvaluationConflict { revision } => write!(
                f,
                "revision {revision} already carries this evaluation run-set with different \
                 figures — the stored record stands"
            ),
            Self::UnknownRevision { revision } => write!(
                f,
                "revision {revision} has no proposal on record; nothing can be evaluated, \
                 adjudicated, or restored"
            ),
            Self::StaleDecision { revision, current } => write!(
                f,
                "revision {revision} is durably {current}; the decision record claimed a \
                 different prior state — nothing was written"
            ),
            Self::StaleRestore {
                revision,
                current,
                rolled_back,
            } => write!(
                f,
                "revision {revision} is durably {current} (rolled_back={rolled_back}); only a \
                 promoted, unrestored revision restores — nothing was written"
            ),
        }
    }
}

impl std::error::Error for RevisionLedgerError {}

impl From<JsonError> for RevisionLedgerError {
    fn from(error: JsonError) -> Self {
        Self::Corrupt {
            detail: error.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// The port
// ---------------------------------------------------------------------------

/// The §19.3/§19.4 candidate-revision store (spec §6; plan Task 23). The
/// decision state is transition-derived; every method performs durable IO
/// and carries a deadline.
pub trait RevisionLedger {
    /// Admits one proposal (the durable PENDING revision). Idempotent on
    /// the declared [`RevisionId`]: the same revision re-admitted with
    /// agreeing facts is a no-op; a disagreeing re-admission is
    /// [`RevisionLedgerError::ProposalConflict`], and the stored proposal
    /// stands.
    fn admit_proposal(
        &self,
        proposal: RevisionProposal,
        deadline: Deadline,
    ) -> Result<ProposalAdmission, RevisionLedgerError>;

    /// The admitted proposal; `None` for an unknown revision.
    fn proposal(
        &self,
        revision: &RevisionId,
        deadline: Deadline,
    ) -> Result<Option<RevisionProposal>, RevisionLedgerError>;

    /// Appends one isolated-evaluation record. Idempotent on the run-set
    /// identity (the four run ids): the same set re-appended with agreeing
    /// facts is a no-op; a disagreeing re-append is
    /// [`RevisionLedgerError::EvaluationConflict`]. The revision must have
    /// a proposal on record ([`RevisionLedgerError::UnknownRevision`]).
    fn append_evaluation(
        &self,
        record: EvaluationRecord,
        deadline: Deadline,
    ) -> Result<EvaluationAppend, RevisionLedgerError>;

    /// The revision's evaluation records, in append order.
    fn evaluations(
        &self,
        revision: &RevisionId,
        deadline: Deadline,
    ) -> Result<Vec<EvaluationRecord>, RevisionLedgerError>;

    /// Applies one decision record — the ledger's ONLY decision writer.
    /// Refuses an unknown revision, a `claimed` state that disagrees with
    /// the derived decision ([`RevisionLedgerError::StaleDecision`]), and
    /// any record the state machine forbids composing past a final state.
    fn apply_decision(
        &self,
        record: RevisionDecisionRecord,
        deadline: Deadline,
    ) -> Result<RevisionEntry, RevisionLedgerError>;

    /// The revision's applied decision records, in apply order.
    fn decisions(
        &self,
        revision: &RevisionId,
        deadline: Deadline,
    ) -> Result<Vec<RevisionDecisionRecord>, RevisionLedgerError>;

    /// Records one restore transition (append-only). Refuses a revision
    /// that is not currently promoted-and-unrestored
    /// ([`RevisionLedgerError::StaleRestore`]) — a rollback of a
    /// never-promoted revision has nothing to restore.
    fn record_restore(
        &self,
        record: RestoreRecord,
        deadline: Deadline,
    ) -> Result<RevisionEntry, RevisionLedgerError>;

    /// The revision's restore records, in append order.
    fn restores(
        &self,
        revision: &RevisionId,
        deadline: Deadline,
    ) -> Result<Vec<RestoreRecord>, RevisionLedgerError>;

    /// The revision's derived entry; `None` for an unknown revision.
    fn revision(
        &self,
        revision: &RevisionId,
        deadline: Deadline,
    ) -> Result<Option<RevisionEntry>, RevisionLedgerError>;
}

// ---------------------------------------------------------------------------
// Shared JSON helpers (the capability-ledger codec's conventions)
// ---------------------------------------------------------------------------

fn reject_unknown(object: &JsonValue, known: &[&str]) -> Result<(), RevisionLedgerError> {
    for key in object.keys() {
        if !known.contains(&key.as_str()) {
            return Err(RevisionLedgerError::Corrupt {
                detail: format!("the ledger payload carries unknown field {key:?}"),
            });
        }
    }
    Ok(())
}

fn string_field<'a>(
    object: &'a JsonValue,
    field: &'static str,
) -> Result<&'a str, RevisionLedgerError> {
    let leaf = field.rsplit('.').next().unwrap_or(field);
    match object.get(leaf) {
        Some(JsonValue::Str(text)) => Ok(text),
        Some(_) => Err(RevisionLedgerError::Corrupt {
            detail: format!("the ledger field {field:?} is not a string"),
        }),
        None => Err(RevisionLedgerError::Corrupt {
            detail: format!("the ledger field {field:?} is absent"),
        }),
    }
}

fn uint_field(object: &JsonValue, field: &'static str) -> Result<u64, RevisionLedgerError> {
    match object.get(field) {
        Some(JsonValue::Number(JsonNumber::Uint(n))) => Ok(*n),
        _ => Err(RevisionLedgerError::Corrupt {
            detail: format!("the ledger field {field:?} is not a non-negative integer"),
        }),
    }
}

fn uint_in(object: &JsonValue, field: &str) -> Result<u64, RevisionLedgerError> {
    match object.get(field) {
        Some(JsonValue::Number(JsonNumber::Uint(n))) => Ok(*n),
        _ => Err(RevisionLedgerError::Corrupt {
            detail: format!("the ledger object carries no {field} figure"),
        }),
    }
}

fn strings_field(
    object: &JsonValue,
    field: &'static str,
) -> Result<Vec<String>, RevisionLedgerError> {
    match object.get(field) {
        Some(JsonValue::Array(items)) => items
            .iter()
            .map(|item| match item {
                JsonValue::Str(text) => Ok(text.clone()),
                _ => Err(RevisionLedgerError::Corrupt {
                    detail: format!("the ledger field {field:?} carries a non-string"),
                }),
            })
            .collect(),
        _ => Err(RevisionLedgerError::Corrupt {
            detail: format!("the ledger field {field:?} is an array of strings"),
        }),
    }
}

fn strings_in(object: &JsonValue, field: &str) -> Result<Vec<String>, RevisionLedgerError> {
    match object.get(field) {
        Some(JsonValue::Array(items)) => items
            .iter()
            .map(|item| match item {
                JsonValue::Str(text) => Ok(text.clone()),
                _ => Err(RevisionLedgerError::Corrupt {
                    detail: format!("the ledger field {field:?} carries a non-string"),
                }),
            })
            .collect(),
        _ => Err(RevisionLedgerError::Corrupt {
            detail: format!("the ledger field {field:?} is an array of strings"),
        }),
    }
}

fn refs_field(
    object: &JsonValue,
    field: &'static str,
) -> Result<Vec<EvidenceRef>, RevisionLedgerError> {
    match object.get(field) {
        Some(JsonValue::Array(items)) => items
            .iter()
            .map(|item| match item {
                JsonValue::Str(text) => EvidenceRef::new(text).map_err(corrupt_field(field)),
                _ => Err(RevisionLedgerError::Corrupt {
                    detail: format!("the ledger field {field:?} carries a non-ref"),
                }),
            })
            .collect(),
        _ => Err(RevisionLedgerError::Corrupt {
            detail: format!("the ledger field {field:?} is an array of evidence refs"),
        }),
    }
}

fn ref_field(object: &JsonValue, field: &'static str) -> Result<EvidenceRef, RevisionLedgerError> {
    EvidenceRef::new(string_field(object, field)?).map_err(corrupt_field(field))
}

fn ref_in(object: &JsonValue, field: &str) -> Result<EvidenceRef, RevisionLedgerError> {
    match object.get(field) {
        Some(JsonValue::Str(text)) => EvidenceRef::new(text).map_err(corrupt_field("evidence ref")),
        _ => Err(RevisionLedgerError::Corrupt {
            detail: format!("the ledger object carries no {field} ref"),
        }),
    }
}

fn refs_json(refs: &[EvidenceRef]) -> JsonValue {
    JsonValue::Array(
        refs.iter()
            .map(|reference| JsonValue::Str(reference.as_str().to_owned()))
            .collect(),
    )
}

/// Maps any typed value-construction error onto a `Corrupt` carrying the
/// field locus.
fn corrupt_field<E: fmt::Display>(field: &'static str) -> impl Fn(E) -> RevisionLedgerError {
    move |error| RevisionLedgerError::Corrupt {
        detail: format!("the ledger field {field:?} is invalid: {error}"),
    }
}
