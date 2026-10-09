//! Deterministic reference [`ToolProgramRuntime`] (H1). It interprets a tiny
//! declarative script with no engine behind it, so the shared suite in
//! [`super::tool_program`] and the app-level driver can run without any
//! program engine.
//!
//! The source text is a JSON array of steps, run in order:
//!
//! - `{"call": "<tool>", "args": <json>}` issues one call (args default to `{}`).
//! - `{"group": [{"call": ..., "args": ...}, ...]}` issues calls together.
//! - `{"spin": true}` stops the program as if it ran past its timeout.
//! - `{"fail": "<message>"}` stops the program with a runtime failure.
//! - `{"return": <json>}` stops the program and completes with that value.
//!
//! A program that runs out of steps completes with `null`. A source that is not
//! a JSON array of these steps is `Failed(Script)`. The mock never reads a clock,
//! so its duration is zero.

use core::time::Duration;

use crate::contract::tool_program::ToolProgramFactory;
use crate::json::{JsonValue, parse};
use crate::tool_catalog::ToolName;
use crate::tool_program::{
    FailureClass, ProgramHost, ProgramOutcome, ProgramStatus, ToolProgramRequest,
    ToolProgramRuntime, finish_outcome,
};

/// The stateless reference runtime.
#[derive(Debug, Clone, Copy, Default)]
pub struct MockProgramRuntime;

/// Builds the reference runtime for the shared suite.
#[derive(Debug, Clone, Copy, Default)]
pub struct MockProgramRuntimeFactory;

impl ToolProgramFactory for MockProgramRuntimeFactory {
    fn label(&self) -> &'static str {
        "mock-program-reference"
    }

    fn build(&self) -> Result<Box<dyn ToolProgramRuntime>, String> {
        Ok(Box::new(MockProgramRuntime))
    }
}

/// One interpreted step.
enum Step {
    Call(ToolName, JsonValue),
    Group(Vec<(ToolName, JsonValue)>),
    Spin,
    Fail(String),
    Return(JsonValue),
}

impl ToolProgramRuntime for MockProgramRuntime {
    fn run(&self, request: &ToolProgramRequest, host: &dyn ProgramHost) -> ProgramOutcome {
        let status = interpret(request.source().text(), host);
        finish_outcome(status, host.record(), request, Duration::ZERO)
    }
}

fn script_error(message: impl Into<String>) -> ProgramStatus {
    ProgramStatus::Failed {
        class: FailureClass::Script,
        message: message.into(),
    }
}

fn interpret(text: &str, host: &dyn ProgramHost) -> ProgramStatus {
    let steps = match parse(text) {
        Ok(JsonValue::Array(steps)) => steps,
        Ok(_) => return script_error("a program is a JSON array of steps"),
        Err(error) => return script_error(error.to_string()),
    };
    for step in &steps {
        match parse_step(step) {
            Ok(Step::Call(name, args)) => {
                host.call(&name, &args);
            }
            Ok(Step::Group(calls)) => {
                host.call_group(&calls);
            }
            Ok(Step::Spin) => return ProgramStatus::TimedOut,
            Ok(Step::Fail(message)) => {
                return ProgramStatus::Failed {
                    class: FailureClass::Runtime,
                    message,
                };
            }
            Ok(Step::Return(value)) => return ProgramStatus::Completed(value),
            Err(message) => return script_error(message),
        }
    }
    ProgramStatus::Completed(JsonValue::Null)
}

fn parse_step(step: &JsonValue) -> Result<Step, String> {
    if let Some(call) = step.get("call") {
        let name = tool_name(call)?;
        let args = step
            .get("args")
            .cloned()
            .unwrap_or_else(|| JsonValue::Object(Vec::new()));
        return Ok(Step::Call(name, args));
    }
    if let Some(JsonValue::Array(members)) = step.get("group") {
        return members
            .iter()
            .map(group_member)
            .collect::<Result<Vec<_>, _>>()
            .map(Step::Group);
    }
    if step.get("spin").is_some() {
        return Ok(Step::Spin);
    }
    if let Some(JsonValue::Str(message)) = step.get("fail") {
        return Ok(Step::Fail(message.clone()));
    }
    if let Some(value) = step.get("return") {
        return Ok(Step::Return(value.clone()));
    }
    Err("a step is a call, a group, spin, fail, or return".to_owned())
}

fn group_member(member: &JsonValue) -> Result<(ToolName, JsonValue), String> {
    let call = member
        .get("call")
        .ok_or_else(|| "a group member names a call".to_owned())?;
    let name = tool_name(call)?;
    let args = member
        .get("args")
        .cloned()
        .unwrap_or_else(|| JsonValue::Object(Vec::new()));
    Ok((name, args))
}

fn tool_name(value: &JsonValue) -> Result<ToolName, String> {
    match value {
        JsonValue::Str(text) => ToolName::new(text).map_err(|error| error.to_string()),
        _ => Err("a call names its tool with a string".to_owned()),
    }
}
