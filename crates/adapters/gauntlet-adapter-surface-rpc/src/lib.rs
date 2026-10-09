//! A stdio projection of the [`ToolSurface`] port.
//!
//! Serves any tool surface over newline-delimited JSON-RPC 2.0, using the
//! tool subset of the Model Context Protocol (`initialize`,
//! `notifications/initialized`, `ping`, `tools/list`, `tools/call`). The
//! protocol is MCP-compatible at the wire level; this adapter owns transport
//! only. The surface owns argument decoding, refusal, and result encoding.
//!
//! Wire rules:
//!
//! - Each message is exactly one JSON document on one line, terminated by
//!   `\n`. Output never contains a raw newline; the JSON writer escapes it.
//! - Every response is flushed before the next request is read.
//! - Input lines are read with a bounded reader. A line longer than
//!   [`ServeLimits::max_line_bytes`] is answered once and ends the session.
//! - Only protocol lines are ever written to the output.

mod framing;

use std::io::{self, BufRead, Write};

use framing::{Frame, read_frame};
use gauntlet_ports::json::{JsonNumber, JsonValue, parse};
use gauntlet_ports::tool_surface::{ToolOutcome, ToolSurface};

/// Protocol revisions this server speaks, newest first.
pub const SUPPORTED_PROTOCOL_VERSIONS: [&str; 3] = ["2025-06-18", "2025-03-26", "2024-11-05"];

/// The tool-subset method names this transport answers. Public so a caller
/// names the protocol once instead of spelling it.
pub const METHOD_TOOLS_LIST: &str = "tools/list";
pub const METHOD_TOOLS_CALL: &str = "tools/call";

/// Default bound on one input line: 1 MiB.
pub const DEFAULT_MAX_LINE_BYTES: usize = 1024 * 1024;

const PARSE_ERROR: i64 = -32700;
const INVALID_REQUEST: i64 = -32600;
const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;
const INTERNAL_ERROR: i64 = -32603;
const NOT_INITIALIZED: i64 = -32002;

/// The identity this server reports in `initialize`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServerInfo {
    pub name: String,
    pub version: String,
}

/// Bounds on untrusted input.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ServeLimits {
    /// The longest accepted input line, terminator excluded.
    pub max_line_bytes: usize,
}

impl Default for ServeLimits {
    fn default() -> Self {
        Self {
            max_line_bytes: DEFAULT_MAX_LINE_BYTES,
        }
    }
}

/// How a served session ended.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ServeEnd {
    /// The input reached end of stream.
    Eof,
    /// The client violated framing limits; the session was closed. The string
    /// names the violation.
    ProtocolViolation(String),
}

/// Serves `surface` over `input` and `output` until end of stream or a
/// protocol violation. Returns `Err` only for I/O failure on the streams.
pub fn serve<R: BufRead, W: Write>(
    surface: &dyn ToolSurface,
    info: &ServerInfo,
    limits: &ServeLimits,
    mut input: R,
    mut output: W,
) -> io::Result<ServeEnd> {
    let mut initialized = false;
    let mut line = Vec::new();
    loop {
        match read_frame(&mut input, limits.max_line_bytes, &mut line)? {
            Frame::End => return Ok(ServeEnd::Eof),
            Frame::TooLong => {
                let detail = format!("request line exceeds {} bytes", limits.max_line_bytes);
                let reply = failure(
                    JsonValue::Null,
                    RpcError::new(INVALID_REQUEST, detail.clone()),
                );
                write_message(&mut output, &reply)?;
                return Ok(ServeEnd::ProtocolViolation(detail));
            }
            Frame::Line => {
                if line.iter().all(u8::is_ascii_whitespace) {
                    continue;
                }
                let reply = match std::str::from_utf8(&line) {
                    Ok(text) => handle(surface, info, &mut initialized, text),
                    Err(_) => Some(failure(
                        JsonValue::Null,
                        RpcError::new(PARSE_ERROR, "request is not valid UTF-8"),
                    )),
                };
                if let Some(reply) = reply {
                    write_message(&mut output, &reply)?;
                }
            }
        }
    }
}

/// One dispatch failure: a JSON-RPC error code and message.
struct RpcError {
    code: i64,
    message: String,
}

impl RpcError {
    fn new(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
        }
    }
}

/// Handles one input line. `None` means the message was a notification and
/// the protocol forbids any response to it.
fn handle(
    surface: &dyn ToolSurface,
    info: &ServerInfo,
    initialized: &mut bool,
    text: &str,
) -> Option<JsonValue> {
    let message = match parse(text) {
        Ok(message) => message,
        Err(error) => {
            return Some(failure(
                JsonValue::Null,
                RpcError::new(PARSE_ERROR, format!("parse error: {error}")),
            ));
        }
    };
    match message {
        JsonValue::Object(_) => {}
        JsonValue::Array(_) => {
            return Some(failure(
                JsonValue::Null,
                RpcError::new(INVALID_REQUEST, "batch requests are not supported"),
            ));
        }
        _ => {
            return Some(failure(
                JsonValue::Null,
                RpcError::new(INVALID_REQUEST, "request must be a JSON object"),
            ));
        }
    }
    let id = match message.get("id") {
        None => None,
        Some(value) => match value {
            JsonValue::Str(_) | JsonValue::Number(_) | JsonValue::Null => Some(value.clone()),
            _ => {
                return Some(failure(
                    JsonValue::Null,
                    RpcError::new(
                        INVALID_REQUEST,
                        "request id must be a string, number, or null",
                    ),
                ));
            }
        },
    };
    let method = match message.get("method") {
        Some(JsonValue::Str(method)) => method.as_str(),
        _ => {
            return id.map(|id| {
                failure(
                    id,
                    RpcError::new(INVALID_REQUEST, "request has no string method"),
                )
            });
        }
    };
    let Some(id) = id else {
        // Notifications never get a response, even when they fail. Only the
        // initialized notification changes session state; every other
        // notification is dropped without dispatch.
        if method == "notifications/initialized" {
            *initialized = true;
        }
        return None;
    };
    if !*initialized && method != "initialize" && method != "ping" {
        return Some(failure(
            id,
            RpcError::new(NOT_INITIALIZED, "server not initialized"),
        ));
    }
    let params = message.get("params");
    let outcome = match method {
        "initialize" => Ok(initialize(info, params)),
        "ping" => Ok(obj(Vec::new())),
        METHOD_TOOLS_LIST => list_tools(surface),
        METHOD_TOOLS_CALL => call_tool(surface, params),
        other => Err(RpcError::new(
            METHOD_NOT_FOUND,
            format!("method not found: {other}"),
        )),
    };
    Some(match outcome {
        Ok(result) => success(id, result),
        Err(error) => failure(id, error),
    })
}

fn initialize(info: &ServerInfo, params: Option<&JsonValue>) -> JsonValue {
    let requested = params.and_then(|p| p.get("protocolVersion"));
    let version = match requested {
        Some(JsonValue::Str(requested))
            if SUPPORTED_PROTOCOL_VERSIONS.contains(&requested.as_str()) =>
        {
            requested.as_str()
        }
        _ => SUPPORTED_PROTOCOL_VERSIONS[0],
    };
    obj(vec![
        ("protocolVersion", string(version)),
        (
            "capabilities",
            obj(vec![(
                "tools",
                obj(vec![("listChanged", JsonValue::Bool(false))]),
            )]),
        ),
        (
            "serverInfo",
            obj(vec![
                ("name", string(&info.name)),
                ("version", string(&info.version)),
            ]),
        ),
    ])
}

fn list_tools(surface: &dyn ToolSurface) -> Result<JsonValue, RpcError> {
    let mut tools = Vec::new();
    for listing in surface.tools() {
        let schema = parse(&listing.input_schema).map_err(|error| {
            RpcError::new(
                INTERNAL_ERROR,
                format!(
                    "tool {} has an unparseable input schema: {error}",
                    listing.name
                ),
            )
        })?;
        tools.push(obj(vec![
            ("name", string(&listing.name)),
            ("description", string(&listing.description)),
            ("inputSchema", schema),
        ]));
    }
    Ok(obj(vec![("tools", JsonValue::Array(tools))]))
}

fn call_tool(surface: &dyn ToolSurface, params: Option<&JsonValue>) -> Result<JsonValue, RpcError> {
    let params = match params {
        Some(params @ JsonValue::Object(_)) => params,
        _ => {
            return Err(RpcError::new(
                INVALID_PARAMS,
                "tools/call params must be an object",
            ));
        }
    };
    let name = match params.get("name") {
        Some(JsonValue::Str(name)) => name.as_str(),
        _ => {
            return Err(RpcError::new(
                INVALID_PARAMS,
                "tools/call name must be a string",
            ));
        }
    };
    let arguments = params
        .get("arguments")
        .cloned()
        .unwrap_or_else(|| obj(Vec::new()));
    match surface.call(name, &arguments) {
        ToolOutcome::Ok(value) => Ok(tool_result(&value.to_json_text(), false)),
        ToolOutcome::Refused { code, detail } => {
            let refusal = obj(vec![("code", string(&code)), ("detail", string(&detail))]);
            Ok(tool_result(&refusal.to_json_text(), true))
        }
        ToolOutcome::Invalid { detail } => Err(RpcError::new(INVALID_PARAMS, detail)),
    }
}

fn tool_result(text: &str, is_error: bool) -> JsonValue {
    obj(vec![
        (
            "content",
            JsonValue::Array(vec![obj(vec![
                ("type", string("text")),
                ("text", string(text)),
            ])]),
        ),
        ("isError", JsonValue::Bool(is_error)),
    ])
}

fn success(id: JsonValue, result: JsonValue) -> JsonValue {
    obj(vec![
        ("jsonrpc", string("2.0")),
        ("id", id),
        ("result", result),
    ])
}

fn failure(id: JsonValue, error: RpcError) -> JsonValue {
    obj(vec![
        ("jsonrpc", string("2.0")),
        ("id", id),
        (
            "error",
            obj(vec![
                ("code", JsonValue::Number(JsonNumber::NegInt(error.code))),
                ("message", JsonValue::Str(error.message)),
            ]),
        ),
    ])
}

fn obj(entries: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value))
            .collect(),
    )
}

fn string(text: &str) -> JsonValue {
    JsonValue::Str(text.to_owned())
}

fn write_message<W: Write>(output: &mut W, value: &JsonValue) -> io::Result<()> {
    let mut text = value.to_json_text();
    text.push('\n');
    output.write_all(text.as_bytes())?;
    output.flush()
}
