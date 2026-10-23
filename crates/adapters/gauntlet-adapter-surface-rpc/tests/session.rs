use std::io::{self, BufReader, Read};

use gauntlet_adapter_surface_rpc::{ServeEnd, ServeLimits, ServerInfo, serve};
use gauntlet_ports::json::{JsonNumber, JsonValue, parse};
use gauntlet_ports::tool_surface::{ToolListing, ToolOutcome, ToolSurface};

/// Two tools whose outcomes the test controls.
struct Fake {
    echo: ToolOutcome,
    probe: ToolOutcome,
}

impl ToolSurface for Fake {
    fn tools(&self) -> Vec<ToolListing> {
        vec![listing("echo"), listing("probe")]
    }

    fn call(&self, name: &str, _arguments: &JsonValue) -> ToolOutcome {
        match name {
            "echo" => self.echo.clone(),
            "probe" => self.probe.clone(),
            other => ToolOutcome::Invalid {
                detail: format!("unknown tool {other}"),
            },
        }
    }
}

fn listing(name: &str) -> ToolListing {
    ToolListing {
        name: name.to_owned(),
        description: format!("the {name} tool"),
        input_schema: r#"{"type":"object","properties":{"text":{"type":"string"}}}"#.to_owned(),
    }
}

fn fake(echo: ToolOutcome, probe: ToolOutcome) -> Fake {
    Fake { echo, probe }
}

fn info() -> ServerInfo {
    ServerInfo {
        name: "test-server".to_owned(),
        version: "0.0.1".to_owned(),
    }
}

/// Serves `input` and returns the end state plus the raw output text.
fn run_raw(surface: &dyn ToolSurface, input: &[u8], limits: &ServeLimits) -> (ServeEnd, String) {
    let mut output = Vec::new();
    let end = serve(surface, &info(), limits, input, &mut output).expect("in-memory io");
    (end, String::from_utf8(output).expect("output is utf-8"))
}

/// Serves one request per line and returns each output line parsed as JSON.
fn run(surface: &dyn ToolSurface, lines: &[&str]) -> (ServeEnd, Vec<JsonValue>) {
    let input: String = lines.iter().map(|line| format!("{line}\n")).collect();
    let (end, raw) = run_raw(surface, input.as_bytes(), &ServeLimits::default());
    assert!(
        raw.is_empty() || raw.ends_with('\n'),
        "every line is terminated"
    );
    let replies = raw
        .lines()
        .map(|line| parse(line).expect("every output line is one JSON document"))
        .collect();
    (end, replies)
}

fn s(text: &str) -> JsonValue {
    JsonValue::Str(text.to_owned())
}

fn error_code(reply: &JsonValue) -> Option<i64> {
    match reply.get("error")?.get("code")? {
        JsonValue::Number(number) => number.as_i64(),
        _ => None,
    }
}

/// The parsed tool value carried in `result.content[0].text`.
fn tool_text(reply: &JsonValue) -> JsonValue {
    let content = reply
        .get("result")
        .and_then(|result| result.get("content"))
        .expect("tool result has content");
    let JsonValue::Array(items) = content else {
        panic!("content is an array");
    };
    let JsonValue::Str(text) = items[0].get("text").expect("text item") else {
        panic!("text is a string");
    };
    parse(text).expect("tool text is JSON")
}

const INIT: &str = r#"{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test-client","version":"0.0.1"}}}"#;
const INITIALIZED: &str = r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#;
const ECHO_CALL: &str = r#"{"jsonrpc":"2.0","id":5,"method":"tools/call","params":{"name":"echo","arguments":{"text":"hi"}}}"#;
const PROBE_CALL: &str =
    r#"{"jsonrpc":"2.0","id":6,"method":"tools/call","params":{"name":"probe","arguments":{}}}"#;

fn ok_surface() -> Fake {
    fake(
        ToolOutcome::Ok(JsonValue::Object(vec![(
            "answer".to_owned(),
            JsonValue::Number(JsonNumber::Uint(42)),
        )])),
        ToolOutcome::Ok(JsonValue::Null),
    )
}

#[test]
fn handshake_echoes_a_supported_client_version() {
    let (end, replies) = run(&ok_surface(), &[INIT]);
    assert_eq!(end, ServeEnd::Eof);
    let result = replies[0].get("result").expect("result");
    assert_eq!(result.get("protocolVersion"), Some(&s("2025-03-26")));
    assert_eq!(
        result
            .get("capabilities")
            .and_then(|c| c.get("tools"))
            .and_then(|t| t.get("listChanged")),
        Some(&JsonValue::Bool(false))
    );
    assert_eq!(
        result.get("serverInfo").and_then(|i| i.get("name")),
        Some(&s("test-server"))
    );
}

#[test]
fn handshake_answers_an_unsupported_version_with_the_newest() {
    let init = INIT.replace("2025-03-26", "1999-01-01");
    let (_, replies) = run(&ok_surface(), &[init.as_str()]);
    let result = replies[0].get("result").expect("result");
    assert_eq!(result.get("protocolVersion"), Some(&s("2025-06-18")));
}

#[test]
fn initialized_notification_yields_no_output() {
    let (_, replies) = run(&ok_surface(), &[INITIALIZED]);
    assert!(replies.is_empty());
}

#[test]
fn ping_is_allowed_before_initialize() {
    let (_, replies) = run(
        &ok_surface(),
        &[r#"{"jsonrpc":"2.0","id":7,"method":"ping"}"#],
    );
    assert_eq!(
        replies[0].get("id"),
        Some(&JsonValue::Number(JsonNumber::Uint(7)))
    );
    assert_eq!(
        replies[0].get("result"),
        Some(&JsonValue::Object(Vec::new()))
    );
}

#[test]
fn tools_list_embeds_each_schema_as_an_object() {
    let (_, replies) = run(
        &ok_surface(),
        &[
            INIT,
            INITIALIZED,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
        ],
    );
    let result = replies[1].get("result").expect("result");
    let JsonValue::Array(tools) = result.get("tools").expect("tools") else {
        panic!("tools is an array");
    };
    assert_eq!(tools.len(), 2);
    assert_eq!(tools[0].get("name"), Some(&s("echo")));
    assert!(matches!(
        tools[0].get("inputSchema"),
        Some(JsonValue::Object(_))
    ));
    assert_eq!(
        tools[0]
            .get("inputSchema")
            .and_then(|schema| schema.get("type")),
        Some(&s("object"))
    );
}

#[test]
fn tools_call_ok_returns_compact_json_text() {
    let (_, replies) = run(&ok_surface(), &[INIT, INITIALIZED, ECHO_CALL]);
    let result = replies[1].get("result").expect("result");
    assert_eq!(result.get("isError"), Some(&JsonValue::Bool(false)));
    assert_eq!(
        tool_text(&replies[1]).get("answer"),
        Some(&JsonValue::Number(JsonNumber::Uint(42)))
    );
}

#[test]
fn tools_call_refusal_is_an_error_result_not_a_protocol_error() {
    let surface = fake(
        ToolOutcome::Refused {
            code: "authority".to_owned(),
            detail: "not allowed here".to_owned(),
        },
        ToolOutcome::Ok(JsonValue::Null),
    );
    let (_, replies) = run(&surface, &[INIT, INITIALIZED, ECHO_CALL]);
    assert!(replies[1].get("error").is_none());
    let result = replies[1].get("result").expect("result");
    assert_eq!(result.get("isError"), Some(&JsonValue::Bool(true)));
    let body = tool_text(&replies[1]);
    assert_eq!(body.get("code"), Some(&s("authority")));
    assert_eq!(body.get("detail"), Some(&s("not allowed here")));
}

#[test]
fn tools_call_invalid_is_a_32602_error_with_the_detail() {
    let surface = fake(
        ToolOutcome::Invalid {
            detail: "arguments.text: expected string".to_owned(),
        },
        ToolOutcome::Ok(JsonValue::Null),
    );
    let (_, replies) = run(&surface, &[INIT, INITIALIZED, ECHO_CALL]);
    assert_eq!(error_code(&replies[1]), Some(-32602));
    assert_eq!(
        replies[1].get("error").and_then(|e| e.get("message")),
        Some(&s("arguments.text: expected string"))
    );
}

#[test]
fn tools_call_before_initialized_is_rejected() {
    let (_, replies) = run(&ok_surface(), &[INIT, ECHO_CALL]);
    assert_eq!(error_code(&replies[1]), Some(-32002));
}

#[test]
fn tools_list_before_initialize_is_rejected() {
    let (_, replies) = run(
        &ok_surface(),
        &[r#"{"jsonrpc":"2.0","id":3,"method":"tools/list"}"#],
    );
    assert_eq!(error_code(&replies[0]), Some(-32002));
    assert_eq!(
        replies[0].get("id"),
        Some(&JsonValue::Number(JsonNumber::Uint(3)))
    );
}

#[test]
fn unknown_method_is_32601() {
    let (_, replies) = run(
        &ok_surface(),
        &[
            INIT,
            INITIALIZED,
            r#"{"jsonrpc":"2.0","id":4,"method":"resources/list"}"#,
        ],
    );
    assert_eq!(error_code(&replies[1]), Some(-32601));
}

#[test]
fn malformed_json_is_32700_with_null_id_and_the_session_continues() {
    let (end, replies) = run(
        &ok_surface(),
        &["{nope", r#"{"jsonrpc":"2.0","id":3,"method":"ping"}"#],
    );
    assert_eq!(end, ServeEnd::Eof);
    assert_eq!(error_code(&replies[0]), Some(-32700));
    assert_eq!(replies[0].get("id"), Some(&JsonValue::Null));
    assert_eq!(
        replies[1].get("id"),
        Some(&JsonValue::Number(JsonNumber::Uint(3)))
    );
    assert!(replies[1].get("result").is_some());
}

#[test]
fn batch_is_rejected_with_32600() {
    let (_, replies) = run(
        &ok_surface(),
        &[r#"[{"jsonrpc":"2.0","id":1,"method":"ping"}]"#],
    );
    assert_eq!(error_code(&replies[0]), Some(-32600));
    assert_eq!(replies[0].get("id"), Some(&JsonValue::Null));
}

#[test]
fn string_and_numeric_ids_echo_exactly() {
    let (_, replies) = run(
        &ok_surface(),
        &[
            r#"{"jsonrpc":"2.0","id":"abc-1","method":"ping"}"#,
            r#"{"jsonrpc":"2.0","id":42,"method":"ping"}"#,
            r#"{"jsonrpc":"2.0","id":1.5,"method":"ping"}"#,
        ],
    );
    assert_eq!(replies[0].get("id"), Some(&s("abc-1")));
    assert_eq!(
        replies[1].get("id"),
        Some(&JsonValue::Number(JsonNumber::Uint(42)))
    );
    assert_eq!(
        replies[2].get("id"),
        Some(&JsonValue::Number(JsonNumber::Float(1.5)))
    );
}

#[test]
fn notifications_never_get_a_response_even_on_error() {
    let (_, replies) = run(
        &ok_surface(),
        &[
            r#"{"jsonrpc":"2.0","method":"bogus/thing"}"#,
            r#"{"jsonrpc":"2.0","method":"tools/call","params":{"name":"echo"}}"#,
            r#"{"jsonrpc":"2.0","method":"tools/list"}"#,
        ],
    );
    assert!(replies.is_empty());
}

#[test]
fn oversized_line_is_answered_once_and_ends_the_session() {
    let line = format!("{}\n", "a".repeat(100));
    let limits = ServeLimits { max_line_bytes: 16 };
    let (end, raw) = run_raw(&ok_surface(), line.as_bytes(), &limits);
    assert!(matches!(end, ServeEnd::ProtocolViolation(_)));
    assert_eq!(raw.lines().count(), 1);
    let reply = parse(raw.trim_end()).expect("one JSON reply");
    assert_eq!(error_code(&reply), Some(-32600));
}

/// An input that never ends and counts how much the reader handed out.
struct Endless {
    served: usize,
}

impl Read for Endless {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        buf.fill(b'a');
        self.served += buf.len();
        Ok(buf.len())
    }
}

#[test]
fn an_unterminated_endless_line_is_bounded_and_refused_promptly() {
    let mut reader = BufReader::with_capacity(4096, Endless { served: 0 });
    let limits = ServeLimits {
        max_line_bytes: 1000,
    };
    let mut output = Vec::new();
    let end =
        serve(&ok_surface(), &info(), &limits, &mut reader, &mut output).expect("in-memory io");
    assert!(matches!(end, ServeEnd::ProtocolViolation(_)));
    // The reader stopped at the first chunk that crossed the limit.
    assert!(
        reader.get_ref().served <= 1000 + 4096,
        "served {}",
        reader.get_ref().served
    );
    assert_eq!(String::from_utf8(output).expect("utf-8").lines().count(), 1);
}

#[test]
fn end_of_stream_ends_cleanly_without_output() {
    let (end, replies) = run(&ok_surface(), &[]);
    assert_eq!(end, ServeEnd::Eof);
    assert!(replies.is_empty());
}

#[test]
fn tool_text_with_newlines_quotes_and_unicode_stays_on_one_line() {
    let tricky = "line one\nline \"two\"\t\\ é \u{1F600} \u{2028}\u{1}";
    let surface = fake(
        ToolOutcome::Ok(JsonValue::Str(tricky.to_owned())),
        ToolOutcome::Ok(JsonValue::Null),
    );
    let (_, raw) = run_raw(
        &surface,
        format!("{INIT}\n{INITIALIZED}\n{ECHO_CALL}\n").as_bytes(),
        &ServeLimits::default(),
    );
    assert_eq!(
        raw.lines().count(),
        2,
        "one line per response, no raw newline"
    );
    let reply = parse(raw.lines().nth(1).expect("call reply")).expect("json");
    assert_eq!(tool_text(&reply), JsonValue::Str(tricky.to_owned()));
}

#[test]
fn tool_result_text_round_trips_to_the_original_value() {
    let original = JsonValue::Object(vec![
        ("uint".to_owned(), JsonValue::Number(JsonNumber::Uint(3))),
        ("neg".to_owned(), JsonValue::Number(JsonNumber::NegInt(-7))),
        (
            "float".to_owned(),
            JsonValue::Number(JsonNumber::Float(0.25)),
        ),
        (
            "flags".to_owned(),
            JsonValue::Array(vec![JsonValue::Bool(true), JsonValue::Null]),
        ),
        (
            "text".to_owned(),
            JsonValue::Str("q\"uo\\te\nnl".to_owned()),
        ),
        (
            "nested".to_owned(),
            JsonValue::Object(vec![("k".to_owned(), JsonValue::Str("v".to_owned()))]),
        ),
    ]);
    let surface = fake(
        ToolOutcome::Ok(original.clone()),
        ToolOutcome::Ok(JsonValue::Null),
    );
    let (_, replies) = run(&surface, &[INIT, INITIALIZED, ECHO_CALL]);
    assert_eq!(tool_text(&replies[1]), original);
}

#[test]
fn two_full_sessions_in_one_stream_stay_in_order() {
    let (_, replies) = run(
        &ok_surface(),
        &[
            INIT,
            INITIALIZED,
            r#"{"jsonrpc":"2.0","id":2,"method":"tools/list"}"#,
            r#"{"jsonrpc":"2.0","id":3,"method":"initialize","params":{"protocolVersion":"2024-11-05"}}"#,
            r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#,
            PROBE_CALL,
        ],
    );
    let ids: Vec<JsonValue> = replies
        .iter()
        .filter_map(|r| r.get("id").cloned())
        .collect();
    assert_eq!(
        ids,
        vec![
            JsonValue::Number(JsonNumber::Uint(1)),
            JsonValue::Number(JsonNumber::Uint(2)),
            JsonValue::Number(JsonNumber::Uint(3)),
            JsonValue::Number(JsonNumber::Uint(6)),
        ]
    );
    assert!(replies.iter().all(|r| r.get("result").is_some()));
    assert_eq!(
        replies[2]
            .get("result")
            .and_then(|r| r.get("protocolVersion")),
        Some(&s("2024-11-05"))
    );
}
