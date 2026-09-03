# COR-1 progress (declare-first; updated as work proceeds)

Builder: COR-1, Task 21 operator surfaces. Tree reconciled at HEAD a71eba9,
crates/ clean at start. Compile lock: `.agents/reports/task21/compile.lock`.

# COR-1 progress (FINAL)

Builder: COR-1, Task 21 operator surfaces. Slice COMPLETE — all ten items
DONE, all six gate groups GREEN on the final tree. Full per-item record:
`cor1-report.md`. Head still a71eba9; nothing staged or committed; both
cli_help snapshots and all frozen files byte-identical to start.

## Items (final)

- C1 DONE · C2 DONE · C3 DONE · C4 DONE · C5 DONE · C6 DONE (recorded
  test limitation: no DNS control for a mixed-address set) · C7 DONE
  (shared-constant shape via gauntlet_app::govern, gate-side drift pin)
  · C8 DONE · C9 DONE · C10 DONE.

## Gate log (final; every line post-last-edit)

- Gate 1: no-bypass suite `--features surfaces` → 8/8 PASS.
- Gate 2: `gauntlet-adapter-surface-json` package → 10/10 PASS.
- Gate 3: `gauntlet-app settle::` → 30/30 PASS; governance pin test →
  1/1 PASS.
- Gate 4: `cargo build -p gauntlet-cli --no-default-features` → PASS.
- Gate 5: `just check && just test && just lint && just deny &&
  just boundaries && just ruler` → 1391/1391 tests, clippy -D warnings
  clean, deny ok, boundaries 228/228, ruler PASS.
- Gate 6: e2e_full_loop_mock default → 12/12; `--features surfaces` →
  12/12.
- Guardrails: `git diff --stat HEAD` empty over both cli_help snapshots,
  answer key, reference.lock, .agents/specs/; Cargo.lock untouched; tree
  left dirty.

Failing-first evidence per item is in `cor1-report.md` (compile RED for
C3/C4, executed assertion REDs for C1/C2/C5/C7/C8/C9 with the exact
panic lines captured in this file's history and the report).

## Items

- C1 (§20.3 log field parity refused/failed): DONE —
  `operator_action.rs`: new private `ActionLogContext` (run + tickets)
  threaded through `decide_operator_action`, set the moment the durable
  reads resolve them; refused and failed arms now carry
  `action run_id unit operator outcome tickets code` (one shape for all
  outcomes; empty until known — never invented). Note still never logged.
  Failing-first: leg 1 RED — "the refused §20.3 line names the run:
  operator-action: action=resolve unit=AK-901 ... outcome=refused
  code=verification_not_pass" (no run_id/tickets field). GREEN in gate 1.
- C2 (re-entry message truth): DONE — re-entry branch now says
  "the settlement is already durable; the queue write failed: {error}";
  fresh-accept path keeps "the settle committed but the queue write
  failed". Test:
  `the_reentry_failure_names_the_durable_truth_the_fresh_one_names_the_commit_and_the_acceptance_names_both_ids`
  (part A re-entry replay + part B chmod-555 fresh-commit window +
  recovery). GREEN in gate 1.
- C3 (accept-error classification): DONE — private `AcceptSource` seam
  (`TcpListener` is the production impl), `serve_while` delegates to
  `serve_accept_loop(&dyn AcceptSource, ...)`, classification in
  `accept_error_is_transient` (continue: Interrupted/ConnectionAborted/
  ConnectionReset/BrokenPipe/WouldBlock/TimedOut/OutOfMemory/StorageFull
  + raw ENFILE(23)/EMFILE(24) fallback because this toolchain's
  `ErrorKind::TooManyOpenFiles` is unstable to name; break: everything
  else incl. EBADF/EINVAL/NotFound classes). Transient prints
  "continuing"; fatal prints "the listener is no longer usable; the
  surface stops" and breaks. Module doc rewritten to describe the real
  classification. Tests: classification table test + injected-sequence
  loop test (2 transient errors, then a served Ok step, then EBADF, then
  an unreachable step; asserts flag never fell, exactly-through-fatal
  consumption, the Ok step answered). Failing-first: the seam/classifier
  did not exist pre-fix — package compile failure (E0425/E0405/E0422)
  captured. GREEN in gate 2.
- C4 (escape access-log target): DONE — `access_log_line(method, target,
  status)` debug-escapes the target; `handle` logs through it. Unit test
  asserts no raw 0x1b and the `\u{1b}` escaped form. Failing-first: same
  compile failure as C3 (helper did not exist). GREEN in gate 2.
- C5 (unit view 404 non-durable run): DONE — `durable_surface.rs
  unit_view` now does the run view's own `load_run` durability check
  first, `RunScopedError::NotFound` with the SAME wording ("run {run} is
  not durable in this state store"); http.rs maps NotFound → 404
  not_found (existing mapping). Derived-run path unaffected in honest
  worlds (its ids come from open tickets). Test:
  `a_unit_read_on_a_non_durable_run_404s_with_the_run_view_s_own_wording`
  (+ run-view parity + derived-path-200 control). Failing-first: RED —
  "the unit read refuses a non-durable anchor: HTTP/1.1 200 OK" with
  default status:"open" body. GREEN in gate 1.
- C6 (bind requires ALL addresses loopback): DONE — `SurfaceHttp::bind`
  now refuses when ANY resolved address is non-loopback (find-first
  offender; same typed `BindRefused`, message names the offender).
  Existing bind tests green in gate 2. No new test: a mixed
  non-loopback+loopback multi-address set needs DNS control this host
  does not offer (the falsifier hit the same wall) — recorded as
  deviation-by-necessity in the report.
- C7 (sandbox_denials pinned to governance vocabulary): DONE — shared
  vocabulary exported from the construction site:
  `gauntlet_app::govern::gate::CONTAINMENT_SUMMARY_TOKENS` (one token per
  containment arm) + `containment_summary_matches`, re-exported at
  `gauntlet_app::govern`. Shape taken: SHARED-CONSTANT (preferred) —
  viable because the CLI already depends on gauntlet-app (no new edge);
  drift is pinned by a new gate-side test asserting every containment
  variant's own `summary()` matches and the authority-class variants do
  not. `durable_surface.rs` filter + module doc + inline comment now
  name the shared matcher and the exact five spellings. Test:
  `the_denial_view_names_the_governance_gate_s_containment_vocabulary`
  (escalates the gate's OWN CapabilityMismatch summary — no "sandbox"
  token — and asserts it appears in /api/v1/state's sandbox_denials;
  asserts the view still invents nothing). Failing-first: RED — "the
  runner-isolation escalation is IN the denial view: []". GREEN in
  gate 1; pin test GREEN in gate 3.
- C8 (non-string note → typed 400): DONE — http.rs body parse: a
  present `note` that is not a JSON string is `400 body_unparsable`
  ("the action body's 'note' must be a JSON string"); unknown fields
  stay ignored, now documented inline (parse-validate-narrow posture).
  Test: extension of `malformed_requests_are_typed_400s` with
  `{"note": 123}`. Failing-first: with the fix temporarily reverted to
  the old coercion, the test RED at the new assertion ("a non-string
  note is 400: 409"); re-applied → GREEN in gate 2 (10/10).
- C9 (acceptance output discloses unit + tickets): DONE —
  `escalations_cmd.rs` acceptance line now prints
  `ticket=<requested> action=<w> operator=<op> — settled unit <unit> of
  run <run>; resolved tickets: <list|(none)>`: the settled unit, the
  requested ticket, and the actually-resolved list are separate named
  facts (both ids verbatim for an explicit-ticket action). §20.3 accepted
  log line already carried unit= and tickets= separately (verified, no
  change needed). No ticket↔unit binding check added (deferred ports
  revision). Failing-first: RED — "the acceptance names the settled
  unit: escalations: ticket=esc-6a98caed-0 ... settled through the
  settlement service's operator path". GREEN in gate 1.
- C10 (doc/comment truth): DONE — lib.rs module doc + `snapshot_json`
  doc now state the actual rule (run preamble, then §20.2 sections in
  priority order, disclosure last) and record the D2 tension instead of
  the false "escalated FIRST = RuntimeSnapshot field order" parenthetical;
  escalations_cmd.rs header names the real path
  (`execute_operator_action` / `settle_operator_action_gate` /
  `settle_operator_action_earned`); gauntlet-cli Cargo.toml says "the
  only feature edge THIS task adds". No behavioral test (doc-only);
  covered by gate 5 compilation + `just ruler`.

## Toolchain facts established

- std ErrorKind mapping on this toolchain (probe): EINTR→Interrupted,
  ECONNABORTED→ConnectionAborted, ECONNRESET→ConnectionReset,
  EPIPE→BrokenPipe, EAGAIN→WouldBlock, EMFILE/ENFILE→TooManyOpenFiles,
  ENOBUFS→Uncategorized, EBADF→Uncategorized, EINVAL→InvalidInput,
  ENOENT→NotFound. `ErrorKind::TooManyOpenFiles` and
  `ErrorKind::Uncategorized` are UNSTABLE to name here (E0658) — the
  classifier names stable kinds and falls back to raw_os_error for the
  EMFILE/ENFILE pair. ENOBUFS therefore lands in the break class (no
  nameable kind); recorded in the report as a known classification limit.

## Gate log

- Failing-first, gate 2 target (pre-fix): `cargo nextest run -p
  gauntlet-adapter-surface-json` → compile error E0425 `access_log_line`,
  E0425 `accept_error_is_transient`, E0405/E0422 (AcceptSource) — the
  C3/C4 mechanisms did not exist. [captured]
- Failing-first, gate 1 target (pre-fix): `cargo nextest run -p
  gauntlet-cli --features surfaces --test operator_actions_no_bypass` →
  3 FAILED / 2 cancelled-by-fail-fast: C5 leg ("HTTP/1.1 200 OK" for a
  non-durable anchor), C7 leg ("denial view: []"), C1 leg (refused §20.3
  line without run_id/tickets). [captured]
- Failing-first, C2/C9 test alone (pre-fix): RED at the C9 assertion —
  acceptance line "settled through the settlement service's operator
  path" names no settled unit. [captured]
- Failing-first, C8 (coercion temporarily re-applied): RED at "a
  non-string note is 400: 409"; C3/C4 tests PASS in the same run.
  [captured]
- Gate 1 (post-fix): `cargo nextest run -p gauntlet-cli --features
  surfaces --test operator_actions_no_bypass` → 8/8 PASS. [GREEN;
  re-run again after rustfmt for final-tree evidence]
- Gate 2 (post-fix): `cargo nextest run -p gauntlet-adapter-surface-json`
  → 10/10 PASS (contract suite included). [GREEN]
- Gate 3: `cargo nextest run -p gauntlet-app settle::` → 30/30 PASS
  (earned-arm tests included); new pin test `govern::gate::tests::
  every_containment_summary_carries_the_shared_vocabulary_and_authority_ones_do_not`
  → PASS (after fixing the test's own unit id to satisfy the UnitId
  pattern; the fix was to the test's fixture string, not to any
  assertion). [GREEN]
- Gate 4: `cargo build -p gauntlet-cli --no-default-features` → PASS.
  [GREEN]
- rustfmt applied to the three touched packages (diffs were exclusively
  in COR-1's new code); `cargo fmt --check` clean.
- Gate 5 (just check && just test && just lint && just deny &&
  just boundaries && just ruler): RUNNING.
- Gate 6 (e2e_full_loop_mock, both configs): PENDING.
