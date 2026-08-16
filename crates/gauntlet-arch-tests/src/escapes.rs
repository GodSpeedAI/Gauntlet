//! Scope escapes — the second half of "what gets scanned must equal what
//! compiles" (recritique R4/R17/R25, round-3 V1/V2/V3, class A3).
//!
//! `crate::walk` guarantees every regular file *inside* a core crate reaches
//! a scanner. Rust and Cargo have five ways to compile a file from *outside*
//! the crate directory anyway, none of which the walk can see:
//!
//! ```text
//! #[path = "../../../shared/wiring.rs"] mod wiring;
//! #[cfg_attr(all(), path = "../../../vendor/store.rs")] mod store;
//! include!("../../../shared/wiring.rs");
//! const KEY: &str = include_str!("../../../shared/model-id.txt");
//! ```
//! ```toml
//! [lib]
//! path = "../../shared/core.rs"     # the crate ROOT, outside the crate
//! ```
//!
//! Each carries only a relative path, which no literal class flags, and the
//! target lives outside every scan scope. This module resolves each such
//! target against the file that declares it and reports any that escapes the
//! declaring crate's own directory.
//!
//! # Three rules learned the hard way (round 3)
//!
//! - **The attribute matcher is structural, not textual (V2).** It used to be
//!   the literal substring `"#[path"`, so `#[cfg_attr(all(), path = "…")]`
//!   and `#[ path = "…"]` — both spellings rustc accepts — matched nothing.
//!   [`scan_escapes`] now finds any attribute whose body assigns a string to a
//!   `path` key, `cfg_attr` included.
//! - **Unresolvable is a violation, never "inside" (V3).** The old resolver
//!   took the first double-quoted string after the directive, so
//!   `include!(concat!(env!("OUT_DIR"), "/gen.rs"))` resolved to the harmless
//!   in-crate path `<crate>/src/OUT_DIR` and reported nothing — while a
//!   `build.rs` copied arbitrary code into `OUT_DIR` and compiled it into the
//!   crate, under `target/`, outside every walk root. An argument that is not
//!   a single plain string literal is now UNRESOLVABLE, and unresolvable is a
//!   violation (I8: missing evidence is not passing evidence). The companion
//!   rule — a `build.rs` in a core crate is itself a violation — lives in
//!   [`build_script_violation`].
//! - **Cargo target paths are source paths (V1).** `[lib].path`,
//!   `[[bin]].path`, `[[test]].path`, `[[bench]].path` and `[[example]].path`
//!   point the compiler at a file the walk never lists. See
//!   [`scan_cargo_target_paths`] and [`TARGET_PATH_EXEMPT_CRATES`].
//!
//! False-positive posture: an in-crate `#[path = "sub/mod.rs"]` or
//! `include_str!("fixture.txt")` resolves inside the crate directory and is
//! not reported — those files are already walked and scanned. Only escapes,
//! absolute targets, and unresolvable targets are violations.

use toml::Table;

/// `include`-family directives whose string argument names a file the
/// compiler will read. Carried WITHOUT the `!`: rustc tokenizes `include !`
/// as the same call (critic-4 m3), so the matcher tolerates whitespace
/// between the word, the `!`, and the `(`.
const INCLUDE_DIRECTIVES: &[&str] = &["include", "include_str", "include_bytes"];

/// Cargo manifest sections whose `path` key relocates a compilation target's
/// source root.
const TARGET_SECTIONS: &[&str] = &["lib", "bin", "test", "bench", "example"];

/// Crates exempt from the Cargo-target-path rule, each with the reason it is
/// exempt. Stated here rather than left implicit, because this channel is the
/// one the boundary suite itself uses (round-3 V1).
///
/// `crates/gauntlet-arch-tests` declares `[[test]] path =
/// "../../tests/contract/…"` on purpose: the frozen answer key addresses this
/// work at repo-root `tests/contract/<name>`, so those files must live there
/// and be reachable as cargo test targets. The suite is not part of the
/// product, depends on no internal crate, is never a dependency of one, and is
/// outside `walk::CORE_CRATES` — so relocating its own test targets cannot
/// smuggle code into a scanned layer. Every crate the rule *does* police is a
/// core crate, where a relocated target root means the compiler and the
/// scanners are looking at different files.
pub const TARGET_PATH_EXEMPT_CRATES: &[&str] = &["crates/gauntlet-arch-tests"];

/// Whether the Cargo-target-path rule applies to a crate directory. See
/// [`TARGET_PATH_EXEMPT_CRATES`] for the one exemption and its justification.
pub fn target_path_rule_applies(crate_dir: &str) -> bool {
    !TARGET_PATH_EXEMPT_CRATES.contains(&crate_dir)
}

/// A directive's argument: either a single plain string literal, or something
/// the scanner cannot resolve to one file.
#[derive(Debug, PartialEq, Eq)]
enum Argument {
    Literal(String),
    /// Macro calls, `env!`/`concat!`, identifiers — anything whose value is
    /// decided at compile time by something other than this text.
    Unresolvable(String),
}

/// Report every `#[path]`/`include*!` target in `text` that resolves outside
/// `crate_dir`, is absolute, or cannot be resolved at all. `path` is the
/// repo-relative path of the declaring file and `crate_dir` its crate's
/// repo-relative directory.
pub fn scan_escapes(path: &str, crate_dir: &str, text: &str) -> Vec<String> {
    let declaring_dir = parent_dir(path);
    let mut out = scan_unterminated(path, text);
    for unit in crate::text::scan_units(text) {
        let raw = unit.text.as_str();
        for (directive, argument) in directive_arguments(raw) {
            report(
                &mut out,
                path,
                crate_dir,
                &declaring_dir,
                unit.line,
                &directive,
                argument,
            );
        }
    }
    out.dedup();
    out
}

fn report(
    out: &mut Vec<String>,
    path: &str,
    crate_dir: &str,
    declaring_dir: &str,
    line: usize,
    directive: &str,
    argument: Argument,
) {
    let target = match argument {
        Argument::Unresolvable(snippet) => {
            out.push(format!(
                "scope-escape: {path}:{line}: `{directive}` takes the argument `{snippet}`, which \
                 is not a single plain string literal — the file it names is decided at compile \
                 time (a build script writing into OUT_DIR, for instance), so no boundary scanner \
                 can read what the compiler compiles. Unresolvable is a violation, not a pass \
                 (I8, spec §24.3)"
            ));
            return;
        }
        Argument::Literal(target) => target,
    };
    if target.is_empty() {
        return;
    }
    if is_absolute(&target) {
        out.push(format!(
            "scope-escape: {path}:{line}: `{directive}` names the absolute path '{target}' — the \
             compiler reads it, no scanner does (spec §24.3)"
        ));
        return;
    }
    let resolved = normalize_join(declaring_dir, &target);
    let inside = resolved == crate_dir || resolved.starts_with(&format!("{crate_dir}/"));
    if !inside {
        out.push(format!(
            "scope-escape: {path}:{line}: `{directive}` pulls '{target}' (resolved: '{resolved}') \
             into {crate_dir} from outside the crate directory — it compiles into a core crate but \
             no boundary scanner ever reads it (spec §24.3)"
        ));
    }
}

/// Every `(directive, argument)` pair in one scan unit: `include*!` calls and
/// `path = "…"` keys inside any attribute, `cfg_attr` included.
///
/// Both matchers tolerate whitespace where rustc does (critic-4 m3):
/// `include ! ("…")` and `# [path = "…"]` are the same call and attribute to
/// rustc, and used to match nothing here.
fn directive_arguments(raw: &str) -> Vec<(String, Argument)> {
    let mut out = Vec::new();
    let bytes = raw.as_bytes();
    // The include family: `include`, `include_str`, `include_bytes`, followed
    // (after any whitespace) by `!`, then `(`.
    let mut from = 0usize;
    while let Some(at) = find_ascii_ci(raw, "include", from) {
        let word_end = at + "include".len() + include_word_tail(bytes, at + "include".len());
        from = word_end;
        let preceded_by_word =
            at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
        if preceded_by_word {
            continue;
        }
        let word = raw[at..word_end].to_lowercase();
        if !INCLUDE_DIRECTIVES.contains(&word.as_str()) {
            continue;
        }
        let mut j = skip_space(bytes, word_end);
        if bytes.get(j) != Some(&b'!') {
            continue;
        }
        j = skip_space(bytes, j + 1);
        if let Some(argument) = include_argument(raw, j) {
            out.push((word, argument));
        }
    }
    // Attributes: `#` (after any whitespace) `[`, body closed structurally.
    let mut from = 0usize;
    while let Some(at) = raw[from..].find('#').map(|found| from + found) {
        from = at + 1;
        let open = skip_space(bytes, at + 1);
        if bytes.get(open) != Some(&b'[') {
            continue;
        }
        from = open + 1;
        let end = attribute_body_end(bytes, open);
        for argument in attribute_path_arguments(&raw[at..end]) {
            out.push(("[path]".to_string(), argument));
        }
    }
    out
}

/// The length of the identifier tail after the `include` prefix, so
/// `include_str` and `included` (prose) are told apart.
fn include_word_tail(bytes: &[u8], from: usize) -> usize {
    let mut end = from;
    while end < bytes.len() && (bytes[end].is_ascii_alphanumeric() || bytes[end] == b'_') {
        end += 1;
    }
    end - from
}

/// Whole-FILE ambiguity, reported so that unscannable input can never read as
/// clean input (I8, critic-5 C3).
///
/// The unit scanners see physical lines, so an attribute or a string that
/// legitimately spans lines looks unbalanced inside a single unit; that is an
/// artifact of splitting, and the bracket scanner already fails SAFE there by
/// treating the whole remaining text as the attribute body (so a `path` key in
/// the tail is still read). What cannot be an artifact is a file that ends
/// inside a string literal or with an unclosed `[`: valid Rust never does
/// that, so either the file is not the Rust the compiler sees, or this
/// scanner's own literal state machine lost synchronization. Both mean the
/// evidence is missing rather than negative.
///
/// Restricted to `.rs` files: `[` in a Markdown or TOML file is prose or a
/// table header, not structure this scanner claims to understand.
fn scan_unterminated(path: &str, text: &str) -> Vec<String> {
    if !path.ends_with(".rs") {
        return Vec::new();
    }
    let bytes = text.as_bytes();
    let mut out = Vec::new();
    let mut brackets: Vec<usize> = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        match bytes[i] {
            b'/' if bytes.get(i + 1) == Some(&b'/') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
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
                        i += 1;
                    }
                }
                if depth > 0 {
                    out.push(format!(
                        "scope-escape: {path}: the file ends inside a `/* … */` comment — the \
                         scanners cannot delimit what follows it, and unscannable input is a \
                         violation, not a pass (I8, spec §24.3)"
                    ));
                }
            }
            b'"' => {
                let end = plain_string_end(bytes, i + 1);
                if end >= bytes.len() && !ends_with_quote(bytes) {
                    out.push(unterminated_string(path, line_of(text, i)));
                    return out;
                }
                i = end;
            }
            b'r' | b'b' => match prefixed_string_end(bytes, i) {
                Some(end) if end >= bytes.len() && !ends_with_quote(bytes) => {
                    out.push(unterminated_string(path, line_of(text, i)));
                    return out;
                }
                Some(end) => i = end,
                None => i += 1,
            },
            b'\'' => match char_literal_end(bytes, i) {
                Some(end) => i = end,
                None => i += 1,
            },
            b'[' => {
                brackets.push(i);
                i += 1;
            }
            b']' => {
                if brackets.pop().is_none() {
                    out.push(format!(
                        "scope-escape: {path}:{}: a `]` closes a bracket that was never opened — \
                         the file is not the Rust the compiler reads, or this scanner's literal \
                         state lost synchronization; either way the evidence is missing, not \
                         negative (I8, spec §24.3)",
                        line_of(text, i)
                    ));
                }
                i += 1;
            }
            _ => i += 1,
        }
    }
    for open in brackets {
        out.push(format!(
            "scope-escape: {path}:{}: a `[` is never closed — an attribute body that does not \
             terminate cannot be delimited, so a `path = \"…\"` key inside it cannot be read; \
             unscannable input is a violation, not a pass (I8, spec §24.3)",
            line_of(text, open)
        ));
    }
    out
}

fn unterminated_string(path: &str, line: usize) -> String {
    format!(
        "scope-escape: {path}:{line}: a string literal is never closed — everything after it is \
         data to this scanner and code to the compiler, so no boundary check on the rest of the \
         file means anything (I8, spec §24.3)"
    )
}

/// Whether the text's last byte closes the literal that reached the end, so a
/// string ending exactly at EOF is not misread as unterminated.
fn ends_with_quote(bytes: &[u8]) -> bool {
    matches!(bytes.last(), Some(b'"' | b'#'))
}

fn line_of(text: &str, at: usize) -> usize {
    text[..at.min(text.len())]
        .bytes()
        .filter(|b| *b == b'\n')
        .count()
        + 1
}

/// The index just past the `]` that closes the attribute opened at `open`
/// (the `[`), or the end of the text if it is unbalanced.
///
/// Brackets inside string and char literals are DATA, not structure (critic-5
/// C3): `#[cfg_attr(not(doc = "]"), path = "../../../vendor/evil.rs")]` is a
/// spelling rustc happily compiles (verified: the `cfg` is false, so `not(…)`
/// applies the `path` key), and the plain depth counter used to return just
/// past the `]` inside the predicate string — the `path = …` tail was never
/// scanned and the escape was invisible. The scanner therefore enters
/// string-literal state for `"…"` (with `\` escapes), raw strings (`r#"…"#`,
/// `br##"…"##`), byte strings (`b"…"`), and char literals (`']'`, `'\\''`),
/// none of which can change bracket depth. A `'` that does not close a char
/// literal (a lifetime, an apostrophe in prose) is treated as an ordinary
/// byte, so lifetimes never desynchronize the scan.
fn attribute_body_end(bytes: &[u8], open: usize) -> usize {
    let mut depth = 0usize;
    let mut i = open;
    while i < bytes.len() {
        match bytes[i] {
            b'"' => {
                i = plain_string_end(bytes, i + 1);
                continue;
            }
            b'\'' => {
                if let Some(end) = char_literal_end(bytes, i) {
                    i = end;
                    continue;
                }
            }
            b'r' | b'b' => {
                if let Some(end) = prefixed_string_end(bytes, i) {
                    i = end;
                    continue;
                }
            }
            b'[' => depth += 1,
            b']' => {
                depth -= 1;
                if depth == 0 {
                    return i + 1;
                }
            }
            _ => {}
        }
        i += 1;
    }
    bytes.len()
}

/// The index just past the closing quote of a plain (or byte) string whose
/// body starts at `at` (just past the opening `"`). `\` escapes the next
/// byte, `\"` included. An unterminated string consumes the rest of the text.
fn plain_string_end(bytes: &[u8], at: usize) -> usize {
    let mut i = at;
    while i < bytes.len() {
        match bytes[i] {
            b'\\' => i += 2,
            b'"' => return i + 1,
            _ => i += 1,
        }
    }
    bytes.len()
}

/// The index just past a char literal starting at `at` (the `'`), if one does.
/// `']'`, `'"'`, `'\\''`, and multi-byte escape bodies are skipped whole; a
/// `'` followed by something that is not a char-literal shape (a lifetime
/// `'a`, an apostrophe in prose) is not a literal and yields `None`.
fn char_literal_end(bytes: &[u8], at: usize) -> Option<usize> {
    match bytes.get(at + 1) {
        Some(b'\\') => (at + 2..bytes.len())
            .find(|&i| bytes[i] == b'\'')
            .map(|i| i + 1),
        Some(second) if bytes.get(at + 2) == Some(&b'\'') && *second != b'\'' => Some(at + 3),
        _ => None,
    }
}

/// The index just past a raw (`r#"…"#`) or byte-raw (`br#"…"#`) string
/// starting at `at`, if one does. The `r`/`b` must start a word of its own —
/// an `r` at the tail of an identifier (`for_r`) is not a string opener.
fn prefixed_string_end(bytes: &[u8], at: usize) -> Option<usize> {
    if at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_') {
        return None;
    }
    let mut j = at;
    if bytes[j] == b'b' {
        j += 1;
    }
    if bytes.get(j) != Some(&b'r') {
        // `b"…"` is a plain byte string; the caller's `"` handling does not
        // see it because it starts at `b`, so consume it here.
        return (bytes.get(j) == Some(&b'"')).then(|| plain_string_end(bytes, j + 1));
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
    let mut i = j + 1;
    while i < bytes.len() {
        if bytes[i] == b'"' && (0..hashes).all(|h| bytes.get(i + 1 + h) == Some(&b'#')) {
            return Some(i + 1 + hashes);
        }
        i += 1;
    }
    Some(bytes.len())
}

/// Every `path = <argument>` key inside an attribute body. This is the
/// structural form of the old `"#[path"` substring match: `#[path = "…"]`,
/// `#[ path = "…"]`, and `#[cfg_attr(all(), path = "…")]` all reach it
/// (round-3 V2).
fn attribute_path_arguments(body: &str) -> Vec<Argument> {
    let bytes = body.as_bytes();
    let mut out = Vec::new();
    let mut from = 0usize;
    while let Some(at) = find_ascii_ci(body, "path", from) {
        from = at + 4;
        let before_is_word =
            at > 0 && (bytes[at - 1].is_ascii_alphanumeric() || bytes[at - 1] == b'_');
        let after_is_word = bytes
            .get(at + 4)
            .is_some_and(|b| b.is_ascii_alphanumeric() || *b == b'_');
        if before_is_word || after_is_word {
            continue;
        }
        let mut j = skip_space(bytes, at + 4);
        if bytes.get(j) != Some(&b'=') {
            continue;
        }
        j = skip_space(bytes, j + 1);
        out.push(argument_at(body, j));
    }
    out
}

/// The argument of an `include*!` call whose name ends at `from`. `None` when
/// the directive is not actually a call (a doc comment naming `include!`
/// without a target), so prose stays clean.
fn include_argument(raw: &str, from: usize) -> Option<Argument> {
    let bytes = raw.as_bytes();
    let open = skip_space(bytes, from);
    if bytes.get(open) != Some(&b'(') {
        return None;
    }
    Some(argument_at(raw, skip_space(bytes, open + 1)))
}

/// Classify whatever starts at `at` as a plain string literal or as
/// unresolvable. Round-3 V3: `concat!`, `env!`, an identifier, or a `format!`
/// are all "the compiler decides", and that is a violation rather than a pass.
fn argument_at(raw: &str, at: usize) -> Argument {
    let bytes = raw.as_bytes();
    if let Some((open, hashes)) = string_literal_open(bytes, at) {
        return match read_string_literal(raw, open, hashes) {
            Some(value) => Argument::Literal(value),
            None => Argument::Unresolvable(snippet(raw, at)),
        };
    }
    Argument::Unresolvable(snippet(raw, at))
}

/// The opening of a string literal at `at`: `"`, `r"`, `r#"`, `r##"`, … as
/// `(index just past the quote, number of hashes)`.
fn string_literal_open(bytes: &[u8], at: usize) -> Option<(usize, usize)> {
    if bytes.get(at) == Some(&b'"') {
        return Some((at + 1, 0));
    }
    if bytes.get(at) != Some(&b'r') {
        return None;
    }
    let mut k = at + 1;
    while bytes.get(k) == Some(&b'#') {
        k += 1;
    }
    (bytes.get(k) == Some(&b'"')).then_some((k + 1, k - at - 1))
}

/// Read a string literal body starting just past its opening quote. `\\` and
/// `\"` are unescaped (enough for a path); a raw literal is read verbatim up
/// to its closing quote plus `hashes` hashes.
fn read_string_literal(raw: &str, open: usize, hashes: usize) -> Option<String> {
    let bytes = raw.as_bytes();
    let mut i = open;
    let mut out = String::new();
    while i < bytes.len() {
        if bytes[i] == b'"' {
            let closed = (0..hashes).all(|h| bytes.get(i + 1 + h) == Some(&b'#'));
            if closed {
                return Some(out);
            }
        }
        if hashes == 0 && bytes[i] == b'\\' && i + 1 < bytes.len() {
            out.push(bytes[i + 1] as char);
            i += 2;
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    None
}

fn snippet(raw: &str, at: usize) -> String {
    let tail = &raw[at.min(raw.len())..];
    let end = tail
        .char_indices()
        .take_while(|(index, _)| *index < 48)
        .last()
        .map(|(index, ch)| index + ch.len_utf8())
        .unwrap_or(0);
    tail[..end].trim_end().to_string()
}

fn skip_space(bytes: &[u8], mut at: usize) -> usize {
    while matches!(bytes.get(at), Some(b' ' | b'\t' | b'\r' | b'\n')) {
        at += 1;
    }
    at
}

/// ASCII-case-insensitive substring search. Used instead of lowercasing the
/// unit, because `to_lowercase` is not length-preserving for every Unicode
/// input and the returned index is used to slice the ORIGINAL text.
fn find_ascii_ci(haystack: &str, needle: &str, from: usize) -> Option<usize> {
    let hay = haystack.as_bytes();
    let pin = needle.as_bytes();
    if pin.is_empty() || from >= hay.len() || hay.len() - from < pin.len() {
        return None;
    }
    (from..=hay.len() - pin.len()).find(|start| {
        hay[*start..*start + pin.len()]
            .iter()
            .zip(pin)
            .all(|(a, b)| a.eq_ignore_ascii_case(b))
    })
}

/// Report any Cargo target whose `path` relocates a compilation unit's source
/// root outside the crate directory (round-3 V1). `[lib]` is a table;
/// `[[bin]]`, `[[test]]`, `[[bench]]` and `[[example]]` are arrays of tables.
pub fn scan_cargo_target_paths(manifest_path: &str, crate_dir: &str, text: &str) -> Vec<String> {
    if !target_path_rule_applies(crate_dir) {
        return Vec::new();
    }
    let Ok(table) = text.parse::<Table>() else {
        return vec![format!(
            "scope-escape: {manifest_path}: malformed Cargo.toml — the target source paths \
             ([lib].path, [[bin]].path, …) cannot be checked, and an unreadable manifest is \
             missing evidence, not a pass (I8)"
        )];
    };
    let mut out = Vec::new();
    for section in TARGET_SECTIONS {
        let Some(value) = table.get(*section) else {
            continue;
        };
        let entries: Vec<&Table> = match value {
            toml::Value::Table(one) => vec![one],
            toml::Value::Array(many) => many.iter().filter_map(|v| v.as_table()).collect(),
            _ => continue,
        };
        for entry in entries {
            let Some(rel) = entry.get("path").and_then(|v| v.as_str()) else {
                continue;
            };
            if is_absolute(rel) {
                out.push(format!(
                    "scope-escape: {manifest_path}: [{section}] path = '{rel}' is absolute — the \
                     compiler reads it, no scanner does (spec §24.3)"
                ));
                continue;
            }
            let resolved = normalize_join(crate_dir, rel);
            if resolved != crate_dir && !resolved.starts_with(&format!("{crate_dir}/")) {
                out.push(format!(
                    "scope-escape: {manifest_path}: [{section}] path = '{rel}' (resolved: \
                     '{resolved}') points a compilation target's source root outside {crate_dir} \
                     — the walk scans a file that is no longer compiled and the compiler compiles \
                     a file nothing scans (spec §24.3, AK-001)"
                ));
            }
        }
    }
    // A build script is a code-injection channel with no scannable output:
    // whatever it writes into OUT_DIR compiles into the crate from under
    // `target/`, outside every walk root (round-3 V3).
    if let Some(build) = table.get("package").and_then(|p| p.get("build"))
        && build.as_bool() != Some(false)
    {
        out.push(format!(
            "scope-escape: {manifest_path}: [package] declares a build script (build = {build}) — \
             a core crate has no legitimate codegen need, and generated code compiles in from \
             under target/ where no boundary scanner reads it (spec §24.3, AK-001)"
        ));
    }
    out
}

/// A `build.rs` at a core crate's root is a violation in its own right — Cargo
/// picks it up without any manifest key, and it can copy arbitrary code into
/// `OUT_DIR` for an `include!` to compile (round-3 V3).
pub fn build_script_violation(rel: &str, crate_dir: &str) -> Option<String> {
    (rel == format!("{crate_dir}/build.rs")).then(|| {
        format!(
            "scope-escape: {rel}: a build script in a core crate — Cargo compiles and runs it \
             without any manifest key, and anything it writes into OUT_DIR compiles into \
             {crate_dir} from under target/, where no boundary scanner reads it. Core crates have \
             no legitimate codegen need (spec §24.3, AK-001)"
        )
    })
}

fn is_absolute(target: &str) -> bool {
    target.starts_with('/')
        || target
            .as_bytes()
            .first()
            .is_some_and(|b| b.is_ascii_alphabetic())
            && target.as_bytes().get(1) == Some(&b':')
            && matches!(target.as_bytes().get(2), Some(b'\\') | Some(b'/'))
}

fn parent_dir(path: &str) -> String {
    match path.rfind('/') {
        Some(at) => path[..at].to_string(),
        None => String::new(),
    }
}

/// Join `rel` onto `base` (both POSIX, repo-relative) and resolve `.`/`..`.
/// A `..` that walks above the repo root yields a path starting with `..`,
/// which can never be inside a crate directory.
fn normalize_join(base: &str, rel: &str) -> String {
    let mut parts: Vec<&str> = base.split('/').filter(|p| !p.is_empty()).collect();
    let normalized = rel.replace('\\', "/");
    for segment in normalized.split('/') {
        match segment {
            "" | "." => {}
            ".." => {
                if parts.last().is_some_and(|p| *p != "..") {
                    parts.pop();
                } else {
                    parts.push("..");
                }
            }
            other => parts.push(other),
        }
    }
    parts.join("/")
}

#[cfg(test)]
mod tests {
    use super::*;

    const APP: &str = "crates/gauntlet-app";
    const APP_LIB: &str = "crates/gauntlet-app/src/lib.rs";
    const DOMAIN: &str = "crates/gauntlet-domain";

    #[test]
    fn path_attribute_escaping_the_crate_is_flagged() {
        // Recritique R4's exact evasion.
        let fixture = "#[path = \"../../../shared/wiring.rs\"]\nmod wiring;\n";
        let violations = scan_escapes(APP_LIB, APP, fixture);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(
            violations[0].starts_with("scope-escape: crates/gauntlet-app/src/lib.rs:1:"),
            "{}",
            violations[0]
        );
        assert!(violations[0].contains("shared/wiring.rs"), "{violations:?}");
    }

    #[test]
    fn every_rustc_spelling_of_the_path_attribute_is_flagged() {
        // Round-3 V2's exact evasions: the matcher used to be the literal
        // substring `"#[path"`, and both of these are valid rustc syntax that
        // compiles a file from outside the crate.
        for fixture in [
            "#[cfg_attr(all(), path = \"../../../vendor/store.rs\")]\nmod store;\n",
            "#[ path = \"../../../vendor/store.rs\"] mod store;\n",
            "#[path=\"../../../vendor/store.rs\"] mod store;\n",
            "#[PATH = \"../../../vendor/store.rs\"] mod store;\n",
            "#[cfg_attr(target_os = \"linux\", path = \"../../../vendor/store.rs\")] mod store;\n",
        ] {
            let violations = scan_escapes(APP_LIB, APP, fixture);
            assert!(
                violations.iter().any(|v| v.contains("vendor/store.rs")),
                "{fixture} -> {violations:?}"
            );
        }
    }

    #[test]
    fn whitespace_split_directive_spellings_are_flagged() {
        // Critic-4 m3: rustc tokenizes `#` `[` and `include` `!` `(` as
        // separate tokens, so `# [path = …]` and `include ! (…)` are the
        // same constructs to the compiler — and used to match nothing here.
        for fixture in [
            "# [path = \"../../../vendor/store.rs\"] mod store;\n",
            "#\t[path = \"../../../vendor/store.rs\"] mod store;\n",
            "include ! (\"../../../vendor/store.rs\");\n",
            "include_str ! (\"../../../vendor/leak.txt\");\n",
            "include\t!\t(\"../../../vendor/wiring.rs\");\n",
        ] {
            let violations = scan_escapes(APP_LIB, APP, fixture);
            assert!(
                violations.iter().any(|v| v.contains("vendor/")),
                "{fixture} -> {violations:?}"
            );
        }
        // The in-crate spelling of the same whitespace forms stays clean, and
        // prose mentioning `include` without a call still does not fire.
        for fixture in [
            "include ! (\"fixture.txt\");\n",
            "//! A doc comment mentioning include and # and [ separately.\n",
            "let included = included_count();\n",
        ] {
            let violations = scan_escapes(APP_LIB, APP, fixture);
            assert!(violations.is_empty(), "{fixture} -> {violations:?}");
        }
    }

    /// Critic-5 C3: a `]` that is DATA used to close the attribute body early,
    /// so `attribute_path_arguments` never saw the `path` key and the outer
    /// loop resumed past the `[` without re-reading the tail. The bracket
    /// scanner is string-aware now, in every literal spelling.
    #[test]
    fn a_bracket_inside_a_literal_does_not_close_the_attribute_body() {
        for fixture in [
            // The critic's own fixture: `doc = "]"` is false outside rustdoc,
            // so `not(…)` holds and the `path` key applies.
            "#[cfg_attr(not(doc = \"]\"), path = \"../../../vendor/evil.rs\")]\nmod evil;\n",
            // Two brackets of data, and the closing one inside the string too.
            "#[cfg_attr(not(doc = \"[]\"), path = \"../../../vendor/evil.rs\")]\nmod evil;\n",
            // A raw string carrying the same byte.
            "#[cfg_attr(not(doc = r\"]\"), path = \"../../../vendor/evil.rs\")]\nmod evil;\n",
            // A hashed raw string, where the naive scanner also loses the quote.
            "#[cfg_attr(not(doc = r#\"]\"#), path = \"../../../vendor/evil.rs\")]\nmod evil;\n",
            // A char literal, and a byte string.
            "#[cfg_attr(not(doc = \"x\"), path = \"../../../vendor/evil.rs\")] // ']'\nmod evil;\n",
            "#[cfg_attr(not(doc = b\"]\"), path = \"../../../vendor/evil.rs\")]\nmod evil;\n",
            // An escaped quote inside the predicate, so the string state must
            // honour `\` escapes to find the real end.
            "#[cfg_attr(not(doc = \"\\\"]\"), path = \"../../../vendor/evil.rs\")]\nmod evil;\n",
        ] {
            let violations = scan_escapes(APP_LIB, APP, fixture);
            assert!(
                violations.iter().any(|v| v.contains("vendor/evil.rs")),
                "{fixture} -> {violations:?}"
            );
        }
    }

    /// The negative half: the same string-aware scan must not invent
    /// violations for ordinary code, or the class would be a false-positive
    /// factory rather than a check. The real tree is covered by
    /// `core_crates_have_no_scope_escapes_or_unscannable_files`.
    #[test]
    fn literals_and_lifetimes_do_not_produce_phantom_escapes() {
        for fixture in [
            "#[serde(rename = \"cross_cutting\")]\npub enum Radius {}\n",
            "pub fn split(s: &str) -> Vec<&str> { s.split(']').collect() }\n",
            "pub fn brackets<'a>(x: &'a [u8]) -> &'a [u8] { &x[1..] }\n",
            "pub const DOC: &str = r#\"a ] and a [ inside a raw string\"#;\n",
            "//! Prose with an unmatched ' apostrophe and a \" quote inside a comment.\n",
            "/* a block comment with [ and \" in it */\npub fn f() {}\n",
            "pub const B: &[u8] = b\"[]\";\n",
        ] {
            let violations = scan_escapes(APP_LIB, APP, fixture);
            assert!(violations.is_empty(), "{fixture} -> {violations:?}");
        }
    }

    /// I8: input this scanner cannot delimit is a violation, not a pass. Both
    /// shapes are impossible in code rustc accepts, which is exactly why a
    /// scanner that reported "clean" for them would be reporting nothing.
    #[test]
    fn unterminated_input_is_a_violation_not_a_skip() {
        for (fixture, needle) in [
            (
                "#[cfg_attr(not(doc = \"]\"), path = \"../../../vendor/evil.rs\"\nmod evil;\n",
                "never closed",
            ),
            (
                "pub const S: &str = \"unterminated\nmod evil;\n",
                "string literal is never closed",
            ),
            (
                "pub fn f() {}\n/* an unterminated block comment\n",
                "inside a `/* … */` comment",
            ),
        ] {
            let violations = scan_escapes(APP_LIB, APP, fixture);
            assert!(
                violations.iter().any(|v| v.contains(needle)),
                "{fixture} -> {violations:?}"
            );
        }
        // The same bytes in a non-Rust file are prose, not structure: the
        // unterminated scan is scoped to `.rs` so a Markdown `[` never fires.
        assert!(
            scan_escapes(
                "crates/gauntlet-app/README.md",
                APP,
                "A sentence with an unmatched [ bracket and a \" quote.\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn attributes_without_a_path_key_are_not_flagged() {
        for fixture in [
            "#[serde(rename = \"cross_cutting\")]\npub enum Radius {}\n",
            "#[cfg(feature = \"path\")]\nmod x;\n",
            "#[doc(alias = \"path\")]\npub fn f() {}\n",
            "#[derive(Debug, Clone)]\npub struct S;\n",
        ] {
            let violations = scan_escapes(APP_LIB, APP, fixture);
            assert!(violations.is_empty(), "{fixture} -> {violations:?}");
        }
    }

    #[test]
    fn include_forms_escaping_the_crate_are_flagged() {
        // Recritique R4/R25: include!, include_str!, include_bytes!.
        for fixture in [
            "include!(\"../../../shared/wiring.rs\");",
            "const KEY: &str = include_str!(\"../../../shared/model-id.txt\");",
            "const BLOB: &[u8] = include_bytes!(\"../../../vendor/blob.bin\");",
            "#[path = \"../../../vendor/leak.rs\"] mod leak;",
            "#[path = \"../../../vendor/store.rs\"] mod store;",
        ] {
            let violations = scan_escapes(APP_LIB, APP, fixture);
            assert_eq!(violations.len(), 1, "{fixture} -> {violations:?}");
            assert!(
                violations[0].starts_with("scope-escape: "),
                "{}",
                violations[0]
            );
        }
    }

    #[test]
    fn unresolvable_include_targets_are_violations_not_passes() {
        // Round-3 V3's exact evasion: `first_string_after` grabbed `OUT_DIR`,
        // joined it to the crate dir, and decided "inside".
        for fixture in [
            "include!(concat!(env!(\"OUT_DIR\"), \"/wiring.rs\"));",
            "include!(concat!(env!(\"OUT_DIR\"), \"/gen.rs\"));",
            "const T: &str = include_str!(env!(\"GAUNTLET_TEMPLATE\"));",
            "include_bytes!(BLOB_PATH);",
            "include!(concat!(\"../\", \"../vendor/store.rs\"));",
        ] {
            let violations = scan_escapes(APP_LIB, APP, fixture);
            assert!(!violations.is_empty(), "{fixture} -> {violations:?}");
            assert!(
                violations
                    .iter()
                    .all(|v| v.contains("not a single plain string literal")),
                "{fixture} -> {violations:?}"
            );
        }
        // And the same shape in a path attribute.
        let attribute = "#[cfg_attr(all(), path = concat!(env!(\"OUT_DIR\"), \"/s.rs\"))] mod s;";
        let violations = scan_escapes(APP_LIB, APP, attribute);
        assert!(
            violations
                .iter()
                .any(|v| v.contains("not a single plain string literal")),
            "{violations:?}"
        );
    }

    #[test]
    fn absolute_include_targets_are_flagged() {
        let violations = scan_escapes(APP_LIB, APP, "include!(\"/opt/shared/wiring.rs\");");
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("absolute path"), "{violations:?}");
    }

    #[test]
    fn escapes_split_across_lines_are_flagged() {
        // The continuation form of the same evasion (recritique R6 shape).
        let fixture = "#[path = \"../../../ven\\\n    dor/leak.rs\"]\nmod leak;\n";
        let violations = scan_escapes(APP_LIB, APP, fixture);
        assert!(
            violations.iter().any(|v| v.contains("vendor/leak.rs")),
            "{violations:?}"
        );
    }

    #[test]
    fn in_crate_paths_and_includes_are_not_flagged() {
        for fixture in [
            "#[path = \"engine/mod.rs\"]\nmod engine;\n",
            "const T: &str = include_str!(\"../fixtures/plan.txt\");\n",
            "include!(\"generated/table.rs\");\n",
            "const T: &str = include_str!(r\"fixture.txt\");\n",
            "//! A doc comment mentioning include! without a target.\n",
            "let s = format!(\"{}\", x);\n",
        ] {
            let violations = scan_escapes(APP_LIB, APP, fixture);
            assert!(violations.is_empty(), "{fixture} -> {violations:?}");
        }
    }

    #[test]
    fn cargo_target_paths_escaping_the_crate_are_flagged() {
        // Round-3 V1's exact evasion: `[lib] path` points the crate ROOT
        // outside the crate, so the scanned file is not compiled and the
        // compiled file is not scanned.
        let fixture =
            "[package]\nname = \"gauntlet-domain\"\n\n[lib]\npath = \"../../shared/core.rs\"\n";
        let violations =
            scan_cargo_target_paths("crates/gauntlet-domain/Cargo.toml", DOMAIN, fixture);
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].contains("shared/core.rs"), "{violations:?}");
        assert!(violations[0].contains("[lib]"), "{violations:?}");

        for (section, body) in [
            (
                "bin",
                "[[bin]]\nname = \"x\"\npath = \"../../shared/main.rs\"\n",
            ),
            (
                "test",
                "[[test]]\nname = \"x\"\npath = \"../../shared/t.rs\"\n",
            ),
            (
                "bench",
                "[[bench]]\nname = \"x\"\npath = \"../../shared/b.rs\"\n",
            ),
            (
                "example",
                "[[example]]\nname = \"x\"\npath = \"../../shared/e.rs\"\n",
            ),
        ] {
            let violations =
                scan_cargo_target_paths("crates/gauntlet-domain/Cargo.toml", DOMAIN, body);
            assert_eq!(violations.len(), 1, "{section} -> {violations:?}");
            assert!(
                violations[0].contains(&format!("[{section}]")),
                "{section} -> {violations:?}"
            );
        }

        // An absolute target path is a violation too.
        let absolute = "[lib]\npath = \"/opt/shared/core.rs\"\n";
        let violations =
            scan_cargo_target_paths("crates/gauntlet-domain/Cargo.toml", DOMAIN, absolute);
        assert!(violations[0].contains("absolute"), "{violations:?}");
    }

    #[test]
    fn in_crate_cargo_target_paths_are_not_flagged() {
        for fixture in [
            "[lib]\npath = \"src/lib.rs\"\n",
            "[[bin]]\nname = \"gauntlet\"\npath = \"src/bin/main.rs\"\n",
            "[package]\nname = \"gauntlet-domain\"\n",
            "[dependencies]\ngauntlet-ports = { path = \"../gauntlet-ports\" }\n",
        ] {
            let violations =
                scan_cargo_target_paths("crates/gauntlet-domain/Cargo.toml", DOMAIN, fixture);
            assert!(violations.is_empty(), "{fixture} -> {violations:?}");
        }
    }

    #[test]
    fn build_scripts_in_a_core_crate_are_violations() {
        // Round-3 V3's other half: a build script needs no manifest key and
        // its output compiles in from under target/, unscanned.
        let by_file = build_script_violation("crates/gauntlet-app/build.rs", APP);
        assert!(by_file.is_some_and(|v| v.contains("build script")));
        assert!(build_script_violation("crates/gauntlet-app/src/lib.rs", APP).is_none());

        let by_key = scan_cargo_target_paths(
            "crates/gauntlet-app/Cargo.toml",
            APP,
            "[package]\nname = \"gauntlet-app\"\nbuild = \"build.rs\"\n",
        );
        assert!(
            by_key.iter().any(|v| v.contains("build script")),
            "{by_key:?}"
        );

        // `build = false` is the opt-OUT and must not be reported.
        let disabled = scan_cargo_target_paths(
            "crates/gauntlet-app/Cargo.toml",
            APP,
            "[package]\nname = \"gauntlet-app\"\nbuild = false\n",
        );
        assert!(disabled.is_empty(), "{disabled:?}");
    }

    #[test]
    fn the_target_path_exemption_is_exactly_the_boundary_suite() {
        // The exemption exists because this suite's own [[test]] targets must
        // live at the answer key's `tests/contract/` locators. It must never
        // widen to a crate whose sources are policed.
        for core in crate::walk::CORE_CRATES {
            assert!(
                target_path_rule_applies(core),
                "the Cargo-target-path rule must police {core}"
            );
        }
        assert!(!target_path_rule_applies("crates/gauntlet-arch-tests"));
        assert_eq!(TARGET_PATH_EXEMPT_CRATES, &["crates/gauntlet-arch-tests"]);
    }

    #[test]
    fn normalize_join_resolves_dot_segments() {
        assert_eq!(
            normalize_join("crates/gauntlet-app/src", "../../../shared/w.rs"),
            "shared/w.rs"
        );
        assert_eq!(
            normalize_join("crates/gauntlet-app/src", "./engine/mod.rs"),
            "crates/gauntlet-app/src/engine/mod.rs"
        );
        assert_eq!(normalize_join("a", "../../../../b"), "../../../b");
    }
}

#[cfg(test)]
mod repo {
    #[test]
    fn core_crates_have_no_scope_escapes_or_unscannable_files() {
        let violations = crate::check_scope_integrity(&crate::repo_root());
        assert!(
            violations.is_empty(),
            "{}",
            crate::summarize("scope-integrity", &violations)
        );
    }
}
