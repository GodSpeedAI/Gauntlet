//! `verification_record@v1` — the typed Annex B.5 mirror (plan Task 11).
//!
//! The wire key order below is the annex's documented order; canonical
//! serialization is fixed construction order over the order-preserving
//! `gauntlet_ports::json` serializer, so the bytes are a function of the
//! values alone. Decoding is STRICT (mirroring `comparison_record`
//! conventions): unknown keys are mechanically rejected for the document and
//! every nested object, so a settlement-adjacent document can never be
//! partially parsed into state (spec §10.3 rule 4, §21.2).
//!
//! Two fields carry the §14 discipline this record exists to keep:
//!
//! - `instrument.deterministic` is the model-rung flag carrier (spec §14.2:
//!   model-backed verification is legitimate only where nothing
//!   deterministic can decide, "and such a claim SHOULD be re-expressed");
//!   every record a non-deterministic instrument mints carries `false`.
//! - `class` carries the typed §14.3 INCONCLUSIVE class
//!   ([`BindingClass`]); it is `null` for every non-INCONCLUSIVE record.
//!   An INCONCLUSIVE record MAY still carry `null` — an instrument that ran
//!   and could not decide reports its uncertainty without inventing a
//!   binding failure — but a binding failure is NEVER representable as PASS
//!   or FAIL, and a non-INCONCLUSIVE record can never carry a class.
//!
//! Payload form (spec Annex B.1): inside a `role_result@v1` envelope the
//! payload object carries the SAME fields minus the `schema` token (the
//! envelope's `payload_schema` names it) — the convention `builder_result@v1`
//! and `comparison_record@v1` already established.
//!
//! ## Recorded mapping: the §14.3 fourth condition has no spec-named class
//!
//! Spec §14.3 states FOUR validity conditions but names THREE classes
//! (`evidence_unresolvable` / `evidence_stale` / `evidence_producer_mismatch`).
//! The locator-mismatch condition ("the locator matches
//! `evidence_required.locator`") has no class of its own. This codec does not
//! invent a fourth token and does not collapse silently: evidence produced at
//! a different locator than the required one is NOT THE REQUIRED EVIDENCE —
//! the requirement's evidence does not resolve at its locator — and is
//! therefore classed [`BindingClass::EvidenceUnresolvable`]. The decision is
//! recorded here and in the task report for critic adjudication.

use core::fmt;

use gauntlet_domain::{
    EpochSeconds, EvidenceKind, EvidenceLocator, EvidenceRef, IterationNo, Outcome, Producer,
    RunId, UnitId,
};

use crate::json::{JsonError, JsonNumber, JsonValue};

/// The document's schema token (spec Annex B.5).
pub const VERIFICATION_RECORD_SCHEMA: &str = "verification_record@v1";

/// The document's declared fields, in the annex's documented order. Any other
/// key in a decoded document is rejected as unknown.
const RECORD_FIELDS: &[&str] = &[
    "schema",
    "run_id",
    "iteration_no",
    "unit_id",
    "claim",
    "requirement",
    "instrument",
    "evidence_ref",
    "produced_at",
    "outcome",
    "class",
    "detail",
];

/// The `requirement` object's declared fields (B.5), in annex order.
const REQUIREMENT_FIELDS: &[&str] = &["kind", "locator", "producer"];

/// The `instrument` object's declared fields (B.5), in annex order.
const INSTRUMENT_FIELDS: &[&str] = &["name", "version", "deterministic"];

/// `verification_record@v1` decode failures — typed, with the schema locus
/// and the offending field wherever one exists (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum VerificationDocError {
    /// The document is not valid JSON.
    Json(JsonError),
    /// The `schema` token is not `verification_record@v1`.
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
    /// iteration, unknown outcome/kind/producer/class token, invalid ref).
    ValueInvalid { field: &'static str, source: String },
    /// `class` disagrees with `outcome`: a class on a non-INCONCLUSIVE
    /// record, or a binding class claimed alongside a PASS/FAIL outcome
    /// (spec §14.3/§14.4 — a binding failure is never a settled outcome).
    ClassOutcomeMismatch {
        outcome: Outcome,
        class: BindingClass,
    },
}

impl fmt::Display for VerificationDocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "verification record: {err}"),
            Self::SchemaUnsupported { found } => write!(
                f,
                "verification record: schema token {found:?} is not \
                 {VERIFICATION_RECORD_SCHEMA:?}"
            ),
            Self::UnknownField { field } => write!(
                f,
                "verification record: {VERIFICATION_RECORD_SCHEMA} rejects unknown field {field:?}"
            ),
            Self::MissingField { field } => write!(
                f,
                "verification record: {VERIFICATION_RECORD_SCHEMA} is missing field {field:?}"
            ),
            Self::TypeInvalid { field, expected } => write!(
                f,
                "verification record: {VERIFICATION_RECORD_SCHEMA} field {field:?} is not \
                 {expected}"
            ),
            Self::ValueInvalid { field, source } => write!(
                f,
                "verification record: {VERIFICATION_RECORD_SCHEMA} field {field:?}: {source}"
            ),
            Self::ClassOutcomeMismatch { outcome, class } => write!(
                f,
                "verification record: outcome {outcome} cannot carry binding class {class} — \
                 binding failures are INCONCLUSIVE (spec 14.3/14.4)"
            ),
        }
    }
}

impl std::error::Error for VerificationDocError {}

impl From<JsonError> for VerificationDocError {
    fn from(source: JsonError) -> Self {
        Self::Json(source)
    }
}

/// The typed §14.3 INCONCLUSIVE class vocabulary — the three classes the
/// spec names, and only those. No catch-all variant exists, so two distinct
/// §14.3 conditions cannot be collapsed into one class by a lazy match arm
/// without the compiler naming the skipped variant.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingClass {
    /// The evidence ref does not resolve — in the evidence store, as a
    /// durable production record, or at `evidence_required.locator`
    /// (the recorded mapping for the fourth §14.3 condition: evidence at a
    /// different locator is not the required evidence).
    EvidenceUnresolvable,
    /// The evidence was not produced in this run at or after the transform
    /// it certifies (spec §14.3: "no stale evidence").
    EvidenceStale,
    /// The evidence's producing instrument does not match
    /// `evidence_required.producer`.
    EvidenceProducerMismatch,
}

impl BindingClass {
    /// Every class, for exhaustive tests.
    pub const ALL: &'static [Self] = &[
        Self::EvidenceUnresolvable,
        Self::EvidenceStale,
        Self::EvidenceProducerMismatch,
    ];

    /// The §14.3 wire token, exact-match parsed.
    pub fn parse(input: &str) -> Result<Self, VerificationDocError> {
        match input {
            "evidence_unresolvable" => Ok(Self::EvidenceUnresolvable),
            "evidence_stale" => Ok(Self::EvidenceStale),
            "evidence_producer_mismatch" => Ok(Self::EvidenceProducerMismatch),
            other => Err(VerificationDocError::ValueInvalid {
                field: "class",
                source: format!("token {other:?} is outside the 14.3 class vocabulary"),
            }),
        }
    }
}

impl fmt::Display for BindingClass {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let token = match self {
            Self::EvidenceUnresolvable => "evidence_unresolvable",
            Self::EvidenceStale => "evidence_stale",
            Self::EvidenceProducerMismatch => "evidence_producer_mismatch",
        };
        f.write_str(token)
    }
}

/// The record's `requirement` object (Annex B.5): the declared evidence
/// requirement the claim was verified against — the §5.3.2
/// `evidence_required` triple (kind, locator, producer).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequirementDoc {
    kind: EvidenceKind,
    locator: EvidenceLocator,
    producer: Producer,
}

impl RequirementDoc {
    pub fn new(kind: EvidenceKind, locator: EvidenceLocator, producer: Producer) -> Self {
        Self {
            kind,
            locator,
            producer,
        }
    }

    pub fn kind(&self) -> EvidenceKind {
        self.kind
    }

    pub fn locator(&self) -> &EvidenceLocator {
        &self.locator
    }

    pub fn producer(&self) -> Producer {
        self.producer
    }

    fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("kind".to_owned(), JsonValue::Str(self.kind.to_string())),
            (
                "locator".to_owned(),
                JsonValue::Str(self.locator.as_str().to_owned()),
            ),
            (
                "producer".to_owned(),
                JsonValue::Str(self.producer.to_string()),
            ),
        ])
    }

    fn from_json(value: &JsonValue) -> Result<Self, VerificationDocError> {
        let JsonValue::Object(_) = value else {
            return Err(VerificationDocError::TypeInvalid {
                field: "requirement",
                expected: "an object",
            });
        };
        reject_unknown(value, REQUIREMENT_FIELDS)?;
        let kind = EvidenceKind::parse(string_field(value, "requirement.kind")?)
            .map_err(|source| value_invalid("requirement.kind", source.to_string()))?;
        let locator = EvidenceLocator::new(string_field(value, "requirement.locator")?)
            .map_err(|source| value_invalid("requirement.locator", source.to_string()))?;
        let producer = Producer::parse(string_field(value, "requirement.producer")?)
            .map_err(|source| value_invalid("requirement.producer", source.to_string()))?;
        Ok(Self {
            kind,
            locator,
            producer,
        })
    }
}

/// The record's `instrument` object (Annex B.5): the named instrument that
/// decided, its version, and the §14.2 model-rung flag. `deterministic`
/// is `false` exactly for model judgment (and any future non-deterministic
/// rung); it is the flag every model-produced record MUST carry.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerifyingInstrument {
    name: String,
    version: String,
    deterministic: bool,
}

impl VerifyingInstrument {
    pub fn new(name: String, version: String, deterministic: bool) -> Self {
        Self {
            name,
            version,
            deterministic,
        }
    }

    /// The instrument's name (§14.1: the record carries "the instrument
    /// identity").
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn version(&self) -> &str {
        &self.version
    }

    /// The §14.2 flag: false exactly when this record was minted by a
    /// non-deterministic (model) rung.
    pub fn deterministic(&self) -> bool {
        self.deterministic
    }

    fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("name".to_owned(), JsonValue::Str(self.name.clone())),
            ("version".to_owned(), JsonValue::Str(self.version.clone())),
            (
                "deterministic".to_owned(),
                JsonValue::Bool(self.deterministic),
            ),
        ])
    }

    fn from_json(value: &JsonValue) -> Result<Self, VerificationDocError> {
        let JsonValue::Object(_) = value else {
            return Err(VerificationDocError::TypeInvalid {
                field: "instrument",
                expected: "an object",
            });
        };
        reject_unknown(value, INSTRUMENT_FIELDS)?;
        let name = non_empty_string(value, "instrument.name")?;
        let version = string_field(value, "instrument.version")?.to_owned();
        let deterministic = match value.get("deterministic") {
            Some(JsonValue::Bool(found)) => *found,
            Some(_) => {
                return Err(VerificationDocError::TypeInvalid {
                    field: "instrument.deterministic",
                    expected: "a boolean",
                });
            }
            None => {
                return Err(VerificationDocError::MissingField {
                    field: "instrument.deterministic",
                });
            }
        };
        Ok(Self {
            name,
            version,
            deterministic,
        })
    }
}

/// Everything a `verification_record@v1` document carries besides its schema
/// token, as a component bag (construction and rebuild-with-modification
/// both go through it).
#[derive(Debug, Clone, PartialEq)]
pub struct VerificationContent {
    pub run_id: RunId,
    pub iteration_no: IterationNo,
    pub unit_id: UnitId,
    pub claim: String,
    pub requirement: RequirementDoc,
    pub instrument: VerifyingInstrument,
    pub evidence: EvidenceRef,
    /// When the verified evidence was produced (§14.1 provenance; an
    /// epoch-seconds instant).
    pub produced_at: EpochSeconds,
    pub outcome: Outcome,
    /// The typed §14.3 class; `None` for every non-INCONCLUSIVE record and
    /// for an instrument's own could-not-decide uncertainty.
    pub class: Option<BindingClass>,
    pub detail: String,
}

/// The typed `verification_record@v1` document (spec Annex B.5).
#[derive(Debug, Clone, PartialEq)]
pub struct VerificationRecordDoc {
    content: VerificationContent,
}

impl VerificationRecordDoc {
    /// Adopts the content bag; the schema token is a constant of the type.
    /// Refuses a content bag whose class/outcome pairing violates §14.3/14.4
    /// (a class on a PASS or FAIL record is a binding failure wearing a
    /// settled outcome — unrepresentable, not detected later).
    pub fn new(content: VerificationContent) -> Result<Self, VerificationDocError> {
        if let Some(class) = content.class
            && content.outcome != Outcome::Inconclusive
        {
            return Err(VerificationDocError::ClassOutcomeMismatch {
                outcome: content.outcome,
                class,
            });
        }
        Ok(Self { content })
    }

    /// The content bag, for access or rebuild-with-modification.
    pub fn content(&self) -> VerificationContent {
        self.content.clone()
    }

    pub fn run_id(&self) -> &RunId {
        &self.content.run_id
    }

    pub fn iteration_no(&self) -> IterationNo {
        self.content.iteration_no
    }

    pub fn unit_id(&self) -> &UnitId {
        &self.content.unit_id
    }

    pub fn claim(&self) -> &str {
        &self.content.claim
    }

    pub fn requirement(&self) -> &RequirementDoc {
        &self.content.requirement
    }

    pub fn instrument(&self) -> &VerifyingInstrument {
        &self.content.instrument
    }

    pub fn evidence(&self) -> &EvidenceRef {
        &self.content.evidence
    }

    pub fn produced_at(&self) -> EpochSeconds {
        self.content.produced_at
    }

    pub fn outcome(&self) -> Outcome {
        self.content.outcome
    }

    /// The typed §14.3 class, when this record's outcome is a binding
    /// failure.
    pub fn class(&self) -> Option<BindingClass> {
        self.content.class
    }

    pub fn detail(&self) -> &str {
        &self.content.detail
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

    /// Decodes and strictly validates a standalone `verification_record@v1`
    /// document (schema token included).
    pub fn decode(text: &str) -> Result<Self, VerificationDocError> {
        let document = crate::json::parse(text)?;
        let JsonValue::Object(_) = &document else {
            return Err(VerificationDocError::TypeInvalid {
                field: "<root>",
                expected: "an object",
            });
        };
        reject_unknown(&document, RECORD_FIELDS)?;
        let schema = string_field(&document, "schema")?;
        if schema != VERIFICATION_RECORD_SCHEMA {
            return Err(VerificationDocError::SchemaUnsupported {
                found: schema.to_owned(),
            });
        }
        Self::content_from_object(&document)
    }

    /// Decodes the payload form (no `schema` member — the enclosing
    /// envelope's `payload_schema` names it).
    pub fn decode_payload(value: &JsonValue) -> Result<Self, VerificationDocError> {
        let JsonValue::Object(_) = value else {
            return Err(VerificationDocError::TypeInvalid {
                field: "<payload>",
                expected: "an object",
            });
        };
        reject_unknown(value, &RECORD_FIELDS[1..])?;
        Self::content_from_object(value)
    }

    fn content_from_object(document: &JsonValue) -> Result<Self, VerificationDocError> {
        let run_id = RunId::new(string_field(document, "run_id")?)
            .map_err(|source| value_invalid("run_id", source.to_string()))?;
        let iteration_no = match document.get("iteration_no") {
            Some(JsonValue::Number(JsonNumber::Uint(n))) => {
                let n = u32::try_from(*n).map_err(|_| VerificationDocError::ValueInvalid {
                    field: "iteration_no",
                    source: format!("iteration {n} exceeds the u32 iteration space"),
                })?;
                IterationNo::new(n)
                    .map_err(|source| value_invalid("iteration_no", source.to_string()))?
            }
            Some(_) => {
                return Err(VerificationDocError::TypeInvalid {
                    field: "iteration_no",
                    expected: "a non-negative integer",
                });
            }
            None => {
                return Err(VerificationDocError::MissingField {
                    field: "iteration_no",
                });
            }
        };
        let unit_id = UnitId::new(string_field(document, "unit_id")?)
            .map_err(|source| value_invalid("unit_id", source.to_string()))?;
        let claim = string_field(document, "claim")?.to_owned();
        let requirement = RequirementDoc::from_json(document.get("requirement").ok_or(
            VerificationDocError::MissingField {
                field: "requirement",
            },
        )?)?;
        let instrument = VerifyingInstrument::from_json(document.get("instrument").ok_or(
            VerificationDocError::MissingField {
                field: "instrument",
            },
        )?)?;
        let evidence = EvidenceRef::new(string_field(document, "evidence_ref")?)
            .map_err(|source| value_invalid("evidence_ref", source.to_string()))?;
        let produced_at = match document.get("produced_at") {
            Some(JsonValue::Number(JsonNumber::Uint(at))) => EpochSeconds::new(*at),
            Some(_) => {
                return Err(VerificationDocError::TypeInvalid {
                    field: "produced_at",
                    expected: "a non-negative integer (epoch seconds)",
                });
            }
            None => {
                return Err(VerificationDocError::MissingField {
                    field: "produced_at",
                });
            }
        };
        let outcome = Outcome::parse(string_field(document, "outcome")?)
            .map_err(|source| value_invalid("outcome", source.to_string()))?;
        let class = match document.get("class") {
            Some(JsonValue::Null) => None,
            Some(JsonValue::Str(token)) => Some(BindingClass::parse(token)?),
            Some(_) => {
                return Err(VerificationDocError::TypeInvalid {
                    field: "class",
                    expected: "null or a 14.3 class token",
                });
            }
            None => {
                return Err(VerificationDocError::MissingField { field: "class" });
            }
        };
        let detail = string_field(document, "detail")?.to_owned();
        Self::new(VerificationContent {
            run_id,
            iteration_no,
            unit_id,
            claim,
            requirement,
            instrument,
            evidence,
            produced_at,
            outcome,
            class,
            detail,
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
                "unit_id".to_owned(),
                JsonValue::Str(self.content.unit_id.as_str().to_owned()),
            ),
            (
                "claim".to_owned(),
                JsonValue::Str(self.content.claim.clone()),
            ),
            ("requirement".to_owned(), self.content.requirement.to_json()),
            ("instrument".to_owned(), self.content.instrument.to_json()),
            (
                "evidence_ref".to_owned(),
                JsonValue::Str(self.content.evidence.as_str().to_owned()),
            ),
            (
                "produced_at".to_owned(),
                JsonValue::Number(JsonNumber::Uint(self.content.produced_at.get())),
            ),
            (
                "outcome".to_owned(),
                JsonValue::Str(self.content.outcome.to_string()),
            ),
            (
                "class".to_owned(),
                match self.content.class {
                    Some(class) => JsonValue::Str(class.to_string()),
                    None => JsonValue::Null,
                },
            ),
            (
                "detail".to_owned(),
                JsonValue::Str(self.content.detail.clone()),
            ),
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
            JsonValue::Str(VERIFICATION_RECORD_SCHEMA.to_owned()),
        ));
        with_schema.extend(entries);
        JsonValue::Object(with_schema)
    }
}

fn reject_unknown(object: &JsonValue, known: &[&str]) -> Result<(), VerificationDocError> {
    for key in object.keys() {
        if !known.contains(&key.as_str()) {
            return Err(VerificationDocError::UnknownField { field: key.clone() });
        }
    }
    Ok(())
}

fn string_field<'a>(
    object: &'a JsonValue,
    field: &'static str,
) -> Result<&'a str, VerificationDocError> {
    let leaf = field.rsplit('.').next().unwrap_or(field);
    match object.get(leaf) {
        Some(JsonValue::Str(s)) => Ok(s),
        Some(_) => Err(VerificationDocError::TypeInvalid {
            field,
            expected: "a string",
        }),
        None => Err(VerificationDocError::MissingField { field }),
    }
}

fn non_empty_string(
    object: &JsonValue,
    field: &'static str,
) -> Result<String, VerificationDocError> {
    let value = string_field(object, field)?;
    if value.is_empty() {
        return Err(VerificationDocError::ValueInvalid {
            field,
            source: "the field must not be empty".to_owned(),
        });
    }
    Ok(value.to_owned())
}

fn value_invalid(field: &'static str, source: String) -> VerificationDocError {
    VerificationDocError::ValueInvalid { field, source }
}

#[cfg(test)]
mod tests {
    use super::{
        BindingClass, RequirementDoc, VERIFICATION_RECORD_SCHEMA, VerificationContent,
        VerificationDocError, VerificationRecordDoc, VerifyingInstrument,
    };
    use gauntlet_domain::{
        EpochSeconds, EvidenceKind, EvidenceLocator, EvidenceRef, IterationNo, Outcome, Producer,
        RunId, UnitId,
    };

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";

    fn sample(outcome: Outcome, class: Option<BindingClass>) -> VerificationRecordDoc {
        VerificationRecordDoc::new(VerificationContent {
            run_id: RunId::new(ULID).expect("valid run id"),
            iteration_no: IterationNo::new(2).expect("iterations are 1-based"),
            unit_id: UnitId::new("AK-0005").expect("valid unit id"),
            claim: "INCONCLUSIVE never satisfies a settlement requirement".to_owned(),
            requirement: RequirementDoc::new(
                EvidenceKind::TestReport,
                EvidenceLocator::new("locator-suite-verification").expect("valid locator"),
                Producer::Instrument,
            ),
            instrument: VerifyingInstrument::new(
                "gauntlet-verifier-fixture".to_owned(),
                "1".to_owned(),
                true,
            ),
            evidence: EvidenceRef::new("evd-suite-0001").expect("valid evidence ref"),
            produced_at: EpochSeconds::new(1_700_000_042),
            outcome,
            class,
            detail: "the instrument decided".to_owned(),
        })
        .expect("the sample content is well formed")
    }

    #[test]
    fn encode_decode_round_trip_is_lossless() {
        let doc = sample(Outcome::Inconclusive, Some(BindingClass::EvidenceStale));
        let text = doc.to_json();
        assert!(text.starts_with(&format!("{{\"schema\":\"{VERIFICATION_RECORD_SCHEMA}\"")));
        let back = VerificationRecordDoc::decode(&text).expect("canonical bytes decode");
        assert_eq!(back, doc);
        assert_eq!(back.to_json(), text, "canonical bytes are a fixed function");
        assert_eq!(back.class(), Some(BindingClass::EvidenceStale));
        assert_eq!(
            back.instrument().name(),
            "gauntlet-verifier-fixture",
            "the record carries its instrument identity"
        );
    }

    #[test]
    fn the_payload_form_round_trips_without_the_schema_member() {
        let doc = sample(Outcome::Pass, None);
        let payload = doc.to_payload_json();
        assert!(!payload.contains("schema"));
        let parsed = crate::json::parse(&payload).expect("the payload is valid JSON");
        let back = VerificationRecordDoc::decode_payload(&parsed).expect("the payload decodes");
        assert_eq!(back, doc);
        assert!(matches!(
            VerificationRecordDoc::decode(&payload),
            Err(VerificationDocError::MissingField { field: "schema" })
        ));
        assert!(matches!(
            VerificationRecordDoc::decode_payload(&crate::json::parse(&doc.to_json()).expect(
                "the document is valid JSON"
            )),
            Err(VerificationDocError::UnknownField { field, .. }) if field == "schema"
        ));
    }

    #[test]
    fn unknown_fields_are_rejected_at_every_level() {
        let doc = sample(Outcome::Pass, None);
        let extended = doc
            .to_json()
            .replace("\"outcome\":", "\"sneaky\":\"x\",\"outcome\":");
        assert!(matches!(
            VerificationRecordDoc::decode(&extended),
            Err(VerificationDocError::UnknownField { field, .. }) if field == "sneaky"
        ));
        let extended = doc.to_json().replace(
            "\"producer\":\"instrument\"",
            "\"producer\":\"instrument\",\"extra\":1",
        );
        assert!(matches!(
            VerificationRecordDoc::decode(&extended),
            Err(VerificationDocError::UnknownField { field, .. }) if field == "extra"
        ));
        let extended = doc.to_json().replace(
            "\"deterministic\":true",
            "\"deterministic\":true,\"model\":\"x\"",
        );
        assert!(matches!(
            VerificationRecordDoc::decode(&extended),
            Err(VerificationDocError::UnknownField { field, .. }) if field == "model"
        ));
    }

    #[test]
    fn wrong_schema_tokens_and_invalid_values_are_refused_whole() {
        let doc = sample(Outcome::Pass, None);
        let wrong = doc
            .to_json()
            .replace(VERIFICATION_RECORD_SCHEMA, "verification_record@v2");
        assert!(matches!(
            VerificationRecordDoc::decode(&wrong),
            Err(VerificationDocError::SchemaUnsupported { .. })
        ));
        let bad_outcome = doc.to_json().replace("\"PASS\"", "\"MAYBE\"");
        assert!(matches!(
            VerificationRecordDoc::decode(&bad_outcome),
            Err(VerificationDocError::ValueInvalid {
                field: "outcome",
                ..
            })
        ));
        let bad_class = doc
            .to_json()
            .replace("\"class\":null", "\"class\":\"evidence_gone\"");
        assert!(matches!(
            VerificationRecordDoc::decode(&bad_class),
            Err(VerificationDocError::ValueInvalid { field: "class", .. })
        ));
        let bad_kind = doc
            .to_json()
            .replace("\"kind\":\"test_report\"", "\"kind\":\"vibe\"");
        assert!(matches!(
            VerificationRecordDoc::decode(&bad_kind),
            Err(VerificationDocError::ValueInvalid {
                field: "requirement.kind",
                ..
            })
        ));
        let empty_name = doc
            .to_json()
            .replace("\"name\":\"gauntlet-verifier-fixture\"", "\"name\":\"\"");
        assert!(matches!(
            VerificationRecordDoc::decode(&empty_name),
            Err(VerificationDocError::ValueInvalid {
                field: "instrument.name",
                ..
            })
        ));
    }

    #[test]
    fn missing_members_are_named() {
        let doc = sample(Outcome::Pass, None);
        let truncated = doc.to_json().replace("\"produced_at\":1700000042,", "");
        assert!(matches!(
            VerificationRecordDoc::decode(&truncated),
            Err(VerificationDocError::MissingField {
                field: "produced_at"
            })
        ));
    }

    #[test]
    fn a_class_on_a_settled_outcome_is_unrepresentable() {
        let result = VerificationRecordDoc::decode(
            &sample(Outcome::Pass, None)
                .to_json()
                .replace("\"class\":null", "\"class\":\"evidence_stale\""),
        );
        assert!(matches!(
            result,
            Err(VerificationDocError::ClassOutcomeMismatch { .. })
        ));
        // The constructor refuses the same pairing before any bytes exist.
        let mut content = sample(Outcome::Pass, None).content();
        content.class = Some(BindingClass::EvidenceUnresolvable);
        assert!(matches!(
            VerificationRecordDoc::new(content),
            Err(VerificationDocError::ClassOutcomeMismatch { .. })
        ));
    }

    #[test]
    fn every_fourteen_three_class_round_trips_and_none_collapses() {
        for class in BindingClass::ALL {
            let doc = sample(Outcome::Inconclusive, Some(*class));
            let text = doc.to_json();
            assert!(text.contains(&format!("\"class\":\"{class}\"")));
            let back = VerificationRecordDoc::decode(&text).expect("the class decodes");
            assert_eq!(back.class(), Some(*class));
        }
        // An INCONCLUSIVE with no binding class (instrument uncertainty) is
        // representable: class null, outcome INCONCLUSIVE.
        let doc = sample(Outcome::Inconclusive, None);
        let back = VerificationRecordDoc::decode(&doc.to_json()).expect("null class decodes");
        assert_eq!(back.outcome(), Outcome::Inconclusive);
        assert_eq!(back.class(), None);
    }
}
