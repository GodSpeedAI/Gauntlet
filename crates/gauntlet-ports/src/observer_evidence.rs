//! `facet_evidence@v1` — the raw-output evidence document every Task-9
//! observer writes for its facet (plan Task 9; spec §5.3.6, §12).
//!
//! The spec does not define the CONTENT format of a facet's raw output —
//! Annex B.3 fixes the bundle wire shape and gives each facet an
//! `evidence_ref` pointing at content-addressed bytes, and §12.2 asks each
//! facet to record the environment fingerprint it observed under. This
//! module is the repo convention that fills that gap: every observe adapter
//! (filesystem, versioning, build, test, command) stores its raw output as
//! ONE of these documents, so the per-facet environment fingerprint and the
//! exact wrapped command live in the evidence bytes even though the B.3
//! wire facet has no such field. The bundle-level
//! `environment_fingerprint` (B.3) is computed by the application-layer
//! assembler; the two reconcile by construction — same process environment,
//! recorded once per facet and once for the bundle.
//!
//! Decoding is STRICT, mirroring [`crate::role_result`] conventions:
//! unknown fields are mechanically rejected (every object key must be a
//! declared field), missing fields are rejected, and the identity fields
//! (`instrument_name`, `instrument_version`, `command`,
//! `env_fingerprint`) must be non-empty — an anonymous facet is exactly
//! what the plan's redesign trigger refuses to admit.
//!
//! Captured output follows a lossy-UTF-8 rule: instrument stdout/stderr are
//! bytes on the far side of the process boundary, and this document is a
//! `String`-shaped wire record, so adapters adopt them via lossy conversion
//! and the replacement characters are the honest record of non-UTF-8 bytes.

use core::fmt;

use crate::json::{JsonError, JsonNumber, JsonValue};

/// The document's schema token (repo convention extending Annex B.3, which
/// does not define facet evidence content).
pub const FACET_EVIDENCE_SCHEMA: &str = "facet_evidence@v1";

/// The declared fields, in the canonical (documented) key order: fixed
/// construction order is the canonical-bytes rule for
/// [`JsonValue::to_json_text`], whose serializer preserves written order.
const FIELDS: &[&str] = &[
    "schema",
    "instrument_name",
    "instrument_version",
    "command",
    "env_fingerprint",
    "exit_status",
    "started_at",
    "ended_at",
    "stdout",
    "stderr",
];

/// The fields that must carry a non-empty string — the document's identity:
/// who ran, what ran, and under what environment. An empty value here is an
/// anonymous facet (plan Task 9 redesign trigger refuses to admit one).
const NON_EMPTY_FIELDS: &[&str] = &[
    "instrument_name",
    "instrument_version",
    "command",
    "env_fingerprint",
];

/// `facet_evidence@v1` decode failures — typed, with the field locus
/// wherever one exists (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum FacetEvidenceError {
    /// The document is not valid JSON.
    Json(JsonError),
    /// The `schema` token is not `facet_evidence@v1`.
    SchemaUnsupported { found: String },
    /// The document carries a field outside the schema.
    UnknownField { field: String },
    /// A declared field is absent.
    MissingField { field: &'static str },
    /// A field has the wrong JSON shape.
    TypeInvalid {
        field: &'static str,
        expected: &'static str,
    },
    /// A field has the right shape but an invalid value (empty identity
    /// string, negative timestamp, fractional count).
    ValueInvalid { field: &'static str, detail: String },
}

impl fmt::Display for FacetEvidenceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(err) => write!(f, "facet evidence: {err}"),
            Self::SchemaUnsupported { found } => write!(
                f,
                "facet evidence: schema token {found:?} is not {FACET_EVIDENCE_SCHEMA:?}"
            ),
            Self::UnknownField { field } => write!(
                f,
                "facet evidence: {FACET_EVIDENCE_SCHEMA} rejects unknown field {field:?}"
            ),
            Self::MissingField { field } => write!(
                f,
                "facet evidence: {FACET_EVIDENCE_SCHEMA} is missing field {field:?}"
            ),
            Self::TypeInvalid { field, expected } => write!(
                f,
                "facet evidence: {FACET_EVIDENCE_SCHEMA} field {field:?} is not {expected}"
            ),
            Self::ValueInvalid { field, detail } => {
                write!(
                    f,
                    "facet evidence: {FACET_EVIDENCE_SCHEMA} field {field:?}: {detail}"
                )
            }
        }
    }
}

impl std::error::Error for FacetEvidenceError {}

impl From<JsonError> for FacetEvidenceError {
    fn from(source: JsonError) -> Self {
        Self::Json(source)
    }
}

/// One instrument's raw output as a wire document (plan Task 9): the
/// wrapper IS the instrument (plan redesign trigger), so the document
/// records the wrapper's identity, the exact wrapped command, the
/// environment fingerprint of the observing process, the observed exit
/// status, the timing window, and the captured output.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FacetEvidenceDoc {
    instrument_name: String,
    instrument_version: String,
    command: String,
    env_fingerprint: String,
    exit_status: i64,
    started_at: u64,
    ended_at: u64,
    stdout: String,
    stderr: String,
}

/// The run facts a wrapper instrument records alongside its identity: what
/// it ran, under what environment, with what result and timing, and what it
/// captured. Grouped so the document constructor stays a three-argument
/// call at every adapter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservedRun {
    /// The exact wrapped command line or query string.
    pub command: String,
    /// The environment fingerprint of the observing process
    /// (`sha256:<hex>` by repo convention).
    pub env_fingerprint: String,
    /// The instrument's observed exit status (any integer — a command that
    /// runs and exits nonzero is a successful observation of failing
    /// reality, plan Task 9).
    pub exit_status: i64,
    pub started_at: u64,
    pub ended_at: u64,
    pub stdout: String,
    pub stderr: String,
}

impl FacetEvidenceDoc {
    /// Builds a document from the instrument identity and the observed run.
    /// The identity fields are validated non-empty here as well, so a
    /// document built in memory obeys the same rule a decoded one does.
    pub fn new(
        instrument_name: String,
        instrument_version: String,
        run: ObservedRun,
    ) -> Result<Self, FacetEvidenceError> {
        let doc = Self {
            instrument_name,
            instrument_version,
            command: run.command,
            env_fingerprint: run.env_fingerprint,
            exit_status: run.exit_status,
            started_at: run.started_at,
            ended_at: run.ended_at,
            stdout: run.stdout,
            stderr: run.stderr,
        };
        // The identity emptiness check is table-driven: NON_EMPTY_FIELDS
        // zipped with the typed field refs, so no field key is spelled
        // outside the declared schema table (order-coupled by the
        // empty-identity tests, which name the offending field).
        let identity: [&str; 4] = [
            doc.instrument_name.as_str(),
            doc.instrument_version.as_str(),
            doc.command.as_str(),
            doc.env_fingerprint.as_str(),
        ];
        for (field, value) in NON_EMPTY_FIELDS.iter().zip(identity) {
            if value.is_empty() {
                return Err(FacetEvidenceError::ValueInvalid {
                    field: leaked_field(field),
                    detail: "an identity field must not be empty — an anonymous facet is never \
                             admitted"
                        .to_owned(),
                });
            }
        }
        Ok(doc)
    }

    /// The wrapper instrument's name (the `gauntlet-observe-<x>` family —
    /// each observe adapter names itself; this port spells none of them).
    pub fn instrument_name(&self) -> &str {
        &self.instrument_name
    }

    /// The wrapper instrument's version.
    pub fn instrument_version(&self) -> &str {
        &self.instrument_version
    }

    /// The exact wrapped command line or query string.
    pub fn command(&self) -> &str {
        &self.command
    }

    /// The environment fingerprint the observation ran under
    /// (`sha256:<hex>` by repo convention).
    pub fn env_fingerprint(&self) -> &str {
        &self.env_fingerprint
    }

    /// The instrument's observed exit status (any integer — a command that
    /// runs and exits nonzero is a successful observation of failing
    /// reality, plan Task 9).
    pub fn exit_status(&self) -> i64 {
        self.exit_status
    }

    pub fn started_at(&self) -> u64 {
        self.started_at
    }

    pub fn ended_at(&self) -> u64 {
        self.ended_at
    }

    pub fn stdout(&self) -> &str {
        &self.stdout
    }

    pub fn stderr(&self) -> &str {
        &self.stderr
    }

    /// Serializes canonically: one JSON object, keys in the documented
    /// [`FIELDS`] order. The entries are ZIPPED from that declared table,
    /// so the key order on the wire is the schema's order by construction
    /// (the order-preserving serializer then makes the bytes a function of
    /// the values alone — pinned by the round-trip test).
    pub fn encode(&self) -> String {
        let values: Vec<JsonValue> = vec![
            JsonValue::Str(FACET_EVIDENCE_SCHEMA.to_owned()),
            JsonValue::Str(self.instrument_name.clone()),
            JsonValue::Str(self.instrument_version.clone()),
            JsonValue::Str(self.command.clone()),
            JsonValue::Str(self.env_fingerprint.clone()),
            integer(self.exit_status),
            JsonValue::Number(JsonNumber::Uint(self.started_at)),
            JsonValue::Number(JsonNumber::Uint(self.ended_at)),
            JsonValue::Str(self.stdout.clone()),
            JsonValue::Str(self.stderr.clone()),
        ];
        JsonValue::Object(
            FIELDS
                .iter()
                .zip(values)
                .map(|(field, value)| ((*field).to_owned(), value))
                .collect(),
        )
        .to_json_text()
    }

    /// Decodes and strictly validates a `facet_evidence@v1` document.
    pub fn decode(text: &str) -> Result<Self, FacetEvidenceError> {
        let document = crate::json::parse(text)?;
        let JsonValue::Object(_) = &document else {
            return Err(FacetEvidenceError::TypeInvalid {
                field: "<root>",
                expected: "an object",
            });
        };
        for key in document.keys() {
            if !FIELDS.contains(&key.as_str()) {
                return Err(FacetEvidenceError::UnknownField { field: key });
            }
        }
        let schema = string_field(&document, "schema")?;
        if schema != FACET_EVIDENCE_SCHEMA {
            return Err(FacetEvidenceError::SchemaUnsupported {
                found: schema.to_owned(),
            });
        }
        // The identity fields' keys come from the declared NON_EMPTY_FIELDS
        // table (order-pinned by the empty-identity tests), so the decode
        // path spells no field key outside the schema tables.
        let instrument_name = non_empty_string(&document, NON_EMPTY_FIELDS[0])?;
        let instrument_version = non_empty_string(&document, NON_EMPTY_FIELDS[1])?;
        let command = non_empty_string(&document, NON_EMPTY_FIELDS[2])?;
        let env_fingerprint = non_empty_string(&document, NON_EMPTY_FIELDS[3])?;
        let exit_status = match document.get("exit_status") {
            Some(JsonValue::Number(number)) => {
                number.as_i64().ok_or(FacetEvidenceError::TypeInvalid {
                    field: "exit_status",
                    expected: "an integer",
                })?
            }
            Some(_) => {
                return Err(FacetEvidenceError::TypeInvalid {
                    field: "exit_status",
                    expected: "an integer",
                });
            }
            None => {
                return Err(FacetEvidenceError::MissingField {
                    field: "exit_status",
                });
            }
        };
        let started_at = timestamp(&document, "started_at")?;
        let ended_at = timestamp(&document, "ended_at")?;
        let stdout = string_field(&document, "stdout")?.to_owned();
        let stderr = string_field(&document, "stderr")?.to_owned();
        Ok(Self {
            instrument_name,
            instrument_version,
            command,
            env_fingerprint,
            exit_status,
            started_at,
            ended_at,
            stdout,
            stderr,
        })
    }
}

/// The single non-`&'static` field locus this module can produce (the
/// `new` validator iterates a `&'static` list, so the leak is bounded to
/// those four constants — never external input).
fn leaked_field(field: &str) -> &'static str {
    NON_EMPTY_FIELDS
        .iter()
        .find(|candidate| **candidate == field)
        .copied()
        .unwrap_or("schema")
}

fn integer(value: i64) -> JsonValue {
    if value >= 0 {
        JsonValue::Number(JsonNumber::Uint(value.unsigned_abs()))
    } else {
        JsonValue::Number(JsonNumber::NegInt(value))
    }
}

fn string_field<'a>(
    document: &'a JsonValue,
    field: &'static str,
) -> Result<&'a str, FacetEvidenceError> {
    match document.get(field) {
        Some(JsonValue::Str(s)) => Ok(s),
        Some(_) => Err(FacetEvidenceError::TypeInvalid {
            field,
            expected: "a string",
        }),
        None => Err(FacetEvidenceError::MissingField { field }),
    }
}

fn non_empty_string(
    document: &JsonValue,
    field: &'static str,
) -> Result<String, FacetEvidenceError> {
    let value = string_field(document, field)?;
    if value.is_empty() {
        return Err(FacetEvidenceError::ValueInvalid {
            field,
            detail: "an identity field must not be empty — an anonymous facet is never admitted"
                .to_owned(),
        });
    }
    Ok(value.to_owned())
}

fn timestamp(document: &JsonValue, field: &'static str) -> Result<u64, FacetEvidenceError> {
    match document.get(field) {
        Some(JsonValue::Number(JsonNumber::Uint(n))) => Ok(*n),
        Some(JsonValue::Number(JsonNumber::NegInt(_))) => Err(FacetEvidenceError::ValueInvalid {
            field,
            detail: "a timestamp is a non-negative epoch-seconds count".to_owned(),
        }),
        Some(_) => Err(FacetEvidenceError::TypeInvalid {
            field,
            expected: "a non-negative integer",
        }),
        None => Err(FacetEvidenceError::MissingField { field }),
    }
}

/// The unambiguous rendering of an argv (plan Task 9): the program is
/// rendered bare, then each argument in order — an argument containing
/// whitespace, a single quote, or a double quote is wrapped in single
/// quotes with embedded single quotes escaped POSIX-style (`'\''`), and
/// every other argument is rendered verbatim. Tokens are joined by single
/// spaces, so `("prog", ["a", "b c"])` renders as `prog a 'b c'`. All
/// Task-9 adapters render commands through this one function, so command
/// strings are comparable across facets.
pub fn command_line(program: &str, args: &[String]) -> String {
    let mut out = String::with_capacity(program.len() + args.len() * 8);
    out.push_str(program);
    for arg in args {
        out.push(' ');
        if arg
            .chars()
            .any(|ch| ch.is_whitespace() || ch == '\'' || ch == '"')
        {
            out.push('\'');
            for ch in arg.chars() {
                if ch == '\'' {
                    out.push_str("'\\''");
                } else {
                    out.push(ch);
                }
            }
            out.push('\'');
        } else {
            out.push_str(arg);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{FACET_EVIDENCE_SCHEMA, FacetEvidenceDoc, ObservedRun, command_line};

    fn sample() -> FacetEvidenceDoc {
        FacetEvidenceDoc::new(
            "gauntlet-observe-suite".to_owned(),
            "1.2.3".to_owned(),
            ObservedRun {
                command: "walk(root=ws, caps=fixed)".to_owned(),
                env_fingerprint: "sha256:0123abcd".to_owned(),
                exit_status: 1,
                started_at: 1_700_000_000,
                ended_at: 1_700_000_003,
                stdout: "line-one\nline-two\n".to_owned(),
                stderr: String::new(),
            },
        )
        .expect("sample fields satisfy the identity constraints")
    }

    #[test]
    fn encode_decode_round_trip_is_lossless() {
        let doc = sample();
        let text = doc.encode();
        let back = FacetEvidenceDoc::decode(&text).expect("canonical bytes decode");
        assert_eq!(back, doc);
        // Canonical bytes are a fixed function of the values: re-encoding
        // the decoded document reproduces the exact text.
        assert_eq!(back.encode(), text);
        // The identity fields survive as themselves.
        assert_eq!(back.instrument_name(), doc.instrument_name());
        assert_eq!(back.exit_status(), doc.exit_status());
        assert_eq!(back.stdout(), doc.stdout());
    }

    #[test]
    fn unknown_fields_are_rejected() {
        let text = sample()
            .encode()
            .replace("\"stderr\":\"\"", "\"stderr\":\"\",\"sneaky\":\"surprise\"");
        let err = FacetEvidenceDoc::decode(&text).expect_err("unknown fields never admitted");
        assert!(
            matches!(err, super::FacetEvidenceError::UnknownField { ref field } if field == "sneaky"),
            "got {err:?}"
        );
    }

    #[test]
    fn missing_fields_are_rejected() {
        // The whole key-value pair is removed (renaming would trip the
        // unknown-field sweep first, a different tooth).
        let text = sample()
            .encode()
            .replace(",\"command\":\"walk(root=ws, caps=fixed)\"", "");
        let err = FacetEvidenceDoc::decode(&text).expect_err("a missing field never defaults");
        assert!(
            matches!(
                err,
                super::FacetEvidenceError::MissingField { field }
                    if field == super::NON_EMPTY_FIELDS[2]
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn an_empty_identity_field_is_rejected_on_decode_and_construction() {
        let text = sample().encode().replace(
            "\"instrument_name\":\"gauntlet-observe-suite\"",
            "\"instrument_name\":\"\"",
        );
        let err = FacetEvidenceDoc::decode(&text).expect_err("an anonymous facet never admits");
        assert!(
            matches!(
                err,
                super::FacetEvidenceError::ValueInvalid {
                    field: "instrument_name",
                    ..
                }
            ),
            "got {err:?}"
        );
        let err = FacetEvidenceDoc::new(
            String::new(),
            "1.2.3".to_owned(),
            ObservedRun {
                command: "walk(root=ws)".to_owned(),
                env_fingerprint: "sha256:0123abcd".to_owned(),
                exit_status: 0,
                started_at: 1,
                ended_at: 2,
                stdout: String::new(),
                stderr: String::new(),
            },
        )
        .expect_err("construction validates the same constraint");
        assert!(
            matches!(
                err,
                super::FacetEvidenceError::ValueInvalid {
                    field: "instrument_name",
                    ..
                }
            ),
            "got {err:?}"
        );
    }

    #[test]
    fn a_wrong_schema_token_is_refused_whole() {
        let text = sample()
            .encode()
            .replace(FACET_EVIDENCE_SCHEMA, "facet_evidence@v2");
        assert!(matches!(
            FacetEvidenceDoc::decode(&text),
            Err(super::FacetEvidenceError::SchemaUnsupported { .. })
        ));
    }

    #[test]
    fn wrong_shapes_are_typed_errors() {
        // Fractional exit status.
        let text = sample()
            .encode()
            .replace("\"exit_status\":1", "\"exit_status\":0.5");
        assert!(matches!(
            FacetEvidenceDoc::decode(&text),
            Err(super::FacetEvidenceError::TypeInvalid {
                field: "exit_status",
                ..
            })
        ));
        // Negative timestamp.
        let text = sample()
            .encode()
            .replace("\"started_at\":1700000000", "\"started_at\":-1");
        assert!(matches!(
            FacetEvidenceDoc::decode(&text),
            Err(super::FacetEvidenceError::ValueInvalid {
                field: "started_at",
                ..
            })
        ));
        // stdout not a string.
        let plain = FacetEvidenceDoc::new(
            "gauntlet-observe-suite".to_owned(),
            "1.2.3".to_owned(),
            ObservedRun {
                command: "walk(root=ws, caps=fixed)".to_owned(),
                env_fingerprint: "sha256:0123abcd".to_owned(),
                exit_status: 0,
                started_at: 1_700_000_000,
                ended_at: 1_700_000_003,
                stdout: "captured".to_owned(),
                stderr: String::new(),
            },
        )
        .expect("sample fields satisfy the identity constraints");
        let text = plain
            .encode()
            .replace("\"stdout\":\"captured\"", "\"stdout\":7");
        assert!(matches!(
            FacetEvidenceDoc::decode(&text),
            Err(super::FacetEvidenceError::TypeInvalid {
                field: "stdout",
                ..
            })
        ));
    }

    #[test]
    fn command_line_quoting_is_pinned() {
        let bare = vec!["out".to_owned(), "check.txt".to_owned()];
        assert_eq!(command_line("walk", &bare), "walk out check.txt");
        // Whitespace-bearing argument: single-quoted whole.
        let spaced = vec!["two words".to_owned()];
        assert_eq!(command_line("probe", &spaced), "probe 'two words'");
        // Embedded double quote: quoted, verbatim inside.
        let quoted = vec!["say \"hi\"".to_owned()];
        assert_eq!(command_line("probe", &quoted), "probe 'say \"hi\"'");
        // Embedded single quote: POSIX close-quote-escape-reopen.
        let apostrophe = vec!["it's".to_owned()];
        assert_eq!(command_line("probe", &apostrophe), "probe 'it'\\''s'");
        // Empty args render the bare program.
        assert_eq!(command_line("probe", &[]), "probe");
        // No whitespace, no quotes: verbatim, no quoting introduced.
        let plain = vec!["PASS".to_owned()];
        assert_eq!(command_line("check", &plain), "check PASS");
    }
}
