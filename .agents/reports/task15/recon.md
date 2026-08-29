# Task 15 recon — Prime-Agent adapter

**Task 15 — Prime-Agent adapter** *(phase 4)*. Written before preregistration
freeze, from repository evidence and live host probes only.

## 1. What the plan and spec demand

Plan Task 15 (`.agents/plans/gauntlet-impl.md:498-521`), five steps:

1. `crates/adapters/agent-prime`: render the Annex C.1 kernel program, capture
   `rlm_child_id`/`name`/`session_dir`/`model` into the handle, return
   immediately.
2. `poll` reads the result-contract path; a mailbox reply without a valid
   result file is `Running`, not `Completed`. Deadline respected; cancel on
   timeout preserves `session_dir` as `session_ref`.
3. Honest `capabilities()`: `isolation: none` unless externally wrapped,
   `result_delivery: file`, `supports_native_subagents: true`, `max_depth`
   from configuration.
4. Harness API surface behind a version probe; probe mismatch is a
   `configuration` failure surfaced by `doctor`.
5. The adapter passes the UNMODIFIED `AgentRunner` contract suite.

Done-when: suite passes unmodified; a child that exits without writing a
result yields `TimedOut`; no code path awaits a value from the delegation
call (grep-asserted in the arch tests).

Gate: `cargo nextest run -p gauntlet-adapter-agent-prime` and the ports
suite. (Note: the plan's second gate line spells the suite filter
`contract::agent_runner -- --adapter prime`; the suite as built in Task 4 is
a function parameterized over a factory — the mock's integration test
(`gauntlet-adapter-agent-mock/tests/contract_suite.rs`) is the established
pattern: supply a factory, assert empty findings. The `--adapter` selection
flag is Task 16's step 3, not this task's.)

Spec surfaces this task implements: §6.1 (`AgentRunner`), §10.3 (result
contract rules 1-6), §10.5 (Prime-Agent mechanics table), Annex C.1 plus the
C.2 rows this task implements (never-await, poll-contract-not-mailbox,
session_ref-only, isolation-none, list_subagents-as-forensics; the C.2 role
installation row and C.3 are Task 17's — interim semantics are the plan's
advisory inline-prompt fallback, plan:571), §8.5 (doctor, probe surfacing).
[RES-D11 corrected]

## 2. Units in play

- AK-008..AK-012 (admission non-blocking; completion requires schema-valid
  result; missing ⇒ TimedOut; malformed ⇒ Failed{malformed_result};
  mismatch ⇒ Failed{contract_mismatch}): their frozen locators are the
  SHARED suite cases (`tests/contract/agent_runner::*`), already proven
  against the mock in Task 8. Task 15 re-evidences them on a second, real
  adapter by passing the same suite unmodified.
- AK-013 (two adapters, one not the mock) is P8 and belongs to Task 16. Not
  discharged here.
- UNK-003 (concrete `runners.default`) is a recorded Tasks 15-16 deferral;
  this task does not force it.

## 3. The harness is PRESENT on this host — the redesign trigger does not fire

The plan's Task-15 redesign trigger ("If the harness is unavailable…") was
probed and does NOT apply:

- `prime-agent` 0.8.0 at `/home/sprime01/.local/bin/prime-agent` (sh shim →
  nub-managed node launcher at `~/.cache/nub/node/24.19.0/bin/prime-agent`,
  package at `~/.cache/nub/node/24.19.0/lib/node_modules/prime-agent`).
- Live round-trip executed during recon:
  `prime-agent --print --offline --no-tools "reply with exactly: PROBE_OK"`
  → `PROBE_OK`. A model provider is configured and functional.
- The upstream Annex-C surface is verifiably present in the installed
  package: `docs/rlm.md` and `docs/rlm-runtime.md` ship in-tree and describe
  exactly what the spec froze — `rlm(prompt, name=…)` returns an admission
  handle `{rlm_child_id, name, session_dir, model}` immediately and never
  the child's answer; results arrive only via `agent_message.send(...,
  receiver_role="parent")` or files; `rlm.list_subagents()` for
  liveness/forensics; `rlm.delete_subagent()`; depth configuration; the
  kernel "runs model-generated Python … with the worker's operating-system
  permissions … not a security sandbox" (⇒ honest `isolation: none`).
- Binary grep confirms the API: `rlm_child_id` appears in
  `dist/core/agent-session.js`, `dist/core/prompts/rlm.js`,
  `dist/core/refinement/refinement.js`.
- CLI control family maps onto the port: `prime-agent list [--json]`
  (liveness/forensics), `prime-agent stop <agent>` (cancel),
  `prime-agent send` (mailbox signal). Children are daemon-backed and
  survive parent exit (docs: long-running-agents.md, daemon.md).

Consequence: the adapter is delivered as a REAL runner, not
"configured but untested". The live-admission half is verified at gate time
by an explicitly env-gated probe (`GAUNTLET_PRIME_LIVE_PROBE=1`), with a
TRIVIAL child (writes its result file, replies) — not a builder, and never
through the authority gate. See §6.

## 4. Delegation transport — how a Rust adapter drives an IPython kernel

Annex C.1: "the adapter emits a kernel cell". The only model-facing tool of
the RLM runtime is `ipython`; the parent session is what executes the
rendered program. Headless shape chosen for production transport:

- `invoke` renders the Annex C.1 kernel program to a file:
  write the runtime-built envelope to the child input path, call
  `await rlm(<role prompt + result-contract instruction>, name=<role>-<slice>-<id-short>)`,
  write the returned handle fields as JSON to HANDLE_PATH.
  **Return point (RES-D1, corrected 2026-08-29):** the adapter watches
  HANDLE_PATH with a bounded timeout that guards the WATCH, not the parent
  process lifetime; `invoke` returns the moment the handle file exists and
  parses — never when the parent print-run ends. After parse, the parent's
  disposition is best-effort graceful stop (bounded; outcome recorded as
  forensics). If the parent model rambles or waits on the child, admission
  is already complete and invoke has already returned.
- The parent is driven headlessly (`prime-agent --print` with tools enabled,
  cwd injected by the composition root — not resolved from `WorkspaceRef`
  inside the adapter; RES-D3) with a message instructing it to execute the
  rendered program verbatim. The child is daemon-backed and keeps running
  after the parent print run exits; the live probe must EVIDENCE that
  survival (handle parsed while the parent is still running; poll reaches
  Completed only after the parent is gone).
- `poll` reads ONLY the result contract (§10.3): valid `role_result@v1` at
  the declared path whose `invocation_id` and `context_envelope_digest`
  match ⇒ `Completed`; malformed ⇒ `Failed{malformed_result}`; mismatched ⇒
  `Failed{contract_mismatch}`; deadline passed with no document ⇒ `TimedOut`
  + cancel (preserving `session_ref`); otherwise `Running`. A mailbox signal
  (`send`) is never consulted for completion. Annex C.4: a poll that finds
  no live process must NOT conclude failure while the contract is pending
  inside the deadline — liveness is forensics, never a state machine input.
- `cancel` shells to `prime-agent stop`; idempotent; unknown handle ⇒ typed
  `UnknownHandle`.
- `collect` reads + validates the document through the ports
  `read_result_contract` path after `Completed`.

The seams: the adapter is parameterized over a **kernel-transport** trait
(submit a rendered program, observe the admission outcome) and a
**harness-control** trait (stop/list). The production impl shells out to the
configured binary; the shared-suite factory binds in-memory doubles that
execute the rendered program's semantics per `RunnerScript`. This is the
suite's own design ("a real adapter maps it to a probe child instructed to
behave that way", `contract/agent_runner.rs:12-17`): the suite exercises the
adapter's real logic — program rendering, handle parsing, poll
classification, deadline handling, idempotent cancel, honest capabilities —
while the harness (kernel + model child) is the double. The suite stays
UNMODIFIED.

## 5. Version probe and doctor surfacing (plan step 4)

- The adapter probes the configured binary: `--version` parsed as semver,
  matched against a supported-range constant, plus a bounded presence probe
  of the Annex-C surface markers (the `docs/rlm.md`/`rlm.js` surface is
  package-internal; the durable contract is the versioned range). Probe
  failure types: binary absent, version unparseable, version outside the
  supported range — each a typed `configuration`-class error.
- Doctor surfacing follows the Task-14 measured-descriptor pattern (check 4
  reads a composition-root-MEASURED `BackendDescriptor`): doctor check 3
  (`check_runners`, `doctor/checks.rs:632`) today reports `unavailable` with
  reason "runner adapters land with plan Tasks 15–16". This task adds the
  resolution half for the prime adapter: when the resolved configuration
  names it, the composition root MEASURES the probe and check 3 reports the
  measured result; a probe mismatch is a check failure (non-zero exit), not
  a runtime surprise. The runner half of check 4 (OBS-1) is likewise fed
  from the measured capabilities.
- Layering: the probe lives in the adapter crate; `gauntlet-app` receives
  measured descriptors as typed input; only the CLI composition root
  constructs the adapter. No adapter identifiers leak inward (arch tests
  already anticipate the crate name: `denylist.rs:620`).

## 6. R-1b boundary — this task must NOT execute a real builder

CURRENT_STATUS hard trigger: close R-1b (per-unit workspace resolution in
`GovernDeps`) BEFORE any real runner executes a builder. Task 15 delivers a
runner but executes NO builder:

- The shared suite runs against transport doubles; no child of any kind is
  spawned by the default test run.
- The env-gated live probe spawns a trivial child that writes a file — it is
  not dispatched for any Unit, does not pass through `AuthorityGate` or
  `GovernDeps`, and exercises no workspace composition.
- No production caller composes `runner.invoke` for a Unit in this task
  (the controller's dispatch path remains mock-fed; R-3's
  `wrap_builder_command` gap is unchanged).

R-1b therefore remains OPEN at task15 settlement, still bound to the first
composed builder execution (Task 16's p8 with a real process adapter, or
Task 20's `run`). The closure record must restate this loudly; the prereg
records it as a nonclaim.

## 7. Existing code facts (file:line, verified this session)

| Fact | Location |
|---|---|
| Port trait + types | `crates/gauntlet-ports/src/agent_runner.rs` (AgentError, InvocationStatus, RunnerCapabilities, RoleInvocationSpec, InvocationHandle) |
| Result contract codec | `crates/gauntlet-ports/src/role_result.rs` (`role_result@v1`, strict decode, `validate_against`, `read_result_contract`) |
| Shared suite (10 cases) | `crates/gauntlet-ports/src/contract/agent_runner.rs` (`agent_runner_suite`, `AgentRunnerFactory`, `RunnerScript`, `ProbeBudget`) |
| Suite teeth | `crates/gauntlet-ports/src/contract/suite_runs.rs` (broken stubs must fail) |
| Mock factory pattern to mirror | `crates/adapters/gauntlet-adapter-agent-mock/tests/contract_suite.rs:18` |
| Stub crate to fill | `crates/adapters/gauntlet-adapter-agent-prime/` (doc-comment only, commit b69c095) |
| Doctor check 3 seam | `crates/gauntlet-app/src/doctor/checks.rs:632` (`check_runners`, resolution_reason "Tasks 15–16") |
| Doctor check 4 measured-descriptor precedent | `doctor/checks.rs:713+`, Task-14 `BackendDescriptor` pattern |
| Defaults module (only home of literals) | `crates/gauntlet-app/src/config/defaults*` (boundary-tested) |
| `rlm` upstream docs in-tree | `~/.cache/nub/node/24.19.0/lib/node_modules/prime-agent/docs/{rlm,rlm-runtime}.md` |

## 8. Risks carried into the prereg

- The `-p` parent round-trip is model-backed: admission latency is not
  deterministic. The live probe must bound it (timeout) and tolerate the
  parent refusing/failing to execute the cell — that outcome is an honest
  probe failure, not a crash.
- Daemon/socket availability on this host is unverified (no `prime-agent
  status` was run during recon). The live probe may need the daemon; if the
  daemon refuses in this WSL environment, the finding is recorded and the
  composed half takes the same "configured but untested" carve-out the plan
  prescribes for the whole harness — recorded, never faked.
- strict-decode of the child's result document requires the child to write
  `role_result@v1` exactly; the kernel program must embed the schema
  instructions (the mock's fixture writer is the reference for the exact
  bytes).
