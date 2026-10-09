//! The shared `ToolProgramRuntime` contract suite (`gauntlet-ports`
//! `contract::tool_program`) run against the REAL codemode runtime, which
//! spawns the repository's Bun host for each run.
//!
//! The shared suite writes its programs in the reference's declarative step
//! format (`[{"call": ...}, {"return": ...}]`), which no JavaScript engine can
//! parse. This file's translator maps those steps to the equivalent async
//! JavaScript body. The translation is test-only, and every case's expected
//! status, trace, and digests are the suite's own.
//!
//! Two cases cannot hold for a JavaScript engine, because of what the host
//! protocol can report. They are NOT run as written and are replaced by
//! JavaScript-native checks in `tests/javascript_programs.rs`:
//!
//! - `only_admitted_tools_execute`: a name that is not admitted never reaches
//!   the parent, so the host record has no refusal entry for it. Its
//!   "refused name is in the trace" rule cannot hold for the codemode host.
//! - `parallel_limit_refuses_the_group`: the host refuses the over-bound call
//!   inside the sandbox, after the in-bound calls of the same group were sent.
//!   The "executes nothing" rule cannot hold for the codemode host.
//!
//! The filter below asserts that these two cases produce EXACTLY those known
//! findings and nothing else, so the adaptation cannot widen silently.

mod support;

use gauntlet_adapter_tool_program_codemode::{CodemodeRuntime, HostCommand};
use gauntlet_ports::contract::ContractFinding;
use gauntlet_ports::contract::tool_program::{ToolProgramFactory, tool_program_suite};
use gauntlet_ports::json::JsonValue;
use gauntlet_ports::tool_program::{
    ProgramHost, ProgramOutcome, ProgramSource, ToolProgramRequest, ToolProgramRuntime,
    finish_outcome,
};

const ADAPTED: &[(&str, &str)] = &[
    (
        "only_admitted_tools_execute",
        "the refused name is absent from the trace as a refusal",
    ),
    (
        "parallel_limit_refuses_the_group",
        "tools executed from a refused group",
    ),
];

/// Maps the reference's declarative steps to an async JavaScript body. Returns
/// `None` when the text is not a step array, so the text runs as written.
fn steps_to_javascript(text: &str) -> Option<String> {
    let JsonValue::Array(steps) = gauntlet_ports::json::parse(text).ok()? else {
        return None;
    };
    let mut body = String::new();
    for step in &steps {
        let line = if let Some(call) = step.get("call") {
            format!("await {};", call_expression(call, step.get("args"))?)
        } else if let Some(JsonValue::Array(members)) = step.get("group") {
            let calls = members
                .iter()
                .map(|member| {
                    member
                        .get("call")
                        .and_then(|call| call_expression(call, member.get("args")))
                })
                .collect::<Option<Vec<_>>>()?;
            format!("await Promise.all([{}]);", calls.join(", "))
        } else if step.get("spin").is_some() {
            "while (true) {}".to_owned()
        } else if let Some(JsonValue::Str(message)) = step.get("fail") {
            format!(
                "throw new Error({});",
                JsonValue::Str(message.clone()).to_json_text()
            )
        } else {
            let value = step.get("return")?;
            format!("return {};", value.to_json_text())
        };
        body.push_str(&line);
        body.push('\n');
    }
    Some(body)
}

/// A dotted tool name is reached through the `tools` namespace tree.
fn call_expression(call: &JsonValue, args: Option<&JsonValue>) -> Option<String> {
    let JsonValue::Str(name) = call else {
        return None;
    };
    let args = args
        .cloned()
        .unwrap_or_else(|| JsonValue::Object(Vec::new()))
        .to_json_text();
    Some(format!("tools.{name}({args})"))
}

/// The translating runtime: the real codemode runtime over the translated
/// program. The outcome is re-assembled against the ORIGINAL request, so the
/// program digest is the suite's, and the trace still comes from the host.
struct TranslatingRuntime {
    inner: CodemodeRuntime,
}

impl ToolProgramRuntime for TranslatingRuntime {
    fn run(&self, request: &ToolProgramRequest, host: &dyn ProgramHost) -> ProgramOutcome {
        let text = steps_to_javascript(request.source().text())
            .unwrap_or_else(|| request.source().text().to_owned());
        let source = ProgramSource::new(&text, request.limits().max_source_bytes)
            .expect("the translated program fits the source cap");
        let translated = ToolProgramRequest::new(
            request.caller().clone(),
            source,
            request.admitted().to_vec(),
            request.limits(),
        )
        .expect("the translated request is valid");
        let outcome = self.inner.run(&translated, host);
        finish_outcome(outcome.status, host.record(), request, outcome.duration)
    }
}

struct CodemodeFactory {
    host: HostCommand,
}

impl ToolProgramFactory for CodemodeFactory {
    fn label(&self) -> &'static str {
        "codemode"
    }

    fn build(&self) -> Result<Box<dyn ToolProgramRuntime>, String> {
        Ok(Box::new(TranslatingRuntime {
            inner: CodemodeRuntime::new(self.host.clone()),
        }))
    }
}

#[test]
fn the_codemode_runtime_passes_the_shared_tool_program_suite() {
    let factory = CodemodeFactory {
        host: support::bun_host(),
    };
    let findings: Vec<ContractFinding> = tool_program_suite(&factory);

    let (adapted, unadapted): (Vec<_>, Vec<_>) = findings
        .into_iter()
        .partition(|finding| ADAPTED.iter().any(|(case, _)| *case == finding.case));
    assert!(
        unadapted.is_empty(),
        "the codemode runtime must pass the shared suite:\n{}",
        render(&unadapted)
    );
    for finding in &adapted {
        let expected = ADAPTED
            .iter()
            .find(|(case, _)| *case == finding.case)
            .map(|(_, detail)| *detail);
        assert!(
            expected.is_some_and(|detail| finding.detail.contains(detail)),
            "an adapted case may fail only with its known host-protocol finding; got:\n{finding}"
        );
    }
    assert_eq!(
        adapted.len(),
        ADAPTED.len(),
        "each adapted case must show its known finding, or the adaptation is no longer needed:\n{}",
        render(&adapted)
    );
}

fn render(findings: &[ContractFinding]) -> String {
    findings
        .iter()
        .map(|finding| finding.to_string())
        .collect::<Vec<_>>()
        .join("\n")
}

#[test]
fn the_translator_maps_each_reference_step_to_javascript() {
    let text = r#"[{"call":"tools.read","args":{"n":1}},{"group":[{"call":"tools.read"}]},{"spin":true},{"return":{"done":true}}]"#;
    let js = steps_to_javascript(text).expect("a step array translates");
    assert_eq!(
        js,
        "await tools.tools.read({\"n\":1});\n\
         await Promise.all([tools.tools.read({})]);\n\
         while (true) {}\n\
         return {\"done\":true};\n"
    );
    assert!(steps_to_javascript("not a program").is_none());
}
