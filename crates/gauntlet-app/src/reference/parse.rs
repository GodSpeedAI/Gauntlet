//! The strict answer-key parser (plan Task 7 step 4) — `.wayfinder/`
//! `answer-key.md` shape: `## AK-NNN` headings, each followed by field
//! lines, blocks separated by blank lines.
//!
//! Untrusted input, admitted under the domain's rules only: every fixed
//! vocabulary field goes through its `wire_enum!` `parse` (an unknown value
//! is a typed error naming the field, the value, and the block — AK-004:
//! never a coerced default), locators through `EvidenceLocator::new`, and
//! the assembled Unit through `Unit::new`, whose tier/authority coupling
//! and producer rule make a structurally invalid block a parse failure
//! (AK-006, AK-007).
//!
//! **Per-block error isolation is the specified behavior**: one malformed
//! block fails that Unit, not the file. [`parse_answer_key`] therefore
//! returns both the well-formed Units and the per-block failures with their
//! loci; [`parse_all_or_fail`] is the all-or-nothing wrapper for callers
//! that need every block or none. Anomalies that belong to no block (a
//! stray line, a heading that opens nothing) are failures carrying no block
//! id — reported, never dropped.

use core::fmt;

use gauntlet_domain::{
    Authority, BlastRadius, EvidenceKind, EvidenceLocator, EvidenceRequirement, Producer, Unit,
    UnitError, UnitId, VerifyBy,
};

/// The answer-key block-id prefix. Unit ids elsewhere may be any §5.2
/// shape; the answer key's own vocabulary is `AK-NNN` (the frozen key is
/// AK-001..AK-070), and a `depends_on` target could never resolve to a
/// block outside it.
const BLOCK_PREFIX: &str = "AK";

/// The field keys a block may carry. `depends_on` is optional; the other
/// five are required. A function, not a `const` table: this is the answer
/// key's typed wire vocabulary (the field `match` in `field_line` is its
/// authority), and the boundary scanner's tier-policy class reserves
/// declaration lines for policy — vocabulary in expression position is the
/// sanctioned shape (literals.rs recorded residual).
fn field_keys() -> &'static [&'static str] {
    &[
        "claim",
        "verify_by",
        "blast_radius",
        "authority",
        "evidence_required",
        "depends_on",
    ]
}

/// Answer-key parse failures — typed, each carrying the 1-based line it was
/// detected on.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AnswerKeyError {
    /// A non-heading line outside any block.
    StrayLine { line: usize, text: String },
    /// A heading-shaped line that does not open an `AK-NNN` block.
    BadHeading { line: usize, text: String },
    /// A block id repeats; the later block fails, the first stands.
    DuplicateBlock { line: usize, id: UnitId },
    /// A field key outside the frozen set.
    UnknownField { line: usize, field: String },
    /// A field key appears twice in one block.
    DuplicateField { line: usize, field: String },
    /// A required field never appeared in the block.
    MissingField { field: &'static str },
    /// A field line without a `key: value` shape, or with an empty value.
    MalformedFieldLine { line: usize, text: String },
    /// A value outside the frozen §5.2 vocabulary (AK-004).
    UnknownEnumValue {
        line: usize,
        field: &'static str,
        value: String,
    },
    /// The `evidence_required` flow mapping is malformed: missing brace,
    /// missing or duplicate sub-key, or stray text.
    MalformedEvidenceMapping { line: usize, detail: String },
    /// The `depends_on` flow sequence is malformed.
    MalformedDependsOn { line: usize, detail: String },
    /// The locator is not a valid `EvidenceLocator`.
    InvalidLocator { line: usize, value: String },
    /// The domain `Unit` constructor refused the assembled block (the
    /// §5.3.2 tier/authority coupling, the producer rule, an empty claim).
    UnitInvalid { cause: UnitError },
}

impl fmt::Display for AnswerKeyError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::StrayLine { line, text } => {
                write!(f, "line {line}: {text:?} sits outside any block")
            }
            Self::BadHeading { line, text } => write!(
                f,
                "line {line}: heading {text:?} does not open an AK-NNN block"
            ),
            Self::DuplicateBlock { line, id } => {
                write!(
                    f,
                    "line {line}: block {id} appears again; block ids are unique"
                )
            }
            Self::UnknownField { line, field } => write!(
                f,
                "line {line}: unknown field key {field:?}; the frozen field set is \
                 {:?}",
                field_keys()
            ),
            Self::DuplicateField { line, field } => {
                write!(f, "line {line}: field {field:?} appears twice in one block")
            }
            Self::MissingField { field } => {
                write!(f, "the block is missing its required field {field:?}")
            }
            Self::MalformedFieldLine { line, text } => write!(
                f,
                "line {line}: {text:?} is not a `key: value` field line with a value"
            ),
            Self::UnknownEnumValue { line, field, value } => write!(
                f,
                "line {line}: field {field:?} carries {value:?}, which is not in the frozen \
                 vocabulary — unknown enum values are errors, never coerced defaults (AK-004)"
            ),
            Self::MalformedEvidenceMapping { line, detail } => write!(
                f,
                "line {line}: the evidence_required mapping is malformed: {detail}"
            ),
            Self::MalformedDependsOn { line, detail } => {
                write!(
                    f,
                    "line {line}: the depends_on sequence is malformed: {detail}"
                )
            }
            Self::InvalidLocator { line, value } => write!(
                f,
                "line {line}: {value:?} is not a valid evidence locator (non-empty, \
                 whitespace-free, relative)"
            ),
            Self::UnitInvalid { cause } => {
                write!(f, "the block violates a Unit invariant: {cause}")
            }
        }
    }
}

impl std::error::Error for AnswerKeyError {}

/// One block's failure: which block (when its id parsed), the heading text
/// as written, and the typed cause with its own line locus.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BlockFailure {
    /// The failing block's id; `None` when the failure belongs to no block
    /// (a stray line) or the heading itself never yielded a valid id.
    pub block: Option<UnitId>,
    /// The heading line as written (empty for failures outside blocks).
    pub heading: String,
    pub cause: AnswerKeyError,
}

impl fmt::Display for BlockFailure {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.block {
            Some(id) => write!(f, "block {id} ({:?}): {}", self.heading, self.cause),
            None => write!(f, "({:?}): {}", self.heading, self.cause),
        }
    }
}

/// The parse result: the well-formed Units and the per-block failures.
/// Both are reported; neither hides the other (I8 — a failure is not an
/// empty success).
#[derive(Debug, Clone, Default)]
pub struct ParsedKey {
    pub units: Vec<Unit>,
    pub failures: Vec<BlockFailure>,
}

impl ParsedKey {
    /// Whether every block parsed into a Unit.
    pub fn is_clean(&self) -> bool {
        self.failures.is_empty()
    }
}

/// One block under assembly.
struct BlockDraft {
    id: Option<UnitId>,
    heading: String,
    /// The 1-based line the block's heading sat on, for failure loci that
    /// surface at close time (duplicate ids).
    open_line: usize,
    claim: Option<String>,
    verify_by: Option<VerifyBy>,
    blast_radius: Option<BlastRadius>,
    authority: Option<Authority>,
    evidence_required: Option<EvidenceRequirement>,
    depends_on: Vec<UnitId>,
    depends_on_seen: bool,
    /// Whether a field-line failure already failed this block. One defect,
    /// one report: close time does not pile a redundant `MissingField`
    /// onto a block whose bad line was already recorded.
    failed: bool,
}

impl BlockDraft {
    fn new(id: Option<UnitId>, heading: String, open_line: usize) -> Self {
        Self {
            id,
            heading,
            open_line,
            claim: None,
            verify_by: None,
            blast_radius: None,
            authority: None,
            evidence_required: None,
            depends_on: Vec::new(),
            depends_on_seen: false,
            failed: false,
        }
    }
}

/// Parses the answer key. Never fails the whole file for a block defect:
/// the failing block lands in `failures` and parsing continues (plan Task 7
/// step 4). CRLF endings are tolerated by stripping one trailing `\r` per
/// line; the pipeline's caller normalizes first anyway.
pub fn parse_answer_key(text: &str) -> ParsedKey {
    let mut parsed = ParsedKey::default();
    let mut seen: Vec<UnitId> = Vec::new();
    let mut draft: Option<BlockDraft> = None;

    for (index, raw_line) in text.split('\n').enumerate() {
        let number = index + 1;
        let line = raw_line.strip_suffix('\r').unwrap_or(raw_line);
        if line.is_empty() {
            close_block(&mut draft, &mut parsed, &mut seen);
            continue;
        }
        if line.starts_with('#') {
            close_block(&mut draft, &mut parsed, &mut seen);
            match open_block(line) {
                Ok(id) => draft = Some(BlockDraft::new(Some(id), line.to_owned(), number)),
                Err(()) => parsed.failures.push(BlockFailure {
                    block: None,
                    heading: line.to_owned(),
                    cause: AnswerKeyError::BadHeading {
                        line: number,
                        text: line.to_owned(),
                    },
                }),
            }
            continue;
        }
        match draft.as_mut() {
            None => parsed.failures.push(BlockFailure {
                block: None,
                heading: String::new(),
                cause: AnswerKeyError::StrayLine {
                    line: number,
                    text: line.to_owned(),
                },
            }),
            Some(block) => match field_line(block, number, line) {
                Ok(()) => {}
                Err(cause) => {
                    block.failed = true;
                    parsed.failures.push(BlockFailure {
                        block: block.id.clone(),
                        heading: block.heading.clone(),
                        cause,
                    });
                }
            },
        }
    }
    close_block(&mut draft, &mut parsed, &mut seen);
    parsed
}

/// The all-or-nothing wrapper: every block well-formed, or the first
/// failure as the error.
pub fn parse_all_or_fail(text: &str) -> Result<Vec<Unit>, BlockFailure> {
    let parsed = parse_answer_key(text);
    if let Some(failure) = parsed.failures.first() {
        return Err(failure.clone());
    }
    Ok(parsed.units)
}

/// `## AK-NNN` → the UnitId. Anything else (wrong depth, trailing text, a
/// non-AK prefix, a malformed id) is `Err(())`; the caller already holds
/// the line text for the typed error.
fn open_block(line: &str) -> Result<UnitId, ()> {
    let rest = line.strip_prefix("## ").ok_or(())?;
    answer_key_id(rest).ok_or(())
}

/// A §5.2 UnitId that also carries the answer-key's `AK` prefix.
fn answer_key_id(text: &str) -> Option<UnitId> {
    let id = UnitId::new(text).ok()?;
    (id.as_str().starts_with(BLOCK_PREFIX)).then_some(id)
}

/// Parses one field line into the block draft, or the typed block failure.
fn field_line(block: &mut BlockDraft, number: usize, line: &str) -> Result<(), AnswerKeyError> {
    let (key, value) = line
        .split_once(':')
        .ok_or_else(|| AnswerKeyError::MalformedFieldLine {
            line: number,
            text: line.to_owned(),
        })?;
    let value = value.strip_prefix(' ').unwrap_or(value);
    if value.is_empty() {
        return Err(AnswerKeyError::MalformedFieldLine {
            line: number,
            text: line.to_owned(),
        });
    }
    let already = |field: &str| AnswerKeyError::DuplicateField {
        line: number,
        field: field.to_owned(),
    };
    match key {
        "claim" => {
            if block.claim.is_some() {
                return Err(already("claim"));
            }
            block.claim = Some(value.to_owned());
        }
        "verify_by" => {
            if block.verify_by.is_some() {
                return Err(already("verify_by"));
            }
            block.verify_by =
                Some(
                    VerifyBy::parse(value).map_err(|_| AnswerKeyError::UnknownEnumValue {
                        line: number,
                        field: "verify_by",
                        value: value.to_owned(),
                    })?,
                );
        }
        "blast_radius" => {
            if block.blast_radius.is_some() {
                return Err(already("blast_radius"));
            }
            block.blast_radius =
                Some(
                    BlastRadius::parse(value).map_err(|_| AnswerKeyError::UnknownEnumValue {
                        line: number,
                        field: "blast_radius",
                        value: value.to_owned(),
                    })?,
                );
        }
        "authority" => {
            if block.authority.is_some() {
                return Err(already("authority"));
            }
            block.authority =
                Some(
                    Authority::parse(value).map_err(|_| AnswerKeyError::UnknownEnumValue {
                        line: number,
                        field: "authority",
                        value: value.to_owned(),
                    })?,
                );
        }
        "evidence_required" => {
            if block.evidence_required.is_some() {
                return Err(already("evidence_required"));
            }
            block.evidence_required = Some(parse_evidence_mapping(number, value)?);
        }
        "depends_on" => {
            if block.depends_on_seen {
                return Err(already("depends_on"));
            }
            block.depends_on_seen = true;
            block.depends_on = parse_depends_on(number, value)?;
        }
        other => {
            return Err(AnswerKeyError::UnknownField {
                line: number,
                field: other.to_owned(),
            });
        }
    }
    Ok(())
}

/// `{kind: K, locator: L, producer: P}` — exactly the three sub-keys, no
/// duplicates, no extras, all present.
fn parse_evidence_mapping(
    number: usize,
    value: &str,
) -> Result<EvidenceRequirement, AnswerKeyError> {
    let malformed = |detail: String| AnswerKeyError::MalformedEvidenceMapping {
        line: number,
        detail,
    };
    let Some(body) = value
        .strip_prefix('{')
        .and_then(|rest| rest.strip_suffix('}'))
    else {
        return Err(malformed(format!(
            "expected {{kind: …, locator: …, producer: …}}, got {value:?}"
        )));
    };
    let mut kind: Option<EvidenceKind> = None;
    let mut locator: Option<EvidenceLocator> = None;
    let mut producer: Option<Producer> = None;
    for part in body.split(',') {
        let (sub_key, sub_value) = part
            .trim()
            .split_once(':')
            .ok_or_else(|| malformed(format!("sub-entry {part:?} is not a `name: value` pair")))?;
        let sub_value = sub_value.trim();
        if sub_value.is_empty() {
            return Err(malformed(format!("sub-key {sub_key:?} carries no value")));
        }
        let unknown = || AnswerKeyError::UnknownEnumValue {
            line: number,
            field: "evidence_required",
            value: sub_value.to_owned(),
        };
        match sub_key.trim() {
            "kind" => {
                if kind.is_some() {
                    return Err(malformed("sub-key `kind` appears twice".to_owned()));
                }
                kind = Some(EvidenceKind::parse(sub_value).map_err(|_| unknown())?);
            }
            "locator" => {
                if locator.is_some() {
                    return Err(malformed("sub-key `locator` appears twice".to_owned()));
                }
                locator = Some(EvidenceLocator::new(sub_value).map_err(|_| {
                    AnswerKeyError::InvalidLocator {
                        line: number,
                        value: sub_value.to_owned(),
                    }
                })?);
            }
            "producer" => {
                if producer.is_some() {
                    return Err(malformed("sub-key `producer` appears twice".to_owned()));
                }
                producer = Some(Producer::parse(sub_value).map_err(|_| unknown())?);
            }
            other => {
                return Err(malformed(format!(
                    "unknown sub-key {other:?}; the mapping carries kind, locator, producer only"
                )));
            }
        }
    }
    let kind = kind.ok_or_else(|| malformed("sub-key `kind` is missing".to_owned()))?;
    let locator = locator.ok_or_else(|| malformed("sub-key `locator` is missing".to_owned()))?;
    let producer = producer.ok_or_else(|| malformed("sub-key `producer` is missing".to_owned()))?;
    Ok(EvidenceRequirement::new(kind, locator, producer))
}

/// `[AK-008]` / `[AK-013, AK-040]` / `[]` — zero or more AK-prefixed ids.
fn parse_depends_on(number: usize, value: &str) -> Result<Vec<UnitId>, AnswerKeyError> {
    let malformed = |detail: String| AnswerKeyError::MalformedDependsOn {
        line: number,
        detail,
    };
    let Some(body) = value
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
    else {
        return Err(malformed(format!(
            "expected [AK-NNN, …] flow sequence, got {value:?}"
        )));
    };
    if body.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut ids = Vec::new();
    for entry in body.split(',') {
        let entry = entry.trim();
        let Some(id) = answer_key_id(entry) else {
            return Err(malformed(format!(
                "{entry:?} is not an AK-NNN unit id a block may depend on"
            )));
        };
        ids.push(id);
    }
    Ok(ids)
}

/// Finishes a block: duplicate-id check, required-field check, then the
/// domain constructor — each failure isolated to this block.
fn close_block(draft: &mut Option<BlockDraft>, parsed: &mut ParsedKey, seen: &mut Vec<UnitId>) {
    // A draft only exists when its heading parsed (open_block produced the
    // id), so the None arm is unreachable by construction and returns
    // rather than inventing a failure locus.
    let Some(block) = draft.take() else {
        return;
    };
    let fail = |parsed: &mut ParsedKey, cause: AnswerKeyError| {
        parsed.failures.push(BlockFailure {
            block: block.id.clone(),
            heading: block.heading.clone(),
            cause,
        });
    };
    let id = match block.id.clone() {
        Some(id) => id,
        None => return,
    };
    if block.failed {
        // The block's defect was recorded where it happened; the Unit is
        // simply not admitted.
        return;
    }
    if seen.contains(&id) {
        fail(
            parsed,
            AnswerKeyError::DuplicateBlock {
                line: block.open_line,
                id,
            },
        );
        return;
    }
    let missing = [
        block.claim.is_none().then_some("claim"),
        block.verify_by.is_none().then_some("verify_by"),
        block.blast_radius.is_none().then_some("blast_radius"),
        block.authority.is_none().then_some("authority"),
        block
            .evidence_required
            .is_none()
            .then_some("evidence_required"),
    ]
    .into_iter()
    .flatten()
    .next();
    if let Some(field) = missing {
        fail(parsed, AnswerKeyError::MissingField { field });
        return;
    }
    // No defaults anywhere: the tuple keeps the presence invariant local,
    // and the None arm is unreachable because the `missing` check directly
    // above already returned for any absent field.
    let built = match (
        block.claim.clone(),
        block.verify_by,
        block.blast_radius,
        block.authority,
        block.evidence_required.clone(),
        block.depends_on,
    ) {
        (
            Some(claim),
            Some(verify_by),
            Some(blast_radius),
            Some(authority),
            Some(evidence_required),
            depends_on,
        ) => Unit::new(
            id.clone(),
            claim,
            verify_by,
            blast_radius,
            authority,
            evidence_required,
            depends_on,
        ),
        absent => {
            unreachable!("every required field was checked present immediately above: {absent:?}")
        }
    };
    match built {
        Ok(unit) => {
            seen.push(id);
            parsed.units.push(unit);
        }
        Err(cause) => fail(parsed, AnswerKeyError::UnitInvalid { cause }),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use gauntlet_domain::{BlastRadius as Blast, Producer as Prod, VerifyBy as Verify};

    /// A minimal well-formed block to mutate per case.
    fn block_text(id: &str) -> String {
        format!(
            "## {id}\nclaim: A checkable claim.\nverify_by: test\nblast_radius: local\n\
             authority: critic\nevidence_required: {{kind: test_report, locator: \
             suite-check-0001, producer: instrument}}\n"
        )
    }

    fn key_of(parts: &[String]) -> String {
        parts.join("\n\n")
    }

    #[test]
    fn a_well_formed_block_parses_into_a_unit() {
        let parsed = parse_answer_key(&block_text("AK-001"));
        assert!(parsed.is_clean(), "{:?}", parsed.failures);
        assert_eq!(parsed.units.len(), 1);
        let unit = &parsed.units[0];
        assert_eq!(unit.id().as_str(), "AK-001");
        assert_eq!(unit.claim(), "A checkable claim.");
        assert_eq!(unit.verify_by(), Verify::Test);
        assert_eq!(unit.blast_radius(), Blast::Local);
        assert_eq!(unit.authority(), gauntlet_domain::Authority::Critic);
        assert_eq!(unit.evidence_required().producer(), Prod::Instrument);
        assert_eq!(
            unit.evidence_required().locator().as_str(),
            "suite-check-0001"
        );
        assert_eq!(
            unit.evidence_required().kind(),
            gauntlet_domain::EvidenceKind::TestReport
        );
        assert!(unit.depends_on().is_empty());
    }

    #[test]
    fn the_frozen_answer_key_parses_seventy_clean_units() {
        // The frozen ruler itself, read-only, never written: the parser
        // must admit exactly AK-001..AK-070 with no failures. A failure
        // here means the parser drifted from the frozen shape.
        let manifest_dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let repo_root = manifest_dir
            .parent()
            .and_then(std::path::Path::parent)
            .expect("the app crate sits two levels under the repository root");
        let key_path = repo_root.join(".wayfinder").join("answer-key.md");
        let text = std::fs::read_to_string(&key_path)
            .unwrap_or_else(|err| panic!("the frozen answer key is readable: {err}"));
        let parsed = parse_answer_key(&text);
        assert!(
            parsed.is_clean(),
            "the frozen key must parse clean: {:?}",
            parsed.failures
        );
        assert_eq!(parsed.units.len(), 70);
        for (index, unit) in parsed.units.iter().enumerate() {
            let expected = format!("AK-{:03}", index + 1);
            assert_eq!(
                unit.id().as_str(),
                expected,
                "units arrive in frozen id order"
            );
        }
        // depends_on appears in both frozen spellings somewhere.
        assert!(
            parsed.units.iter().any(|unit| unit.depends_on().len() == 2),
            "at least one block carries a two-entry depends_on"
        );
        assert!(
            parsed.units.iter().any(|unit| unit.depends_on().len() == 1),
            "at least one block carries a one-entry depends_on"
        );
    }

    #[test]
    fn one_malformed_block_fails_only_itself() {
        let mut bad = block_text("AK-002");
        bad.push_str("verify_by: vibes\n");
        let text = key_of(&[block_text("AK-001"), bad, block_text("AK-003")]);
        let parsed = parse_answer_key(&text);
        assert_eq!(
            parsed.units.len(),
            2,
            "the two well-formed blocks still parse"
        );
        assert_eq!(parsed.failures.len(), 1);
        let failure = &parsed.failures[0];
        assert_eq!(failure.block.as_ref().map(UnitId::as_str), Some("AK-002"));
        assert!(
            matches!(
                &failure.cause,
                AnswerKeyError::DuplicateField { field, .. } if field == "verify_by"
            ),
            "{}",
            failure.cause
        );
    }

    #[test]
    fn an_unknown_enum_value_names_field_value_and_block() {
        let mut bad = block_text("AK-009");
        bad = bad.replace("verify_by: test", "verify_by: vibes");
        let parsed = parse_answer_key(&bad);
        assert!(parsed.units.is_empty());
        assert_eq!(parsed.failures.len(), 1);
        match &parsed.failures[0].cause {
            AnswerKeyError::UnknownEnumValue { field, value, .. } => {
                assert_eq!(*field, "verify_by");
                assert_eq!(value, "vibes");
            }
            other => panic!("expected UnknownEnumValue, got {other}"),
        }
        let text = parsed.failures[0].to_string();
        assert!(text.contains("verify_by"), "{text}");
        assert!(text.contains("vibes"), "{text}");
        assert!(text.contains("AK-009"), "{text}");
    }

    #[test]
    fn every_enum_field_reports_unknown_values_through_the_same_door() {
        // Each fixed vocabulary goes through its wire_enum parse: an
        // unknown token is a typed UnknownEnumValue whatever field carries
        // it (AK-004).
        for replacement in [
            "verify_by: speculation",
            "blast_radius: planetary",
            "authority: the_usual",
            "kind: testimony",
            "producer: model",
        ] {
            let field = replacement.split_once(": ").expect("shaped above").0;
            let good = match field {
                "verify_by" => "verify_by: test",
                "blast_radius" => "blast_radius: local",
                "authority" => "authority: critic",
                "kind" => "kind: test_report",
                "producer" => "producer: instrument",
                other => panic!("no well-formed spelling declared for {other}"),
            };
            let text = block_text("AK-001").replace(good, replacement);
            let parsed = parse_answer_key(&text);
            assert!(
                parsed.units.is_empty(),
                "{replacement} must not admit a Unit"
            );
            assert!(
                matches!(
                    parsed.failures.first().map(|failure| &failure.cause),
                    Some(AnswerKeyError::UnknownEnumValue { .. })
                ),
                "{replacement} must be a typed enum failure, got {:?}",
                parsed.failures
            );
        }
    }

    #[test]
    fn a_missing_required_field_is_a_block_failure() {
        for line in [
            "claim: A checkable claim.\n",
            "verify_by: test\n",
            "blast_radius: local\n",
            "authority: critic\n",
            "evidence_required: {kind: test_report, locator: suite-check-0001, producer: instrument}\n",
        ] {
            let field = line
                .split_once(": ")
                .expect("field lines are `key: value`")
                .0;
            let text = block_text("AK-001").replace(line, "");
            let parsed = parse_answer_key(&text);
            assert!(
                matches!(
                    parsed.failures.first().map(|failure| &failure.cause),
                    Some(AnswerKeyError::MissingField { field: missing }) if *missing == field
                ),
                "removing the {field} line must be a MissingField failure, got {:?}",
                parsed.failures
            );
            assert!(parsed.units.is_empty());
        }
    }

    #[test]
    fn unknown_field_keys_and_stray_lines_are_typed_errors() {
        let mut text = block_text("AK-001");
        text.push_str("verdict: pass\n");
        let parsed = parse_answer_key(&text);
        assert!(
            matches!(
                parsed.failures.first().map(|failure| &failure.cause),
                Some(AnswerKeyError::UnknownField { field, .. }) if field == "verdict"
            ),
            "{:?}",
            parsed.failures
        );
        // A stray line outside any block carries no block id.
        let parsed = parse_answer_key("loose text\n\n## AK-001\nclaim: x\n");
        assert!(
            matches!(
                parsed.failures.first(),
                Some(BlockFailure {
                    block: None,
                    cause: AnswerKeyError::StrayLine { .. },
                    ..
                })
            ),
            "{:?}",
            parsed.failures
        );
    }

    #[test]
    fn malformed_evidence_mappings_fail_the_block_not_the_file() {
        let cases = [
            "evidence_required: kind: test_report, locator: l1, producer: instrument",
            "evidence_required: {kind: test_report, locator: l1",
            "evidence_required: {kind: test_report, producer: instrument}",
            "evidence_required: {locator: l1, producer: instrument}",
            "evidence_required: {kind: test_report, locator: l1}",
            "evidence_required: {kind: test_report, locator: l1, producer: instrument, extra: 1}",
            "evidence_required: {kind: test_report, kind: log, locator: l1, producer: instrument}",
            "evidence_required: {}",
            "evidence_required: {kind: test_report, locator: has space, producer: instrument}",
        ];
        for value in cases {
            let text = block_text("AK-004").replace(
                "evidence_required: {kind: test_report, locator: suite-check-0001, producer: instrument}",
                value,
            );
            let parsed = parse_answer_key(&text);
            assert!(parsed.units.is_empty(), "{value:?} must not admit a Unit");
            assert!(
                matches!(
                    parsed.failures.first().map(|failure| &failure.cause),
                    Some(
                        AnswerKeyError::MalformedEvidenceMapping { .. }
                            | AnswerKeyError::InvalidLocator { .. }
                    )
                ),
                "{value:?} must be a mapping/locator failure, got {:?}",
                parsed.failures
            );
        }
    }

    #[test]
    fn depends_on_parses_all_three_frozen_spellings() {
        for (value, expected) in [("[]", 0), ("[AK-008]", 1), ("[AK-013, AK-040]", 2)] {
            let mut text = block_text("AK-041");
            text.push_str(&format!("depends_on: {value}\n"));
            let parsed = parse_answer_key(&text);
            assert!(parsed.is_clean(), "{:?}", parsed.failures);
            assert_eq!(parsed.units[0].depends_on().len(), expected);
        }
        for bad in ["AK-008", "[AK-8]", "[ab-008]", "[,]", "[AK-008,]"] {
            let mut text = block_text("AK-041");
            text.push_str(&format!("depends_on: {bad}\n"));
            let parsed = parse_answer_key(&text);
            assert!(
                matches!(
                    parsed.failures.first().map(|failure| &failure.cause),
                    Some(AnswerKeyError::MalformedDependsOn { .. })
                ),
                "depends_on {bad:?} must be malformed, got {:?}",
                parsed.failures
            );
        }
    }

    #[test]
    fn non_ak_headings_and_wrong_depth_are_heading_failures() {
        for heading in [
            "## AK-1",
            "## AK-0001x",
            "## AB-001",
            "## AK-001 title one",
            "##AK-001",
            "### AK-001",
            "# AK-001",
            "## Summary",
        ] {
            let text = format!("{heading}\nclaim: x\n");
            let parsed = parse_answer_key(&text);
            assert!(
                matches!(
                    parsed.failures.first().map(|failure| &failure.cause),
                    Some(AnswerKeyError::BadHeading { .. })
                ),
                "heading {heading:?} must be refused, got {:?}",
                parsed.failures
            );
            assert!(parsed.units.is_empty());
        }
    }

    #[test]
    fn a_duplicate_block_id_fails_the_later_block() {
        let text = key_of(&[block_text("AK-001"), block_text("AK-001")]);
        let parsed = parse_answer_key(&text);
        assert_eq!(parsed.units.len(), 1, "the first stands");
        assert!(
            matches!(
                parsed.failures.first().map(|failure| &failure.cause),
                Some(AnswerKeyError::DuplicateBlock { id, .. }) if id.as_str() == "AK-001"
            ),
            "{:?}",
            parsed.failures
        );
    }

    #[test]
    fn a_tier_authority_coupling_violation_is_a_block_failure() {
        // AK-006's parse face: irreversible work under critic authority
        // must fail validation at parse time, as a UnitInvalid block
        // failure carrying the typed domain cause.
        // AK-006's authority is already `critic` in the fixture; only the
        // tier is rewritten.
        let text =
            block_text("AK-006").replace("blast_radius: local", "blast_radius: irreversible");
        let parsed = parse_answer_key(&text);
        assert!(parsed.units.is_empty());
        match parsed.failures.first().map(|failure| &failure.cause) {
            Some(AnswerKeyError::UnitInvalid { cause }) => {
                assert!(
                    matches!(cause, UnitError::IrreversibleRequiresHuman { .. }),
                    "{cause}"
                );
            }
            other => panic!("expected UnitInvalid, got {other:?}"),
        }
    }

    #[test]
    fn parse_all_or_fail_is_all_or_nothing() {
        assert_eq!(
            parse_all_or_fail(&block_text("AK-001"))
                .expect("clean parses")
                .len(),
            1
        );
        let mut bad = block_text("AK-002");
        bad.push_str("phantom_key: 1\n");
        let failure = parse_all_or_fail(&bad).expect_err("any failure refuses the whole key");
        assert!(
            matches!(failure.cause, AnswerKeyError::UnknownField { .. }),
            "{}",
            failure.cause
        );
    }

    #[test]
    fn crlf_endings_are_tolerated_line_by_line() {
        let crlf = block_text("AK-001").replace('\n', "\r\n");
        let parsed = parse_answer_key(&crlf);
        assert!(parsed.is_clean(), "{:?}", parsed.failures);
        assert_eq!(parsed.units.len(), 1);
    }
}
