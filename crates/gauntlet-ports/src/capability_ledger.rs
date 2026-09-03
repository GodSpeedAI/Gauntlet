//! `CapabilityLedger` — spec §19.1's two durable stores with opposite write
//! policies, in one port (plan Task 22, charter D1):
//!
//! > | **Developmental memory** | Every attempt, failure, correction,
//! >   comparison, payment, settlement, recovery. Raw. | Immediate,
//! >   append-only |
//! > | **Capability ledger** | Promoted skills, shortcuts, routes, prompts,
//! >   model-routing rules. | Governed promotion only |
//!
//! - The **observation log** ([`CapabilityLedger::append_observation`]) is
//!   the developmental-memory substrate: any caller appends any class of
//!   observation at any time — a failed attempt is still memory — and the
//!   append is IDEMPOTENT on the observation's declared
//!   [`ObservationIdentity`] (the `record_run` precedent: the same identity
//!   re-appended with the same facts is a no-op; a re-append that DISAGREES
//!   on any fact is a typed [`CapabilityLedgerError::ObservationConflict`],
//!   never a silent overwrite).
//! - The **capability register** ([`CapabilityLedger::capability`]) carries
//!   `CapabilityId` → [`CapabilityStatus`] and rises ONLY through
//!   first-class [`StatusTransition`] records
//!   ([`CapabilityLedger::apply_transition`]). There is deliberately NO
//!   `set_status` method: elevation is earned by composing a typed
//!   transition record — the app-layer promotion service is the composer
//!   that runs the §19.2 conditions — and the record itself refuses to
//!   compose a transition the status state machine forbids
//!   ([`StatusTransition::new`], the domain `Capability::restatus` table).
//!
//! ## The observation row records exactly the facts available
//!
//! [`ObservationRecord`] is one flat typed row covering every §19.1 left
//! column class ([`ObservationClass`]); the fields a class does not carry
//! stay `None`. Nothing here is invented: a class with no durable reader in
//! the substrate (role invocations, comparison documents) is projected by
//! its importer from what the substrate CAN read or not at all — never
//! fabricated. The content axes the §19.2 variation condition compares are
//! computed by [`ObservationRecord::axes`] from the row's own populated
//! fields ([`VariationAxis`]), so a row can never claim axes its facts do
//! not carry.
//!
//! `at` is the observation's RECORDING instant (the appender's `Clock`
//! reading), never a claimed fact of the substrate: identity idempotency
//! excludes it, so an honest re-import at a later instant is a no-op.
//! [`ObservationRecord::occurred_at`], where the substrate names one, is
//! the fact's OWN occurrence instant (for example a verification record's
//! `produced_at`) — a substrate fact, carried through idempotency like
//! every other fact. Window decisions order by SUBSTRATE CHRONOLOGY —
//! `(occurred_at, ledger seq)` — not by import order: runs complete and
//! import out of chronological order under parallelism. The honest
//! boundary (COR-2 C7): the seq tiebreak is deterministic but IS append
//! order, so an `occurred_at` tie between decisive observations is
//! order-dependent by construction — a recorded residual, not a claim
//! this ordering retires.
//!
//! ## Tracked dimensions (spec §19.2)
//!
//! performance · variation · transfer · recovery · orchestration burden ·
//! settlement reliability · failure rate · gate erosion · stability. The
//! register records what the §19.2 promotion decision must name:
//! [`PromotionEvidence`] is the durable decision record; [`PromotionRefusal`]
//! is the durable per-condition refusal record (§19.4: insufficient
//! evidence ⟹ provisional ⟹ not promoted).
//!
//! Every method takes a [`Deadline`] (spec §6 preamble). JSON codecs are
//! hand-rolled over [`crate::json`] (the workspace has no serde); decoding
//! is strict, and a stored payload that does not decode is
//! [`CapabilityLedgerError::Corrupt`], never a degraded record.

use core::fmt;

use gauntlet_domain::{
    CapabilityId, CapabilityStatus, Digest, EpochSeconds, EvidenceKind, EvidenceLocator,
    EvidenceRef, MicroUsd, Outcome, RunId, Spent, UnitId, VerificationRef,
};

use crate::deadline::Deadline;
use crate::json::{JsonError, JsonNumber, JsonValue};

// The shared contract suite lives in `crate::contract::capability_ledger`;
// this module is the port itself.

// ---------------------------------------------------------------------------
// Observation identity
// ---------------------------------------------------------------------------

/// An observation's declared identity — the idempotency key of the
/// developmental-memory log (charter D1). `fact` is the caller's stable
/// within-run discriminator of the recorded fact (for example the
/// verification ref a verification outcome was decided under); two
/// appends of the same identity are the same memory.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct ObservationIdentity {
    capability: CapabilityId,
    class: ObservationClass,
    run: RunId,
    fact: String,
}

/// `ObservationIdentity` construction failures.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservationIdentityError {
    /// The within-run fact discriminator was empty — an identity that
    /// cannot discriminate is not an identity.
    EmptyFact,
}

impl fmt::Display for ObservationIdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyFact => write!(
                f,
                "the observation identity's fact discriminator must not be empty"
            ),
        }
    }
}

impl std::error::Error for ObservationIdentityError {}

impl ObservationIdentity {
    pub fn new(
        capability: CapabilityId,
        class: ObservationClass,
        run: RunId,
        fact: String,
    ) -> Result<Self, ObservationIdentityError> {
        if fact.is_empty() {
            return Err(ObservationIdentityError::EmptyFact);
        }
        Ok(Self {
            capability,
            class,
            run,
            fact,
        })
    }

    pub fn capability(&self) -> &CapabilityId {
        &self.capability
    }

    pub fn class(&self) -> ObservationClass {
        self.class
    }

    pub fn run(&self) -> &RunId {
        &self.run
    }

    pub fn fact(&self) -> &str {
        &self.fact
    }
}

/// The §19.1 developmental-memory classes — every class of run fact the
/// left column names. The vocabulary is the spec's, exact-match parsed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ObservationClass {
    Attempt,
    Failure,
    Correction,
    Comparison,
    Payment,
    Settlement,
    Recovery,
}

impl ObservationClass {
    /// Every class, for exhaustive tests.
    pub const ALL: &'static [Self] = &[
        Self::Attempt,
        Self::Failure,
        Self::Correction,
        Self::Comparison,
        Self::Payment,
        Self::Settlement,
        Self::Recovery,
    ];

    /// The wire token, exact-match parsed (spec §5.2 rule: unknown tokens
    /// are parse errors, never coerced defaults).
    pub fn parse(input: &str) -> Result<Self, CapabilityLedgerError> {
        match input {
            "attempt" => Ok(Self::Attempt),
            "failure" => Ok(Self::Failure),
            "correction" => Ok(Self::Correction),
            "comparison" => Ok(Self::Comparison),
            "payment" => Ok(Self::Payment),
            "settlement" => Ok(Self::Settlement),
            "recovery" => Ok(Self::Recovery),
            other => Err(CapabilityLedgerError::Corrupt {
                detail: format!("observation class token {other:?} is outside the 19.1 vocabulary"),
            }),
        }
    }
}

impl fmt::Display for ObservationClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let token = match self {
            Self::Attempt => "attempt",
            Self::Failure => "failure",
            Self::Correction => "correction",
            Self::Comparison => "comparison",
            Self::Payment => "payment",
            Self::Settlement => "settlement",
            Self::Recovery => "recovery",
        };
        f.write_str(token)
    }
}

// ---------------------------------------------------------------------------
// Variation axes (the §19.2 recorded-variation content classes)
// ---------------------------------------------------------------------------

/// One CONTENT axis a settlement observation contributes to the §19.2
/// recorded-variation condition (charter D4). The set is exactly the
/// durable-derivable content classes: unit identity, the frozen reference
/// the run settled against, and the evidence requirement's kind and
/// locator class. `run_id` and timestamps are deliberately NOT axes —
/// they differ across every run, which is precisely the false-variation
/// trap AK-055 names.
///
/// Recorded mapping: D4 also names blast radius/tier as axis classes, but
/// no `StateStore` read the developmental import projects carries a unit's
/// blast radius or tier, and facts the substrate cannot read are never
/// invented (charter D3). The typed set therefore omits them — fail-closed,
/// because fewer axes make variation harder to prove, never easier.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VariationAxis {
    Unit(UnitId),
    ReferenceDigest(Digest),
    EvidenceKind(EvidenceKind),
    EvidenceLocator(EvidenceLocator),
}

impl VariationAxis {
    /// A deterministic sort key (the union recorded in
    /// [`PromotionEvidence`] is sorted, so the decision record's bytes are
    /// a function of the values alone).
    pub fn sort_key(&self) -> String {
        match self {
            Self::Unit(unit) => format!("unit:{}", unit),
            Self::ReferenceDigest(digest) => format!("reference:{digest}"),
            Self::EvidenceKind(kind) => format!("kind:{kind}"),
            Self::EvidenceLocator(locator) => format!("locator:{}", locator.as_str()),
        }
    }

    fn to_json(&self) -> JsonValue {
        let (axis, value) = match self {
            Self::Unit(unit) => ("unit", unit.as_str().to_owned()),
            Self::ReferenceDigest(digest) => ("reference_digest", digest.to_string()),
            Self::EvidenceKind(kind) => ("evidence_kind", kind.to_string()),
            Self::EvidenceLocator(locator) => ("evidence_locator", locator.as_str().to_owned()),
        };
        JsonValue::Object(vec![
            ("axis".to_owned(), JsonValue::Str(axis.to_owned())),
            ("value".to_owned(), JsonValue::Str(value)),
        ])
    }

    fn from_json(value: &JsonValue) -> Result<Self, CapabilityLedgerError> {
        let JsonValue::Object(_) = value else {
            return Err(CapabilityLedgerError::Corrupt {
                detail: "a variation axis is an object".to_owned(),
            });
        };
        let axis = string_field(value, "axis")?;
        let raw = string_field(value, "value")?;
        match axis {
            "unit" => Ok(Self::Unit(
                UnitId::new(raw).map_err(corrupt_field("unit axis"))?,
            )),
            "reference_digest" => Ok(Self::ReferenceDigest(
                Digest::parse(raw).map_err(corrupt_field("reference axis"))?,
            )),
            "evidence_kind" => Ok(Self::EvidenceKind(
                EvidenceKind::parse(raw).map_err(corrupt_field("evidence-kind axis"))?,
            )),
            "evidence_locator" => Ok(Self::EvidenceLocator(
                EvidenceLocator::new(raw).map_err(corrupt_field("evidence-locator axis"))?,
            )),
            other => Err(CapabilityLedgerError::Corrupt {
                detail: format!("variation axis token {other:?} is outside the typed set"),
            }),
        }
    }
}

impl fmt::Display for VariationAxis {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.sort_key())
    }
}

// ---------------------------------------------------------------------------
// Observation record
// ---------------------------------------------------------------------------

/// One developmental-memory row (spec §19.1 left column). The fields a
/// class does not carry stay `None` — the row records exactly the facts
/// its appender had, never invented ones (charter D3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationRecord {
    identity: ObservationIdentity,
    /// The RECORDING instant (the appender's `Clock` reading). Excluded
    /// from identity idempotency; never a claimed substrate fact.
    at: EpochSeconds,
    /// The fact's own occurrence instant in the substrate, when the
    /// substrate names one (a verification record's `produced_at` for the
    /// attempt/failure/settlement classes). A substrate fact like any
    /// other: part of identity idempotency, and the chronology every
    /// window decision orders by. Classes the substrate assigns no
    /// occurrence instant carry `None` — a decided window with a `None`
    /// row is undecidable and refuses, never passes.
    occurred_at: Option<EpochSeconds>,
    unit: Option<UnitId>,
    outcome: Option<Outcome>,
    /// The verifying instrument's §14.2 model-rung flag, when the fact's
    /// decision names an instrument (the gate-erosion condition's input).
    instrument_deterministic: Option<bool>,
    evidence_kind: Option<EvidenceKind>,
    evidence_locator: Option<EvidenceLocator>,
    /// The frozen reference the fact's run settled against.
    reference_digest: Option<Digest>,
    /// The verification the fact was decided under, when one exists.
    verification: Option<VerificationRef>,
    /// The pinned evidence the fact was decided over, when the substrate
    /// names it (settlements carry their committed evidence).
    evidence: Option<EvidenceRef>,
    /// The run's correction-selection count at recording time — the typed
    /// orchestration-burden proxy (charter D4 condition 5).
    orchestration_cost: Option<u64>,
    /// The durable spend counters the fact's checkpoint carried (payments).
    spend: Option<Spent>,
    detail: String,
}

impl ObservationRecord {
    /// Assembles a row. Every fact field is optional; the identity and the
    /// recording instant are required.
    #[allow(clippy::too_many_arguments)]
    pub fn new(identity: ObservationIdentity, at: EpochSeconds, detail: String) -> Self {
        Self {
            identity,
            at,
            occurred_at: None,
            unit: None,
            outcome: None,
            instrument_deterministic: None,
            evidence_kind: None,
            evidence_locator: None,
            reference_digest: None,
            verification: None,
            evidence: None,
            orchestration_cost: None,
            spend: None,
            detail,
        }
    }

    pub fn with_unit(mut self, unit: UnitId) -> Self {
        self.unit = Some(unit);
        self
    }

    /// Names the fact's own occurrence instant in the substrate (a
    /// verification record's `produced_at`). Only for classes whose
    /// occurrence the substrate actually records — never invented.
    pub fn with_occurred_at(mut self, occurred_at: EpochSeconds) -> Self {
        self.occurred_at = Some(occurred_at);
        self
    }

    pub fn with_outcome(mut self, outcome: Outcome) -> Self {
        self.outcome = Some(outcome);
        self
    }

    pub fn with_instrument_deterministic(mut self, deterministic: bool) -> Self {
        self.instrument_deterministic = Some(deterministic);
        self
    }

    pub fn with_evidence_kind(mut self, kind: EvidenceKind) -> Self {
        self.evidence_kind = Some(kind);
        self
    }

    pub fn with_evidence_locator(mut self, locator: EvidenceLocator) -> Self {
        self.evidence_locator = Some(locator);
        self
    }

    pub fn with_reference_digest(mut self, digest: Digest) -> Self {
        self.reference_digest = Some(digest);
        self
    }

    pub fn with_verification(mut self, verification: VerificationRef) -> Self {
        self.verification = Some(verification);
        self
    }

    pub fn with_evidence(mut self, evidence: EvidenceRef) -> Self {
        self.evidence = Some(evidence);
        self
    }

    pub fn with_orchestration_cost(mut self, cost: u64) -> Self {
        self.orchestration_cost = Some(cost);
        self
    }

    pub fn with_spend(mut self, spend: Spent) -> Self {
        self.spend = Some(spend);
        self
    }

    /// Rebuilds the row with a different detail (the disagreeing re-append
    /// case's builder).
    pub fn with_detail(mut self, detail: String) -> Self {
        self.detail = detail;
        self
    }

    pub fn identity(&self) -> &ObservationIdentity {
        &self.identity
    }

    /// The recording instant.
    pub fn at(&self) -> EpochSeconds {
        self.at
    }

    /// The fact's own occurrence instant in the substrate, when named.
    pub fn occurred_at(&self) -> Option<EpochSeconds> {
        self.occurred_at
    }

    pub fn unit(&self) -> Option<UnitId> {
        self.unit.clone()
    }

    pub fn outcome(&self) -> Option<Outcome> {
        self.outcome
    }

    pub fn instrument_deterministic(&self) -> Option<bool> {
        self.instrument_deterministic
    }

    pub fn evidence_kind(&self) -> Option<EvidenceKind> {
        self.evidence_kind
    }

    pub fn evidence_locator(&self) -> Option<EvidenceLocator> {
        self.evidence_locator.clone()
    }

    pub fn reference_digest(&self) -> Option<&Digest> {
        self.reference_digest.as_ref()
    }

    pub fn verification(&self) -> Option<&VerificationRef> {
        self.verification.as_ref()
    }

    /// The pinned evidence the fact was decided over, when named.
    pub fn evidence(&self) -> Option<&EvidenceRef> {
        self.evidence.as_ref()
    }

    pub fn orchestration_cost(&self) -> Option<u64> {
        self.orchestration_cost
    }

    pub fn spend(&self) -> Option<&Spent> {
        self.spend.as_ref()
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    /// The CONTENT axes this row contributes to the §19.2 recorded-variation
    /// condition — computed from the row's own populated fields, so a row
    /// can never claim axes its facts do not carry.
    pub fn axes(&self) -> Vec<VariationAxis> {
        let mut axes = Vec::new();
        if let Some(unit) = &self.unit {
            axes.push(VariationAxis::Unit(unit.clone()));
        }
        if let Some(digest) = &self.reference_digest {
            axes.push(VariationAxis::ReferenceDigest(digest.clone()));
        }
        if let Some(kind) = &self.evidence_kind {
            axes.push(VariationAxis::EvidenceKind(*kind));
        }
        if let Some(locator) = &self.evidence_locator {
            axes.push(VariationAxis::EvidenceLocator(locator.clone()));
        }
        axes
    }

    /// Whether two rows agree on every FACT (everything but the recording
    /// instant). Same identity + agreeing facts is the idempotent no-op;
    /// same identity + disagreeing facts is
    /// [`CapabilityLedgerError::ObservationConflict`].
    pub fn facts_agree(&self, other: &ObservationRecord) -> bool {
        self.unit == other.unit
            && self.outcome == other.outcome
            && self.occurred_at == other.occurred_at
            && self.instrument_deterministic == other.instrument_deterministic
            && self.evidence_kind == other.evidence_kind
            && self.evidence_locator == other.evidence_locator
            && self.reference_digest == other.reference_digest
            && self.verification == other.verification
            && self.evidence == other.evidence
            && self.orchestration_cost == other.orchestration_cost
            && self.spend == other.spend
            && self.detail == other.detail
    }

    pub fn to_json(&self) -> String {
        let optional_str = |value: &Option<String>| {
            value
                .as_ref()
                .map_or(JsonValue::Null, |text| JsonValue::Str(text.clone()))
        };
        let unit = self.unit.as_ref().map(|unit| unit.as_str().to_owned());
        let outcome = self.outcome.as_ref().map(|outcome| outcome.to_string());
        let kind = self.evidence_kind.as_ref().map(|kind| kind.to_string());
        let locator = self
            .evidence_locator
            .as_ref()
            .map(|locator| locator.as_str().to_owned());
        let reference = self
            .reference_digest
            .as_ref()
            .map(|digest| digest.to_string());
        let verification = self
            .verification
            .as_ref()
            .map(|verification| verification.as_str().to_owned());
        let spend = self.spend.as_ref().map(spend_json);
        JsonValue::Object(vec![
            (
                "capability".to_owned(),
                JsonValue::Str(self.identity.capability.as_str().to_owned()),
            ),
            (
                "class".to_owned(),
                JsonValue::Str(self.identity.class.to_string()),
            ),
            (
                "run_id".to_owned(),
                JsonValue::Str(self.identity.run.as_str().to_owned()),
            ),
            (
                "fact".to_owned(),
                JsonValue::Str(self.identity.fact.clone()),
            ),
            (
                "at".to_owned(),
                JsonValue::Number(JsonNumber::Uint(self.at.get())),
            ),
            (
                "occurred_at".to_owned(),
                self.occurred_at.map_or(JsonValue::Null, |instant| {
                    JsonValue::Number(JsonNumber::Uint(instant.get()))
                }),
            ),
            ("unit_id".to_owned(), optional_str(&unit)),
            ("outcome".to_owned(), optional_str(&outcome)),
            (
                "instrument_deterministic".to_owned(),
                self.instrument_deterministic
                    .map_or(JsonValue::Null, JsonValue::Bool),
            ),
            ("evidence_kind".to_owned(), optional_str(&kind)),
            ("evidence_locator".to_owned(), optional_str(&locator)),
            ("reference_digest".to_owned(), optional_str(&reference)),
            ("verification".to_owned(), optional_str(&verification)),
            (
                "evidence_ref".to_owned(),
                self.evidence.as_ref().map_or(JsonValue::Null, |reference| {
                    JsonValue::Str(reference.as_str().to_owned())
                }),
            ),
            (
                "orchestration_cost".to_owned(),
                self.orchestration_cost.map_or(JsonValue::Null, |cost| {
                    JsonValue::Number(JsonNumber::Uint(cost))
                }),
            ),
            ("spend".to_owned(), spend.unwrap_or(JsonValue::Null)),
            ("detail".to_owned(), JsonValue::Str(self.detail.clone())),
        ])
        .to_json_text()
    }

    /// Strict decode of the canonical row bytes: unknown keys are rejected,
    /// and a payload that does not decode is `Corrupt`, never a degraded
    /// record.
    pub fn decode(text: &str) -> Result<Self, CapabilityLedgerError> {
        const FIELDS: &[&str] = &[
            "capability",
            "class",
            "run_id",
            "fact",
            "at",
            "occurred_at",
            "unit_id",
            "outcome",
            "instrument_deterministic",
            "evidence_kind",
            "evidence_locator",
            "reference_digest",
            "verification",
            "evidence_ref",
            "orchestration_cost",
            "spend",
            "detail",
        ];
        let document =
            crate::json::parse(text).map_err(|error| CapabilityLedgerError::Corrupt {
                detail: format!("an observation payload is not valid JSON: {error}"),
            })?;
        let JsonValue::Object(_) = &document else {
            return Err(CapabilityLedgerError::Corrupt {
                detail: "an observation payload is an object".to_owned(),
            });
        };
        reject_unknown(&document, FIELDS)?;
        let capability = CapabilityId::new(string_field(&document, "capability")?)
            .map_err(corrupt_field("capability"))?;
        let class = ObservationClass::parse(string_field(&document, "class")?)?;
        let run =
            RunId::new(string_field(&document, "run_id")?).map_err(corrupt_field("run_id"))?;
        let fact = string_field(&document, "fact")?.to_owned();
        let identity = ObservationIdentity::new(capability, class, run, fact).map_err(|error| {
            CapabilityLedgerError::Corrupt {
                detail: error.to_string(),
            }
        })?;
        let at = uint_field(&document, "at")?;
        let mut record = ObservationRecord::new(
            identity,
            EpochSeconds::new(at),
            string_field(&document, "detail")?.to_owned(),
        );
        if !is_null(&document, "occurred_at") {
            record.occurred_at = Some(EpochSeconds::new(uint_field(&document, "occurred_at")?));
        }
        if !is_null(&document, "unit_id") {
            record.unit = Some(
                UnitId::new(string_field(&document, "unit_id")?)
                    .map_err(corrupt_field("unit_id"))?,
            );
        }
        if !is_null(&document, "outcome") {
            record.outcome = Some(
                Outcome::parse(string_field(&document, "outcome")?)
                    .map_err(corrupt_field("outcome"))?,
            );
        }
        match document.get("instrument_deterministic") {
            None | Some(JsonValue::Null) => {}
            Some(JsonValue::Bool(flag)) => record.instrument_deterministic = Some(*flag),
            Some(_) => {
                return Err(CapabilityLedgerError::Corrupt {
                    detail: "instrument_deterministic is null or a boolean".to_owned(),
                });
            }
        }
        if !is_null(&document, "evidence_kind") {
            record.evidence_kind = Some(
                EvidenceKind::parse(string_field(&document, "evidence_kind")?)
                    .map_err(corrupt_field("evidence_kind"))?,
            );
        }
        if !is_null(&document, "evidence_locator") {
            record.evidence_locator = Some(
                EvidenceLocator::new(string_field(&document, "evidence_locator")?)
                    .map_err(corrupt_field("evidence_locator"))?,
            );
        }
        if !is_null(&document, "reference_digest") {
            record.reference_digest = Some(
                Digest::parse(string_field(&document, "reference_digest")?)
                    .map_err(corrupt_field("reference_digest"))?,
            );
        }
        if !is_null(&document, "verification") {
            record.verification = Some(
                VerificationRef::new(string_field(&document, "verification")?)
                    .map_err(corrupt_field("verification"))?,
            );
        }
        if !is_null(&document, "evidence_ref") {
            record.evidence = Some(
                EvidenceRef::new(string_field(&document, "evidence_ref")?)
                    .map_err(corrupt_field("evidence_ref"))?,
            );
        }
        match document.get("orchestration_cost") {
            None | Some(JsonValue::Null) => {}
            Some(JsonValue::Number(JsonNumber::Uint(cost))) => {
                record.orchestration_cost = Some(*cost);
            }
            Some(_) => {
                return Err(CapabilityLedgerError::Corrupt {
                    detail: "orchestration_cost is null or a non-negative integer".to_owned(),
                });
            }
        }
        match document.get("spend") {
            None | Some(JsonValue::Null) => {}
            Some(spend @ JsonValue::Object(_)) => {
                let rounds = uint_field(spend, "rounds")?;
                let wall_seconds = uint_field(spend, "wall_seconds")?;
                let tokens = uint_field(spend, "tokens")?;
                let cost_usd_micros = uint_field(spend, "cost_usd_micros")?;
                let diagnostic_tokens = uint_field(spend, "diagnostic_tokens")?;
                let diagnostic_cost_usd_micros = uint_field(spend, "diagnostic_cost_usd_micros")?;
                record.spend = Some(Spent::from_counters(
                    u32::try_from(rounds).map_err(corrupt_field("spend.rounds"))?,
                    wall_seconds,
                    tokens,
                    MicroUsd::new(cost_usd_micros),
                    diagnostic_tokens,
                    MicroUsd::new(diagnostic_cost_usd_micros),
                ));
            }
            Some(_) => {
                return Err(CapabilityLedgerError::Corrupt {
                    detail: "spend is null or an object".to_owned(),
                });
            }
        }
        Ok(record)
    }
}

// ---------------------------------------------------------------------------
// Promotion refusals and the decision record
// ---------------------------------------------------------------------------

/// One §19.2/§19.4 condition refusal, typed (charter D4). The vocabulary is
/// the five promotion conditions plus the fail-closed evidence gaps: a
/// window that cannot decide a condition refuses, it never passes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PromotionRefusal {
    /// Fewer settlement-class observations than the configured minimum.
    InsufficientSettlements { have: usize, need: u32 },
    /// A settlement observation in the window is missing decision facts
    /// (the unit, the frozen reference, the evidence kind or locator, the
    /// verifying instrument, the orchestration cost, the verification ref,
    /// the pinned evidence, or the substrate occurrence instant). A sparse
    /// window cannot decide the settlement conditions: it refuses — naming
    /// the missing classes — it never passes, and it never credits
    /// variation. Absence is not variation.
    InsufficientSettlementEvidence {
        /// The missing fact classes, sorted and deduplicated.
        missing: Vec<String>,
    },
    /// The settlement window's content axes are identical — the variation
    /// is not real (AK-055's teeth; the plan's Done-when).
    InsufficientVariation,
    /// No recovery-class observation on record.
    NoRecoveryObserved,
    /// The outcome window has no decided attempts (or a window half has
    /// none): failure rate reads as insufficient evidence, never a pass.
    InsufficientOutcomeEvidence,
    /// The failure rate rose from the window's earlier half to its later
    /// half. Figures are (failures, decided) per half.
    FailureRateIncreased {
        earlier_failures: u64,
        earlier_decided: u64,
        later_failures: u64,
        later_decided: u64,
    },
    /// A settlement window missing any settlement's instrument fact — the
    /// erosion condition cannot decide, so it refuses.
    InsufficientInstrumentEvidence,
    /// A later settlement was verified by a strictly weaker instrument
    /// class than an earlier one: the settlements got easier because
    /// verification got weaker. A regression, not learning.
    GateErosion,
    /// A settlement window missing any settlement's orchestration-cost
    /// proxy — the burden condition cannot decide, so it refuses.
    InsufficientBurdenEvidence,
    /// Per-settlement orchestration cost increased across the window.
    OrchestrationBurdenIncreased { earlier: u64, later: u64 },
}

impl PromotionRefusal {
    /// Every refusal, in the canonical order the service records them.
    pub const ALL_RULES: &'static [&'static str] = &[
        "insufficient_settlements",
        "insufficient_settlement_evidence",
        "insufficient_variation",
        "no_recovery_observed",
        "insufficient_outcome_evidence",
        "failure_rate_increased",
        "insufficient_instrument_evidence",
        "gate_erosion",
        "insufficient_burden_evidence",
        "orchestration_burden_increased",
    ];

    /// The refusal's wire token.
    pub fn rule(&self) -> &'static str {
        match self {
            Self::InsufficientSettlements { .. } => "insufficient_settlements",
            Self::InsufficientSettlementEvidence { .. } => "insufficient_settlement_evidence",
            Self::InsufficientVariation => "insufficient_variation",
            Self::NoRecoveryObserved => "no_recovery_observed",
            Self::InsufficientOutcomeEvidence => "insufficient_outcome_evidence",
            Self::FailureRateIncreased { .. } => "failure_rate_increased",
            Self::InsufficientInstrumentEvidence => "insufficient_instrument_evidence",
            Self::GateErosion => "gate_erosion",
            Self::InsufficientBurdenEvidence => "insufficient_burden_evidence",
            Self::OrchestrationBurdenIncreased { .. } => "orchestration_burden_increased",
        }
    }

    fn to_json(&self) -> JsonValue {
        let mut object = vec![("rule".to_owned(), JsonValue::Str(self.rule().to_owned()))];
        match self {
            Self::InsufficientSettlements { have, need } => {
                object.push((
                    "have".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(*have as u64)),
                ));
                object.push((
                    "need".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(u64::from(*need))),
                ));
            }
            Self::InsufficientSettlementEvidence { missing } => {
                object.push((
                    "missing".to_owned(),
                    JsonValue::Array(
                        missing
                            .iter()
                            .map(|class| JsonValue::Str(class.clone()))
                            .collect(),
                    ),
                ));
            }
            Self::FailureRateIncreased {
                earlier_failures,
                earlier_decided,
                later_failures,
                later_decided,
            } => {
                for (name, value) in [
                    ("earlier_failures", *earlier_failures),
                    ("earlier_decided", *earlier_decided),
                    ("later_failures", *later_failures),
                    ("later_decided", *later_decided),
                ] {
                    object.push((name.to_owned(), JsonValue::Number(JsonNumber::Uint(value))));
                }
            }
            Self::OrchestrationBurdenIncreased { earlier, later } => {
                object.push((
                    "earlier".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(*earlier)),
                ));
                object.push((
                    "later".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(*later)),
                ));
            }
            Self::InsufficientVariation
            | Self::NoRecoveryObserved
            | Self::InsufficientOutcomeEvidence
            | Self::InsufficientInstrumentEvidence
            | Self::GateErosion
            | Self::InsufficientBurdenEvidence => {}
        }
        JsonValue::Object(object)
    }

    fn from_json(value: &JsonValue) -> Result<Self, CapabilityLedgerError> {
        let JsonValue::Object(_) = value else {
            return Err(CapabilityLedgerError::Corrupt {
                detail: "a promotion refusal is an object".to_owned(),
            });
        };
        let rule = string_field(value, "rule")?;
        let uint_of = |name: &str| -> Result<u64, CapabilityLedgerError> {
            match value.get(name) {
                Some(JsonValue::Number(JsonNumber::Uint(n))) => Ok(*n),
                _ => Err(CapabilityLedgerError::Corrupt {
                    detail: format!("the {rule} refusal carries no {name} figure"),
                }),
            }
        };
        match rule {
            "insufficient_settlements" => Ok(Self::InsufficientSettlements {
                have: usize::try_from(uint_of("have")?).map_err(corrupt_field("have"))?,
                need: u32::try_from(uint_of("need")?).map_err(corrupt_field("need"))?,
            }),
            "insufficient_settlement_evidence" => {
                let missing = match value.get("missing") {
                    Some(JsonValue::Array(items)) => items
                        .iter()
                        .map(|item| match item {
                            JsonValue::Str(text) => Ok(text.clone()),
                            _ => Err(CapabilityLedgerError::Corrupt {
                                detail: "the missing-fact list carries a non-string".to_owned(),
                            }),
                        })
                        .collect::<Result<Vec<_>, _>>()?,
                    _ => {
                        return Err(CapabilityLedgerError::Corrupt {
                            detail: "the insufficient-settlement-evidence refusal carries no \
                             missing-fact list"
                                .to_owned(),
                        });
                    }
                };
                Ok(Self::InsufficientSettlementEvidence { missing })
            }
            "insufficient_variation" => Ok(Self::InsufficientVariation),
            "no_recovery_observed" => Ok(Self::NoRecoveryObserved),
            "insufficient_outcome_evidence" => Ok(Self::InsufficientOutcomeEvidence),
            "failure_rate_increased" => Ok(Self::FailureRateIncreased {
                earlier_failures: uint_of("earlier_failures")?,
                earlier_decided: uint_of("earlier_decided")?,
                later_failures: uint_of("later_failures")?,
                later_decided: uint_of("later_decided")?,
            }),
            "insufficient_instrument_evidence" => Ok(Self::InsufficientInstrumentEvidence),
            "gate_erosion" => Ok(Self::GateErosion),
            "insufficient_burden_evidence" => Ok(Self::InsufficientBurdenEvidence),
            "orchestration_burden_increased" => Ok(Self::OrchestrationBurdenIncreased {
                earlier: uint_of("earlier")?,
                later: uint_of("later")?,
            }),
            other => Err(CapabilityLedgerError::Corrupt {
                detail: format!("refusal rule token {other:?} is outside the typed vocabulary"),
            }),
        }
    }
}

impl fmt::Display for PromotionRefusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InsufficientSettlements { have, need } => write!(
                f,
                "insufficient settlements: {have} on record where {need} are required — one \
                 successful run is not capability (spec 19.2)"
            ),
            Self::InsufficientVariation => write!(
                f,
                "insufficient variation: the settlements' content axes are identical, so the \
                 variation is not real (spec 19.2; AK-055)"
            ),
            Self::InsufficientSettlementEvidence { missing } => write!(
                f,
                "insufficient settlement evidence: the settlement window is missing recorded \
                 facts ({}) — a sparse window cannot decide the settlement conditions, so it \
                 refuses; absence is not variation (spec 19.2)",
                missing.join(", ")
            ),
            Self::NoRecoveryObserved => write!(
                f,
                "no recovery event on record (spec 19.2: plus a recovery event handled)"
            ),
            Self::InsufficientOutcomeEvidence => write!(
                f,
                "the outcome window cannot decide the failure rate — insufficient evidence \
                 reads as a refusal, never a pass (spec 19.2)"
            ),
            Self::FailureRateIncreased {
                earlier_failures,
                earlier_decided,
                later_failures,
                later_decided,
            } => write!(
                f,
                "the failure rate increased ({earlier_failures}/{earlier_decided} earlier, \
                 {later_failures}/{later_decided} later) (spec 19.2)"
            ),
            Self::InsufficientInstrumentEvidence => write!(
                f,
                "a settlement names no verifying instrument — gate erosion cannot be decided, \
                 so it refuses (spec 19.2)"
            ),
            Self::GateErosion => write!(
                f,
                "gate erosion: a later settlement was verified by a strictly weaker instrument \
                 class — a workflow faster by skipping verification is a regression, not \
                 learning (spec 19.2)"
            ),
            Self::InsufficientBurdenEvidence => write!(
                f,
                "a settlement carries no orchestration-cost proxy — the burden condition \
                 cannot decide, so it refuses (spec 19.2)"
            ),
            Self::OrchestrationBurdenIncreased { earlier, later } => write!(
                f,
                "orchestration burden increased ({earlier} earlier, {later} later) (spec 19.2)"
            ),
        }
    }
}

/// One settlement the promotion decision rests on — the durable witness
/// naming the run, the unit, the verification that decided it, and the
/// evidence it was decided over.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SettlementWitness {
    pub run: RunId,
    pub unit: UnitId,
    pub verification: VerificationRef,
    pub evidence: EvidenceRef,
}

impl SettlementWitness {
    fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            (
                "run".to_owned(),
                JsonValue::Str(self.run.as_str().to_owned()),
            ),
            (
                "unit".to_owned(),
                JsonValue::Str(self.unit.as_str().to_owned()),
            ),
            (
                "verification".to_owned(),
                JsonValue::Str(self.verification.as_str().to_owned()),
            ),
            (
                "evidence".to_owned(),
                JsonValue::Str(self.evidence.as_str().to_owned()),
            ),
        ])
    }

    fn from_json(value: &JsonValue) -> Result<Self, CapabilityLedgerError> {
        let JsonValue::Object(_) = value else {
            return Err(CapabilityLedgerError::Corrupt {
                detail: "a settlement witness is an object".to_owned(),
            });
        };
        Ok(Self {
            run: RunId::new(string_field(value, "run")?).map_err(corrupt_field("witness run"))?,
            unit: UnitId::new(string_field(value, "unit")?)
                .map_err(corrupt_field("witness unit"))?,
            verification: VerificationRef::new(string_field(value, "verification")?)
                .map_err(corrupt_field("witness verification"))?,
            evidence: EvidenceRef::new(string_field(value, "evidence")?)
                .map_err(corrupt_field("witness evidence"))?,
        })
    }
}

/// The §19.2 decision record a PROMOTED transition carries (charter D5:
/// "full §19.2 satisfaction, decision record with evidence refs durable").
/// Every figure is the condition's own output; the witnesses name the
/// settlements the evidence rests on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotionEvidence {
    pub min_settlements_configured: u32,
    pub settlements_observed: usize,
    /// The union of the settlement window's content axes, sorted (the
    /// recorded variation that WAS real).
    pub variation_axes: Vec<VariationAxis>,
    pub recovery_observations: usize,
    /// (failures, decided) per window half — the non-increasing failure
    /// rate's recorded figures.
    pub failure_rate_earlier: (u64, u64),
    pub failure_rate_later: (u64, u64),
    /// The settlement window's instrument determinism, in window order —
    /// the non-erosion fact.
    pub instrument_series: Vec<bool>,
    /// The settlement window's orchestration-cost proxies, in window
    /// order — the non-increasing burden fact.
    pub orchestration_costs: Vec<u64>,
    pub settlements: Vec<SettlementWitness>,
}

impl PromotionEvidence {
    pub fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            (
                "min_settlements_configured".to_owned(),
                JsonValue::Number(JsonNumber::Uint(u64::from(self.min_settlements_configured))),
            ),
            (
                "settlements_observed".to_owned(),
                JsonValue::Number(JsonNumber::Uint(self.settlements_observed as u64)),
            ),
            (
                "variation_axes".to_owned(),
                JsonValue::Array(
                    self.variation_axes
                        .iter()
                        .map(VariationAxis::to_json)
                        .collect(),
                ),
            ),
            (
                "recovery_observations".to_owned(),
                JsonValue::Number(JsonNumber::Uint(self.recovery_observations as u64)),
            ),
            (
                "failure_rate_earlier".to_owned(),
                rate_json(self.failure_rate_earlier),
            ),
            (
                "failure_rate_later".to_owned(),
                rate_json(self.failure_rate_later),
            ),
            (
                "instrument_series".to_owned(),
                JsonValue::Array(
                    self.instrument_series
                        .iter()
                        .map(|flag| JsonValue::Bool(*flag))
                        .collect(),
                ),
            ),
            (
                "orchestration_costs".to_owned(),
                JsonValue::Array(
                    self.orchestration_costs
                        .iter()
                        .map(|cost| JsonValue::Number(JsonNumber::Uint(*cost)))
                        .collect(),
                ),
            ),
            (
                "settlements".to_owned(),
                JsonValue::Array(
                    self.settlements
                        .iter()
                        .map(SettlementWitness::to_json)
                        .collect(),
                ),
            ),
        ])
    }

    pub fn from_json(value: &JsonValue) -> Result<Self, CapabilityLedgerError> {
        let JsonValue::Object(_) = value else {
            return Err(CapabilityLedgerError::Corrupt {
                detail: "the promotion evidence is an object".to_owned(),
            });
        };
        let uint_of = |name: &str| -> Result<u64, CapabilityLedgerError> {
            match value.get(name) {
                Some(JsonValue::Number(JsonNumber::Uint(n))) => Ok(*n),
                _ => Err(CapabilityLedgerError::Corrupt {
                    detail: format!("the promotion evidence carries no {name} figure"),
                }),
            }
        };
        let rate_of = |name: &str| -> Result<(u64, u64), CapabilityLedgerError> {
            let rate = value
                .get(name)
                .ok_or_else(|| CapabilityLedgerError::Corrupt {
                    detail: format!("the promotion evidence carries no {name}"),
                })?;
            Ok((uint_in(rate, "failures")?, uint_in(rate, "decided")?))
        };
        let axes = match value.get("variation_axes") {
            Some(JsonValue::Array(items)) => items
                .iter()
                .map(VariationAxis::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            _ => {
                return Err(CapabilityLedgerError::Corrupt {
                    detail: "the promotion evidence carries no variation_axes array".to_owned(),
                });
            }
        };
        let instruments = match value.get("instrument_series") {
            Some(JsonValue::Array(items)) => {
                let mut flags = Vec::with_capacity(items.len());
                for item in items {
                    match item {
                        JsonValue::Bool(flag) => flags.push(*flag),
                        _ => {
                            return Err(CapabilityLedgerError::Corrupt {
                                detail: "instrument_series carries a non-boolean".to_owned(),
                            });
                        }
                    }
                }
                flags
            }
            _ => {
                return Err(CapabilityLedgerError::Corrupt {
                    detail: "the promotion evidence carries no instrument_series array".to_owned(),
                });
            }
        };
        let costs = match value.get("orchestration_costs") {
            Some(JsonValue::Array(items)) => {
                let mut costs = Vec::with_capacity(items.len());
                for item in items {
                    match item {
                        JsonValue::Number(JsonNumber::Uint(cost)) => costs.push(*cost),
                        _ => {
                            return Err(CapabilityLedgerError::Corrupt {
                                detail: "orchestration_costs carries a non-integer".to_owned(),
                            });
                        }
                    }
                }
                costs
            }
            _ => {
                return Err(CapabilityLedgerError::Corrupt {
                    detail: "the promotion evidence carries no orchestration_costs array"
                        .to_owned(),
                });
            }
        };
        let settlements = match value.get("settlements") {
            Some(JsonValue::Array(items)) => items
                .iter()
                .map(SettlementWitness::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            _ => {
                return Err(CapabilityLedgerError::Corrupt {
                    detail: "the promotion evidence carries no settlements array".to_owned(),
                });
            }
        };
        Ok(Self {
            min_settlements_configured: u32::try_from(uint_of("min_settlements_configured")?)
                .map_err(corrupt_field("min_settlements_configured"))?,
            settlements_observed: usize::try_from(uint_of("settlements_observed")?)
                .map_err(corrupt_field("settlements_observed"))?,
            variation_axes: axes,
            recovery_observations: usize::try_from(uint_of("recovery_observations")?)
                .map_err(corrupt_field("recovery_observations"))?,
            failure_rate_earlier: rate_of("failure_rate_earlier")?,
            failure_rate_later: rate_of("failure_rate_later")?,
            instrument_series: instruments,
            orchestration_costs: costs,
            settlements,
        })
    }
}

fn rate_json(rate: (u64, u64)) -> JsonValue {
    JsonValue::Object(vec![
        (
            "failures".to_owned(),
            JsonValue::Number(JsonNumber::Uint(rate.0)),
        ),
        (
            "decided".to_owned(),
            JsonValue::Number(JsonNumber::Uint(rate.1)),
        ),
    ])
}

/// The canonical durable spend object (all six §5.3.7 counters — the
/// payment observation records the checkpoint's counters whole, lossless).
fn spend_json(spent: &Spent) -> JsonValue {
    JsonValue::Object(vec![
        (
            "rounds".to_owned(),
            JsonValue::Number(JsonNumber::Uint(u64::from(spent.rounds()))),
        ),
        (
            "wall_seconds".to_owned(),
            JsonValue::Number(JsonNumber::Uint(spent.wall_seconds())),
        ),
        (
            "tokens".to_owned(),
            JsonValue::Number(JsonNumber::Uint(spent.tokens())),
        ),
        (
            "cost_usd_micros".to_owned(),
            JsonValue::Number(JsonNumber::Uint(spent.cost_usd().get())),
        ),
        (
            "diagnostic_tokens".to_owned(),
            JsonValue::Number(JsonNumber::Uint(spent.diagnostic_tokens())),
        ),
        (
            "diagnostic_cost_usd_micros".to_owned(),
            JsonValue::Number(JsonNumber::Uint(spent.diagnostic_cost_usd().get())),
        ),
    ])
}

// ---------------------------------------------------------------------------
// Status transitions — the register's only writer
// ---------------------------------------------------------------------------

/// Why a status moved. The kind MUST match the transition's target status
/// ([`StatusTransition::new`] enforces): promotions carry the §19.2
/// decision record, refusals carry the per-condition reasons (§19.4's
/// "insufficient evidence ⟹ provisional ⟹ not promoted") — never an
/// empty set, which composes no transition ([`StatusTransitionError::EmptyRefusal`]) —
/// revocations carry the actor's detail.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TransitionReason {
    Promoted(PromotionEvidence),
    Refused(Vec<PromotionRefusal>),
    Revoked(String),
}

impl TransitionReason {
    fn to_json(&self) -> JsonValue {
        match self {
            Self::Promoted(evidence) => {
                let mut object = vec![("kind".to_owned(), JsonValue::Str("promoted".to_owned()))];
                if let JsonValue::Object(entries) = evidence.to_json() {
                    object.extend(entries);
                }
                JsonValue::Object(object)
            }
            Self::Refused(refusals) => JsonValue::Object(vec![
                ("kind".to_owned(), JsonValue::Str("refused".to_owned())),
                (
                    "refusals".to_owned(),
                    JsonValue::Array(refusals.iter().map(PromotionRefusal::to_json).collect()),
                ),
            ]),
            Self::Revoked(detail) => JsonValue::Object(vec![
                ("kind".to_owned(), JsonValue::Str("revoked".to_owned())),
                ("detail".to_owned(), JsonValue::Str(detail.clone())),
            ]),
        }
    }

    fn from_json(value: &JsonValue) -> Result<Self, CapabilityLedgerError> {
        let JsonValue::Object(_) = value else {
            return Err(CapabilityLedgerError::Corrupt {
                detail: "a transition reason is an object".to_owned(),
            });
        };
        match string_field(value, "kind")? {
            "promoted" => Ok(Self::Promoted(PromotionEvidence::from_json(value)?)),
            "refused" => {
                let refusals = match value.get("refusals") {
                    Some(JsonValue::Array(items)) => items
                        .iter()
                        .map(PromotionRefusal::from_json)
                        .collect::<Result<Vec<_>, _>>()?,
                    _ => {
                        return Err(CapabilityLedgerError::Corrupt {
                            detail: "the refused reason carries no refusals array".to_owned(),
                        });
                    }
                };
                Ok(Self::Refused(refusals))
            }
            "revoked" => Ok(Self::Revoked(string_field(value, "detail")?.to_owned())),
            other => Err(CapabilityLedgerError::Corrupt {
                detail: format!("transition reason kind {other:?} is outside the vocabulary"),
            }),
        }
    }
}

/// `StatusTransition` construction failures — the transition record
/// refuses to compose what the status state machine forbids, so a bare
/// set-status shape has no typed value to travel through (charter D2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum StatusTransitionError {
    /// The reason kind does not carry the target status (a promotion
    /// reason on a refusal target, or the reverse).
    ReasonMismatch { to: CapabilityStatus },
    /// A `Refused` reason carried an EMPTY refusal set. A refusal is the
    /// durable record of the per-condition reasons that failed; an empty
    /// one is a decision record with no decision in it — the honest
    /// promotion service can never produce one (an all-conditions-pass
    /// evaluation promotes instead) — so it composes no transition and no
    /// leg may write one (COR-2 C6).
    EmptyRefusal,
    /// The from→to move is outside the state machine (the domain
    /// `Capability::restatus` table, plus the one documented non-elevating
    /// extension: Provisional→Provisional re-records updated refusal
    /// reasons on an unchanged status — it can never elevate).
    IllegalMove {
        from: CapabilityStatus,
        to: CapabilityStatus,
    },
}

impl fmt::Display for StatusTransitionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::ReasonMismatch { to } => {
                write!(f, "the transition reason does not carry the {to} target")
            }
            Self::EmptyRefusal => write!(
                f,
                "a refused transition carried zero refusal reasons — a refusal records \
                 the conditions that failed; an empty one is not a decision"
            ),
            Self::IllegalMove { from, to } => write!(
                f,
                "capability status cannot move {from} to {to} (spec 5.3.8 state machine; \
                 revoked is sticky)"
            ),
        }
    }
}

impl std::error::Error for StatusTransitionError {}

/// Whether the register's state machine permits from→to. The domain
/// `Capability::restatus` table verbatim, plus exactly one documented
/// non-elevating move (Provisional→Provisional: a re-evaluation that
/// refuses again re-records its reasons; the status cannot rise by it).
/// Revoked is terminal — sticky by construction: no move leaves it.
pub fn is_legal_transition(from: CapabilityStatus, to: CapabilityStatus) -> bool {
    matches!(
        (from, to),
        (CapabilityStatus::Observed, CapabilityStatus::Provisional)
            | (CapabilityStatus::Observed, CapabilityStatus::Promoted)
            | (CapabilityStatus::Provisional, CapabilityStatus::Provisional)
            | (CapabilityStatus::Provisional, CapabilityStatus::Promoted)
            | (CapabilityStatus::Provisional, CapabilityStatus::Revoked)
            | (CapabilityStatus::Promoted, CapabilityStatus::Revoked)
    )
}

/// One first-class status transition (charter D1/D2/D5): the register's
/// ONLY status writer, carrying its reason and evidence. The record
/// refuses to compose an illegal move; the ledger that applies it
/// additionally refuses a `from` that disagrees with the durable status
/// ([`CapabilityLedgerError::StaleTransition`]) and a capability that has
/// never been observed ([`CapabilityLedgerError::UnknownCapability`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StatusTransition {
    capability: CapabilityId,
    from: CapabilityStatus,
    to: CapabilityStatus,
    reason: TransitionReason,
    evidence: Vec<EvidenceRef>,
    at: EpochSeconds,
}

impl StatusTransition {
    /// Composes a transition. `from` is the caller's claim about the
    /// durable status; the applying ledger validates the claim.
    pub fn new(
        capability: CapabilityId,
        from: CapabilityStatus,
        to: CapabilityStatus,
        reason: TransitionReason,
        evidence: Vec<EvidenceRef>,
        at: EpochSeconds,
    ) -> Result<Self, StatusTransitionError> {
        let reason_carries = match &reason {
            TransitionReason::Promoted(_) => to == CapabilityStatus::Promoted,
            TransitionReason::Refused(refusals) => {
                // A refusal is the durable record of the conditions that
                // failed; an EMPTY set is a record the honest promotion
                // service can never produce (an all-conditions-pass
                // evaluation promotes instead), so it composes no
                // transition and no leg may write one (COR-2 C6).
                if refusals.is_empty() {
                    return Err(StatusTransitionError::EmptyRefusal);
                }
                to == CapabilityStatus::Provisional
            }
            TransitionReason::Revoked(_) => to == CapabilityStatus::Revoked,
        };
        if !reason_carries {
            return Err(StatusTransitionError::ReasonMismatch { to });
        }
        if !is_legal_transition(from, to) {
            return Err(StatusTransitionError::IllegalMove { from, to });
        }
        Ok(Self {
            capability,
            from,
            to,
            reason,
            evidence,
            at,
        })
    }

    pub fn capability(&self) -> &CapabilityId {
        &self.capability
    }

    pub fn from(&self) -> CapabilityStatus {
        self.from
    }

    pub fn to(&self) -> CapabilityStatus {
        self.to
    }

    pub fn reason(&self) -> &TransitionReason {
        &self.reason
    }

    pub fn evidence(&self) -> &[EvidenceRef] {
        &self.evidence
    }

    pub fn at(&self) -> EpochSeconds {
        self.at
    }

    pub fn to_json(&self) -> String {
        JsonValue::Object(vec![
            (
                "capability".to_owned(),
                JsonValue::Str(self.capability.as_str().to_owned()),
            ),
            ("from".to_owned(), JsonValue::Str(self.from.to_string())),
            ("to".to_owned(), JsonValue::Str(self.to.to_string())),
            ("reason".to_owned(), self.reason.to_json()),
            (
                "evidence".to_owned(),
                JsonValue::Array(
                    self.evidence
                        .iter()
                        .map(|reference| JsonValue::Str(reference.as_str().to_owned()))
                        .collect(),
                ),
            ),
            (
                "at".to_owned(),
                JsonValue::Number(JsonNumber::Uint(self.at.get())),
            ),
        ])
        .to_json_text()
    }

    pub fn decode(text: &str) -> Result<Self, CapabilityLedgerError> {
        const FIELDS: &[&str] = &["capability", "from", "to", "reason", "evidence", "at"];
        let document =
            crate::json::parse(text).map_err(|error| CapabilityLedgerError::Corrupt {
                detail: format!("a transition payload is not valid JSON: {error}"),
            })?;
        let JsonValue::Object(_) = &document else {
            return Err(CapabilityLedgerError::Corrupt {
                detail: "a transition payload is an object".to_owned(),
            });
        };
        reject_unknown(&document, FIELDS)?;
        let capability = CapabilityId::new(string_field(&document, "capability")?)
            .map_err(corrupt_field("capability"))?;
        let from = CapabilityStatus::parse(string_field(&document, "from")?)
            .map_err(corrupt_field("from"))?;
        let to =
            CapabilityStatus::parse(string_field(&document, "to")?).map_err(corrupt_field("to"))?;
        let reason = TransitionReason::from_json(document.get("reason").ok_or_else(|| {
            CapabilityLedgerError::Corrupt {
                detail: "the transition payload carries no reason".to_owned(),
            }
        })?)?;
        let evidence = match document.get("evidence") {
            Some(JsonValue::Array(items)) => {
                let mut references = Vec::with_capacity(items.len());
                for item in items {
                    match item {
                        JsonValue::Str(text) => references
                            .push(EvidenceRef::new(text).map_err(corrupt_field("evidence ref"))?),
                        _ => {
                            return Err(CapabilityLedgerError::Corrupt {
                                detail: "evidence carries a non-string ref".to_owned(),
                            });
                        }
                    }
                }
                references
            }
            _ => {
                return Err(CapabilityLedgerError::Corrupt {
                    detail: "the transition payload carries no evidence array".to_owned(),
                });
            }
        };
        let at = uint_field(&document, "at")?;
        // The decoded record re-validates its own shape: a payload that
        // names an illegal move or a mismatched reason is corruption,
        // never a transition.
        Self::new(
            capability,
            from,
            to,
            reason,
            evidence,
            EpochSeconds::new(at),
        )
        .map_err(|error| CapabilityLedgerError::Corrupt {
            detail: format!("the stored transition violates its own state machine: {error}"),
        })
    }
}

// ---------------------------------------------------------------------------
// Register entry and append receipt
// ---------------------------------------------------------------------------

/// The capability register's derived entry (charter D5): `Observed` = has
/// observations, never evaluated; `Provisional` = evaluated and refused,
/// reasons durable; `Promoted` = full §19.2 satisfaction, decision record
/// durable; `Revoked` = explicit revoke, sticky. The current status is the
/// latest transition's target — derived from the transition history, never
/// a separately mutable field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilityEntry {
    pub capability: CapabilityId,
    pub status: CapabilityStatus,
    /// The first observation's recording instant.
    pub registered_at: EpochSeconds,
    /// The latest applied transition's instant; `None` while the
    /// capability was never evaluated.
    pub last_transition_at: Option<EpochSeconds>,
}

/// The receipt of one observation append: the ledger-wide monotonic
/// sequence (append order — the ordering every window decision reads) and
/// whether the identity was already present (the idempotent no-op).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationAppend {
    pub sequence: u64,
    pub idempotent: bool,
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// `CapabilityLedger` failures — typed (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum CapabilityLedgerError {
    /// A durable append failed. Nothing was recorded.
    AppendFailed { detail: String },
    /// A read failed.
    ReadFailed { detail: String },
    /// Stored state exists but does not decode into the typed records.
    Corrupt { detail: String },
    /// The same observation identity was re-appended with DISAGREEING
    /// facts — the `record_run` precedent: surfaced, never silently
    /// absorbed, and the stored row stands.
    ObservationConflict { identity: ObservationIdentity },
    /// A transition declared a `from` that disagrees with the durable
    /// status. Nothing was written.
    StaleTransition {
        capability: CapabilityId,
        current: CapabilityStatus,
    },
    /// A transition named a capability with no observations on record.
    /// Nothing was written.
    UnknownCapability { capability: CapabilityId },
    /// An elevating transition (`TransitionReason::Promoted`) named a
    /// witness that does not resolve, in the ledger's OWN observation log,
    /// to a Settlement observation of the same capability with matching
    /// (run, unit, verification, evidence) facts. Elevation is an earned
    /// write: the promotion's witnesses must be memories that exist.
    /// Nothing was written.
    UnrootedPromotion {
        capability: CapabilityId,
        witness: SettlementWitness,
    },
}

impl fmt::Display for CapabilityLedgerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::AppendFailed { detail } => write!(f, "ledger append failed: {detail}"),
            Self::ReadFailed { detail } => write!(f, "ledger read failed: {detail}"),
            Self::Corrupt { detail } => write!(f, "the capability ledger is damaged: {detail}"),
            Self::ObservationConflict { identity } => write!(
                f,
                "observation {} of run {} for capability {} is already recorded with \
                 different facts — the stored row stands (the record_run precedent)",
                identity.class, identity.run, identity.capability
            ),
            Self::StaleTransition {
                capability,
                current,
            } => write!(
                f,
                "capability {capability} is durably {current}; the transition declared a \
                 different prior status — nothing was written"
            ),
            Self::UnknownCapability { capability } => write!(
                f,
                "capability {capability} has no observations on record; an unevaluated \
                 capability cannot transition"
            ),
            Self::UnrootedPromotion {
                capability,
                witness,
            } => write!(
                f,
                "the promotion of capability {capability} names witness (run {}, unit {}, \
                 verification {}, evidence {}) that resolves to no Settlement observation in \
                 this ledger's own observation log — elevation is an earned write and the \
                 promotion's witnesses must be memories that exist; nothing was written",
                witness.run, witness.unit, witness.verification, witness.evidence
            ),
        }
    }
}

impl std::error::Error for CapabilityLedgerError {}

impl From<JsonError> for CapabilityLedgerError {
    fn from(error: JsonError) -> Self {
        Self::Corrupt {
            detail: error.to_string(),
        }
    }
}

// ---------------------------------------------------------------------------
// The port
// ---------------------------------------------------------------------------

/// Whether one promotion witness resolves, in `log`, to a Settlement
/// observation of `capability` with matching (run, unit, verification,
/// evidence) facts — [`CapabilityLedger`]'s log-rooting obligation for
/// elevating transitions (COR-1 C3). Conforming `apply_transition`
/// implementations refuse [`CapabilityLedgerError::UnrootedPromotion`]
/// for any witness of a `TransitionReason::Promoted` reason this
/// predicate rejects. Pure provenance over the ledger's OWN observation
/// log; nothing here is §19.2 logic.
pub fn promotion_witness_is_rooted(
    log: &[ObservationRecord],
    capability: &CapabilityId,
    witness: &SettlementWitness,
) -> bool {
    log.iter().any(|record| {
        record.identity().capability() == capability
            && record.identity().class() == ObservationClass::Settlement
            && record.identity().run() == &witness.run
            && record.unit().as_ref() == Some(&witness.unit)
            && record.verification() == Some(&witness.verification)
            && record.evidence() == Some(&witness.evidence)
    })
}

/// The two §19.1 stores (spec §6): the append-only developmental-memory
/// observation log and the governed capability register. Every method
/// performs durable IO and carries a deadline.
///
/// Write policy, opposite by design (charter D2): observation appends are
/// ANY caller, NEVER gated on outcome; the register's only status writer
/// is [`CapabilityLedger::apply_transition`], and elevating transitions
/// are composed by the app-layer promotion service after the §19.2
/// conditions hold — there is no bare set-status method to bypass them.
pub trait CapabilityLedger {
    /// Appends one observation to the developmental-memory log. Ungated,
    /// immediate, append-only (spec §19.1: "Raw. Immediate, append-only" —
    /// a failed attempt is still memory). Idempotent on the declared
    /// [`ObservationIdentity`]: the same identity with agreeing facts is a
    /// no-op ([`ObservationAppend::idempotent`] is true); a disagreeing
    /// re-append is [`CapabilityLedgerError::ObservationConflict`], and
    /// the stored row stands. The first observation of a capability
    /// registers it `Observed`.
    fn append_observation(
        &self,
        observation: ObservationRecord,
        deadline: Deadline,
    ) -> Result<ObservationAppend, CapabilityLedgerError>;

    /// The capability's observations, in append order. A capability with
    /// no observations is an empty vector, never an error.
    fn observations(
        &self,
        capability: &CapabilityId,
        deadline: Deadline,
    ) -> Result<Vec<ObservationRecord>, CapabilityLedgerError>;

    /// The register entry; `None` for a capability with no observations.
    fn capability(
        &self,
        capability: &CapabilityId,
        deadline: Deadline,
    ) -> Result<Option<CapabilityEntry>, CapabilityLedgerError>;

    /// Applies one status transition — the register's ONLY status writer.
    /// Refuses a capability with no observations
    /// ([`CapabilityLedgerError::UnknownCapability`]), a `from` claim that
    /// disagrees with the durable status
    /// ([`CapabilityLedgerError::StaleTransition`]), and any move the
    /// state machine forbids (the record type refuses to compose one; a
    /// conforming adapter re-derives the same refusal).
    ///
    /// PORT OBLIGATION (COR-1 C3, enforced by the shared contract suite on
    /// every leg): an elevating transition — one whose reason is
    /// [`TransitionReason::Promoted`] — is a LOG-ROOTED earned write: it
    /// is refused with [`CapabilityLedgerError::UnrootedPromotion`] unless
    /// EVERY witness in the evidence resolves, in this ledger's OWN
    /// observation log, to a Settlement observation of the same capability
    /// with matching (run, unit, verification, evidence) facts. Nothing
    /// about this is §19.2 logic — the ledger cannot know the promotion
    /// conditions — it is purely "the promotion's witnesses must be
    /// memories that exist". A fabricated `PromotionEvidence` applied
    /// directly through this method must never elevate the register.
    /// (Recorded boundary: log-rooting does not defend against a writer
    /// who forges the observation log itself — the developmental log is
    /// append-only and auditable by design; the sealed-mint alternative,
    /// an authority type with no public constructor for elevating reasons,
    /// is a recorded ports-revision candidate. The composition root hands
    /// ledgers only to trusted code.)
    ///
    /// The applied transition is durable history; nothing is ever edited
    /// or removed.
    fn apply_transition(
        &self,
        transition: StatusTransition,
        deadline: Deadline,
    ) -> Result<CapabilityEntry, CapabilityLedgerError>;

    /// The capability's applied transitions, in apply order. An
    /// unevaluated capability has none.
    fn transitions(
        &self,
        capability: &CapabilityId,
        deadline: Deadline,
    ) -> Result<Vec<StatusTransition>, CapabilityLedgerError>;
}

// ---------------------------------------------------------------------------
// Shared JSON helpers (the verification-record codec's conventions)
// ---------------------------------------------------------------------------

fn reject_unknown(object: &JsonValue, known: &[&str]) -> Result<(), CapabilityLedgerError> {
    for key in object.keys() {
        if !known.contains(&key.as_str()) {
            return Err(CapabilityLedgerError::Corrupt {
                detail: format!("the ledger payload carries unknown field {key:?}"),
            });
        }
    }
    Ok(())
}

fn string_field<'a>(
    object: &'a JsonValue,
    field: &'static str,
) -> Result<&'a str, CapabilityLedgerError> {
    let leaf = field.rsplit('.').next().unwrap_or(field);
    match object.get(leaf) {
        Some(JsonValue::Str(text)) => Ok(text),
        Some(_) => Err(CapabilityLedgerError::Corrupt {
            detail: format!("the ledger field {field:?} is not a string"),
        }),
        None => Err(CapabilityLedgerError::Corrupt {
            detail: format!("the ledger field {field:?} is absent"),
        }),
    }
}

fn uint_field(object: &JsonValue, field: &str) -> Result<u64, CapabilityLedgerError> {
    match object.get(field) {
        Some(JsonValue::Number(JsonNumber::Uint(n))) => Ok(*n),
        _ => Err(CapabilityLedgerError::Corrupt {
            detail: format!("the ledger field {field:?} is not a non-negative integer"),
        }),
    }
}

fn uint_in(object: &JsonValue, field: &str) -> Result<u64, CapabilityLedgerError> {
    match object.get(field) {
        Some(JsonValue::Number(JsonNumber::Uint(n))) => Ok(*n),
        _ => Err(CapabilityLedgerError::Corrupt {
            detail: format!("the ledger object carries no {field} figure"),
        }),
    }
}

fn is_null(object: &JsonValue, field: &str) -> bool {
    matches!(object.get(field), None | Some(JsonValue::Null))
}

/// Maps any typed value-construction error onto a `Corrupt` carrying the
/// field locus.
fn corrupt_field<E: fmt::Display>(field: &'static str) -> impl Fn(E) -> CapabilityLedgerError {
    move |error| CapabilityLedgerError::Corrupt {
        detail: format!("the ledger field {field:?} is invalid: {error}"),
    }
}
