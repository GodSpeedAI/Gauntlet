# Task 21 — COR-2 correction brief (builder, micro-slice)

You are building the final micro-correction slice for plan Task 21 at
/home/sprime01/projects/gauntlet. The tree: HEAD a71eba9 + the COR-1
slice (currently STAGED in the index — leave the staging as you find
it; add your own edits with `git add` ONLY for your touched files if
you stage at all, or leave everything unstaged-on-top; do NOT commit,
do NOT unstage or revert COR-1 work). Findings + adjudication:
`.agents/reports/task21/falsifier-round2.md` (R2-F-1/2/3 — full
executed repros and fix directions) and
`.agents/reports/task21/falsifier-adjudication-r2.md`. Read both
first, plus `AGENTS.md`.

Three items, tests-first, behavior-preserving:

## D1 (R2-F-1) — ticket-id strings never reach log sinks raw

Two loci, both CLI-side; NO change to `TicketId`'s grammar or any
ports type:
- `crates/gauntlet-cli/src/escalations_cmd.rs:213-215`: the pre-parse
  error line interpolates the RAW `--ticket` argument — a newline in it
  forges arbitrary following stderr lines. Escape the argument
  (`{:?}` or equivalent control-byte-safe formatting).
- `crates/gauntlet-cli/src/operator_action.rs` §20.3 failed arm
  (`:132-137`): `detail` embeds queue/parse error Display strings that
  can carry attacker-shaped ticket ids (raw ESC confirmed). Escape the
  dynamic detail in the log line while keeping the exit/nonzero and the
  human-readable refusal output on the non-log path unchanged (the
  `Err` return feeds user-facing output too — check both consumers;
  escape in the LOG, keep or separately-escape the user-facing one, and
  record which you did).
Tests: a control-byte ticket id (a) produces no raw 0x1b byte in the
captured stderr §20.3 line, and (b) a newline-carrying `--ticket`
argument produces no second forged-looking line in stderr. Failing-first.

## D2 (R2-F-2) — the access log escapes the method like the target

`crates/adapters/gauntlet-adapter-surface-json/src/http.rs:808-814`
`access_log_line`: the method is still raw `Display`. Apply the same
escape treatment as the target (C4's fix). Behavior-preserving: the
parser's method handling and all 405/404 semantics are UNCHANGED —
this is a log-sink fix only. Test: extend the existing escape test —
a request line with control bytes in the method produces no raw 0x1b /
0x07 in the line. Failing-first.

## D3 (R2-F-3) — bounded backoff on the transient accept branch

`crates/adapters/gauntlet-adapter-surface-json/src/http.rs:199-207`:
the transient arm currently `continue`s immediately — persistent EMFILE
spins at 100% CPU flooding ~37 MB/s of "continuing" lines (r2's
executed repro, RLIMIT_NOFILE=5). Add a bounded backoff: a short
constant sleep (your choice in the 10–100 ms range; keep it a named
constant with a comment tying it to this finding) before continuing on
a transient error. Classification semantics UNCHANGED (transient
continues, fatal breaks; "continuing" still prints per transient
event — the sleep bounds its rate). The C3 injected-sequence tests
must stay green; if their runtime grows materially, note it.
Test: extend the injected-sequence test (or add one) asserting N
consecutive transient errors take at least N × (backoff − small
slack) — a timing assertion with generous slack so it is not flaky;
mark it plainly if you must make it tolerant. Record honestly if a
timing assertion proves too flaky for CI and pin the behavior by the
constant + code review instead.

## Gates (green on your final tree; one compiler at a time)

```
flock .agents/reports/task21/compile.lock -c '<cmd> && sccache --stop-server'
```

1. `cargo nextest run -p gauntlet-cli --features surfaces --test operator_actions_no_bypass` (8/8 today)
2. `cargo nextest run -p gauntlet-adapter-surface-json` (10/10 today)
3. `cargo build -p gauntlet-cli --no-default-features`
4. `cargo nextest run -p gauntlet-cli [--features surfaces] --test e2e_full_loop_mock` (both configs, 12/12)
5. `just lint && just boundaries && just ruler`
(Do NOT run the full `just test` — the orchestrator runs the closure
battery personally.)

## Guardrails

- Do not touch `.wayfinder/answer-key.md`, `.gauntlet/reference.lock`,
  `.agents/specs/`, `.jolli/`, either `cli_help*.txt` snapshot, or any
  file outside the three items.
- Do not commit. Do not unstage or modify the staged COR-1 content.
- No new dependencies. No behavior change beyond the three fixes.
- Frozen artifacts stay diff-clean vs HEAD.

## Result contract

`.agents/reports/task21/cor2-report.md`: per item — what changed
(file:line), failing-first evidence, gate evidence (post-last-edit),
deviations. Also maintain `cor2-progress.md` (STARTED/DONE per item).
Your final message: completion statement + per-item status + report
path only.
