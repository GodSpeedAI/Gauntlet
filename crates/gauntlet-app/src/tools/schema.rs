//! The JSON Schema subset the semantic tool runtime validates against.
//!
//! The subset is closed. A schema that uses any other keyword, any other type
//! name, or an object without `additionalProperties: false` is refused when the
//! runtime is built, so an operation can never be listed with a schema the
//! runtime cannot enforce. Validation reports the offending path in the same
//! wording the strict wire decoders use (`arguments.items[1].kind: unknown
//! field`), so a projection sees one vocabulary.

use gauntlet_ports::json::{JsonNumber, JsonValue};

/// The largest argument document the runtime admits, in compact JSON bytes.
pub const MAX_ARGUMENT_BYTES: usize = 1 << 20;

/// The deepest argument document the runtime admits. A scalar is depth 0; the
/// outermost object or array is depth 1.
pub const MAX_ARGUMENT_DEPTH: usize = 16;

const ROOT: &str = "arguments";

/// The only keywords a schema in the subset may use.
const KEYWORDS: &[&str] = &[
    "type",
    "description",
    "properties",
    "required",
    "additionalProperties",
    "items",
    "enum",
    "minimum",
    "maximum",
];

/// The only type names a schema in the subset may use.
const TYPES: &[&str] = &["object", "string", "integer", "boolean", "array", "null"];

/// Why a schema falls outside the subset, or why it was built wrong.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SchemaError {
    /// The schema is not an object, or a keyword has the wrong shape.
    Malformed { path: String, detail: String },
    /// A keyword outside the subset appears.
    UnsupportedKeyword { path: String, keyword: String },
    /// An object schema is not closed with `additionalProperties: false`.
    OpenObject { path: String },
}

impl std::fmt::Display for SchemaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Malformed { path, detail } => write!(f, "{path}: {detail}"),
            Self::UnsupportedKeyword { path, keyword } => {
                write!(
                    f,
                    "{path}: keyword '{keyword}' is outside the supported subset"
                )
            }
            Self::OpenObject { path } => {
                write!(
                    f,
                    "{path}: object schema must set additionalProperties to false"
                )
            }
        }
    }
}

impl std::error::Error for SchemaError {}

/// Checks that `schema` lies in the subset and every object is closed.
pub fn check_subset(schema: &JsonValue) -> Result<(), SchemaError> {
    check_node(schema, ROOT)
}

fn check_node(schema: &JsonValue, path: &str) -> Result<(), SchemaError> {
    let JsonValue::Object(entries) = schema else {
        return Err(SchemaError::Malformed {
            path: path.to_string(),
            detail: "a schema must be an object".to_string(),
        });
    };
    for (key, _) in entries {
        if !KEYWORDS.contains(&key.as_str()) {
            return Err(SchemaError::UnsupportedKeyword {
                path: path.to_string(),
                keyword: key.clone(),
            });
        }
    }
    let types = declared_types(schema, path)?;
    let is_object = types.iter().any(|t| t == "object") || schema.get("properties").is_some();
    if is_object && schema.get("additionalProperties") != Some(&JsonValue::Bool(false)) {
        return Err(SchemaError::OpenObject {
            path: path.to_string(),
        });
    }
    if let Some(properties) = schema.get("properties") {
        let JsonValue::Object(declared) = properties else {
            return Err(malformed(path, "properties must be an object"));
        };
        for (key, child) in declared {
            check_node(child, &format!("{path}.{key}"))?;
        }
        if let Some(required) = schema.get("required") {
            let JsonValue::Array(names) = required else {
                return Err(malformed(path, "required must be an array"));
            };
            for name in names {
                let JsonValue::Str(name) = name else {
                    return Err(malformed(path, "required entries are strings"));
                };
                if !declared.iter().any(|(key, _)| key == name) {
                    return Err(malformed(
                        path,
                        "required names a property that is not declared",
                    ));
                }
            }
        }
    } else if schema.get("required").is_some() {
        return Err(malformed(path, "required needs properties"));
    }
    if let Some(items) = schema.get("items") {
        check_node(items, &format!("{path}[]"))?;
    }
    if let Some(choices) = schema.get("enum") {
        let JsonValue::Array(values) = choices else {
            return Err(malformed(path, "enum must be an array"));
        };
        if values.is_empty() || !values.iter().all(|v| matches!(v, JsonValue::Str(_))) {
            return Err(malformed(path, "enum must hold one or more strings"));
        }
    }
    for bound in ["minimum", "maximum"] {
        if let Some(value) = schema.get(bound)
            && !matches!(value, JsonValue::Number(_))
        {
            return Err(malformed(path, "bounds are numbers"));
        }
    }
    Ok(())
}

fn declared_types(schema: &JsonValue, path: &str) -> Result<Vec<String>, SchemaError> {
    let names: Vec<String> = match schema.get("type") {
        None => return Ok(Vec::new()),
        Some(JsonValue::Str(name)) => vec![name.clone()],
        Some(JsonValue::Array(names)) => {
            let mut out = Vec::new();
            for name in names {
                let JsonValue::Str(name) = name else {
                    return Err(malformed(path, "type entries are strings"));
                };
                out.push(name.clone());
            }
            out
        }
        Some(_) => return Err(malformed(path, "type is a string or an array of strings")),
    };
    if names.iter().any(|name| !TYPES.contains(&name.as_str())) {
        return Err(malformed(path, "type names a type outside the subset"));
    }
    Ok(names)
}

fn malformed(path: &str, detail: &str) -> SchemaError {
    SchemaError::Malformed {
        path: path.to_string(),
        detail: detail.to_string(),
    }
}

/// Validates `value` against a schema already checked by [`check_subset`].
/// Returns the first violation as `path: problem`, with the path rooted at
/// `arguments`.
pub fn validate(schema: &JsonValue, value: &JsonValue) -> Result<(), String> {
    check_value(schema, value, ROOT)
}

fn check_value(schema: &JsonValue, value: &JsonValue, path: &str) -> Result<(), String> {
    // A schema that passed `check_subset` always has an object shape here.
    let JsonValue::Object(_) = schema else {
        return Err(format!("{path}: schema is not an object"));
    };
    let types = declared_types(schema, path).map_err(|e| e.to_string())?;
    if !types.is_empty() && !types.iter().any(|t| type_matches(t, value)) {
        return Err(type_error(&types, path));
    }
    if let Some(JsonValue::Array(choices)) = schema.get("enum") {
        let admitted = matches!(value, JsonValue::Str(text)
            if choices.iter().any(|c| matches!(c, JsonValue::Str(c) if c == text)));
        if !admitted {
            return Err(format!("{path}: not one of the allowed values"));
        }
    }
    match value {
        JsonValue::Number(number) => check_bounds(schema, *number, path)?,
        JsonValue::Object(entries) => check_object(schema, entries, path)?,
        JsonValue::Array(items) => {
            if let Some(item_schema) = schema.get("items") {
                for (index, item) in items.iter().enumerate() {
                    check_value(item_schema, item, &format!("{path}[{index}]"))?;
                }
            }
        }
        _ => {}
    }
    Ok(())
}

fn check_object(
    schema: &JsonValue,
    entries: &[(String, JsonValue)],
    path: &str,
) -> Result<(), String> {
    let declared: &[(String, JsonValue)] = match schema.get("properties") {
        Some(JsonValue::Object(declared)) => declared,
        _ => &[],
    };
    for (index, (key, _)) in entries.iter().enumerate() {
        if entries[..index].iter().any(|(earlier, _)| earlier == key) {
            return Err(format!("{path}.{key}: duplicate field"));
        }
        if !declared.iter().any(|(name, _)| name == key) {
            return Err(format!("{path}.{key}: unknown field"));
        }
    }
    if let Some(JsonValue::Array(required)) = schema.get("required") {
        for name in required {
            if let JsonValue::Str(name) = name
                && !entries.iter().any(|(key, _)| key == name)
            {
                return Err(format!("{path}: missing field {name}"));
            }
        }
    }
    for (key, child) in entries {
        if let Some((_, child_schema)) = declared.iter().find(|(name, _)| name == key) {
            check_value(child_schema, child, &format!("{path}.{key}"))?;
        }
    }
    Ok(())
}

fn check_bounds(schema: &JsonValue, number: JsonNumber, path: &str) -> Result<(), String> {
    let Some(integer) = exact_integer(number) else {
        return Ok(());
    };
    if let Some(JsonValue::Number(min)) = schema.get("minimum")
        && let Some(min) = exact_integer(*min)
        && integer < min
    {
        return Err(format!("{path}: below the minimum {min}"));
    }
    if let Some(JsonValue::Number(max)) = schema.get("maximum")
        && let Some(max) = exact_integer(*max)
        && integer > max
    {
        return Err(format!("{path}: above the maximum {max}"));
    }
    Ok(())
}

fn exact_integer(number: JsonNumber) -> Option<i128> {
    match number {
        JsonNumber::Uint(n) => Some(i128::from(n)),
        JsonNumber::NegInt(n) => Some(i128::from(n)),
        JsonNumber::Float(_) => None,
    }
}

fn type_matches(name: &str, value: &JsonValue) -> bool {
    match (name, value) {
        ("object", JsonValue::Object(_))
        | ("string", JsonValue::Str(_))
        | ("boolean", JsonValue::Bool(_))
        | ("array", JsonValue::Array(_))
        | ("null", JsonValue::Null) => true,
        ("integer", JsonValue::Number(number)) => exact_integer(*number).is_some(),
        _ => false,
    }
}

fn type_error(types: &[String], path: &str) -> String {
    match types {
        [only] => match only.as_str() {
            "object" => format!("{path}: expected an object"),
            "string" => format!("{path}: expected a string"),
            "integer" => format!("{path}: expected an integer"),
            "boolean" => format!("{path}: expected a boolean"),
            "array" => format!("{path}: expected an array"),
            _ => format!("{path}: expected null"),
        },
        _ => format!("{path}: expected one of {}", types.join(", ")),
    }
}

/// Checks the size and depth bounds on an argument document, before any schema
/// work. Depth recursion stops one level past the bound.
pub fn check_argument_bounds(value: &JsonValue) -> Result<(), String> {
    if exceeds_depth(value, 0) {
        return Err(format!(
            "{ROOT}: nests deeper than {MAX_ARGUMENT_DEPTH} levels"
        ));
    }
    let bytes = value.to_json_text().len();
    if bytes > MAX_ARGUMENT_BYTES {
        return Err(format!("{ROOT}: exceeds {MAX_ARGUMENT_BYTES} bytes"));
    }
    Ok(())
}

fn exceeds_depth(value: &JsonValue, depth: usize) -> bool {
    match value {
        JsonValue::Object(entries) => {
            depth + 1 > MAX_ARGUMENT_DEPTH
                || entries
                    .iter()
                    .any(|(_, child)| exceeds_depth(child, depth + 1))
        }
        JsonValue::Array(items) => {
            depth + 1 > MAX_ARGUMENT_DEPTH
                || items.iter().any(|child| exceeds_depth(child, depth + 1))
        }
        _ => false,
    }
}
