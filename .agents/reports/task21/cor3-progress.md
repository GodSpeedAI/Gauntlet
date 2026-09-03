# Task 21 — COR-3 progress (builder log, declare-first)

Tree: HEAD a71eba9 + staged COR-1 + unstaged COR-2 (staging untouched;
COR-3 edits land unstaged on top; nothing is committed by this slice).
Commission: `.agents/reports/task21/cor3-brief.md` — close the CLASS
(one escaping discipline across the escalations/operator-action command
family), not the R3 loci; enumeration-free invariant teeth; failing-first.

- T1 (declare): STARTED+DONE 2026-09-02 — this file.
- T2 (invariant test): STARTED 2026-09-02 — new enumeration-free
  integration test in `operator_actions_no_bypass.rs` seeding hostile
  records through the queue's own substrate + argv (ESC ticket id,
  newline operator identity through the sanctioned resolve, quarantined
  stem with a forged §20.3 line, hostile dispatcher subcommand/argument)
  and driving list / refused / failed / accepted(fresh+re-entry) /
  dispatcher paths; asserts over the FULL captured stderr+stdout:
  (a) zero raw bytes < 0x20 except \n and \t, (b) the `^operator-action:`
  stderr-line count equals the actions actually driven, (c) every hostile
  payload lands in its escaped form.
  RED 2026-09-02 — `cor3-red-run.log`: 11 violations — (a) 4 raw 0x1b
  bytes ([accepted-esc] stdout+stderr, [list] stdout), (b) 6
  `^operator-action:` stderr lines for 4 driven actions (forged lines
  from [dispatcher] + [list-unknown-arg]), (c) 7 missing-escape
  violations. Failing-first proven.
- T3 (the family fix): STARTED 2026-09-02.
  DONE 2026-09-02 — `log_quote` (Rust Debug escaping, byte-identical on
  clean text) added in `operator_action.rs`; applied to the §20.3 arms'
  tickets fields + `escalations_cmd.rs` dispatcher unknown-subcommand,
  list unknown-argument, quarantined stem, list ticket/operator fields,
  CLI acceptance ticket/resolved fields.
  Test-expectation correction (not a source issue): the FAILED leg's
  `tickets=` field is honestly EMPTY (queue read failed before any
  durable read named the ticket — COR-1's no-invention shape, visible in
  the RED run's own (b) listing); the (c) check now requires the ticket
  id via the read-failure detail instead.
- T4 (GREEN + gates 1–5): GREEN 2026-09-02 — `cor3-green-run.log`:
  invariant test PASS. Gates (post-last-edit, one compiler at a time,
  under the compile lock):
  1. operator_actions_no_bypass: 13/13 PASS (`cor3-gate1.log`)
  2. cli_surface + escalations_cli: 7/7 PASS (`cor3-gate2.log`)
  3. gauntlet-adapter-surface-json: 11/11 PASS (`cor3-gate3.log`)
  4. build -p gauntlet-cli --no-default-features: Finished (`cor3-gate4.log`)
  5. just lint (clippy -D warnings + fmt) && just boundaries (228/228)
     && just ruler (PASS, frozen digests match) (`cor3-gate5.log`)
  Gate-exit note: gates 1/3 wrapper exits were 2 — the `sccache
  --stop-server` tail inside the flock'd command could not connect
  (already stopped by a prior gate): the known-harmless pattern recorded
  in the falsifier round-3 provenance; the gate commands themselves
  passed (Summary lines cited in the report).
- T5 (cor3-report.md): DONE 2026-09-02 — audit table, RED/GREEN
  evidence, gate evidence, deviations.

## r4 addendum (tests-only; closes falsifier r4's single minor)

- T6 (boundary drives): STARTED 2026-09-02 — the invariant test's drive
  set grew by the three hostile-id READ-failure shapes (missing ESC id,
  ESC id claimed cross-wired → TicketIdentityMismatch, corrupt record
  under an ESC stem).
  RED-first 2026-09-02 — the drives ran under the blanket invariant
  BEFORE the assertions were fixed: `cor3-r4-red-run.log` shows 4 raw
  0x1b violations, all in the boundary captures' human failure lines
  (two in [boundary-crosswired] — requested + claimed ids); the §20.3
  count held at 7 == 7. The finding reproduced exactly.
- T7 (boundary tooth + carve-out): DONE 2026-09-02 — the boundary
  captures moved OUT of the blanket set; a dedicated tooth pins the
  recorded COR-2 carve-out BY CONSTRUCTION (raw bytes confined to the
  ONE human failure line; stderr exactly two lines; §20.3 line escaped
  and greppable; ids verbatim in the human line; exit 1; (b) count spans
  the boundary drives); the doc comment declares the carve-out and the
  widening-must-fail contract. GREEN: `cor3-r4-green-run.log` (1 passed).
- T8 (r4 gates): DONE 2026-09-02 — (1) no-bypass suite (surfaces) 13/13
  `cor3-r4-gate1.log`; (2) cli_surface + escalations_cli (default) 7/7
  `cor3-r4-gate2.log`; (3) `just lint` PASS `cor3-r4-gate3.log` (after
  one `cargo fmt` whitespace fix, tests-only; nextest gates re-run
  post-fmt on the final tree). No source change; nothing outside the
  test file; no commit.
- T9 (report addendum): DONE 2026-09-02 — "r4 ADDENDUM" section in
  `cor3-report.md` (RED-first evidence, tooth description, gates).
