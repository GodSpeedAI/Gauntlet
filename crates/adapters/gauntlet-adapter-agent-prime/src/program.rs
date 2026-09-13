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
//! 4. linger — RESULT-AWARE and bounded (RC2 correction,
//!    task26-dcritic-apparatus-repair, supersedes the fixed-sleep shape
//!    this step used to name): a child of a print-mode parent
//!    materializes ONLY while the parent session stays alive past
//!    admission (COR-D2 divergence #4, load-bearing, verified by
//!    execution: no linger, child never appears; a linger long enough,
//!    the child completed its work) — but a FIXED sleep cannot fit work
//!    of unknown length, so the cell instead polls for the declared
//!    result-contract FILE, bounded BELOW by the linger floor (the
//!    materialization guarantee) and ABOVE by the invocation's own
//!    deadline (so the parent never outlives what the runtime would
//!    already time out). Polling the result FILE from the PARENT's own
//!    kernel is NOT "awaiting the child's answer from `rlm(...)`" (Annex
//!    C.2): the parent never reads the child's reply or its mailbox, and
//!    the loop's exit is time-and-file bounded, never indefinite (Annex
//!    C.4);
//! 5. return.
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

/// The rendered cell's poll cadence while it waits for the result-contract
/// file to appear (RC2 correction, task26-dcritic-apparatus-repair): a
/// short, fixed step — the loop's TOTAL bound comes from
/// [`KernelProgramSpec::linger_seconds`] and
/// [`KernelProgramSpec::deadline_epoch`], never from this constant.
const LINGER_POLL_SECONDS: u64 = 2;

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
    /// The MINIMUM in-cell linger, in seconds, AFTER the handle write
    /// (COR-D2 divergence #4): child creation is asynchronous and
    /// materializes only while the parent session stays alive past
    /// admission — this is the FLOOR of the RC2 bounded poll loop, not a
    /// fixed sleep; the loop may run longer, up to `deadline_epoch`. The
    /// linger is the PARENT's cell — never a wait in `invoke` (D1
    /// untouched).
    pub linger_seconds: u64,
    /// The path the CHILD's result-contract document is expected to land
    /// at, from the parent's own filesystem view (RC2 correction): the
    /// parent's cell polls for this file's existence rather than sleeping
    /// a fixed span.
    pub result_contract_path: String,
    /// The invocation's absolute deadline, as epoch seconds (RC2
    /// correction): the poll loop's UPPER bound — the parent never
    /// outlives what the runtime's own deadline already permits. Waiting
    /// on the result FILE (not the child's reply) up to this bound is not
    /// awaiting a child answer from `rlm(...)` (Annex C.2).
    pub deadline_epoch: u64,
}

/// Renders the Annex C.1 kernel program. The exact byte shape is pinned by
/// `the_rendered_program_is_byte_pinned` below.
pub fn render(spec: &KernelProgramSpec) -> String {
    let envelope = py_str(&spec.envelope_path);
    let child_input = py_str(&spec.child_input_path);
    let handle_path = py_str(&spec.handle_path);
    let name = py_str(&spec.invocation_name);
    let prompt = py_str(&spec.role_prompt);
    let result_path = py_str(&spec.result_contract_path);
    let model = spec
        .model_hint
        .as_ref()
        .map(|model| format!("    model={},\n", py_str(model)))
        .unwrap_or_default();
    let linger_floor = spec.linger_seconds.to_string();
    let deadline_epoch = spec.deadline_epoch.to_string();
    let poll_seconds = LINGER_POLL_SECONDS.to_string();
    // NOTE: no line-continuation backslashes here — Rust string
    // continuations strip leading whitespace, which would silently
    // re-indent the emitted Python. The indentation below is explicit.
    [
        "# gauntlet prime-agent adapter — kernel program (spec Annex C.1)\n",
        "# Delegation returns an ADMISSION handle and never the child's answer;\n",
        "# results arrive only through the declared result contract.\n",
        "import json, pathlib\n",
        "envelope = pathlib.Path(",
        envelope.as_str(),
        ").read_text()\n",
        "child_input = pathlib.Path(",
        child_input.as_str(),
        ")\n",
        "child_input.write_text(envelope)\n",
        "handle = await rlm(\n",
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
        // RC2 correction (task26-dcritic-apparatus-repair, recorded
        // beside COR-D2 #4 in the same style): the parent's cell used to
        // `time.sleep(<fixedN>)` here — a fixed sleep cannot fit work of
        // unknown length, so a child doing real work past the sleep was
        // aborted mid-work when the (then-disposable) parent was torn
        // down. The cell now POLLS for the declared result-contract FILE
        // instead, bounded BELOW by the linger floor (COR-D2 #4's
        // materialization guarantee still holds: the child needs SOME
        // minimum time past admission to appear) and bounded ABOVE by the
        // invocation's own deadline (the parent never outlives what the
        // runtime would already time out). Waiting on the result FILE
        // from the PARENT's own kernel is NOT "awaiting the child's
        // answer from `rlm(...)`" (Annex C.2): the parent never reads the
        // child's reply or its mailbox, and the loop's exit is
        // time-and-file bounded, never indefinite (Annex C.4).
        "import time\n",
        "_gauntlet_linger_floor_ends = time.time() + ",
        linger_floor.as_str(),
        "\n",
        "_gauntlet_deadline_ends = ",
        deadline_epoch.as_str(),
        "\n",
        "_gauntlet_bound_ends = max(_gauntlet_linger_floor_ends, _gauntlet_deadline_ends)\n",
        "_gauntlet_result_path = pathlib.Path(",
        result_path.as_str(),
        ")\n",
        "while time.time() < _gauntlet_bound_ends and not _gauntlet_result_path.exists():\n",
        "    time.sleep(",
        poll_seconds.as_str(),
        ")\n",
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
            linger_seconds: 30,
            result_contract_path: "/state/runs/r/inv/i/result.json".to_owned(),
            deadline_epoch: 1_700_000_600,
        }
    }

    /// The byte pin: any change to the emitted program is a deliberate,
    /// reviewed edit that updates this pin (Annex C.1 shape). Without a
    /// model hint NO model kwarg is emitted (the child inherits the
    /// parent model). COR-D2: all four handle fields are coerced with
    /// str(...) (`session_dir` is a PosixPath upstream).
    ///
    /// PIN UPDATE (RC2 correction, task26-dcritic-apparatus-repair): the
    /// cell's tail changed from a fixed `time.sleep(30)` to a
    /// result-aware bounded poll loop — see the correction note beside
    /// [`render`]'s tail-emitting lines. This is a deliberate, reviewed
    /// change to the pinned bytes, not a drift.
    #[test]
    fn the_rendered_program_is_byte_pinned() {
        let expected = [
            "# gauntlet prime-agent adapter — kernel program (spec Annex C.1)",
            "# Delegation returns an ADMISSION handle and never the child's answer;",
            "# results arrive only through the declared result contract.",
            "import json, pathlib",
            "envelope = pathlib.Path('/state/runs/r/inv/i/envelope.json').read_text()",
            "child_input = pathlib.Path('/state/runs/r/inv/i/child-input.json')",
            "child_input.write_text(envelope)",
            "handle = await rlm(",
            "    'implement the assigned slice',",
            "    name='builder-ws01-01ARZ3N',",
            ")",
            "pathlib.Path('/state/runs/r/inv/i/result.handle').write_text(json.dumps({",
            "    'rlm_child_id': str(handle.rlm_child_id),",
            "    'name': str(handle.name),",
            "    'session_dir': str(handle.session_dir),",
            "    'model': str(handle.model),",
            "}))",
            "import time",
            "_gauntlet_linger_floor_ends = time.time() + 30",
            "_gauntlet_deadline_ends = 1700000600",
            "_gauntlet_bound_ends = max(_gauntlet_linger_floor_ends, _gauntlet_deadline_ends)",
            "_gauntlet_result_path = pathlib.Path('/state/runs/r/inv/i/result.json')",
            "while time.time() < _gauntlet_bound_ends and not _gauntlet_result_path.exists():",
            "    time.sleep(2)",
        ]
        .join("\n")
            + "\n";
        assert_eq!(render(&spec()), expected);
    }

    /// A model hint is forwarded as the `model` kwarg (RES-D6 decision:
    /// forward — upstream selects per call).
    #[test]
    fn a_model_hint_is_forwarded_as_the_model_kwarg() {
        let mut hinted = spec();
        hinted.model_hint = Some("acme/model-x".to_owned());
        let program = render(&hinted);
        for line in [
            "handle = await rlm(",
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
        assert!(!render(&spec()).contains("model="));
    }

    /// The rendered program carries the frozen Annex C.1 semantics: an
    /// admission handle is recorded (coerced to strings — COR-D2 #3),
    /// and NO child answer is awaited or captured anywhere (the `rlm`
    /// call's only use is `handle =`). The bounded result-poll (RC2) is
    /// the cell's LAST effect; nothing reads a child reply, and the loop
    /// is bounded both by the linger floor and by the invocation's own
    /// deadline.
    #[test]
    fn the_rendered_program_captures_the_admission_handle_only() {
        let program = render(&spec());
        for fragment in [
            "await rlm(",
            "'rlm_child_id': str(handle.rlm_child_id)",
            "'session_dir': str(handle.session_dir)",
            "'model': str(handle.model)",
            "_gauntlet_bound_ends = max(_gauntlet_linger_floor_ends, _gauntlet_deadline_ends)",
            "_gauntlet_result_path.exists()",
        ] {
            assert!(
                program.contains(fragment),
                "missing {fragment:?}: {program}"
            );
        }
        // The bounded poll is the LAST effect — the parent stays alive
        // until the result lands or its bound passes, then the cell ends;
        // nothing reads a child reply.
        assert!(
            program
                .trim_end()
                .ends_with(&format!("time.sleep({LINGER_POLL_SECONDS})")),
            "the program must end with the bounded result-poll: {program}"
        );
        assert!(
            !program.contains("agent_message"),
            "no mailbox read: {program}"
        );
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
        let program = render(&pinned);
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
            render(&spec()).lines().count(),
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
}
