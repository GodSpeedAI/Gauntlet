//! Verification evidence store: `definitions/VERIFICATION.toml`.
//!
//! Each `[[record]]` says what was observed about one harness field, by what kind of evidence,
//! and where the observation came from. The file is strict and fail-closed: unknown root or
//! record keys, unknown harnesses, unknown fields, unknown kinds, and malformed dates are all
//! rejected. Tiers (weakest to strongest) are defined in `definitions/CATALOG.md`.
//!
//! This module only parses and validates. It never runs a harness binary.

use crate::subset_toml::{ParseError, Parser, Table, Value};
use std::collections::BTreeSet;
use std::fmt;

/// File name of the evidence store inside the definitions directory.
pub const VERIFICATION_FILE: &str = "VERIFICATION.toml";

/// Harnesses that have evidence records but no definition file, because an adapter crate owns
/// their behavior (`gauntlet-adapter-agent-prime`). Their rows are rendered by the matrix, not
/// by the definition loader.
pub const ADAPTER_OWNED_HARNESSES: &[&str] = &["prime-agent"];

/// Fields a record may name. Every entry is a claim a definition or matrix row can make.
pub const LEGAL_FIELDS: &[&str] = &[
    "executables",
    "launch.args",
    "launch.prompt_mode",
    "launch.model_arg",
    "session.identity",
    "session.resume_argv",
    "capabilities.model_selection",
    "capabilities.structured_events",
    "control.interrupt",
    "lifecycle.source",
];

const RECORD_KEYS: &[&str] = &[
    "harness",
    "field",
    "kind",
    "probe_argv",
    "observed",
    "harness_version",
    "date",
];

/// Evidence tier. Ordered weakest to strongest.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EvidenceKind {
    /// Extracted from the pinned Herdr donor with audited provenance.
    Catalogued,
    /// Exercised by parser, validator, or launch-argv tests with no real binary.
    Fixture,
    /// The installed binary's own `--help` text documents the flag or behavior. Not executed
    /// with a prompt.
    HelpVerified,
    /// A real run exercised the claim. `--version` live-verifies only discovery plus version.
    LiveVerified,
}

impl EvidenceKind {
    /// The literal used in the TOML file and in matrix output.
    pub fn name(self) -> &'static str {
        match self {
            Self::Catalogued => "catalogued",
            Self::Fixture => "fixture",
            Self::HelpVerified => "help-verified",
            Self::LiveVerified => "live-verified",
        }
    }

    fn parse(raw: &str) -> Option<Self> {
        [
            Self::Catalogued,
            Self::Fixture,
            Self::HelpVerified,
            Self::LiveVerified,
        ]
        .into_iter()
        .find(|k| k.name() == raw)
    }
}

/// One validated evidence record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VerificationRecord {
    pub harness: String,
    pub field: String,
    pub kind: EvidenceKind,
    pub probe_argv: Vec<String>,
    pub observed: String,
    pub harness_version: String,
    pub date: String,
}

/// Why a verification file was rejected.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VerificationError {
    Parse(ParseError),
    Invalid { record: usize, reason: String },
}

impl fmt::Display for VerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Parse(e) => write!(f, "{VERIFICATION_FILE}: line {}: {:?}", e.line, e.kind),
            Self::Invalid { record, reason } => {
                write!(f, "{VERIFICATION_FILE}: record {record}: {reason}")
            }
        }
    }
}

/// Parse and validate a verification file. `known_harnesses` is the set of legal harness ids
/// (definition ids plus [`ADAPTER_OWNED_HARNESSES`]).
pub fn parse_verification(
    input: &str,
    known_harnesses: &BTreeSet<String>,
) -> Result<Vec<VerificationRecord>, VerificationError> {
    let parser = Parser::new(input).map_err(VerificationError::Parse)?;
    let root = parser.parse().map_err(VerificationError::Parse)?;

    for key in root.keys() {
        if key != "schema" && key != "record" {
            return Err(invalid(0, format!("unknown root key {key:?}")));
        }
    }
    match root.get("schema") {
        Some(Value::Int(1)) => {}
        _ => return Err(invalid(0, "schema must be 1".to_string())),
    }

    let items = match root.get("record") {
        None => Vec::new(),
        Some(Value::Array(items)) => items.clone(),
        Some(_) => return Err(invalid(0, "record must be an array of tables".to_string())),
    };

    let mut records: Vec<VerificationRecord> = Vec::new();
    for (index, item) in items.iter().enumerate() {
        let number = index + 1;
        let Value::Table(table) = item else {
            return Err(invalid(number, "record is not a table".to_string()));
        };
        let record = parse_record(table, number, known_harnesses)?;
        if records.iter().any(|r| {
            r.harness == record.harness && r.field == record.field && r.kind == record.kind
        }) {
            return Err(invalid(
                number,
                format!(
                    "duplicate {} record for {}.{}",
                    record.kind.name(),
                    record.harness,
                    record.field
                ),
            ));
        }
        records.push(record);
    }
    Ok(records)
}

fn invalid(record: usize, reason: String) -> VerificationError {
    VerificationError::Invalid { record, reason }
}

fn parse_record(
    table: &Table,
    number: usize,
    known_harnesses: &BTreeSet<String>,
) -> Result<VerificationRecord, VerificationError> {
    for key in table.keys() {
        if !RECORD_KEYS.contains(&key.as_str()) {
            return Err(invalid(number, format!("unknown key {key:?}")));
        }
    }
    let text = |key: &str| -> Result<String, VerificationError> {
        match table.get(key) {
            Some(Value::Str(s)) if !s.trim().is_empty() => Ok(s.clone()),
            _ => Err(invalid(number, format!("{key} must be a non-empty string"))),
        }
    };

    let harness = text("harness")?;
    if !known_harnesses.contains(&harness) {
        return Err(invalid(number, format!("unknown harness {harness:?}")));
    }
    let field = text("field")?;
    if !LEGAL_FIELDS.contains(&field.as_str()) {
        return Err(invalid(number, format!("illegal field {field:?}")));
    }
    let kind_raw = text("kind")?;
    let kind = EvidenceKind::parse(&kind_raw)
        .ok_or_else(|| invalid(number, format!("illegal kind {kind_raw:?}")))?;

    let probe_argv = match table.get("probe_argv") {
        Some(Value::Array(items)) => {
            let mut argv = Vec::new();
            for item in items {
                match item {
                    Value::Str(s) if !s.trim().is_empty() => argv.push(s.clone()),
                    _ => {
                        return Err(invalid(
                            number,
                            "probe_argv must hold non-empty strings".to_string(),
                        ));
                    }
                }
            }
            argv
        }
        _ => return Err(invalid(number, "probe_argv must be an array".to_string())),
    };

    let observed = text("observed")?;
    if observed.len() > 200 {
        return Err(invalid(number, "observed exceeds 200 bytes".to_string()));
    }
    let harness_version = text("harness_version")?;
    let date = text("date")?;
    if !is_iso_date(&date) {
        return Err(invalid(number, format!("date {date:?} is not YYYY-MM-DD")));
    }

    if kind != EvidenceKind::Catalogued && probe_argv.is_empty() {
        return Err(invalid(
            number,
            format!("{} requires a non-empty probe_argv", kind.name()),
        ));
    }
    if kind == EvidenceKind::LiveVerified && harness_version == "n/a" {
        return Err(invalid(
            number,
            "live-verified requires a real harness_version".to_string(),
        ));
    }

    Ok(VerificationRecord {
        harness,
        field,
        kind,
        probe_argv,
        observed,
        harness_version,
        date,
    })
}

fn is_iso_date(s: &str) -> bool {
    let b = s.as_bytes();
    b.len() == 10
        && b[4] == b'-'
        && b[7] == b'-'
        && b.iter()
            .enumerate()
            .all(|(i, c)| i == 4 || i == 7 || c.is_ascii_digit())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ids(names: &[&str]) -> BTreeSet<String> {
        names.iter().map(|s| s.to_string()).collect()
    }

    const OK: &str = r#"
schema = 1

[[record]]
harness = "claude"
field = "launch.prompt_mode"
kind = "help-verified"
probe_argv = ["claude", "--help"]
observed = "-p, --print Print response and exit"
harness_version = "2.1.295"
date = "2026-10-09"

[[record]]
harness = "claude"
field = "executables"
kind = "live-verified"
probe_argv = ["claude", "--version"]
observed = "2.1.295 (Claude Code)"
harness_version = "2.1.295"
date = "2026-10-09"
"#;

    const LIVE_ONLY: &str = r#"
schema = 1

[[record]]
harness = "claude"
field = "executables"
kind = "live-verified"
probe_argv = ["claude", "--version"]
observed = "2.1.295 (Claude Code)"
harness_version = "n/a"
date = "2026-10-09"
"#;

    #[test]
    fn accepts_well_formed_records_in_order() {
        let records = parse_verification(OK, &ids(&["claude"])).expect("valid");
        assert_eq!(records.len(), 2);
        assert_eq!(records[0].kind, EvidenceKind::HelpVerified);
        assert_eq!(records[1].kind, EvidenceKind::LiveVerified);
        assert_eq!(records[1].probe_argv, vec!["claude", "--version"]);
    }

    #[test]
    fn empty_file_is_valid_and_has_no_records() {
        let records = parse_verification("schema = 1\n", &ids(&[])).expect("valid");
        assert!(records.is_empty());
    }

    #[test]
    fn rejects_unknown_root_and_record_keys() {
        assert!(parse_verification("schema = 1\nextra = 2\n", &ids(&[])).is_err());
        let bad = OK.replace(
            "date = \"2026-10-09\"",
            "date = \"2026-10-09\"\nnote = \"x\"",
        );
        assert!(parse_verification(&bad, &ids(&["claude"])).is_err());
    }

    #[test]
    fn rejects_unknown_harness_field_and_kind() {
        assert!(parse_verification(OK, &ids(&["codex"])).is_err());
        let bad_field = OK.replace("launch.prompt_mode", "launch.shell");
        assert!(parse_verification(&bad_field, &ids(&["claude"])).is_err());
        let bad_kind = OK.replace("help-verified", "verified");
        assert!(parse_verification(&bad_kind, &ids(&["claude"])).is_err());
    }

    #[test]
    fn live_verified_requires_probe_argv_and_version() {
        let no_argv = OK.replace(
            "probe_argv = [\"claude\", \"--version\"]",
            "probe_argv = []",
        );
        assert!(parse_verification(&no_argv, &ids(&["claude"])).is_err());
        assert!(parse_verification(LIVE_ONLY, &ids(&["claude"])).is_err());
        let ok_live = LIVE_ONLY.replace("n/a", "2.1.295");
        assert!(parse_verification(&ok_live, &ids(&["claude"])).is_ok());
    }

    #[test]
    fn rejects_bad_dates_and_duplicates() {
        let bad_date = OK.replace("2026-10-09", "2026/10/09");
        assert!(parse_verification(&bad_date, &ids(&["claude"])).is_err());
        let dup = format!(
            "{OK}{}",
            &OK[OK.find("[[record]]").unwrap()
                ..OK.find("\n[[record]]\nharness = \"claude\"\nfield = \"executables\"")
                    .unwrap()]
        );
        assert!(parse_verification(&dup, &ids(&["claude"])).is_err());
    }

    #[test]
    fn rejects_wrong_schema_and_unknown_root_array() {
        assert!(parse_verification("schema = 2\n", &ids(&[])).is_err());
        assert!(parse_verification("schema = 1\n[other]\nx = 1\n", &ids(&[])).is_err());
    }
}
