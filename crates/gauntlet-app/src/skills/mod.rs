//! Skill lint — plan Task 19 Slice B, Annex D.4's closing rule ("skills
//! describe procedure, never decide settlement/authority/budget") and spec
//! §8.1 ("secrets by indirection only... never logs the value").
//!
//! `.agents/skills/` holds two very different kinds of content: this
//! repository's own Gauntlet-authored skills (companions ported from
//! upstream, plus the repo-owned `gauntlet-run` and `wayfinder-gauntlet`),
//! and untracked local tool directories (`jolli*/`) that some other agent
//! runtime drops in the same tree. [`REPO_OWNED_SKILLS`] is the explicit,
//! closed list of the former — [`lint_installed_skills`] reads ONLY those
//! named subdirectories, never a directory glob over `.agents/skills/`, so
//! a foreign local skill can never accidentally pass or fail this lint
//! (task19 `IN-FLIGHT.md` decision 2).
//!
//! [`lint_skill_text`] validates one `SKILL.md` document against a small,
//! closed schema (task19 `IN-FLIGHT.md` decision 3):
//!
//! 1. front matter parses under the shared grammar
//!    ([`crate::config::gauntlet_md::parse`] — reused, not reimplemented,
//!    the same precedent as [`crate::roles::definition`]);
//! 2. exactly `name` and `description` are required, non-empty strings;
//!    `name` MUST equal the skill's own directory name;
//! 3. the only recognized optional key is `disable-model-invocation`
//!    (bool); any other front-matter key is deny-by-default rejected —
//!    the same lesson task18 COR2 paid for: an allowlist that silently
//!    under-rejects is worse than an explicit closed set;
//! 4. the body (everything after the closing fence) contains no control
//!    logic (a closed, explicit denylist — see [`CONTROL_LOGIC_IDENTIFIERS`]
//!    / [`CONTROL_LOGIC_PHRASES`]), no absolute path, and no secret-shaped
//!    literal.
//!
//! ## Hyphenated front-matter keys
//!
//! The external `SKILL.md` convention this repository's skills follow
//! spells its one optional key `disable-model-invocation` (a hyphen), as
//! `wayfinder-gauntlet/SKILL.md` already does. [`crate::config::gauntlet_md`]'s
//! shared grammar deliberately restricts a key to a bare identifier
//! (`[A-Za-z_][A-Za-z0-9_]*`, no hyphen) — GAUNTLET.md and role files are
//! snake_case only, and relaxing that grammar to admit hyphens for every
//! caller is outside this task's scope and would weaken a rule those other
//! two call sites rely on staying strict. Rather than fork the grammar,
//! [`normalize_front_matter_keys`] performs a narrow, length-preserving,
//! line-local substitution — a hyphen inside a would-be key token becomes
//! an underscore — over ONLY the fenced front-matter region, before
//! handing the text to the real parser. Every other character, every
//! value, and every line outside the fence passes through byte-identical,
//! so the line/column numbers `gauntlet_md` reports stay accurate. This is
//! a spelling bridge, not a second grammar: structure, quoting, nesting,
//! and value parsing are still 100% `gauntlet_md::parse`.
//!
//! ## Never echo a secret
//!
//! [`SkillLintError::SecretLikeValueDetected`] carries only the line
//! number where a secret-shaped literal was found — never the matched
//! text (spec §8.1: a value that looks like a credential must never reach
//! error text, logs, or a diff, redacted or not).
//!
//! ## What this lint is not
//!
//! The control-logic check ([`CONTROL_LOGIC_IDENTIFIERS`] /
//! [`CONTROL_LOGIC_PHRASES`], [`scan_control_logic`]) is a LEXICAL scan
//! over a closed, exact-phrase/identifier list, NOT semantic understanding
//! of the skill body. A settlement, authority, or budget decision phrased
//! in yet another way than the pinned identifiers and phrases can still
//! evade it entirely — this is a disclosed, accepted residual, not an
//! oversight (task19 `.agents/reports/task19/critic-design.md` RES-01
//! traces three concrete sentences that evaded the pre-COR1 list; COR1
//! closed exactly those three, plus their natural singular/plural
//! variants — not the general case). A clean `skills::lint` run is
//! evidence that the shipped content does not match a KNOWN decisive
//! pattern; it is never proof the text contains no control logic in any
//! phrasing. Human/critic review remains the backstop for skill content,
//! the same posture `crates/gauntlet-arch-tests/src/single_writer.rs`'s
//! own "THREAT MODEL — what this rule is not" section documents for a
//! sibling lexical scanner in this codebase.
//!
//! task19 COR2 (`.agents/reports/task19/falsifier-round1.md` FALS-01 Part
//! B) added one further, narrow exemption — to [`CONTROL_LOGIC_PHRASES`]
//! matches only, never to [`CONTROL_LOGIC_IDENTIFIERS`] (those stay exact
//! API names and are always flagged unconditionally). When a phrase match
//! is found on a line, and the substring `"the runtime"` (case-insensitive)
//! appears earlier on that same line, before the match's own start
//! position, the match is treated as sanctioned third-person descriptive
//! prose about what the runtime does (`gauntlet-run`'s own required voice
//! — "the runtime settles a Unit when X", never "settle the unit when X")
//! and is not flagged. Say plainly what this is NOT: it is a per-line
//! TEXTUAL PROXIMITY check, not grammatical subject parsing. It will not
//! catch a deliberately adversarial sentence like "This skill, not the
//! runtime, settles the unit" (that stays part of the disclosed residual
//! above), and it can itself be defeated by a sentence that mentions "the
//! runtime" earlier in the line for an unrelated reason while still making
//! a skill-authored settlement/authority/budget decision later in that
//! same line. Recorded here honestly rather than overclaimed.

use std::fmt;
use std::path::Path;

use crate::config::gauntlet_md::{self, FmEntry, FmValue, FrontMatter};

/// The validated front matter of one skill's `SKILL.md` — the fields this
/// lint actually inspects (task19 `IN-FLIGHT.md` decision 3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillFrontMatter {
    pub name: String,
    pub description: String,
    pub disable_model_invocation: Option<bool>,
}

/// Skill lint failures — typed, each carrying its failure locus (a line
/// number, where the grammar or a scan can give one). Two deliberate
/// completions beyond `IN-FLIGHT.md`'s illustrative enum, both a builder
/// discretion call the design contract explicitly leaves open ("reuse
/// `gauntlet_md::FrontMatter` directly if adding a typed wrapper is not
/// pulling its weight — builder's call"): `FrontMatterGrammar` wraps the
/// shared grammar's own typed error verbatim (its own line/column) instead
/// of collapsing every grammar failure into one unterminated-fence case,
/// and `InvalidFieldKind` / `Io` are needed for a field of the wrong kind
/// (e.g. `name: 12`) and a missing/unreadable file to stay typed rather
/// than degrading to a different, less precise variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillLintError {
    /// No front-matter fence at all, or an empty fenced region — there is
    /// nothing to validate `name`/`description` against.
    MissingFrontMatter,
    /// The front-matter grammar itself rejected the document (tab
    /// indentation, an unterminated fence, a duplicate key, ...) — the
    /// wrapped error carries its own line and column.
    FrontMatterGrammar {
        source: gauntlet_md::GauntletMdError,
    },
    /// A required field (`name` or `description`) is absent.
    MissingField { field: &'static str },
    /// A required field is present but its string value is empty (or
    /// whitespace-only).
    EmptyField { field: &'static str, line: usize },
    /// A field's value is not the kind the schema requires.
    InvalidFieldKind {
        field: &'static str,
        expected: &'static str,
        found: &'static str,
        line: usize,
    },
    /// `name` does not equal the skill's own directory name.
    NameMismatch {
        expected: String,
        found: String,
        line: usize,
    },
    /// A front-matter key outside the closed set `name` / `description` /
    /// `disable-model-invocation` (task19 `IN-FLIGHT.md` decision 3).
    UnknownFrontMatterKey { key: String, line: usize },
    /// The body names the runtime's own control vocabulary: an exact
    /// identifier, or a decisive phrase, from the closed denylist.
    ControlLogicDetected { pattern: String, line: usize },
    /// The body contains an absolute filesystem path.
    AbsolutePathDetected { line: usize },
    /// The body (or the front matter) contains a secret-shaped literal.
    /// The matched text is NEVER carried here (spec §8.1) — the line is
    /// enough to locate it.
    SecretLikeValueDetected { line: usize },
    /// The `SKILL.md` file itself could not be read.
    Io { path: String, message: String },
}

impl fmt::Display for SkillLintError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::MissingFrontMatter => write!(
                f,
                "skill lint: no front matter — a SKILL.md needs name and description"
            ),
            Self::FrontMatterGrammar { source } => write!(f, "skill lint: {source}"),
            Self::MissingField { field } => {
                write!(f, "skill lint: required field '{field}' is absent")
            }
            Self::EmptyField { field, line } => write!(
                f,
                "skill lint line {line}: field '{field}' is present but empty"
            ),
            Self::InvalidFieldKind {
                field,
                expected,
                found,
                line,
            } => write!(
                f,
                "skill lint line {line}: field '{field}' expects {expected} but carries {found}"
            ),
            Self::NameMismatch {
                expected,
                found,
                line,
            } => write!(
                f,
                "skill lint line {line}: front matter name '{found}' does not equal the skill's \
                 own directory name '{expected}'"
            ),
            Self::UnknownFrontMatterKey { key, line } => write!(
                f,
                "skill lint line {line}: unknown front-matter key '{key}' — the schema is \
                 closed to name, description, and disable-model-invocation"
            ),
            Self::ControlLogicDetected { pattern, line } => write!(
                f,
                "skill lint line {line}: body names control-logic vocabulary ('{pattern}') — a \
                 skill describes procedure, it never decides settlement, authority, or budget"
            ),
            Self::AbsolutePathDetected { line } => write!(
                f,
                "skill lint line {line}: body contains an absolute filesystem path"
            ),
            Self::SecretLikeValueDetected { line } => write!(
                f,
                "skill lint line {line}: a secret-shaped literal — the matched value is never \
                 printed"
            ),
            Self::Io { path, message } => write!(f, "skill lint: reading '{path}': {message}"),
        }
    }
}

impl std::error::Error for SkillLintError {}

/// The explicit, closed list of repo-owned skill directories under
/// `.agents/skills/` (task19 `IN-FLIGHT.md` decision 2). `.agents/skills/`
/// also holds untracked, non-Gauntlet local tool directories (`jolli*/`);
/// [`lint_installed_skills`] never globs the directory, so those are never
/// scanned or asserted about.
pub const REPO_OWNED_SKILLS: &[&str] = &[
    "wayfinder-gauntlet",
    "grilling",
    "domain-formation",
    "research",
    "prototype",
    "gauntlet-run",
    "domainforge-sea",
    "neatcode",
];

/// The control-logic denylist's exact identifiers — anchored on the
/// runtime's own control vocabulary (spec's `SettlementService`,
/// `settle_unit`, `AuthorityGate`, `GateAdmission`, `BudgetMeter`,
/// `Outcome::Pass`, `Outcome::Fail`) so it cannot false-positive on
/// ordinary procedural prose. Matched case-insensitively.
const CONTROL_LOGIC_IDENTIFIERS: &[&str] = &[
    "SettlementService",
    "settle_unit",
    "AuthorityGate",
    "GateAdmission",
    "BudgetMeter",
    "Outcome::Pass",
    "Outcome::Fail",
];

/// The control-logic denylist's decisive multi-word phrases. Deliberately
/// NOT a bare word like "settle" or "authority" alone — that would
/// false-positive on ordinary procedural prose describing what the
/// runtime does (e.g. "the runtime settles a Unit when..."). Matched
/// case-insensitively.
///
/// task19 COR1 (RES-01) widened this list with seven additional phrases to
/// close three concrete sentences the critic traced as evading the
/// original five (`.agents/reports/task19/critic-design.md` RES-01): a
/// skill claiming to settle a Unit itself ("settles the unit", definite
/// article — distinct from the legal-prose regression fixture's "settles
/// **a** Unit"), a skill instructing budget authorization ("authorize
/// additional budget" / "authorize the budget" / "authorizes additional
/// budget" — distinct from the existing "approve the budget" / "deny the
/// budget"), and a skill instructing authority elevation ("elevate the
/// unit's authority" / "elevates the unit's authority" / "grants the
/// unit authority"). See "## What this lint is not" below: this widening
/// closes the three reproduced sentences, not the general case.
const CONTROL_LOGIC_PHRASES: &[&str] = &[
    "mark the unit as settled",
    "grant authority",
    "approve the budget",
    "deny the budget",
    "the unit is now settled",
    "settles the unit",
    "authorize additional budget",
    "authorize the budget",
    "authorizes additional budget",
    "elevate the unit's authority",
    "elevates the unit's authority",
    "grants the unit authority",
];

/// Vendor token prefixes the secret-like scan recognizes as a hit
/// anywhere on a line.
const SECRET_PREFIXES: &[&str] = &["sk-", "ghp_", "AKIA", "xoxb-", "xoxp-", "xoxa-"];

/// Assignment labels the secret-like scan checks: `<label>:` whose value
/// does not start with `$` is a hit, in front matter or body alike.
const SECRET_LABELS: &[&str] = &["key", "token", "password", "secret"];

/// Parses and validates one `SKILL.md` document's text against the schema
/// in the module documentation. `dir_name` is the skill's own directory
/// name — `name` in the front matter must equal it exactly.
pub fn lint_skill_text(dir_name: &str, text: &str) -> Result<SkillFrontMatter, SkillLintError> {
    let normalized = normalize_front_matter_keys(text);
    let front = gauntlet_md::parse(&normalized)
        .map_err(|source| SkillLintError::FrontMatterGrammar { source })?;
    if front.is_empty() {
        return Err(SkillLintError::MissingFrontMatter);
    }
    let front_matter = validate_schema(dir_name, &front)?;

    let (body, body_start_line) = body_with_start_line(text);
    scan_control_logic(&body, body_start_line)?;
    scan_secrets(text)?;
    scan_absolute_paths(text)?;

    Ok(front_matter)
}

/// Reads exactly the [`REPO_OWNED_SKILLS`] subdirectories' `SKILL.md`
/// under `skills_root` and lints each one — never a directory glob (module
/// documentation).
pub fn lint_installed_skills(
    skills_root: &Path,
) -> Vec<(&'static str, Result<SkillFrontMatter, SkillLintError>)> {
    REPO_OWNED_SKILLS
        .iter()
        .map(|&name| {
            let path = skills_root.join(name).join("SKILL.md");
            let result = match std::fs::read_to_string(&path) {
                Ok(text) => lint_skill_text(name, &text),
                Err(err) => Err(SkillLintError::Io {
                    path: path.display().to_string(),
                    message: err.to_string(),
                }),
            };
            (name, result)
        })
        .collect()
}

/// Walks the parsed front matter's top-level entries against the closed
/// schema (module documentation, task19 `IN-FLIGHT.md` decision 3).
fn validate_schema(
    dir_name: &str,
    front: &FrontMatter,
) -> Result<SkillFrontMatter, SkillLintError> {
    let mut name: Option<(String, usize)> = None;
    let mut description: Option<String> = None;
    let mut disable_model_invocation: Option<bool> = None;

    for entry in &front.entries {
        match entry.key.as_str() {
            "name" => {
                let (value, line) = expect_non_empty_str(entry, "name")?;
                name = Some((value, line));
            }
            "description" => {
                let (value, _line) = expect_non_empty_str(entry, "description")?;
                description = Some(value);
            }
            "disable_model_invocation" => match &entry.value {
                FmValue::Bool(flag) => disable_model_invocation = Some(*flag),
                other => {
                    return Err(SkillLintError::InvalidFieldKind {
                        field: "disable-model-invocation",
                        expected: "a boolean",
                        found: kind_word(other),
                        line: entry.line,
                    });
                }
            },
            other => {
                return Err(SkillLintError::UnknownFrontMatterKey {
                    key: other.to_owned(),
                    line: entry.line,
                });
            }
        }
    }

    let (name, name_line) = name.ok_or(SkillLintError::MissingField { field: "name" })?;
    let description = description.ok_or(SkillLintError::MissingField {
        field: "description",
    })?;

    if name != dir_name {
        return Err(SkillLintError::NameMismatch {
            expected: dir_name.to_owned(),
            found: name,
            line: name_line,
        });
    }

    Ok(SkillFrontMatter {
        name,
        description,
        disable_model_invocation,
    })
}

fn expect_non_empty_str(
    entry: &FmEntry,
    field: &'static str,
) -> Result<(String, usize), SkillLintError> {
    match &entry.value {
        FmValue::Str(text) => {
            if text.trim().is_empty() {
                Err(SkillLintError::EmptyField {
                    field,
                    line: entry.line,
                })
            } else {
                Ok((text.clone(), entry.line))
            }
        }
        other => Err(SkillLintError::InvalidFieldKind {
            field,
            expected: "a string",
            found: kind_word(other),
            line: entry.line,
        }),
    }
}

fn kind_word(value: &FmValue) -> &'static str {
    match value {
        FmValue::Str(_) => "a string",
        FmValue::Int(_) => "a number",
        FmValue::Bool(_) => "a boolean",
        FmValue::Seq(_) => "a list",
        FmValue::Map(_) => "a mapping",
    }
}

/// Bridges the external `SKILL.md` hyphenated-key convention to
/// `gauntlet_md`'s bare-identifier key grammar — see the module
/// documentation's "Hyphenated front-matter keys" section. Operates only
/// on the fenced front-matter region, located the same way
/// `gauntlet_md::parse` locates it, so a document with no fence (or an
/// unterminated one) passes through unchanged and `gauntlet_md::parse`
/// reports the same failure it always would.
fn normalize_front_matter_keys(text: &str) -> String {
    let lines: Vec<&str> = text.lines().collect();
    let mut open = None;
    for (index, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if line.trim() == "---" {
            open = Some(index);
        }
        break;
    }
    let Some(open) = open else {
        return text.to_owned();
    };
    let mut close = None;
    for (index, line) in lines.iter().enumerate().skip(open + 1) {
        let trimmed = line.trim();
        if trimmed == "---" || trimmed.starts_with("---") {
            close = Some(index);
            break;
        }
    }
    let Some(close) = close else {
        return text.to_owned();
    };
    let mut out = String::with_capacity(text.len());
    for (index, line) in lines.iter().enumerate() {
        if index > 0 {
            out.push('\n');
        }
        if index > open && index < close {
            out.push_str(&normalize_key_line(line));
        } else {
            out.push_str(line);
        }
    }
    if text.ends_with('\n') {
        out.push('\n');
    }
    out
}

/// Replaces hyphens with underscores in the leading key token of one
/// front-matter line (`<indent><key-ish-token>:`), leaving everything
/// else on the line — value text, comments, nested indentation — byte
/// identical. A no-op on any line that isn't shaped like a key followed
/// immediately by `:` (blank lines, comments, value continuations).
fn normalize_key_line(line: &str) -> String {
    let indent_len = line.len() - line.trim_start().len();
    let (indent, rest) = line.split_at(indent_len);
    let key_end = rest
        .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_' || c == '-'))
        .unwrap_or(rest.len());
    if key_end == 0 || !rest[..key_end].contains('-') {
        return line.to_owned();
    }
    if rest[key_end..].starts_with(':') {
        let normalized_key = rest[..key_end].replace('-', "_");
        format!("{indent}{normalized_key}{}", &rest[key_end..])
    } else {
        line.to_owned()
    }
}

/// Locates the prose body and the 1-based file line number of its first
/// line, mirroring `gauntlet_md::parse`'s own fence detection so the body
/// split can never drift from the front-matter parse of the same
/// document. A document with no fence is entirely body, starting at line
/// 1 — the same "no leading fence means no front matter" reading
/// `gauntlet_md::parse` documents.
fn body_with_start_line(text: &str) -> (String, usize) {
    let lines: Vec<&str> = text.lines().collect();
    let mut open = None;
    for (index, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if line.trim() == "---" {
            open = Some(index);
        }
        break;
    }
    let Some(open) = open else {
        return (text.to_owned(), 1);
    };
    let mut close = None;
    for (index, line) in lines.iter().enumerate().skip(open + 1) {
        let trimmed = line.trim();
        if trimmed == "---" || trimmed.starts_with("---") {
            close = Some(index);
            break;
        }
    }
    let Some(close) = close else {
        return (String::new(), lines.len() + 1);
    };
    let body_start = close + 1;
    (lines[body_start..].join("\n"), body_start + 1)
}

/// Scans the body for the closed control-logic denylist (module
/// documentation, [`CONTROL_LOGIC_IDENTIFIERS`] / [`CONTROL_LOGIC_PHRASES`]),
/// case-insensitive substring matching, line by line.
///
/// [`CONTROL_LOGIC_IDENTIFIERS`] matches are always flagged unconditionally
/// — exact Rust API names, a narrow true-positive signal. A
/// [`CONTROL_LOGIC_PHRASES`] match carries the task19 COR2 runtime-
/// exemption instead (module documentation "## What this lint is not"):
/// when `"the runtime"` (case-insensitive) appears earlier on the same
/// line, before the phrase match's own start byte, the match is sanctioned
/// descriptive prose and is not flagged; scanning continues to the next
/// pattern and the next line as normal.
fn scan_control_logic(body: &str, body_start_line: usize) -> Result<(), SkillLintError> {
    const RUNTIME_MENTION: &str = "the runtime";

    for (offset, line) in body.lines().enumerate() {
        let line_no = body_start_line + offset;
        let lowered = line.to_lowercase();

        for pattern in CONTROL_LOGIC_IDENTIFIERS {
            if lowered.contains(pattern.to_lowercase().as_str()) {
                return Err(SkillLintError::ControlLogicDetected {
                    pattern: (*pattern).to_owned(),
                    line: line_no,
                });
            }
        }

        for pattern in CONTROL_LOGIC_PHRASES {
            let lowered_pattern = pattern.to_lowercase();
            let Some(match_start) = lowered.find(lowered_pattern.as_str()) else {
                continue;
            };
            let runtime_exempt = lowered[..match_start].contains(RUNTIME_MENTION);
            if !runtime_exempt {
                return Err(SkillLintError::ControlLogicDetected {
                    pattern: (*pattern).to_owned(),
                    line: line_no,
                });
            }
        }
    }
    Ok(())
}

/// Scans the whole document (front matter and body alike — a secret is no
/// less a secret inside front matter) for a secret-shaped literal.
fn scan_secrets(text: &str) -> Result<(), SkillLintError> {
    for (offset, line) in text.lines().enumerate() {
        if line_has_secret(line) {
            return Err(SkillLintError::SecretLikeValueDetected { line: offset + 1 });
        }
    }
    Ok(())
}

fn line_has_secret(line: &str) -> bool {
    if line.contains("-----BEGIN") {
        return true;
    }
    if SECRET_PREFIXES.iter().any(|prefix| line.contains(prefix)) {
        return true;
    }
    SECRET_LABELS
        .iter()
        .any(|label| label_assignment_is_secret(line, label))
}

/// True when `line` carries a `<label>:` assignment (case-insensitive,
/// word-boundary anchored so "monkey:" never matches the label "key")
/// whose value does not start with `$` — front matter or body alike (the
/// same secret-indirection rule spec §8.1 states for role files).
fn label_assignment_is_secret(line: &str, label: &str) -> bool {
    let lowered = line.to_lowercase();
    let bytes = lowered.as_bytes();
    let mut search_from = 0usize;
    while let Some(relative) = lowered[search_from..].find(label) {
        let start = search_from + relative;
        let end = start + label.len();
        // task19 COR2 (FALS-02): only an alphanumeric character immediately
        // before the label counts as a boundary violation. `_` no longer
        // blocks a match, so a compound label convention (`api_key:`,
        // `secret_key:`, `auth_token:`, `client_secret:`) now correctly
        // matches its `SECRET_LABELS` label — `"monkey:"` still correctly
        // fails to match label `"key"` (the byte before is `n`, alphanumeric).
        let left_boundary_ok = start == 0 || !bytes[start - 1].is_ascii_alphanumeric();
        if left_boundary_ok && lowered[end..].starts_with(':') {
            let value = line[end + 1..].trim_start();
            if !value.is_empty() && !value.starts_with('$') {
                return true;
            }
        }
        search_from = end;
    }
    false
}

/// Scans the whole document for an absolute filesystem path: a
/// whitespace/bracket-delimited token starting with `/` (never a URL
/// token, since a scheme-prefixed token — letters, then a colon, then two
/// slashes — starts with the scheme, not `/`), or a Windows-style
/// `[A-Za-z]:\` drive path. Relative paths like the local reference
/// directory or `crates/gauntlet-app/...` never match — they don't start
/// with `/`.
fn scan_absolute_paths(text: &str) -> Result<(), SkillLintError> {
    for (offset, line) in text.lines().enumerate() {
        if line_has_absolute_path(line) {
            return Err(SkillLintError::AbsolutePathDetected { line: offset + 1 });
        }
    }
    Ok(())
}

fn line_has_absolute_path(line: &str) -> bool {
    if windows_drive_path(line) {
        return true;
    }
    line.split(|c: char| {
        c.is_whitespace() || matches!(c, '(' | ')' | '[' | ']' | '<' | '>' | '"' | '\'' | '`')
    })
    .any(|token| token.starts_with('/') && token.len() > 1)
}

/// True when `line` contains a Windows-style drive path: an ASCII letter
/// immediately followed by `:\`.
fn windows_drive_path(line: &str) -> bool {
    let bytes = line.as_bytes();
    for i in 0..bytes.len() {
        if bytes[i].is_ascii_alphabetic()
            && bytes.get(i + 1) == Some(&b':')
            && bytes.get(i + 2) == Some(&b'\\')
        {
            return true;
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;

    const VALID_FRONT_MATTER: &str = "---\nname: sample-skill\ndescription: A sample skill for \
        unit tests.\ndisable-model-invocation: true\n---\n";

    fn doc(body: &str) -> String {
        format!("{VALID_FRONT_MATTER}{body}")
    }

    #[test]
    fn a_clean_skill_document_lints() {
        let front = lint_skill_text("sample-skill", &doc("Ordinary procedural prose.\n"))
            .expect("a well-formed skill document lints clean");
        assert_eq!(front.name, "sample-skill");
        assert_eq!(front.description, "A sample skill for unit tests.");
        assert_eq!(front.disable_model_invocation, Some(true));
    }

    #[test]
    fn a_document_without_disable_model_invocation_still_lints() {
        let text = "---\nname: sample-skill\ndescription: A sample skill.\n---\nBody text.\n";
        let front = lint_skill_text("sample-skill", text).expect("the optional key may be absent");
        assert_eq!(front.disable_model_invocation, None);
    }

    #[test]
    fn no_front_matter_fence_is_refused() {
        let error = lint_skill_text("sample-skill", "# Just a title\n\nSome prose.\n")
            .expect_err("a document with no fence has nothing to validate");
        assert!(matches!(error, SkillLintError::MissingFrontMatter));
    }

    #[test]
    fn a_name_field_mismatch_is_refused() {
        let error = lint_skill_text("other-dir", &doc("Body.\n"))
            .expect_err("name must equal the directory name");
        assert!(matches!(
            error,
            SkillLintError::NameMismatch { expected, found, .. }
                if expected == "other-dir" && found == "sample-skill"
        ));
    }

    #[test]
    fn a_missing_required_field_is_refused() {
        let text = "---\nname: sample-skill\n---\nBody.\n";
        let error = lint_skill_text("sample-skill", text).expect_err("description is required");
        assert!(matches!(
            error,
            SkillLintError::MissingField {
                field: "description"
            }
        ));
    }

    #[test]
    fn an_unknown_front_matter_key_is_refused() {
        let text =
            "---\nname: sample-skill\ndescription: A sample skill.\nlicense: MIT\n---\nBody.\n";
        let error = lint_skill_text("sample-skill", text)
            .expect_err("the schema is closed to name, description, disable-model-invocation");
        assert!(matches!(
            error,
            SkillLintError::UnknownFrontMatterKey { key, .. } if key == "license"
        ));
    }

    #[test]
    fn a_bare_control_word_in_ordinary_prose_is_not_over_rejected() {
        // "settle" and "authority" alone must never trip the denylist —
        // only the exact identifiers and decisive multi-word phrases do
        // (IN-FLIGHT.md Slice B).
        let text = doc(
            "The runtime settles a Unit when its evidence is verified, under its \
                         own authority model.\n",
        );
        lint_skill_text("sample-skill", &text)
            .expect("ordinary procedural prose naming 'settle'/'authority' is not control logic");
    }

    #[test]
    fn an_exact_control_identifier_is_refused() {
        let text = doc("This calls SettlementService::settle_unit directly.\n");
        let error = lint_skill_text("sample-skill", &text)
            .expect_err("an exact control-logic identifier is refused");
        assert!(matches!(error, SkillLintError::ControlLogicDetected { .. }));
    }

    #[test]
    fn a_decisive_control_phrase_is_refused() {
        let text = doc("The procedure says to grant authority once review completes.\n");
        let error = lint_skill_text("sample-skill", &text)
            .expect_err("a decisive control-logic phrase is refused");
        assert!(matches!(error, SkillLintError::ControlLogicDetected { .. }));
    }

    // task19 COR1 (RES-01): the three critic-traced sentences that evaded
    // the pre-COR1 CONTROL_LOGIC_PHRASES list must now be caught. Each
    // sentence below is copied verbatim from
    // `.agents/reports/task19/critic-design.md`'s RES-01 reproduction.

    #[test]
    fn a_skill_claiming_to_settle_the_unit_itself_is_refused() {
        let text = doc("This skill settles the Unit once its evidence passes.\n");
        let error = lint_skill_text("sample-skill", &text)
            .expect_err("a skill claiming to settle the Unit itself is refused");
        assert!(matches!(error, SkillLintError::ControlLogicDetected { .. }));
    }

    #[test]
    fn a_skill_instructing_budget_authorization_is_refused() {
        let text = doc("The agent should authorize additional budget for this unit.\n");
        let error = lint_skill_text("sample-skill", &text)
            .expect_err("a skill instructing budget authorization is refused");
        assert!(matches!(error, SkillLintError::ControlLogicDetected { .. }));
    }

    #[test]
    fn a_skill_instructing_authority_elevation_is_refused() {
        let text = doc("Elevate the Unit's authority to cross_cutting before dispatching.\n");
        let error = lint_skill_text("sample-skill", &text)
            .expect_err("a skill instructing authority elevation is refused");
        assert!(matches!(error, SkillLintError::ControlLogicDetected { .. }));
    }

    // task19 COR2 (FALS-01 Part B): CONTROL_LOGIC_PHRASES is a subject-blind
    // substring match, so COR1's widening also caught legitimate,
    // exactly-precedented third-person descriptive prose about what *the
    // runtime* does (the module's own sanctioned voice — see
    // `a_bare_control_word_in_ordinary_prose_is_not_over_rejected` above).
    // Each sentence below is copied verbatim from
    // `.agents/reports/task19/falsifier-round1.md`'s FALS-01 Part B
    // reproduction and must now lint clean under the runtime-exemption.

    #[test]
    fn a_runtime_descriptive_sentence_settling_the_unit_is_not_over_rejected() {
        let text = doc(
            "The runtime settles the unit once every blocking residual is resolved and \
             evidence is verified.\n",
        );
        lint_skill_text("sample-skill", &text).expect(
            "descriptive prose about the runtime settling the unit is sanctioned, not \
             control logic",
        );
    }

    #[test]
    fn a_runtime_descriptive_sentence_authorizing_budget_is_not_over_rejected() {
        let text = doc(
            "When the recovery reserve is untouched, the runtime authorizes additional \
             budget for the next round automatically.\n",
        );
        lint_skill_text("sample-skill", &text).expect(
            "descriptive prose about the runtime authorizing budget is sanctioned, not \
             control logic",
        );
    }

    #[test]
    fn a_runtime_descriptive_sentence_granting_authority_is_not_over_rejected() {
        let text = doc(
            "During dispatch the runtime grants the unit authority consistent with its \
             declared blast radius.\n",
        );
        lint_skill_text("sample-skill", &text).expect(
            "descriptive prose about the runtime granting authority is sanctioned, not \
             control logic",
        );
    }

    #[test]
    fn a_skill_claiming_to_settle_the_unit_without_a_runtime_mention_is_still_refused() {
        // The key discriminator (falsifier-round1.md COR2 requirement):
        // the runtime-exemption must not regress COR1's closure. This is
        // the same sentence as
        // `a_skill_claiming_to_settle_the_unit_itself_is_refused` above,
        // re-asserted here explicitly as proof the fix is the mechanism
        // (absence of "the runtime" before the match), not a widening back
        // to COR1's pre-fix behavior.
        let text = doc("This skill settles the Unit once its evidence passes.\n");
        let error = lint_skill_text("sample-skill", &text).expect_err(
            "a violation sentence with no preceding \"the runtime\" mention on the line \
             still correctly rejects",
        );
        assert!(matches!(error, SkillLintError::ControlLogicDetected { .. }));
    }

    #[test]
    fn an_absolute_path_in_the_body_is_refused() {
        // The boundary suite's own literal scan polices test text like any
        // other source (see `crate::config::gauntlet_md`'s test module for
        // the same note) — this token is deliberately not one of the
        // suite's recognized path-root names, so it stays a clean,
        // synthetic absolute-path fixture rather than a real machine path.
        let text = doc("See /gauntlet-fixture-root/tool for details.\n");
        let error =
            lint_skill_text("sample-skill", &text).expect_err("an absolute path is refused");
        assert!(matches!(error, SkillLintError::AbsolutePathDetected { .. }));
    }

    #[test]
    fn a_relative_path_is_not_over_rejected() {
        let text = doc(
            "See the local reference directory and crates/gauntlet-app/src for \
                         details.\n",
        );
        lint_skill_text("sample-skill", &text).expect("relative paths are never flagged");
    }

    #[test]
    fn a_token_with_no_leading_slash_is_not_flagged_as_an_absolute_path() {
        // A token that merely CONTAINS `/` (a URL's own path segment,
        // among other shapes) is never an absolute path unless the token
        // itself starts with `/` — the same property that lets a URL's
        // scheme-prefixed token through untouched.
        let text = doc("See docs/path/example for details.\n");
        lint_skill_text("sample-skill", &text)
            .expect("a token that doesn't start with / is never flagged");
    }

    #[test]
    fn a_secret_shaped_literal_is_refused_and_never_echoed() {
        let text = doc("token: sk-abcdef0123456789\n");
        let error = lint_skill_text("sample-skill", &text)
            .expect_err("a literal token assignment without $ indirection is refused");
        assert!(matches!(
            error,
            SkillLintError::SecretLikeValueDetected { .. }
        ));
        let rendered = error.to_string();
        assert!(
            !rendered.contains("sk-abcdef0123456789"),
            "the matched secret value must never appear in the error text"
        );
    }

    #[test]
    fn an_indirected_value_is_not_flagged_as_a_secret() {
        let text = doc("model: $GAUNTLET_BUILDER_MODEL\ntoken: $API_TOKEN\n");
        lint_skill_text("sample-skill", &text)
            .expect("a $VAR indirection is never treated as a literal secret");
    }

    #[test]
    fn a_pem_block_marker_is_refused() {
        let text = doc("-----BEGIN PRIVATE KEY-----\n");
        let error =
            lint_skill_text("sample-skill", &text).expect_err("a PEM block marker is refused");
        assert!(matches!(
            error,
            SkillLintError::SecretLikeValueDetected { .. }
        ));
    }

    // task19 COR2 (FALS-02): the left-boundary check used to treat `_` the
    // same as an alphanumeric character, which blocked every compound
    // label convention (`api_key:`, `secret_key:`, `auth_token:`,
    // `client_secret:`) from ever matching its `SECRET_LABELS` label. Each
    // reproduction below is copied verbatim (same label, same value shape)
    // from `.agents/reports/task19/falsifier-round1.md`'s FALS-02
    // reproduction and must now correctly reject.

    #[test]
    fn a_compound_api_key_label_is_refused() {
        let text = doc("api_key: q7f2mZpLX9rT4vN8cKdE3wYh\n");
        let error = lint_skill_text("sample-skill", &text)
            .expect_err("a compound api_key: label must match the key label");
        assert!(matches!(
            error,
            SkillLintError::SecretLikeValueDetected { .. }
        ));
    }

    #[test]
    fn a_compound_secret_key_label_is_refused() {
        let text = doc("secret_key: q7f2mZpLX9rT4vN8cKdE3wYh\n");
        let error = lint_skill_text("sample-skill", &text)
            .expect_err("a compound secret_key: label must match the key label");
        assert!(matches!(
            error,
            SkillLintError::SecretLikeValueDetected { .. }
        ));
    }

    #[test]
    fn a_compound_auth_token_label_is_refused() {
        let text = doc("auth_token: q7f2mZpLX9rT4vN8cKdE3wYh\n");
        let error = lint_skill_text("sample-skill", &text)
            .expect_err("a compound auth_token: label must match the token label");
        assert!(matches!(
            error,
            SkillLintError::SecretLikeValueDetected { .. }
        ));
    }

    #[test]
    fn a_compound_client_secret_label_is_refused() {
        let text = doc("client_secret: q7f2mZpLX9rT4vN8cKdE3wYh\n");
        let error = lint_skill_text("sample-skill", &text)
            .expect_err("a compound client_secret: label must match the secret label");
        assert!(matches!(
            error,
            SkillLintError::SecretLikeValueDetected { .. }
        ));
    }

    #[test]
    fn a_word_merely_ending_in_a_label_is_not_flagged_as_a_secret() {
        // The original word-boundary purpose this check exists for:
        // "monkey:" must never match the label "key" — the character
        // immediately before "key" is "n", alphanumeric, so the boundary
        // is still correctly violated after the COR2 fix.
        let text = doc("monkey: q7f2mZpLX9rT4vN8cKdE3wYh\n");
        lint_skill_text("sample-skill", &text)
            .expect("a word that merely ends with a label spelling is not a label match");
    }

    #[test]
    fn the_wayfinder_gauntlet_front_matter_shape_parses() {
        // The exact shape task18 already committed: hyphenated
        // disable-model-invocation key, which gauntlet_md's bare-identifier
        // grammar cannot parse without this module's key normalization.
        let text = "---\nname: wayfinder-gauntlet\ndescription: Form a Gauntlet reference from a \
                     loose intent.\ndisable-model-invocation: true\n---\nDestination first.\n";
        let front = lint_skill_text("wayfinder-gauntlet", text)
            .expect("the real wayfinder-gauntlet front-matter shape lints clean");
        assert_eq!(front.disable_model_invocation, Some(true));
    }
}
