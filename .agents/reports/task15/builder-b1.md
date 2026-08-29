# Task 15 / B1+B2 — builder result contract

Builder: Prime-Agent adapter (slices B1 + B2). Status: **COMPLETE, all
gates green**. Nothing committed (orchestrator commits). Recovery record:
`.agents/reports/task15/b1-progress.md`.

## What was delivered

### B1 — `crates/adapters/gauntlet-adapter-agent-prime` (was a doc-comment stub; doc kept + extended)

Production files (all new except `src/lib.rs`):

- `src/lib.rs` — module doc extended; exports; `RUNNER_TOKEN` (the
  adapter-identifying literal lives ONLY here, under `crates/adapters/**`);
  capability-named façade `RecursiveSubagentRunner` (bwrap precedent).
- `src/config.rs` — typed `PrimeAdapterConfig` with `Default`:
  `binary` (bare name `prime-agent`, PATH-resolved at use — no absolute
  path literal), `max_depth` (default 1, §8.6), `admission_timeout`
  (default 600 s, bounded). The supported version range is deliberately
  NOT here (RES-D8: injected by the composition root from the app
  defaults).
- `src/program.rs` — the Annex C.1 kernel-program renderer, BYTE-PINNED
  (round-trip tests, one pin without model kwarg + one with): copies the
  envelope to the child input path, `await rlm(<composed prompt>,
  name=<role>-<run_short>-<inv_short>[, model=<hint>])`, writes the four
  admission-handle fields as JSON to the handle path, returns. No model
  hint ⇒ no model kwarg (RES-D6). `compose_child_prompt` renders the
  role body + the result-contract instruction: atomic write (tmp + flush
  + rename) to the declared path, `invocation_id`/`context_envelope_digest`
  echo (§10.3 rule 6), `GAUNTLET_RESULT_PATH` named, reply-is-signal/
  file-is-contract rule. Python literal escaping is hostile-input tested
  (quotes, backslashes, newlines, control chars).
- `src/transport.rs` — the two harness seams:
  - `KernelTransport::submit(&KernelSubmission, Deadline) ->
    Result<AdmissionHandle, TransportError>`. `AdmissionHandle` carries
    EXACTLY `rlm_child_id`/`name`/`session_dir`/`model` (pinned by the
    arch scan, rule C) — a child answer is unrepresentable (RES-D5).
    Strict handle decode from the handle FILE: unknown fields, missing
    fields, wrong shapes and empties are typed errors; production
    transport and suite doubles share `read_admission`, so the suite
    exercises the real untrusted-input parse.
  - `ProcessKernelTransport` (RES-D1, blocking correction): spawns the
    headless parent print run (`--print`, program inline, delimited,
    cwd = the INJECTED parent cwd), then runs the BOUNDED HANDLE WATCH —
    the timeout guards the watch on HANDLE_PATH, never the parent
    process lifetime (partial-write retries while the parent is alive).
    On parse it records the D1-split forensics (`parent.try_wait()` state
    AT PARSE via `take_forensics()`), then best-effort GRACEFUL stop
    (SIGTERM via the `kill` utility, 1 s bounded grace, last-resort
    kill), every step recorded. Failure taxonomy (RES-D4):
    `SpawnFailed` ⇒ transport/binary; `HandleAbsent`/`HandleUnparsable`
    ⇒ nothing verifiably admitted. std-only process spawning.
  - `HarnessControl::{stop, list}` — `ProcessHarnessControl` shells to
    the configured binary (`stop <child>`, `list --json`), bounded;
    already-gone maps to `NoSuchChild` (cancel treats it as success);
    `list` is forensics/orphan-sweep ONLY — never a poll input (Annex
    C.2/C.4).
- `src/runner.rs` — `PrimeAgentRunner<T: KernelTransport, C:
  HarnessControl>` implementing `AgentRunner`:
  - RES-D3 anchor: invoke creates
    `<state_dir>/runs/<run_id>/inv/<invocation_id>/`; the contract token
    resolves as the literal LEAF (domain type refuses absolute tokens —
    first guard; the adapter refuses nested/`..` — second guard; both
    pinned by tests). Envelope/child-input/handle are derived siblings.
    Parent cwd is an injected `PathBuf`; the adapter never resolves a
    `WorkspaceRef`.
  - `invoke`: duplicate admission refused; envelope written; program
    rendered; submits; ADMISSION ONLY (RES-D1 — the transport watches
    the handle, disposes of the parent, returns the four fields);
    `session_dir` recorded losslessly as `session-dir:<session_dir>`
    (the domain `SessionRef` refuses path-rooted tokens — a real
    session_dir IS a path; documented, reversible, forensics-only).
    Failure taxonomy + ORPHAN SWEEP (RES-D4): on handle-absent/
    unparsable admission failure the runner consults `control.list` by
    the deterministic invocation name, stops discovered children, and
    records the sweep outcome in the error detail; `SpawnFailed` ⇒
    `AgentError::Transport`.
  - `poll` (RES-D2/D12): the ONE state input is the result contract via
    `read_result_contract` (strict decode + rule-6 identity check,
    `failure_class()` mapping): valid ⇒ `Completed{result_ref}`;
    malformed ⇒ `Failed{malformed_result}`; mismatched ⇒
    `Failed{contract_mismatch}`; unreadable file ⇒ typed `Transport`
    error (never coerced Running); deadline passed without a document ⇒
    `TimedOut` + best-effort stop (locus recorded; the verdict stands);
    otherwise `Running{last_event_at = contract-path mtime else
    admission instant, payment_so_far = zero — explicitly NOT a
    measurement; budget truth stays with the BudgetMeter}`. NO mailbox
    is ever read; liveness is never consulted; terminal verdicts pin and
    re-report verbatim (a late-writing child cannot resurrect a
    TimedOut). One clock read per poll; clock is the INJECTED port (the
    suite's pinned deadlines are in the past — a system clock could not
    drive them).
  - `collect` after Completed, same document every time; `cancel`
    idempotent (`AlreadyTerminal` on later calls), caller's deadline
    bounds the stop, stranger handle ⇒ `UnknownHandle`; failed stop
    leaves state unpinned (retryable).
  - `capabilities()` (RES-D6): native_subagents TRUE, streaming_events
    FALSE (print-mode surface), model_selection TRUE (model_hint
    forwarded), isolation NONE (Annex C.2 honesty — the tier check can
    refuse this adapter), result_delivery FILE, max_depth from config.
  - `forensic_record(handle)` — session_ref + invocation name + control
    note, evidence only (I11).
- `src/probe.rs` — `probe(binary, &VersionRange, deadline)`: presence
  (PATH resolution), bounded `--version`, strict semver parse
  (v-prefix tolerated), range match (min inclusive, limit exclusive).
  Typed configuration-class errors: `BinaryAbsent` / `VersionUnparseable`
  / `VersionUnsupported{range}` / `ProbeFailed`. NO package-marker or
  install-layout probe (RES-D8: dropped).
- `src/clock.rs` — production `SystemClock` (Clock port); the runner
  always takes the port.

Tests (adapter crate, 19/19):

- `tests/contract_suite.rs` — the UNMODIFIED shared suite
  (`agent_runner_suite(&PrimeRunnerFactory, ProbeBudget::INSTANT)`)
  passes with ZERO findings: AK-009..AK-012 re-evidenced on a second,
  real adapter. Doubles execute the rendered program's SEMANTICS only
  (transport double writes the handle document and reads it back through
  the real strict parse; the child's result write is driven by the
  virtual clock the runner itself polls; the result lands as a REAL FILE
  at the RES-D3 anchored path). NO process is spawned by the default
  battery (R-1b honored). Plus: orphan-sweep evidence, spawn-failure ⇒
  Transport classification, stranger-handle refusals.
- `tests/live_probe.rs` — env-gated (`GAUNTLET_PRIME_LIVE_PROBE=1`),
  skipped otherwise; NOT in the default battery; costs one real model
  round trip when enabled. Asserts: version probe; ONE real admission
  through the production transport; the RES-D1 EVIDENCE LEG (forensic
  note shows `handle parsed while parent.try_wait()=NotExited` then
  `parent exited` before polling — proving invoke did not wait on the
  parent and completion is file-derived, which also evidences
  daemon-backed child survival); poll → Completed; collect identity;
  idempotent cancel twice + stranger ⇒ UnknownHandle. Total bound 1200 s;
  every failure names its locus. NOT yet executed against the live
  harness (operator may run: `GAUNTLET_PRIME_LIVE_PROBE=1 cargo nextest
  run -p gauntlet-adapter-agent-prime live_probe`).
- Unit tests across config/program/probe/runner/clock (byte pins, escaping
  round-trip, leaf-anchor guards, name mapping, range parsing).

### Arch await-ban (`crates/gauntlet-arch-tests/src/delegation.rs`, new)

Greppable form and exactly what it catches (RES-D5):

- Rule A: the result-decoding CALLS (`read_result_contract(`,
  `RoleResult::decode(`) in any adapter file other than `runner.rs` —
  catches the transport seam (or any helper module) deriving a result
  from what it observed, including parsing parent stdout as a result
  source.
- Rule B: inside `runner.rs`, those calls only within `fn poll`/`fn
  collect` (rustfmt fn-header tracking) — catches `invoke` (or a helper)
  deriving a result from the delegation call.
- Rule C: `AdmissionHandle` carries EXACTLY the four admission fields —
  catches an answer-shaped field being added to the transport's success
  payload.
- Documented EXEMPTION: the kernel template's `await rlm(...)` is the
  upstream admission-await (Annex C.1); the scan keys on result-decoding
  calls, not `await` tokens, so it does not self-flag — and any future
  await-shaped rule must declare the `program.rs` template exemption
  here.
- NEGATIVE CONTROL: fixtures planted in the test (not committed to the
  adapter) prove rules A, B, C each REPORT, and that a missing crate
  cannot pass silently. 5/5 arch tests pass; the real adapter is clean.

### B2 — doctor check 3 resolution half (measured-descriptor pattern)

- `crates/gauntlet-app/src/config/defaults.rs`: `HARNESS_VERSION_MIN` /
  `HARNESS_VERSION_LIMIT` — the range's owning home (RES-D8); named
  adapter-agnostically (no adapter identifier in the app layer).
- `doctor/checks.rs`: typed `RunnerDescriptor { token, measured, note }`
  (BackendDescriptor mirror). `check_runners(resolved, runner)`: named
  runner + measured descriptor ⇒ resolution half reports
  resolved/failed-with-locus instead of the blanket unavailable;
  `runner_probe_failed` / `runner_resolution_mismatch` are FAILS
  (non-zero exit class); no descriptor ⇒ historical unavailability
  stands. Evidence names the measured dimension: harness API-surface
  version compatibility ONLY. `check_capabilities(resolved, backend,
  runner)`: the runner half consumes the same descriptor — failed probe
  ⇒ Fail, measured ⇒ Pass ("that dimension only"; isolation-vs-tier
  stays the dispatch gate's). Check 5 untouched; no check weakened.
- `doctor/mod.rs`: `DoctorInput.runner_measure` hook, consulted only
  when `runners.default` resolves.
- `crates/gauntlet-cli` (composition root, following Task 14): production
  dep on the runner adapter; `measure_runner(token)` fronts the adapter
  (`RUNNER_TOKEN` comparison), injects the defaults-module range, probes
  with `doctor_bound()`. Only capability-named items are spelled.
- 4 new doctor tests: measured pass (checks 3 AND 4 upgrade from
  unavailable to pass); failed probe fails 3 and 4 with the locus;
  mistokened descriptor; no-hook unavailability.

## Gates run (exact results)

| Gate | Result |
|---|---|
| `just check-package gauntlet-adapter-agent-prime` | PASS (no warnings) |
| `just test-package gauntlet-adapter-agent-prime` | 22/22 PASS (after COR-D1; 19/19 before) |
| `just test-package gauntlet-app` | 350/350 PASS (doctor: 32 pre-existing + 4 new) |
| `just check` | PASS (workspace, all targets) |
| `just test` | 1108/1108 PASS (full workspace) |
| `just lint` | PASS (clippy `-D warnings` + `cargo fmt --check`) |
| `just boundaries` | 220/220 PASS |
| `just ruler` | PASS — digests match the frozen lock |

### COR-D1 addendum (gate-time correction #1, orchestrator-ordered)

The orchestrator's live-probe run fired prereg P3 honestly: the REAL
harness prints `--version` to STDERR (stdout empty, stderr `0.8.0`, exit
0). `src/probe.rs` now pipes BOTH streams and parses the first semver
token from the COMBINED output (strict parser unchanged);
`VersionUnparseable { binary, stdout, stderr }` carries both streams for
full diagnosis; typed errors and the fail-closed direction are unchanged.
Three hermetic fixture-script tests pin the behavior (stderr-only clean,
stdout-only clean, both-streams-in-error) — no real-harness dependency in
the default battery. Re-run after the fix: `just test-package
gauntlet-adapter-agent-prime` 22/22 PASS, `just lint` PASS. The live
probe itself was NOT re-run by the builder (orchestrator will verify).

Suite byte-identity (RES-D9 evidence): `git diff
crates/gauntlet-ports/src/contract/agent_runner.rs` is EMPTY. The plan's
second gate line (`contract::agent_runner -- --adapter prime`) does not
exist as such; the substitution (factory-pattern integration per the mock
precedent, `tests/contract_suite.rs`) is the recorded proof — for the
closure record to state.

## Deviations from the brief (with reasons)

1. **Supported-range constant moved out of the adapter** (brief said "a
   supported-range constant … in the crate's own declared-constants
   location"): superseded by the accepted correction RES-D8 — typed
   `VersionRange` injected from the app defaults module. No literal and
   no marker probe remain in the adapter.
2. **invoke/transport shape**: the brief's "submits a rendered kernel
   program and returns a typed ADMISSION outcome" was implemented per
   the BLOCKING correction RES-D1 — return pinned to parsed-handle
   existence with a bounded handle watch; the parent run is disposed
   best-effort after parse; the timeout guards the watch, not the
   parent's lifetime.
3. **Session_ref encoding**: `SessionRef` (domain) refuses path-rooted
   tokens while a real harness session_dir IS a path; the adapter
   records it losslessly as `session-dir:<session_dir>`. Documented in
   runner.rs; forensics-only.
4. **Doctor test tokens**: the new doctor tests use a neutral
   `probed-runner` token (an adapter literal in gauntlet-app trips the
   denylist — caught once by `just boundaries`, then fixed).
5. **Two clippy allows in test code**: contract_suite.rs is
   crate-level `arc_with_non_send_sync` (doubles are deliberately
   single-threaded — the mock runner's own documented posture) and one
   `type_complexity` on the SuiteClock callback field. Production code
   carries none (transport forensics use a Mutex to stay Send+Sync).
6. **Not executed**: the env-gated live probe was not run (it costs a
   real model round trip and is explicitly opt-in). The composition
   (`runner.invoke` for a Unit) remains unwired per R-1b — R-1b stays
   OPEN, unchanged by this task.

## Open questions for the orchestrator

1. Live probe execution: run
   `GAUNTLET_PRIME_LIVE_PROBE=1 cargo nextest run -p
   gauntlet-adapter-agent-prime live_probe` when a model round trip is
   authorized (recon §8 notes daemon availability on this host is
   unverified; a daemon refusal is an honest finding, never faked).
2. Cargo.lock shows the workspace-member edge (cli → prime adapter) —
   expected; no external dependency changed.
3. RES-D11 recorded: role installation/verification (Annex C.2 row 5 /
   C.3) is Task-17-owned; this task implements the never-await /
   poll-contract / session_ref-only / isolation-none /
   list-as-forensics rows. Interim role semantics are advisory
   inline-prompt.

## Orchestrator re-verification commands

```bash
cd /home/sprime01/projects/gauntlet
git diff --exit-code -- crates/gauntlet-ports/src/contract/agent_runner.rs   # suite byte-identity
just check-package gauntlet-adapter-agent-prime
just test-package gauntlet-adapter-agent-prime
just test-package gauntlet-app
just check
just test
just lint
just boundaries
just ruler
# optional, costs one real model round trip:
GAUNTLET_PRIME_LIVE_PROBE=1 cargo nextest run -p gauntlet-adapter-agent-prime live_probe
```
