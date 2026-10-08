use std::collections::BTreeMap;

/// Minimal TOML value type supporting only subset features.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    Str(String),
    Int(i64),
    Bool(bool),
    Array(Vec<Value>),
    Table(Table),
}

/// A table is a BTreeMap from string keys to values.
pub type Table = BTreeMap<String, Value>;

/// Parse error with line number and kind.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParseError {
    pub line: usize,
    pub kind: ParseErrorKind,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseErrorKind {
    InvalidUtf8,
    LineTooLong,
    FileTooLarge,
    InvalidTable,
    InvalidKey,
    InvalidValue,
    DuplicateTable,
    DuplicateKey,
    MultilineString,
    InlineTable,
    DottedKey,
    InvalidFloat,
    InvalidDate,
    NullByte,
    NonAsciiControl,
}

impl std::fmt::Display for ParseErrorKind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidUtf8 => write!(f, "invalid UTF-8"),
            Self::LineTooLong => write!(f, "line exceeds 4096 bytes"),
            Self::FileTooLarge => write!(f, "file exceeds 64 KiB"),
            Self::InvalidTable => write!(f, "invalid table header"),
            Self::InvalidKey => write!(f, "invalid key name"),
            Self::InvalidValue => write!(f, "invalid value"),
            Self::DuplicateTable => write!(f, "duplicate table"),
            Self::DuplicateKey => write!(f, "duplicate key"),
            Self::MultilineString => write!(f, "multiline strings not supported"),
            Self::InlineTable => write!(f, "inline tables not supported"),
            Self::DottedKey => write!(f, "dotted keys not supported"),
            Self::InvalidFloat => write!(f, "floats not supported"),
            Self::InvalidDate => write!(f, "dates not supported"),
            Self::NullByte => write!(f, "null byte in input"),
            Self::NonAsciiControl => write!(f, "non-ASCII control character"),
        }
    }
}

pub struct Parser {
    lines: Vec<String>,
}

impl Parser {
    pub fn new(input: &str) -> Result<Self, ParseError> {
        // Check for NUL bytes
        if input.contains('\0') {
            return Err(ParseError {
                line: 1,
                kind: ParseErrorKind::NullByte,
            });
        }

        // Check file size (64 KiB = 65536 bytes)
        if input.len() > 65536 {
            return Err(ParseError {
                line: 1,
                kind: ParseErrorKind::FileTooLarge,
            });
        }

        // Check for non-ASCII control characters and split lines
        let mut lines = Vec::new();
        for (line_no, line) in input.lines().enumerate() {
            // Check line length (4096 bytes)
            if line.len() > 4096 {
                return Err(ParseError {
                    line: line_no + 1,
                    kind: ParseErrorKind::LineTooLong,
                });
            }

            // Check for non-ASCII control characters (0x00-0x1F except tab/newline)
            for ch in line.chars() {
                if (ch as u32) < 0x20 && ch != '\t' {
                    return Err(ParseError {
                        line: line_no + 1,
                        kind: ParseErrorKind::NonAsciiControl,
                    });
                }
                if (ch as u32) == 0x7F {
                    return Err(ParseError {
                        line: line_no + 1,
                        kind: ParseErrorKind::NonAsciiControl,
                    });
                }
            }

            lines.push(line.to_string());
        }

        Ok(Parser { lines })
    }

    pub fn parse(&self) -> Result<Table, ParseError> {
        let mut root = Table::new();
        let mut current_table: Vec<String> = Vec::new();
        let mut declared_tables = std::collections::HashSet::new();
        let mut line_no = 0;

        while line_no < self.lines.len() {
            let line = &self.lines[line_no];
            line_no += 1;

            // Remove comment and trim
            let trimmed = line.split('#').next().unwrap_or("").trim();

            if trimmed.is_empty() {
                continue;
            }

            // Check for table header
            if trimmed.starts_with('[') {
                if !trimmed.ends_with(']') {
                    return Err(ParseError {
                        line: line_no,
                        kind: ParseErrorKind::InvalidTable,
                    });
                }

                let header = &trimmed[1..trimmed.len() - 1].trim();

                // Check for dotted keys [a.b]
                if header.contains('.') {
                    return Err(ParseError {
                        line: line_no,
                        kind: ParseErrorKind::DottedKey,
                    });
                }

                // Check for duplicate table declaration
                if declared_tables.contains(*header) {
                    return Err(ParseError {
                        line: line_no,
                        kind: ParseErrorKind::DuplicateTable,
                    });
                }

                declared_tables.insert(header.to_string());
                current_table = vec![header.to_string()];
                continue;
            }

            // Parse key = value
            let (key, value_str) = self.parse_kv_pair(trimmed, line_no)?;

            // Parse value
            let value = self.parse_value(&value_str, line_no)?;

            // Insert into appropriate table
            if current_table.is_empty() {
                if root.contains_key(&key) {
                    return Err(ParseError {
                        line: line_no,
                        kind: ParseErrorKind::DuplicateKey,
                    });
                }
                root.insert(key, value);
            } else {
                let table_name = &current_table[0];
                let table = root
                    .entry(table_name.clone())
                    .or_insert_with(|| Value::Table(Table::new()));

                if let Value::Table(t) = table {
                    if t.contains_key(&key) {
                        return Err(ParseError {
                            line: line_no,
                            kind: ParseErrorKind::DuplicateKey,
                        });
                    }
                    t.insert(key, value);
                } else {
                    return Err(ParseError {
                        line: line_no,
                        kind: ParseErrorKind::DuplicateTable,
                    });
                }
            }
        }

        Ok(root)
    }

    fn parse_kv_pair(&self, trimmed: &str, line_no: usize) -> Result<(String, String), ParseError> {
        let eq_pos = match trimmed.find('=') {
            Some(p) => p,
            None => {
                return Err(ParseError {
                    line: line_no,
                    kind: ParseErrorKind::InvalidKey,
                });
            }
        };

        let key = trimmed[..eq_pos].trim();
        let value = trimmed[eq_pos + 1..].trim();

        // Validate key name (bare identifier)
        if key.is_empty()
            || !key
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '-')
            || !key.chars().next().unwrap().is_ascii()
        {
            return Err(ParseError {
                line: line_no,
                kind: ParseErrorKind::InvalidKey,
            });
        }

        Ok((key.to_string(), value.to_string()))
    }

    fn parse_value(&self, value_str: &str, line_no: usize) -> Result<Value, ParseError> {
        let value_str = value_str.trim();

        // Check for multiline strings
        if value_str.starts_with("\"\"\"") || value_str.starts_with("'''") {
            return Err(ParseError {
                line: line_no,
                kind: ParseErrorKind::MultilineString,
            });
        }

        // Basic string "..."
        if value_str.starts_with('"') {
            return self.parse_basic_string(value_str, line_no);
        }

        // Literal string '...'
        if value_str.starts_with('\'') {
            return self.parse_literal_string(value_str, line_no);
        }

        // Boolean
        if value_str == "true" {
            return Ok(Value::Bool(true));
        }
        if value_str == "false" {
            return Ok(Value::Bool(false));
        }

        // Array
        if value_str.starts_with('[') {
            return self.parse_array(value_str, line_no);
        }

        // Inline table
        if value_str.starts_with('{') {
            return Err(ParseError {
                line: line_no,
                kind: ParseErrorKind::InlineTable,
            });
        }

        // Integer
        if let Ok(i) = value_str.parse::<i64>() {
            return Ok(Value::Int(i));
        }

        // Float (not supported)
        if value_str.contains('.') || value_str.to_lowercase().contains('e') {
            return Err(ParseError {
                line: line_no,
                kind: ParseErrorKind::InvalidFloat,
            });
        }

        // Date/time
        if value_str.contains('-') && value_str.contains(':') {
            return Err(ParseError {
                line: line_no,
                kind: ParseErrorKind::InvalidDate,
            });
        }

        Err(ParseError {
            line: line_no,
            kind: ParseErrorKind::InvalidValue,
        })
    }

    fn parse_basic_string(&self, s: &str, line_no: usize) -> Result<Value, ParseError> {
        if !s.ends_with('"') {
            return Err(ParseError {
                line: line_no,
                kind: ParseErrorKind::InvalidValue,
            });
        }

        let content = &s[1..s.len() - 1];
        let mut result = String::new();
        let mut chars = content.chars();

        while let Some(ch) = chars.next() {
            if ch == '\\' {
                match chars.next() {
                    Some('"') => result.push('"'),
                    Some('\\') => result.push('\\'),
                    Some('n') => result.push('\n'),
                    Some('t') => result.push('\t'),
                    Some('r') => result.push('\r'),
                    _ => {
                        return Err(ParseError {
                            line: line_no,
                            kind: ParseErrorKind::InvalidValue,
                        });
                    }
                }
            } else {
                result.push(ch);
            }
        }

        Ok(Value::Str(result))
    }

    fn parse_literal_string(&self, s: &str, line_no: usize) -> Result<Value, ParseError> {
        if !s.ends_with('\'') {
            return Err(ParseError {
                line: line_no,
                kind: ParseErrorKind::InvalidValue,
            });
        }

        let content = &s[1..s.len() - 1];
        Ok(Value::Str(content.to_string()))
    }

    fn parse_array(&self, s: &str, line_no: usize) -> Result<Value, ParseError> {
        if !s.ends_with(']') {
            return Err(ParseError {
                line: line_no,
                kind: ParseErrorKind::InvalidValue,
            });
        }

        let content = &s[1..s.len() - 1].trim();

        if content.is_empty() {
            return Ok(Value::Array(Vec::new()));
        }

        let mut elements = Vec::new();
        let mut current = String::new();
        let mut in_string = false;
        let mut string_char = ' ';

        for ch in content.chars() {
            match ch {
                '"' | '\'' if !in_string => {
                    in_string = true;
                    string_char = ch;
                    current.push(ch);
                }
                c if in_string && c == string_char => {
                    in_string = false;
                    current.push(c);
                }
                ',' if !in_string => {
                    let trimmed = current.trim();
                    if !trimmed.is_empty() {
                        elements.push(self.parse_value(trimmed, line_no)?);
                    }
                    current.clear();
                }
                _ => current.push(ch),
            }
        }

        let trimmed = current.trim();
        if !trimmed.is_empty() {
            elements.push(self.parse_value(trimmed, line_no)?);
        }

        Ok(Value::Array(elements))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_input() {
        let parser = Parser::new("").unwrap();
        let result = parser.parse().unwrap();
        assert!(result.is_empty());
    }

    #[test]
    fn test_simple_key_value() {
        let parser = Parser::new("id = \"test\"").unwrap();
        let result = parser.parse().unwrap();
        assert_eq!(result.get("id"), Some(&Value::Str("test".to_string())));
    }

    #[test]
    fn test_comment_removal() {
        let parser = Parser::new("id = \"test\" # comment").unwrap();
        let result = parser.parse().unwrap();
        assert_eq!(result.get("id"), Some(&Value::Str("test".to_string())));
    }

    #[test]
    fn test_literal_string() {
        let parser = Parser::new("id = 'test'").unwrap();
        let result = parser.parse().unwrap();
        assert_eq!(result.get("id"), Some(&Value::Str("test".to_string())));
    }

    #[test]
    fn test_escape_sequences() {
        let parser = Parser::new(r#"msg = "line1\nline2\ttab\\\"end""#).unwrap();
        let result = parser.parse().unwrap();
        if let Some(Value::Str(s)) = result.get("msg") {
            // \n -> newline, \t -> tab, \\ -> backslash, \" -> quote
            assert_eq!(s, "line1\nline2\ttab\\\"end");
        } else {
            panic!("Expected string value");
        }
    }

    #[test]
    fn test_integer() {
        let parser = Parser::new("count = 42").unwrap();
        let result = parser.parse().unwrap();
        assert_eq!(result.get("count"), Some(&Value::Int(42)));
    }

    #[test]
    fn test_boolean_true() {
        let parser = Parser::new("flag = true").unwrap();
        let result = parser.parse().unwrap();
        assert_eq!(result.get("flag"), Some(&Value::Bool(true)));
    }

    #[test]
    fn test_boolean_false() {
        let parser = Parser::new("flag = false").unwrap();
        let result = parser.parse().unwrap();
        assert_eq!(result.get("flag"), Some(&Value::Bool(false)));
    }

    #[test]
    fn test_string_array() {
        let parser = Parser::new("items = ['a', 'b', 'c']").unwrap();
        let result = parser.parse().unwrap();
        if let Some(Value::Array(arr)) = result.get("items") {
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0], Value::Str("a".to_string()));
            assert_eq!(arr[1], Value::Str("b".to_string()));
            assert_eq!(arr[2], Value::Str("c".to_string()));
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_integer_array() {
        let parser = Parser::new("nums = [1, 2, 3]").unwrap();
        let result = parser.parse().unwrap();
        if let Some(Value::Array(arr)) = result.get("nums") {
            assert_eq!(arr.len(), 3);
            assert_eq!(arr[0], Value::Int(1));
        } else {
            panic!("Expected array");
        }
    }

    #[test]
    fn test_table_section() {
        let parser = Parser::new("[section]\nkey = \"value\"").unwrap();
        let result = parser.parse().unwrap();
        assert!(result.contains_key("section"));
        if let Some(Value::Table(t)) = result.get("section") {
            assert_eq!(t.get("key"), Some(&Value::Str("value".to_string())));
        }
    }

    #[test]
    fn test_reject_multiline_string_triple_double() {
        let parser = Parser::new(r#"x = """text""""#).unwrap();
        let err = parser.parse();
        assert!(matches!(
            err,
            Err(ParseError {
                kind: ParseErrorKind::MultilineString,
                ..
            })
        ));
    }

    #[test]
    fn test_reject_multiline_string_triple_single() {
        let parser = Parser::new("x = '''text'''").unwrap();
        let err = parser.parse();
        assert!(matches!(
            err,
            Err(ParseError {
                kind: ParseErrorKind::MultilineString,
                ..
            })
        ));
    }

    #[test]
    fn test_reject_inline_table() {
        let parser = Parser::new("x = {a = 1}").unwrap();
        let err = parser.parse();
        assert!(matches!(
            err,
            Err(ParseError {
                kind: ParseErrorKind::InlineTable,
                ..
            })
        ));
    }

    #[test]
    fn test_reject_dotted_keys() {
        let parser = Parser::new("[a.b]\nkey = 1").unwrap();
        let err = parser.parse();
        assert!(matches!(
            err,
            Err(ParseError {
                kind: ParseErrorKind::DottedKey,
                ..
            })
        ));
    }

    #[test]
    fn test_reject_float() {
        let parser = Parser::new("x = 3.14").unwrap();
        let err = parser.parse();
        assert!(matches!(
            err,
            Err(ParseError {
                kind: ParseErrorKind::InvalidFloat,
                ..
            })
        ));
    }

    #[test]
    fn test_reject_nul_byte() {
        let result = Parser::new("x\0y");
        assert!(matches!(
            result,
            Err(ParseError {
                kind: ParseErrorKind::NullByte,
                ..
            })
        ));
    }

    #[test]
    fn test_reject_file_too_large() {
        let large = "x = \"y\"\n".repeat(10000);
        let result = Parser::new(&large);
        assert!(matches!(
            result,
            Err(ParseError {
                kind: ParseErrorKind::FileTooLarge,
                ..
            })
        ));
    }

    #[test]
    fn test_reject_line_too_long() {
        let long_line = "x = \"".to_string() + &"a".repeat(5000) + "\"";
        let result = Parser::new(&long_line);
        assert!(matches!(
            result,
            Err(ParseError {
                kind: ParseErrorKind::LineTooLong,
                ..
            })
        ));
    }

    #[test]
    fn test_reject_duplicate_key() {
        let parser = Parser::new("x = 1\nx = 2").unwrap();
        let err = parser.parse();
        assert!(matches!(
            err,
            Err(ParseError {
                kind: ParseErrorKind::DuplicateKey,
                ..
            })
        ));
    }

    #[test]
    fn test_reject_duplicate_table() {
        let parser = Parser::new("[t]\n[t]\nkey = 1").unwrap();
        let err = parser.parse();
        assert!(matches!(
            err,
            Err(ParseError {
                kind: ParseErrorKind::DuplicateTable,
                ..
            })
        ));
    }
}
