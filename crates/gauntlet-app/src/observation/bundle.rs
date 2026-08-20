//! `observation_bundle@v1` — the typed Annex B.3 mirror (plan Task 9).
//!
//! The wire key order below is the annex's documented order; canonical
//! serialization is fixed construction order over the order-preserving
//! `gauntlet_ports::json` serializer, so the bytes are a function of the
//! values alone. Decoding is STRICT (mirroring `role_result`
//! conventions): unknown keys are mechanically rejected for the document
//! and every nested object, and the claims facet's
//! `excluded_from_verification` must literally be `true` on the wire — a
//! document that claims otherwise does not decode.
//!
//! Two repo-documented narrowings of the annex's open shapes: the facet
//! `summary` object carries at most a `text` string (B.3 shows `{}` with
//! no inner schema), and the timestamp fields are epoch-second integers
//! (B.3 shows empty-string placeholders; the ports' `EpochSeconds` is the
//! repo's timestamp vocabulary).

use core::fmt;

use gauntlet_domain::{Digest, EvidenceRef, IterationNo, RunId};
use gauntlet_ports::json::{JsonError, JsonNumber, JsonValue};
use gauntlet_ports::observer::ObservationFacet;
use gauntlet_ports::tokens::UnknownToken;

/// The document's schema token (spec Annex B.3).
pub const OBSERVATION_BUNDLE_SCHEMA: &str = "observation_bundle@v1";

/// The claims facet's recorded source (B.3 `source: "builder"`).
pub const CLAIMS_SOURCE: &str = "builder";

/// The document's declared fields, in the annex's documented order.
const BUNDLE_FIELDS: &[&str] = &[
    "schema",
    "run_id",
    "iteration_no",
    "observed_at",
    "project_revision",
    "environment_fingerprint",
    "observation_digest",
    "facets",
    "claims_facet",
    "unavailable_evidence",
    "observation_failures",
];

/// The wire facet's declared fields (B.3 `facets` entries).
const FACET_FIELDS: &[&str] = &[
    "kind",
    "instrument",
    "command",
    "exit_status",
    "started_at",
    "ended_at",
    "evidence_ref",
    "summary",
];

/// `observation_bundle@v1` decode failures — typed, with the field locus
/// wherever one exists (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum ObservationDocError {
    /// The document is not valid JSON.
    Json(JsonError),
    /// The `schema` token is not `observation_bundle@v1`.
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
    /// A field has the right shape but an invalid value (unknown kind
    /// token, non-`true` exclusion flag, invalid digest, zero iteration).
    ValueInvalid { field: &'static str, source: String },
}

impl fmt::Display for ObservationDocError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "observation bundle: {err}"),
            Self::SchemaUnsupported { found } => write!(
                f,
                "observation bundle: schema token {found:?} is not {OBSERVATION_BUNDLE_SCHEMA:?}"
            ),
            Self::UnknownField { field } => write!(
                f,
                "observation bundle: {OBSERVATION_BUNDLE_SCHEMA} rejects unknown field {field:?}"
            ),
            Self::MissingField { field } => write!(
                f,
                "observation bundle: {OBSERVATION_BUNDLE_SCHEMA} is missing field {field:?}"
            ),
            Self::TypeInvalid { field, expected } => write!(
                f,
                "observation bundle: {OBSERVATION_BUNDLE_SCHEMA} field {field:?} is not {expected}"
            ),
            Self::ValueInvalid { field, source } => write!(
                f,
                "observation bundle: {OBSERVATION_BUNDLE_SCHEMA} field {field:?}: {source}"
            ),
        }
    }
}

impl std::error::Error for ObservationDocError {}

impl From<JsonError> for ObservationDocError {
    fn from(source: JsonError) -> Self {
        Self::Json(source)
    }
}

impl From<UnknownToken> for ObservationDocError {
    fn from(source: UnknownToken) -> Self {
        Self::ValueInvalid {
            field: "facets.kind",
            source: source.to_string(),
        }
    }
}

/// One instrument facet as the B.3 wire record: the facet-kind token, the
/// instrument's name and version, the exact command, the exit status and
/// timing window (JSON `null` when the producing facet carried none), the
/// content-addressed evidence ref, and the summary object.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FacetRecord {
    kind: String,
    instrument_name: String,
    instrument_version: String,
    command: Option<String>,
    exit_status: Option<i64>,
    started_at: Option<u64>,
    ended_at: Option<u64>,
    evidence_ref: String,
    summary: Option<String>,
}

impl FacetRecord {
    /// Converts a ports facet into its wire record (the named-constructor
    /// form of `From<&ObservationFacet>`). Every field is copied verbatim;
    /// absent port-side optionals become JSON `null`s, never fabricated
    /// values.
    pub fn from_facet(facet: &ObservationFacet) -> Self {
        Self {
            kind: facet.kind().token(),
            instrument_name: facet.instrument().name().as_str().to_owned(),
            instrument_version: facet.instrument().version().to_owned(),
            command: facet.command().map(str::to_owned),
            exit_status: facet.exit_status(),
            started_at: facet.started_at().map(|at| at.get()),
            ended_at: facet.ended_at().map(|at| at.get()),
            evidence_ref: facet.evidence_ref().as_str().to_owned(),
            summary: facet.summary().map(str::to_owned),
        }
    }

    /// The facet-kind wire token (validated against the `FacetKind`
    /// vocabulary on decode).
    pub fn kind(&self) -> &str {
        &self.kind
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

    /// The instrument's recorded exit status, when it produced one.
    pub fn exit_status(&self) -> Option<i64> {
        self.exit_status
    }

    pub fn started_at(&self) -> Option<u64> {
        self.started_at
    }

    pub fn ended_at(&self) -> Option<u64> {
        self.ended_at
    }

    /// The content-addressed evidence ref string.
    pub fn evidence_ref(&self) -> &str {
        &self.evidence_ref
    }

    /// The instrument's own summary, when it produced one.
    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }

    fn to_json(&self) -> JsonValue {
        // Schema-driven: entries are ZIPPED from the declared FACET_FIELDS
        // table, so the wire key order is the schema's order by
        // construction (canonical bytes pinned by the round-trip tests).
        let values: Vec<JsonValue> = vec![
            JsonValue::Str(self.kind.clone()),
            JsonValue::Object(vec![
                (
                    "name".to_owned(),
                    JsonValue::Str(self.instrument_name.clone()),
                ),
                (
                    "version".to_owned(),
                    JsonValue::Str(self.instrument_version.clone()),
                ),
            ]),
            optional_str(self.command.clone()),
            optional_int(self.exit_status),
            optional_uint(self.started_at),
            optional_uint(self.ended_at),
            JsonValue::Str(self.evidence_ref.clone()),
            match &self.summary {
                Some(text) => {
                    JsonValue::Object(vec![("text".to_owned(), JsonValue::Str(text.clone()))])
                }
                None => JsonValue::Object(Vec::new()),
            },
        ];
        JsonValue::Object(
            FACET_FIELDS
                .iter()
                .zip(values)
                .map(|(field, value)| ((*field).to_owned(), value))
                .collect(),
        )
    }

    fn from_json(value: &JsonValue) -> Result<Self, ObservationDocError> {
        let JsonValue::Object(_) = value else {
            return Err(ObservationDocError::TypeInvalid {
                field: "facets",
                expected: "an array of objects",
            });
        };
        reject_unknown(value, FACET_FIELDS)?;
        let kind = string_field(value, "facets.kind")?;
        gauntlet_ports::observer::FacetKind::parse(kind).map_err(ObservationDocError::from)?;
        let instrument = match value.get("instrument") {
            Some(object @ JsonValue::Object(_)) => object,
            Some(_) => {
                return Err(ObservationDocError::TypeInvalid {
                    field: "facets.instrument",
                    expected: "an object",
                });
            }
            None => {
                return Err(ObservationDocError::MissingField {
                    field: "facets.instrument",
                });
            }
        };
        reject_unknown(instrument, &["name", "version"])?;
        let instrument_name = non_empty_string(instrument, "facets.instrument.name")?;
        let instrument_version = non_empty_string(instrument, "facets.instrument.version")?;
        let command = match value.get("command") {
            Some(JsonValue::Str(s)) => Some(s.clone()),
            Some(JsonValue::Null) => None,
            Some(_) => {
                return Err(ObservationDocError::TypeInvalid {
                    field: "facets.command",
                    expected: "a string or null",
                });
            }
            None => {
                return Err(ObservationDocError::MissingField {
                    field: "facets.command",
                });
            }
        };
        let exit_status = optional_int_field(value, "facets.exit_status")?;
        let started_at = optional_uint_field(value, "facets.started_at")?;
        let ended_at = optional_uint_field(value, "facets.ended_at")?;
        let evidence_ref = match value.get("evidence_ref") {
            Some(JsonValue::Str(s)) => {
                EvidenceRef::new(s).map_err(|source| ObservationDocError::ValueInvalid {
                    field: "facets.evidence_ref",
                    source: source.to_string(),
                })?;
                s.clone()
            }
            Some(_) => {
                return Err(ObservationDocError::TypeInvalid {
                    field: "facets.evidence_ref",
                    expected: "a string",
                });
            }
            None => {
                return Err(ObservationDocError::MissingField {
                    field: "facets.evidence_ref",
                });
            }
        };
        let summary = match value.get("summary") {
            Some(JsonValue::Object(entries)) => {
                let mut text: Option<String> = None;
                for (key, entry) in entries {
                    if key == "text" {
                        let JsonValue::Str(s) = entry else {
                            return Err(ObservationDocError::TypeInvalid {
                                field: "facets.summary.text",
                                expected: "a string",
                            });
                        };
                        text = Some(s.clone());
                    } else {
                        return Err(ObservationDocError::UnknownField { field: key.clone() });
                    }
                }
                text
            }
            Some(_) => {
                return Err(ObservationDocError::TypeInvalid {
                    field: "facets.summary",
                    expected: "an object",
                });
            }
            None => {
                return Err(ObservationDocError::MissingField {
                    field: "facets.summary",
                });
            }
        };
        Ok(Self {
            kind: kind.to_owned(),
            instrument_name,
            instrument_version,
            command,
            exit_status,
            started_at,
            ended_at,
            evidence_ref,
            summary,
        })
    }
}

/// The builder-authored claims facet (spec §12.3, Annex B.3): stored for
/// forensics, excluded from verification inputs BY CONSTRUCTION — there is
/// no conversion from this type into [`FacetRecord`], and
/// `VerificationInputs` is built from the instrument-facet array only
/// (`input` module). `excluded_from_verification` is a constant `true`
/// with no setter; on the wire, a document that spells it `false` fails
/// to decode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ClaimsFacet {
    content: String,
}

impl ClaimsFacet {
    /// Wraps the builder claims content. The source and exclusion flag are
    /// constants of the type.
    pub fn new(content: String) -> Self {
        Self { content }
    }

    /// Renders claim strings as the canonical JSON array text (pinned by
    /// test): `["claim one","claim two"]`, escaping per the canonical
    /// serializer. This is the single rendering rule for claims content.
    pub fn render_content(claims: &[String]) -> String {
        JsonValue::Array(claims.iter().map(|c| JsonValue::Str(c.clone())).collect()).to_json_text()
    }

    /// The claims content, forensic only — never verification input (I2).
    pub fn content(&self) -> &str {
        &self.content
    }

    /// The recorded source; always [`CLAIMS_SOURCE`].
    pub fn source(&self) -> &'static str {
        CLAIMS_SOURCE
    }

    /// Always `true`; there is no setter.
    pub fn excluded_from_verification(&self) -> bool {
        true
    }

    fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            (
                "source".to_owned(),
                JsonValue::Str(CLAIMS_SOURCE.to_owned()),
            ),
            (
                "excluded_from_verification".to_owned(),
                JsonValue::Bool(true),
            ),
            ("content".to_owned(), JsonValue::Str(self.content.clone())),
        ])
    }

    fn from_json(value: &JsonValue) -> Result<Self, ObservationDocError> {
        let JsonValue::Object(_) = value else {
            return Err(ObservationDocError::TypeInvalid {
                field: "claims_facet",
                expected: "an object",
            });
        };
        reject_unknown(value, &["source", "excluded_from_verification", "content"])?;
        let source = string_field(value, "claims_facet.source")?;
        if source != CLAIMS_SOURCE {
            return Err(ObservationDocError::ValueInvalid {
                field: "claims_facet.source",
                source: format!(
                    "the claims facet's source is {source:?}; only {CLAIMS_SOURCE:?} is defined"
                ),
            });
        }
        match value.get("excluded_from_verification") {
            Some(JsonValue::Bool(true)) => {}
            Some(JsonValue::Bool(false)) => {
                return Err(ObservationDocError::ValueInvalid {
                    field: "claims_facet.excluded_from_verification",
                    source: "a claims facet that claims inclusion does not decode — exclusion \
                             is structural, not a flag someone sets"
                        .to_owned(),
                });
            }
            Some(_) => {
                return Err(ObservationDocError::TypeInvalid {
                    field: "claims_facet.excluded_from_verification",
                    expected: "the boolean true",
                });
            }
            None => {
                return Err(ObservationDocError::MissingField {
                    field: "claims_facet.excluded_from_verification",
                });
            }
        }
        let content = string_field(value, "claims_facet.content")?.to_owned();
        Ok(Self { content })
    }
}

/// One `unavailable_evidence` entry (B.3 shape `{kind, reason}`): the
/// facet-kind token of the probe whose evidence could not be produced,
/// and why. Recorded, never dropped (I8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnavailableEvidenceRecord {
    kind: String,
    reason: String,
}

impl UnavailableEvidenceRecord {
    pub fn new(kind: String, reason: String) -> Self {
        Self { kind, reason }
    }

    /// The facet-kind token of the missing probe.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    pub fn reason(&self) -> &str {
        &self.reason
    }

    fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("kind".to_owned(), JsonValue::Str(self.kind.clone())),
            ("reason".to_owned(), JsonValue::Str(self.reason.clone())),
        ])
    }

    fn from_json(value: &JsonValue) -> Result<Self, ObservationDocError> {
        let JsonValue::Object(_) = value else {
            return Err(ObservationDocError::TypeInvalid {
                field: "unavailable_evidence",
                expected: "an array of objects",
            });
        };
        reject_unknown(value, &["kind", "reason"])?;
        let kind = non_empty_string(value, "unavailable_evidence.kind")?;
        gauntlet_ports::observer::FacetKind::parse(&kind).map_err(ObservationDocError::from)?;
        let reason = string_field(value, "unavailable_evidence.reason")?.to_owned();
        Ok(Self { kind, reason })
    }
}

/// One `observation_failures` entry (B.3 shape `{kind, error_class,
/// detail}`): a probe that ran and failed, classified. Recorded, never
/// dropped (I8).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationFailureRecord {
    kind: String,
    error_class: String,
    detail: String,
}

impl ObservationFailureRecord {
    pub fn new(kind: String, error_class: String, detail: String) -> Self {
        Self {
            kind,
            error_class,
            detail,
        }
    }

    /// The facet-kind token of the failed probe.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// The failure classification (`instrument_failed` | `unavailable`
    /// for observer errors; `provenance_mismatch` for the assembler's
    /// cross-check).
    pub fn error_class(&self) -> &str {
        &self.error_class
    }

    pub fn detail(&self) -> &str {
        &self.detail
    }

    fn to_json(&self) -> JsonValue {
        JsonValue::Object(vec![
            ("kind".to_owned(), JsonValue::Str(self.kind.clone())),
            (
                "error_class".to_owned(),
                JsonValue::Str(self.error_class.clone()),
            ),
            ("detail".to_owned(), JsonValue::Str(self.detail.clone())),
        ])
    }

    fn from_json(value: &JsonValue) -> Result<Self, ObservationDocError> {
        let JsonValue::Object(_) = value else {
            return Err(ObservationDocError::TypeInvalid {
                field: "observation_failures",
                expected: "an array of objects",
            });
        };
        reject_unknown(value, &["kind", "error_class", "detail"])?;
        let kind = non_empty_string(value, "observation_failures.kind")?;
        gauntlet_ports::observer::FacetKind::parse(&kind).map_err(ObservationDocError::from)?;
        let error_class = non_empty_string(value, "observation_failures.error_class")?;
        let detail = string_field(value, "observation_failures.detail")?.to_owned();
        Ok(Self {
            kind,
            error_class,
            detail,
        })
    }
}

/// Everything a bundle document carries besides its schema token, as a
/// component bag (construction and rebuild-with-modification both go
/// through it).
#[derive(Debug, Clone, PartialEq)]
pub struct BundleContent {
    pub run_id: RunId,
    pub iteration_no: IterationNo,
    pub observed_at: u64,
    pub project_revision: String,
    pub environment_fingerprint: String,
    pub observation_digest: Digest,
    pub facets: Vec<FacetRecord>,
    pub claims_facet: ClaimsFacet,
    pub unavailable_evidence: Vec<UnavailableEvidenceRecord>,
    pub observation_failures: Vec<ObservationFailureRecord>,
}

/// The typed `observation_bundle@v1` document (spec Annex B.3).
#[derive(Debug, Clone, PartialEq)]
pub struct ObservationBundleDoc {
    content: BundleContent,
}

impl ObservationBundleDoc {
    /// Adopts the content bag; the schema token is a constant of the type.
    pub fn new(content: BundleContent) -> Self {
        Self { content }
    }

    /// The content bag, for access or rebuild-with-modification.
    pub fn content(&self) -> BundleContent {
        self.content.clone()
    }

    pub fn run_id(&self) -> &RunId {
        &self.content.run_id
    }

    pub fn iteration_no(&self) -> IterationNo {
        self.content.iteration_no
    }

    /// When the bundle was assembled (epoch seconds).
    pub fn observed_at(&self) -> u64 {
        self.content.observed_at
    }

    /// The revision the observation was taken against (caller-supplied at
    /// assembly; a plan Task 12 wiring point).
    pub fn project_revision(&self) -> &str {
        &self.content.project_revision
    }

    /// The bundle-level environment fingerprint (B.3).
    pub fn environment_fingerprint(&self) -> &str {
        &self.content.environment_fingerprint
    }

    pub fn observation_digest(&self) -> &Digest {
        &self.content.observation_digest
    }

    /// The instrument facets, in observation order. The claims facet is
    /// NOT in this array (structural exclusion).
    pub fn facets(&self) -> &[FacetRecord] {
        &self.content.facets
    }

    pub fn claims_facet(&self) -> &ClaimsFacet {
        &self.content.claims_facet
    }

    pub fn unavailable_evidence(&self) -> &[UnavailableEvidenceRecord] {
        &self.content.unavailable_evidence
    }

    pub fn observation_failures(&self) -> &[ObservationFailureRecord] {
        &self.content.observation_failures
    }

    /// Canonical serialization: keys in the annex's documented order.
    pub fn to_json(&self) -> String {
        self.object_entries().to_json_text()
    }

    /// The domain view (spec §5.3.6): digest plus the two gap arrays.
    /// Mapped: an unavailable entry's locator becomes `facet:<kind-token>`;
    /// a failure's instrument becomes the kind token (the wire record
    /// carries no instrument name — recorded limitation), its probe
    /// `facet:<kind-token>`, its failure text `<error_class>: <detail>`.
    pub fn as_domain_view(&self) -> gauntlet_domain::ObservationBundle {
        let unavailable = self
            .content
            .unavailable_evidence
            .iter()
            .map(|record| {
                gauntlet_domain::UnavailableEvidence::new(
                    gauntlet_domain::EvidenceLocator::new(&format!("facet:{}", record.kind))
                        .unwrap_or_else(|_| {
                            gauntlet_domain::EvidenceLocator::new("facet").expect(
                                "the fallback locator is well formed when a kind token is not",
                            )
                        }),
                    record.reason.clone(),
                )
            })
            .collect();
        let failures = self
            .content
            .observation_failures
            .iter()
            .map(|record| {
                let instrument =
                    gauntlet_domain::InstrumentName::new(&record.kind).unwrap_or_else(|_| {
                        gauntlet_domain::InstrumentName::new("observer").expect(
                            "the fallback instrument name is well formed when a kind token is not",
                        )
                    });
                gauntlet_domain::ObservationFailure::new(
                    instrument,
                    format!("facet:{}", record.kind),
                    format!("{}: {}", record.error_class, record.detail),
                )
            })
            .collect();
        gauntlet_domain::ObservationBundle::new(
            self.content.observation_digest.clone(),
            unavailable,
            failures,
        )
    }

    /// Decodes and strictly validates an `observation_bundle@v1`
    /// document.
    pub fn decode(text: &str) -> Result<Self, ObservationDocError> {
        let document = gauntlet_ports::json::parse(text)?;
        let JsonValue::Object(_) = &document else {
            return Err(ObservationDocError::TypeInvalid {
                field: "<root>",
                expected: "an object",
            });
        };
        reject_unknown(&document, BUNDLE_FIELDS)?;
        let schema = string_field(&document, "schema")?;
        if schema != OBSERVATION_BUNDLE_SCHEMA {
            return Err(ObservationDocError::SchemaUnsupported {
                found: schema.to_owned(),
            });
        }
        let run_id = match document.get("run_id") {
            Some(JsonValue::Str(s)) => {
                RunId::new(s).map_err(|source| ObservationDocError::ValueInvalid {
                    field: "run_id",
                    source: source.to_string(),
                })?
            }
            Some(_) => {
                return Err(ObservationDocError::TypeInvalid {
                    field: "run_id",
                    expected: "a string",
                });
            }
            None => {
                return Err(ObservationDocError::MissingField { field: "run_id" });
            }
        };
        let iteration_no = match document.get("iteration_no") {
            Some(JsonValue::Number(JsonNumber::Uint(n))) => {
                let n = u32::try_from(*n).map_err(|_| ObservationDocError::ValueInvalid {
                    field: "iteration_no",
                    source: format!("iteration {n} exceeds the u32 iteration space"),
                })?;
                IterationNo::new(n).map_err(|source| ObservationDocError::ValueInvalid {
                    field: "iteration_no",
                    source: source.to_string(),
                })?
            }
            Some(_) => {
                return Err(ObservationDocError::TypeInvalid {
                    field: "iteration_no",
                    expected: "a non-negative integer",
                });
            }
            None => {
                return Err(ObservationDocError::MissingField {
                    field: "iteration_no",
                });
            }
        };
        let observed_at = match document.get("observed_at") {
            Some(JsonValue::Number(JsonNumber::Uint(n))) => *n,
            Some(_) => {
                return Err(ObservationDocError::TypeInvalid {
                    field: "observed_at",
                    expected: "a non-negative integer",
                });
            }
            None => {
                return Err(ObservationDocError::MissingField {
                    field: "observed_at",
                });
            }
        };
        let project_revision = non_empty_string(&document, "project_revision")?;
        let environment_fingerprint = non_empty_string(&document, "environment_fingerprint")?;
        let observation_digest = match document.get("observation_digest") {
            Some(JsonValue::Str(s)) => {
                Digest::parse(s).map_err(|source| ObservationDocError::ValueInvalid {
                    field: "observation_digest",
                    source: source.to_string(),
                })?
            }
            Some(_) => {
                return Err(ObservationDocError::TypeInvalid {
                    field: "observation_digest",
                    expected: "a digest string",
                });
            }
            None => {
                return Err(ObservationDocError::MissingField {
                    field: "observation_digest",
                });
            }
        };
        let facets = match document.get("facets") {
            Some(JsonValue::Array(items)) => items
                .iter()
                .map(FacetRecord::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => {
                return Err(ObservationDocError::TypeInvalid {
                    field: "facets",
                    expected: "an array",
                });
            }
            None => {
                return Err(ObservationDocError::MissingField { field: "facets" });
            }
        };
        let claims_facet = ClaimsFacet::from_json(document.get("claims_facet").ok_or(
            ObservationDocError::MissingField {
                field: "claims_facet",
            },
        )?)?;
        let unavailable_evidence = match document.get("unavailable_evidence") {
            Some(JsonValue::Array(items)) => items
                .iter()
                .map(UnavailableEvidenceRecord::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => {
                return Err(ObservationDocError::TypeInvalid {
                    field: "unavailable_evidence",
                    expected: "an array",
                });
            }
            None => {
                return Err(ObservationDocError::MissingField {
                    field: "unavailable_evidence",
                });
            }
        };
        let observation_failures = match document.get("observation_failures") {
            Some(JsonValue::Array(items)) => items
                .iter()
                .map(ObservationFailureRecord::from_json)
                .collect::<Result<Vec<_>, _>>()?,
            Some(_) => {
                return Err(ObservationDocError::TypeInvalid {
                    field: "observation_failures",
                    expected: "an array",
                });
            }
            None => {
                return Err(ObservationDocError::MissingField {
                    field: "observation_failures",
                });
            }
        };
        Ok(Self {
            content: BundleContent {
                run_id,
                iteration_no,
                observed_at,
                project_revision,
                environment_fingerprint,
                observation_digest,
                facets,
                claims_facet,
                unavailable_evidence,
                observation_failures,
            },
        })
    }

    /// The full canonical object (shared with the digest module's
    /// timestamp-free normalization, which skips keys under its own
    /// control — see `digest`).
    pub(super) fn object_entries(&self) -> JsonValue {
        JsonValue::Object(vec![
            (
                "schema".to_owned(),
                JsonValue::Str(OBSERVATION_BUNDLE_SCHEMA.to_owned()),
            ),
            (
                "run_id".to_owned(),
                JsonValue::Str(self.content.run_id.as_str().to_owned()),
            ),
            (
                "iteration_no".to_owned(),
                JsonValue::Number(JsonNumber::Uint(u64::from(self.content.iteration_no.get()))),
            ),
            (
                "observed_at".to_owned(),
                JsonValue::Number(JsonNumber::Uint(self.content.observed_at)),
            ),
            (
                "project_revision".to_owned(),
                JsonValue::Str(self.content.project_revision.clone()),
            ),
            (
                "environment_fingerprint".to_owned(),
                JsonValue::Str(self.content.environment_fingerprint.clone()),
            ),
            (
                "observation_digest".to_owned(),
                JsonValue::Str(self.content.observation_digest.to_string()),
            ),
            (
                "facets".to_owned(),
                JsonValue::Array(
                    self.content
                        .facets
                        .iter()
                        .map(FacetRecord::to_json)
                        .collect(),
                ),
            ),
            (
                "claims_facet".to_owned(),
                self.content.claims_facet.to_json(),
            ),
            (
                "unavailable_evidence".to_owned(),
                JsonValue::Array(
                    self.content
                        .unavailable_evidence
                        .iter()
                        .map(UnavailableEvidenceRecord::to_json)
                        .collect(),
                ),
            ),
            (
                "observation_failures".to_owned(),
                JsonValue::Array(
                    self.content
                        .observation_failures
                        .iter()
                        .map(ObservationFailureRecord::to_json)
                        .collect(),
                ),
            ),
        ])
    }
}

fn optional_str(value: Option<String>) -> JsonValue {
    match value {
        Some(s) => JsonValue::Str(s),
        None => JsonValue::Null,
    }
}

fn optional_int(value: Option<i64>) -> JsonValue {
    match value {
        Some(n) if n >= 0 => JsonValue::Number(JsonNumber::Uint(n.unsigned_abs())),
        Some(n) => JsonValue::Number(JsonNumber::NegInt(n)),
        None => JsonValue::Null,
    }
}

fn optional_uint(value: Option<u64>) -> JsonValue {
    match value {
        Some(n) => JsonValue::Number(JsonNumber::Uint(n)),
        None => JsonValue::Null,
    }
}

fn optional_int_field(
    object: &JsonValue,
    field: &'static str,
) -> Result<Option<i64>, ObservationDocError> {
    match object.get(field.rsplit('.').next().unwrap_or(field)) {
        Some(JsonValue::Number(number)) => {
            number
                .as_i64()
                .map(Some)
                .ok_or(ObservationDocError::TypeInvalid {
                    field,
                    expected: "an integer or null",
                })
        }
        Some(JsonValue::Null) => Ok(None),
        Some(_) => Err(ObservationDocError::TypeInvalid {
            field,
            expected: "an integer or null",
        }),
        None => Err(ObservationDocError::MissingField { field }),
    }
}

fn optional_uint_field(
    object: &JsonValue,
    field: &'static str,
) -> Result<Option<u64>, ObservationDocError> {
    let leaf = field.rsplit('.').next().unwrap_or(field);
    match object.get(leaf) {
        Some(JsonValue::Number(JsonNumber::Uint(n))) => Ok(Some(*n)),
        Some(JsonValue::Number(JsonNumber::NegInt(_))) => Err(ObservationDocError::ValueInvalid {
            field,
            source: "a timestamp is a non-negative epoch-seconds count".to_owned(),
        }),
        Some(JsonValue::Null) => Ok(None),
        Some(_) => Err(ObservationDocError::TypeInvalid {
            field,
            expected: "a non-negative integer or null",
        }),
        None => Err(ObservationDocError::MissingField { field }),
    }
}

fn reject_unknown(object: &JsonValue, known: &[&str]) -> Result<(), ObservationDocError> {
    for key in object.keys() {
        if !known.contains(&key.as_str()) {
            return Err(ObservationDocError::UnknownField { field: key.clone() });
        }
    }
    Ok(())
}

fn string_field<'a>(
    object: &'a JsonValue,
    field: &'static str,
) -> Result<&'a str, ObservationDocError> {
    let leaf = field.rsplit('.').next().unwrap_or(field);
    match object.get(leaf) {
        Some(JsonValue::Str(s)) => Ok(s),
        Some(_) => Err(ObservationDocError::TypeInvalid {
            field,
            expected: "a string",
        }),
        None => Err(ObservationDocError::MissingField { field }),
    }
}

fn non_empty_string(
    object: &JsonValue,
    field: &'static str,
) -> Result<String, ObservationDocError> {
    let value = string_field(object, field)?;
    if value.is_empty() {
        return Err(ObservationDocError::ValueInvalid {
            field,
            source: "the field must not be empty".to_owned(),
        });
    }
    Ok(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::{
        CLAIMS_SOURCE, ClaimsFacet, FacetRecord, OBSERVATION_BUNDLE_SCHEMA, ObservationBundleDoc,
        ObservationDocError,
    };
    use crate::observation::bundle::BundleContent;
    use gauntlet_domain::{Digest, EvidenceRef, InstrumentName, IterationNo, RunId};
    use gauntlet_ports::observer::{FacetKind, InstrumentIdentity, ObservationFacet};

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const BASE_INSTANT: u64 = 1_700_000_000;
    const CANARY_CLAIM: &str = "the artifact passes its own declared self-check";

    fn digest() -> Digest {
        Digest::new("sha256", &"ab".repeat(32)).expect("valid digest parts")
    }

    fn facet(kind: FacetKind, exit: Option<i64>) -> ObservationFacet {
        let instrument = InstrumentIdentity::new(
            InstrumentName::new("suite-probe").expect("valid instrument name"),
            "1.2.3".to_owned(),
        );
        let evidence: EvidenceRef = EvidenceRef::new("evd-suite-0001").expect("valid evidence ref");
        ObservationFacet::new(kind, instrument, evidence)
            .with_invocation(Some("probe".to_owned()), exit)
            .with_window(
                Some(gauntlet_domain::EpochSeconds::new(BASE_INSTANT)),
                Some(gauntlet_domain::EpochSeconds::new(BASE_INSTANT + 3)),
            )
            .with_summary(Some("one probe, observed".to_owned()))
    }

    fn sample_doc() -> ObservationBundleDoc {
        ObservationBundleDoc::new(BundleContent {
            run_id: RunId::new(ULID).expect("valid run id"),
            iteration_no: IterationNo::new(1).expect("iterations are 1-based"),
            observed_at: BASE_INSTANT,
            project_revision: "rev-suite-01".to_owned(),
            environment_fingerprint: "sha256:0123abcd".to_owned(),
            observation_digest: digest(),
            facets: vec![
                FacetRecord::from_facet(&facet(FacetKind::Test, Some(1))),
                FacetRecord::from_facet(&facet(FacetKind::Filesystem, None)),
            ],
            claims_facet: ClaimsFacet::new(CANARY_CLAIM.to_owned()),
            unavailable_evidence: Vec::new(),
            observation_failures: Vec::new(),
        })
    }

    #[test]
    fn encode_decode_round_trip_is_lossless() {
        let doc = sample_doc();
        let text = doc.to_json();
        assert!(text.starts_with(&format!("{{\"schema\":\"{OBSERVATION_BUNDLE_SCHEMA}\"")));
        let back = ObservationBundleDoc::decode(&text).expect("canonical bytes decode");
        assert_eq!(back, doc);
        assert_eq!(back.to_json(), text, "canonical bytes are a fixed function");
        assert_eq!(back.claims_facet().source(), CLAIMS_SOURCE);
        assert!(back.claims_facet().excluded_from_verification());
        assert_eq!(back.facets().len(), 2);
        assert_eq!(back.facets()[0].exit_status(), Some(1));
        assert_eq!(back.facets()[1].command(), Some("probe"));
        // The claims facet is NOT among the facets.
        assert!(back.facets().iter().all(|record| record.kind() != "claims"));
    }

    #[test]
    fn unknown_fields_are_rejected_at_every_level() {
        let doc = sample_doc();
        let extended = doc
            .to_json()
            .replace("\"claims_facet\":", "\"sneaky\":\"x\",\"claims_facet\":");
        assert!(matches!(
            ObservationBundleDoc::decode(&extended),
            Err(ObservationDocError::UnknownField { field, .. }) if field == "sneaky"
        ));
        let extended = doc.to_json().replace(
            "\"evidence_ref\":\"evd-suite-0001\"",
            "\"evidence_ref\":\"evd-suite-0001\",\"extra\":1",
        );
        assert!(matches!(
            ObservationBundleDoc::decode(&extended),
            Err(ObservationDocError::UnknownField { field, .. }) if field == "extra"
        ));
    }

    #[test]
    fn a_claims_facet_claiming_inclusion_does_not_decode() {
        let doc = sample_doc();
        let flipped = doc.to_json().replace(
            "\"excluded_from_verification\":true",
            "\"excluded_from_verification\":false",
        );
        assert!(matches!(
            ObservationBundleDoc::decode(&flipped),
            Err(ObservationDocError::ValueInvalid {
                field: "claims_facet.excluded_from_verification",
                ..
            })
        ));
    }

    #[test]
    fn wrong_schema_tokens_and_unknown_kinds_are_refused_whole() {
        let doc = sample_doc();
        let wrong = doc
            .to_json()
            .replace(OBSERVATION_BUNDLE_SCHEMA, "observation_bundle@v2");
        assert!(matches!(
            ObservationBundleDoc::decode(&wrong),
            Err(ObservationDocError::SchemaUnsupported { .. })
        ));
        let unknown_kind = doc
            .to_json()
            .replace("\"kind\":\"test\"", "\"kind\":\"magic\"");
        assert!(matches!(
            ObservationBundleDoc::decode(&unknown_kind),
            Err(ObservationDocError::ValueInvalid {
                field: "facets.kind",
                ..
            })
        ));
    }

    #[test]
    fn claims_rendering_is_the_pinned_json_array_form() {
        let claims = vec!["claim one".to_owned(), "claim two".to_owned()];
        assert_eq!(
            ClaimsFacet::render_content(&claims),
            "[\"claim one\",\"claim two\"]"
        );
        assert_eq!(ClaimsFacet::render_content(&[]), "[]");
        let facet = ClaimsFacet::new(ClaimsFacet::render_content(&claims));
        assert_eq!(facet.content(), "[\"claim one\",\"claim two\"]");
    }

    #[test]
    fn from_facet_copies_verbatim_and_nulls_absent_optionals() {
        let record = FacetRecord::from_facet(&facet(FacetKind::Versioning, None));
        assert_eq!(record.kind(), "versioning");
        assert_eq!(record.instrument_name(), "suite-probe");
        assert_eq!(record.instrument_version(), "1.2.3");
        assert_eq!(record.exit_status(), None);
        assert!(record.started_at().is_some() && record.ended_at().is_some());
        assert_eq!(record.evidence_ref(), "evd-suite-0001");
        assert_eq!(record.summary(), Some("one probe, observed"));
        // The wire form spells absent optionals as JSON nulls.
        let doc = ObservationBundleDoc::new(BundleContent {
            run_id: RunId::new(ULID).expect("valid run id"),
            iteration_no: IterationNo::new(1).expect("iterations are 1-based"),
            observed_at: BASE_INSTANT,
            project_revision: "rev-suite-01".to_owned(),
            environment_fingerprint: "sha256:0123abcd".to_owned(),
            observation_digest: digest(),
            facets: vec![record],
            claims_facet: ClaimsFacet::new(String::new()),
            unavailable_evidence: Vec::new(),
            observation_failures: Vec::new(),
        });
        assert!(doc.to_json().contains("\"exit_status\":null"));
        let back = ObservationBundleDoc::decode(&doc.to_json()).expect("nulls decode");
        assert_eq!(back.facets()[0].exit_status(), None);
    }

    #[test]
    fn every_facet_kind_token_is_a_valid_locator_and_instrument_name() {
        // Established invariant behind as_domain_view's fallback-free
        // construction: every wire token of the FacetKind vocabulary is a
        // valid EvidenceLocator and InstrumentName, so the domain view can
        // never fail to build from a decoded document.
        for kind in FacetKind::ALL {
            let token = kind.token();
            let locator = gauntlet_domain::EvidenceLocator::new(&format!("facet:{token}"));
            assert!(locator.is_ok(), "{token} must be a valid locator");
            let instrument = gauntlet_domain::InstrumentName::new(&token);
            assert!(
                instrument.is_ok(),
                "{token} must be a valid instrument name"
            );
        }
    }

    #[test]
    fn as_domain_view_maps_gaps_and_the_digest() {
        let mut content = sample_doc().content();
        content
            .unavailable_evidence
            .push(super::UnavailableEvidenceRecord::new(
                "browser".to_owned(),
                "the probe found nothing at this locator".to_owned(),
            ));
        content
            .observation_failures
            .push(super::ObservationFailureRecord::new(
                FacetKind::Test.token().to_owned(),
                "instrument_failed".to_owned(),
                "the instrument exited without a report".to_owned(),
            ));
        let doc = ObservationBundleDoc::new(content);
        let view = doc.as_domain_view();
        assert_eq!(view.observation_digest(), &digest());
        assert!(view.has_gaps());
        assert_eq!(
            view.unavailable_evidence()[0].locator().as_str(),
            "facet:browser"
        );
        assert_eq!(
            view.observation_failures()[0].instrument().as_str(),
            FacetKind::Test.token(),
        );
        assert_eq!(view.observation_failures()[0].probe(), "facet:test");
        assert_eq!(
            view.observation_failures()[0].failure(),
            "instrument_failed: the instrument exited without a report"
        );
    }
}
