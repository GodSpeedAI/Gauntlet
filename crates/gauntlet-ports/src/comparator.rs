//! `Comparator` — spec §6.2: "`compare(envelope: ComparisonEnvelope) ->
//! ComparisonRecord` — Envelope is built by the runtime (I4). Comparator
//! implementations MAY be model-backed or deterministic."
//!
//! I4 (envelope construction): the runtime builds the comparison envelope
//! from the §13.2 allowlist — the frozen reference's digest, the
//! runtime-rendered reference view the critic may see, the facet-carrying
//! observation view (the verification-input projection: instrument facets,
//! digest, bundle ref, BOTH gap arrays), the comparison-contract view
//! (version, instantiated dimensions, severity rubric), the applicable
//! constraints, and the unresolved-but-relevant residuals. No builder
//! trajectory, reasoning, identity, attempt count, prior verdict, operator
//! chat, or claims content is present (P2 asserts this structurally over
//! the canonical serialization).
//!
//! **Boundary decision (ledgered, plan Task 10):** the observation side was
//! widened from the digest+gaps-only projection to the facet-carrying
//! verification-input view, closing the P3 comparison-consumption half — a
//! comparator wired before this change could not see the recorded
//! instrument failure. This widening is the ledgered Task-10 trigger for
//! the Task-9 deferrals "envelope widening" (B-3c + A-M3) and "domain view
//! drops facets" (B-8a). Adding ANY further field to
//! [`ComparisonEnvelope`] or [`ObservationView`] is a boundary decision,
//! not a refactor.
//!
//! Claims stay excluded at the TYPE level (Task-9 discipline): there is no
//! claims-typed member anywhere in the envelope, and the facet summaries
//! key on the typed [`FacetKind`] vocabulary — `claims` is not a variant
//! of it, so no runtime `if kind == claims` check exists or is needed.

use core::fmt;
use std::sync::Arc;

use gauntlet_domain::{
    ComparisonRecord, Digest, EpochSeconds, EvidenceRef, InvocationState, IterationNo,
    ReferenceVersion, Residual, RunId, Severity,
};
use sha2::{Digest as _, Sha256};

use crate::agent_runner::RoleInvocationSpec;
use crate::comparison_record::ComparisonRecordDoc;
use crate::deadline::Deadline;
use crate::evidence_store::EvidenceStore;
use crate::json::{JsonNumber, JsonValue};
use crate::observer::FacetKind;
use crate::state_store::{ObservationBinding, RunEvent, RunEventKind, StateStore};
use gauntlet_domain::EvidenceKind;

/// The declared field vocabulary of one facet summary as canonically
/// serialized inside the envelope's observation view — the same reduced
/// field set as the application's verification inputs (kind, instrument
/// name+version, command, exit status, evidence ref). The canonical bytes
/// are zipped from this table, so the key order is the declared order by
/// construction.
const FACET_SUMMARY_FIELDS: &[&str] = &[
    "kind",
    "instrument_name",
    "instrument_version",
    "command",
    "exit_status",
    "evidence_ref",
];

/// `Comparator` failures — typed (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum ComparatorError {
    /// The envelope is not a shape this comparator can operate on.
    EnvelopeInvalid { detail: String },
    /// The comparison itself failed (model unreachable, contract
    /// unparseable, result document missing or malformed). Uncertainty is
    /// never converted into success (§21.2): a failed comparison is an
    /// error, not a pass.
    ComparisonFailed { detail: String },
    /// The critic dispatch did not reach a terminal state within the
    /// deadline (model-backed adapters). A timeout is never a pass.
    TimedOut { detail: String },
    /// A remote critic could not be confirmed terminal after an error. Its
    /// dispatch permit is quarantined rather than released; recovery must
    /// re-probe the recorded invocation before capacity can be reused.
    RecoveryRequired { detail: String },
    /// The critic's result contract does not match the dispatch — wrong
    /// invocation identity, wrong envelope digest, or a non-comparison
    /// payload (spec §10.3 rule 6, `contract_mismatch` class).
    ContractMismatch { detail: String },
}

impl fmt::Display for ComparatorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EnvelopeInvalid { detail } => {
                write!(f, "the comparison envelope is invalid: {detail}")
            }
            Self::ComparisonFailed { detail } => {
                write!(f, "the comparison failed: {detail}")
            }
            Self::TimedOut { detail } => {
                write!(f, "the comparison timed out: {detail}")
            }
            Self::RecoveryRequired { detail } => {
                write!(f, "comparison recovery is required: {detail}")
            }
            Self::ContractMismatch { detail } => {
                write!(
                    f,
                    "the critic result contract does not match the dispatch: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for ComparatorError {}

/// Calculates the §13.2 digest of the canonical envelope bytes.  This lives
/// with the port because both deterministic and model adapters consume the
/// same bytes; an application-only check would leave direct adapter callers
/// able to dispatch a stale digest.
pub fn calculate_envelope_digest(envelope: &ComparisonEnvelope) -> Digest {
    let mut hasher = Sha256::new();
    hasher.update(envelope.to_json().as_bytes());
    let hex: String = hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    Digest::new("sha256", &hex).expect("SHA-256 hex satisfies the Digest grammar by construction")
}

/// Refuses an envelope whose declared digest does not cover the exact
/// canonical bytes a comparator would consume.  This is deliberately a
/// `ContractMismatch`: dispatching different bytes under an old digest breaks
/// the invocation/record coupling required by §13.2.
pub fn verify_envelope_digest(envelope: &ComparisonEnvelope) -> Result<(), ComparatorError> {
    let recomputed = calculate_envelope_digest(envelope);
    if recomputed != envelope.envelope_digest {
        return Err(ComparatorError::ContractMismatch {
            detail: format!(
                "envelope digest {} does not cover the consumed canonical bytes (recomputed {})",
                envelope.envelope_digest, recomputed
            ),
        });
    }
    Ok(())
}

/// One instrument facet as the envelope carries it: the typed kind, the
/// instrument's name and version, the exact command (provenance, not a
/// verdict — the exit status beside it is the verdict carrier), the exit
/// status, and the content-addressed evidence ref. Construction takes the
/// TYPED [`FacetKind`], so a claims facet — which is a different type in
/// the application layer and not a `FacetKind` variant — has no path in.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FacetSummary {
    kind: FacetKind,
    instrument_name: String,
    instrument_version: String,
    command: Option<String>,
    exit_status: Option<i64>,
    evidence_ref: String,
}

impl FacetSummary {
    pub fn new(
        kind: FacetKind,
        instrument_name: String,
        instrument_version: String,
        command: Option<String>,
        exit_status: Option<i64>,
        evidence_ref: String,
    ) -> Self {
        Self {
            kind,
            instrument_name,
            instrument_version,
            command,
            exit_status,
            evidence_ref,
        }
    }

    pub fn kind(&self) -> FacetKind {
        self.kind
    }

    pub fn instrument_name(&self) -> &str {
        &self.instrument_name
    }

    pub fn instrument_version(&self) -> &str {
        &self.instrument_version
    }

    pub fn command(&self) -> Option<&str> {
        self.command.as_deref()
    }

    pub fn exit_status(&self) -> Option<i64> {
        self.exit_status
    }

    pub fn evidence_ref(&self) -> &str {
        &self.evidence_ref
    }

    fn to_json(&self) -> JsonValue {
        let values: Vec<JsonValue> = vec![
            JsonValue::Str(self.kind.token().to_owned()),
            JsonValue::Str(self.instrument_name.clone()),
            JsonValue::Str(self.instrument_version.clone()),
            match &self.command {
                Some(command) => JsonValue::Str(command.clone()),
                None => JsonValue::Null,
            },
            match self.exit_status {
                Some(status) if status >= 0 => {
                    JsonValue::Number(JsonNumber::Uint(status.unsigned_abs()))
                }
                Some(status) => JsonValue::Number(JsonNumber::NegInt(status)),
                None => JsonValue::Null,
            },
            JsonValue::Str(self.evidence_ref.clone()),
        ];
        JsonValue::Object(
            FACET_SUMMARY_FIELDS
                .iter()
                .zip(values)
                .map(|(field, value)| ((*field).to_owned(), value))
                .collect(),
        )
    }
}

/// One `unavailable_evidence` gap (spec §12.3, Annex B.3 shape): the typed
/// facet kind of the probe whose evidence could not be produced, and why.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnavailableGap {
    kind: FacetKind,
    reason: String,
}

impl UnavailableGap {
    pub fn new(kind: FacetKind, reason: String) -> Self {
        Self { kind, reason }
    }

    pub fn kind(&self) -> FacetKind {
        self.kind
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            (
                "kind".to_owned(),
                JsonValue::Str(self.kind.token().to_owned()),
            ),
            ("reason".to_owned(), JsonValue::Str(self.reason.clone())),
        ])
    }
}

/// One `observation_failures` gap (spec §12.3, Annex B.3 shape): a probe
/// that ran and failed, classified.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailureGap {
    kind: FacetKind,
    error_class: String,
    detail: String,
}

impl FailureGap {
    pub fn new(kind: FacetKind, error_class: String, detail: String) -> Self {
        Self {
            kind,
            error_class,
            detail,
        }
    }

    pub fn kind(&self) -> FacetKind {
        self.kind
    }

    pub fn error_class(&self) -> &str {
        &self.error_class
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            (
                "kind".to_owned(),
                JsonValue::Str(self.kind.token().to_owned()),
            ),
            (
                "error_class".to_owned(),
                JsonValue::Str(self.error_class.clone()),
            ),
            ("detail".to_owned(), JsonValue::Str(self.detail.clone())),
        ])
    }
}

/// The envelope's observation side: EXACTLY the verification-input field
/// set (digest, bundle ref, instrument facet summaries, both gap arrays) —
/// that similarity is the point: the envelope's observation side IS the
/// verification-input view (plan Task 10 D1). Built from typed parts; the
/// claims facet has no representation here.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationView {
    observation_digest: Digest,
    bundle_ref: EvidenceRef,
    facets: Vec<FacetSummary>,
    unavailable_evidence: Vec<UnavailableGap>,
    observation_failures: Vec<FailureGap>,
}

impl ObservationView {
    /// Crate-private: an observation view is a PROJECTION of durable bundle
    /// bytes, never a value a caller composes. [`ObservationView::project`] is
    /// the only way one comes into existence outside this module's own
    /// canonicalization.
    pub(crate) fn new(
        observation_digest: Digest,
        bundle_ref: EvidenceRef,
        facets: Vec<FacetSummary>,
        unavailable_evidence: Vec<UnavailableGap>,
        observation_failures: Vec<FailureGap>,
    ) -> Self {
        Self {
            observation_digest,
            bundle_ref,
            facets,
            unavailable_evidence,
            observation_failures,
        }
    }

    /// Projects the §5.3.6 observation-bundle wire document into the
    /// verification-input view the envelope carries.
    ///
    /// This is the CONSUMPTION side of the bundle schema: the comparison port
    /// reads the durable bytes itself rather than accepting a view someone
    /// else derived. The claims facet is not read — there is no code path that
    /// looks at it, which is Task 9's structural exclusion preserved here
    /// rather than restated as a filter.
    ///
    /// The bundle must declare the governed run and iteration; the view's
    /// `bundle_ref` is the authority's DURABLY BOUND ref, never an argument.
    pub(crate) fn project(
        authority: &RunComparisonAuthority,
        raw: &[u8],
    ) -> Result<Self, ComparatorError> {
        fn invalid(detail: String) -> ComparatorError {
            ComparatorError::EnvelopeInvalid { detail }
        }
        fn text<'a>(value: &'a JsonValue, field: &str) -> Result<&'a str, ComparatorError> {
            match value.get(field) {
                Some(JsonValue::Str(found)) => Ok(found.as_str()),
                _ => Err(invalid(format!(
                    "the observation bundle field {field:?} is missing or is not a string"
                ))),
            }
        }
        fn array<'a>(
            value: &'a JsonValue,
            field: &str,
        ) -> Result<&'a [JsonValue], ComparatorError> {
            match value.get(field) {
                Some(JsonValue::Array(items)) => Ok(items.as_slice()),
                None => Ok(&[]),
                Some(_) => Err(invalid(format!(
                    "the observation bundle field {field:?} is not an array"
                ))),
            }
        }
        fn kind(value: &JsonValue, locus: &str) -> Result<FacetKind, ComparatorError> {
            let token = text(value, "kind")?;
            FacetKind::parse(token).map_err(|_| {
                invalid(format!(
                    "the observation bundle's {locus} carries kind token {token:?}, which is \
                     outside the facet-kind vocabulary"
                ))
            })
        }

        let body = core::str::from_utf8(raw)
            .map_err(|error| invalid(format!("observation evidence is not UTF-8: {error}")))?;
        let document = crate::json::parse(body)
            .map_err(|error| invalid(format!("observation evidence is not JSON: {error}")))?;

        let declared_run = text(&document, "run_id")?;
        if declared_run != authority.run_id().as_str() {
            return Err(invalid(format!(
                "the durably bound observation bundle declares run {declared_run}, not the \
                 governed run {}",
                authority.run_id()
            )));
        }
        let declared_iteration = match document.get("iteration_no") {
            Some(JsonValue::Number(number)) => number.as_u64().ok_or_else(|| {
                invalid("the observation bundle's iteration_no is not a whole number".to_owned())
            })?,
            _ => {
                return Err(invalid(
                    "the observation bundle field \"iteration_no\" is missing or is not a number"
                        .to_owned(),
                ));
            }
        };
        if declared_iteration != u64::from(authority.iteration_no().get()) {
            return Err(invalid(format!(
                "the durably bound observation bundle declares iteration {declared_iteration}, \
                 not the governed iteration {}",
                authority.iteration_no().get()
            )));
        }
        let observation_digest = Digest::parse(text(&document, "observation_digest")?)
            .map_err(|error| invalid(format!("the observation digest does not decode: {error}")))?;
        // Re-derive the §12.4 digest from the bundle's own bytes. A digest a
        // document declares about itself is not evidence of anything: without
        // this the observation digest that lands in the comparison record — and
        // in durable state — would be whatever the bytes at the bound ref
        // asserted. The reference side of this envelope has always been
        // verified this way; this makes the observation side symmetric.
        let recomputed = crate::observation_digest::observation_digest(&document);
        if recomputed != observation_digest {
            return Err(invalid(format!(
                "the observation bundle declares observation_digest {observation_digest} but its \
                 own bytes digest to {recomputed}"
            )));
        }
        // ...and the content must be the content the RUN durably recorded, not
        // merely a self-consistent document. This is what makes the evidence
        // store non-authoritative: bytes served at the bound ref that are not
        // the bound observation are refused here, whoever served them.
        if &recomputed != authority.observation().digest() {
            return Err(invalid(format!(
                "the bytes at the durably bound observation ref digest to {recomputed}, but run \
                 {} iteration {} durably recorded observation {}",
                authority.run_id(),
                authority.iteration_no().get(),
                authority.observation().digest()
            )));
        }

        let mut facets = Vec::new();
        for entry in array(&document, "facets")? {
            let instrument = entry.get("instrument").ok_or_else(|| {
                invalid("an observation facet carries no instrument identity".to_owned())
            })?;
            facets.push(FacetSummary::new(
                kind(entry, "instrument facet")?,
                text(instrument, "name")?.to_owned(),
                text(instrument, "version")?.to_owned(),
                match entry.get("command") {
                    Some(JsonValue::Str(found)) => Some(found.clone()),
                    _ => None,
                },
                match entry.get("exit_status") {
                    Some(JsonValue::Number(number)) => number.as_i64(),
                    _ => None,
                },
                text(entry, "evidence_ref")?.to_owned(),
            ));
        }
        let mut unavailable = Vec::new();
        for entry in array(&document, "unavailable_evidence")? {
            unavailable.push(UnavailableGap::new(
                kind(entry, "unavailable-evidence gap")?,
                text(entry, "reason")?.to_owned(),
            ));
        }
        let mut failures = Vec::new();
        for entry in array(&document, "observation_failures")? {
            failures.push(FailureGap::new(
                kind(entry, "observation-failure gap")?,
                text(entry, "error_class")?.to_owned(),
                text(entry, "detail")?.to_owned(),
            ));
        }
        Ok(Self {
            observation_digest,
            bundle_ref: authority.observation_ref().clone(),
            facets,
            unavailable_evidence: unavailable,
            observation_failures: failures,
        })
    }

    pub fn observation_digest(&self) -> &Digest {
        &self.observation_digest
    }

    pub fn bundle_ref(&self) -> &EvidenceRef {
        &self.bundle_ref
    }

    pub fn facets(&self) -> &[FacetSummary] {
        &self.facets
    }

    pub fn unavailable_evidence(&self) -> &[UnavailableGap] {
        &self.unavailable_evidence
    }

    pub fn observation_failures(&self) -> &[FailureGap] {
        &self.observation_failures
    }

    fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            (
                "observation_digest".to_owned(),
                JsonValue::Str(self.observation_digest.to_string()),
            ),
            (
                "bundle_ref".to_owned(),
                JsonValue::Str(self.bundle_ref.as_str().to_owned()),
            ),
            (
                "facets".to_owned(),
                JsonValue::Array(self.facets.iter().map(FacetSummary::to_json).collect()),
            ),
            (
                "unavailable_evidence".to_owned(),
                JsonValue::Array(
                    self.unavailable_evidence
                        .iter()
                        .map(UnavailableGap::to_json)
                        .collect(),
                ),
            ),
            (
                "observation_failures".to_owned(),
                JsonValue::Array(
                    self.observation_failures
                        .iter()
                        .map(FailureGap::to_json)
                        .collect(),
                ),
            ),
        ])
    }
}

/// One severity-rubric row of the comparison contract (spec §9.5): a
/// severity and the definition that makes a residual that severity under
/// this reference.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RubricEntry {
    pub severity: Severity,
    pub definition: String,
}

impl RubricEntry {
    pub fn new(severity: Severity, definition: String) -> Self {
        Self {
            severity,
            definition,
        }
    }

    fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            (
                "severity".to_owned(),
                JsonValue::Str(self.severity.to_string()),
            ),
            (
                "definition".to_owned(),
                JsonValue::Str(self.definition.clone()),
            ),
        ])
    }
}

/// The envelope's comparison-contract view (spec §13.2 allowlist member
/// "comparison contract (dimension, rubric, severity definitions)"): the
/// version in force (recorded on every `ComparisonRecord`, spec §9.5), the
/// dimensions this reference instantiates (spec §13.4), and the severity
/// rubric.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContractView {
    version: String,
    dimensions: Vec<String>,
    rubric: Vec<RubricEntry>,
}

impl ContractView {
    pub fn new(version: String, dimensions: Vec<String>, rubric: Vec<RubricEntry>) -> Self {
        Self {
            version,
            dimensions,
            rubric,
        }
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    pub fn dimensions(&self) -> &[String] {
        &self.dimensions
    }

    pub fn rubric(&self) -> &[RubricEntry] {
        &self.rubric
    }

    fn to_json(&self) -> JsonValue {
        // Dimensions and rubric rows are declarations, not priority queues.
        // Sort their semantic-set representation before canonical bytes.
        let mut dimensions = self.dimensions.clone();
        dimensions.sort();
        let mut rubric = self.rubric.clone();
        rubric.sort_by(|left, right| {
            left.severity
                .to_string()
                .cmp(&right.severity.to_string())
                .then(left.definition.cmp(&right.definition))
        });
        JsonValue::Object(vec![
            ("version".to_owned(), JsonValue::Str(self.version.clone())),
            (
                "dimensions".to_owned(),
                JsonValue::Array(dimensions.into_iter().map(JsonValue::Str).collect()),
            ),
            (
                "severity_rubric".to_owned(),
                JsonValue::Array(rubric.iter().map(RubricEntry::to_json).collect()),
            ),
        ])
    }
}

/// One applicable constraint (spec §13.2 allowlist member "applicable
/// constraints"). The spec leaves the member's shape open; the envelope
/// carries it as a stable identifier plus the constraint statement
/// rendered from the reference, so a fresh-context critic can honor it
/// without seeing anything outside the allowlist.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplicableConstraint {
    id: String,
    statement: String,
}

impl ApplicableConstraint {
    pub fn new(id: String, statement: String) -> Self {
        Self { id, statement }
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub fn statement(&self) -> &str {
        &self.statement
    }

    fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("id".to_owned(), JsonValue::Str(self.id.clone())),
            (
                "statement".to_owned(),
                JsonValue::Str(self.statement.clone()),
            ),
        ])
    }
}

fn residual_to_json(residual: &Residual) -> JsonValue {
    JsonValue::Object(vec![
        (
            "id".to_owned(),
            JsonValue::Str(residual.id().as_str().to_owned()),
        ),
        (
            "severity".to_owned(),
            JsonValue::Str(residual.severity().to_string()),
        ),
        (
            "detail".to_owned(),
            JsonValue::Str(residual.detail().to_owned()),
        ),
        match residual.dimension() {
            Some(dimension) => ("dimension".to_owned(), JsonValue::Str(dimension.to_owned())),
            None => ("dimension".to_owned(), JsonValue::Null),
        },
    ])
}

/// The runtime-built comparison envelope (spec §6.2, §13.2, invariant I4):
/// everything a fresh-context critic is allowed to see, and nothing else.
///
/// The canonical serialization ([`ComparisonEnvelope::to_json`]) excludes
/// the `envelope_digest` member itself — the digest is a function of every
/// other member, exactly like the observation bundle's self-referential
/// digest field — so any boundary can recompute the digest over the
/// serialized bytes and compare (P2's structural assertion runs over those
/// bytes).
#[derive(Debug, Clone)]
pub struct ComparisonEnvelope {
    pub(crate) run_id: RunId,
    pub(crate) iteration_no: IterationNo,
    /// The run's frozen reference digest — carried so the I1 cross-check is
    /// possible at every later boundary.
    pub(crate) reference_digest: Digest,
    /// The runtime-rendered, allowlisted view of the reference the critic
    /// compares against.
    pub(crate) reference_view: String,
    /// What was actually observed, at FACET level (its digest couples the
    /// record, §5.3.6; the facets carry the instrument reality P3's
    /// comparison half composes over).
    pub(crate) observation: ObservationView,
    /// The comparison contract in force (version + instantiated dimensions
    /// + severity rubric — spec §9.5, §13.2).
    pub(crate) contract: ContractView,
    /// Constraints applicable to this comparison (spec §13.2 allowlist).
    pub(crate) constraints: Vec<ApplicableConstraint>,
    /// Unresolved residuals that remain relevant (spec §13.2 allowlist).
    pub(crate) open_residuals: Vec<Residual>,
    /// Digest of the canonical serialization of every member above (spec
    /// §13.2: stored on the `RoleInvocation` and the `ComparisonRecord`;
    /// carried here so the comparator can copy it through).
    pub(crate) envelope_digest: Digest,
}

impl ComparisonEnvelope {
    /// Read-only accessors deliberately make this serializable shape observational.
    /// Only the runtime issuer may assemble it into a dispatch capability.
    pub fn run_id(&self) -> &RunId {
        &self.run_id
    }
    pub fn iteration_no(&self) -> IterationNo {
        self.iteration_no
    }
    pub fn reference_digest(&self) -> &Digest {
        &self.reference_digest
    }
    pub fn reference_view(&self) -> &str {
        &self.reference_view
    }
    pub fn observation(&self) -> &ObservationView {
        &self.observation
    }
    pub fn contract(&self) -> &ContractView {
        &self.contract
    }
    pub fn constraints(&self) -> &[ApplicableConstraint] {
        &self.constraints
    }
    pub fn open_residuals(&self) -> &[Residual] {
        &self.open_residuals
    }
    pub fn envelope_digest(&self) -> &Digest {
        &self.envelope_digest
    }

    /// Mints the serializable envelope shape for one governed run/iteration.
    ///
    /// `run_id`, `iteration_no` come from the earned
    /// [`RunComparisonAuthority`]; `reference_digest` and `reference_view`
    /// come from the [`FrozenReferenceProjection`] that authority loaded.
    /// None of the four is a caller argument any more, so an envelope naming a
    /// reference or a run the caller chose is unrepresentable. The envelope
    /// digest is computed here, over the canonical bytes, and is not settable
    /// from outside.
    ///
    /// Crate-private: assembly is a step INSIDE
    /// [`TrustedComparisonContext::issue`], not a public convenience. A
    /// caller cannot assemble an envelope and then present it for dispatch,
    /// because there is no public entry that accepts one.
    pub(crate) fn assemble(
        authority: &RunComparisonAuthority,
        projection: &FrozenReferenceProjection,
        observation: ObservationView,
        contract: ContractView,
        constraints: Vec<ApplicableConstraint>,
        open_residuals: Vec<Residual>,
    ) -> Self {
        let mut envelope = Self {
            run_id: authority.run_id().clone(),
            iteration_no: authority.iteration_no(),
            reference_digest: projection.digest().clone(),
            reference_view: projection.view().to_owned(),
            observation,
            contract,
            constraints,
            open_residuals,
            envelope_digest: Digest::new("sha256", &"00".repeat(32))
                .expect("the placeholder digest satisfies the grammar"),
        };
        // `to_json` excludes the self-referential digest member, so computing
        // it here is order-independent with any later canonical-byte check.
        envelope.envelope_digest = calculate_envelope_digest(&envelope);
        envelope
    }

    /// Canonical serialization, documented key order per member: `run_id`,
    /// `iteration_no`, `reference_digest`, `reference_view`,
    /// `observation`, `contract`, `constraints`, `open_residuals` — the
    /// self-referential `envelope_digest` is excluded (see the type doc).
    /// The bytes are a fixed function of the members alone; P2's blindness
    /// proof asserts over them.
    pub fn to_json(&self) -> String {
        // All collection members here are semantic sets. Canonicalize at the
        // serialization boundary as well as builder construction, so direct
        // ports callers cannot create input-order-dependent digests.
        let mut facets = self.observation.facets().to_vec();
        facets.sort_by(|left, right| {
            (
                left.kind().token(),
                left.instrument_name(),
                left.instrument_version(),
                left.command(),
                left.exit_status(),
                left.evidence_ref(),
            )
                .cmp(&(
                    right.kind().token(),
                    right.instrument_name(),
                    right.instrument_version(),
                    right.command(),
                    right.exit_status(),
                    right.evidence_ref(),
                ))
        });
        let mut unavailable = self.observation.unavailable_evidence().to_vec();
        unavailable.sort_by(|left, right| {
            (left.kind().token(), left.reason()).cmp(&(right.kind().token(), right.reason()))
        });
        let mut failures = self.observation.observation_failures().to_vec();
        failures.sort_by(|left, right| {
            (left.kind().token(), left.error_class(), left.detail()).cmp(&(
                right.kind().token(),
                right.error_class(),
                right.detail(),
            ))
        });
        let observation = ObservationView::new(
            self.observation.observation_digest().clone(),
            self.observation.bundle_ref().clone(),
            facets,
            unavailable,
            failures,
        );
        let mut constraints = self.constraints.clone();
        constraints.sort_by(|left, right| {
            (left.id(), left.statement()).cmp(&(right.id(), right.statement()))
        });
        let mut open_residuals = self.open_residuals.clone();
        open_residuals.sort_by(|left, right| {
            (
                left.id().as_str(),
                left.dimension(),
                left.severity().to_string(),
                left.detail(),
            )
                .cmp(&(
                    right.id().as_str(),
                    right.dimension(),
                    right.severity().to_string(),
                    right.detail(),
                ))
        });
        JsonValue::Object(vec![
            (
                "run_id".to_owned(),
                JsonValue::Str(self.run_id.as_str().to_owned()),
            ),
            (
                "iteration_no".to_owned(),
                JsonValue::Number(JsonNumber::Uint(u64::from(self.iteration_no.get()))),
            ),
            (
                "reference_digest".to_owned(),
                JsonValue::Str(self.reference_digest.to_string()),
            ),
            (
                "reference_view".to_owned(),
                JsonValue::Str(self.reference_view.clone()),
            ),
            ("observation".to_owned(), observation.to_json()),
            ("contract".to_owned(), self.contract.to_json()),
            (
                "constraints".to_owned(),
                JsonValue::Array(
                    constraints
                        .iter()
                        .map(ApplicableConstraint::to_json)
                        .collect(),
                ),
            ),
            (
                "open_residuals".to_owned(),
                JsonValue::Array(open_residuals.iter().map(residual_to_json).collect()),
            ),
        ])
        .to_json_text()
    }
}

/// The earned comparison authority for one durable run at one iteration.
///
/// This type is never *constructed* — it is *obtained*. Its sole constructor
/// performs the durable run lookup through the [`StateStore`] port, and the
/// run's own frozen [`ReferenceVersion`] is what comes out of it. No API
/// accepts a caller-supplied reference identity downstream of this point, so
/// choosing the reference is unrepresentable rather than merely rejected.
///
/// Every authority-bearing step of the comparison chain — loading the frozen
/// projection, assembling the envelope, minting the dispatch context, and
/// durable acceptance — demands a reference to this token. The residual trust
/// assumption is therefore exactly one: the `StateStore` injected at the
/// composition root is the real one (spec §7.1 adapter selection), not a
/// public convenience constructor anywhere in the tree.
#[derive(Debug, Clone)]
pub struct RunComparisonAuthority {
    run_id: RunId,
    iteration_no: IterationNo,
    reference: ReferenceVersion,
    observation: ObservationBinding,
}

impl RunComparisonAuthority {
    /// Opens the authority for a durable run at one iteration. Two durable
    /// reads happen here and nowhere else:
    ///
    /// 1. the run itself — refused when it was never made durable, because an
    ///    undurable run can produce no consequential comparison (§18.1); its
    ///    own frozen [`ReferenceVersion`] is carried out;
    /// 2. the iteration's **observation binding** — refused when the
    ///    observation owner has bound none. This is what terminates
    ///    observation provenance in durable run state: a bundle is this run's
    ///    observation because the run records it, not because the bundle is
    ///    well-formed, correctly hashed, pinned, and says so.
    pub fn open(
        state: &dyn StateStore,
        run_id: &RunId,
        iteration_no: IterationNo,
        deadline: Deadline,
    ) -> Result<Self, ComparatorError> {
        let run = state
            .load_run(run_id, deadline)
            .map_err(|error| ComparatorError::EnvelopeInvalid {
                detail: format!("loading durable run {run_id} failed: {error}"),
            })?
            .ok_or_else(|| ComparatorError::EnvelopeInvalid {
                detail: format!(
                    "run {run_id} is not durable; comparison authority cannot be opened"
                ),
            })?;
        let observation = state
            .observation_binding(run_id, iteration_no, deadline)
            .map_err(|error| ComparatorError::EnvelopeInvalid {
                detail: format!(
                    "reading the durable observation binding of run {run_id} iteration {} \
                     failed: {error}",
                    iteration_no.get()
                ),
            })?
            .ok_or_else(|| ComparatorError::EnvelopeInvalid {
                detail: format!(
                    "run {run_id} iteration {} has no durably bound observation bundle; there \
                     is nothing this comparison could be about",
                    iteration_no.get()
                ),
            })?;
        Ok(Self {
            run_id: run_id.clone(),
            iteration_no,
            reference: run.reference_version().clone(),
            observation,
        })
    }

    pub fn run_id(&self) -> &RunId {
        &self.run_id
    }
    pub fn iteration_no(&self) -> IterationNo {
        self.iteration_no
    }
    /// The run's frozen reference version — the only one this chain accepts.
    pub fn reference(&self) -> &ReferenceVersion {
        &self.reference
    }
    /// The iteration's durably bound observation — its evidence ref AND the
    /// §12.4 digest of its content. No API downstream of this point takes an
    /// observation evidence ref as an argument, so substituting a different
    /// valid bundle is unrepresentable rather than rejected; and because the
    /// content digest is bound too, serving different bytes at the bound ref
    /// is refused rather than compared.
    pub fn observation(&self) -> &ObservationBinding {
        &self.observation
    }

    /// The bound bundle's evidence ref.
    pub fn observation_ref(&self) -> &EvidenceRef {
        self.observation.evidence()
    }
}

/// A provenance-checked rendering of the frozen reference. This port-owned
/// value has no caller-supplied-view constructor: `load` obtains exactly the
/// bytes named by the frozen `ReferenceVersion`, normalizes them under the
/// frozen-reference rules, and verifies the pinned digest.
#[derive(Debug, Clone)]
pub struct FrozenReferenceProjection {
    version: ReferenceVersion,
    view: String,
}

impl FrozenReferenceProjection {
    /// Loads the frozen reference named by the DURABLE RUN's own
    /// [`ReferenceVersion`] — obtained from the earned authority, never from a
    /// caller argument — and verifies the normalized bytes against that pinned
    /// digest. A caller can no longer pair bytes it wrote with a digest it
    /// chose: there is no parameter for either.
    pub fn load(
        authority: &RunComparisonAuthority,
        evidence: &dyn EvidenceStore,
        deadline: Deadline,
    ) -> Result<Self, ComparatorError> {
        Self::load_version(authority.reference(), evidence, deadline)
    }

    /// The same governed load over ANY durable run's frozen
    /// [`ReferenceVersion`] (plan Task 11): the verification chain holds the
    /// run itself from `StateStore::load_run` — its version is exactly as
    /// durable as the comparison authority's. The bytes are loaded at the
    /// version's pinned ref and verified against its pinned digest; neither
    /// is a caller argument.
    pub fn load_version(
        version: &ReferenceVersion,
        evidence: &dyn EvidenceStore,
        deadline: Deadline,
    ) -> Result<Self, ComparatorError> {
        let version = version.clone();
        let raw = evidence
            .get(version.evidence_ref(), deadline)
            .map_err(|error| ComparatorError::EnvelopeInvalid {
                detail: format!("loading frozen reference evidence failed: {error}"),
            })?
            .ok_or_else(|| ComparatorError::EnvelopeInvalid {
                detail: format!(
                    "pinned frozen reference evidence {} is absent",
                    version.evidence_ref()
                ),
            })?;
        let text = std::str::from_utf8(&raw).map_err(|error| ComparatorError::EnvelopeInvalid {
            detail: format!(
                "frozen reference evidence is not UTF-8 at {}",
                error.valid_up_to()
            ),
        })?;
        let folded = text.replace("\r\n", "\n").replace('\r', "\n");
        let mut lines: Vec<&str> = folded.split('\n').collect();
        while matches!(lines.last(), Some(last) if last.is_empty()) {
            lines.pop();
        }
        let normalized = if lines.is_empty() {
            String::new()
        } else {
            format!("{}\n", lines.join("\n"))
        };
        let digest = Digest::new(
            "sha256",
            &sha2::Sha256::digest(normalized.as_bytes())
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>(),
        )
        .expect("sha-256 output is a valid digest");
        if &digest != version.digest() {
            return Err(ComparatorError::ContractMismatch {
                detail: format!(
                    "frozen reference evidence digest {} differs from ReferenceVersion {}",
                    digest,
                    version.digest()
                ),
            });
        }
        Ok(Self {
            version,
            view: normalized,
        })
    }

    pub fn version(&self) -> &ReferenceVersion {
        &self.version
    }
    pub fn digest(&self) -> &Digest {
        self.version.digest()
    }
    pub fn view(&self) -> &str {
        &self.view
    }
}

/// The only dispatchable comparison context. Its fields are intentionally
/// opaque; serializable `ComparisonEnvelope` values remain observational docs
/// but cannot be re-admitted to a comparator. Construction demands the
/// provenance-checked frozen projection and proves the envelope's reference
/// digest and rendered view are exactly that projection.
#[derive(Debug)]
pub struct TrustedComparisonContext {
    envelope: ComparisonEnvelope,
    reference_version: ReferenceVersion,
    /// The earned authority this context was minted under. It rides INSIDE the
    /// opaque capability so durable acceptance can demand it without any caller
    /// being able to supply one: holding a trusted context is the only way to
    /// hold an authority downstream of the admission service.
    authority: RunComparisonAuthority,
}

impl TrustedComparisonContext {
    /// Mints the single dispatch capability for one governed run/iteration.
    ///
    /// This is the ONLY public constructor of comparison authority in the
    /// tree, and every input it does not take is the point:
    ///
    /// - the run, the iteration and the frozen reference come from the earned
    ///   [`RunComparisonAuthority`];
    /// - the observation bundle is read from the store at the authority's
    ///   **durably bound** ref — there is no bundle argument, so a different
    ///   individually valid, correctly hashed, pinned bundle cannot be named;
    /// - the observation view is PROJECTED from those bytes here, so no caller
    ///   composes the facets the critic sees;
    /// - the envelope digest is computed here over the canonical bytes, and
    ///   the P2 structural blindness assertion runs over those same bytes
    ///   before anything is dispatchable.
    ///
    /// The remaining trust assumption is the composition root's adapter
    /// selection (spec §7.1), not a public convenience constructor.
    pub fn issue(
        authority: &RunComparisonAuthority,
        projection: &FrozenReferenceProjection,
        evidence: &dyn EvidenceStore,
        contract: ContractView,
        constraints: Vec<ApplicableConstraint>,
        open_residuals: Vec<Residual>,
        deadline: Deadline,
    ) -> Result<Self, ComparatorError> {
        let bundle_ref = authority.observation_ref();
        let raw = evidence
            .get(bundle_ref, deadline)
            .map_err(|error| ComparatorError::EnvelopeInvalid {
                detail: format!("reading bound observation evidence {bundle_ref} failed: {error}"),
            })?
            .ok_or_else(|| ComparatorError::EnvelopeInvalid {
                detail: format!(
                    "the durably bound observation evidence {bundle_ref} is absent from the \
                     evidence store"
                ),
            })?;
        let observation = ObservationView::project(authority, &raw)?;
        let envelope = ComparisonEnvelope::assemble(
            authority,
            projection,
            observation,
            contract,
            constraints,
            open_residuals,
        );
        // Defense in depth, stated honestly: `ComparisonEnvelope::to_json`
        // emits only fixed literal keys, and `ObservationView::project` never
        // reads the bundle's claims facet, so no reachable input makes this
        // call fire. It guards a FUTURE envelope member, not a live hole, and
        // there is no test that can make it trip from this side — blindness on
        // this path is a property of the closed member list, not of this
        // check. Note also that the check is field-NAME-only: builder-authored
        // prose can legitimately ride in the VALUES of caller-supplied members
        // (`ApplicableConstraint::statement`, `RubricEntry::definition`), which
        // is Task 9's structural claims exclusion doing the real work, not this.
        assert_blind(&envelope.to_json()).map_err(|violation| {
            ComparatorError::EnvelopeInvalid {
                detail: violation.to_string(),
            }
        })?;
        Self::from_frozen_projection(authority, projection.clone(), envelope)
    }

    /// Crate-private: the composition step of [`Self::issue`]. Public
    /// construction from an envelope the caller already holds is exactly the
    /// bypass this chain exists to remove.
    pub(crate) fn from_frozen_projection(
        authority: &RunComparisonAuthority,
        projection: FrozenReferenceProjection,
        envelope: ComparisonEnvelope,
    ) -> Result<Self, ComparatorError> {
        verify_envelope_digest(&envelope)?;
        if envelope.reference_digest != *projection.digest()
            || envelope.reference_view != projection.view()
        {
            return Err(ComparatorError::ContractMismatch {
                detail: "comparison envelope reference view/digest is not the verified frozen projection".to_owned(),
            });
        }
        // The projection and the envelope must both be this run's, at this
        // iteration. Authority is single-run and single-iteration: a context
        // earned for one iteration cannot be re-presented at the next.
        if projection.version().digest() != authority.reference().digest() {
            return Err(ComparatorError::ContractMismatch {
                detail: "the frozen projection is not the durable run's frozen reference"
                    .to_owned(),
            });
        }
        if &envelope.run_id != authority.run_id()
            || envelope.iteration_no != authority.iteration_no()
        {
            return Err(ComparatorError::ContractMismatch {
                detail: format!(
                    "comparison envelope names run {}/iteration {} but the authority is {}/{}",
                    envelope.run_id,
                    envelope.iteration_no.get(),
                    authority.run_id(),
                    authority.iteration_no().get()
                ),
            });
        }
        Ok(Self {
            reference_version: projection.version().clone(),
            envelope,
            authority: authority.clone(),
        })
    }

    /// The governed authority this context was minted under.
    ///
    /// Crate-private. `Comparator` is a public trait whose methods receive
    /// `&TrustedComparisonContext`, so a public accessor would hand every
    /// adapter implementation a clonable authority that outlives the
    /// dispatch — the capability would leak out of every callback.
    pub(crate) fn authority(&self) -> &RunComparisonAuthority {
        &self.authority
    }

    /// Dispatches a comparator for its native B.4 document, wrapping what it
    /// returns as a [`ComparisonProduct`]. This is the only way a document
    /// acquires the provenance durable acceptance demands: `Comparator` stays
    /// implementable by adapters, but the value that counts is minted HERE, by
    /// the holder of the capability, not by the implementation.
    pub fn document_from(
        &self,
        comparator: &dyn Comparator,
        deadline: Deadline,
    ) -> Result<Option<ComparisonProduct<'_>>, ComparatorError> {
        Ok(comparator
            .comparison_document(self, deadline)?
            .map(|document| ComparisonProduct {
                context: self,
                spec: None,
                document,
            }))
    }

    /// Dispatches a comparator for the lossy domain summary (§6.2's declared
    /// signature). A summary is never consequential on its own, so it is not
    /// wrapped as a product.
    pub fn compare_with(
        &self,
        comparator: &dyn Comparator,
        deadline: Deadline,
    ) -> Result<ComparisonRecord, ComparatorError> {
        comparator.compare(self, deadline)
    }

    /// Admits a comparator without polling (the two-phase fanout capability).
    pub fn admit_with(
        &self,
        comparator: &dyn Comparator,
        deadline: Deadline,
    ) -> Result<Option<PendingProduct<'_>>, ComparatorError> {
        Ok(comparator
            .admit(self, deadline)?
            .map(|pending| PendingProduct {
                context: self,
                pending,
            }))
    }

    pub fn envelope(&self) -> &ComparisonEnvelope {
        &self.envelope
    }
    pub fn reference_version(&self) -> &ReferenceVersion {
        &self.reference_version
    }
}

/// The comparison port (spec §6.2). Comparators produce residuals only —
/// a comparator verdict alone never settles a Unit (AK-031, enforced at
/// settlement).
/// A comparison admitted to a runner but not yet collected. This separates
/// prompt admission from fan-in so review modes can admit a bounded batch
/// before their first poll. The handle remains owned by the pending value,
/// preserving invocation identity and cancellation responsibility.
pub trait PendingComparison {
    /// Attempts terminal collection without consuming the pending handle. On
    /// failure the caller still owns this value and MUST call [`Self::cancel`]
    /// before releasing its admission permit, or quarantine that permit when
    /// cancellation cannot confirm termination.
    fn collect(&mut self, deadline: Deadline) -> Result<ComparisonRecordDoc, ComparatorError>;

    /// Cancels an admitted comparison that has not been collected. Fanout uses
    /// this when a sibling fails so no uncollected critic continues consuming
    /// resources after the review has already failed.
    fn cancel(&mut self, _deadline: Deadline) -> Result<(), ComparatorError> {
        Ok(())
    }

    /// The admitted §5.3.5 invocation this comparison was dispatched under,
    /// when the comparator dispatched one. Durable acceptance binds the
    /// returned document to exactly this tuple; deterministic comparators have
    /// no invocation and return `None`.
    fn invocation_spec(&self) -> Option<&RoleInvocationSpec> {
        None
    }
}

/// Mandatory durable acceptance boundary. It has no no-op implementation:
/// every model comparison pins its immutable document then asks the state store
/// to validate and record the admitted run/invocation tuple.
#[derive(Clone)]
pub struct MandatoryComparisonAcceptance {
    state: Arc<dyn StateStore>,
    evidence: Arc<dyn EvidenceStore>,
    /// The timestamp recorded on the [`RunEventKind::InvocationTerminal`]
    /// event this acceptance appends for the comparator invocation it
    /// admits (task26-dcritic-apparatus-repair B3/B7). Fixed at
    /// construction — the exact convention [`RoleInvocation::admitted_at`]
    /// already uses for every invocation the composing runner admits
    /// within one run (the caller's per-run composition-time snapshot,
    /// never recomputed per event).
    at: EpochSeconds,
}

impl MandatoryComparisonAcceptance {
    pub fn new(
        state: Arc<dyn StateStore>,
        evidence: Arc<dyn EvidenceStore>,
        at: EpochSeconds,
    ) -> Self {
        Self {
            state,
            evidence,
            at,
        }
    }

    /// Accepts one comparison from a [`ComparisonProduct`] — a document a
    /// comparator RETURNED under a governed capability. There is no overload
    /// taking a bare document: a caller who has built a `ComparisonRecordDoc`
    /// itself has nothing to pass here, and the state-store port in turn
    /// demands the [`AdmittedComparison`] only this method mints.
    pub fn accept(
        &self,
        product: &ComparisonProduct<'_>,
        deadline: Deadline,
    ) -> Result<AdmittedComparison, ComparatorError> {
        let authority = product.context.authority();
        let document = product.document();
        let spec = product
            .invocation_spec()
            .ok_or_else(|| ComparatorError::ContractMismatch {
                detail: "mandatory acceptance refused a comparison with no admitted invocation \
                         to bind to"
                    .to_owned(),
            })?;
        if &spec.run_id != authority.run_id()
            || spec.iteration_no != authority.iteration_no()
            || document.reference_digest() != authority.reference().digest()
        {
            return Err(ComparatorError::ContractMismatch {
                detail:
                    "mandatory acceptance refused a document outside its governed run authority"
                        .to_owned(),
            });
        }
        // The spec is the COMPARATOR'S account of what it was dispatched with,
        // and `RoleInvocationSpec`/`ContextEnvelope` are publicly constructible.
        // Checking the document against the spec alone therefore lets an
        // implementation close the loop on itself: it can report an envelope
        // this capability never issued and hand back a document bound to that.
        // Anchor the spec to the envelope THIS context actually minted first;
        // everything below then chains to the issued bytes.
        let issued = product.context.envelope();
        if spec.context_envelope.digest() != issued.envelope_digest()
            || spec.context_envelope.body() != issued.to_json()
        {
            return Err(ComparatorError::ContractMismatch {
                detail: "mandatory acceptance refused an invocation whose context envelope is \
                         not the one this capability issued"
                    .to_owned(),
            });
        }
        if document.run_id() != &spec.run_id
            || document.iteration_no() != spec.iteration_no
            || document.envelope_digest() != spec.context_envelope.digest()
        {
            return Err(ComparatorError::ContractMismatch {
                detail:
                    "mandatory acceptance refused a document not bound to its admitted invocation"
                        .to_owned(),
            });
        }
        let evidence = self
            .evidence
            .put(
                document.to_json().as_bytes(),
                EvidenceKind::Transcript,
                deadline,
            )
            .map_err(|error| ComparatorError::ComparisonFailed {
                detail: format!("persisting accepted comparison evidence failed: {error}"),
            })?;
        self.evidence
            .pin(&spec.run_id, &evidence, deadline)
            .map_err(|error| ComparatorError::ComparisonFailed {
                detail: format!("pinning accepted comparison evidence failed: {error}"),
            })?;
        let admitted = AdmittedComparison {
            spec: spec.clone(),
            document: document.clone(),
            evidence,
        };
        self.state
            .accept_comparison(&admitted, deadline)
            .map_err(|error| ComparatorError::ComparisonFailed {
                detail: format!("recording accepted comparison failed: {error}"),
            })?;
        // task26-dcritic-apparatus-repair B3/B7: an accepted comparison IS
        // this invocation's terminal disposition — the comparator's
        // product was collected and durably bound. Without this event the
        // invocation is durably admitted (RunEventKind::InvocationAdmitted,
        // written at dispatch by the runner wrapper) but never durably
        // TERMINAL, so it is invisible to every reader that reconstructs
        // "which invocations actually ran" from the event log alone
        // (gauntlet_app::baseline::metrics::derive_loop_metrics's
        // review_dispatches, and any other lifecycle-fact consumer) — the
        // exact asymmetry independently confirmed against durable state:
        // an accepted critic comparison left `role_invocation` rows and a
        // `comparison` record, but zero InvocationTerminal events, so
        // `review_dispatches` silently read back 0 despite the critic
        // genuinely having run and its product having been accepted.
        // Mirrors controller.rs's own InvocationTerminal write for the
        // builder dispatch path (the same event kind, the same meaning:
        // "this invocation reached a terminal state", spec §5.3.5) so a
        // durable reader never has to know which dispatch loop produced
        // the invocation to find its terminal fact.
        self.state
            .append_event(
                RunEvent {
                    run_id: spec.run_id.clone(),
                    kind: RunEventKind::InvocationTerminal {
                        invocation_id: spec.invocation_id.clone(),
                        state: InvocationState::Completed,
                    },
                    at: self.at,
                },
                deadline,
            )
            .map_err(|error| ComparatorError::ComparisonFailed {
                detail: format!(
                    "recording the accepted comparison's invocation terminal failed: {error}"
                ),
            })?;
        Ok(admitted)
    }
}

// ---------------------------------------------------------------------------
// P2 structural blindness (AK-026/AK-027) — the deny-list assertion over the
// SERIALIZED envelope. Spec §13.2: the comparator "does not receive builder
// conversation history, builder reasoning, implementation rationalizations,
// attempt counts, builder identity (when blind comparison is feasible), prior
// verdicts on this unit, or operator chat".
//
// This lives in the PORT, not in an application helper, because it must run
// inside [`TrustedComparisonContext::issue`]. An assertion a caller could
// route around by minting a capability some other way is not an assertion.
// ---------------------------------------------------------------------------

/// The §13.2 deny-list's stable object-field vocabulary. The minting path
/// rejects these names anywhere in the parsed envelope shape; it does not
/// reject identical words quoted inside legitimate frozen-reference prose, or
/// inside instrument-produced values such as an executed command. Text that
/// merely ORIGINATED with a builder is not thereby contraband — Task 9's
/// structural claims exclusion is what keeps builder CLAIMS out, and it does
/// so by giving them no representation at all.
pub const DENY_LISTED_FIELD_NAMES: &[&str] = &[
    "builder_conversation",
    "builder_history",
    "builder_reasoning",
    "implementation_rationale",
    "builder_rationale",
    "attempt_count",
    "attempt_counts",
    "builder_identity",
    "prior_verdicts",
    "operator_chat",
    "claims",
    "claims_facet",
];

/// A blindness violation over serialized envelope bytes (P2; AK-027's
/// designed-to-fail pair targets this check).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlindnessViolation {
    /// The deny-listed field name found in the bytes.
    pub field: &'static str,
}

impl fmt::Display for BlindnessViolation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "the serialized envelope carries deny-listed field {:?} — structural blindness \
             is violated (spec 13.2, P2)",
            self.field
        )
    }
}

impl std::error::Error for BlindnessViolation {}

/// The P2 structural assertion: the serialized envelope has none of the
/// §13.2 deny-listed *object fields*. It deliberately parses the envelope
/// rather than scanning prose values: the frozen reference may legitimately
/// quote a prohibited word, and an instrument-produced value such as an exact
/// executed command may legitimately contain text a builder wrote, while an
/// injected builder field is never legitimate.
pub fn assert_blind(bytes: &str) -> Result<(), BlindnessViolation> {
    let value = crate::json::parse(bytes).map_err(|_| BlindnessViolation {
        field: "invalid_envelope_json",
    })?;
    fn visit(value: &JsonValue) -> Option<&'static str> {
        match value {
            JsonValue::Object(entries) => {
                for (key, child) in entries {
                    for denied in DENY_LISTED_FIELD_NAMES {
                        if key == denied {
                            return Some(denied);
                        }
                    }
                    if let Some(denied) = visit(child) {
                        return Some(denied);
                    }
                }
                None
            }
            JsonValue::Array(values) => values.iter().find_map(visit),
            _ => None,
        }
    }
    visit(&value)
        .map(|field| Err(BlindnessViolation { field }))
        .unwrap_or(Ok(()))
}

/// A comparison document that a comparator RETURNED under a governed dispatch
/// capability. It has no public constructor: a `ComparisonRecordDoc` a caller
/// merely built has nothing that can carry it to durable acceptance.
///
/// It borrows the capability it was produced under, so a product cannot
/// outlive the single-use context that minted it.
pub struct ComparisonProduct<'a> {
    context: &'a TrustedComparisonContext,
    spec: Option<RoleInvocationSpec>,
    document: ComparisonRecordDoc,
}

impl ComparisonProduct<'_> {
    /// The returned document, for aggregation and inspection. Reading it is
    /// harmless; it is durable acceptance that demands the product itself.
    pub fn document(&self) -> &ComparisonRecordDoc {
        &self.document
    }

    /// The admitted invocation this document is bound to, when the comparator
    /// dispatched one. Deterministic comparators have none.
    pub fn invocation_spec(&self) -> Option<&RoleInvocationSpec> {
        self.spec.as_ref()
    }

    /// Consumes the product for its document (aggregation input).
    pub fn into_document(self) -> ComparisonRecordDoc {
        self.document
    }
}

/// An admitted, not-yet-collected comparison, bound to the capability it was
/// admitted under. Collection yields a [`ComparisonProduct`].
pub struct PendingProduct<'a> {
    context: &'a TrustedComparisonContext,
    pending: Box<dyn PendingComparison>,
}

impl<'a> PendingProduct<'a> {
    /// Attempts terminal collection without consuming the handle. On failure
    /// the caller still owns this value and MUST call [`Self::cancel`] before
    /// releasing its admission permit.
    pub fn collect(
        &mut self,
        deadline: Deadline,
    ) -> Result<ComparisonProduct<'a>, ComparatorError> {
        let document = self.pending.collect(deadline)?;
        Ok(ComparisonProduct {
            context: self.context,
            spec: self.pending.invocation_spec().cloned(),
            document,
        })
    }

    /// Cancels an admitted comparison that has not been collected.
    pub fn cancel(&mut self, deadline: Deadline) -> Result<(), ComparatorError> {
        self.pending.cancel(deadline)
    }
}

/// A durably accepted comparison. Minted only by
/// [`MandatoryComparisonAcceptance::accept`], which is the only code that
/// pins the document and cross-checks its invocation tuple. The state-store
/// port demands one of these, so there is no ungoverned durable comparison
/// append anywhere in the tree.
pub struct AdmittedComparison {
    spec: RoleInvocationSpec,
    document: ComparisonRecordDoc,
    evidence: EvidenceRef,
}

impl AdmittedComparison {
    /// The admitted critic invocation the record is bound to (§5.3.5).
    pub fn spec(&self) -> &RoleInvocationSpec {
        &self.spec
    }
    /// The accepted `comparison_record@v1` document (Annex B.4).
    pub fn document(&self) -> &ComparisonRecordDoc {
        &self.document
    }
    /// The pinned, content-addressed evidence the document was written to.
    pub fn evidence(&self) -> &EvidenceRef {
        &self.evidence
    }
}

/// A semantic factory for regular comparator work. The application selects
/// review mode; adapters supply comparators without exposing vendor details.
pub trait ComparatorSource {
    fn across_dimensions(&self) -> Result<Box<dyn Comparator>, String>;
    fn for_dimension(&self, dimension: &str) -> Result<Box<dyn Comparator>, String>;
}

/// A separate semantic capability for the required high-assurance adversarial
/// pass. A normal [`ComparatorSource`] cannot occupy this slot merely by
/// attaching a label. Concrete adapter factories must construct comparators
/// with their adversarial instruction enabled.
pub trait AdversarialComparatorSource {
    fn adversarial_across_dimensions(&self) -> Result<AdversarialComparator, String>;
}

/// The distinct type the high-assurance slot accepts. `review` will not take a
/// `Box<dyn Comparator>` here, so the slot cannot be filled by passing an
/// ordinary comparator where an adversarial one was expected.
///
/// **What this does NOT prove, stated plainly:** the constructor
/// [`AdversarialComparator::instructed`] wraps whatever comparator an adapter
/// hands it. That the wrapped comparator genuinely carries the adversarial
/// instruction is an obligation of the adapter that builds it — the same trust
/// level as the `StateStore` or `EvidenceStore` chosen at the composition root
/// (spec §7.1), and not a property this type verifies. An earlier version of
/// this doc claimed the type made relabelling impossible; it does not.
///
/// **Recorded residual (Task 10):** durable attestation that the adversarial
/// INSTRUCTION was the one dispatched would need a carrier on `RoleInvocation`
/// (§5.3.5) or `comparator_identity` (Annex B.4); neither exists in the frozen
/// reference, and no Task-10 plan step or AK claims it. It is ledgered as
/// later scope, not claimed here.
pub struct AdversarialComparator(Box<dyn Comparator>);

impl AdversarialComparator {
    /// Wraps a comparator the ADAPTER has already constructed with its
    /// adversarial instruction enabled. This constructor takes the adapter's
    /// word for that; see the type doc for exactly what is and is not proven.
    pub fn instructed(comparator: Box<dyn Comparator>) -> Self {
        Self(comparator)
    }

    pub fn as_comparator(&self) -> &dyn Comparator {
        self.0.as_ref()
    }
}

pub trait Comparator {
    /// Compares the observation against the reference view under the
    /// envelope's contract. Blocking (model-backed adapters exist), so a
    /// deadline is required (§6 preamble).
    fn compare(
        &self,
        context: &TrustedComparisonContext,
        deadline: Deadline,
    ) -> Result<ComparisonRecord, ComparatorError>;

    /// The full B.4 document, when this comparator natively produces one.
    /// Review aggregation uses it directly so wire-level residual evidence,
    /// hard gates, and comparator provenance are never reboxed through the
    /// lossy domain summary. Legacy/domain-only comparators return `None`.
    fn comparison_document(
        &self,
        _context: &TrustedComparisonContext,
        _deadline: Deadline,
    ) -> Result<Option<ComparisonRecordDoc>, ComparatorError> {
        Ok(None)
    }

    /// Admits without polling. `Some` is the two-phase capability used by
    /// fanout; `None` preserves compatibility for deterministic comparators.
    fn admit(
        &self,
        _context: &TrustedComparisonContext,
        _deadline: Deadline,
    ) -> Result<Option<Box<dyn PendingComparison>>, ComparatorError> {
        Ok(None)
    }
}

#[cfg(test)]
mod canonical_tests {
    use super::{
        ApplicableConstraint, ComparisonEnvelope, ContractView, FacetSummary, ObservationView,
    };
    use crate::observer::FacetKind;
    use gauntlet_domain::{Digest, EvidenceRef, IterationNo, RunId};

    fn envelope(reversed: bool) -> ComparisonEnvelope {
        let mut facets = vec![
            FacetSummary::new(
                FacetKind::Test,
                "z".into(),
                "1".into(),
                None,
                Some(0),
                "evd-z".into(),
            ),
            FacetSummary::new(
                FacetKind::Build,
                "a".into(),
                "1".into(),
                None,
                Some(0),
                "evd-a".into(),
            ),
        ];
        let mut dimensions = vec!["security".to_owned(), "functional behavior".to_owned()];
        let mut constraints = vec![
            ApplicableConstraint::new("z".into(), "last".into()),
            ApplicableConstraint::new("a".into(), "first".into()),
        ];
        if reversed {
            facets.reverse();
            dimensions.reverse();
            constraints.reverse();
        }
        ComparisonEnvelope {
            run_id: RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("run"),
            iteration_no: IterationNo::new(1).expect("iteration"),
            reference_digest: Digest::new("sha256", "aa11").expect("digest"),
            reference_view: "reference".into(),
            observation: ObservationView::new(
                Digest::new("sha256", "bb22").expect("digest"),
                EvidenceRef::new("evd-bundle-0001").expect("evidence"),
                facets,
                vec![],
                vec![],
            ),
            contract: ContractView::new("1".into(), dimensions, vec![]),
            constraints,
            open_residuals: vec![],
            envelope_digest: Digest::new("sha256", "cc33").expect("digest"),
        }
    }

    #[test]
    fn canonical_envelope_ignores_semantically_unordered_input_order() {
        assert_eq!(envelope(false).to_json(), envelope(true).to_json());
    }

    /// Teeth for [`super::verify_envelope_digest`].
    ///
    /// From OUTSIDE this crate a stale envelope is unrepresentable — every
    /// field is private and `assemble` computes the digest — so no external
    /// test can make the check fire, and a test that merely asserts `is_ok()`
    /// on a well-formed envelope proves nothing about the refusal. This test
    /// builds the stale value in-crate, where the fields are reachable, so the
    /// refusal branch is exercised rather than assumed.
    #[test]
    fn an_envelope_whose_digest_does_not_label_its_bytes_is_refused() {
        let honest = {
            let mut value = envelope(false);
            value.envelope_digest = super::calculate_envelope_digest(&value);
            value
        };
        super::verify_envelope_digest(&honest).expect("a self-consistent envelope verifies");

        let mut stale = honest.clone();
        stale.reference_view = "a different rendering of the frozen standard".into();
        let refusal = super::verify_envelope_digest(&stale);
        assert!(
            matches!(
                refusal,
                Err(super::ComparatorError::ContractMismatch { .. })
            ),
            "an envelope whose digest labels other bytes must be refused, got {refusal:?}"
        );
    }
}
