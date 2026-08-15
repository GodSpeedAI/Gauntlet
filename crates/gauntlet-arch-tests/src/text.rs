//! Scan-unit normalization shared by both source scanners (recritique R6).
//!
//! Reading a file line by line is defeated by two ordinary-looking Rust
//! constructs that put a term back together at compile time:
//!
//! ```text
//! pub const MODEL: &str = "claud\
//! e-3-5-sonnet";                      // string line-continuation
//! pub const STORE: &str = concat!("sq", "lite");   // adjacent fragments
//! ```
//!
//! Neither physical line contains the compiled term. [`scan_units`] therefore
//! yields BOTH forms of every file:
//!
//! - the physical lines, exactly as today; and
//! - the *logical* lines, produced by collapsing `\` + newline + leading
//!   whitespace (Rust's own line-continuation rule) and by joining adjacent
//!   string-literal fragments separated by `","` (the `concat!` shape).
//!
//! Both are scanned because normalization can hide as well as reveal: joining
//! `"80", "80"` produces `8080`, but joining `"8080", "1"` produces `80801`,
//! which the port class would no longer see. A violation found in either form
//! is reported; duplicates are collapsed by the scanners.
//!
//! A logical line is reported at the line of its FIRST fragment, which is
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

/// Every scan unit for a file: physical lines first, then any logical line
/// that differs from its physical counterpart. Order is stable so violation
/// lists are reproducible.
pub fn scan_units(src: &str) -> Vec<ScanUnit> {
    let mut out = physical_lines(src);
    for unit in logical_lines(src) {
        if !out.contains(&unit) {
            out.push(unit);
        }
    }
    out
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

/// If `bytes[at]` (a `"`) closes a fragment that is immediately followed by
/// `,` and another `"`, the index just past that opening quote, plus the
/// number of newlines consumed.
fn literal_join_end(bytes: &[u8], at: usize) -> Option<(usize, usize)> {
    let mut newlines = 0usize;
    let mut j = at + 1;
    while matches!(bytes.get(j), Some(b' ' | b'\t' | b'\r' | b'\n')) {
        if bytes[j] == b'\n' {
            newlines += 1;
        }
        j += 1;
    }
    if bytes.get(j) != Some(&b',') {
        return None;
    }
    j += 1;
    while matches!(bytes.get(j), Some(b' ' | b'\t' | b'\r' | b'\n')) {
        if bytes[j] == b'\n' {
            newlines += 1;
        }
        j += 1;
    }
    if bytes.get(j) != Some(&b'"') {
        return None;
    }
    Some((j + 1, newlines))
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
}
