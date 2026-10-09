//! The codemode host protocol, version 1 (`workbench/src/codemode/PROTOCOL.md`).
//!
//! Encoding is total. Decoding is strict: every line must be one JSON object
//! with `"v": 1`, a known `t`, exactly the keys the message allows, and the
//! fields it requires. Anything else is a fault, and the caller fails closed.

use gauntlet_ports::json::{JsonNumber, JsonValue, parse};
use gauntlet_ports::tool_catalog::ToolName;
use gauntlet_ports::tool_program::{FailureClass, ProgramLimit, ProgramStatus, ToolProgramRequest};
use gauntlet_ports::tool_surface::ToolOutcome;

/// The only protocol version this adapter speaks.
pub(crate) const VERSION: u64 = 1;

/// Host messages the parent accepts.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum HostMessage {
    /// One tool call. `id` is the sandbox's id for it.
    Call {
        id: u64,
        tool: ToolName,
        args: JsonValue,
    },
    /// The program's end, already mapped to the port's status.
    Done(ProgramStatus),
}

/// Renders the `run` message. `timeout_ms` is the host-side bound; the caller
/// has already clamped it to at least one millisecond.
pub(crate) fn encode_run(request: &ToolProgramRequest, timeout_ms: u64) -> String {
    let limits = request.limits();
    object(vec![
        ("v", number(VERSION)),
        ("t", text("run")),
        ("source", text(request.source().text())),
        (
            "admitted",
            JsonValue::Array(
                request
                    .admitted()
                    .iter()
                    .map(|name| text(name.as_str()))
                    .collect(),
            ),
        ),
        (
            "limits",
            object(vec![
                ("timeout_ms", number(timeout_ms)),
                ("max_tool_calls", number(u64::from(limits.max_tool_calls))),
                (
                    "max_parallel_calls",
                    number(u64::from(limits.max_parallel_calls)),
                ),
                (
                    "max_result_bytes",
                    number(u64::try_from(limits.max_result_bytes).unwrap_or(u64::MAX)),
                ),
                (
                    "max_source_bytes",
                    number(u64::try_from(limits.max_source_bytes).unwrap_or(u64::MAX)),
                ),
            ]),
        ),
    ])
    .to_json_text()
}

/// Renders the `result` answering call `id`. A refusal is data to the program
/// (it arrives as a thrown `ToolError`), so every outcome is delivered.
pub(crate) fn encode_result(id: u64, outcome: &ToolOutcome) -> String {
    let body = match outcome {
        ToolOutcome::Ok(value) => object(vec![("kind", text("ok")), ("value", value.clone())]),
        ToolOutcome::Refused { code, detail } => object(vec![
            ("kind", text("refused")),
            ("value", object(vec![("code", text(code))])),
            ("message", text(detail)),
        ]),
        ToolOutcome::Invalid { detail } => {
            object(vec![("kind", text("invalid")), ("message", text(detail))])
        }
    };
    object(vec![
        ("v", number(VERSION)),
        ("t", text("result")),
        ("id", number(id)),
        ("outcome", body),
    ])
    .to_json_text()
}

/// Renders the `cancel` message, which stops the program at once.
pub(crate) fn encode_cancel() -> String {
    object(vec![("v", number(VERSION)), ("t", text("cancel"))]).to_json_text()
}

/// Decodes one host line strictly. The error text names the first rule broken.
pub(crate) fn decode(line: &str) -> Result<HostMessage, String> {
    let value = parse(line).map_err(|error| format!("a host line is not JSON: {error}"))?;
    let JsonValue::Object(fields) = value else {
        return Err("a host line is not a JSON object".to_owned());
    };
    match fields.iter().find(|(key, _)| key == "v") {
        Some((_, JsonValue::Number(number))) if number.as_u64() == Some(VERSION) => {}
        _ => return Err("a host line has the wrong protocol version".to_owned()),
    }
    match str_field(&fields, "t")? {
        "call" => decode_call(&fields),
        "done" => decode_done(&fields),
        other => Err(format!("unknown host message type {other:?}")),
    }
}

fn decode_call(fields: &[(String, JsonValue)]) -> Result<HostMessage, String> {
    check_keys(fields, &["v", "t", "id", "tool", "args"])?;
    let id = match field(fields, "id") {
        Some(JsonValue::Number(number)) => number
            .as_u64()
            .ok_or_else(|| "a call id is a non-negative integer".to_owned())?,
        _ => return Err("a call id is a non-negative integer".to_owned()),
    };
    let tool = ToolName::new(str_field(fields, "tool")?)
        .map_err(|error| format!("a call names an invalid tool: {error}"))?;
    let args = field(fields, "args")
        .cloned()
        .ok_or_else(|| "a call has no args".to_owned())?;
    Ok(HostMessage::Call { id, tool, args })
}

fn decode_done(fields: &[(String, JsonValue)]) -> Result<HostMessage, String> {
    check_keys(fields, &["v", "t", "status", "value", "message", "limit"])?;
    let status = str_field(fields, "status")?;
    let value = field(fields, "value")
        .cloned()
        .ok_or_else(|| "a done message has no value".to_owned())?;
    let message = optional_str(fields, "message")?;
    let limit = optional_str(fields, "limit")?;
    let status = match status {
        "completed" => {
            if message.is_some() || limit.is_some() {
                return Err("a completed done message carries no message or limit".to_owned());
            }
            ProgramStatus::Completed(value)
        }
        "failed" | "timed_out" | "cancelled" | "limit_exceeded" => {
            if value != JsonValue::Null {
                return Err(format!("a {status} done message carries a null value"));
            }
            let message = message
                .ok_or_else(|| format!("a {status} done message carries a message"))?
                .to_owned();
            match status {
                "failed" => {
                    if limit.is_some() {
                        return Err("only a limit_exceeded done message names a limit".to_owned());
                    }
                    let class = if message.starts_with("SyntaxError") {
                        FailureClass::Script
                    } else {
                        FailureClass::Runtime
                    };
                    ProgramStatus::Failed { class, message }
                }
                "timed_out" => ProgramStatus::TimedOut,
                "cancelled" => ProgramStatus::Cancelled,
                _ => ProgramStatus::LimitExceeded(map_limit(limit)?),
            }
        }
        other => return Err(format!("unknown done status {other:?}")),
    };
    Ok(HostMessage::Done(status))
}

fn map_limit(limit: Option<&str>) -> Result<ProgramLimit, String> {
    match limit {
        Some("tool_calls") => Ok(ProgramLimit::ToolCalls),
        Some("parallel") => Ok(ProgramLimit::ParallelCalls),
        Some("result_bytes") => Ok(ProgramLimit::ResultBytes),
        Some("source_bytes") => Ok(ProgramLimit::SourceBytes),
        Some("memory") => Ok(ProgramLimit::Memory),
        Some(other) => Err(format!("unknown limit {other:?}")),
        None => Err("a limit_exceeded done message names no limit".to_owned()),
    }
}

fn check_keys(fields: &[(String, JsonValue)], allowed: &[&str]) -> Result<(), String> {
    for (key, _) in fields {
        if !allowed.contains(&key.as_str()) {
            return Err(format!("an unknown key {key:?} is a protocol fault"));
        }
    }
    Ok(())
}

fn field<'a>(fields: &'a [(String, JsonValue)], key: &str) -> Option<&'a JsonValue> {
    fields
        .iter()
        .find(|(name, _)| name == key)
        .map(|(_, value)| value)
}

fn str_field<'a>(fields: &'a [(String, JsonValue)], key: &str) -> Result<&'a str, String> {
    match field(fields, key) {
        Some(JsonValue::Str(text)) => Ok(text),
        _ => Err(format!(
            "the field {key:?} is required and must be a string"
        )),
    }
}

fn optional_str<'a>(
    fields: &'a [(String, JsonValue)],
    key: &str,
) -> Result<Option<&'a str>, String> {
    match field(fields, key) {
        None => Ok(None),
        Some(JsonValue::Str(text)) => Ok(Some(text)),
        Some(_) => Err(format!("the field {key:?} must be a string")),
    }
}

fn object(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        fields
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

fn number(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber::Uint(value))
}

fn text(value: &str) -> JsonValue {
    JsonValue::Str(value.to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn done(line: &str) -> Result<HostMessage, String> {
        decode(line)
    }

    #[test]
    fn a_completed_done_decodes_to_its_value() {
        let message = done(r#"{"v":1,"t":"done","status":"completed","value":{"n":1}}"#);
        assert!(matches!(
            message,
            Ok(HostMessage::Done(ProgramStatus::Completed(_)))
        ));
    }

    #[test]
    fn every_fault_shape_is_refused_with_a_reason() {
        let cases = [
            "not json",
            "[]",
            r#"{"t":"call","id":1,"tool":"tools.read","args":{}}"#,
            r#"{"v":2,"t":"call","id":1,"tool":"tools.read","args":{}}"#,
            r#"{"v":1,"t":"ping"}"#,
            r#"{"v":1,"t":"call","id":1,"tool":"tools.read","args":{},"extra":true}"#,
            r#"{"v":1,"t":"call","id":-1,"tool":"tools.read","args":{}}"#,
            r#"{"v":1,"t":"call","id":1,"tool":"Bad Name","args":{}}"#,
            r#"{"v":1,"t":"done","status":"completed"}"#,
            r#"{"v":1,"t":"done","status":"failed","value":1,"message":"x"}"#,
            r#"{"v":1,"t":"done","status":"limit_exceeded","value":null,"message":"x"}"#,
            r#"{"v":1,"t":"done","status":"limit_exceeded","value":null,"message":"x","limit":"nope"}"#,
            r#"{"v":1,"t":"done","status":"completed","value":null,"message":"x"}"#,
        ];
        for line in cases {
            assert!(decode(line).is_err(), "must refuse {line}");
        }
    }

    #[test]
    fn a_syntax_error_is_a_script_failure_and_others_are_runtime() {
        let script = decode(
            r#"{"v":1,"t":"done","status":"failed","value":null,"message":"SyntaxError: x"}"#,
        );
        assert!(matches!(
            script,
            Ok(HostMessage::Done(ProgramStatus::Failed {
                class: FailureClass::Script,
                ..
            }))
        ));
        let runtime =
            decode(r#"{"v":1,"t":"done","status":"failed","value":null,"message":"TypeError: x"}"#);
        assert!(matches!(
            runtime,
            Ok(HostMessage::Done(ProgramStatus::Failed {
                class: FailureClass::Runtime,
                ..
            }))
        ));
    }

    #[test]
    fn the_result_frame_carries_the_outcome_kind() {
        let text = encode_result(
            7,
            &ToolOutcome::Refused {
                code: "not_admitted".to_owned(),
                detail: "no".to_owned(),
            },
        );
        let parsed = parse(&text).expect("the result is JSON");
        assert_eq!(
            parsed.get("outcome").and_then(|o| o.get("kind")),
            Some(&JsonValue::Str("refused".to_owned()))
        );
        assert_eq!(parsed.get("id"), Some(&number(7)));
    }
}
