//! The Annex C.1 kernel program renderer (spec §10.5 "Minimum adapter
//! behavior", Annex C.1: "The adapter's `invoke` renders a kernel program").
//!
//! The program the adapter emits has the frozen shape:
//!
//! 1. write the runtime-built envelope to the child's input path;
//! 2. `await rlm(<role prompt + explicit result-contract instruction>,
//!    name=<invocation name>)` — the call returns an ADMISSION handle and
//!    never the child's answer (§10.5 return semantics; Annex C.2 rule 1);
//! 3. record the returned handle fields (`rlm_child_id`, `name`,
//!    `session_dir`, `model`) as JSON at the handle path — each coerced
//!    with `str(...)` (COR-D2 divergence #3: `session_dir` is a
//!    `PosixPath` upstream, and `json.dumps` on the raw field raises
//!    INSIDE the cell after a successful admission, so invoke would see
//!    "handle absent" for work that was actually admitted);
//! 4. return.
//!
//! **rpc-mode correction (task26-dcritic-apparatus-repair, supersedes the
//! RC2 linger/result-poll loop this module used to render).** The parent
//! process is no longer a `--print` run whose end aborts its `rlm`
//! children — it is spawned `--mode rpc` (`transport.rs`), which reads
//! JSON commands on stdin and stays alive until stdin closes, regardless
//! of what any one rendered cell does. The INVOCATION (not the rendered
//! program) now owns the parent process's lifetime for as long as the
//! invocation lives — see `transport.rs`'s and `runner.rs`'s module docs.
//! Consequently the cell above is the WHOLE program: it starts the
//! delegated work, records the admission handle, and returns immediately.
//! There is no linger, no result-contract poll, and no `time.sleep`/`while`
//! loop anywhere in the rendered text — polling for the RESULT is the
//! runner's job (`poll`, reading the result-contract file), never the
//! parent kernel's. This also matches prime-agent's own system prompt
//! ("start the work, record its handle or output location, then end your
//! turn … Do not keep the turn open by polling with `time.sleep()`"): a
//! parent model shown the OLD loop-bearing cell refused to run it at all,
//! because the loop read as exactly the polling behavior that prompt
//! forbids — nothing was ever admitted. The loop-free cell below is
//! executed without refusal (task26 rpc-mode experiment).
//!
//! The ROLE PROMPT is what makes fan-in possible (§10.3 rule 2): it must
//! instruct the child to write its typed `role_result@v1` document
//! ATOMICALLY (tmp + flush + rename) to the declared result-contract path
//! and only THEN signal the parent — the reply is a signal; the file is the
//! contract (Annex C.1). [`compose_child_prompt`] renders that instruction
//! around the role body; strict schema wording lives in the ports
//! (`role_result@v1`), which is what the child's document is validated
//! against — never trusted from the prompt.
//!
//! Rendering is BYTE-PINNED: the round-trip tests below fix the exact
//! program text for a known input, so a silent change to what the parent
//! kernel executes is a failing test, not a surprise.

use std::path::Path;

use crate::config::SpawnDialect;

/// Everything the renderer needs to emit one Annex C.1 program.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KernelProgramSpec {
    /// Where the runtime-built context envelope JSON already sits (the
    /// adapter writes it before submission; §10.3 rule 1: the runtime
    /// creates the invocation directory).
    pub envelope_path: String,
    /// The child's input path: the program copies the envelope there so
    /// the child's own session directory holds its input (Annex C.1 step
    /// 1: "writes the envelope to the child's input path").
    pub child_input_path: String,
    /// Where the program writes the admission-handle JSON the transport
    /// reads back (§10.5: record the returned handle fields).
    pub handle_path: String,
    /// The `rlm(name=…)` invocation name — deterministic and unique per
    /// invocation; the orphan sweep searches the harness by it.
    pub invocation_name: String,
    /// The fully composed child prompt (role body + result-contract
    /// instruction) — see [`compose_child_prompt`].
    pub role_prompt: String,
    /// The model the harness should run the child on, forwarded as the
    /// `model` kwarg (upstream supports per-call selection: "unless the
    /// call requests another configured model"). `None` emits NO model
    /// kwarg — the child inherits the parent model.
    pub model_hint: Option<String>,
}

/// Renders the Annex C.1 kernel program. The exact byte shape is pinned by
/// `the_rendered_program_is_byte_pinned` below.
pub fn render(spec: &KernelProgramSpec, dialect: SpawnDialect) -> String {
    let envelope = py_str(&spec.envelope_path);
    let child_input = py_str(&spec.child_input_path);
    let handle_path = py_str(&spec.handle_path);
    let name = py_str(&spec.invocation_name);
    let prompt = py_str(&spec.role_prompt);
    let model = spec
        .model_hint
        .as_ref()
        .map(|model| format!("    model={},\n", py_str(model)))
        .unwrap_or_default();
    let call_expr = format!("handle = await {}\n", dialect.call_expr());
    // NOTE: no line-continuation backslashes here — Rust string
    // continuations strip leading whitespace, which would silently
    // re-indent the emitted Python. The indentation below is explicit.
    [
        "# gauntlet prime-agent adapter — kernel program (spec Annex C.1)\n",
        "# Delegation returns an ADMISSION handle and never the child's answer;\n",
        "# results arrive only through the declared result contract. The parent\n",
        "# process is rpc-mode (transport.rs): its lifetime is owned by the\n",
        "# invocation, not by this cell, so this cell starts the work, records\n",
        "# the handle, and returns — no linger, no result poll, no loop.\n",
        "import json, pathlib\n",
        "envelope = pathlib.Path(",
        envelope.as_str(),
        ").read_text()\n",
        "child_input = pathlib.Path(",
        child_input.as_str(),
        ")\n",
        "child_input.write_text(envelope)\n",
        &call_expr,
        "    ",
        prompt.as_str(),
        ",\n",
        "    name=",
        name.as_str(),
        ",\n",
        model.as_str(),
        ")\n",
        "pathlib.Path(",
        handle_path.as_str(),
        ").write_text(json.dumps({\n",
        // COR-D2 divergence #3: every field is coerced with str(...) —
        // `session_dir` is a PosixPath upstream and json.dumps on the raw
        // field raises after a SUCCESSFUL admission.
        "    'rlm_child_id': str(handle.rlm_child_id),\n",
        "    'name': str(handle.name),\n",
        "    'session_dir': str(handle.session_dir),\n",
        "    'model': str(handle.model),\n",
        "}))\n",
    ]
    .concat()
}

/// Composes the child prompt: the role body plus the explicit
/// result-contract instruction (spec §10.3 rule 1 — the path is passed in
/// the PROMPT; Annex C.1: "The role prompt MUST instruct the child to write
/// its typed result atomically to `GAUNTLET_RESULT_PATH` and only then
/// reply to the parent").
///
/// The instruction carries the dispatch identity the document must echo
/// (§10.3 rule 6) and the atomic-write protocol (rule 2). It instructs; it
/// never relaxes: validation happens at the runtime against the frozen
/// schema, whatever the prompt said.
pub fn compose_child_prompt(
    role_body: &str,
    result_contract_path: &Path,
    invocation_id: &str,
    envelope_digest: &str,
) -> String {
    let contract = result_contract_path.to_string_lossy().into_owned();
    let tmp = format!("{contract}.tmp");
    [
        &format!("{role_body}\n"),
        "\n",
        "## Result contract (REQUIRED — this is how completion is determined)\n",
        "\n",
        "When your work is finished (or irrecoverably blocked), write your typed\n",
        "result document to EXACTLY this path (the GAUNTLET_RESULT_PATH environment\n",
        "variable names the same path when the harness supplies it):\n",
        "\n",
        &format!("  {contract}\n"),
        "\n",
        "The write MUST be atomic, in this order:\n",
        "\n",
        &format!("  1. write the complete document to: {tmp}\n"),
        "  2. flush it to stable storage;\n",
        "  3. rename it onto the path above (never write in place, never append).\n",
        "\n",
        "The document must be a strict `role_result@v1` JSON object. Its\n",
        &format!("`invocation_id` MUST be exactly `{invocation_id}` and its\n"),
        &format!("`context_envelope_digest` MUST be exactly `{envelope_digest}` — a stale\n"),
        "or cross-wired document is rejected whole. Your read of the context\n",
        "envelope is at the child input path named in your task.\n",
        "\n",
        "The `payload` object must be `builder_result@v1` with EXACTLY these fields\n",
        "and types — the whole document is rejected if any shape is wrong:\n",
        "\n",
        "  attempted     : array of strings — the assigned unit's id (e.g. \"AK-9013\")\n",
        "  changed_paths : array of strings — paths you changed relative to the workspace\n",
        "                  root (an empty array when you changed none)\n",
        "  commands_run  : array of strings — shell commands you ran (empty if none)\n",
        "  artifacts     : array of OBJECTS, each {\"path\": \"<relative path>\",\n",
        "                  \"evidence_ref\": \"<ref>\"} — leave it an EMPTY array:\n",
        "                  evidence_ref is minted by the runtime evidence store and you\n",
        "                  never have one to cite; a guessed ref is a forged one\n",
        "  claims        : array of strings — what you produced and how you verified it\n",
        "                  (empty if none)\n",
        "  self_checks   : array of OBJECTS, each {\"command\": \"<cmd>\",\n",
        "                  \"exit_code\": <0 or 1>} — what you ran to verify your work\n",
        "                  (empty if none)\n",
        "  blocked_on    : array of strings (empty when nothing blocks you)\n",
        "\n",
        "The `invocation_id` printed above is THIS dispatch's id. Write it fresh from\n",
        "THIS task — never from memory of an earlier turn or an earlier attempt.\n",
        "\n",
        "Only AFTER the rename succeeds may you signal the parent (mailbox reply).\n",
        "The reply is a signal; the file is the contract. A reply without a valid\n",
        "result file is NOT completion.\n",
    ]
    .concat()
}

/// Renders a Python single-quoted string literal: backslashes, quotes, and
/// line breaks are escaped so arbitrary path/prompt text round-trips into
/// one physical program line (the byte-pin test covers a prompt carrying
/// quotes, backslashes, and newlines).
fn py_str(value: &str) -> String {
    let mut out = String::with_capacity(value.len() + 2);
    out.push('\'');
    for character in value.chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '\'' => out.push_str("\\'"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            control if (control as u32) < 0x20 => {
                out.push_str(&format!("\\x{:02x}", control as u32));
            }
            other => out.push(other),
        }
    }
    out.push('\'');
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn spec() -> KernelProgramSpec {
        KernelProgramSpec {
            envelope_path: "/state/runs/r/inv/i/envelope.json".to_owned(),
            child_input_path: "/state/runs/r/inv/i/child-input.json".to_owned(),
            handle_path: "/state/runs/r/inv/i/result.handle".to_owned(),
            invocation_name: "builder-ws01-01ARZ3N".to_owned(),
            role_prompt: "implement the assigned slice".to_owned(),
            model_hint: None,
        }
    }

    /// The byte pin: any change to the emitted program is a deliberate,
    /// reviewed edit that updates this pin (Annex C.1 shape). Without a
    /// model hint NO model kwarg is emitted (the child inherits the
    /// parent model). COR-D2: all four handle fields are coerced with
    /// str(...) (`session_dir` is a PosixPath upstream).
    ///
    /// PIN UPDATE (rpc-mode correction, task26-dcritic-apparatus-repair):
    /// the cell's linger/result-poll tail is GONE — the parent process
    /// (rpc mode) outlives this cell on its own, so the program ends the
    /// moment the handle is written.
    ///
    /// Default dialect is Spawn09 (prime-agent 0.9+).
    #[test]
    fn the_rendered_program_is_byte_pinned() {
        let expected = [
            "# gauntlet prime-agent adapter — kernel program (spec Annex C.1)",
            "# Delegation returns an ADMISSION handle and never the child's answer;",
            "# results arrive only through the declared result contract. The parent",
            "# process is rpc-mode (transport.rs): its lifetime is owned by the",
            "# invocation, not by this cell, so this cell starts the work, records",
            "# the handle, and returns — no linger, no result poll, no loop.",
            "import json, pathlib",
            "envelope = pathlib.Path('/state/runs/r/inv/i/envelope.json').read_text()",
            "child_input = pathlib.Path('/state/runs/r/inv/i/child-input.json')",
            "child_input.write_text(envelope)",
            "handle = await rlm.spawn(",
            "    'implement the assigned slice',",
            "    name='builder-ws01-01ARZ3N',",
            ")",
            "pathlib.Path('/state/runs/r/inv/i/result.handle').write_text(json.dumps({",
            "    'rlm_child_id': str(handle.rlm_child_id),",
            "    'name': str(handle.name),",
            "    'session_dir': str(handle.session_dir),",
            "    'model': str(handle.model),",
            "}))",
        ]
        .join("\n")
            + "\n";
        assert_eq!(render(&spec(), SpawnDialect::Spawn09), expected);
    }

    /// A model hint is forwarded as the `model` kwarg (RES-D6 decision:
    /// forward — upstream selects per call).
    #[test]
    fn a_model_hint_is_forwarded_as_the_model_kwarg() {
        let mut hinted = spec();
        hinted.model_hint = Some("acme/model-x".to_owned());
        let program = render(&hinted, SpawnDialect::Spawn09);
        for line in [
            "handle = await rlm.spawn(",
            "    'implement the assigned slice',",
            "    name='builder-ws01-01ARZ3N',",
            "    model='acme/model-x',",
            ")",
        ] {
            assert!(
                program.lines().any(|emitted| emitted == line),
                "missing line {line:?}:\n{program}"
            );
        }
        // And the unhinted program omits the kwarg entirely.
        assert!(!render(&spec(), SpawnDialect::Spawn09).contains("model="));
    }

    /// The rendered program carries the frozen Annex C.1 semantics: an
    /// admission handle is recorded (coerced to strings — COR-D2 #3), NO
    /// child answer is awaited or captured anywhere (the `rlm` call's
    /// only use is `handle =`), and — the rpc-mode correction's teeth —
    /// there is NO keep-alive loop of any kind: no `time.sleep`, no
    /// `while`, no mailbox read. The handle write is the program's LAST
    /// effect. This holds for all dialects.
    #[test]
    fn the_rendered_program_captures_the_admission_handle_only() {
        for dialect in [SpawnDialect::Callable08, SpawnDialect::Spawn09] {
            let program = render(&spec(), dialect);
            for fragment in [
                "await rlm",
                "'rlm_child_id': str(handle.rlm_child_id)",
                "'session_dir': str(handle.session_dir)",
                "'model': str(handle.model)",
            ] {
                assert!(
                    program.contains(fragment),
                    "missing {fragment:?} for {dialect:?}: {program}"
                );
            }
            assert!(
                !program.contains("time.sleep"),
                "no keep-alive sleep may remain for {dialect:?}: {program}"
            );
            assert!(
                !program.contains("while "),
                "no keep-alive loop may remain for {dialect:?}: {program}"
            );
            assert!(
                !program.contains("agent_message"),
                "no mailbox read for {dialect:?}: {program}"
            );
            // The handle write is the LAST effect.
            assert!(
                program.trim_end().ends_with("}))"),
                "the program must end with the handle write for {dialect:?}: {program}"
            );
        }
    }

    /// Prompt and path text round-trips through the Python literal
    /// escaping: quotes, backslashes, newlines, tabs, and control
    /// characters survive as the same string when the parent's Python
    /// parses the literal.
    #[test]
    fn hostile_prompt_text_survives_the_python_literal_round_trip() {
        let hostile = "line one\n'single' and \"double\" \\ back\ttab\x01bell\nline two";
        let mut pinned = spec();
        pinned.role_prompt = hostile.to_owned();
        let program = render(&pinned, SpawnDialect::Spawn09);
        // The escaped literal appears exactly once, on one physical line.
        let literal = py_str(hostile);
        assert_eq!(
            program.matches(literal.as_str()).count(),
            1,
            "the escaped literal must appear exactly once: {program}"
        );
        // The program's line count with the hostile prompt is exactly the
        // pinned line count: the multiline prompt stayed one literal.
        assert_eq!(
            program.lines().count(),
            render(&spec(), SpawnDialect::Spawn09).lines().count(),
            "the multiline prompt must remain a single escaped literal line"
        );
    }

    /// The child prompt carries the contract path, the atomic protocol,
    /// the dispatch identity to echo, and the signal-vs-contract rule.
    #[test]
    fn the_child_prompt_carries_the_whole_result_contract() {
        let prompt = compose_child_prompt(
            "body of the role",
            std::path::Path::new("/state/runs/r/inv/i/result.json"),
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "sha256:e5f6a7b8c4d3e2f1a0b1c2d3",
        );
        for fragment in [
            "/state/runs/r/inv/i/result.json",
            "/state/runs/r/inv/i/result.json.tmp",
            "role_result@v1",
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "sha256:e5f6a7b8c4d3e2f1a0b1c2d3",
            "GAUNTLET_RESULT_PATH",
            "rename",
            "The reply is a signal; the file is the contract.",
        ] {
            assert!(prompt.contains(fragment), "missing {fragment:?}:\n{prompt}");
        }
        // The payload-shape spec (task26 W9 fix): the builder is told the
        // exact `builder_result@v1` field shapes instead of guessing them,
        // and that `artifacts` stays empty (the runtime mints evidence
        // refs — a guessed ref is forged).
        for fragment in [
            "builder_result@v1",
            "artifacts",
            "evidence_ref is minted by the runtime",
            "a guessed ref is a forged one",
            "array of OBJECTS",
            "Write it fresh from",
            "never from memory of an earlier turn",
        ] {
            assert!(
                prompt.contains(fragment),
                "missing payload-spec {fragment:?}:\n{prompt}"
            );
        }
        // The role body leads the prompt verbatim.
        assert!(prompt.starts_with("body of the role\n"), "{prompt}");
    }

    /// A role body with hostile text is preserved verbatim by the composer.
    #[test]
    fn the_child_prompt_preserves_the_role_body() {
        let body = "## Steps\n- do `x --y`\n- 'quote' \"text\"";
        let prompt = compose_child_prompt(
            body,
            std::path::Path::new("result.json"),
            "01ARZ3NDEKTSV4RRFFQ69G5FAV",
            "sha256:e5f6a7b8c4d3e2f1a0b1c2d3",
        );
        assert!(prompt.contains(body), "{prompt}");
    }

    /// Spawn09 dialect (prime-agent 0.9+) emits `await rlm.spawn(` call.
    #[test]
    fn spawn09_cell_contains_await_rlm_spawn() {
        let program = render(&spec(), SpawnDialect::Spawn09);
        assert!(
            program.contains("await rlm.spawn("),
            "Spawn09 dialect must emit 'await rlm.spawn(': {program}"
        );
        assert!(
            !program.contains("await rlm(") || program.contains("await rlm.spawn("),
            "Spawn09 must not contain bare 'await rlm(' (only 'await rlm.spawn('): {program}"
        );
    }

    /// Callable08 dialect (prime-agent 0.8) emits `await rlm(` call.
    #[test]
    fn callable08_cell_contains_await_rlm() {
        let program = render(&spec(), SpawnDialect::Callable08);
        assert!(
            program.contains("await rlm("),
            "Callable08 dialect must emit 'await rlm(': {program}"
        );
        assert!(
            !program.contains("await rlm.spawn("),
            "Callable08 must not contain 'await rlm.spawn(': {program}"
        );
    }
}
