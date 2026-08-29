# Task 15 — orchestrator adjudication of the design review (RES-D1..D12)

Source: `.agents/reports/task15/critic-design.md` (blind design review,
verdict NOT_CONFIRM: 1 blocking / 5 major / 6 minor). Adjudicated by the
orchestrator 2026-08-29 before B1 completion; corrections routed to the
builder mid-flight (see `correction-design.md` routing note in IN-FLIGHT).

| Residual | Sev | Disposition | Decision |
|---|---|---|---|
| RES-D1 invoke returns on parent-run end, not handle existence | **blocking** | **ACCEPT in full** | `invoke` returns the moment the handle file is observed+parsed (bounded watch on HANDLE_PATH; the timeout guards the WATCH, not the parent process lifetime). After parse, the parent run's disposition is: best-effort graceful stop (harness `stop` when the parent is daemon-registered, else terminate the spawned process), bounded, outcome recorded as forensics — never blocking return beyond the stop bound. The live probe MUST evidence the D1 split: handle parsed while `parent.try_wait()` still reports NotExited, then a poll reaching Completed only after the parent is gone — which also proves daemon-backed child survival (registered blindspot). |
| RES-D2 late writer resurrects TimedOut | major | **ACCEPT** | The adapter persists the first terminal verdict per admitted invocation_id (adapter-local control state — permitted; I11 forbids only session_dir as control state) and every later poll re-reports it verbatim. Post-terminal stability = port contract. |
| RES-D3 filesystem anchors unspecified | major | **ACCEPT** | ONE resolution rule, pinned before implementation: invoke creates `<state_dir>/runs/<run_id>/inv/<invocation_id>/`; the opaque `ResultContractPath` token resolves as the literal leaf inside that directory; envelope + handle files are derived siblings. `WorkspaceRef` → parent cwd is an INJECTED typed input from the composition root (production resolution unreachable this task; R-1b's territory); suite doubles and the live probe inject temp roots. |
| RES-D4 orphan child on crash between spawn and handle write | major | **ACCEPT** | (a) Invoke failure taxonomy: handle absent/invalid/undecodable ⇒ `AgentError::AdmissionRefused`; transport/binary failure ⇒ `AgentError::Transport`. (b) Best-effort orphan sweep on admission failure: consult the control seam's list by the deterministic invocation name, `stop` any discovered child, record the sweep outcome inside the error detail. |
| RES-D5 transport outcome type unpinned; await-grep self-flag hazard | major | **ACCEPT** | The kernel-transport trait's success payload is a struct carrying EXACTLY `rlm_child_id`/`name`/`session_dir`/`model`, strict-decoded from the handle file (unknown fields rejected, typed errors) — no raw strings, no program output on the success path. The arch await-ban grep's subject is Rust-side consumption of delegation outcomes (answer-shaped parsing of transport results or parent stdout); it must carry a documented EXEMPTION for the pinned kernel-template constant (whose `await rlm(...)` is the upstream admission-await, Annex C.1) and the mandatory negative control. |
| RES-D6 capabilities booleans + model_hint disposition | major | **ACCEPT, decision: forward** | All six values declared: native_subagents true; streaming_events false (print-mode surface; prereg D5); model_selection **true** because the rendered program forwards `model_hint` into the `rlm(...)` call (upstream supports per-call model selection: "unless the call requests another configured model"); isolation None; delivery File; max_depth from config. `model_hint: None` ⇒ no model kwarg emitted. |
| RES-D7 doctor descriptor pattern completeness | minor | **ACCEPT** | Runner descriptor mirrors the BackendDescriptor precedent verbatim (token-match against the resolved runner name, measured value, note). A FAILED measured probe ⇒ Fail (non-zero exit class) on check 3's resolution half AND check 4's runner half — never Unavailable-inside-Pass. Evidence text names the measured dimension: harness API-surface version compatibility only (prereg D4). |
| RES-D8 probe constant home + marker probe | minor | **ACCEPT, decision: drop marker probe** | Supported-range constant lives in typed probe configuration injected by the composition root from the app defaults module (not a bare literal in the adapter). The package-internal marker probe is DROPPED — the versioned range is the durable contract; coupling to install layout is non-durable. |
| RES-D9 plan gate-line substitution must be recorded | minor | **ACCEPT** | The closure record states the substitution (factory-pattern integration per the mock precedent) and shows `git diff` empty on `crates/gauntlet-ports/src/contract/agent_runner.rs`. |
| RES-D10 recon §7 path typo | minor | **ACCEPT** | Recon corrected: `crates/adapters/gauntlet-adapter-agent-mock/tests/contract_suite.rs`. |
| RES-D11 Annex C.2 role-install rows are Task 17's | minor | **ACCEPT** | Task 15 implements the never-await / poll-contract / session_ref-only / isolation-none / list_subagents-as-forensics rows; role install/verify (C.2 row 5, C.3) is Task-17-owned; interim semantics = advisory inline-prompt (plan:571). Recorded here and in the closure. |
| RES-D12 Running payload sourcing | minor | **ACCEPT** | `last_event_at` = last observed state change of the result-contract path (mtime), else admission time. `payment_so_far` = zero with an explicit adapter-documented statement that child spend is NOT observable through this surface — it is not a measurement and must not be consumed as one; budget truth stays with the runtime's BudgetMeter. |

No residual REJECTED. The verdict stands: the design was NOT_CONFIRM and is
corrected by this adjudication + the builder routing before B1 completes.

---

## Gate-phase addendum — prereg P3 fired (live probe, orchestrator run)

**2026-08-29, orchestrator-personal live probe, attempt 1: FAILED HONESTLY.**
`VersionUnparseable { binary: ".../prime-agent", output: "" }` at the probe's
first step. Verified locus: this harness prints `--version` to **stderr**
(stdout empty, stderr `0.8.0`, exit 0 — reproduced with separated streams;
the recon's own `2>&1` probes masked it). `list --json` verified printing to
stdout (valid JSON, 1821 bytes) — divergence isolated to the version flag.

This is prereg prediction P3 exactly: a harness-surface divergence forcing a
probe correction, recorded, never faked. The fail-closed direction held (the
probe refused to guess). Correction COR-D1 routed to the builder: capture
both streams, parse the combined output, keep typed errors strict, add a
hermetic stderr-only fixture test. Daemon-machinery blindspot partially
resolved (list works); child survival after parent exit still owed to the
live probe's RES-D1 evidence leg on the re-run.

---

## Gate-phase addendum 2 — COR-D2: live admission diagnosed by execution

Orchestrator-executed probes (6 bounded parent runs + 1 real child, all in
`~/.cache/gauntlet-probe/`, evidence cited inline). Findings, in the order
they were isolated:

1. **COR-D1 (landed):** `--version` prints to stderr. Probe now parses the
   combined streams; verified working (the re-run reached live admission).
2. **Divergence #2 — `list --json` shape:** the harness returns
   `{"sessions": [...]}` (top-level OBJECT), not a bare JSON array. The
   control-seam parser expects an array — the orphan sweep's
   "not a JSON array" error in the live-probe re-run is this. Fix: strict
   acceptance of exactly `{sessions: [...]}`, typed rejection otherwise.
3. **Divergence #3 — handle field types:** `RLMSpawnHandle.session_dir` is
   a `PosixPath` (not a str). `json.dumps` on the raw fields raises
   `TypeError: Object of type PosixPath is not JSON serializable` INSIDE
   the rendered cell — admission SUCCEEDS, then the handle write dies, so
   invoke sees "handle absent". The upstream doc example
   (`print(handle.rlm_child_id, ...)`) conceals field types. Fix: the
   rendered program coerces all four fields with `str(...)`.
4. **Divergence #4 — child creation needs a living parent (the load-bearing
   one):** with the cell ending right after the handle write, the admitted
   child NEVER materialized (no work, no trace, parent ran to natural
   completion). With a 60s in-cell linger after the handle write, the child
   completed its work 15s in (`DONE` written; inherited parent model
   `openai-codex/gpt-5.6-terra`). Child creation is asynchronous and needs
   the parent session alive past admission. Fix: the rendered cell appends
   a bounded configurable LINGER (default 30s, evidence-cited) after the
   handle write; invoke STILL returns at handle parse (D1 unchanged — the
   linger is the parent's, never invoke's); the transport's post-parse
   disposition becomes a detached bounded reaper (wait linger + grace, then
   stop if still alive) instead of an immediate SIGTERM.

Empirical admission record (t6 probe): handle
`{"rlm_child_id": "sub-76db0de9", "name": "probe-child-6",
"session_dir": ".../session-artifacts/01a04f8a.../sub-76db0de9",
"model": "openai-codex/gpt-5.6-terra"}`; child file `DONE` at +15s; parent
exit 0. The RES-D1 evidence leg (handle parsed while parent alive) is now
demonstrable on the real harness by construction.

Annex C.4's "idle children may unload and reload" caveat and the
daemon-survival blindspot are thereby partially RESOLVED with evidence:
children of a print-mode parent work iff the parent lingers; the re-run of
the live probe must still evidence the full poll→Completed leg.
