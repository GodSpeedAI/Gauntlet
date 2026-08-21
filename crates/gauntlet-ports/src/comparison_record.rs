//! `comparison_record@v1` — the typed Annex B.4 mirror (plan Task 10).
//!
//! The wire key order below is the annex's documented order; canonical
//! serialization is fixed construction order over the order-preserving
//! `gauntlet_ports::json` serializer, so the bytes are a function of the
//! values alone. Decoding is STRICT (mirroring `role_result`
//! conventions): unknown keys are mechanically rejected for the document
//! and every nested object, so a settlement-adjacent document can never be
//! partially parsed into state (spec §10.3 rule 4, §21.2).
//!
//! One repo-documented extension of the annex's example: the
//! `envelope_digest` member. Spec §13.2 requires the serialized envelope's
//! digest to be "stored on both the RoleInvocation and the
//! ComparisonRecord"; Annex B.4's example does not spell the field, and
//! B.4 is a MINIMUM shape — the same reconciliation class as Task 9's
//! environment-fingerprint-in-evidence-doc. The field is REQUIRED here.
//!
//! Payload form (spec Annex B.1): inside a `role_result@v1` envelope the
//! payload object carries the SAME fields minus the `schema` token (the
//! envelope's `payload_schema` names it) — the convention
//! `builder_result@v1` already established.

use core::fmt;

use gauntlet_domain::{
    Digest, EvidenceRef, IterationNo, ResidualId, RunId, Severity, UnitId, Verdict,
};

use crate::json::{JsonError, JsonNumber, JsonValue};

/// The document's schema token (spec Annex B.4).
pub const COMPARISON_RECORD_SCHEMA: &str = "comparison_record@v1";

/// The document's declared fields, in the annex's documented order (plus
/// the §13.2-required `envelope_digest`, inserted after
/// `observation_digest`). Any other key in a decoded document is rejected
/// as unknown.
const RECORD_FIELDS: &[&str] = &[
    "schema",
    "run_id",
    "iteration_no",
    "reference_digest",
    "observation_digest",
    "envelope_digest",
    "comparison_contract_version",
    "comparator_identity",
    "residuals",
    "hard_gate_results",
    "verdict",
];

/// The `comparator_identity` object's declared fields (B.4).
const IDENTITY_FIELDS: &[&str] = &["runner", "model", "dimension"];

/// The `residuals` entries' declared fields (B.4), in annex order.
const RESIDUAL_FIELDS: &[&str] = &[
    "id",
    "dimension",
    "unit_refs",
    "expected",
    "observed",
    "evidence_refs",
    "severity",
    "confidence",
    "disputed",
    "diagnosis",
    "cheapest_discriminating_test",
    "candidate_corrections",
];

/// The `hard_gate_results` entries' declared fields (B.4).
const GATE_FIELDS: &[&str] = &["unit_id", "held", "evidence_refs"];

/// The `verdict` object's declared fields (B.4).
const VERDICT_FIELDS: &[&str] = &["status", "rationale"];

/// `comparison_record@v1` decode failures — typed, with the schema locus
/// and the offending field wherever one exists (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum ComparisonDocError {
    /// The document is not valid JSON.
    Json(JsonError),
    /// The `schema` token is not `comparison_record@v1`.
    SchemaUnsupported { found: String },
    /// An object carries a field outside its schema.
    UnknownField { field: String },
    /// A declared field is absent.
    MissingField { field: &'static str },
    /// A field has the wrong JSON shape.
    TypeInvalid {
        field: &'static str,
        expected: &'static str,
    },
    /// A field has the right shape but an invalid value (bad ULID, zero
    /// iteration, unknown severity or verdict token, invalid digest).
    ValueInvalid { field: &'static str, source: String },
}

impl fmt::Display for ComparisonDocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "comparison record: {err}"),
            Self::SchemaUnsupported { found } => write!(
                f,
                "comparison record: schema token {found:?} is not {COMPARISON_RECORD_SCHEMA:?}"
            ),
            Self::UnknownField { field } => write!(
                f,
                "comparison record: {COMPARISON_RECORD_SCHEMA} rejects unknown field {field:?}"
            ),
            Self::MissingField { field } => write!(
                f,
                "comparison record: {COMPARISON_RECORD_SCHEMA} is missing field {field:?}"
            ),
            Self::TypeInvalid { field, expected } => write!(
                f,
                "comparison record: {COMPARISON_RECORD_SCHEMA} field {field:?} is not {expected}"
            ),
            Self::ValueInvalid { field, source } => write!(
                f,
                "comparison record: {COMPARISON_RECORD_SCHEMA} field {field:?}: {source}"
            ),
        }
    }
}

impl std::error::Error for ComparisonDocError {}

impl From<JsonError> for ComparisonDocError {
    fn from(source: JsonError) -> Self {
        Self::Json(source)
    }
}

/// The comparator's recorded provenance (Annex B.4 `comparator_identity`;
/// AK-028-class): which runner produced the record, which model (empty for
/// deterministic comparators), and which dimension the record speaks to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComparatorIdentity {
    runner: String,
    model: String,
    dimension: String,
}

impl ComparatorIdentity {
    pub fn new(runner: String, model: String, dimension: String) -> Self {
        Self {
            runner,
            model,
            dimension,
        }
    }

    pub fn runner(&self) -> &str {
        &self.runner
    }

    /// The model identity, empty for deterministic comparators.
    pub fn model(&self) -> &str {
        &self.model
    }

    pub fn dimension(&self) -> &str {
        &self.dimension
    }

    fn to_json(&self) -> JsonValue {
        let values = vec![
            JsonValue::Str(self.runner.clone()),
            JsonValue::Str(self.model.clone()),
            JsonValue::Str(self.dimension.clone()),
        ];
        JsonValue::Object(
            IDENTITY_FIELDS
                .iter()
                .zip(values)
                .map(|(field, value)| ((*field).to_owned(), value))
                .collect(),
        )
    }

    fn from_json(value: &JsonValue) -> Result<Self, ComparisonDocError> {
        let JsonValue::Object(_) = value else {
            return Err(ComparisonDocError::TypeInvalid {
                field: "comparator_identity",
                expected: "an object",
            });
        };
        reject_unknown(value, IDENTITY_FIELDS)?;
        Ok(Self {
            runner: non_empty_string(value, "comparator_identity.runner")?,
            model: string_field(value, "comparator_identity.model")?.to_owned(),
            dimension: string_field(value, "comparator_identity.dimension")?.to_owned(),
        })
    }
}

/// One residual as the B.4 wire record (a superset of the domain
/// `Residual`: the wire form carries the expected/observed pair, unit
/// refs, evidence refs, confidence, the dispute flag, diagnosis, and the
/// correction-selection fields).
#[derive(Debug, Clone, PartialEq)]
pub struct ResidualDoc {
    id: ResidualId,
    dimension: String,
    unit_refs: Vec<UnitId>,
    expected: String,
    observed: String,
    evidence_refs: Vec<EvidenceRef>,
    severity: Severity,
    confidence: f64,
    disputed: bool,
    diagnosis: String,
    cheapest_discriminating_test: String,
    candidate_corrections: Vec<String>,
}

impl ResidualDoc {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: ResidualId,
        dimension: String,
        unit_refs: Vec<UnitId>,
        expected: String,
        observed: String,
        evidence_refs: Vec<EvidenceRef>,
        severity: Severity,
        confidence: f64,
        disputed: bool,
        diagnosis: String,
        cheapest_discriminating_test: String,
        candidate_corrections: Vec<String>,
    ) -> Self {
        Self {
            id,
            dimension,
            unit_refs,
            expected,
            observed,
            evidence_refs,
            severity,
            confidence,
            disputed,
            diagnosis,
            cheapest_discriminating_test,
            candidate_corrections,
        }
    }

    /// The merge key's dimension component (spec §13.5: residuals are
    /// keyed by `(dimension, expected, observed_locator)`).
    pub fn id(&self) -> &ResidualId {
        &self.id
    }

    pub fn dimension(&self) -> &str {
        &self.dimension
    }

    pub fn unit_refs(&self) -> &[UnitId] {
        &self.unit_refs
    }

    pub fn expected(&self) -> &str {
        &self.expected
    }

    /// The observed LOCATOR — the §13.5 merge key's third component.
    pub fn observed(&self) -> &str {
        &self.observed
    }

    pub fn evidence_refs(&self) -> &[EvidenceRef] {
        &self.evidence_refs
    }

    pub fn severity(&self) -> Severity {
        self.severity
    }

    pub fn confidence(&self) -> f64 {
        self.confidence
    }

    pub fn disputed(&self) -> bool {
        self.disputed
    }

    pub fn diagnosis(&self) -> &str {
        &self.diagnosis
    }

    pub fn cheapest_discriminating_test(&self) -> &str {
        &self.cheapest_discriminating_test
    }

    pub fn candidate_corrections(&self) -> &[String] {
        &self.candidate_corrections
    }

    /// Spec §13.5: a blocking residual forces an aggregate fail verdict.
    pub fn is_blocking(&self) -> bool {
        self.severity == Severity::Blocking
    }

    /// The §13.5 merge key: `(dimension, expected, observed_locator)`.
    pub fn merge_key(&self) -> (&str, &str, &str) {
        (
            self.dimension.as_str(),
            self.expected.as_str(),
            self.observed.as_str(),
        )
    }

    fn to_json(&self) -> JsonValue {
        let values = vec![
            JsonValue::Str(self.id.as_str().to_owned()),
            JsonValue::Str(self.dimension.clone()),
            JsonValue::Array(
                self.unit_refs
                    .iter()
                    .map(|unit| JsonValue::Str(unit.as_str().to_owned()))
                    .collect(),
            ),
            JsonValue::Str(self.expected.clone()),
            JsonValue::Str(self.observed.clone()),
            JsonValue::Array(
                self.evidence_refs
                    .iter()
                    .map(|evidence| JsonValue::Str(evidence.as_str().to_owned()))
                    .collect(),
            ),
            JsonValue::Str(self.severity.to_string()),
            JsonValue::Number(JsonNumber::Float(self.confidence)),
            JsonValue::Bool(self.disputed),
            JsonValue::Str(self.diagnosis.clone()),
            JsonValue::Str(self.cheapest_discriminating_test.clone()),
            JsonValue::Array(
                self.candidate_corrections
                    .iter()
                    .map(|correction| JsonValue::Str(correction.clone()))
                    .collect(),
            ),
        ];
        JsonValue::Object(
            RESIDUAL_FIELDS
                .iter()
                .zip(values)
                .map(|(field, value)| ((*field).to_owned(), value))
                .collect(),
        )
    }

    fn from_json(value: &JsonValue) -> Result<Self, ComparisonDocError> {
        let JsonValue::Object(_) = value else {
            return Err(ComparisonDocError::TypeInvalid {
                field: "residuals",
                expected: "an array of objects",
            });
        };
        reject_unknown(value, RESIDUAL_FIELDS)?;
        let id = ResidualId::new(string_field(value, "residuals.id")?)
            .map_err(|source| value_invalid("residuals.id", source))?;
        let dimension = string_field(value, "residuals.dimension")?.to_owned();
        let unit_refs = string_array(value, "residuals.unit_refs")?
            .iter()
            .map(|raw| {
                UnitId::new(raw).map_err(|source| value_invalid("residuals.unit_refs", source))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let expected = string_field(value, "residuals.expected")?.to_owned();
        let observed = string_field(value, "residuals.observed")?.to_owned();
        let evidence_refs = string_array(value, "residuals.evidence_refs")?
            .iter()
            .map(|raw| {
                EvidenceRef::new(raw)
                    .map_err(|source| value_invalid("residuals.evidence_refs", source))
            })
            .collect::<Result<Vec<_>, _>>()?;
        let severity = Severity::parse(string_field(value, "residuals.severity")?)
            .map_err(|source| value_invalid("residuals.severity", source))?;
        let confidence = match value.get("confidence") {
            Some(JsonValue::Number(number)) => number.as_f64(),
            Some(_) => {
                return Err(ComparisonDocError::TypeInvalid {
                    field: "residuals.confidence",
                    expected: "a number",
                });
            }
            None => {
                return Err(ComparisonDocError::MissingField {
                    field: "residuals.confidence",
                });
            }
        };
        let disputed = match value.get("disputed") {
            Some(JsonValue::Bool(disputed)) => *disputed,
            Some(_) => {
                return Err(ComparisonDocError::TypeInvalid {
                    field: "residuals.disputed",
                    expected: "a boolean",
                });
            }
            None => {
                return Err(ComparisonDocError::MissingField {
                    field: "residuals.disputed",
                });
            }
        };
        Ok(Self {
            id,
            dimension,
            unit_refs,
            expected,
            observed,
            evidence_refs,
            severity,
            confidence,
            disputed,
            diagnosis: string_field(value, "residuals.diagnosis")?.to_owned(),
            cheapest_discriminating_test: string_field(
                value,
                "residuals.cheapest_discriminating_test",
            )?
            .to_owned(),
            candidate_corrections: string_array(value, "residuals.candidate_corrections")?,
        })
    }
}

/// One hard-gate result (Annex B.4): a Unit whose blocking requirement was
/// evaluated, whether it held, and the evidence.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HardGateResult {
    unit_id: UnitId,
    held: bool,
    evidence_refs: Vec<EvidenceRef>,
}

impl HardGateResult {
    pub fn new(unit_id: UnitId, held: bool, evidence_refs: Vec<EvidenceRef>) -> Self {
        Self {
            unit_id,
            held,
            evidence_refs,
        }
    }

    pub fn unit_id(&self) -> &UnitId {
        &self.unit_id
    }

    pub fn held(&self) -> bool {
        self.held
    }

    pub fn evidence_refs(&self) -> &[EvidenceRef] {
        &self.evidence_refs
    }

    fn to_json(&self) -> JsonValue {
        let values = vec![
            JsonValue::Str(self.unit_id.as_str().to_owned()),
            JsonValue::Bool(self.held),
            JsonValue::Array(
                self.evidence_refs
                    .iter()
                    .map(|evidence| JsonValue::Str(evidence.as_str().to_owned()))
                    .collect(),
            ),
        ];
        JsonValue::Object(
            GATE_FIELDS
                .iter()
                .zip(values)
                .map(|(field, value)| ((*field).to_owned(), value))
                .collect(),
        )
    }

    fn from_json(value: &JsonValue) -> Result<Self, ComparisonDocError> {
        let JsonValue::Object(_) = value else {
            return Err(ComparisonDocError::TypeInvalid {
                field: "hard_gate_results",
                expected: "an array of objects",
            });
        };
        reject_unknown(value, GATE_FIELDS)?;
        let unit_id = UnitId::new(string_field(value, "hard_gate_results.unit_id")?)
            .map_err(|source| value_invalid("hard_gate_results.unit_id", source))?;
        let held = match value.get("held") {
            Some(JsonValue::Bool(held)) => *held,
            Some(_) => {
                return Err(ComparisonDocError::TypeInvalid {
                    field: "hard_gate_results.held",
                    expected: "a boolean",
                });
            }
            None => {
                return Err(ComparisonDocError::MissingField {
                    field: "hard_gate_results.held",
                });
            }
        };
        let evidence_refs = string_array(value, "hard_gate_results.evidence_refs")?
            .iter()
            .map(|raw| {
                EvidenceRef::new(raw)
                    .map_err(|source| value_invalid("hard_gate_results.evidence_refs", source))
            })
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            unit_id,
            held,
            evidence_refs,
        })
    }
}

/// The record's verdict (Annex B.4 `verdict`): the three-state status plus
/// the rationale.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordVerdict {
    status: Verdict,
    rationale: String,
}

impl RecordVerdict {
    pub fn new(status: Verdict, rationale: String) -> Self {
        Self { status, rationale }
    }

    pub fn status(&self) -> Verdict {
        self.status
    }

    pub fn rationale(&self) -> &str {
        &self.rationale
    }

    fn to_json(&self) -> JsonValue {
        let values = vec![
            JsonValue::Str(self.status.to_string()),
            JsonValue::Str(self.rationale.clone()),
        ];
        JsonValue::Object(
            VERDICT_FIELDS
                .iter()
                .zip(values)
                .map(|(field, value)| ((*field).to_owned(), value))
                .collect(),
        )
    }

    fn from_json(value: &JsonValue) -> Result<Self, ComparisonDocError> {
        let JsonValue::Object(_) = value else {
            return Err(ComparisonDocError::TypeInvalid {
                field: "verdict",
                expected: "an object",
            });
        };
        reject_unknown(value, VERDICT_FIELDS)?;
        let status = Verdict::parse(string_field(value, "verdict.status")?)
            .map_err(|source| value_invalid("verdict.status", source))?;
        Ok(Self {
            status,
            rationale: string_field(value, "verdict.rationale")?.to_owned(),
        })
    }
}

/// Everything a `comparison_record@v1` document carries besides its schema
/// token, as a component bag (construction and rebuild-with-modification
/// both go through it).
#[derive(Debug, Clone, PartialEq)]
pub struct RecordContent {
    pub run_id: RunId,
    pub iteration_no: IterationNo,
    pub reference_digest: Digest,
    pub observation_digest: Digest,
    pub envelope_digest: Digest,
    pub contract_version: String,
    pub identity: ComparatorIdentity,
    pub residuals: Vec<ResidualDoc>,
    pub hard_gate_results: Vec<HardGateResult>,
    pub verdict: RecordVerdict,
}

/// The typed `comparison_record@v1` document (spec Annex B.4).
#[derive(Debug, Clone, PartialEq)]
pub struct ComparisonRecordDoc {
    content: RecordContent,
}

impl ComparisonRecordDoc {
    /// Adopts the content bag; the schema token is a constant of the type.
    pub fn new(content: RecordContent) -> Self {
        Self { content }
    }

    /// The content bag, for access or rebuild-with-modification.
    pub fn content(&self) -> RecordContent {
        self.content.clone()
    }

    pub fn run_id(&self) -> &RunId {
        &self.content.run_id
    }

    pub fn iteration_no(&self) -> IterationNo {
        self.content.iteration_no
    }

    pub fn reference_digest(&self) -> &Digest {
        &self.content.reference_digest
    }

    pub fn observation_digest(&self) -> &Digest {
        &self.content.observation_digest
    }

    /// The §13.2-required digest of the envelope this record was produced
    /// under (the documented B.4-minimum-shape extension; see the module
    /// doc).
    pub fn envelope_digest(&self) -> &Digest {
        &self.content.envelope_digest
    }

    pub fn contract_version(&self) -> &str {
        &self.content.contract_version
    }

    pub fn identity(&self) -> &ComparatorIdentity {
        &self.content.identity
    }

    pub fn residuals(&self) -> &[ResidualDoc] {
        &self.content.residuals
    }

    pub fn hard_gate_results(&self) -> &[HardGateResult] {
        &self.content.hard_gate_results
    }

    pub fn verdict(&self) -> &RecordVerdict {
        &self.content.verdict
    }

    /// Canonical serialization: keys in the annex's documented order, the
    /// `schema` token included.
    pub fn to_json(&self) -> String {
        self.object_with_schema().to_json_text()
    }

    /// The payload form (Annex B.1): the same fields MINUS the `schema`
    /// token, for embedding inside a `role_result@v1` envelope whose
    /// `payload_schema` names this schema.
    pub fn to_payload_json(&self) -> String {
        self.fields().to_json_text()
    }

    /// Decodes and strictly validates a standalone `comparison_record@v1`
    /// document (schema token included).
    pub fn decode(text: &str) -> Result<Self, ComparisonDocError> {
        let document = crate::json::parse(text)?;
        let JsonValue::Object(_) = &document else {
            return Err(ComparisonDocError::TypeInvalid {
                field: "<root>",
                expected: "an object",
            });
        };
        reject_unknown(&document, RECORD_FIELDS)?;
        let schema = string_field(&document, "schema")?;
        if schema != COMPARISON_RECORD_SCHEMA {
            return Err(ComparisonDocError::SchemaUnsupported {
                found: schema.to_owned(),
            });
        }
        let content = Self::content_from_object(&document)?;
        Ok(Self { content })
    }

    /// Decodes the payload form (no `schema` member — the enclosing
    /// envelope's `payload_schema` names it).
    pub fn decode_payload(value: &JsonValue) -> Result<Self, ComparisonDocError> {
        let JsonValue::Object(_) = value else {
            return Err(ComparisonDocError::TypeInvalid {
                field: "<payload>",
                expected: "an object",
            });
        };
        reject_unknown(value, &RECORD_FIELDS[1..])?;
        Ok(Self {
            content: Self::content_from_object(value)?,
        })
    }

    fn content_from_object(document: &JsonValue) -> Result<RecordContent, ComparisonDocError> {
        let run_id = RunId::new(string_field(document, "run_id")?)
            .map_err(|source| value_invalid("run_id", source))?;
        let iteration_no = match document.get("iteration_no") {
            Some(JsonValue::Number(JsonNumber::Uint(n))) => {
                let n = u32::try_from(*n).map_err(|_| ComparisonDocError::ValueInvalid {
                    field: "iteration_no",
                    source: format!("iteration {n} exceeds the u32 iteration space"),
                })?;
                IterationNo::new(n).map_err(|source| value_invalid("iteration_no", source))?
            }
            Some(_) => {
                return Err(ComparisonDocError::TypeInvalid {
                    field: "iteration_no",
                    expected: "a non-negative integer",
                });
            }
            None => {
                return Err(ComparisonDocError::MissingField {
                    field: "iteration_no",
                });
            }
        };
        let reference_digest = Digest::parse(string_field(document, "reference_digest")?)
            .map_err(|source| value_invalid("reference_digest", source))?;
        let observation_digest = Digest::parse(string_field(document, "observation_digest")?)
            .map_err(|source| value_invalid("observation_digest", source))?;
        let envelope_digest = Digest::parse(string_field(document, "envelope_digest")?)
            .map_err(|source| value_invalid("envelope_digest", source))?;
        let contract_version = non_empty_string(document, "comparison_contract_version")?;
        let identity = ComparatorIdentity::from_json(document.get("comparator_identity").ok_or(
            ComparisonDocError::MissingField {
                field: "comparator_identity",
            },
        )?)?;
        let residuals = match document.get("residuals") {
            Some(JsonValue::Array(items)) => items
                .iter()
                .map(ResidualDoc::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => {
                return Err(ComparisonDocError::TypeInvalid {
                    field: "residuals",
                    expected: "an array",
                });
            }
            None => {
                return Err(ComparisonDocError::MissingField { field: "residuals" });
            }
        };
        let hard_gate_results = match document.get("hard_gate_results") {
            Some(JsonValue::Array(items)) => items
                .iter()
                .map(HardGateResult::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => {
                return Err(ComparisonDocError::TypeInvalid {
                    field: "hard_gate_results",
                    expected: "an array",
                });
            }
            None => {
                return Err(ComparisonDocError::MissingField {
                    field: "hard_gate_results",
                });
            }
        };
        let verdict = RecordVerdict::from_json(
            document
                .get("verdict")
                .ok_or(ComparisonDocError::MissingField { field: "verdict" })?,
        )?;
        Ok(RecordContent {
            run_id,
            iteration_no,
            reference_digest,
            observation_digest,
            envelope_digest,
            contract_version,
            identity,
            residuals,
            hard_gate_results,
            verdict,
        })
    }

    fn fields(&self) -> JsonValue {
        JsonValue::Object(vec![
            (
                "run_id".to_owned(),
                JsonValue::Str(self.content.run_id.as_str().to_owned()),
            ),
            (
                "iteration_no".to_owned(),
                JsonValue::Number(JsonNumber::Uint(u64::from(self.content.iteration_no.get()))),
            ),
            (
                "reference_digest".to_owned(),
                JsonValue::Str(self.content.reference_digest.to_string()),
            ),
            (
                "observation_digest".to_owned(),
                JsonValue::Str(self.content.observation_digest.to_string()),
            ),
            (
                "envelope_digest".to_owned(),
                JsonValue::Str(self.content.envelope_digest.to_string()),
            ),
            (
                "comparison_contract_version".to_owned(),
                JsonValue::Str(self.content.contract_version.clone()),
            ),
            (
                "comparator_identity".to_owned(),
                self.content.identity.to_json(),
            ),
            (
                "residuals".to_owned(),
                JsonValue::Array(
                    self.content
                        .residuals
                        .iter()
                        .map(ResidualDoc::to_json)
                        .collect(),
                ),
            ),
            (
                "hard_gate_results".to_owned(),
                JsonValue::Array(
                    self.content
                        .hard_gate_results
                        .iter()
                        .map(HardGateResult::to_json)
                        .collect(),
                ),
            ),
            ("verdict".to_owned(), self.content.verdict.to_json()),
        ])
    }

    fn object_with_schema(&self) -> JsonValue {
        let JsonValue::Object(entries) = self.fields() else {
            // `fields` always builds an object; the arm exists for the
            // compiler, not for callers.
            return self.fields();
        };
        let mut with_schema = Vec::with_capacity(entries.len() + 1);
        with_schema.push((
            "schema".to_owned(),
            JsonValue::Str(COMPARISON_RECORD_SCHEMA.to_owned()),
        ));
        with_schema.extend(entries);
        JsonValue::Object(with_schema)
    }
}

fn reject_unknown(object: &JsonValue, known: &[&str]) -> Result<(), ComparisonDocError> {
    for key in object.keys() {
        if !known.contains(&key.as_str()) {
            return Err(ComparisonDocError::UnknownField { field: key.clone() });
        }
    }
    Ok(())
}

fn string_field<'a>(
    object: &'a JsonValue,
    field: &'static str,
) -> Result<&'a str, ComparisonDocError> {
    let leaf = field.rsplit('.').next().unwrap_or(field);
    match object.get(leaf) {
        Some(JsonValue::Str(s)) => Ok(s),
        Some(_) => Err(ComparisonDocError::TypeInvalid {
            field,
            expected: "a string",
        }),
        None => Err(ComparisonDocError::MissingField { field }),
    }
}

fn non_empty_string(object: &JsonValue, field: &'static str) -> Result<String, ComparisonDocError> {
    let value = string_field(object, field)?;
    if value.is_empty() {
        return Err(ComparisonDocError::ValueInvalid {
            field,
            source: "the field must not be empty".to_owned(),
        });
    }
    Ok(value.to_owned())
}

fn string_array(
    object: &JsonValue,
    field: &'static str,
) -> Result<Vec<String>, ComparisonDocError> {
    let leaf = field.rsplit('.').next().unwrap_or(field);
    match object.get(leaf) {
        Some(JsonValue::Array(items)) => items
            .iter()
            .map(|item| match item {
                JsonValue::Str(s) => Ok(s.clone()),
                _ => Err(ComparisonDocError::TypeInvalid {
                    field,
                    expected: "an array of strings",
                }),
            })
            .collect(),
        Some(_) => Err(ComparisonDocError::TypeInvalid {
            field,
            expected: "an array of strings",
        }),
        None => Err(ComparisonDocError::MissingField { field }),
    }
}

fn value_invalid(field: &'static str, source: gauntlet_domain::ValueError) -> ComparisonDocError {
    ComparisonDocError::ValueInvalid {
        field,
        source: source.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{
        COMPARISON_RECORD_SCHEMA, ComparatorIdentity, ComparisonDocError, ComparisonRecordDoc,
        HardGateResult, RecordContent, RecordVerdict, ResidualDoc,
    };
    use gauntlet_domain::{
        Digest, EvidenceRef, IterationNo, ResidualId, RunId, Severity, UnitId, Verdict,
    };

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    fn digest(hex: &str) -> Digest {
        Digest::new("sha256", hex).expect("valid digest parts")
    }

    fn sample() -> ComparisonRecordDoc {
        ComparisonRecordDoc::new(RecordContent {
            run_id: RunId::new(ULID).expect("valid run id"),
            iteration_no: IterationNo::new(2).expect("iterations are 1-based"),
            reference_digest: digest(&"11".repeat(32)),
            observation_digest: digest(&"22".repeat(32)),
            envelope_digest: digest(&"33".repeat(32)),
            contract_version: "1".to_owned(),
            identity: ComparatorIdentity::new(
                "a-deterministic-runner".to_owned(),
                String::new(),
                "functional behavior".to_owned(),
            ),
            residuals: vec![ResidualDoc::new(
                ResidualId::new("res-000001").expect("valid id"),
                "functional behavior".to_owned(),
                vec![UnitId::new("AK-0014").expect("valid unit id")],
                "the reference behavior".to_owned(),
                "facet:test exit 1".to_owned(),
                vec![EvidenceRef::new("evd-suite-0001").expect("valid evidence ref")],
                Severity::Blocking,
                0.9,
                false,
                "the instrument failed".to_owned(),
                "re-run the test facet".to_owned(),
                vec!["fix the failing path".to_owned()],
            )],
            hard_gate_results: vec![HardGateResult::new(
                UnitId::new("AK-0014").expect("valid unit id"),
                false,
                vec![EvidenceRef::new("evd-suite-0001").expect("valid evidence ref")],
            )],
            verdict: RecordVerdict::new(
                Verdict::Fail,
                "one blocking residual forces fail".to_owned(),
            ),
        })
    }

    #[test]
    fn encode_decode_round_trip_is_lossless() {
        let doc = sample();
        let text = doc.to_json();
        assert!(text.starts_with(&format!("{{\"schema\":\"{COMPARISON_RECORD_SCHEMA}\"")));
        let back = ComparisonRecordDoc::decode(&text).expect("canonical bytes decode");
        assert_eq!(back, doc);
        assert_eq!(back.to_json(), text, "canonical bytes are a fixed function");
        assert_eq!(back.envelope_digest(), &digest(&"33".repeat(32)));
        assert_eq!(back.residuals()[0].merge_key().0, "functional behavior");
    }

    #[test]
    fn the_payload_form_round_trips_without_the_schema_member() {
        let doc = sample();
        let payload = doc.to_payload_json();
        assert!(!payload.contains("schema"));
        let parsed = crate::json::parse(&payload).expect("the payload is valid JSON");
        let back = ComparisonRecordDoc::decode_payload(&parsed).expect("the payload decodes");
        assert_eq!(back, doc);
        // The payload form is rejected by the standalone decoder (no schema
        // token to check) and vice versa.
        assert!(matches!(
            ComparisonRecordDoc::decode(&payload),
            Err(ComparisonDocError::MissingField { field: "schema" })
        ));
        assert!(matches!(
            ComparisonRecordDoc::decode_payload(&crate::json::parse(&doc.to_json()).expect(
                "the document is valid JSON"
            )),
            Err(ComparisonDocError::UnknownField { field, .. }) if field == "schema"
        ));
    }

    #[test]
    fn unknown_fields_are_rejected_at_every_level() {
        let doc = sample();
        let extended = doc
            .to_json()
            .replace("\"verdict\":", "\"sneaky\":\"x\",\"verdict\":");
        assert!(matches!(
            ComparisonRecordDoc::decode(&extended),
            Err(ComparisonDocError::UnknownField { field, .. }) if field == "sneaky"
        ));
        let extended = doc.to_json().replace(
            "\"runner\":\"a-deterministic-runner\"",
            "\"runner\":\"a-deterministic-runner\",\"extra\":1",
        );
        assert!(matches!(
            ComparisonRecordDoc::decode(&extended),
            Err(ComparisonDocError::UnknownField { field, .. }) if field == "extra"
        ));
        let extended = doc.to_json().replace(
            "\"cheapest_discriminating_test\":\"re-run the test facet\"",
            "\"cheapest_discriminating_test\":\"re-run the test facet\",\"note\":\"x\"",
        );
        assert!(matches!(
            ComparisonRecordDoc::decode(&extended),
            Err(ComparisonDocError::UnknownField { field, .. }) if field == "note"
        ));
    }

    #[test]
    fn wrong_schema_tokens_and_invalid_values_are_refused_whole() {
        let doc = sample();
        let wrong = doc
            .to_json()
            .replace(COMPARISON_RECORD_SCHEMA, "comparison_record@v2");
        assert!(matches!(
            ComparisonRecordDoc::decode(&wrong),
            Err(ComparisonDocError::SchemaUnsupported { .. })
        ));
        let bad_severity = doc
            .to_json()
            .replace("\"severity\":\"blocking\"", "\"severity\":\"fatal\"");
        assert!(matches!(
            ComparisonRecordDoc::decode(&bad_severity),
            Err(ComparisonDocError::ValueInvalid {
                field: "residuals.severity",
                ..
            })
        ));
        let bad_verdict = doc
            .to_json()
            .replace("\"status\":\"fail\"", "\"status\":\"maybe\"");
        assert!(matches!(
            ComparisonRecordDoc::decode(&bad_verdict),
            Err(ComparisonDocError::ValueInvalid {
                field: "verdict.status",
                ..
            })
        ));
        let bad_unit = doc.to_json().replace("\"AK-0014\"", "\"not a unit\"");
        assert!(matches!(
            ComparisonRecordDoc::decode(&bad_unit),
            Err(ComparisonDocError::ValueInvalid {
                field: "residuals.unit_refs",
                ..
            })
        ));
        let empty_version = doc.to_json().replace(
            "\"comparison_contract_version\":\"1\"",
            "\"comparison_contract_version\":\"\"",
        );
        assert!(matches!(
            ComparisonRecordDoc::decode(&empty_version),
            Err(ComparisonDocError::ValueInvalid {
                field: "comparison_contract_version",
                ..
            })
        ));
    }

    #[test]
    fn missing_members_are_named() {
        let doc = sample();
        let truncated = doc
            .to_json()
            .replace("\"envelope_digest\":\"sha256:3333333333333333333333333333333333333333333333333333333333333333\",", "");
        assert!(matches!(
            ComparisonRecordDoc::decode(&truncated),
            Err(ComparisonDocError::MissingField {
                field: "envelope_digest"
            })
        ));
    }
}
