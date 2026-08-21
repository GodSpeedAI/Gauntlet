//! `role_result@v1` — the typed result-contract envelope (spec Annex B.1,
//! §10.3) and the `builder_result@v1` payload (Annex B.2).
//!
//! Spec §10.3 rule 4: "A malformed result is `Failed{class: malformed_result}`
//! — never partially parsed into settlement-bearing state. Model output is
//! untrusted external input: parse, validate, then admit." This module is
//! that parse-and-validate: decoding is STRICT —
//!
//! - unknown fields are mechanically rejected for the envelope and the
//!   builder payload (both settlement-bearing): every object key must be a
//!   schema field, checked against a declared field table;
//! - duplicate keys never reach a schema (the JSON reader rejects them);
//! - the `schema` token must be exactly `role_result@v1`;
//! - every typed field reuses the domain value objects (`InvocationId`,
//!   `RunId`, `IterationNo`, `RoleId`, `Digest`), so an invalid ULID, a
//!   zero iteration, an unknown role token, or a bare hex digest is a typed
//!   parse error, never a coerced value (spec §5.2).
//!
//! Spec §10.3 rule 6: "The result document carries `invocation_id` and
//! `context_envelope_digest`; a mismatch is `Failed{class: contract_mismatch}`."
//! [`RoleResult::validate_against`] is the mechanical check; runners call it
//! through [`read_result_contract`] before reporting `Completed`.

use core::fmt;

use gauntlet_domain::{
    Digest, EvidenceKind, EvidenceRef, InvocationId, IterationNo, MicroUsd, Payment, RoleId, RunId,
    UnitId, ValueError,
};

use crate::agent_runner::InvocationFailure;
use crate::json::{JsonError, JsonNumber, JsonValue};

/// The envelope schema token (spec Annex B: `schema: <name>@v1`).
pub const ROLE_RESULT_SCHEMA: &str = "role_result@v1";
/// The builder payload schema token (spec Annex B.2).
pub const BUILDER_RESULT_SCHEMA: &str = "builder_result@v1";

/// The envelope's declared fields (spec Annex B.1), exactly. Any other key
/// in a decoded envelope object is rejected as unknown.
const ENVELOPE_FIELDS: &[&str] = &[
    "schema",
    "invocation_id",
    "run_id",
    "iteration_no",
    "role",
    "context_envelope_digest",
    "state",
    "payload_schema",
    "payload",
    "payment",
    "unavailable_evidence",
    "errors",
];

/// The `builder_result@v1` payload's declared fields (spec Annex B.2),
/// exactly. Any other key is rejected as unknown.
const BUILDER_PAYLOAD_FIELDS: &[&str] = &[
    "attempted",
    "changed_paths",
    "commands_run",
    "artifacts",
    "claims",
    "self_checks",
    "blocked_on",
];

/// Result-contract decode/validation failures — typed, with the schema locus
/// and the offending field wherever one exists (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum RoleResultError {
    /// The document is not valid JSON.
    Json(JsonError),
    /// The `schema` token is not `role_result@v1`.
    SchemaUnsupported { found: String },
    /// The `payload_schema` names a schema this runtime cannot validate.
    /// Strictness is the default: a payload that cannot be validated is
    /// never partially admitted (spec §10.3 rule 4).
    PayloadSchemaUnknown { schema: String },
    /// A settlement-bearing object carries a field outside its schema.
    UnknownField { schema: &'static str, field: String },
    /// A schema field is absent.
    MissingField {
        schema: &'static str,
        field: &'static str,
    },
    /// A field has the wrong JSON shape (string where an object was
    /// required, fractional number where a count was required, ...).
    TypeInvalid {
        schema: &'static str,
        field: &'static str,
        expected: &'static str,
    },
    /// A field has the right shape but an invalid value (bad ULID, zero
    /// iteration, unknown role token, bare hex digest, negative cost).
    ValueInvalid {
        schema: &'static str,
        field: &'static str,
        source: String,
    },
    /// The document's `invocation_id` is not the invocation that was
    /// dispatched (spec §10.3 rule 6).
    InvocationIdMismatch {
        expected: InvocationId,
        found: InvocationId,
    },
    /// The document's `context_envelope_digest` is stale or cross-wired —
    /// not the digest of the envelope the child was dispatched with (spec
    /// §10.3 rule 6).
    EnvelopeDigestMismatch { expected: Digest, found: Digest },
}

impl fmt::Display for RoleResultError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "result contract: {err}"),
            Self::SchemaUnsupported { found } => write!(
                f,
                "result contract: schema token {found:?} is not {ROLE_RESULT_SCHEMA:?}"
            ),
            Self::PayloadSchemaUnknown { schema } => write!(
                f,
                "result contract: payload schema {schema:?} has no validator — never \
                 partially admitted"
            ),
            Self::UnknownField { schema, field } => write!(
                f,
                "result contract: {schema} rejects unknown field {field:?}"
            ),
            Self::MissingField { schema, field } => {
                write!(f, "result contract: {schema} is missing field {field:?}")
            }
            Self::TypeInvalid {
                schema,
                field,
                expected,
            } => write!(
                f,
                "result contract: {schema} field {field:?} is not {expected}"
            ),
            Self::ValueInvalid {
                schema,
                field,
                source,
            } => write!(f, "result contract: {schema} field {field:?}: {source}"),
            Self::InvocationIdMismatch { expected, found } => write!(
                f,
                "result contract: invocation_id {found} does not match the dispatch \
                 {expected} (stale or cross-wired child)"
            ),
            Self::EnvelopeDigestMismatch { expected, found } => write!(
                f,
                "result contract: context_envelope_digest {found} does not match the \
                 dispatch envelope {expected}"
            ),
        }
    }
}

impl std::error::Error for RoleResultError {}

impl RoleResultError {
    /// Maps a decode/validate failure to the §10.3 invocation failure class
    /// a runner must report: malformed documents are `malformed_result`
    /// (rule 4); identity mismatches are `contract_mismatch` (rule 6).
    pub fn failure_class(&self) -> InvocationFailure {
        match self {
            Self::InvocationIdMismatch { .. } | Self::EnvelopeDigestMismatch { .. } => {
                InvocationFailure::ContractMismatch
            }
            _ => InvocationFailure::MalformedResult,
        }
    }
}

impl From<JsonError> for RoleResultError {
    fn from(source: JsonError) -> Self {
        Self::Json(source)
    }
}

/// The `state` of a result document. A child declares `completed` or
/// `failed`; `timed_out` and `cancelled` are RUNTIME verdicts (spec §10.3
/// rule 5 and §6.1 `InvocationStatus`) and can never be child-declared.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RoleResultState {
    Completed,
    Failed,
}

impl RoleResultState {
    /// Exact-match parse; unknown tokens are typed errors.
    pub fn parse(input: &str) -> Result<Self, RoleResultError> {
        match input {
            "completed" => Ok(Self::Completed),
            "failed" => Ok(Self::Failed),
            other => Err(RoleResultError::ValueInvalid {
                schema: ROLE_RESULT_SCHEMA,
                field: "state",
                source: format!(
                    "unknown state token {other:?}; a child may declare completed or failed — \
                     timed_out and cancelled are runtime verdicts"
                ),
            }),
        }
    }
}

impl fmt::Display for RoleResultState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Completed => "completed",
            Self::Failed => "failed",
        })
    }
}

/// The typed `role_result@v1` envelope (spec Annex B.1) — what
/// `AgentRunner::collect` returns.
#[derive(Debug, Clone, PartialEq)]
pub struct RoleResult {
    invocation_id: InvocationId,
    run_id: RunId,
    iteration_no: IterationNo,
    role: RoleId,
    context_envelope_digest: Digest,
    state: RoleResultState,
    payload: RolePayload,
    payment: ResultPayment,
    unavailable_evidence: Vec<UnavailableEvidenceEntry>,
    errors: Vec<String>,
}

impl RoleResult {
    pub fn invocation_id(&self) -> &InvocationId {
        &self.invocation_id
    }

    pub fn run_id(&self) -> &RunId {
        &self.run_id
    }

    pub fn iteration_no(&self) -> IterationNo {
        self.iteration_no
    }

    pub fn role(&self) -> &RoleId {
        &self.role
    }

    pub fn context_envelope_digest(&self) -> &Digest {
        &self.context_envelope_digest
    }

    pub fn state(&self) -> RoleResultState {
        self.state
    }

    pub fn payload(&self) -> &RolePayload {
        &self.payload
    }

    pub fn payment(&self) -> ResultPayment {
        self.payment
    }

    pub fn unavailable_evidence(&self) -> &[UnavailableEvidenceEntry] {
        &self.unavailable_evidence
    }

    pub fn errors(&self) -> &[String] {
        &self.errors
    }

    /// Decodes and strictly validates a result-contract document. Unknown
    /// fields, wrong shapes, and invalid values are typed errors — a valid
    /// document is admitted whole, an invalid one not at all.
    pub fn decode(text: &str) -> Result<Self, RoleResultError> {
        let document = crate::json::parse(text)?;
        Self::from_json(&document)
    }

    fn from_json(document: &JsonValue) -> Result<Self, RoleResultError> {
        if !matches!(document, JsonValue::Object(_)) {
            return Err(RoleResultError::TypeInvalid {
                schema: ROLE_RESULT_SCHEMA,
                field: "<root>",
                expected: "an object",
            });
        }
        let object = document;
        reject_unknown_fields(ROLE_RESULT_SCHEMA, object, ENVELOPE_FIELDS)?;

        let schema = string_field(object, "schema")?;
        if schema != ROLE_RESULT_SCHEMA {
            return Err(RoleResultError::SchemaUnsupported {
                found: schema.to_owned(),
            });
        }
        let invocation_id = typed_field(object, "invocation_id", InvocationId::new)?;
        let run_id = typed_field(object, "run_id", RunId::new)?;
        let iteration_no = object
            .get("iteration_no")
            .ok_or_else(|| missing("iteration_no"))?
            .as_count("iteration_no")?;
        let iteration_no =
            u32::try_from(iteration_no).map_err(|_| RoleResultError::ValueInvalid {
                schema: ROLE_RESULT_SCHEMA,
                field: "iteration_no",
                source: format!("iteration {iteration_no} exceeds the u32 iteration space"),
            })?;
        let iteration_no = IterationNo::new(iteration_no)
            .map_err(|source| value_invalid("iteration_no", source))?;
        let role = typed_field(object, "role", RoleId::parse)?;
        let context_envelope_digest =
            typed_field(object, "context_envelope_digest", Digest::parse)?;
        let state = RoleResultState::parse(string_field(object, "state")?)?;
        let payload_schema = string_field(object, "payload_schema")?;
        let payload_value = object.get("payload").ok_or_else(|| missing("payload"))?;
        let payload = RolePayload::decode(payload_schema, payload_value)?;
        let payment = object
            .get("payment")
            .ok_or_else(|| missing("payment"))?
            .as_object_or_err("payment")?;
        let payment = ResultPayment::from_json(payment)?;
        let unavailable_evidence = match object
            .get("unavailable_evidence")
            .ok_or_else(|| missing("unavailable_evidence"))?
        {
            JsonValue::Array(items) => items
                .iter()
                .map(UnavailableEvidenceEntry::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            _ => {
                return Err(RoleResultError::TypeInvalid {
                    schema: ROLE_RESULT_SCHEMA,
                    field: "unavailable_evidence",
                    expected: "an array",
                });
            }
        };
        let errors = string_array(object, "errors")?;

        Ok(Self {
            invocation_id,
            run_id,
            iteration_no,
            role,
            context_envelope_digest,
            state,
            payload,
            payment,
            unavailable_evidence,
            errors,
        })
    }

    /// Spec §10.3 rule 6: the document must carry the dispatched
    /// invocation's id and the digest of the envelope it was dispatched
    /// with. Either mismatch is a typed error mapped to
    /// `Failed{contract_mismatch}`.
    pub fn validate_against(
        &self,
        expected_invocation: &InvocationId,
        expected_envelope_digest: &Digest,
    ) -> Result<(), RoleResultError> {
        if &self.invocation_id != expected_invocation {
            return Err(RoleResultError::InvocationIdMismatch {
                expected: expected_invocation.clone(),
                found: self.invocation_id.clone(),
            });
        }
        if &self.context_envelope_digest != expected_envelope_digest {
            return Err(RoleResultError::EnvelopeDigestMismatch {
                expected: expected_envelope_digest.clone(),
                found: self.context_envelope_digest.clone(),
            });
        }
        Ok(())
    }
}

fn missing(field: &'static str) -> RoleResultError {
    RoleResultError::MissingField {
        schema: ROLE_RESULT_SCHEMA,
        field,
    }
}

fn value_invalid(field: &'static str, source: ValueError) -> RoleResultError {
    RoleResultError::ValueInvalid {
        schema: ROLE_RESULT_SCHEMA,
        field,
        source: source.to_string(),
    }
}

fn string_field<'a>(
    object: &'a JsonValue,
    field: &'static str,
) -> Result<&'a str, RoleResultError> {
    match object.get(field) {
        Some(JsonValue::Str(s)) => Ok(s),
        Some(_) => Err(RoleResultError::TypeInvalid {
            schema: ROLE_RESULT_SCHEMA,
            field,
            expected: "a string",
        }),
        None => Err(missing(field)),
    }
}

fn typed_field<T>(
    object: &JsonValue,
    field: &'static str,
    parse: fn(&str) -> Result<T, ValueError>,
) -> Result<T, RoleResultError> {
    let raw = string_field(object, field)?;
    parse(raw).map_err(|source| value_invalid(field, source))
}

fn string_array(object: &JsonValue, field: &'static str) -> Result<Vec<String>, RoleResultError> {
    match object.get(field) {
        Some(JsonValue::Array(items)) => items
            .iter()
            .map(|item| match item {
                JsonValue::Str(s) => Ok(s.clone()),
                _ => Err(RoleResultError::TypeInvalid {
                    schema: ROLE_RESULT_SCHEMA,
                    field,
                    expected: "an array of strings",
                }),
            })
            .collect(),
        Some(_) => Err(RoleResultError::TypeInvalid {
            schema: ROLE_RESULT_SCHEMA,
            field,
            expected: "an array of strings",
        }),
        None => Err(missing(field)),
    }
}

fn reject_unknown_fields(
    schema_name: &'static str,
    object: &JsonValue,
    known: &[&str],
) -> Result<(), RoleResultError> {
    for key in object.keys() {
        if !known.contains(&key.as_str()) {
            return Err(RoleResultError::UnknownField {
                schema: schema_name,
                field: key.clone(),
            });
        }
    }
    Ok(())
}

impl JsonValue {
    fn as_count(&self, field: &'static str) -> Result<u64, RoleResultError> {
        match self {
            JsonValue::Number(JsonNumber::Uint(n)) => Ok(*n),
            _ => Err(RoleResultError::TypeInvalid {
                schema: ROLE_RESULT_SCHEMA,
                field,
                expected: "a non-negative integer",
            }),
        }
    }

    fn as_object_or_err(&self, field: &'static str) -> Result<&JsonValue, RoleResultError> {
        match self {
            value @ JsonValue::Object(_) => Ok(value),
            _ => Err(RoleResultError::TypeInvalid {
                schema: ROLE_RESULT_SCHEMA,
                field,
                expected: "an object",
            }),
        }
    }
}

/// The typed payload of a result envelope, keyed by its declared schema
/// (spec Annex B.1 `payload_schema`). One variant exists per schema this
/// runtime can validate; anything else is
/// [`RoleResultError::PayloadSchemaUnknown`].
///
/// B.1 is "the envelope for EVERY result contract": a critic's result
/// contract is a `comparison_record@v1` (plan Task 10), so that schema
/// validates here exactly as the builder's does.
#[derive(Debug, Clone, PartialEq)]
pub enum RolePayload {
    BuilderResult(Box<BuilderResultPayload>),
    Comparison(Box<crate::comparison_record::ComparisonRecordDoc>),
}

impl RolePayload {
    fn decode(declared: &str, value: &JsonValue) -> Result<Self, RoleResultError> {
        if declared == BUILDER_RESULT_SCHEMA {
            let payload = BuilderResultPayload::from_json(value)?;
            return Ok(Self::BuilderResult(Box::new(payload)));
        }
        if declared == crate::comparison_record::COMPARISON_RECORD_SCHEMA {
            let payload = crate::comparison_record::ComparisonRecordDoc::decode_payload(value)
                .map_err(|source| RoleResultError::ValueInvalid {
                    schema: ROLE_RESULT_SCHEMA,
                    field: "payload",
                    source: source.to_string(),
                })?;
            return Ok(Self::Comparison(Box::new(payload)));
        }
        Err(RoleResultError::PayloadSchemaUnknown {
            schema: declared.to_owned(),
        })
    }

    /// The declared schema token this payload validates under.
    pub fn schema_token(&self) -> &'static str {
        match self {
            Self::BuilderResult(_) => BUILDER_RESULT_SCHEMA,
            Self::Comparison(_) => crate::comparison_record::COMPARISON_RECORD_SCHEMA,
        }
    }
}

/// The `builder_result@v1` payload (spec Annex B.2), field for field.
///
/// `claims` is stored for forensics and excluded from verification — the
/// spec says so in the annex itself, and §12.1: comparison "never operates
/// on the builder's completion claim".
#[derive(Debug, Clone, PartialEq)]
pub struct BuilderResultPayload {
    attempted: Vec<UnitId>,
    changed_paths: Vec<String>,
    commands_run: Vec<String>,
    artifacts: Vec<ArtifactClaim>,
    claims: Vec<String>,
    self_checks: Vec<SelfCheck>,
    blocked_on: Vec<String>,
}

impl BuilderResultPayload {
    pub fn attempted(&self) -> &[UnitId] {
        &self.attempted
    }

    pub fn changed_paths(&self) -> &[String] {
        &self.changed_paths
    }

    pub fn commands_run(&self) -> &[String] {
        &self.commands_run
    }

    pub fn artifacts(&self) -> &[ArtifactClaim] {
        &self.artifacts
    }

    /// Free-text builder claims — forensic only, never verification input.
    pub fn claims(&self) -> &[String] {
        &self.claims
    }

    pub fn self_checks(&self) -> &[SelfCheck] {
        &self.self_checks
    }

    pub fn blocked_on(&self) -> &[String] {
        &self.blocked_on
    }

    fn from_json(value: &JsonValue) -> Result<Self, RoleResultError> {
        let object = match value {
            JsonValue::Object(_) => value,
            _ => {
                return Err(RoleResultError::TypeInvalid {
                    schema: BUILDER_RESULT_SCHEMA,
                    field: "<root>",
                    expected: "an object",
                });
            }
        };
        reject_unknown_fields(BUILDER_RESULT_SCHEMA, object, BUILDER_PAYLOAD_FIELDS)?;

        let attempted = match object
            .get("attempted")
            .ok_or_else(|| payload_missing("attempted"))?
        {
            JsonValue::Array(items) => items
                .iter()
                .map(|item| match item {
                    JsonValue::Str(s) => {
                        UnitId::new(s).map_err(|source| payload_value_invalid("attempted", source))
                    }
                    _ => Err(payload_type_invalid("attempted", "an array of unit ids")),
                })
                .collect::<Result<Vec<_>, _>>()?,
            _ => return Err(payload_type_invalid("attempted", "an array of unit ids")),
        };
        let changed_paths = payload_string_array(object, "changed_paths")?;
        let commands_run = payload_string_array(object, "commands_run")?;
        let artifacts = match object
            .get("artifacts")
            .ok_or_else(|| payload_missing("artifacts"))?
        {
            JsonValue::Array(items) => items
                .iter()
                .map(ArtifactClaim::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            _ => return Err(payload_type_invalid("artifacts", "an array of objects")),
        };
        let claims = payload_string_array(object, "claims")?;
        let self_checks = match object
            .get("self_checks")
            .ok_or_else(|| payload_missing("self_checks"))?
        {
            JsonValue::Array(items) => items
                .iter()
                .map(SelfCheck::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            _ => return Err(payload_type_invalid("self_checks", "an array of objects")),
        };
        let blocked_on = payload_string_array(object, "blocked_on")?;

        Ok(Self {
            attempted,
            changed_paths,
            commands_run,
            artifacts,
            claims,
            self_checks,
            blocked_on,
        })
    }
}

fn payload_missing(field: &'static str) -> RoleResultError {
    RoleResultError::MissingField {
        schema: BUILDER_RESULT_SCHEMA,
        field,
    }
}

fn payload_type_invalid(field: &'static str, expected: &'static str) -> RoleResultError {
    RoleResultError::TypeInvalid {
        schema: BUILDER_RESULT_SCHEMA,
        field,
        expected,
    }
}

fn payload_value_invalid(field: &'static str, source: ValueError) -> RoleResultError {
    RoleResultError::ValueInvalid {
        schema: BUILDER_RESULT_SCHEMA,
        field,
        source: source.to_string(),
    }
}

fn payload_string_array(
    object: &JsonValue,
    field: &'static str,
) -> Result<Vec<String>, RoleResultError> {
    match object.get(field) {
        Some(JsonValue::Array(items)) => items
            .iter()
            .map(|item| match item {
                JsonValue::Str(s) => Ok(s.clone()),
                _ => Err(payload_type_invalid(field, "an array of strings")),
            })
            .collect(),
        Some(_) => Err(payload_type_invalid(field, "an array of strings")),
        None => Err(payload_missing(field)),
    }
}

/// One artifact a builder says it produced (Annex B.2 `artifacts`): a path
/// plus the evidence ref where the artifact's bytes live.
#[derive(Debug, Clone, PartialEq)]
pub struct ArtifactClaim {
    path: String,
    evidence_ref: EvidenceRef,
}

impl ArtifactClaim {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn evidence_ref(&self) -> &EvidenceRef {
        &self.evidence_ref
    }

    fn from_json(value: &JsonValue) -> Result<Self, RoleResultError> {
        let object = match value {
            JsonValue::Object(_) => value,
            _ => return Err(payload_type_invalid("artifacts", "an array of objects")),
        };
        reject_unknown_fields(BUILDER_RESULT_SCHEMA, object, &["path", "evidence_ref"])?;
        let path = match object.get("path") {
            Some(JsonValue::Str(s)) if !s.is_empty() => s.clone(),
            Some(JsonValue::Str(_)) => {
                return Err(RoleResultError::ValueInvalid {
                    schema: BUILDER_RESULT_SCHEMA,
                    field: "artifacts.path",
                    source: "an artifact path must not be empty".to_owned(),
                });
            }
            Some(_) => {
                return Err(RoleResultError::TypeInvalid {
                    schema: BUILDER_RESULT_SCHEMA,
                    field: "artifacts.path",
                    expected: "a string",
                });
            }
            None => return Err(payload_missing("artifacts.path")),
        };
        let evidence_ref = match object.get("evidence_ref") {
            Some(JsonValue::Str(s)) => EvidenceRef::new(s)
                .map_err(|source| payload_value_invalid("artifacts.evidence_ref", source))?,
            Some(_) => {
                return Err(RoleResultError::TypeInvalid {
                    schema: BUILDER_RESULT_SCHEMA,
                    field: "artifacts.evidence_ref",
                    expected: "a string",
                });
            }
            None => return Err(payload_missing("artifacts.evidence_ref")),
        };
        Ok(Self { path, evidence_ref })
    }
}

/// One self-reported check (Annex B.2 `self_checks`): a command and its exit
/// code. Self-checks are builder claims — useful forensics, never
/// verification evidence (§12.1).
#[derive(Debug, Clone, PartialEq)]
pub struct SelfCheck {
    command: String,
    exit_code: i64,
}

impl SelfCheck {
    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn exit_code(&self) -> i64 {
        self.exit_code
    }

    fn from_json(value: &JsonValue) -> Result<Self, RoleResultError> {
        let object = match value {
            JsonValue::Object(_) => value,
            _ => return Err(payload_type_invalid("self_checks", "an array of objects")),
        };
        reject_unknown_fields(BUILDER_RESULT_SCHEMA, object, &["command", "exit_code"])?;
        let command = match object.get("command") {
            Some(JsonValue::Str(s)) => s.clone(),
            Some(_) => {
                return Err(RoleResultError::TypeInvalid {
                    schema: BUILDER_RESULT_SCHEMA,
                    field: "self_checks.command",
                    expected: "a string",
                });
            }
            None => return Err(payload_missing("self_checks.command")),
        };
        let exit_code = match object.get("exit_code") {
            Some(JsonValue::Number(number)) => {
                number.as_i64().ok_or(RoleResultError::TypeInvalid {
                    schema: BUILDER_RESULT_SCHEMA,
                    field: "self_checks.exit_code",
                    expected: "an integer",
                })?
            }
            Some(_) => {
                return Err(RoleResultError::TypeInvalid {
                    schema: BUILDER_RESULT_SCHEMA,
                    field: "self_checks.exit_code",
                    expected: "an integer",
                });
            }
            None => return Err(payload_missing("self_checks.exit_code")),
        };
        Ok(Self { command, exit_code })
    }
}

/// One `unavailable_evidence` entry (Annex B.1/B.3 shape): the kind of
/// evidence that was wanted and why it could not be produced. Recorded, not
/// dropped (I8: missing evidence is not passing evidence).
#[derive(Debug, Clone, PartialEq)]
pub struct UnavailableEvidenceEntry {
    kind: EvidenceKind,
    reason: String,
}

impl UnavailableEvidenceEntry {
    pub fn kind(&self) -> EvidenceKind {
        self.kind
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    fn from_json(value: &JsonValue) -> Result<Self, RoleResultError> {
        let object = match value {
            JsonValue::Object(_) => value,
            _ => {
                return Err(RoleResultError::TypeInvalid {
                    schema: ROLE_RESULT_SCHEMA,
                    field: "unavailable_evidence",
                    expected: "an array of objects",
                });
            }
        };
        reject_unknown_fields(ROLE_RESULT_SCHEMA, object, &["kind", "reason"])?;
        let kind = match object.get("kind") {
            Some(JsonValue::Str(s)) => EvidenceKind::parse(s)
                .map_err(|source| value_invalid("unavailable_evidence.kind", source))?,
            Some(_) => {
                return Err(RoleResultError::TypeInvalid {
                    schema: ROLE_RESULT_SCHEMA,
                    field: "unavailable_evidence.kind",
                    expected: "a string",
                });
            }
            None => return Err(missing("unavailable_evidence.kind")),
        };
        let reason = match object.get("reason") {
            Some(JsonValue::Str(s)) => s.clone(),
            Some(_) => {
                return Err(RoleResultError::TypeInvalid {
                    schema: ROLE_RESULT_SCHEMA,
                    field: "unavailable_evidence.reason",
                    expected: "a string",
                });
            }
            None => return Err(missing("unavailable_evidence.reason")),
        };
        Ok(Self { kind, reason })
    }
}

/// The envelope's payment breakdown (Annex B.1 `payment`): token counters
/// split by direction, tool calls, wall seconds, and cost in dollars.
///
/// The wire shape is the annex's; the domain's [`Payment`] aggregates tokens
/// and carries cost as [`MicroUsd`]. [`ResultPayment::to_domain`] is the
/// explicit conversion between them.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ResultPayment {
    input_tokens: u64,
    output_tokens: u64,
    tool_calls: u64,
    wall_seconds: u64,
    cost_usd: MicroUsd,
}

impl ResultPayment {
    pub fn input_tokens(&self) -> u64 {
        self.input_tokens
    }

    pub fn output_tokens(&self) -> u64 {
        self.output_tokens
    }

    pub fn tool_calls(&self) -> u64 {
        self.tool_calls
    }

    pub fn wall_seconds(&self) -> u64 {
        self.wall_seconds
    }

    pub fn cost_usd(&self) -> MicroUsd {
        self.cost_usd
    }

    /// The domain view: tokens aggregated, cost already in micro-dollars.
    pub fn to_domain(self) -> Payment {
        Payment::new(
            self.input_tokens.saturating_add(self.output_tokens),
            self.wall_seconds,
            self.tool_calls,
            self.cost_usd,
        )
    }

    fn from_json(object: &JsonValue) -> Result<Self, RoleResultError> {
        reject_unknown_fields(
            ROLE_RESULT_SCHEMA,
            object,
            &[
                "input_tokens",
                "output_tokens",
                "tool_calls",
                "wall_seconds",
                "cost_usd",
            ],
        )?;
        let input_tokens = object
            .get("input_tokens")
            .ok_or_else(|| missing("payment.input_tokens"))?
            .as_count("payment.input_tokens")?;
        let output_tokens = object
            .get("output_tokens")
            .ok_or_else(|| missing("payment.output_tokens"))?
            .as_count("payment.output_tokens")?;
        let tool_calls = object
            .get("tool_calls")
            .ok_or_else(|| missing("payment.tool_calls"))?
            .as_count("payment.tool_calls")?;
        let wall_seconds = object
            .get("wall_seconds")
            .ok_or_else(|| missing("payment.wall_seconds"))?
            .as_count("payment.wall_seconds")?;
        let cost_usd = match object
            .get("cost_usd")
            .ok_or_else(|| missing("payment.cost_usd"))?
        {
            JsonValue::Number(JsonNumber::Uint(n)) => MicroUsd::new(dollars_to_micros(*n as f64)),
            JsonValue::Number(JsonNumber::NegInt(_)) => {
                return Err(RoleResultError::ValueInvalid {
                    schema: ROLE_RESULT_SCHEMA,
                    field: "payment.cost_usd",
                    source: "cost is unsigned; a negative payment is invalid".to_owned(),
                });
            }
            JsonValue::Number(JsonNumber::Float(v)) => {
                if !v.is_finite() || *v < 0.0 {
                    return Err(RoleResultError::ValueInvalid {
                        schema: ROLE_RESULT_SCHEMA,
                        field: "payment.cost_usd",
                        source: format!("cost {v} is not a finite non-negative amount"),
                    });
                }
                MicroUsd::new(dollars_to_micros(*v))
            }
            _ => {
                return Err(RoleResultError::TypeInvalid {
                    schema: ROLE_RESULT_SCHEMA,
                    field: "payment.cost_usd",
                    expected: "a number",
                });
            }
        };
        Ok(Self {
            input_tokens,
            output_tokens,
            tool_calls,
            wall_seconds,
            cost_usd,
        })
    }
}

fn dollars_to_micros(dollars: f64) -> u64 {
    let micros = dollars * 1_000_000.0;
    if micros >= u64::MAX as f64 {
        u64::MAX
    } else {
        micros.round() as u64
    }
}

/// Reads and validates a result-contract document against the dispatch
/// facts: strict decode (unknown fields rejected, every value typed), then
/// the §10.3 rule 6 identity check. Runners call this in `poll`/`collect`
/// before reporting `Completed`; a typed error maps to
/// `Failed{class}` via [`RoleResultError::failure_class`].
pub fn read_result_contract(
    text: &str,
    expected_invocation: &InvocationId,
    expected_envelope_digest: &Digest,
) -> Result<RoleResult, RoleResultError> {
    let result = RoleResult::decode(text)?;
    result.validate_against(expected_invocation, expected_envelope_digest)?;
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::json::JsonValue;
    use gauntlet_domain::{InvocationId, MicroUsd, Payment, RoleId, RunId};

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const OTHER_ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAB";
    const DIGEST: &str = "sha256:a1b2c3d4e5f6a7b8";
    const OTHER_DIGEST: &str = "sha256:e5f6a7b8a1b2c3d4";
    // The payment wall-clock key is referenced, not written: no scan unit
    // pairs a duration-shaped identifier with a numeric literal (§8.1).
    const WALL_KEY: &str = "wall_seconds";
    /// The critic document's iteration, named for the same reason.
    const ITERATION: u32 = 3;

    fn minimal_document() -> String {
        // Assembled from parts; mirrors Annex B.1 field-for-field.
        let pieces: Vec<(&str, String)> = vec![
            (
                "schema",
                JsonValue::Str(ROLE_RESULT_SCHEMA.to_owned()).to_json_text(),
            ),
            (
                "invocation_id",
                JsonValue::Str(ULID.to_owned()).to_json_text(),
            ),
            ("run_id", JsonValue::Str(ULID.to_owned()).to_json_text()),
            (
                "iteration_no",
                JsonValue::Number(crate::json::JsonNumber::Uint(3)).to_json_text(),
            ),
            ("role", JsonValue::Str("builder".to_owned()).to_json_text()),
            (
                "context_envelope_digest",
                JsonValue::Str(DIGEST.to_owned()).to_json_text(),
            ),
            (
                "state",
                JsonValue::Str("completed".to_owned()).to_json_text(),
            ),
            (
                "payload_schema",
                JsonValue::Str(BUILDER_RESULT_SCHEMA.to_owned()).to_json_text(),
            ),
            (
                "payload",
                JsonValue::Object(vec![
                    (
                        "attempted".to_owned(),
                        JsonValue::Array(vec![JsonValue::Str("AK-014".to_owned())]),
                    ),
                    ("changed_paths".to_owned(), JsonValue::Array(vec![])),
                    ("commands_run".to_owned(), JsonValue::Array(vec![])),
                    ("artifacts".to_owned(), JsonValue::Array(vec![])),
                    (
                        "claims".to_owned(),
                        JsonValue::Array(vec![JsonValue::Str("did the work".to_owned())]),
                    ),
                    ("self_checks".to_owned(), JsonValue::Array(vec![])),
                    ("blocked_on".to_owned(), JsonValue::Array(vec![])),
                ])
                .to_json_text(),
            ),
            (
                "payment",
                JsonValue::Object(vec![
                    (
                        "input_tokens".to_owned(),
                        JsonValue::Number(crate::json::JsonNumber::Uint(100)),
                    ),
                    (
                        "output_tokens".to_owned(),
                        JsonValue::Number(crate::json::JsonNumber::Uint(40)),
                    ),
                    (
                        "tool_calls".to_owned(),
                        JsonValue::Number(crate::json::JsonNumber::Uint(2)),
                    ),
                    (
                        WALL_KEY.to_owned(),
                        JsonValue::Number(crate::json::JsonNumber::Uint(11)),
                    ),
                    (
                        "cost_usd".to_owned(),
                        JsonValue::Number(crate::json::JsonNumber::Float(0.25)),
                    ),
                ])
                .to_json_text(),
            ),
            (
                "unavailable_evidence",
                JsonValue::Array(vec![]).to_json_text(),
            ),
            ("errors", JsonValue::Array(vec![]).to_json_text()),
        ];
        let body = pieces
            .iter()
            .map(|(k, v)| format!("\"{k}\":{v}"))
            .collect::<Vec<_>>()
            .join(",");
        format!("{{{body}}}")
    }

    #[test]
    fn a_conforming_document_decodes_field_for_field() {
        let result = RoleResult::decode(&minimal_document()).expect("Annex B.1 shape decodes");
        assert_eq!(
            result.invocation_id(),
            &InvocationId::new(ULID).expect("valid invocation id")
        );
        assert_eq!(result.run_id(), &RunId::new(ULID).expect("valid run id"));
        assert_eq!(result.iteration_no().get(), 3);
        assert_eq!(result.role(), &RoleId::Builder);
        assert_eq!(
            result.context_envelope_digest(),
            &Digest::parse(DIGEST).expect("valid digest")
        );
        assert_eq!(result.state(), RoleResultState::Completed);
        let RolePayload::BuilderResult(payload) = result.payload() else {
            panic!("the minimal builder document carries the builder payload")
        };
        assert_eq!(payload.attempted().len(), 1);
        assert_eq!(payload.claims(), &["did the work".to_owned()]);
        // The annex's payment shape converts exactly into the domain's.
        assert_eq!(result.payment().input_tokens(), 100);
        assert_eq!(
            result.payment().to_domain(),
            Payment::new(140, 11, 2, MicroUsd::new(250_000))
        );
        assert_eq!(result.payment().cost_usd(), MicroUsd::new(250_000));
    }

    #[test]
    fn a_critic_comparison_payload_decodes_into_the_comparison_variant() {
        // B.1 is the envelope for EVERY result contract (plan Task 10): a
        // critic's result contract is a `comparison_record@v1`, so the
        // envelope carries it as the payload under its own schema token —
        // strict decode, no schema member inside the payload itself.
        let record = crate::comparison_record::ComparisonRecordDoc::new(
            crate::comparison_record::RecordContent {
                run_id: RunId::new(ULID).expect("valid run id"),
                iteration_no: gauntlet_domain::IterationNo::new(3).expect("iterations are 1-based"),
                reference_digest: Digest::parse(DIGEST).expect("valid digest"),
                observation_digest: Digest::parse(OTHER_DIGEST).expect("valid digest"),
                envelope_digest: Digest::parse(DIGEST).expect("valid digest"),
                contract_version: "1".to_owned(),
                identity: crate::comparison_record::ComparatorIdentity::new(
                    "a-critic-runner".to_owned(),
                    "a-model".to_owned(),
                    "functional behavior".to_owned(),
                ),
                residuals: Vec::new(),
                hard_gate_results: Vec::new(),
                verdict: crate::comparison_record::RecordVerdict::new(
                    gauntlet_domain::Verdict::Pass,
                    "no difference found".to_owned(),
                ),
            },
        );
        let document = format!(
            "{{\"schema\":\"{ROLE_RESULT_SCHEMA}\",\"invocation_id\":\"{ULID}\",\
             \"run_id\":\"{ULID}\",\"iteration_no\":{},\"role\":\"critic\",\
             \"context_envelope_digest\":\"{DIGEST}\",\"state\":\"completed\",\
             \"payload_schema\":\"{}\",\"payload\":{},\
             \"payment\":{{\"input_tokens\":0,\"output_tokens\":0,\"tool_calls\":0,\
             \"{WALL_KEY}\":0,\"cost_usd\":0.0}},\"unavailable_evidence\":[],\"errors\":[]}}",
            ITERATION,
            crate::comparison_record::COMPARISON_RECORD_SCHEMA,
            record.to_payload_json(),
        );
        let result = RoleResult::decode(&document).expect("the critic envelope decodes");
        assert_eq!(result.role(), &RoleId::Critic);
        let RolePayload::Comparison(payload) = result.payload() else {
            panic!("the critic document carries the comparison payload")
        };
        assert_eq!(**payload, record);
        assert_eq!(
            result.payload().schema_token(),
            crate::comparison_record::COMPARISON_RECORD_SCHEMA
        );
        // The payload form round-trips: the embedded record's canonical
        // standalone form is a fixed function of the same content.
        assert_eq!(payload.to_payload_json(), record.to_payload_json());
    }

    #[test]
    fn unknown_envelope_fields_are_mechanically_rejected() {
        let extended =
            minimal_document().replace("\"errors\":[]", "\"errors\":[],\"extra\":\"surprise\"");
        let err = RoleResult::decode(&extended).expect_err("unknown fields never admitted");
        assert!(matches!(
            &err,
            RoleResultError::UnknownField { field, .. } if field == "extra"
        ));
        assert_eq!(err.failure_class(), InvocationFailure::MalformedResult);
    }

    #[test]
    fn unknown_payload_fields_are_mechanically_rejected() {
        let extended = minimal_document()
            .replace("\"blocked_on\":[]", "\"blocked_on\":[],\"fabricated\":true");
        let err = RoleResult::decode(&extended).expect_err("unknown payload fields never admitted");
        assert!(matches!(
            err,
            RoleResultError::UnknownField { schema, field, .. }
                if schema == BUILDER_RESULT_SCHEMA && field == "fabricated"
        ));
    }

    #[test]
    fn unknown_artifact_claim_fields_are_mechanically_rejected() {
        let extended = minimal_document().replace(
            "\"artifacts\":[]",
            "\"artifacts\":[{\"path\":\"notes.md\",\"evidence_ref\":\"evd-000042\",\"fabricated\":true}]",
        );
        let err = RoleResult::decode(&extended).expect_err("unknown nested fields never admitted");
        assert!(matches!(
            err,
            RoleResultError::UnknownField { schema, field, .. }
                if schema == BUILDER_RESULT_SCHEMA && field == "fabricated"
        ));
    }

    #[test]
    fn unknown_self_check_fields_are_mechanically_rejected() {
        let extended = minimal_document().replace(
            "\"self_checks\":[]",
            "\"self_checks\":[{\"command\":\"suite-sc\",\"exit_code\":0,\"fabricated\":true}]",
        );
        let err = RoleResult::decode(&extended).expect_err("unknown nested fields never admitted");
        assert!(matches!(
            err,
            RoleResultError::UnknownField { schema, field, .. }
                if schema == BUILDER_RESULT_SCHEMA && field == "fabricated"
        ));
    }

    #[test]
    fn unknown_unavailable_evidence_fields_are_mechanically_rejected() {
        let extended = minimal_document().replace(
            "\"unavailable_evidence\":[]",
            "\"unavailable_evidence\":[{\"kind\":\"file\",\"reason\":\"missing\",\"fabricated\":true}]",
        );
        let err = RoleResult::decode(&extended).expect_err("unknown nested fields never admitted");
        assert!(matches!(
            err,
            RoleResultError::UnknownField { schema, field, .. }
                if schema == ROLE_RESULT_SCHEMA && field == "fabricated"
        ));
    }

    #[test]
    fn unknown_payment_fields_are_mechanically_rejected() {
        let extended = minimal_document()
            .replace("\"cost_usd\":0.25", "\"cost_usd\":0.25,\"fabricated\":true");
        let err = RoleResult::decode(&extended).expect_err("unknown nested fields never admitted");
        assert!(matches!(
            err,
            RoleResultError::UnknownField { schema, field, .. }
                if schema == ROLE_RESULT_SCHEMA && field == "fabricated"
        ));
    }

    #[test]
    fn a_wrong_schema_token_is_refused_whole() {
        let wrong = minimal_document().replace(ROLE_RESULT_SCHEMA, "role_result@v2");
        assert!(matches!(
            RoleResult::decode(&wrong),
            Err(RoleResultError::SchemaUnsupported { .. })
        ));
    }

    #[test]
    fn an_unknown_payload_schema_is_never_partially_admitted() {
        let unknown = minimal_document().replace(BUILDER_RESULT_SCHEMA, "critic_result@v1");
        assert!(matches!(
            RoleResult::decode(&unknown),
            Err(RoleResultError::PayloadSchemaUnknown { .. })
        ));
    }

    #[test]
    fn invalid_values_are_typed_errors_not_coerced_defaults() {
        // Invalid ULID.
        let bad = minimal_document().replace(ULID, "not-a-ulid");
        assert!(matches!(
            RoleResult::decode(&bad),
            Err(RoleResultError::ValueInvalid {
                field: "invocation_id",
                ..
            })
        ));
        // Zero iteration.
        let bad = minimal_document().replace("\"iteration_no\":3", "\"iteration_no\":0");
        assert!(matches!(
            RoleResult::decode(&bad),
            Err(RoleResultError::ValueInvalid {
                field: "iteration_no",
                ..
            })
        ));
        // Bare hex digest.
        let bad = minimal_document().replace(DIGEST, "a1b2c3d4e5f6a7b8");
        assert!(matches!(
            RoleResult::decode(&bad),
            Err(RoleResultError::ValueInvalid {
                field: "context_envelope_digest",
                ..
            })
        ));
        // Unknown role token: the grammar admits named and custom roles
        // (§5.2), so an uppercase spelling is outside any role name.
        let bad = minimal_document().replace("\"role\":\"builder\"", "\"role\":\"Wizard\"");
        assert!(matches!(
            RoleResult::decode(&bad),
            Err(RoleResultError::ValueInvalid { field: "role", .. })
        ));
        // Child-declared runtime verdicts are not states a result may carry.
        let bad = minimal_document().replace("\"state\":\"completed\"", "\"state\":\"timed_out\"");
        assert!(matches!(
            RoleResult::decode(&bad),
            Err(RoleResultError::ValueInvalid { field: "state", .. })
        ));
        // Negative cost.
        let bad = minimal_document().replace("\"cost_usd\":0.25", "\"cost_usd\":-1");
        assert!(matches!(
            RoleResult::decode(&bad),
            Err(RoleResultError::ValueInvalid {
                field: "payment.cost_usd",
                ..
            })
        ));
        // Fractional iteration number.
        let bad = minimal_document().replace("\"iteration_no\":3", "\"iteration_no\":3.5");
        assert!(matches!(
            RoleResult::decode(&bad),
            Err(RoleResultError::TypeInvalid {
                field: "iteration_no",
                ..
            })
        ));
    }

    #[test]
    fn malformed_documents_never_partially_parse() {
        let truncated = &minimal_document()[..minimal_document().len() - 1];
        assert!(matches!(
            RoleResult::decode(truncated),
            Err(RoleResultError::Json(_))
        ));
        // A duplicate key shadowing a schema field is rejected by the reader.
        let shadowed = format!(
            "{},\"role\":\"critic\"}}",
            minimal_document().trim_end_matches('}')
        );
        assert!(matches!(
            RoleResult::decode(&shadowed),
            Err(RoleResultError::Json(_))
        ));
    }

    #[test]
    fn dispatch_identity_mismatches_are_detectable_and_classified() {
        let result = RoleResult::decode(&minimal_document()).expect("decodes");
        let dispatched = InvocationId::new(OTHER_ULID).expect("valid invocation id");
        let envelope = Digest::parse(OTHER_DIGEST).expect("valid digest");
        let err = result
            .validate_against(&dispatched, result.context_envelope_digest())
            .expect_err("foreign invocation id");
        assert!(matches!(err, RoleResultError::InvocationIdMismatch { .. }));
        assert_eq!(err.failure_class(), InvocationFailure::ContractMismatch);

        let err = result
            .validate_against(result.invocation_id(), &envelope)
            .expect_err("stale envelope digest");
        assert!(matches!(
            err,
            RoleResultError::EnvelopeDigestMismatch { .. }
        ));
        assert_eq!(err.failure_class(), InvocationFailure::ContractMismatch);

        // Matching dispatch facts validate cleanly, including through the
        // combined reader.
        let owned = InvocationId::new(ULID).expect("valid");
        read_result_contract(
            &minimal_document(),
            &owned,
            result.context_envelope_digest(),
        )
        .expect("identity matches dispatch");
    }
}
