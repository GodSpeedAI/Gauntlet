//! Minimal strict JSON reader/writer — hand-rolled, std only.
//!
//! The `role_result@v1` envelope (spec Annex B.1) arrives as a JSON document
//! written by an agent, and "Model output is untrusted external input: parse,
//! validate, then admit" (spec §21.2). Zero external dependencies is a hard
//! constraint on this crate (spec §4.1: ports depend on the domain only), so
//! the reader is written here rather than pulled in.
//!
//! Strictness that matters for settlement-bearing documents:
//!
//! - **Duplicate object keys are rejected.** A document where one field
//!   silently shadows another is ambiguous evidence, not data.
//! - **Trailing content after the top-level value is rejected.**
//! - **Recursion depth is bounded** so hostile input cannot exhaust the
//!   stack; exceeding the bound is a typed error, not a crash.
//! - Numbers keep their shape: unsigned integers, signed integers, and
//!   floats are distinct variants, so a schema can refuse `-3` where a count
//!   is required without float round-tripping.
//!
//! Unknown-field rejection is NOT this module's job — a document model cannot
//! know a schema. Each typed schema in [`crate::role_result`] consumes the
//! object keys it knows and mechanically rejects any key left over.

use core::fmt;

/// Maximum nesting depth. Deeply nested JSON carries no legitimate result
/// payload; the bound makes stack exhaustion a typed error.
const MAX_DEPTH: usize = 64;

/// JSON reader failures — typed, with the byte offset where parsing stopped.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum JsonError {
    UnexpectedByte { at: usize, found: u8 },
    UnterminatedString { at: usize },
    InvalidEscape { at: usize },
    InvalidNumber { at: usize },
    InvalidLiteral { at: usize },
    TrailingContent { at: usize },
    DepthExceeded { at: usize },
    DuplicateKey { at: usize, key: String },
}

impl fmt::Display for JsonError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnexpectedByte { at, found } => {
                write!(f, "json: unexpected byte {found:#x} at offset {at}")
            }
            Self::UnterminatedString { at } => {
                write!(f, "json: string starting at offset {at} never terminates")
            }
            Self::InvalidEscape { at } => write!(f, "json: invalid escape at offset {at}"),
            Self::InvalidNumber { at } => write!(f, "json: invalid number at offset {at}"),
            Self::InvalidLiteral { at } => write!(f, "json: invalid literal at offset {at}"),
            Self::TrailingContent { at } => {
                write!(
                    f,
                    "json: content continues after the top-level value at offset {at}"
                )
            }
            Self::DepthExceeded { at } => write!(
                f,
                "json: nesting deeper than {MAX_DEPTH} levels at offset {at} — rejected, not \
                 recursed into"
            ),
            Self::DuplicateKey { at, key } => write!(
                f,
                "json: duplicate object key {key:?} at offset {at} — ambiguous documents are \
                 not admitted"
            ),
        }
    }
}

impl std::error::Error for JsonError {}

/// A parsed JSON number, keeping its shape.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum JsonNumber {
    /// A non-negative integer literal.
    Uint(u64),
    /// A negative integer literal.
    NegInt(i64),
    /// Anything with a fraction or exponent. Always finite.
    Float(f64),
}

impl JsonNumber {
    /// The value as `u64` when it is a non-negative integer literal;
    /// negative, fractional, or exponent forms are `None`.
    pub fn as_u64(self) -> Option<u64> {
        match self {
            Self::Uint(n) => Some(n),
            _ => None,
        }
    }

    /// The value as `i64` when it is an integer literal of either sign.
    pub fn as_i64(self) -> Option<i64> {
        match self {
            Self::Uint(n) => i64::try_from(n).ok(),
            Self::NegInt(n) => Some(n),
            Self::Float(_) => None,
        }
    }

    /// The value as a finite `f64`.
    pub fn as_f64(self) -> f64 {
        match self {
            Self::Uint(n) => n as f64,
            Self::NegInt(n) => n as f64,
            Self::Float(v) => v,
        }
    }
}

/// A parsed JSON document.
#[derive(Debug, Clone, PartialEq)]
pub enum JsonValue {
    Null,
    Bool(bool),
    Number(JsonNumber),
    Str(String),
    Array(Vec<JsonValue>),
    /// Key order is preserved as written; duplicates were rejected at parse
    /// time, so lookups are unambiguous.
    Object(Vec<(String, JsonValue)>),
}

impl JsonValue {
    /// The value at `key`, if this is an object containing it.
    pub fn get(&self, key: &str) -> Option<&JsonValue> {
        match self {
            Self::Object(entries) => entries.iter().find(|(k, _)| k == key).map(|(_, v)| v),
            _ => None,
        }
    }

    /// Every key, in written order — schemas sweep this to reject unknown
    /// fields mechanically.
    pub fn keys(&self) -> Vec<String> {
        match self {
            Self::Object(entries) => entries.iter().map(|(k, _)| k.clone()).collect(),
            _ => Vec::new(),
        }
    }

    /// Serializes the value as JSON text. Object key order is preserved;
    /// strings are escaped; integers never gain a fraction. The output is
    /// valid input for [`parse`].
    pub fn to_json_text(&self) -> String {
        let mut out = String::new();
        self.write(&mut out);
        out
    }

    fn write(&self, out: &mut String) {
        match self {
            Self::Null => out.push_str("null"),
            Self::Bool(true) => out.push_str("true"),
            Self::Bool(false) => out.push_str("false"),
            Self::Number(JsonNumber::Uint(n)) => {
                out.push_str(&n.to_string());
            }
            Self::Number(JsonNumber::NegInt(n)) => {
                out.push_str(&n.to_string());
            }
            Self::Number(JsonNumber::Float(v)) => {
                if v.is_finite() {
                    out.push_str(&v.to_string());
                } else {
                    out.push_str("null");
                }
            }
            Self::Str(s) => write_escaped(s, out),
            Self::Array(items) => {
                out.push('[');
                for (index, item) in items.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    item.write(out);
                }
                out.push(']');
            }
            Self::Object(entries) => {
                out.push('{');
                for (index, (key, value)) in entries.iter().enumerate() {
                    if index > 0 {
                        out.push(',');
                    }
                    write_escaped(key, out);
                    out.push(':');
                    value.write(out);
                }
                out.push('}');
            }
        }
    }
}

fn write_escaped(s: &str, out: &mut String) {
    out.push('"');
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\u{8}' => out.push_str("\\b"),
            '\u{c}' => out.push_str("\\f"),
            c if (c as u32) < 0x20 => {
                out.push_str(&format!("\\u{:04x}", c as u32));
            }
            c => out.push(c),
        }
    }
    out.push('"');
}

/// Parses exactly one JSON document; trailing content is an error.
pub fn parse(text: &str) -> Result<JsonValue, JsonError> {
    let mut reader = Reader {
        bytes: text.as_bytes(),
        at: 0,
    };
    reader.skip_ws();
    let value = reader.value(0)?;
    reader.skip_ws();
    if reader.at < reader.bytes.len() {
        return Err(JsonError::TrailingContent { at: reader.at });
    }
    Ok(value)
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl<'a> Reader<'a> {
    fn skip_ws(&mut self) {
        while matches!(self.bytes.get(self.at), Some(b' ' | b'\t' | b'\n' | b'\r')) {
            self.at += 1;
        }
    }

    fn value(&mut self, depth: usize) -> Result<JsonValue, JsonError> {
        if depth >= MAX_DEPTH {
            return Err(JsonError::DepthExceeded { at: self.at });
        }
        match self.bytes.get(self.at) {
            Some(b'{') => self.object(depth),
            Some(b'[') => self.array(depth),
            Some(b'"') => Ok(JsonValue::Str(self.string()?)),
            Some(b't') => self.literal("true", JsonValue::Bool(true)),
            Some(b'f') => self.literal("false", JsonValue::Bool(false)),
            Some(b'n') => self.literal("null", JsonValue::Null),
            Some(b'-' | b'0'..=b'9') => self.number(),
            Some(other) => Err(JsonError::UnexpectedByte {
                at: self.at,
                found: *other,
            }),
            None => Err(JsonError::UnexpectedByte {
                at: self.at,
                found: 0,
            }),
        }
    }

    fn literal(&mut self, word: &str, value: JsonValue) -> Result<JsonValue, JsonError> {
        if self.bytes[self.at..].starts_with(word.as_bytes()) {
            self.at += word.len();
            Ok(value)
        } else {
            Err(JsonError::InvalidLiteral { at: self.at })
        }
    }

    fn object(&mut self, depth: usize) -> Result<JsonValue, JsonError> {
        self.at += 1; // '{'
        let mut entries: Vec<(String, JsonValue)> = Vec::new();
        self.skip_ws();
        if self.bytes.get(self.at) == Some(&b'}') {
            self.at += 1;
            return Ok(JsonValue::Object(entries));
        }
        loop {
            self.skip_ws();
            if self.bytes.get(self.at) != Some(&b'"') {
                return Err(JsonError::UnexpectedByte {
                    at: self.at,
                    found: self.bytes.get(self.at).copied().unwrap_or(0),
                });
            }
            let key = self.string()?;
            self.skip_ws();
            if self.bytes.get(self.at) != Some(&b':') {
                return Err(JsonError::UnexpectedByte {
                    at: self.at,
                    found: self.bytes.get(self.at).copied().unwrap_or(0),
                });
            }
            self.at += 1;
            self.skip_ws();
            let value = self.value(depth + 1)?;
            if entries.iter().any(|(k, _)| *k == key) {
                return Err(JsonError::DuplicateKey { at: self.at, key });
            }
            entries.push((key, value));
            self.skip_ws();
            match self.bytes.get(self.at) {
                Some(b',') => self.at += 1,
                Some(b'}') => {
                    self.at += 1;
                    return Ok(JsonValue::Object(entries));
                }
                other => {
                    return Err(JsonError::UnexpectedByte {
                        at: self.at,
                        found: other.copied().unwrap_or(0),
                    });
                }
            }
        }
    }

    fn array(&mut self, depth: usize) -> Result<JsonValue, JsonError> {
        self.at += 1; // '['
        let mut items = Vec::new();
        self.skip_ws();
        if self.bytes.get(self.at) == Some(&b']') {
            self.at += 1;
            return Ok(JsonValue::Array(items));
        }
        loop {
            self.skip_ws();
            items.push(self.value(depth + 1)?);
            self.skip_ws();
            match self.bytes.get(self.at) {
                Some(b',') => self.at += 1,
                Some(b']') => {
                    self.at += 1;
                    return Ok(JsonValue::Array(items));
                }
                other => {
                    return Err(JsonError::UnexpectedByte {
                        at: self.at,
                        found: other.copied().unwrap_or(0),
                    });
                }
            }
        }
    }

    fn string(&mut self) -> Result<String, JsonError> {
        let start = self.at;
        self.at += 1; // opening quote
        let mut out = String::new();
        loop {
            let byte = match self.bytes.get(self.at) {
                Some(byte) => *byte,
                None => return Err(JsonError::UnterminatedString { at: start }),
            };
            match byte {
                b'"' => {
                    self.at += 1;
                    return Ok(out);
                }
                b'\\' => {
                    self.at += 1;
                    let escape = match self.bytes.get(self.at) {
                        Some(escape) => *escape,
                        None => return Err(JsonError::UnterminatedString { at: start }),
                    };
                    self.at += 1;
                    match escape {
                        b'"' => out.push('"'),
                        b'\\' => out.push('\\'),
                        b'/' => out.push('/'),
                        b'b' => out.push('\u{8}'),
                        b'f' => out.push('\u{c}'),
                        b'n' => out.push('\n'),
                        b'r' => out.push('\r'),
                        b't' => out.push('\t'),
                        b'u' => {
                            let high = self.hex4()?;
                            let code = if (0xd800..0xdc00).contains(&high) {
                                // A leading surrogate must be followed by a
                                // trailing surrogate escape.
                                if self.bytes.get(self.at) != Some(&b'\\')
                                    || self.bytes.get(self.at + 1) != Some(&b'u')
                                {
                                    return Err(JsonError::InvalidEscape { at: self.at - 2 });
                                }
                                self.at += 2;
                                let low = self.hex4()?;
                                if !(0xdc00..0xe000).contains(&low) {
                                    return Err(JsonError::InvalidEscape { at: self.at - 4 });
                                }
                                0x1_0000 + ((high - 0xd800) << 10) + (low - 0xdc00)
                            } else if (0xdc00..0xe000).contains(&high) {
                                return Err(JsonError::InvalidEscape { at: self.at - 4 });
                            } else {
                                high
                            };
                            match char::from_u32(code) {
                                Some(ch) => out.push(ch),
                                None => return Err(JsonError::InvalidEscape { at: self.at - 4 }),
                            }
                        }
                        _ => return Err(JsonError::InvalidEscape { at: self.at - 2 }),
                    }
                }
                0x00..=0x1f => {
                    return Err(JsonError::UnexpectedByte {
                        at: self.at,
                        found: byte,
                    });
                }
                _ => {
                    // Copy the whole UTF-8 sequence verbatim; input is a
                    // &str, so sequences are well-formed by construction.
                    let ch_len = utf8_len(byte);
                    let end = (self.at + ch_len).min(self.bytes.len());
                    match core::str::from_utf8(&self.bytes[self.at..end]) {
                        Ok(chunk) => out.push_str(chunk),
                        Err(_) => return Err(JsonError::UnterminatedString { at: start }),
                    }
                    self.at = end;
                }
            }
        }
    }

    fn hex4(&mut self) -> Result<u32, JsonError> {
        let mut value = 0u32;
        for _ in 0..4 {
            let digit = match self.bytes.get(self.at) {
                Some(byte) => *byte,
                None => return Err(JsonError::UnterminatedString { at: self.at }),
            };
            let nibble = match digit {
                b'0'..=b'9' => u32::from(digit - b'0'),
                b'a'..=b'f' => u32::from(digit - b'a') + 10,
                b'A'..=b'F' => u32::from(digit - b'A') + 10,
                _ => return Err(JsonError::InvalidEscape { at: self.at }),
            };
            value = value * 16 + nibble;
            self.at += 1;
        }
        Ok(value)
    }

    fn number(&mut self) -> Result<JsonValue, JsonError> {
        let start = self.at;
        let negative = self.bytes.get(self.at) == Some(&b'-');
        if negative {
            self.at += 1;
        }
        // Integer part: `0` alone (a following digit is a leading zero,
        // which JSON forbids), or [1-9][0-9]*.
        match self.bytes.get(self.at) {
            Some(b'0') => {
                self.at += 1;
                if matches!(self.bytes.get(self.at), Some(b'0'..=b'9')) {
                    return Err(JsonError::InvalidNumber { at: start });
                }
            }
            Some(b'1'..=b'9') => {
                while matches!(self.bytes.get(self.at), Some(b'0'..=b'9')) {
                    self.at += 1;
                }
            }
            _ => return Err(JsonError::InvalidNumber { at: start }),
        }
        let mut is_float = false;
        if self.bytes.get(self.at) == Some(&b'.') {
            is_float = true;
            self.at += 1;
            if !matches!(self.bytes.get(self.at), Some(b'0'..=b'9')) {
                return Err(JsonError::InvalidNumber { at: self.at });
            }
            while matches!(self.bytes.get(self.at), Some(b'0'..=b'9')) {
                self.at += 1;
            }
        }
        if matches!(self.bytes.get(self.at), Some(b'e' | b'E')) {
            is_float = true;
            self.at += 1;
            if matches!(self.bytes.get(self.at), Some(b'+' | b'-')) {
                self.at += 1;
            }
            if !matches!(self.bytes.get(self.at), Some(b'0'..=b'9')) {
                return Err(JsonError::InvalidNumber { at: self.at });
            }
            while matches!(self.bytes.get(self.at), Some(b'0'..=b'9')) {
                self.at += 1;
            }
        }
        let text = core::str::from_utf8(&self.bytes[start..self.at])
            .map_err(|_| JsonError::InvalidNumber { at: start })?;
        if !is_float {
            if negative {
                if let Ok(n) = text.parse::<i64>() {
                    return Ok(JsonValue::Number(JsonNumber::NegInt(n)));
                }
            } else if let Ok(n) = text.parse::<u64>() {
                return Ok(JsonValue::Number(JsonNumber::Uint(n)));
            }
        }
        let v: f64 = text
            .parse()
            .map_err(|_| JsonError::InvalidNumber { at: start })?;
        if !v.is_finite() {
            return Err(JsonError::InvalidNumber { at: start });
        }
        Ok(JsonValue::Number(JsonNumber::Float(v)))
    }
}

fn utf8_len(first: u8) -> usize {
    match first {
        0x00..=0x7f => 1,
        0xc0..=0xdf => 2,
        0xe0..=0xef => 3,
        0xf0..=0xf7 => 4,
        _ => 1,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scalars_and_shapes_parse_and_round_trip() {
        let doc = parse(
            "{\"s\":\"a\\n\\\"\\\\é\",\"u\":3,\"neg\":-3,\"f\":0.5,\"t\":true,\"n\":null,\
             \"arr\":[1,2]}",
        )
        .expect("well-formed");
        assert_eq!(doc.get("s"), Some(&JsonValue::Str("a\n\"\\é".to_owned())));
        assert_eq!(doc.get("u"), Some(&JsonValue::Number(JsonNumber::Uint(3))));
        assert_eq!(
            doc.get("neg"),
            Some(&JsonValue::Number(JsonNumber::NegInt(-3)))
        );
        assert_eq!(
            doc.get("f"),
            Some(&JsonValue::Number(JsonNumber::Float(0.5)))
        );
        assert_eq!(doc.get("t"), Some(&JsonValue::Bool(true)));
        assert_eq!(doc.get("n"), Some(&JsonValue::Null));
        let round = parse(&doc.to_json_text()).expect("writer output reparses");
        assert_eq!(round, doc);
        // Number shapes stay distinct through accessors.
        assert_eq!(JsonNumber::Uint(3).as_u64(), Some(3));
        assert_eq!(JsonNumber::NegInt(-3).as_u64(), None);
        assert_eq!(JsonNumber::NegInt(-3).as_i64(), Some(-3));
        assert_eq!(JsonNumber::Uint(3).as_f64(), 3.0);
    }

    #[test]
    fn duplicate_keys_are_rejected() {
        let err = parse("{\"a\":1,\"a\":2}").expect_err("shadowing a key is ambiguous");
        assert!(matches!(err, JsonError::DuplicateKey { key, .. } if key == "a"));
    }

    #[test]
    fn malformed_documents_are_typed_errors() {
        assert!(matches!(
            parse("{\"a\":1} trailing"),
            Err(JsonError::TrailingContent { .. })
        ));
        assert!(matches!(parse(""), Err(JsonError::UnexpectedByte { .. })));
        assert!(matches!(
            parse("{\"a\"}"),
            Err(JsonError::UnexpectedByte { .. })
        ));
        assert!(matches!(
            parse("[1,]"),
            Err(JsonError::UnexpectedByte { .. })
        ));
        assert!(matches!(
            parse("\"unterminated"),
            Err(JsonError::UnterminatedString { .. })
        ));
        assert!(matches!(
            parse("\"bad \\q escape\""),
            Err(JsonError::InvalidEscape { .. })
        ));
        assert!(matches!(parse("01"), Err(JsonError::InvalidNumber { .. })));
        assert!(matches!(parse("1."), Err(JsonError::InvalidNumber { .. })));
        assert!(matches!(parse("1e"), Err(JsonError::InvalidNumber { .. })));
        assert!(matches!(
            parse("tru"),
            Err(JsonError::InvalidLiteral { .. })
        ));
        // Raw control bytes inside strings are rejected.
        assert!(matches!(
            parse("\"a\nb\""),
            Err(JsonError::UnexpectedByte { .. })
        ));
    }

    #[test]
    fn nesting_past_the_bound_is_a_typed_error_not_a_crash() {
        // Build the text at runtime so the fixture depth is adjustable.
        let opens: String = "[".repeat(MAX_DEPTH + 4);
        let closes: String = "]".repeat(MAX_DEPTH + 4);
        let text = format!("{opens}{closes}");
        assert!(matches!(parse(&text), Err(JsonError::DepthExceeded { .. })));
        // Just inside the bound parses.
        let opens: String = "[".repeat(MAX_DEPTH - 1);
        let closes: String = "]".repeat(MAX_DEPTH - 1);
        let text = format!("{opens}{closes}");
        assert!(parse(&text).is_ok());
    }

    #[test]
    fn surrogate_pairs_decode_and_lone_surrogates_are_rejected() {
        assert_eq!(
            parse("\"\\ud83d\\ude00\"").expect("pair"),
            JsonValue::Str("\u{1f600}".to_owned())
        );
        assert!(matches!(
            parse("\"\\ud83d alone\""),
            Err(JsonError::InvalidEscape { .. })
        ));
        assert!(matches!(
            parse("\"\\ude00 leading low\""),
            Err(JsonError::InvalidEscape { .. })
        ));
    }
}
