# Task 15 / B1+B2 builder progress — Prime-Agent adapter

Recovery record. Appended after each completed unit of work. If this builder
dies, reconcile `git status` against the units below before treating anything
as unstarted (AGENTS.md session-interruption rule).

## Reading phase — COMPLETE

Read in order: AGENTS.md (in context), `.agents/reports/task15/recon.md`,
plan Task 15 (gauntlet-impl.md:498-521), spec §6.1/§10.3/§10.5/§8.5/§21.2/
Annex C, `gauntlet-ports/src/agent_runner.rs`, `role_result.rs`,
`contract/agent_runner.rs` (UNMODIFIABLE — untouched), mock
`runner.rs` + `tests/contract_suite.rs`, `doctor/checks.rs` (check 3 at
`check_runners`, check 4 `BackendDescriptor` precedent), `doctor/mod.rs`
(`DoctorInput`/`assemble`), `doctor/report.rs` (`CheckOutcome::combine`),
arch-tests `denylist.rs`/`walk.rs`/`literals.rs`, upstream `rlm.md` +
`rlm-runtime.md` (read-only, outside repo).

Key decisions derived from reading (with sources):

1. The adapter runner takes an INJECTED `Clock` port. The shared suite's
   `dispatch_spec` pins deadlines at `1_700_000_030` (year 2023 — already
   past), so a system-clock poll would report `TimedOut` for every suite
   case. Suite doubles drive a virtual clock (step per `now()` call);
   production binds a `SystemClock`.
2. `poll` calls `clock.now()` EXACTLY ONCE, after the result-file read, so
   the doubles' child-writes-on-tick behavior stays deterministic
   (invoke never calls the clock).
3. `ResultContractPath` is relative-by-construction (domain values.rs), so
   the real adapter resolves it against a composition-supplied `state_dir`
   root with containment checks (mirrors the mock's two-guard posture).
4. Denylist scope: `crates/adapters/**` is OUTSIDE the vendor/literal scans
   (arch-tests walk.rs CORE_CRATES); the composition root may name the
   adapter crate (masked) but not bare vendors. Segment-literal rule is
   Inner-only. The new crate's last segment `prime` is not an Annex-B wire
   key, so the maskable-wire-key pin stays `["command"]`.
5. B2 mirrors Task 14 exactly: new typed `RunnerDescriptor` input on
   `check_runners`, consulted only when `runners.default` resolves; default
   hook `None` keeps every existing doctor test green.

## MID-FLIGHT CORRECTIONS RECEIVED (orchestrator order, 2026-08-29)

RES-D1..D12 all ACCEPTED upstream; integrated into the implementation below.
See `.agents/reports/task15/design-adjudication.md`.

## UNIT 1 COMPLETE — crate scaffold + seams + renderer + runner + probe (lib compiles, 14/14 unit tests pass)

- RES-D1: `ProcessKernelTransport::submit` = spawn parent → BOUNDED HANDLE
  WATCH on `<key>.handle.json` (partial-write retries while the parent is
  alive) → strict parse → D1-split forensics (`parent.try_wait()` state AT
  PARSE, via `take_forensics()`) → best-effort GRACEFUL stop (SIGTERM via
  `kill` util, 1s bounded grace, last-resort kill), outcome recorded. The
  timeout guards the watch, never the parent lifetime.
- RES-D3: `invocation_dir()` = `<state_dir>/runs/<run_id>/inv/<invocation_id>/`;
  contract token must be a single leaf component (domain type refuses
  absolute; adapter refuses nested/`..` — both guards pinned by tests);
  envelope/child-input/handle are derived siblings. Parent cwd is an
  injected `PathBuf` (`parent_cwd`), never resolved from `WorkspaceRef`.
- RES-D4: taxonomy — `SpawnFailed` ⇒ `AgentError::Transport`; `HandleAbsent`
  /`HandleUnparsable` ⇒ `AgentError::AdmissionRefused` + orphan sweep via
  `control.list()` filtered by the deterministic invocation name + `stop`,
  outcome recorded in the error detail.
- RES-D5: `AdmissionHandle` struct with EXACTLY the four fields, strict
  decode (unknown fields rejected), `to_document()` shared by production
  transport and doubles so the suite exercises the real parse.
- RES-D6: `model_hint` forwarded into the rendered program as the `model`
  kwarg (omitted when None); capabilities unchanged (model_selection true).
- RES-D8: probe takes injected `VersionRange { min, limit }` (min inclusive,
  limit exclusive); no range literal and no marker probe in the adapter.
  The owning constants land in the app defaults module in B2.
- RES-D12: `Running.last_event_at` = contract-path mtime, else admission
  instant; `payment_so_far` = zero with the not-a-measurement doc.
- RES-D2 was already the design: terminal verdicts pin in the entry and
  re-report verbatim.

Files landed (all under `crates/adapters/gauntlet-adapter-agent-prime/`):
`src/lib.rs` (module doc kept + extended), `src/config.rs`, `src/probe.rs`,
`src/program.rs` (byte-pinned renderer + child-prompt composer, explicit
indentation — Rust `\`-continuations strip leading whitespace and were
silently re-indenting the emitted Python; fixed and pinned),
`src/transport.rs` (seams + process impls), `src/runner.rs`,
`src/clock.rs` (SystemClock), `src/test_support.rs` (cfg(test) only).
Cargo.toml untouched (dependencies already declared).

Checks run: `just check-package gauntlet-adapter-agent-prime` — PASS (no
warnings); `just test-package gauntlet-adapter-agent-prime` — 14/14 PASS.

One test-typo iteration: the domain `ResultContractPath::new` rejects
absolute tokens, so the hostile-token test pins the TYPE as first guard and
the adapter leaf check as second.

## UNIT 2 COMPLETE — contract suite integration test + arch await-ban + live probe

- `tests/contract_suite.rs`: the UNMODIFIED shared suite
  (`agent_runner_suite(&PrimeRunnerFactory, ProbeBudget::INSTANT)`) passes
  — 10/10 cases, zero findings. Doubles: `ScriptedTransport` (performs the
  rendered program's admission side effects, reads back through the REAL
  `read_admission` strict parse), `ScriptedControl` (records stops), and a
  `SuiteClock` (one step per `now()`; the child's result write is hooked
  to the tick so "work finishes between polls" is deterministic). The
  child's document lands as a REAL FILE at the RES-D3 anchored path
  `<state>/runs/<run>/inv/<inv>/<leaf>`, so poll/collect classify through
  actual file IO. NO processes are spawned (R-1b honored).
  Extra integration tests: orphan-sweep on handle-absent admission
  (AdmissionRefused + "stopped discovered child" in detail); spawn-failure
  ⇒ AgentError::Transport; stranger-handle refusals.
- Iterations: (a) the suite's dispatch deadlines are in the past, hence
  the injected clock (design); (b) the child-write threshold moved to
  tick 4 (invoke consumes one tick for the RES-D12 admission instant);
  (c) `SessionRef` refuses path-rooted tokens, so session_dir is recorded
  losslessly as `session-dir:<session_dir>` (documented in runner.rs).
- Arch await-ban: `crates/gauntlet-arch-tests/src/delegation.rs` — rules A
  (no decode calls outside runner.rs), B (in runner.rs only inside
  fn poll/fn collect; rustfmt fn-header tracking), C (AdmissionHandle
  carries EXACTLY the four admission fields). Documented exemption: the
  kernel template's `await rlm(...)` is the upstream admission-await
  (Annex C.1); the scan keys on result-decoding CALLS, not `await`
  tokens. Negative controls planted IN THE TEST prove rules A, B, C fire
  and that a missing crate cannot pass silently.
- Live probe: `tests/live_probe.rs`, gated on `GAUNTLET_PRIME_LIVE_PROBE=1`
  (skipped silently-with-a-note otherwise). Runs the PRODUCTION transport/
  control/clock; asserts the version probe, ONE real admission, the D1
  split via the transport's forensic note (`handle parsed while
  parent.try_wait()=NotExited` + `parent exited` before polling), poll →
  Completed from the file alone, collect identity, idempotent cancel ×2,
  stranger ⇒ UnknownHandle. Total bound 1200 s, honest failures name the
  locus. Added `impl KernelTransport for Arc<T>` so the forensic surface
  survives the runner taking ownership.

Checks run: `just test-package gauntlet-adapter-agent-prime` — 19/19 PASS;
`cargo nextest run -p gauntlet-arch-tests delegation` — 5/5 PASS;
`cargo clippy -p gauntlet-arch-tests --all-targets -- -D warnings` — clean.

## UNIT 3 COMPLETE — B2 doctor measured-descriptor + wiring

- `crates/gauntlet-app/src/config/defaults.rs`: `HARNESS_VERSION_MIN` /
  `HARNESS_VERSION_LIMIT` (min inclusive, limit exclusive) — the owning
  home of the supported range (RES-D8); named adapter-agnostically so no
  adapter identifier enters the app layer.
- `doctor/checks.rs`: new typed `RunnerDescriptor { token, measured, note }`
  (BackendDescriptor mirror, RES-D7). `check_runners(resolved, runner)`:
  with a named runner + measured descriptor ⇒ Pass/Fail on the resolution
  half (failed probe = `runner_probe_failed` configuration failure with
  locus; mistokened = `runner_resolution_mismatch`); no descriptor ⇒ the
  historical unavailability stands (all 32 pre-existing doctor tests stay
  green). Evidence names the measured dimension: harness API-surface
  version compatibility ONLY. `check_capabilities(resolved, backend,
  runner)`: the runner half is fed from the same descriptor — failed probe
  ⇒ Fail, measured ⇒ Pass with "version compatibility measured (that
  dimension only)" + per-invocation dispatch-gate note. Check 5 untouched.
- `doctor/mod.rs`: `DoctorInput.runner_measure` hook; `assemble` consults
  it only when `runners.default` resolves.
- `crates/gauntlet-cli`: production dep on the runner adapter crate;
  `measure_runner(token)` fronts the adapter, injects the defaults-module
  range, probes with `doctor_bound()`. Token comparison against the
  adapter's `RUNNER_TOKEN` constant — the literal lives only under
  `crates/adapters/**`.
- 4 new doctor tests (measured pass; failed probe fails checks 3 AND 4;
  mistokened; no-hook unavailability). Test docs use a neutral
  `probed-runner` token — an adapter literal in gauntlet-app tripped the
  denylist once (caught by `just boundaries`, fixed, re-green).

## CLOSING GATES — ALL GREEN (2026-08-29)

- `just check-package gauntlet-adapter-agent-prime` — PASS, no warnings
- `just test-package gauntlet-adapter-agent-prime` — 19/19 PASS
- `just test-package gauntlet-app` — 350/350 PASS (doctor: 32 pre-existing + 4 new)
- `just check` — PASS (workspace, all targets)
- `just test` — 1108/1108 PASS (full workspace)
- `just lint` — clippy -D warnings clean + `cargo fmt --check` clean
- `just boundaries` — 220/220 PASS
- `just ruler` — PASS (digests match the frozen lock)
- Suite byte-identity: `git diff crates/gauntlet-ports/src/contract/
  agent_runner.rs` is EMPTY (RES-D9 evidence for the closure record)
- Not committed — orchestrator commits.

## COR-D1 — gate-time correction (live probe fired prereg P3, orchestrator-verified)

The orchestrator ran the live probe: it FAILED honestly at step 1 with
`VersionUnparseable { output: "" }` — the REAL harness prints `--version`
to STDERR (stdout empty, stderr `0.8.0`, exit 0; the recon probe merged
streams and masked the divergence). Fixed in `src/probe.rs`:

- BOTH streams are piped and captured; the strict parser is unchanged and
  scans the COMBINED output (stdout first, then stderr).
- `VersionUnparseable { binary, stdout, stderr }` now carries BOTH
  streams' content (Display renders both) — the locus is fully
  diagnosable whatever a future harness does.
- Typed errors and the fail-closed direction unchanged; `list --json` is
  stdout-based and unaffected (verified upstream by the orchestrator).
- Hermetic unit tests (unix-gated fixture scripts, NOT the real harness):
  stderr-only version probes clean (THE pin); stdout-only still clean
  (regression guard); neither stream parseable ⇒ the error carries both
  streams. One test iteration: the fixture bound was a fixed past epoch —
  the probe reads the wall clock, so the bound is now a span past now.
- Module doc records the COR-D1 finding and the stream-capture posture.

Gates re-run: `just test-package gauntlet-adapter-agent-prime` — 22/22
PASS (3 new fixture tests); `just lint` — PASS. Live probe NOT re-run by
the builder (orchestrator runs it after this fix lands).

## COR-D2 — gate-time correction #2 (live admission diagnosed by execution, orchestrator)

Orchestrator-executed probes (6 bounded parent runs + 1 real child;
adjudication addendum 2) isolated three divergences; all three fixed:

1. **Renderer (`program.rs`)**: the upstream `RLMSpawnHandle.session_dir`
   is a `PosixPath` — `json.dumps` on the raw fields raised
   `TypeError: Object of type PosixPath is not JSON serializable` INSIDE
   the cell after a SUCCESSFUL admission, so invoke saw "handle absent".
   All four handle fields are now coerced with `str(...)` in the rendered
   program; both byte-pin round-trip tests updated (escaping tests pass
   on top of the coercion unchanged).
2. **Linger (load-bearing, renderer + config + transport)**: a child of a
   print-mode parent materializes ONLY while the parent stays alive past
   admission (verified: no linger → child never appears; 60 s in-cell
   linger → child completed its work at +15 s).
   - `PrimeAdapterConfig.admission_linger_seconds` (default 30,
     evidence-cited doc + `admission_linger()` accessor).
   - The rendered cell appends `import time; time.sleep(<linger>)` AFTER
     the handle write — the PARENT's linger; `invoke` STILL returns the
     moment the handle parses (D1 untouched).
   - The transport's SUCCESS-path disposition is now a DETACHED bounded
     reaper thread (`std::thread`, moved `Child`, forensics shared via
     `Arc<Mutex<Option<String>>>`): wait linger (recording a natural
     exit), then SIGTERM + grace + last-resort kill only if still alive.
     The note records `reaper dispatched (linger Ns)` at parse and the
     reaper's outcome lands on the forensic stream later. Failure paths
     keep the IMMEDIATE bounded stop (`stop_bounded(child, ZERO)`).
3. **Harness control (`list --json` shape)**: the harness returns
   `{"sessions": [...]}` — a top-level OBJECT (divergence #2). The parse
   is now the pure `parse_listing` (exported): EXACTLY the
   `{"sessions": [...]}` shape accepted; bare arrays, missing/
   wrong-shaped/extra-keyed wrappers, invalid JSON, and entries without
   `session_name` are typed errors, never coerced. Five hermetic fixture
   tests pin the shapes (no real-harness dependency).
- `tests/live_probe.rs` updated: constructor takes the linger; the D1
  assertions now pin `handle parsed while parent.try_wait()=NotExited`
  + `reaper dispatched` (the reaper's own outcome arrives later on the
  forensic stream by design).

Gates re-run: `just test-package gauntlet-adapter-agent-prime` — 27/27
PASS; `just check-package gauntlet-adapter-agent-prime` — PASS;
`just lint` — PASS. Live probe NOT re-run by the builder (orchestrator
runs it after this fix lands).

## COR-D3 — final correction (falsifier round 1 CONFIRM; missing teeth added)

The falsifier's teeth mutations exposed three guarantees with no
committed test that fails when broken (M2, M7, M1-nuance). TESTS ONLY —
zero production-code changes, frozen suite untouched:

- **F1 handle-decode teeth** (`src/transport.rs`, `mod
  handle_decode_teeth`): all 16 A5 hostile classes through
  `AdmissionHandle::parse` — 1 ANSWER-SHAPED unknown field
  (`"answer":"the result is 42"`), 4 missing-field, 2 empty-string,
  3 wrong-typed, 6 wrong-shape — each asserted as the typed
  `HandleUnparsable`; the class inventory is pinned (len == 16) and a
  valid-document control keeps the refusals honest; the answer-shaped
  refusal NAMES the field; `read_admission` refuses a hostile FILE (the
  shared production/double path). End-to-end in
  `tests/contract_suite.rs`: an `AnswerShapedHandleTransport` double ⇒
  `invoke` returns `AdmissionRefused` naming `answer` (mutation M2's
  exact end-to-end shape).
- **F2 cancel no-second-effect teeth** (`tests/contract_suite.rs`): a
  `CountingControl` double with an `Rc`-shared stop log — cancel +
  second cancel + third ⇒ typed no-ops with EXACTLY ONE harness stop
  ever (mutation M7's second re-stop fails here by construction).
- **F3 late-writer resurrection teeth** (`tests/contract_suite.rs`): the
  exact M1/A1 shape — admit, poll the virtual clock past the deadline ⇒
  `TimedOut`, THEN write a schema-valid identity-MATCHING document at
  the anchored contract path ⇒ three more polls all `TimedOut` (never
  Completed), `collect` ⇒ `ResultNotReady{TimedOut}`.

Gates re-run: `just test-package gauntlet-adapter-agent-prime` — 34/34
PASS (7 new teeth tests); `just check-package
gauntlet-adapter-agent-prime` — PASS; `just lint` — PASS. No mechanism
mutated (per the order); no live probe run.

## NEXT — orchestrator settlement

