//! Scan-unit normalization shared by both source scanners (recritique R6,
//! round-3 V6/V11/V17), and the ONE Rust-aware lexing pass every source
//! consumer reads (round 6, critic-6 C1 / critic-7 C2).
//!
//! # The lexing pass
//!
//! Every question of the form "is this quote (or bracket, or slash) data or
//! structure?" is answered by a single state machine, [`regions`], which
//! delimits string literals (plain, byte, raw, byte-raw), char literals
//! (`'"'` included), line comments, and nested block comments, and decodes
//! `\xNN`/`\u{NN}` escapes in string bodies. The denylist, the literal
//! scanner, and the escape scanner all consume that pass's output through
//! [`string_literals`], [`string_literal_spans`], [`attribute_spans`], and
//! [`bracket_end`]. No class carries its own quote counting any more: the
//! per-class scanners that predated this pass disagreed about the same bytes
//! (`escapes` was char-literal aware, `literals` was not), and the weaker one
//! fed the denylist — so a single `'"'` earlier on a line deleted every
//! following literal from eight classes and from AK-014's adapter-segment
//! rule (critic-6 C1).
//!
//! Reading a file line by line is defeated by four ordinary-looking Rust
//! constructs that put a term back together at compile time:
//!
//! ```text
//! pub const MODEL: &str = "claud\
//! e-3-5-sonnet";                      // string line-continuation
//! pub const STORE: &str = concat!("sq", "lite");   // adjacent fragments
//! pub const DB: &str = "\x73qlite";                // numeric escape
//! pub static MODE: &str =
//!     "adversarial";                               // rustfmt statement break
//! ```
//!
//! No physical line contains the compiled term. [`scan_units`] therefore
//! yields FOUR forms of every file:
//!
//! - the physical lines, exactly as today;
//! - the *logical* lines, produced by collapsing `\` + newline + leading
//!   whitespace (Rust's own line-continuation rule) and by joining adjacent
//!   string-literal fragments separated by `","` (the `concat!` shape,
//!   including raw-string and comment-separated fragments — round-3 V11);
//! - the *statement* lines, produced by joining a line that ends on an open
//!   continuation token (`=`, `(`, `,`, `[`, `+`) with the lines that follow
//!   it up to the terminating `;` — the shape rustfmt produces whenever a
//!   declaration exceeds the line width (round-3 V17) — and a declaration
//!   head whose value sits on the NEXT line (`pub const X: u64` /
//!   `= 900_000;`), a split rustc accepts just as happily (critic-4 M1);
//! - the *escape-decoded* form of each of the above, in which `\xNN` and
//!   `\u{NN}` become the character they compile to (round-3 V6).
//!
//! All are scanned because normalization can hide as well as reveal: joining
//! `"80", "80"` produces `8080`, but joining `"8080", "1"` produces `80801`,
//! which the port class would no longer see. A violation found in any form
//! is reported; duplicates are collapsed by the scanners.
//!
//! A joined line is reported at the line of its FIRST fragment, which is
//! where a reader has to start editing.

/// One unit of scanning: the 1-based original line its first byte came from,
/// and the text to match against.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScanUnit {
    pub line: usize,
    pub text: String,
}

/// The file's physical lines, 1:1, `\r` stripped.
pub fn physical_lines(src: &str) -> Vec<ScanUnit> {
    src.lines()
        .enumerate()
        .map(|(index, line)| ScanUnit {
            line: index + 1,
            text: line.trim_end_matches('\r').to_string(),
        })
        .collect()
}

/// The file's logical lines: line-continuations collapsed and adjacent
/// string-literal fragments joined. For a file containing neither construct
/// this is identical to [`physical_lines`].
pub fn logical_lines(src: &str) -> Vec<ScanUnit> {
    let bytes = src.as_bytes();
    let mut out: Vec<ScanUnit> = Vec::new();
    let mut buf: Vec<u8> = Vec::new();
    // `start` is the original line the current segment opened on; `phys` is
    // the original line the cursor is standing on.
    let mut start = 1usize;
    let mut phys = 1usize;
    let mut i = 0usize;

    while i < bytes.len() {
        // An escaped backslash is content, not a continuation marker.
        if bytes[i] == b'\\' && bytes.get(i + 1) == Some(&b'\\') {
            buf.push(b'\\');
            buf.push(b'\\');
            i += 2;
            continue;
        }
        // Rust string line-continuation: `\` <horizontal ws> newline <ws>.
        if bytes[i] == b'\\'
            && let Some(next) = continuation_end(bytes, i)
        {
            phys += 1;
            i = next;
            continue;
        }
        // Adjacent string-literal fragments: `"` ws* `,` ws* `"` — the shape
        // `concat!("sq", "lite")` compiles to one contiguous string.
        if bytes[i] == b'"'
            && let Some((next, newlines)) = literal_join_end(bytes, i)
        {
            phys += newlines;
            i = next;
            continue;
        }
        if bytes[i] == b'\n' {
            out.push(take_unit(start, &mut buf));
            phys += 1;
            start = phys;
            i += 1;
            continue;
        }
        if bytes[i] == b'\r' {
            i += 1;
            continue;
        }
        buf.push(bytes[i]);
        i += 1;
    }
    if !buf.is_empty() {
        out.push(take_unit(start, &mut buf));
    }
    out
}

/// Maximum number of physical lines a generic statement unit may span. This is
/// the loose join — any line ending on an open token — so it stays short: it
/// exists to catch rustfmt's ordinary wrapping, and welding hundreds of lines
/// of arbitrary code together would create matches nobody wrote.
///
/// The class that actually NEEDS an identifier and its value in one unit (the
/// timeout and tier-policy classes) is served by [`declaration_units`] instead,
/// which joins a `const`/`static`/`let` declaration to its terminating `;` with
/// the much larger [`MAX_DECLARATION_LINES`] budget — and reports a
/// declaration it cannot join as a violation rather than skipping it
/// (critic-5 M1, invariant I8).
const MAX_STATEMENT_LINES: usize = 24;

/// Maximum number of physical lines a single `const`/`static`/`let`
/// declaration may span before this scanner declares the input unscannable.
///
/// The bound exists because a scanner must terminate, not because a longer
/// declaration is legal: exceeding it is a VIOLATION, surfaced by
/// [`unjoinable_declarations`], never a silent skip. Critic-5 M1 defeated the
/// old fixed 24-line window with 24 filler comment lines between the `=` and
/// the value; any fixed window can be out-run, which is why the response to
/// running out is a report rather than silence.
pub const MAX_DECLARATION_LINES: usize = 500;

/// Byte sequences that leave a line syntactically open, so the statement
/// continues on the next physical line. `=` is the one rustfmt produces for an
/// over-long `const`/`static` declaration (round-3 V17); the others cover
/// multi-line calls, argument lists, and array literals.
fn opens_continuation(trimmed: &str) -> bool {
    matches!(
        trimmed.as_bytes().last(),
        Some(b'=' | b'(' | b',' | b'[' | b'+')
    )
}

/// Whether `trimmed` is a directive head whose argument sits on a LATER line
/// (critic-7 C2), and the next physical line begins with the byte that
/// continues it. Two families:
///
/// - an include-family word as the line's LAST word — `include!`,
///   `include_str`, `std::include`, with or without the `!` — whose call
///   opens on the next line (`!` or `(`), and
/// - an attribute head (`#[`, `#![`, whitespace-tolerant) whose `= value ]`
///   tail opens on the next line (`=`).
///
/// `include!` ends on `!` and `#[path` on a letter, so no open-continuation
/// token ever joined these; the head and its argument landed in two units and
/// NEITHER carried the pair, which is exactly the shape that hid
/// `include!`⏎`("../../shared/wiring.rs")` from the escape scanner.
fn directive_head_awaits(trimmed: &str, next_first: Option<u8>) -> bool {
    let Some(next) = next_first else {
        return false;
    };
    let last_word = trimmed
        .split_whitespace()
        .next_back()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let bare = last_word.trim_end_matches('!');
    let name = bare.rsplit(':').next().unwrap_or(bare);
    if matches!(name, "include" | "include_str" | "include_bytes") && matches!(next, b'!' | b'(') {
        return true;
    }
    attribute_head(trimmed.trim_start()) && next == b'='
}

/// Whether the line opens an attribute: `#[`, `#![`, or the whitespace-tolerant
/// `# [` spelling (critic-4 m3).
fn attribute_head(trimmed: &str) -> bool {
    let bytes = trimmed.as_bytes();
    if bytes.first() != Some(&b'#') {
        return false;
    }
    let mut i = 1;
    if bytes.get(i) == Some(&b'!') {
        i += 1;
    }
    while matches!(bytes.get(i), Some(b' ' | b'\t')) {
        i += 1;
    }
    bytes.get(i) == Some(&b'[')
}

/// Whether the line opens with a declaration keyword after any qualifier
/// tokens — `pub const`, `static`, `let`, `pub(crate) static`, … Doc comments
/// and prose never qualify: their first token is `//!` or a word, not one of
/// these.
pub(crate) fn declaration_head(trimmed: &str) -> bool {
    for token in trimmed.split_whitespace() {
        match token {
            "pub" | "async" | "unsafe" | "extern" => continue,
            t if t.starts_with("pub(") => continue,
            "const" | "static" | "let" => return true,
            _ => return false,
        }
    }
    false
}

/// Statement units: a line that ends on an open continuation token, a
/// declaration head whose value sits on the NEXT line (critic-4 M1), or a
/// directive head whose argument sits on the next line (critic-7 C2), joined
/// with the lines that follow it, up to and including the one carrying the
/// terminating `;`.
///
/// Both split shapes exist because the scanners require a construct and its
/// argument in ONE unit: an ordinary rustfmt line break after `=` used to
/// split an identifier from its value (round-3 V17), and so did the split
/// BEFORE the `=` (`pub const X: u64` / `= 900_000;`, critic-4 M1) and the
/// split between a directive head and its `=`/`(` (`include!` / `("…");`,
/// `#[path` / `= "…"]` — critic-7 C2), both of which rustc accepts and
/// neither of which any open-continuation token ever saw. Joining stays
/// deliberately conservative — an open token, a declaration head followed by
/// a line that begins with `=` or `:`, or a directive head whose next line
/// begins with the byte that continues it — so ordinary prose and markdown
/// tables are never welded together.
pub fn statement_lines(src: &str) -> Vec<ScanUnit> {
    let physical = physical_lines(src);
    let mut out = Vec::new();
    for (index, unit) in physical.iter().enumerate() {
        let trimmed = unit.text.trim_end();
        if trimmed.is_empty() || trimmed.contains(';') {
            continue;
        }
        let next_head = physical
            .get(index + 1)
            .map(|next| next.text.trim_start().as_bytes().first().copied());
        let awaits_value =
            declaration_head(trimmed) && matches!(next_head, Some(Some(b'=') | Some(b':')));
        let awaits_argument = directive_head_awaits(trimmed, next_head.flatten());
        if !opens_continuation(trimmed) && !awaits_value && !awaits_argument {
            continue;
        }
        let mut text = trimmed.to_string();
        let mut joined = false;
        for next in physical.iter().skip(index + 1).take(MAX_STATEMENT_LINES) {
            text.push(' ');
            text.push_str(next.text.trim());
            joined = true;
            if next.text.contains(';') {
                break;
            }
        }
        if joined {
            out.push(ScanUnit {
                line: unit.line,
                text,
            });
        }
    }
    out
}

/// One `const`/`static`/`let` declaration, joined from its head line to the
/// physical line carrying its terminating `;`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Declaration {
    /// The 1-based physical line the declaration head sits on.
    pub line: usize,
    /// The joined text, whether or not the terminator was reached.
    pub text: String,
    /// Whether the `;` was found inside [`MAX_DECLARATION_LINES`].
    pub terminated: bool,
}

/// Every multi-line `const`/`static`/`let` declaration in `src`, joined to its
/// terminator.
///
/// This is the correct-but-bounded join critic-5 M1 asked for. The generic
/// statement join keys on "the line ends with an open token", which the
/// evasion defeats simply by putting the filler AFTER the `=`; a declaration,
/// by contrast, has a syntactic end — the `;` — so the join can be driven by
/// the language rather than by a formatting heuristic. Comment-only filler
/// lines between the head and the value are consumed like any other line, so
/// the distance between the identifier and its value stops mattering.
///
/// `;` inside a string literal or after `//` on the line does not terminate a
/// declaration; over-joining is the safe direction (an extra line of context
/// can only add matches), under-joining is the one that loses evidence.
pub fn declaration_units(src: &str) -> Vec<Declaration> {
    let physical = physical_lines(src);
    let mut out = Vec::new();
    for (index, unit) in physical.iter().enumerate() {
        let head = unit.text.trim();
        if !declaration_head(head) || terminates_statement(head) {
            continue;
        }
        // Comments are dropped from the JOINED form (they are still scanned as
        // physical lines). Critic-5 M1's filler is comment lines between the
        // `=` and the value, and leaving them in would make `//` the token
        // introducing the value — which is exactly how the timeout class was
        // kept silent.
        let mut text = strip_comments(head);
        let mut terminated = false;
        for next in physical.iter().skip(index + 1).take(MAX_DECLARATION_LINES) {
            let line = strip_comments(next.text.trim());
            let line = line.trim();
            if !line.is_empty() {
                text.push(' ');
                text.push_str(line);
            }
            if terminates_statement(line) {
                terminated = true;
                break;
            }
        }
        out.push(Declaration {
            line: unit.line,
            text,
            terminated,
        });
    }
    out
}

/// The 1-based lines of declarations this scanner could not join to their
/// terminator. Each is a violation for its caller (I8): the timeout and
/// tier-policy classes need the identifier and the value in one unit, so a
/// declaration that outruns the budget is a hole, and a hole must be reported
/// rather than passed over.
pub fn unjoinable_declarations(src: &str) -> Vec<usize> {
    declaration_units(src)
        .into_iter()
        .filter(|declaration| !declaration.terminated)
        .map(|declaration| declaration.line)
        .collect()
}

/// One physical line with its comments removed: `// …` to end of line and any
/// `/* … */` span that opens and closes on the line. Quotes are honoured, so a
/// `"//"` inside a string literal is content, not a comment.
fn strip_comments(line: &str) -> String {
    let bytes = line.as_bytes();
    let mut out = String::with_capacity(line.len());
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if bytes.get(i + 1) == Some(&b'/') => break,
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                let mut j = i + 2;
                while j + 1 < bytes.len() && !(bytes[j] == b'*' && bytes[j + 1] == b'/') {
                    j += 1;
                }
                i = if j + 1 < bytes.len() {
                    j + 2
                } else {
                    bytes.len()
                };
            }
            b'"' => {
                let start = i;
                i += 1;
                while i < bytes.len() {
                    match bytes[i] {
                        b'\\' => i += 2,
                        b'"' => {
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
                out.push_str(&line[start..i.min(line.len())]);
            }
            _ => {
                let start = i;
                i += 1;
                while i < bytes.len() && (bytes[i] & 0xC0) == 0x80 {
                    i += 1;
                }
                out.push_str(&line[start..i]);
            }
        }
    }
    out
}

/// Whether a physical line carries a statement terminator that is code: a `;`
/// outside any string literal and outside a `//` comment.
fn terminates_statement(line: &str) -> bool {
    let bytes = line.as_bytes();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if bytes.get(i + 1) == Some(&b'/') => return false,
            b'"' => {
                i += 1;
                while i < bytes.len() {
                    match bytes[i] {
                        b'\\' => i += 2,
                        b'"' => {
                            i += 1;
                            break;
                        }
                        _ => i += 1,
                    }
                }
            }
            b';' => return true,
            _ => i += 1,
        }
    }
    false
}

/// Decode the numeric and unicode string escapes rustc resolves at compile
/// time: `"\x73qlite"` and `"\u{73}qlite"` both compile to `"sqlite"`, and
/// neither used to contain a denied term in any scanned form (round-3 V6).
/// An escaped backslash is copied verbatim so `"C:\\x41"` is not "decoded".
pub fn decode_escapes(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = String::with_capacity(text.len());
    let mut i = 0usize;
    while i < bytes.len() {
        if bytes[i] != b'\\' {
            // Copy whole UTF-8 characters, never half of one.
            let start = i;
            i += 1;
            while i < bytes.len() && (bytes[i] & 0xC0) == 0x80 {
                i += 1;
            }
            out.push_str(&text[start..i]);
            continue;
        }
        match bytes.get(i + 1) {
            Some(b'\\') => {
                out.push_str("\\\\");
                i += 2;
            }
            Some(b'x') if i + 3 < bytes.len() => match u32::from_str_radix(&text[i + 2..i + 4], 16)
            {
                Ok(code) => match char::from_u32(code) {
                    Some(decoded) => {
                        out.push(decoded);
                        i += 4;
                    }
                    None => {
                        out.push('\\');
                        i += 1;
                    }
                },
                Err(_) => {
                    out.push('\\');
                    i += 1;
                }
            },
            Some(b'u') if bytes.get(i + 2) == Some(&b'{') => {
                match text[i + 3..].find('}').map(|at| i + 3 + at) {
                    Some(close) => {
                        let digits = text[i + 3..close].replace('_', "");
                        match u32::from_str_radix(&digits, 16)
                            .ok()
                            .and_then(char::from_u32)
                        {
                            Some(decoded) => {
                                out.push(decoded);
                                i = close + 1;
                            }
                            None => {
                                out.push('\\');
                                i += 1;
                            }
                        }
                    }
                    None => {
                        out.push('\\');
                        i += 1;
                    }
                }
            }
            _ => {
                out.push('\\');
                i += 1;
            }
        }
    }
    out
}

// ---------------------------------------------------------------------------
// The lexing pass (round 6). One state machine, consumed by every source
// scanner; see the module docs. The helpers below it answer every
// "is this byte data or structure?" question on top of the SAME pass.
// ---------------------------------------------------------------------------

/// One lexed region of Rust-like text. Everything not covered by a region is
/// code as far as the boundary scanners are concerned.
///
/// The pass is deliberately tolerant of non-Rust text (`.toml`, `.md`, `.json`
/// files are scanned too): string bodies are read the same way in every
/// language these scanners meet, comments follow Rust rules, and a `//`
/// directly preceded by `:` is treated as a URL scheme divider rather than a
/// comment opener so `https://…` in prose never desynchronizes the scan. The
/// residual is a line comment in non-Rust text spelled `x://c` directly after
/// an identifier — content after it on that one line is skipped for string
/// extraction, which can only lose, never invent, a match.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Region {
    /// A string literal in any spelling (`"…"`, `b"…"`, `r"…"`, `r#"…"#`,
    /// `br##"…"##`): its whole byte span (prefix and hashes included) and its
    /// decoded value — `\xNN` and `\u{NN}` become the character they compile
    /// to. An unterminated literal runs to the end of the text with
    /// `terminated: false`.
    Str {
        span: std::ops::Range<usize>,
        value: String,
        terminated: bool,
    },
    /// A char literal, `'"'` and `'\''` included. A `'` that does not close a
    /// char-literal shape (a lifetime, an apostrophe in prose) is not a
    /// region.
    Char { span: std::ops::Range<usize> },
    /// A `//` line comment or a (nested) `/* … */` block comment.
    Comment {
        span: std::ops::Range<usize>,
        terminated: bool,
    },
}

impl Region {
    /// The region's byte span in the scanned text.
    pub fn span(&self) -> std::ops::Range<usize> {
        match self {
            Region::Str { span, .. } | Region::Char { span } | Region::Comment { span, .. } => {
                span.clone()
            }
        }
    }

    /// Whether the region ran to the end of the text without its terminator.
    pub fn is_unterminated(&self) -> bool {
        matches!(
            self,
            Region::Str {
                terminated: false,
                ..
            } | Region::Comment {
                terminated: false,
                ..
            }
        )
    }
}

/// The one lexing pass: every string, char, and comment region of `src`, in
/// order, non-overlapping (critic-6 C1: the previous per-class quote counting
/// treated every `"` byte as a delimiter, so a `'"'` char literal, a quote
/// inside a raw string, or a `/* " */` comment shifted the pairing for the
/// rest of the unit — and the weaker of two private scanners was the one
/// feeding the denylist).
pub fn regions(src: &str) -> Vec<Region> {
    let bytes = src.as_bytes();
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if bytes.get(i + 1) == Some(&b'/') && (i == 0 || bytes[i - 1] != b':') => {
                let start = i;
                while i < bytes.len() && bytes[i] != b'\n' {
                    i = next_char_end(bytes, i);
                }
                out.push(Region::Comment {
                    span: start..i,
                    terminated: true,
                });
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                let start = i;
                let mut depth = 1usize;
                i += 2;
                while i < bytes.len() && depth > 0 {
                    if bytes[i] == b'/' && bytes.get(i + 1) == Some(&b'*') {
                        depth += 1;
                        i += 2;
                    } else if bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/') {
                        depth -= 1;
                        i += 2;
                    } else {
                        i = next_char_end(bytes, i);
                    }
                }
                out.push(Region::Comment {
                    span: start..i,
                    terminated: depth == 0,
                });
            }
            b'"' => {
                let (end, value, terminated) = read_plain_string(src, i);
                out.push(Region::Str {
                    span: i..end,
                    value,
                    terminated,
                });
                i = end;
            }
            b'b' | b'r' if prefixed_string_opens_at(bytes, i) => {
                if let Some((end, value, terminated)) = read_prefixed_string(src, i) {
                    out.push(Region::Str {
                        span: i..end,
                        value,
                        terminated,
                    });
                    i = end;
                } else {
                    i += 1;
                }
            }
            b'\'' => match char_literal_end(bytes, i) {
                Some(end) => {
                    out.push(Region::Char { span: i..end });
                    i = end;
                }
                None => i = next_char_end(bytes, i),
            },
            _ => i = next_char_end(bytes, i),
        }
    }
    out
}

/// Every string literal's decoded value — the ONE extraction the denylist's
/// adapter-segment rule and every literal class consume.
pub fn string_literals(raw: &str) -> Vec<String> {
    regions(raw)
        .into_iter()
        .filter_map(|region| match region {
            Region::Str { value, .. } => Some(value),
            _ => None,
        })
        .collect()
}

/// Every string literal as `(opening byte offset, closing byte offset,
/// decoded value)` — for the classes that need to know WHERE a literal sits
/// (the tier-policy class asks whether it sits inside an attribute body; the
/// denylist's Annex-B.2 mask asks what surrounds it).
pub fn string_literal_spans(raw: &str) -> Vec<(usize, usize, String)> {
    regions(raw)
        .into_iter()
        .filter_map(|region| match region {
            Region::Str { span, value, .. } => Some((span.start, span.end, value)),
            _ => None,
        })
        .collect()
}

/// Every attribute's byte span (`#[…]` and `#![…]`, whitespace-tolerant),
/// computed with the same lexing pass — so a `#[` inside a comment or a
/// string is NOT an attribute (critic-6 C3: a comment containing `#[` used to
/// disable the tier class for the whole unit).
pub fn attribute_spans(src: &str) -> Vec<std::ops::Range<usize>> {
    let bytes = src.as_bytes();
    let regs = regions(src);
    let mut out = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if let Some(span) = span_at(&regs, i) {
            i = span.end;
            continue;
        }
        if bytes[i] == b'#' {
            let mut j = i + 1;
            while matches!(bytes.get(j), Some(b' ' | b'\t')) {
                j += 1;
            }
            if bytes.get(j) == Some(&b'!') {
                j += 1;
                while matches!(bytes.get(j), Some(b' ' | b'\t')) {
                    j += 1;
                }
            }
            if bytes.get(j) == Some(&b'[') {
                let end = bracket_end_with(src, j, &regs);
                out.push(i..end);
                i = end;
                continue;
            }
        }
        i = next_char_end(bytes, i);
    }
    out
}

/// The index just past the `]` that closes the `[` at `open`, skipping
/// string, char, and comment regions — data cannot change bracket depth. The
/// end of the text when the bracket never closes (fail-safe: the caller scans
/// the whole tail).
pub fn bracket_end(src: &str, open: usize) -> usize {
    bracket_end_with(src, open, &regions(src))
}

fn bracket_end_with(src: &str, open: usize, regs: &[Region]) -> usize {
    let bytes = src.as_bytes();
    let mut depth: i64 = 0;
    let mut i = open;
    while i < bytes.len() {
        if let Some(span) = span_at(regs, i) {
            i = span.end;
            continue;
        }
        match bytes[i] {
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i = next_char_end(bytes, i);
    }
    bytes.len()
}

fn span_at(regions: &[Region], at: usize) -> Option<std::ops::Range<usize>> {
    regions
        .iter()
        .find(|region| region.span().contains(&at))
        .map(Region::span)
}

/// The index just past the whole UTF-8 character starting at `at` (one byte
/// for ASCII). The lexer only advances with this, so every index it computes
/// stays on a character boundary.
fn next_char_end(bytes: &[u8], at: usize) -> usize {
    let mut end = at + 1;
    while end < bytes.len() && (bytes[end] & 0xC0) == 0x80 {
        end += 1;
    }
    end
}

/// Read the plain (or byte) string whose opening `"` is at `at`: escapes
/// resolved (`\xNN`, `\u{NN}` decoded; any other `\X` contributes `X`), value
/// decoded, plus the index just past the closing quote.
fn read_plain_string(src: &str, at: usize) -> (usize, String, bool) {
    let bytes = src.as_bytes();
    let mut i = at + 1;
    let mut value = String::new();
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => match bytes.get(i + 1) {
                None => i += 1,
                Some(b'x') if hex_pair(bytes, i + 2) => {
                    let code = u32::from_str_radix(&src[i + 2..i + 4], 16)
                        .ok()
                        .and_then(char::from_u32);
                    match code {
                        Some(decoded) => {
                            value.push(decoded);
                            i += 4;
                        }
                        None => {
                            value.push('x');
                            i += 2;
                        }
                    }
                }
                Some(b'u') if bytes.get(i + 2) == Some(&b'{') => {
                    match unicode_escape_end(bytes, i + 3) {
                        Some((close, code)) => match char::from_u32(code) {
                            Some(decoded) => {
                                value.push(decoded);
                                i = close;
                            }
                            None => {
                                value.push('u');
                                i += 2;
                            }
                        },
                        None => {
                            value.push('u');
                            i += 2;
                        }
                    }
                }
                Some(_) => {
                    let end = next_char_end(bytes, i + 1);
                    value.push_str(&src[i + 1..end]);
                    i = end;
                }
            },
            b'"' => return (i + 1, value, true),
            _ => {
                let end = next_char_end(bytes, i);
                value.push_str(&src[i..end]);
                i = end;
            }
        }
    }
    (bytes.len(), value, false)
}

/// Whether the two bytes at `at` are both ASCII hex digits.
fn hex_pair(bytes: &[u8], at: usize) -> bool {
    at + 1 < bytes.len() && bytes[at].is_ascii_hexdigit() && bytes[at + 1].is_ascii_hexdigit()
}

/// The end of a `\u{…}` body: `(index just past the '}', decoded code point)`
/// as `(usize, u32)`, with `_` separators allowed.
fn unicode_escape_end(bytes: &[u8], at: usize) -> Option<(usize, u32)> {
    let mut i = at;
    let mut digits = String::new();
    while i < bytes.len() && bytes[i] != b'}' {
        if bytes[i] == b'_' {
            i += 1;
            continue;
        }
        if !bytes[i].is_ascii_hexdigit() {
            return None;
        }
        digits.push(bytes[i] as char);
        i += 1;
    }
    if i >= bytes.len() || digits.is_empty() {
        return None;
    }
    u32::from_str_radix(&digits, 16)
        .ok()
        .map(|code| (i + 1, code))
}

/// Whether a raw/byte string opens at `at`: the `r`/`b` must start a word of
/// its own (an `r` at the tail of an identifier is not an opener).
fn prefixed_string_opens_at(bytes: &[u8], at: usize) -> bool {
    if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
        return false;
    }
    let mut j = at;
    if bytes[j] == b'b' {
        j += 1;
    }
    match bytes.get(j) {
        Some(b'r') => {
            j += 1;
            while bytes.get(j) == Some(&b'#') {
                j += 1;
            }
            bytes.get(j) == Some(&b'"')
        }
        // `b"…"` is a plain byte string opened at the `b`.
        Some(b'"') => true,
        _ => false,
    }
}

/// Read the raw (`r#"…"#`) or byte (`b"…"`) string opening at `at`. Raw
/// bodies are verbatim (no escapes); byte strings use the plain rules.
fn read_prefixed_string(src: &str, at: usize) -> Option<(usize, String, bool)> {
    let bytes = src.as_bytes();
    let mut j = at;
    if bytes[j] == b'b' {
        j += 1;
    }
    if bytes.get(j) != Some(&b'r') {
        return Some(read_plain_string(src, j));
    }
    j += 1;
    let mut hashes = 0usize;
    while bytes.get(j) == Some(&b'#') {
        hashes += 1;
        j += 1;
    }
    if bytes.get(j) != Some(&b'"') {
        return None;
    }
    let body = j + 1;
    let mut i = body;
    while i < bytes.len() {
        if bytes[i] == b'"' && (0..hashes).all(|h| bytes.get(i + 1 + h) == Some(&b'#')) {
            return Some((i + 1 + hashes, src[body..i].to_string(), true));
        }
        i = next_char_end(bytes, i);
    }
    Some((bytes.len(), src[body..].to_string(), false))
}

/// The index just past a char literal starting at `at` (the `'`), if one does.
/// `']'`, `'"'`, `'\\''`, and escape bodies are skipped whole; a `'` followed
/// by something that is not a char-literal shape (a lifetime `'a`, an
/// apostrophe in prose) is not a literal and yields `None`.
fn char_literal_end(bytes: &[u8], at: usize) -> Option<usize> {
    match bytes.get(at + 1) {
        Some(b'\\') => (at + 2..bytes.len())
            .find(|&i| bytes[i] == b'\'')
            .map(|i| i + 1),
        Some(second) if bytes.get(at + 2) == Some(&b'\'') && *second != b'\'' => Some(at + 3),
        _ => None,
    }
}

/// Every scan unit for a file: physical lines first, then any logical,
/// statement, or escape-decoded form that differs from what is already there.
/// Order is stable so violation lists are reproducible.
pub fn scan_units(src: &str) -> Vec<ScanUnit> {
    let mut out = physical_lines(src);
    for unit in logical_lines(src) {
        push_unique(&mut out, unit);
    }
    for unit in statement_lines(src) {
        push_unique(&mut out, unit);
    }
    // Declarations joined to their `;` (critic-5 M1). Only the joins that
    // reached the terminator become scan units; one that did not is reported
    // as a violation by the literal scanner instead, because a half-joined
    // blob is evidence of nothing.
    for declaration in declaration_units(src) {
        if declaration.terminated {
            push_unique(
                &mut out,
                ScanUnit {
                    line: declaration.line,
                    text: declaration.text,
                },
            );
        }
    }
    // The escape-decoded form of every unit produced so far: a term spelled
    // `"\x73qlite"` compiles to `"sqlite"` in whichever form carries it.
    let decoded: Vec<ScanUnit> = out
        .iter()
        .filter_map(|unit| {
            let text = decode_escapes(&unit.text);
            (text != unit.text).then_some(ScanUnit {
                line: unit.line,
                text,
            })
        })
        .collect();
    for unit in decoded {
        push_unique(&mut out, unit);
    }
    out
}

fn push_unique(out: &mut Vec<ScanUnit>, unit: ScanUnit) {
    if !out.contains(&unit) {
        out.push(unit);
    }
}

fn take_unit(line: usize, buf: &mut Vec<u8>) -> ScanUnit {
    let text = String::from_utf8(std::mem::take(buf))
        .expect("segments split only on ASCII bytes, so UTF-8 is preserved");
    ScanUnit { line, text }
}

/// If `bytes[at]` (a `\`) opens a line continuation, the index just past the
/// newline and its leading whitespace.
fn continuation_end(bytes: &[u8], at: usize) -> Option<usize> {
    let mut j = at + 1;
    while matches!(bytes.get(j), Some(b' ' | b'\t' | b'\r')) {
        j += 1;
    }
    if bytes.get(j) != Some(&b'\n') {
        return None;
    }
    j += 1;
    while matches!(bytes.get(j), Some(b' ' | b'\t')) {
        j += 1;
    }
    Some(j)
}

/// Skip whitespace and `/* … */` block comments from `at`, counting newlines.
/// Round-3 V11: `concat!("claude", /* id */ "-3-5-sonnet")` compiles to one
/// contiguous string, so an interleaved comment must not stop the join.
fn skip_join_filler(bytes: &[u8], mut j: usize, newlines: &mut usize) -> usize {
    loop {
        while matches!(bytes.get(j), Some(b' ' | b'\t' | b'\r' | b'\n')) {
            if bytes[j] == b'\n' {
                *newlines += 1;
            }
            j += 1;
        }
        if bytes.get(j) == Some(&b'/') && bytes.get(j + 1) == Some(&b'*') {
            let mut k = j + 2;
            while k + 1 < bytes.len() && !(bytes[k] == b'*' && bytes[k + 1] == b'/') {
                if bytes[k] == b'\n' {
                    *newlines += 1;
                }
                k += 1;
            }
            if k + 1 >= bytes.len() {
                return j;
            }
            j = k + 2;
            continue;
        }
        return j;
    }
}

/// The index just past the opening quote of a string literal starting at `j`,
/// accepting the raw forms `r"`, `r#"`, `r##"`, … Round-3 V11: `concat!("sq",
/// r"lite")` compiles to `"sqlite"`, and the plain-quote-only matcher missed
/// it.
fn string_literal_open(bytes: &[u8], j: usize) -> Option<usize> {
    if bytes.get(j) == Some(&b'"') {
        return Some(j + 1);
    }
    if bytes.get(j) != Some(&b'r') {
        return None;
    }
    let mut k = j + 1;
    while bytes.get(k) == Some(&b'#') {
        k += 1;
    }
    (bytes.get(k) == Some(&b'"')).then_some(k + 1)
}

/// If `bytes[at]` (a `"`) closes a fragment that is followed by `,` and
/// another string-literal fragment — plain or raw, with block comments
/// allowed between — the index just past that opening quote, plus the number
/// of newlines consumed.
fn literal_join_end(bytes: &[u8], at: usize) -> Option<(usize, usize)> {
    let mut newlines = 0usize;
    let mut j = skip_join_filler(bytes, at + 1, &mut newlines);
    if bytes.get(j) != Some(&b',') {
        return None;
    }
    j += 1;
    j = skip_join_filler(bytes, j, &mut newlines);
    let open = string_literal_open(bytes, j)?;
    Some((open, newlines))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn line_continuation_is_collapsed_at_the_first_fragments_line() {
        // Recritique R6's exact evasion.
        let src = "pub const MODEL: &str = \"claud\\\n    e-3-5-sonnet\";\n";
        let logical = logical_lines(src);
        assert_eq!(logical.len(), 1, "{logical:?}");
        assert_eq!(logical[0].line, 1);
        assert!(
            logical[0].text.contains("claude-3-5-sonnet"),
            "{}",
            logical[0].text
        );

        let path = "pub const ROOT: &str = \"/hom\\\n    e/agent/state\";\n";
        let logical = logical_lines(path);
        assert!(logical[0].text.contains("/home/agent/state"), "{logical:?}");
    }

    #[test]
    fn adjacent_literal_fragments_are_joined() {
        // Recritique R6's `concat!` evasion, on one line and split over two.
        let src = "pub const STORE: &str = concat!(\"sq\", \"lite\");\n";
        let logical = logical_lines(src);
        assert!(logical[0].text.contains("sqlite"), "{logical:?}");

        let split = "let s = concat!(\n    \"sq\",\n    \"lite\",\n);\n";
        let logical = logical_lines(split);
        assert!(
            logical.iter().any(|u| u.text.contains("sqlite")),
            "{logical:?}"
        );
    }

    #[test]
    fn escaped_backslash_is_not_a_continuation() {
        let src = "let p = \"C:\\\\tools\";\nlet q = 1;\n";
        let logical = logical_lines(src);
        assert_eq!(logical.len(), 2, "{logical:?}");
        assert!(logical[0].text.contains("c:\\\\tools") || logical[0].text.contains("C:\\\\tools"));
    }

    #[test]
    fn ordinary_files_have_identical_physical_and_logical_lines() {
        let src = "fn main() {\n    let x = 1;\n}\n";
        assert_eq!(physical_lines(src), logical_lines(src));
        assert_eq!(scan_units(src).len(), 3);
    }

    #[test]
    fn scan_units_keeps_both_forms_when_they_differ() {
        let src = "let v = [\"8080\", \"1\"];\n";
        let units = scan_units(src);
        // Physical form keeps `8080` visible; the joined form does not.
        assert!(units.iter().any(|u| u.text.contains("8080")));
        assert!(units.iter().any(|u| u.text.contains("80801")));
    }

    #[test]
    fn raw_string_and_comment_separated_fragments_are_joined() {
        // Round-3 V11's exact evasions: `literal_join_end` required the byte
        // shape `" ws* , ws* "`, so a raw-string fragment or an interleaved
        // block comment broke the `concat!` normalization entirely.
        let raw = "pub const STORE: &str = concat!(\"sq\", r\"lite\");\n";
        assert!(
            scan_units(raw).iter().any(|u| u.text.contains("sqlite")),
            "{:?}",
            scan_units(raw)
        );

        let hashed = "pub const STORE: &str = concat!(\"sq\", r#\"lite\"#);\n";
        assert!(
            scan_units(hashed).iter().any(|u| u.text.contains("sqlite")),
            "{:?}",
            scan_units(hashed)
        );

        let commented = "pub const MODEL: &str = concat!(\"claude\", /* id */ \"-3-5-sonnet\");\n";
        assert!(
            scan_units(commented)
                .iter()
                .any(|u| u.text.contains("claude-3-5-sonnet")),
            "{:?}",
            scan_units(commented)
        );
    }

    #[test]
    fn numeric_and_unicode_escapes_are_decoded() {
        // Round-3 V6's exact evasions: both compile to the forbidden value.
        for (src, needle) in [
            ("pub const STORE: &str = \"\\x73qlite\";\n", "sqlite"),
            ("pub const STORE: &str = \"\\u{73}qlite\";\n", "sqlite"),
            (
                "pub const ROOT: &str = \"/\\x74mp/gauntlet/state\";\n",
                "/tmp/gauntlet/state",
            ),
            (
                "pub const ROOT: &str = \"\\u{2f}tmp/gauntlet\";\n",
                "/tmp/gauntlet",
            ),
        ] {
            assert!(
                scan_units(src).iter().any(|u| u.text.contains(needle)),
                "{src} -> {:?}",
                scan_units(src)
            );
        }
        // An escaped backslash is content, not an escape introducer.
        assert_eq!(decode_escapes("c:\\\\x41"), "c:\\\\x41");
    }

    #[test]
    fn statements_split_across_lines_are_joined_to_the_semicolon() {
        // Round-3 V17's exact evasions — the shape rustfmt produces once the
        // declaration exceeds the line width.
        let timeout = "pub const GAUNTLET_BUILDER_INVOCATION_STALL_TIMEOUT_MILLISECONDS: u64 =\n    900_000;\n";
        let units = scan_units(timeout);
        assert!(
            units
                .iter()
                .any(|u| u.text.to_lowercase().contains("timeout") && u.text.contains("900_000")),
            "{units:?}"
        );
        assert!(
            units
                .iter()
                .any(|u| u.line == 1 && u.text.contains("900_000")),
            "the joined unit must report at its first line: {units:?}"
        );

        let tier = "pub static REVIEW_MODE: &str =\n    \"adversarial\";\n";
        let units = scan_units(tier);
        assert!(
            units
                .iter()
                .any(|u| u.text.contains("static ") && u.text.contains("adversarial")),
            "{units:?}"
        );
    }

    #[test]
    fn statement_joining_does_not_weld_prose_together() {
        // The join starts only at an open continuation token, so markdown
        // tables, doc prose, and closed statements stay separate units.
        let prose = "//! | max_wall_seconds | 3600 | bounded |\n//! Retry and stall policy.\n";
        assert!(
            statement_lines(prose).is_empty(),
            "{:?}",
            statement_lines(prose)
        );

        let closed = "let a = 1;\nlet b = 2;\n";
        assert!(statement_lines(closed).is_empty());
    }

    #[test]
    fn statements_split_before_the_equals_are_joined() {
        // Critic-4 M1's exact evasions: the value line STARTS with `=`, which
        // the open-continuation rule never saw, so the identifier and its
        // value lived in two units and neither carried both.
        let timeout = "pub const STALL_TIMEOUT_MS: u64\n    = 900_000;\n";
        let units = scan_units(timeout);
        assert!(
            units.iter().any(|u| u.line == 1
                && u.text.to_lowercase().contains("timeout")
                && u.text.contains("900_000")),
            "{units:?}"
        );

        let tier = "pub static REVIEW_MODE: &str\n    = \"adversarial\";\n";
        let units = scan_units(tier);
        assert!(
            units.iter().any(|u| u.line == 1
                && u.text.contains("static ")
                && u.text.contains("adversarial")),
            "{units:?}"
        );

        // The colon-first spelling joins the same way.
        let colon = "pub const LIMIT_MS: u64\n    : u64 = 5_000;\n";
        let units = scan_units(colon);
        assert!(
            units
                .iter()
                .any(|u| u.line == 1 && u.text.contains("5_000")),
            "{units:?}"
        );

        // A declaration followed by an ordinary line still does NOT join: the
        // next line must begin with `=` or `:`.
        let prose = "let me describe the retry policy\nin this paragraph.\n";
        assert!(
            statement_lines(prose).is_empty(),
            "{:?}",
            statement_lines(prose)
        );
        let doc = "//! let the stall policy\n//! be documented here.\n";
        assert!(
            statement_lines(doc).is_empty(),
            "{:?}",
            statement_lines(doc)
        );
    }

    #[test]
    fn directive_heads_split_before_their_argument_are_joined() {
        // Critic-7 C2's exact evasions: `include!` ends on `!` and `#[path`
        // ends on a letter, so no open-continuation token ever joined the
        // head to the line carrying its argument — and neither unit carried
        // the (directive, argument) pair.
        let include = "include!\n    (\"../../../shared/wiring.rs\");\n";
        let units = scan_units(include);
        assert!(
            units.iter().any(|u| u.line == 1
                && u.text
                    .replace(' ', "")
                    .contains("include!(\"../../../shared/wiring.rs\")")),
            "{units:?}"
        );

        // The `!` may itself sit on the value line, and a path prefix is
        // legal rustc.
        let include2 = "std::include\n!(\"../../../vendor/leak.rs\");\n";
        assert!(
            scan_units(include2)
                .iter()
                .any(|u| u.text.contains("vendor/leak.rs")),
            "{:?}",
            scan_units(include2)
        );

        let attribute = "#[path\n    = \"../../../vendor/store.rs\"]\nmod store;\n";
        assert!(
            scan_units(attribute).iter().any(|u| u
                .text
                .replace(' ', "")
                .contains("#[path=\"../../../vendor/store.rs\"]")),
            "{:?}",
            scan_units(attribute)
        );

        // The cfg_attr spelling of the same split.
        let cfg_attr = "#[cfg_attr(all(), path\n    = \"../../../vendor/store.rs\")]\nmod store;\n";
        assert!(
            scan_units(cfg_attr).iter().any(|u| u
                .text
                .replace(' ', "")
                .contains("path=\"../../../vendor/store.rs\")")),
            "{:?}",
            scan_units(cfg_attr)
        );

        // The inner-attribute spelling.
        let inner = "#![path\n    = \"../../../vendor/store.rs\"]\n";
        assert!(
            scan_units(inner).iter().any(|u| u
                .text
                .replace(' ', "")
                .contains("#![path=\"../../../vendor/store.rs\"]")),
            "{:?}",
            scan_units(inner)
        );

        // Prose does not join: a word ending in `include` followed by an
        // ordinary line is not a call.
        let prose = "we deliberately include\nsome prose here.\n";
        assert!(
            statement_lines(prose).is_empty(),
            "{:?}",
            statement_lines(prose)
        );
        let prose2 = "the docs include\nnothing else.\n";
        assert!(statement_lines(prose2).is_empty());
    }

    /// Critic-6 C1: the one lexing pass must treat a quote that is DATA —
    /// inside a char literal, a raw string, or a comment — as data, so the
    /// literals around it keep their true values.
    #[test]
    fn quotes_that_are_data_do_not_shift_the_extraction() {
        // The critic's own fixture: the `'"'` in `trim_matches('"')` used to
        // contribute a delimiter quote, deleting every following literal.
        let char_quote = "match spec.trim_matches('\"') { \"fast\" => \"mock\", _ => \"prime\" }";
        assert_eq!(
            string_literals(char_quote),
            vec!["fast".to_string(), "mock".to_string(), "prime".to_string()],
            "{:?}",
            regions(char_quote)
        );

        // A quote inside a raw string, and a hashed raw string.
        assert_eq!(
            string_literals("let s = r#\"pass a \" here\"#.trim(); let t = \"sqlite\";"),
            vec!["pass a \" here".to_string(), "sqlite".to_string()]
        );

        // A quote inside a block comment hides nothing after it.
        assert_eq!(
            string_literals("{ /* \" */ let db = \"state/gauntlet.db\"; }"),
            vec!["state/gauntlet.db".to_string()]
        );

        // A quote inside a line comment, and a byte string.
        assert_eq!(
            string_literals("let a = b\"x\"; // trailing \" quote\nlet b = \"y\";"),
            vec!["x".to_string(), "y".to_string()]
        );

        // Lifetimes are not literals, and neither is an apostrophe in prose.
        assert!(string_literals("pub fn f<'a>(x: &'a str) -> &'a str { x }").is_empty());
        assert!(string_literals("//! it's fine, don't worry").is_empty());

        // Escapes decode to the value they compile to.
        assert_eq!(
            string_literals("pub const S: &str = \"\\x73qlite\";"),
            vec!["sqlite".to_string()]
        );
        assert_eq!(
            string_literals("pub const P: &str = \"/\\u{2f}tmp\";"),
            vec!["//tmp".to_string()]
        );
        // An escaped backslash is content, not an escape introducer.
        assert_eq!(
            string_literals("let p = \"C:\\\\tools\";"),
            vec!["C:\\tools".to_string()]
        );

        // Non-ASCII content survives extraction as itself.
        assert_eq!(
            string_literals("let s = \"groß/ünïcode\";"),
            vec!["groß/ünïcode"]
        );
    }

    /// Critic-6 C3's machinery: attribute spans come from the lexing pass,
    /// so a `#[` inside a comment or a string is not an attribute.
    #[test]
    fn attribute_spans_come_from_the_lexing_pass_not_substrings() {
        let real = "#[serde(rename = \"cross_cutting\")]\npub enum R {}\n";
        let expected_end = real.find("]\npub").map(|at| at + 1).unwrap_or_default();
        assert_eq!(
            attribute_spans(real),
            vec![0..expected_end],
            "{:?}",
            attribute_spans(real)
        );

        // A `#[` inside a comment is not an attribute.
        let commented = "let a = 1; // matches the #[review] attribute\n";
        assert!(
            attribute_spans(commented).is_empty(),
            "{:?}",
            attribute_spans(commented)
        );

        // Nor inside a string.
        let strung = "let name = \"#[path]\";\n";
        assert!(
            attribute_spans(strung).is_empty(),
            "{:?}",
            attribute_spans(strung)
        );

        // A bracket inside a comment or string does not close the body.
        let data = "#[cfg_attr(not(doc = \"]\"), path = \"../../../x.rs\")] mod m;\n";
        assert_eq!(
            bracket_end(data, 1),
            data.find("] mod m;").map(|at| at + 1).unwrap_or(0)
        );
        let commented_bracket = "#[cfg_attr(/* ] */ all(), path = \"../../../x.rs\")] mod m;\n";
        assert_eq!(
            bracket_end(commented_bracket, 1),
            commented_bracket
                .find("] mod m;")
                .map(|at| at + 1)
                .unwrap_or(0)
        );
    }
}
