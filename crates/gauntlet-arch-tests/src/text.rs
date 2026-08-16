//! Scan-unit normalization shared by both source scanners (recritique R6,
//! round-3 V6/V11/V17).
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

/// Whether the line opens with a declaration keyword after any qualifier
/// tokens — `pub const`, `static`, `let`, `pub(crate) static`, … Doc comments
/// and prose never qualify: their first token is `//!` or a word, not one of
/// these.
fn declaration_head(trimmed: &str) -> bool {
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

/// Statement units: a line that ends on an open continuation token, or a
/// declaration head whose value sits on the NEXT line (critic-4 M1), joined
/// with the lines that follow it, up to and including the one carrying the
/// terminating `;`.
///
/// Both shapes exist because the timeout and tier-policy classes require the
/// identifier and its value in ONE unit: an ordinary rustfmt line break after
/// `=` used to split them (round-3 V17), and so did the split BEFORE the `=`
/// (`pub const X: u64` / `= 900_000;`), which rustc accepts just as happily
/// and which no open-continuation token ever saw (critic-4 M1). Joining stays
/// deliberately conservative — an open token, or a declaration head followed
/// by a line that begins with `=` or `:` — so ordinary prose and markdown
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
        if !opens_continuation(trimmed) && !awaits_value {
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
}
