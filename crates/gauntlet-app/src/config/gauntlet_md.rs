//! GAUNTLET.md front matter — spec §8.2, the constrained grammar (plan Task 5
//! decision D1: std-only, strictness is the feature).
//!
//! Spec §8.2: "Structure: OPTIONAL YAML front matter (runtime config)
//! followed by OPTIONAL H2 sections used as documentation. Front matter MUST
//! decode to a map. Unknown keys are ignored for forward compatibility, but
//! `gauntlet doctor` lists them so typos surface."
//!
//! This is a hand-rolled parser over a DOCUMENTED SUBSET of YAML — the same
//! posture as the zero-dependency JSON reader in `gauntlet-ports`. It is not
//! a YAML implementation, and that is deliberate: the front matter is a
//! configuration contract under this repository's control, and anything
//! outside the grammar is a typed parse error that `gauntlet doctor` check 1
//! reports, never a silent guess. If key handling ever grows per-key
//! branches, the plan's redesign trigger fires: schema-driven
//! deserialization with the schema kept under `docs/schemas/`.
//!
//! # The grammar
//!
//! Accepted:
//!
//! - an optional opening `---` fence; when present, front matter runs to the
//!   first line whose content is exactly `---`; a file without a leading
//!   fence has NO front matter (every line is documentation) and parses as an
//!   empty map;
//! - nested mappings by two-or-more-space indentation (children strictly
//!   deeper than their parent; siblings at one indent);
//! - `key: value` with keys as bare identifiers `[A-Za-z_][A-Za-z0-9_]*`
//!   immediately followed by `:`;
//! - scalar values: bare strings, `"quoted"` / `'quoted` strings, integers,
//!   and the booleans `true` / `false`;
//! - inline flow sequences `[a, b, c]` of scalars, and inline flow mappings
//!   `{k: v, k2: v2}` of scalar values (the `runners.by_role` shape);
//! - `#` comments to end of line (never inside a quoted string), and blank
//!   lines, anywhere in the front matter;
//! - `$VAR` and `${VAR}` tokens inside string values, preserved verbatim —
//!   the parser keeps raw text; indirection resolution is
//!   [`crate::config::resolve`]'s job.
//!
//! Rejected, each a typed error carrying `line:col`:
//!
//! - a leading byte-order mark (U+FEFF) — rejected, never tolerated: a
//!   tolerated BOM hides the opening fence and silently reclassifies the
//!   whole contract as documentation;
//! - tabs anywhere in indentation;
//! - block scalars (`|`, `>`), anchors (`&`), aliases (`*`), tags (`!`);
//! - a second document marker (`--- …`) inside the front matter;
//! - duplicate keys in one mapping, block or flow (ambiguous documents are
//!   not admitted);
//! - any top-level or nested line that is not a mapping entry (a sequence
//!   item, a bare scalar);
//! - unclosed quotes and unclosed flow brackets;
//! - an empty key; a key not immediately followed by `:`;
//! - trailing content after a closed quote or flow;
//! - an indentation that matches no open block (mixed sibling widths);
//! - a deeper-indented line after a scalar-valued key (a scalar has no
//!   children).
//!
//! Values are kept as raw strings plus a tiny typed value enum; conversion
//! against the known-key table happens in [`crate::config::resolve`].

use core::fmt;

/// A parsed scalar inside a flow sequence or mapping.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FmScalar {
    Str(String),
    Int(i64),
    Bool(bool),
}

impl FmScalar {
    /// The raw text of a scalar, as the canonical form serializes it.
    pub fn raw(&self) -> String {
        match self {
            Self::Str(s) => s.clone(),
            Self::Int(n) => n.to_string(),
            Self::Bool(b) => b.to_string(),
        }
    }
}

/// One `key: value` entry, with the physical line it appeared on — unknown
/// keys are reported with their line (§8.2: "doctor lists them so typos
/// surface").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FmEntry {
    pub key: String,
    pub value: FmValue,
    pub line: usize,
}

/// A front-matter value. Maps preserve written order and carry per-entry
/// lines; sequences are flat lists of scalars (the grammar has no block
/// sequences).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FmValue {
    Str(String),
    Int(i64),
    Bool(bool),
    Seq(Vec<FmScalar>),
    Map(Vec<FmEntry>),
}

impl FmValue {
    /// Whether this is the empty map a bare `key:` line produces.
    pub fn as_map(&self) -> Option<&[FmEntry]> {
        match self {
            Self::Map(entries) => Some(entries),
            _ => None,
        }
    }
}

/// The front matter of one GAUNTLET.md: a map plus, per entry, its line.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct FrontMatter {
    pub entries: Vec<FmEntry>,
}

impl FrontMatter {
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

/// Front-matter parse failures — typed, with line and column (1-based, byte
/// columns) of the deciding character.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GauntletMdError {
    TabIndentation {
        line: usize,
        col: usize,
    },
    BlockScalar {
        line: usize,
        col: usize,
    },
    Anchor {
        line: usize,
        col: usize,
    },
    Alias {
        line: usize,
        col: usize,
    },
    Tag {
        line: usize,
        col: usize,
    },
    SecondDocument {
        line: usize,
    },
    DuplicateKey {
        line: usize,
        col: usize,
        key: String,
    },
    NotAMappingEntry {
        line: usize,
        col: usize,
    },
    UnclosedQuote {
        line: usize,
        col: usize,
    },
    UnclosedBracket {
        line: usize,
        col: usize,
    },
    EmptyKey {
        line: usize,
        col: usize,
    },
    ColonExpected {
        line: usize,
        col: usize,
    },
    InvalidKeyCharacter {
        line: usize,
        col: usize,
        found: char,
    },
    TrailingContent {
        line: usize,
        col: usize,
    },
    MixedIndentation {
        line: usize,
        col: usize,
        open_indent: usize,
    },
    BlockAfterScalar {
        line: usize,
        col: usize,
    },
    UnterminatedFrontMatter {
        line: usize,
    },
    /// The document's first byte sequence is a UTF-8 byte-order mark. The
    /// parser REJECTS it rather than tolerating it: U+FEFF is not
    /// whitespace, so a tolerated BOM makes the opening fence unmatchable
    /// and the entire config layer reads as documentation — a silent
    /// precedence inversion. Rejection keeps the failure at the file
    /// position, with the remediation in the message.
    ByteOrderMark {
        line: usize,
        col: usize,
    },
}

impl fmt::Display for GauntletMdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::TabIndentation { line, col } => write!(
                f,
                "gauntlet md {line}:{col}: tab in indentation — the front-matter grammar indents \
                 with spaces only (spec 8.2)"
            ),
            Self::BlockScalar { line, col } => write!(
                f,
                "gauntlet md {line}:{col}: block scalars are outside the front-matter grammar — \
                 write the value inline (spec 8.2)"
            ),
            Self::Anchor { line, col } => write!(
                f,
                "gauntlet md {line}:{col}: anchors are outside the front-matter grammar (spec 8.2)"
            ),
            Self::Alias { line, col } => write!(
                f,
                "gauntlet md {line}:{col}: aliases are outside the front-matter grammar (spec 8.2)"
            ),
            Self::Tag { line, col } => write!(
                f,
                "gauntlet md {line}:{col}: tags are outside the front-matter grammar (spec 8.2)"
            ),
            Self::SecondDocument { line } => write!(
                f,
                "gauntlet md {line}: a second document marker inside the front matter — one \
                 document per contract (spec 8.2)"
            ),
            Self::DuplicateKey { line, col, key } => write!(
                f,
                "gauntlet md {line}:{col}: duplicate key '{key}' in one mapping — an ambiguous \
                 document is not admitted (spec 8.2)"
            ),
            Self::NotAMappingEntry { line, col } => write!(
                f,
                "gauntlet md {line}:{col}: the front matter MUST decode to a map — this line is \
                 not a `key: value` entry (spec 8.2)"
            ),
            Self::UnclosedQuote { line, col } => write!(
                f,
                "gauntlet md {line}:{col}: quoted string never closes (spec 8.2)"
            ),
            Self::UnclosedBracket { line, col } => write!(
                f,
                "gauntlet md {line}:{col}: flow sequence or mapping never closes (spec 8.2)"
            ),
            Self::EmptyKey { line, col } => {
                write!(f, "gauntlet md {line}:{col}: empty key (spec 8.2)")
            }
            Self::ColonExpected { line, col } => write!(
                f,
                "gauntlet md {line}:{col}: a key must be followed immediately by ':' (spec 8.2)"
            ),
            Self::InvalidKeyCharacter { line, col, found } => write!(
                f,
                "gauntlet md {line}:{col}: '{found}' cannot start a key — keys are bare \
                 identifiers (spec 8.2)"
            ),
            Self::TrailingContent { line, col } => write!(
                f,
                "gauntlet md {line}:{col}: content continues after a closed quote or flow — only \
                 a comment may follow (spec 8.2)"
            ),
            Self::MixedIndentation {
                line,
                col,
                open_indent,
            } => write!(
                f,
                "gauntlet md {line}:{col}: indentation matches no open block (open blocks at \
                 {open_indent} spaces) — siblings share one width (spec 8.2)"
            ),
            Self::BlockAfterScalar { line, col } => write!(
                f,
                "gauntlet md {line}:{col}: a deeper line follows a scalar value — a scalar has no \
                 children (spec 8.2)"
            ),
            Self::UnterminatedFrontMatter { line } => write!(
                f,
                "gauntlet md {line}: the opening fence never closes — front matter runs to a \
                 line of exactly '---' (spec 8.2)"
            ),
            Self::ByteOrderMark { line, col } => write!(
                f,
                "gauntlet md {line}:{col}: the document begins with a byte-order mark (U+FEFF) — \
                 remove the byte-order mark (save the file as UTF-8 without BOM) and the front \
                 matter will parse (spec 8.2)"
            ),
        }
    }
}

impl std::error::Error for GauntletMdError {}

/// Parses one GAUNTLET.md document. Everything after a closing fence (or the
/// whole file, when it opens without one) is documentation and ignored.
pub fn parse(text: &str) -> Result<FrontMatter, GauntletMdError> {
    // A leading byte-order mark is rejected, not tolerated (see the variant):
    // tolerance silently discards the whole config layer.
    if text.starts_with('\u{feff}') {
        return Err(GauntletMdError::ByteOrderMark { line: 1, col: 1 });
    }
    let lines: Vec<&str> = text.lines().collect();
    let mut opening: Option<usize> = None;
    for (index, line) in lines.iter().enumerate() {
        if line.trim().is_empty() {
            continue;
        }
        if line.trim() == "---" {
            opening = Some(index);
        }
        break;
    }
    let Some(open) = opening else {
        return Ok(FrontMatter::default());
    };
    let mut close: Option<usize> = None;
    for (index, line) in lines.iter().enumerate().skip(open + 1) {
        let trimmed = line.trim();
        if trimmed == "---" {
            close = Some(index);
            break;
        }
        if trimmed.starts_with("---") {
            return Err(GauntletMdError::SecondDocument { line: index + 1 });
        }
    }
    let Some(close) = close else {
        return Err(GauntletMdError::UnterminatedFrontMatter { line: lines.len() });
    };
    parse_region(&lines[open + 1..close], open + 2)
}

/// One open block while parsing the fenced region. `owner` names the
/// (frame, entry) whose value this block's entries become when it closes.
struct Frame {
    indent: usize,
    entries: Vec<FmEntry>,
    owner: Option<(usize, usize)>,
    expects_child: bool,
}

/// Parses the fenced region: `first_line` is the 1-based file line of the
/// region's first content line.
fn parse_region(lines: &[&str], first_line: usize) -> Result<FrontMatter, GauntletMdError> {
    let mut stack: Vec<Frame> = vec![Frame {
        indent: 0,
        entries: Vec::new(),
        owner: None,
        expects_child: false,
    }];
    for (index, raw) in lines.iter().enumerate() {
        let line_no = first_line + index;
        let mut indent = 0usize;
        let mut first_col = 0usize;
        for (offset, ch) in raw.char_indices() {
            match ch {
                ' ' => indent += 1,
                '\t' => {
                    return Err(GauntletMdError::TabIndentation {
                        line: line_no,
                        col: offset + 1,
                    });
                }
                _ => {
                    first_col = offset;
                    break;
                }
            }
        }
        let rest = &raw[indent..];
        if rest.trim().is_empty() || rest.trim_start().starts_with('#') {
            continue;
        }
        // Close every block this line is shallower than.
        let mut closed_a_block = false;
        while stack.len() > 1 && indent < stack[stack.len() - 1].indent {
            closed_a_block = true;
            close_top_frame(&mut stack);
        }
        let top = &stack[stack.len() - 1];
        let top_indent = top.indent;
        if indent > top_indent {
            if closed_a_block {
                // The line sits deeper than an open block but shallower than
                // the one just closed: it matches no open block's width.
                return Err(GauntletMdError::MixedIndentation {
                    line: line_no,
                    col: first_col + 1,
                    open_indent: top_indent,
                });
            }
            if !stack[stack.len() - 1].expects_child {
                return Err(GauntletMdError::BlockAfterScalar {
                    line: line_no,
                    col: first_col + 1,
                });
            }
            let owner_frame = stack.len() - 1;
            let owner_entry = stack[owner_frame].entries.len().saturating_sub(1);
            stack.push(Frame {
                indent,
                entries: Vec::new(),
                owner: Some((owner_frame, owner_entry)),
                expects_child: false,
            });
        }
        let entry = parse_entry(rest, line_no, indent)?;
        let expects_child = matches!(entry.value, FmValue::Map(ref inner) if inner.is_empty());
        let last = stack.len() - 1;
        let frame = &mut stack[last];
        if frame
            .entries
            .iter()
            .any(|existing| existing.key == entry.key)
        {
            return Err(GauntletMdError::DuplicateKey {
                line: line_no,
                col: first_col + 1,
                key: entry.key,
            });
        }
        frame.entries.push(entry);
        frame.expects_child = expects_child;
    }
    while stack.len() > 1 {
        close_top_frame(&mut stack);
    }
    let root = stack.pop().expect("the root frame is never popped");
    Ok(FrontMatter {
        entries: root.entries,
    })
}

/// Pops the innermost block and writes its entries into its owner entry,
/// clearing the parent's child expectation (the block is closed; a later
/// deeper line is an error, not a second child).
fn close_top_frame(stack: &mut Vec<Frame>) {
    let Some(frame) = stack.pop() else {
        return;
    };
    let Some((frame_idx, entry_idx)) = frame.owner else {
        return;
    };
    let value = FmValue::Map(frame.entries);
    if let Some(parent) = stack.get_mut(frame_idx) {
        if let Some(entry) = parent.entries.get_mut(entry_idx) {
            entry.value = value;
        }
        parent.expects_child = false;
    }
}

/// Whether a character may start a key: `[A-Za-z_]`.
fn is_key_start(ch: char) -> bool {
    ch.is_ascii_alphabetic() || ch == '_'
}

/// Whether a character may continue a key: `[A-Za-z0-9_]`.
fn is_key_continue(ch: char) -> bool {
    ch.is_ascii_alphanumeric() || ch == '_'
}

/// Parses one `key: value` line body (indentation stripped).
fn parse_entry(rest: &str, line_no: usize, indent: usize) -> Result<FmEntry, GauntletMdError> {
    let first = rest.chars().next().expect("caller skips blank lines");
    let col = indent + 1;
    match first {
        '-' => {
            return Err(GauntletMdError::NotAMappingEntry { line: line_no, col });
        }
        ':' => return Err(GauntletMdError::EmptyKey { line: line_no, col }),
        '|' | '>' => {
            return Err(GauntletMdError::BlockScalar { line: line_no, col });
        }
        '&' => return Err(GauntletMdError::Anchor { line: line_no, col }),
        '*' => return Err(GauntletMdError::Alias { line: line_no, col }),
        '!' => return Err(GauntletMdError::Tag { line: line_no, col }),
        _ => {}
    }
    if !is_key_start(first) {
        return Err(GauntletMdError::InvalidKeyCharacter {
            line: line_no,
            col,
            found: first,
        });
    }
    let mut key_end = first.len_utf8();
    for (offset, ch) in rest.char_indices().skip(first.len_utf8()) {
        if is_key_continue(ch) {
            key_end = offset + ch.len_utf8();
        } else {
            break;
        }
    }
    let key = &rest[..key_end];
    let after = &rest[key_end..];
    let Some(colon) = after.chars().next() else {
        return Err(GauntletMdError::ColonExpected {
            line: line_no,
            col: indent + key_end + 1,
        });
    };
    if colon != ':' {
        return Err(GauntletMdError::ColonExpected {
            line: line_no,
            col: indent + key_end + 1,
        });
    }
    let value_text = after[1..].trim_start();
    let value = parse_value(value_text, line_no, indent + key_end + 2)?;
    Ok(FmEntry {
        key: key.to_owned(),
        value,
        line: line_no,
    })
}

/// Parses the value part after `key:` (already skipped whitespace).
fn parse_value(text: &str, line_no: usize, col: usize) -> Result<FmValue, GauntletMdError> {
    let first = text.chars().next();
    let Some(first) = first else {
        return Ok(FmValue::Map(Vec::new()));
    };
    match first {
        '#' => Ok(FmValue::Map(Vec::new())),
        '|' | '>' => Err(GauntletMdError::BlockScalar { line: line_no, col }),
        '&' => Err(GauntletMdError::Anchor { line: line_no, col }),
        '*' => Err(GauntletMdError::Alias { line: line_no, col }),
        '!' => Err(GauntletMdError::Tag { line: line_no, col }),
        '"' | '\'' => {
            let (value, tail) = read_quoted(text, line_no, col)?;
            expect_comment_tail(tail, line_no, col + text.len() - tail.len())?;
            Ok(FmValue::Str(value))
        }
        '[' => {
            let (items, tail) = read_flow_seq(text, line_no, col)?;
            expect_comment_tail(tail, line_no, col + text.len() - tail.len())?;
            Ok(FmValue::Seq(items))
        }
        '{' => {
            let (entries, tail) = read_flow_map(text, line_no, col)?;
            expect_comment_tail(tail, line_no, col + text.len() - tail.len())?;
            Ok(FmValue::Map(entries))
        }
        _ => Ok(bare_value(text)),
    }
}

/// A bare (unquoted) value: the text up to a comment marker, trimmed. Booleans
/// and integers classify; everything else stays a raw string, `$VAR` tokens
/// included.
fn bare_value(text: &str) -> FmValue {
    let cut = comment_start(text);
    let bare = text[..cut].trim_end();
    if bare == "true" {
        return FmValue::Bool(true);
    }
    if bare == "false" {
        return FmValue::Bool(false);
    }
    if let Ok(n) = bare.parse::<i64>() {
        return FmValue::Int(n);
    }
    FmValue::Str(bare.to_owned())
}

/// Byte offset where a `#` comment begins: at the start, or preceded by a
/// space or tab. A `#` glued to text is part of the value.
fn comment_start(text: &str) -> usize {
    let bytes = text.as_bytes();
    for (offset, byte) in bytes.iter().enumerate() {
        if *byte == b'#' && (offset == 0 || matches!(bytes[offset - 1], b' ' | b'\t')) {
            return offset;
        }
    }
    text.len()
}

/// After a closing quote or bracket only whitespace and a comment may follow.
fn expect_comment_tail(tail: &str, line_no: usize, col: usize) -> Result<(), GauntletMdError> {
    let trimmed = tail.trim_start();
    if trimmed.is_empty() || trimmed.starts_with('#') {
        Ok(())
    } else {
        Err(GauntletMdError::TrailingContent { line: line_no, col })
    }
}

/// Reads a quoted string (no escape sequences — a strictness decision
/// documented in the module header). Returns the value and the tail after the
/// closing quote.
fn read_quoted(text: &str, line_no: usize, col: usize) -> Result<(String, &str), GauntletMdError> {
    let quote = text.as_bytes()[0];
    let mut end = None;
    for (offset, byte) in text.bytes().enumerate().skip(1) {
        if byte == quote {
            end = Some(offset);
            break;
        }
    }
    let Some(end) = end else {
        return Err(GauntletMdError::UnclosedQuote { line: line_no, col });
    };
    Ok((text[1..end].to_owned(), &text[end + 1..]))
}

/// Reads `[a, b, c]` — scalars only, no nested flows.
fn read_flow_seq(
    text: &str,
    line_no: usize,
    col: usize,
) -> Result<(Vec<FmScalar>, &str), GauntletMdError> {
    let mut items = Vec::new();
    let mut rest = text[1..].trim_start();
    loop {
        if let Some(after_close) = rest.strip_prefix(']') {
            return Ok((items, after_close));
        }
        let (item, tail) = read_flow_scalar(rest, line_no, col)?;
        items.push(item);
        rest = tail.trim_start();
        if let Some(next) = rest.strip_prefix(',') {
            rest = next.trim_start();
            continue;
        }
        if let Some(after_close) = rest.strip_prefix(']') {
            return Ok((items, after_close));
        }
        if rest.is_empty() {
            return Err(GauntletMdError::UnclosedBracket { line: line_no, col });
        }
        let offset = text.len() - rest.len();
        return Err(GauntletMdError::TrailingContent {
            line: line_no,
            col: col + offset,
        });
    }
}

/// Reads `{k: v, k2: v2}` — identifier keys, scalar values. Duplicate keys
/// are rejected exactly as block mappings reject them: a flow mapping with
/// the same key twice is an ambiguous document, and last-wins would let two
/// different contracts collapse onto one effective configuration.
fn read_flow_map(
    text: &str,
    line_no: usize,
    col: usize,
) -> Result<(Vec<FmEntry>, &str), GauntletMdError> {
    let mut entries = Vec::new();
    let mut rest = text[1..].trim_start();
    loop {
        if let Some(after_close) = rest.strip_prefix('}') {
            return Ok((entries, after_close));
        }
        let key_end = rest
            .find(|c: char| !(c.is_ascii_alphanumeric() || c == '_'))
            .unwrap_or(rest.len());
        if key_end == 0 {
            let offset = text.len() - rest.len();
            return Err(GauntletMdError::InvalidKeyCharacter {
                line: line_no,
                col: col + offset,
                found: rest.chars().next().expect("checked non-empty"),
            });
        }
        let key = rest[..key_end].to_owned();
        if entries.iter().any(|existing| existing.key == key) {
            let offset = text.len() - rest.len();
            return Err(GauntletMdError::DuplicateKey {
                line: line_no,
                col: col + offset,
                key,
            });
        }
        rest = rest[key_end..].trim_start();
        let Some(':') = rest.chars().next() else {
            let offset = text.len() - rest.len();
            return Err(GauntletMdError::ColonExpected {
                line: line_no,
                col: col + offset,
            });
        };
        rest = rest[1..].trim_start();
        let (value, tail) = read_flow_scalar(rest, line_no, col)?;
        entries.push(FmEntry {
            key,
            value: scalar_to_value(value),
            line: line_no,
        });
        rest = tail.trim_start();
        if let Some(next) = rest.strip_prefix(',') {
            rest = next.trim_start();
            continue;
        }
        if let Some(after_close) = rest.strip_prefix('}') {
            return Ok((entries, after_close));
        }
        if rest.is_empty() {
            return Err(GauntletMdError::UnclosedBracket { line: line_no, col });
        }
        let offset = text.len() - rest.len();
        return Err(GauntletMdError::TrailingContent {
            line: line_no,
            col: col + offset,
        });
    }
}

fn scalar_to_value(scalar: FmScalar) -> FmValue {
    match scalar {
        FmScalar::Str(s) => FmValue::Str(s),
        FmScalar::Int(n) => FmValue::Int(n),
        FmScalar::Bool(b) => FmValue::Bool(b),
    }
}

/// One scalar inside a flow: quoted string, or bare up to `,` / `]` / `}`.
fn read_flow_scalar(
    text: &str,
    line_no: usize,
    col: usize,
) -> Result<(FmScalar, &str), GauntletMdError> {
    match text.chars().next() {
        Some('"' | '\'') => {
            let (value, tail) = read_quoted(text, line_no, col)?;
            Ok((FmScalar::Str(value), tail))
        }
        // Nested flows are outside the grammar: reject with the position.
        Some('[' | '{') => Err(GauntletMdError::TrailingContent { line: line_no, col }),
        _ => {
            let end = text.find([',', ']', '}']).unwrap_or(text.len());
            let bare = text[..end].trim_end();
            Ok((classify_scalar(bare), &text[end..]))
        }
    }
}

fn classify_scalar(bare: &str) -> FmScalar {
    if let Ok(n) = bare.parse::<i64>() {
        return FmScalar::Int(n);
    }
    match bare {
        "true" => FmScalar::Bool(true),
        "false" => FmScalar::Bool(false),
        _ => FmScalar::Str(bare.to_owned()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn err_of(text: &str) -> GauntletMdError {
        parse(text).expect_err("fixture must fail")
    }

    #[test]
    fn no_fence_means_no_front_matter() {
        let doc = "# A title\n\n## Section\nprose and nothing else\n";
        let fm = parse(doc).expect("documentation only");
        assert!(fm.is_empty());
    }

    #[test]
    fn empty_fences_parse_to_an_empty_map() {
        let fm = parse("---\n---\n").expect("empty front matter");
        assert!(fm.is_empty());
    }

    #[test]
    fn nested_maps_scalars_and_comments_parse() {
        // Values here avoid path-shaped spellings: the boundary suite's
        // literal scan polices test text like any other source, and the
        // real reference paths live in the spec-example fixture file.
        let doc = "---\n# leading comment\nreference:\n  manifest: manifest-one.yaml\n  \
                    version: 1\n  frozen: true\nroles:\n  dir: roles-area\nbudget:\n  \
                    max_rounds: 5\n  max_tokens: 7000\nobservation:\n  facets: [versioning, \
                    filesystem, build, test]\n---\n## After\n";
        let fm = parse(doc).expect("well-formed front matter");
        assert_eq!(fm.entries.len(), 4);
        let reference = fm.entries[0].value.as_map().expect("map");
        assert_eq!(reference.len(), 3);
        assert_eq!(
            reference[0].value,
            FmValue::Str("manifest-one.yaml".to_owned())
        );
        assert_eq!(reference[1].value, FmValue::Int(1));
        assert_eq!(reference[2].value, FmValue::Bool(true));
        let budget = fm.entries[2].value.as_map().expect("map");
        assert_eq!(budget[0].value, FmValue::Int(5));
        let facets = match &fm.entries[3].value {
            FmValue::Map(entries) => match &entries[0].value {
                FmValue::Seq(items) => items.clone(),
                other => panic!("expected sequence, got {other:?}"),
            },
            other => panic!("expected map, got {other:?}"),
        };
        assert_eq!(facets.len(), 4);
        assert_eq!(facets[0], FmScalar::Str("versioning".to_owned()));
        // A `#` after a value comments; inside quotes it is text.
        let with_comment = parse("---\nroles:\n  dir: roles-area # one comment\n---\n")
            .expect("comment after value");
        let roles = with_comment.entries[0].value.as_map().expect("map");
        assert_eq!(roles[0].value, FmValue::Str("roles-area".to_owned()));
        let quoted = parse("---\nroles:\n  dir: \"a # b\"\n---\n").expect("hash inside quotes");
        let roles = quoted.entries[0].value.as_map().expect("map");
        assert_eq!(roles[0].value, FmValue::Str("a # b".to_owned()));
    }

    #[test]
    fn flow_mappings_and_indirection_tokens_survive_verbatim() {
        // The indirection suffix here avoids any absolute-looking path shape
        // in test text; the spec-example fixture file exercises the real
        // spellings.
        let doc = "---\nrunners:\n  by_role:\n    builder:  { runner: seeded, model: \
                    $GAUNTLET_BUILDER_MODEL }\n    verifier: { runner: none }\nworkspace:\n  \
                    root: ${INDIRECTION_VAR}/leaves\n---\n";
        let fm = parse(doc).expect("flow mappings parse");
        let runners = fm.entries[0].value.as_map().expect("runners map");
        let by_role = match &runners[0].value {
            FmValue::Map(entries) => entries,
            other => panic!("expected map, got {other:?}"),
        };
        assert_eq!(by_role[0].key, "builder");
        let builder = by_role[0].value.as_map().expect("flow map");
        assert_eq!(builder[0].key, "runner");
        assert_eq!(builder[0].value, FmValue::Str("seeded".to_owned()));
        assert_eq!(
            builder[1].value,
            FmValue::Str("$GAUNTLET_BUILDER_MODEL".to_owned())
        );
        let workspace = fm.entries[1].value.as_map().expect("workspace");
        assert_eq!(
            workspace[0].value,
            FmValue::Str("${INDIRECTION_VAR}/leaves".to_owned())
        );
    }

    #[test]
    fn entries_carry_physical_lines() {
        let fm = parse("---\nroles:\n  dir: roles-area\n---\n").expect("parses");
        assert_eq!(fm.entries[0].line, 2);
        let roles = fm.entries[0].value.as_map().expect("map");
        assert_eq!(roles[0].line, 3);
    }

    #[test]
    fn every_rejected_shape_is_its_own_typed_error() {
        // Tab indentation.
        assert!(matches!(
            err_of("---\nroles:\n\tdir: x\n---\n"),
            GauntletMdError::TabIndentation { line: 3, col: 1 }
        ));
        // Block scalars, anchors, aliases, tags.
        assert!(matches!(
            err_of("---\nnote: |\n  folded\n---\n"),
            GauntletMdError::BlockScalar { line: 2, .. }
        ));
        assert!(matches!(
            err_of("---\nnote: >\n  text\n---\n"),
            GauntletMdError::BlockScalar { line: 2, .. }
        ));
        assert!(matches!(
            err_of("---\n&anchor: 1\n---\n"),
            GauntletMdError::Anchor { line: 2, .. }
        ));
        assert!(matches!(
            err_of("---\nvalue: *alias\n---\n"),
            GauntletMdError::Alias { line: 2, .. }
        ));
        assert!(matches!(
            err_of("---\nvalue: !tagged\n---\n"),
            GauntletMdError::Tag { line: 2, .. }
        ));
        // Second document marker.
        assert!(matches!(
            err_of("---\n--- part two\n---\n"),
            GauntletMdError::SecondDocument { line: 2 }
        ));
        // Duplicate keys.
        assert!(matches!(
            err_of("---\nbudget:\n  max_rounds: 5\n  max_rounds: 6\n---\n"),
            GauntletMdError::DuplicateKey { line: 4, key, .. } if key == "max_rounds"
        ));
        // Non-map top level and nested.
        assert!(matches!(
            err_of("---\n- one\n- two\n---\n"),
            GauntletMdError::NotAMappingEntry { line: 2, .. }
        ));
        // Unclosed quote, unclosed bracket.
        assert!(matches!(
            err_of("---\nvalue: \"open\n---\n"),
            GauntletMdError::UnclosedQuote { line: 2, .. }
        ));
        assert!(matches!(
            err_of("---\nfacets: [a, b\n---\n"),
            GauntletMdError::UnclosedBracket { line: 2, .. }
        ));
        assert!(matches!(
            err_of("---\nrunners: { default: x\n---\n"),
            GauntletMdError::UnclosedBracket { line: 2, .. }
        ));
        // Empty key; missing colon; invalid key character.
        assert!(matches!(
            err_of("---\n: value\n---\n"),
            GauntletMdError::EmptyKey { line: 2, .. }
        ));
        assert!(matches!(
            err_of("---\nkey value\n---\n"),
            GauntletMdError::ColonExpected { line: 2, .. }
        ));
        assert!(matches!(
            err_of("---\nkey.: 1\n---\n"),
            GauntletMdError::ColonExpected { line: 2, .. }
        ));
        assert!(matches!(
            err_of("---\n9lives: 1\n---\n"),
            GauntletMdError::InvalidKeyCharacter { line: 2, found, .. } if found == '9'
        ));
        // Trailing content after closed quote and flow.
        assert!(matches!(
            err_of("---\nvalue: \"closed\" extra\n---\n"),
            GauntletMdError::TrailingContent { line: 2, .. }
        ));
        assert!(matches!(
            err_of("---\nfacets: [a] extra\n---\n"),
            GauntletMdError::TrailingContent { line: 2, .. }
        ));
        // Mixed sibling indentation: 4 then 3 under one parent.
        assert!(matches!(
            err_of("---\nroles:\n    dir: x\n   note: y\n---\n"),
            GauntletMdError::MixedIndentation { line: 4, .. }
        ));
        // A scalar value cannot gain children.
        assert!(matches!(
            err_of("---\nroles: pinned\n  dir: x\n---\n"),
            GauntletMdError::BlockAfterScalar { line: 3, .. }
        ));
        // Unterminated fence.
        assert!(matches!(
            err_of("---\nroles:\n  dir: x\n"),
            GauntletMdError::UnterminatedFrontMatter { .. }
        ));
    }

    #[test]
    fn a_leading_byte_order_mark_is_rejected_not_tolerated() {
        // Tolerated, a BOM hides the opening fence and the whole contract
        // silently becomes documentation (empty front matter); rejected, the
        // failure carries the file position and the remediation.
        let doc = "\u{feff}---\nbudget:\n  max_rounds: 5\n---\n";
        assert!(matches!(
            err_of(doc),
            GauntletMdError::ByteOrderMark { line: 1, col: 1 }
        ));
        // The byte-identical control without the BOM parses.
        let control = doc.trim_start_matches('\u{feff}');
        let fm = parse(control).expect("control parses");
        let budget = fm.entries[0].value.as_map().expect("map");
        assert_eq!(budget[0].value, FmValue::Int(5));
    }

    #[test]
    fn duplicate_keys_in_flow_mappings_are_rejected_like_block_keys() {
        // The inline flow form enforces the same one-key-per-mapping rule;
        // last-wins would let two documents differing only by a duplicated
        // flow key collapse onto one effective configuration and fingerprint.
        let dup = "---\nrunners:\n  by_role:\n    builder: { runner: aaa, runner: bbb }\n---\n";
        assert!(matches!(
            err_of(dup),
            GauntletMdError::DuplicateKey { line: 4, col, key, .. } if key == "runner" && col > 1
        ));
        // The single-key spelling still parses; the duplicated document no
        // longer shares its effective shape (it does not parse at all).
        let single = parse("---\nrunners:\n  by_role:\n    builder: { runner: bbb }\n---\n")
            .expect("parses");
        let runners = single.entries[0].value.as_map().expect("runners map");
        let by_role = match &runners[0].value {
            FmValue::Map(entries) => entries,
            other => panic!("expected map, got {other:?}"),
        };
        let builder = by_role[0].value.as_map().expect("flow map");
        assert_eq!(builder.len(), 1);
        assert_eq!(builder[0].value, FmValue::Str("bbb".to_owned()));
    }

    #[test]
    fn the_spec_example_fixture_parses_with_flow_maps_and_indirections() {
        let path = crate::config::fixture_path("spec-example.md");
        let text = std::fs::read_to_string(&path)
            .unwrap_or_else(|err| panic!("read {}: {err}", path.display()));
        let fm = parse(&text).expect("the §8.2 example is inside the grammar");
        assert_eq!(fm.entries.len(), 12);
        // Spot checks over the shapes that matter downstream.
        let runners = fm
            .entries
            .iter()
            .find(|entry| entry.key == "runners")
            .and_then(|entry| entry.value.as_map())
            .expect("runners map");
        assert_eq!(runners[0].key, "default");
        assert_eq!(
            runners[0].value,
            FmValue::Str("$GAUNTLET_AGENT_RUNNER".to_owned())
        );
        let budget = fm
            .entries
            .iter()
            .find(|entry| entry.key == "budget")
            .and_then(|entry| entry.value.as_map())
            .expect("budget map");
        assert_eq!(budget.len(), 8);
        // The inline flow mapping by_role entries parse as maps.
        let by_role = runners
            .iter()
            .find(|entry| entry.key == "by_role")
            .and_then(|entry| entry.value.as_map())
            .expect("by_role map");
        assert_eq!(by_role.len(), 3);
        let builder = by_role[0].value.as_map().expect("builder flow map");
        assert_eq!(builder.len(), 2);
    }
}
