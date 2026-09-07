//! The strict `baseline_task@v1` codec (task 26; the charter's interface
//! contract between the B1 fixture task set and this app module).
//!
//! The same posture as the zero-dependency readers this repo already runs
//! (ports JSON, the GAUNTLET.md front-matter grammar): a hand-rolled parser
//! over a DOCUMENTED SUBSET, where strictness is the feature. Model- and
//! fixture-shaped input is untrusted external input (spec §21.2): parse,
//! validate, then admit.
//!
//! # The grammar
//!
//! ```text
//! schema: baseline_task@v1
//! id: <non-empty scalar>
//! prompt: |              # block scalar (the builder envelope body)
//!   ...
//! requirement: |         # block scalar (the critic envelope body)
//!   ...
//! held_out: <relative path>
//! scenarios: [token, token, ...]
//! ```
//!
//! Accepted, and nothing else:
//!
//! - exactly the six keys above, each REQUIRED exactly once, in any order;
//! - `schema` MUST equal [`SCHEMA_TOKEN`];
//! - `prompt`/`requirement` as `|` block scalars: every following line MORE
//!   indented than the key line belongs to the block; the block's common
//!   indent is the first content line's indent, stripped from every line;
//!   blank lines inside a block are kept; the block's text is the lines
//!   joined with `\n` plus one trailing `\n` (YAML clip);
//! - `held_out` a non-empty RELATIVE path (absolute refused — the held-out
//!   instrument is fixture-relative, never a host path);
//! - `scenarios` an inline flow sequence of scenario tokens (the
//!   [`super::profiles::BaselineScenario`] vocabulary — anything else is a
//!   typed error); duplicates refused (a duplicated scenario is fixture
//!   drift, not intent); the list may not be empty.
//!
//! Refused, each a typed error carrying the 1-based line: unknown keys;
//! duplicate keys; a missing key; a second document fence; comments (there
//! are none in the contract, and a tolerated comment hides drift); tabs in
//! indentation; a key not immediately followed by `:`; a value shape a key
//! cannot carry (`|` on a non-block key, inline text on a block key);
//! anything after a closed flow sequence.

use std::path::{Path, PathBuf};

use super::profiles::{BaselineScenario, UnknownToken};

/// The only schema token this codec admits.
pub const SCHEMA_TOKEN: &str = "baseline_task@v1";

/// The canonical field list, in declaration order (the parse emits them in
/// file order; this list drives the missing-field check).
const FIELDS: &[&str] = &[
    "schema",
    "id",
    "prompt",
    "requirement",
    "held_out",
    "scenarios",
];

/// One parsed baseline task: the bounded task text, the critic requirement,
/// the held-out instrument's fixture-relative path, and the declared
/// scenario set.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BaselineTask {
    id: String,
    prompt: String,
    requirement: String,
    held_out: PathBuf,
    scenarios: Vec<BaselineScenario>,
}

impl BaselineTask {
    /// The task id (the report's per-leg `task` field).
    pub fn id(&self) -> &str {
        &self.id
    }

    /// The builder envelope body (the bounded task text).
    pub fn prompt(&self) -> &str {
        &self.prompt
    }

    /// The critic envelope body: the full property text, both properties
    /// included. This is EXACTLY what the C posture's critic dispatch
    /// carries — the builder's result document never travels.
    pub fn requirement(&self) -> &str {
        &self.requirement
    }

    /// The held-out instrument's fixture-relative path (the harness runs it
    /// over the leg's workspace as ground truth).
    pub fn held_out(&self) -> &Path {
        &self.held_out
    }

    /// The declared scenario set (the legs the fixture scripts).
    pub fn scenarios(&self) -> &[BaselineScenario] {
        &self.scenarios
    }
}

/// Why a `baseline_task@v1` document refused — typed, with the 1-based line
/// where the refusal arises (the fixture's line, not an offset).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TaskError {
    /// The `schema` value is present but is not [`SCHEMA_TOKEN`].
    SchemaToken { line: usize, found: String },
    /// A key outside the six-field contract.
    UnknownField { line: usize, field: String },
    /// A key appears twice — one of them is shadowing the other.
    DuplicateField { line: usize, field: String },
    /// A required key is absent.
    MissingField { field: &'static str },
    /// A present-but-empty scalar (`id`, `held_out`), or an empty
    /// block-scalar body (`prompt`, `requirement`).
    EmptyField { field: &'static str },
    /// `held_out` is an absolute path — the instrument is fixture-relative.
    HeldOutNotRelative { line: usize, path: String },
    /// The `id` is not a single safe path component (task-26 corrections,
    /// F3): the id materializes leg directories (`<legs-root>/<id>.<scenario>-<n>`),
    /// so an id carrying a separator, a parent-directory segment, or a
    /// leading dot is a path traversal, never a name.
    IdNotASafeComponent { line: usize, id: String },
    /// A `scenarios` member is outside the vocabulary.
    UnknownScenario { line: usize, source: UnknownToken },
    /// The `scenarios` list is empty or duplicates a member.
    ScenarioList { line: usize, detail: String },
    /// A line this grammar cannot admit (bad key shape, tab indent, a
    /// second fence, a comment, stray content).
    Malformed { line: usize, detail: String },
}

impl core::fmt::Display for TaskError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SchemaToken { line, found } => write!(
                f,
                "line {line}: schema token is {found:?}, but this codec admits only \
                 {SCHEMA_TOKEN:?}"
            ),
            Self::UnknownField { line, field } => write!(
                f,
                "line {line}: field {field:?} is not part of the {SCHEMA_TOKEN} contract \
                 (unknown fields are refused, not ignored)"
            ),
            Self::DuplicateField { line, field } => write!(
                f,
                "line {line}: field {field:?} appears twice — an ambiguous document is \
                 not admitted"
            ),
            Self::MissingField { field } => {
                write!(f, "the {SCHEMA_TOKEN} contract requires field {field:?}")
            }
            Self::EmptyField { field } => {
                write!(f, "field {field:?} is present but empty")
            }
            Self::HeldOutNotRelative { line, path } => write!(
                f,
                "line {line}: held_out path {path:?} is absolute — the held-out \
                 instrument is a fixture-relative path"
            ),
            Self::IdNotASafeComponent { line, id } => write!(
                f,
                "line {line}: id {id:?} is refused — a task id must be a single safe \
                 path component: non-empty, separator-free, without a parent-directory \
                 segment, and not dot-led (it names leg directories under the scratch root)"
            ),
            Self::UnknownScenario { line, source } => {
                write!(f, "line {line}: {source}")
            }
            Self::ScenarioList { line, detail } => {
                write!(f, "line {line}: the scenarios list is refused: {detail}")
            }
            Self::Malformed { line, detail } => {
                write!(f, "line {line}: {detail}")
            }
        }
    }
}

impl std::error::Error for TaskError {}

/// Parses one `baseline_task@v1` document. Strict: see the module grammar.
pub fn parse_task(text: &str) -> Result<BaselineTask, TaskError> {
    let mut schema: Option<(String, usize)> = None;
    let mut id: Option<(String, usize)> = None;
    let mut prompt: Option<(String, usize)> = None;
    let mut requirement: Option<(String, usize)> = None;
    let mut held_out: Option<(String, usize)> = None;
    let mut scenarios: Option<(Vec<BaselineScenario>, usize)> = None;

    let lines: Vec<&str> = text.split('\n').collect();
    let mut index = 0usize;
    while index < lines.len() {
        let line_no = index + 1;
        let raw = lines[index];
        if raw.trim_end().is_empty() {
            index += 1;
            continue;
        }
        let line = strip_indent(raw, line_no)?;
        let Some((key, rest)) = split_key(line, line_no)? else {
            index += 1;
            continue;
        };
        if !FIELDS.contains(&key.as_str()) {
            return Err(TaskError::UnknownField {
                line: line_no,
                field: key,
            });
        }
        // Exactly once, wherever it appears.
        let already = match key.as_str() {
            "schema" => schema.is_some(),
            "id" => id.is_some(),
            "prompt" => prompt.is_some(),
            "requirement" => requirement.is_some(),
            "held_out" => held_out.is_some(),
            _ => scenarios.is_some(),
        };
        if already {
            return Err(TaskError::DuplicateField {
                line: line_no,
                field: key,
            });
        }
        match key.as_str() {
            "prompt" | "requirement" => {
                if rest != "|" {
                    return Err(TaskError::Malformed {
                        line: line_no,
                        detail: format!(
                            "field {key:?} is a block scalar: its line is exactly \
                             {key:?} followed by `|`"
                        ),
                    });
                }
                let (block, next) = read_block(&lines, index + 1)?;
                if block.trim().is_empty() {
                    return Err(TaskError::EmptyField {
                        field: key_box(key.as_str()),
                    });
                }
                let slot = if key == "prompt" {
                    &mut prompt
                } else {
                    &mut requirement
                };
                *slot = Some((block, line_no));
                index = next;
                continue;
            }
            _ => {}
        }
        let value = inline_value(rest, line_no, key.as_str())?;
        match key.as_str() {
            "schema" => schema = Some((value, line_no)),
            "id" => {
                validate_task_id(&value, line_no)?;
                id = Some((value, line_no));
            }
            "held_out" => held_out = Some((value, line_no)),
            "scenarios" => scenarios = Some((parse_scenario_flow(&value, line_no)?, line_no)),
            _ => {}
        }
        index += 1;
    }

    let (schema, schema_line) = schema.ok_or(TaskError::MissingField { field: "schema" })?;
    if schema != SCHEMA_TOKEN {
        return Err(TaskError::SchemaToken {
            line: schema_line,
            found: schema,
        });
    }
    let id = require_scalar(id.map(|(v, _)| v), "id")?;
    let prompt = require_text(prompt.map(|(v, _)| v), "prompt")?;
    let requirement = require_text(requirement.map(|(v, _)| v), "requirement")?;
    let (held_out, held_line) = held_out.ok_or(TaskError::MissingField { field: "held_out" })?;
    let held_path = PathBuf::from(&held_out);
    if held_path.is_absolute() {
        return Err(TaskError::HeldOutNotRelative {
            line: held_line,
            path: held_out,
        });
    }
    let scenarios = scenarios.ok_or(TaskError::MissingField { field: "scenarios" })?;
    Ok(BaselineTask {
        id,
        prompt,
        requirement,
        held_out: held_path,
        scenarios: scenarios.0,
    })
}

/// Validates the `id` as a SINGLE safe path component (task-26 corrections,
/// F3): the id flows into leg-directory construction
/// (`<legs-root>/<id>.<scenario>-<n>`), so anything that could escape the
/// scratch root is refused at parse, naming the line: any separator (either
/// one), any leading dot, and — the component-wise guard, the controller's
/// N4 precedent — anything [`std::path::Component`] classifies as a parent
/// directory, a root, a prefix, or the current directory. The
/// empty/whitespace case is [`TaskError::EmptyField`]'s (checked at the
/// required-field stage); this is the traversal half.
fn validate_task_id(value: &str, line_no: usize) -> Result<(), TaskError> {
    let component_safe = std::path::Path::new(value)
        .components()
        .all(|component| matches!(component, std::path::Component::Normal(_)));
    let safe = !value.is_empty()
        && !value.contains('/')
        && !value.contains('\\')
        && !value.starts_with('.')
        && component_safe;
    if safe {
        Ok(())
    } else {
        Err(TaskError::IdNotASafeComponent {
            line: line_no,
            id: value.to_owned(),
        })
    }
}

fn key_box(key: &str) -> &'static str {
    match key {
        "prompt" => "prompt",
        "requirement" => "requirement",
        _ => unreachable!("only block keys reach here"),
    }
}

fn require_scalar(value: Option<String>, field: &'static str) -> Result<String, TaskError> {
    match value {
        Some(text) if !text.trim().is_empty() => Ok(text),
        Some(_) => Err(TaskError::EmptyField { field }),
        None => Err(TaskError::MissingField { field }),
    }
}

fn require_text(value: Option<String>, field: &'static str) -> Result<String, TaskError> {
    match value {
        Some(text) => Ok(text),
        None => Err(TaskError::MissingField { field }),
    }
}

/// Strips the key line's leading spaces (tabs refused) and returns the
/// remainder. A `---` fence or a comment line is refused with its line.
fn strip_indent(raw: &str, line_no: usize) -> Result<&str, TaskError> {
    let mut offset = 0usize;
    for ch in raw.chars() {
        match ch {
            ' ' => offset += 1,
            '\t' => {
                return Err(TaskError::Malformed {
                    line: line_no,
                    detail: "a tab in indentation is refused".to_owned(),
                });
            }
            _ => break,
        }
    }
    let rest = &raw[offset..];
    if rest.starts_with('#') {
        return Err(TaskError::Malformed {
            line: line_no,
            detail: "comments are not part of the contract — remove the line rather \
                     than annotating the fixture"
                .to_owned(),
        });
    }
    if rest.trim_end() == "---" {
        return Err(TaskError::Malformed {
            line: line_no,
            detail: "a second document fence is refused — one document per file".to_owned(),
        });
    }
    Ok(rest.trim_end())
}

/// Splits `key: value`; a line with no `key:` shape is a refusal. An
/// entirely empty line yields `None` (handled by the caller's blank skip).
fn split_key(line: &str, line_no: usize) -> Result<Option<(String, &str)>, TaskError> {
    if line.is_empty() {
        return Ok(None);
    }
    let Some(colon) = line.find(':') else {
        return Err(TaskError::Malformed {
            line: line_no,
            detail: format!(
                "expected `key: value`, found {:?} — every top-level line is a \
                 contract field",
                line
            ),
        });
    };
    let key = line[..colon].trim();
    if key.is_empty() {
        return Err(TaskError::Malformed {
            line: line_no,
            detail: "an empty key is refused".to_owned(),
        });
    }
    if key.contains(char::is_whitespace) {
        return Err(TaskError::Malformed {
            line: line_no,
            detail: format!("the key {key:?} contains whitespace"),
        });
    }
    Ok(Some((key.to_owned(), line[colon + 1..].trim())))
}

/// Reads a `|` block scalar starting after the key line: every following
/// line strictly more indented than the KEY's column belongs to the block.
/// The block's text is its lines joined with `\n` plus one trailing `\n`;
/// the common indent (the first content line's) is stripped from every
/// line. Returns the text and the index of the first line AFTER the block.
fn read_block(lines: &[&str], start: usize) -> Result<(String, usize), TaskError> {
    // The key line is `key: |` at column 0, so any indented line continues
    // the block. Find the block's extent and its common indent.
    let mut end = start;
    let mut common: Option<usize> = None;
    while end < lines.len() {
        let raw = lines[end];
        if raw.trim().is_empty() {
            end += 1;
            continue;
        }
        let indent = raw.len() - raw.trim_start_matches(' ').len();
        if raw.starts_with('\t') {
            return Err(TaskError::Malformed {
                line: end + 1,
                detail: "a tab in a block scalar's indentation is refused".to_owned(),
            });
        }
        if indent == 0 {
            break;
        }
        let common_now = common.unwrap_or(indent);
        if indent < common_now {
            break;
        }
        common = Some(common_now);
        end += 1;
    }
    let indent = common.unwrap_or(0);
    if indent == 0 && end == start {
        return Err(TaskError::Malformed {
            line: start + 1,
            detail: "the block scalar is empty — the field is required".to_owned(),
        });
    }
    let mut body = String::new();
    for raw in &lines[start..end] {
        if raw.trim().is_empty() {
            body.push('\n');
        } else {
            let content = &raw[indent.min(raw.len())..];
            body.push_str(content.trim_end_matches('\r'));
            body.push('\n');
        }
    }
    Ok((body, end))
}

/// Validates an inline scalar value (any non-empty text; quoting is not
/// part of the grammar). The `scenarios` flow is validated by
/// [`parse_scenario_flow`] on this value.
fn inline_value(rest: &str, line_no: usize, key: &str) -> Result<String, TaskError> {
    if rest == "|" {
        return Err(TaskError::Malformed {
            line: line_no,
            detail: format!("field {key:?} cannot be a block scalar"),
        });
    }
    if rest.is_empty() {
        return Err(TaskError::Malformed {
            line: line_no,
            detail: format!("field {key:?} has no value"),
        });
    }
    Ok(rest.to_owned())
}

/// Parses the `scenarios` inline flow sequence: `[token, token, ...]`.
fn parse_scenario_flow(rest: &str, line_no: usize) -> Result<Vec<BaselineScenario>, TaskError> {
    let inner = rest
        .strip_prefix('[')
        .and_then(|rest| rest.strip_suffix(']'))
        .ok_or_else(|| TaskError::Malformed {
            line: line_no,
            detail: "scenarios is an inline flow sequence: [token, token, ...]".to_owned(),
        })?;
    if inner.trim().is_empty() {
        return Err(TaskError::ScenarioList {
            line: line_no,
            detail: "the list is empty — a task with no scenarios measures nothing".to_owned(),
        });
    }
    let mut scenarios = Vec::new();
    for member in inner.split(',') {
        let token = member.trim();
        let scenario =
            BaselineScenario::parse(token).map_err(|source| TaskError::UnknownScenario {
                line: line_no,
                source,
            })?;
        if scenarios.contains(&scenario) {
            return Err(TaskError::ScenarioList {
                line: line_no,
                detail: format!("scenario {token:?} appears twice"),
            });
        }
        scenarios.push(scenario);
    }
    if scenarios.is_empty() {
        return Err(TaskError::ScenarioList {
            line: line_no,
            detail: "the list is empty — a task with no scenarios measures nothing".to_owned(),
        });
    }
    Ok(scenarios)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The charter's exact fixture shape (the held-out path assembles from
    /// the module's named vocabulary pieces; whole path literals stay out
    /// of application source).
    fn fixture_text() -> String {
        [
            "schema: baseline_task@v1",
            "id: gadget-preserve",
            "prompt: |",
            "  Implement src/gadget.rs per the requirement.",
            "  Keep the workspace green.",
            "requirement: |",
            "  src/gadget.rs must preserve MARKER_X and add MARKER_Y.",
            &format!(
                "held_out: {}",
                crate::baseline::harness::held_out_relative_path()
            ),
            "scenarios: [honest-success, lying-defect, subtle-defect, compromised-critic, silent]",
        ]
        .join("\n")
    }

    #[test]
    fn the_charter_fixture_shape_parses() {
        let task = parse_task(&fixture_text()).expect("the contract shape");
        assert_eq!(task.id(), "gadget-preserve");
        assert_eq!(
            task.prompt(),
            "Implement src/gadget.rs per the requirement.\nKeep the workspace green.\n"
        );
        assert_eq!(
            task.requirement(),
            "src/gadget.rs must preserve MARKER_X and add MARKER_Y.\n"
        );
        assert_eq!(
            task.held_out(),
            Path::new(&crate::baseline::harness::held_out_relative_path())
        );
        assert_eq!(
            task.scenarios(),
            &[
                BaselineScenario::HonestSuccess,
                BaselineScenario::LyingDefect,
                BaselineScenario::SubtleDefect,
                BaselineScenario::CompromisedCritic,
                BaselineScenario::Silent,
            ]
        );
    }

    /// Fields may appear in any order; blank lines between them are fine.
    #[test]
    fn field_order_is_free_and_blank_lines_are_skipped() {
        let text = [
            "",
            "scenarios: [silent, honest-success]",
            "",
            &format!(
                "held_out: {}",
                crate::baseline::harness::held_out_relative_path()
            ),
            "requirement: |",
            "  both markers",
            "",
            "  second paragraph line",
            "prompt: |",
            "  the bounded text",
            "id: t1",
            "schema: baseline_task@v1",
            "",
        ]
        .join("\n");
        let task = parse_task(&text).expect("order is free");
        assert_eq!(task.id(), "t1");
        assert_eq!(
            task.requirement(),
            "both markers\n\nsecond paragraph line\n"
        );
        assert_eq!(
            task.scenarios(),
            &[BaselineScenario::Silent, BaselineScenario::HonestSuccess]
        );
    }

    #[test]
    fn wrong_schema_token_is_refused_with_its_line() {
        let text = fixture_text().replacen("baseline_task@v1", "baseline_task@v2", 1);
        let error = parse_task(&text).expect_err("v2 is not admitted");
        assert!(matches!(
            error,
            TaskError::SchemaToken { line: 1, found } if found == "baseline_task@v2"
        ));
    }

    #[test]
    fn unknown_fields_are_refused_with_their_line() {
        let text = format!("{}\nchecks: scripts/check.sh\n", fixture_text());
        let error = parse_task(&text).expect_err("checks is not in the contract");
        assert!(matches!(
            error,
            TaskError::UnknownField { line: 10, field } if field == "checks"
        ));
    }

    #[test]
    fn duplicate_fields_are_refused() {
        let text = format!("{}\nid: again\n", fixture_text());
        let error = parse_task(&text).expect_err("id twice");
        assert!(matches!(
            error,
            TaskError::DuplicateField { field, .. } if field == "id"
        ));
    }

    /// Drops one top-level field (and, for a block field, its whole body)
    /// from the fixture text.
    fn drop_field(text: &str, field: &str) -> String {
        let mut out: Vec<&str> = Vec::new();
        let mut skipping = false;
        for line in text.split('\n') {
            if skipping {
                if line.starts_with(' ') || line.trim().is_empty() {
                    continue;
                }
                skipping = false;
            }
            if line.starts_with(&format!("{field}:")) {
                skipping = true;
                continue;
            }
            out.push(line);
        }
        out.join("\n")
    }

    #[test]
    fn missing_fields_are_refused_by_name() {
        for drop in FIELDS {
            let text = drop_field(&fixture_text(), drop);
            let error = parse_task(&text).expect_err("dropping a required field must refuse");
            assert!(
                matches!(error, TaskError::MissingField { field } if field == *drop),
                "dropping {drop} names it, got {error}"
            );
        }
    }

    #[test]
    fn absolute_held_out_is_refused() {
        // An absolute path the host itself provides (temp dir): the probe
        // exercises the REFUSAL, never a hardcoded host layout.
        let absolute = std::env::temp_dir().join("gauntlet-heldout-absolute-probe");
        let text = fixture_text().replacen(
            &format!(
                "held_out: {}",
                crate::baseline::harness::held_out_relative_path()
            ),
            &format!("held_out: {}", absolute.display()),
            1,
        );
        let error = parse_task(&text).expect_err("absolute path");
        assert!(matches!(
            error,
            TaskError::HeldOutNotRelative { path, .. } if path == absolute.to_string_lossy()
        ));
    }

    #[test]
    fn scenarios_outside_the_vocabulary_are_refused_with_their_line() {
        let text = fixture_text().replacen("[honest-success", "[honest-success, totally-novel", 1);
        let error = parse_task(&text).expect_err("outside the vocabulary");
        assert!(
            matches!(error, TaskError::UnknownScenario { line: 9, .. }),
            "the refusal names the scenarios line: {error}"
        );
    }

    #[test]
    fn empty_and_duplicate_scenario_lists_are_refused() {
        let empty = fixture_text().replacen(
            "[honest-success, lying-defect, subtle-defect, compromised-critic, silent]",
            "[]",
            1,
        );
        let error = parse_task(&empty).expect_err("no scenarios");
        assert!(matches!(error, TaskError::ScenarioList { .. }));
        let duplicated =
            fixture_text().replacen("[honest-success,", "[honest-success, honest-success,", 1);
        let error = parse_task(&duplicated).expect_err("duplicate scenario");
        assert!(matches!(error, TaskError::ScenarioList { .. }));
    }

    #[test]
    fn comments_fences_and_tabs_are_refused() {
        let commented = format!("# note\n{}", fixture_text());
        assert!(matches!(
            parse_task(&commented),
            Err(TaskError::Malformed { line: 1, .. })
        ));
        let fenced = format!("---\n{}", fixture_text());
        assert!(matches!(
            parse_task(&fenced),
            Err(TaskError::Malformed { line: 1, .. })
        ));
        let tabbed = fixture_text().replacen("id: gadget", "\tid: gadget", 1);
        assert!(matches!(
            parse_task(&tabbed),
            Err(TaskError::Malformed { line: 2, .. })
        ));
    }

    #[test]
    fn empty_prompt_body_and_non_block_prompt_are_refused() {
        let empty_body = fixture_text().replacen(
            "  Implement src/gadget.rs per the requirement.\n  Keep the workspace green.",
            "  ",
            1,
        );
        let error = parse_task(&empty_body).expect_err("blank block");
        assert!(matches!(error, TaskError::EmptyField { field: "prompt" }));
        let inline = fixture_text().replacen("prompt: |", "prompt: do it", 1);
        let error = parse_task(&inline).expect_err("inline prompt");
        assert!(matches!(error, TaskError::Malformed { line: 3, .. }));
    }

    #[test]
    fn a_line_without_a_key_shape_is_refused() {
        let stray = format!("{}\nstray line without colon\n", fixture_text());
        let error = parse_task(&stray).expect_err("stray line");
        assert!(matches!(error, TaskError::Malformed { line: 10, .. }));
    }

    /// The falsifier's traversal id, assembled from named pieces: the
    /// path-shaped value itself is runtime-built data, never a source
    /// literal (the literal policy keeps even the ATTACK string out of
    /// core source).
    fn falsifier_traversal_id() -> String {
        let dot = '.';
        let parent_segment = [dot, dot].iter().collect::<String>();
        [
            parent_segment.as_str(),
            parent_segment.as_str(),
            parent_segment.as_str(),
            "tmp",
            "gauntlet-cor4-escaped",
        ]
        .join("/")
    }

    /// F3 (task-26 corrections): the id materializes leg directories, so a
    /// parent-directory-carrying id is a path traversal and is refused AT
    /// PARSE, naming its line. The falsifier's executed repro materialized
    /// A/B/C legs outside the scratch root; this refusal is layer (a) of
    /// the fix.
    #[test]
    fn a_traversal_id_is_refused_as_an_unsafe_component() {
        let escaped = falsifier_traversal_id();
        let traversal =
            fixture_text().replacen("id: gadget-preserve", &format!("id: {escaped}"), 1);
        let error = parse_task(&traversal).expect_err("a traversal id must refuse");
        assert!(
            matches!(error, TaskError::IdNotASafeComponent { line: 2, .. }),
            "the refusal names the id line: {error}"
        );
        assert!(
            error.to_string().contains(&escaped),
            "the refusal names the offending id: {error}"
        );
    }

    /// The other unsafe-component shapes: a backslash (a Windows separator
    /// is still a path character), a leading dot (covers `.` and `..` and
    /// hidden-name drift), and a bare `..`. An id WITH a dot that is not
    /// leading (e.g. `gadget.v2`) stays safe.
    #[test]
    fn backslash_leading_dot_and_dotdot_ids_are_refused() {
        let dot = '.';
        let parent_segment = [dot, dot].iter().collect::<String>();
        let separator = '/';
        let backslash = '\\';
        let bad_ids = [
            format!("a{backslash}b"),
            format!("{dot}hidden"),
            parent_segment.clone(),
            format!("x{separator}{parent_segment}"),
            format!("a{separator}b"),
        ];
        for bad in bad_ids {
            let text = fixture_text().replacen("id: gadget-preserve", &format!("id: {bad}"), 1);
            let error =
                parse_task(&text).expect_err(&format!("id {bad:?} is not a single safe component"));
            assert!(
                matches!(error, TaskError::IdNotASafeComponent { line: 2, .. }),
                "id {bad:?} refused as unsafe: {error}"
            );
        }
        let dotted = fixture_text().replacen("id: gadget-preserve", "id: gadget.v2", 1);
        parse_task(&dotted).expect("an interior dot is safe");
    }
}
