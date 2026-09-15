//! A minimal, dependency-free parser for the `baseline.json` format this
//! adapter writes ([`crate::seed::manifest_to_json`]) — no external JSON
//! crate is on this layer's allowlist beyond `sha2`, and the format is
//! entirely ours (a flat object of path → `{sha256,size,executable}`), so a
//! small hand-rolled parser is the whole dependency.

use std::collections::BTreeMap;

use crate::seed::{FileDigest, Manifest};

/// Parses a manifest previously produced by
/// [`crate::seed::manifest_to_json`]. Any structural surprise is a typed
/// error string naming the byte offset, never a silent partial parse.
pub fn parse_manifest_json(input: &str) -> Result<Manifest, String> {
    let mut chars = input.char_indices().peekable();
    skip_ws(&mut chars);
    expect(&mut chars, '{')?;
    let mut manifest: Manifest = BTreeMap::new();
    skip_ws(&mut chars);
    if peek_char(&mut chars) == Some('}') {
        chars.next();
        return Ok(manifest);
    }
    loop {
        skip_ws(&mut chars);
        let key = parse_string(&mut chars)?;
        skip_ws(&mut chars);
        expect(&mut chars, ':')?;
        skip_ws(&mut chars);
        let digest = parse_digest_object(&mut chars)?;
        manifest.insert(key, digest);
        skip_ws(&mut chars);
        match chars.next() {
            Some((_, ',')) => continue,
            Some((_, '}')) => break,
            other => return Err(format!("expected ',' or '}}', got {other:?}")),
        }
    }
    Ok(manifest)
}

type Chars<'a> = std::iter::Peekable<std::str::CharIndices<'a>>;

fn peek_char(chars: &mut Chars) -> Option<char> {
    chars.peek().map(|(_, ch)| *ch)
}

fn skip_ws(chars: &mut Chars) {
    while matches!(peek_char(chars), Some(ch) if ch.is_whitespace()) {
        chars.next();
    }
}

fn expect(chars: &mut Chars, expected: char) -> Result<(), String> {
    match chars.next() {
        Some((_, ch)) if ch == expected => Ok(()),
        other => Err(format!("expected {expected:?}, got {other:?}")),
    }
}

fn parse_string(chars: &mut Chars) -> Result<String, String> {
    expect(chars, '"')?;
    let mut out = String::new();
    loop {
        match chars.next() {
            Some((_, '"')) => return Ok(out),
            Some((_, '\\')) => match chars.next() {
                Some((_, '"')) => out.push('"'),
                Some((_, '\\')) => out.push('\\'),
                Some((_, 'n')) => out.push('\n'),
                Some((_, 'u')) => {
                    let mut hex = String::new();
                    for _ in 0..4 {
                        match chars.next() {
                            Some((_, digit)) => hex.push(digit),
                            None => return Err("truncated \\u escape".to_owned()),
                        }
                    }
                    let code = u32::from_str_radix(&hex, 16)
                        .map_err(|error| format!("invalid \\u escape {hex:?}: {error}"))?;
                    if let Some(ch) = char::from_u32(code) {
                        out.push(ch);
                    }
                }
                other => return Err(format!("unsupported escape: {other:?}")),
            },
            Some((_, ch)) => out.push(ch),
            None => return Err("unterminated string".to_owned()),
        }
    }
}

fn parse_digest_object(chars: &mut Chars) -> Result<FileDigest, String> {
    expect(chars, '{')?;
    let mut sha256 = None;
    let mut size = None;
    let mut executable = None;
    skip_ws(chars);
    if peek_char(chars) == Some('}') {
        chars.next();
        return Err("digest object missing required fields".to_owned());
    }
    loop {
        skip_ws(chars);
        let key = parse_string(chars)?;
        skip_ws(chars);
        expect(chars, ':')?;
        skip_ws(chars);
        match key.as_str() {
            "sha256" => sha256 = Some(parse_string(chars)?),
            "size" => size = Some(parse_number(chars)?),
            "executable" => executable = Some(parse_bool(chars)?),
            other => return Err(format!("unknown digest field {other:?}")),
        }
        skip_ws(chars);
        match chars.next() {
            Some((_, ',')) => continue,
            Some((_, '}')) => break,
            other => return Err(format!("expected ',' or '}}', got {other:?}")),
        }
    }
    Ok(FileDigest {
        sha256: sha256.ok_or("digest object missing sha256")?,
        size: size.ok_or("digest object missing size")?,
        executable: executable.ok_or("digest object missing executable")?,
    })
}

fn parse_number(chars: &mut Chars) -> Result<u64, String> {
    let mut raw = String::new();
    while matches!(peek_char(chars), Some(ch) if ch.is_ascii_digit()) {
        raw.push(chars.next().expect("peeked").1);
    }
    raw.parse::<u64>()
        .map_err(|error| format!("invalid number {raw:?}: {error}"))
}

fn parse_bool(chars: &mut Chars) -> Result<bool, String> {
    if input_matches(chars, "true") {
        Ok(true)
    } else if input_matches(chars, "false") {
        Ok(false)
    } else {
        Err("expected boolean".to_owned())
    }
}

fn input_matches(chars: &mut Chars, literal: &str) -> bool {
    let mut lookahead = chars.clone();
    for expected in literal.chars() {
        match lookahead.next() {
            Some((_, ch)) if ch == expected => continue,
            _ => return false,
        }
    }
    for _ in literal.chars() {
        chars.next();
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_manifest_object() {
        let json = r#"{
  "a/b.txt": {"sha256": "deadbeef", "size": 3, "executable": false}
}
"#;
        let manifest = parse_manifest_json(json).expect("parses");
        assert_eq!(manifest.len(), 1);
        let digest = &manifest["a/b.txt"];
        assert_eq!(digest.sha256, "deadbeef");
        assert_eq!(digest.size, 3);
        assert!(!digest.executable);
    }

    #[test]
    fn parses_the_empty_manifest() {
        let manifest = parse_manifest_json("{}").expect("parses");
        assert!(manifest.is_empty());
    }
}
