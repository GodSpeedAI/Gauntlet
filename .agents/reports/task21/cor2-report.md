# Task 21 — COR-2 report (builder)

Tree: HEAD a71eba9 + the staged COR-1 slice (untouched; staging state as
found). The COR-2 delta is UNSTAGED on top and touches exactly four code
files — `crates/gauntlet-cli/src/escalations_cmd.rs`,
`crates/gauntlet-cli/src/operator_action.rs`,
`crates/gauntlet-cli/tests/operator_actions_no_bypass.rs`,
`crates/adapters/gauntlet-adapter-surface-json/src/http.rs` — plus this
report and `cor2-progress.md`. Nothing committed; no manifest touched; no
new dependencies; `.wayfinder/`, `.gauntlet/`, `.agents/specs/`, `.jolli/`
and both `cli_help*.txt` snapshots diff-clean vs HEAD. Every cargo/just
invocation ran under
`flock .agents/reports/task21/compile.lock -c '<cmd> && sccache --stop-server'`.

## D1 (R2-F-1) — ticket-id strings never reach log sinks raw — DONE

What changed (two loci; no `TicketId` grammar or ports change):

- `crates/gauntlet-cli/src/escalations_cmd.rs:213-219` — the pre-parse
  refusal now prints the raw `--ticket` argument DEBUG-ESCAPED
  (`{ticket:?}`), so a newline in it cannot forge following stderr lines.
- `crates/gauntlet-cli/src/operator_action.rs:132-146` — the §20.3 FAILED
  arm prints the dynamic `detail` DEBUG-ESCAPED (`{detail:?}`), so queue/
  store error Displays carrying attacker-shaped ticket ids (raw ESC
  proven by r2) land escaped in the audit line, like the `{:?}`-escaped
  operator identity beside them.

Consumer decision (recorded, per the brief's latitude): the escape is
applied ONLY in the log sink. The typed `Err` return's two non-log
consumers stay VERBATIM: the CLI's human-readable failure line
(`escalations_cmd.rs`, `Err(detail)` arm) keeps raw Display, and the JSON
surface's 500 `action_failed` body (`durable_surface.rs:510-513`) keeps
the verbatim detail (control bytes are escaped by the JSON string
encoding at the transport). Exit codes and the refusal/nonzero posture
are unchanged.

Failing-first evidence (both RED for the executed expected reason, on the
pre-fix tree):

- `a_control_byte_ticket_id_lands_escaped_in_the_failed_action_log_line`
  (new, `operator_actions_no_bypass.rs:1327`): panicked with
  `no raw escape byte reaches the §20.3 line: "operator-action: ...
  outcome=failed ... — escalation ticket \u{1b}[31mred-ticket is not a
  valid record: No such file or directory (os error 2)"` — the raw 0x1b
  in the §20.3 line, exactly r2's locus 2.
- `a_newline_carrying_ticket_argument_cannot_forge_a_following_log_line`
  (new, `:1369`): panicked with the forged line PRESENT:
  `"gauntlet escalations resolve: x\noperator-action: action=resolve
  run_id=FORGEDRUN outcome=accepted tickets=FAKE-TKT is not a usable
  ticket id"` — r2's locus 1 verbatim.

## D2 (R2-F-2) — the access log escapes the method like the target — DONE

What changed:

- `crates/adapters/gauntlet-adapter-surface-json/src/http.rs:825-833` —
  `access_log_line` now formats `{method:?}` beside the already-escaped
  `{target:?}` (C4's shape). Log-sink only: `parse_head`'s method
  handling and all 405/404 semantics are untouched; the plain line reads
  `surface: "GET" "/api/v1/state" -> 200`.

Failing-first evidence: the existing escape test, extended with the
method probes (`http.rs:886-916`), RED pre-fix with
`no raw control byte from the method reaches the log: "surface:
\u{1b}[31mFORGED\u{7} \"/api/v1/state\" -> 405"` — r2's executed repro
shape (ESC+BEL through the method).

## D3 (R2-F-3) — bounded backoff on the transient accept branch — DONE

What changed:

- `crates/adapters/gauntlet-adapter-surface-json/src/http.rs:516-523` —
  named constant `TRANSIENT_ACCEPT_BACKOFF: Duration =
  Duration::from_millis(50)`, documented against R2-F-3 (the executed
  persistent-EMFILE repro: ~37 MB/s of "continuing" lines at 100% of a
  core).
- `http.rs:200-211` — the transient branch sleeps the constant before
  `continue`. Classification semantics UNCHANGED (transient continues,
  fatal breaks; the typed message still prints per transient event — the
  sleep bounds its rate). Module doc updated (line 10) to keep doc truth.
- New test `consecutive_transient_accept_errors_cannot_hot_spin_the_loop`
  (`http.rs:1115-1160`): 4 consecutive EMFILE injects + one EBADF;
  asserts elapsed >= 4 × (backoff − 20 ms) = 120 ms AND that the loop
  consumed exactly through the fatal error (so the timing cannot pass by
  exiting early). Generous-slack LOWER bound only: `thread::sleep` is
  at-least semantics, so the floor is not flake-prone (fixed code
  measures ~200 ms); no upper-bound assertion was added. The test runs
  as a plain (non-ignored, non-marked) test.

Failing-first evidence: RED pre-fix with the hot-spin reproduced —
`4 transient continues cost at least the bounded backoff (120ms):
40.572µs`.

C3 injected-sequence tests: still green; the behavioral test grew by
~0.1 s (two 50 ms sleeps), total 0.117 s — not material. The new timing
test runs ~0.22 s.

## Gates (green on the final tree, post-last-edit; one compiler at a time)

1. `cargo nextest run -p gauntlet-cli --features surfaces --test
   operator_actions_no_bypass` — 10/10 PASS (8 pre-existing + 2 new D1;
   re-run after the fmt fix noted below).
2. `cargo nextest run -p gauntlet-adapter-surface-json` — 11/11 PASS
   (10 pre-existing, one extended by D2, + 1 new D3).
3. `cargo build -p gauntlet-cli --no-default-features` — Finished, no
   warnings.
4. `cargo nextest run -p gauntlet-cli --test e2e_full_loop_mock` —
   12/12 PASS default AND 12/12 PASS `--features surfaces`.
5. `just lint && just boundaries && just ruler` — clippy `-D warnings`
   clean, `cargo fmt --check` clean, arch tests 228/228 PASS (incl. the
   hardcoded-literal and single-writer scans), ruler PASS (answer key and
   manifest digests match the frozen lock).

The full `just test` was NOT run (reserved to the orchestrator's closure
battery, per the brief).

## D1 addendum (operator-assigned, same contract) — the pre-parse `--unit` refusal + same-function sweep — DONE

What changed (same file as D1 locus 1; no other file touched by the
addendum):

- `crates/gauntlet-cli/src/escalations_cmd.rs:226-230` — the pre-parse
  `--unit` refusal now prints the raw argument DEBUG-ESCAPED
  (`{unit:?}`), closing the identical forged-line class D1 fixed for
  `--ticket` one arm away.
- Same-function sweep finding, fixed: `escalations_cmd.rs:198-204` — the
  `unknown argument '{other}'` arm interpolated the raw positional token
  verbatim; now `{other:?}` (DEBUG-ESCAPED at the log sink, same
  posture). This was the only OTHER raw-argument interpolation in
  `operator_action`.

Sweep record for the rest of the function (no action, class closed):
`--{name} given twice` / `--{name} requires a value` interpolate only the
fixed flag names; the missing-required line interpolates nothing; the
operator-identity line's error (`OperatorIdentityError::Blank`) embeds no
input (fixed prose, verified at `gauntlet-app/src/settle/mod.rs:210-216`);
the config-error line carries a typed `resolve_context` error, not a raw
argument. Out-of-function same-class loci RECORDED, not fixed (outside
the locus mandate): the `escalations` dispatcher's `unknown subcommand
'{other}'` (`escalations_cmd.rs:34`) and `escalations list`'s `unknown
argument '{other}'` (`:59`) — both one-line `{other:?}` fixes of the
same shape if the orchestrator wants the class closed file-wide.

Failing-first evidence (both RED for the executed expected reason, on
the pre-fix tree):

- `a_control_byte_unit_argument_lands_escaped_and_forges_nothing` (new,
  `operator_actions_no_bypass.rs:1404`, the mandated `--unit` extension
  of the D1 pattern): panicked with
  `no raw escape byte reaches the refusal's stderr: "gauntlet escalations
  resolve: \u{1b}[31mAK-901\noperator-action: action=resolve run_id=
  FORGEDRUN outcome=accepted tickets=FAKE-TKT is not a valid unit id"` —
  raw ESC and the forged §20.3-shaped second line, the identical class.
- `a_newline_carrying_unknown_argument_cannot_forge_a_following_log_line`
  (new, `:1447`, teeth for the sweep finding): panicked with the forged
  line PRESENT inside `unknown argument 'x\noperator-action: ...'`.

Gate evidence (post-addendum-fix, final tree):

1. Gate 1 `operator_actions_no_bypass` (surfaces): **12/12 PASS** (10 +
   the 2 addendum tests).
2. Gate 2 `gauntlet-adapter-surface-json`: **11/11 PASS** (untouched by
   the addendum; the run's nonzero shell exit was the known harmless
   sccache client shutdown timeout AFTER the passing run, the same noise
   r2 recorded).
5. Gate 5 `just lint && just boundaries && just ruler`: clippy `-D
   warnings` clean, `cargo fmt --check` clean, arch tests **228/228
   PASS**, ruler **PASS** (exit 0).

## Deviations / recorded notes

- First gate-5 run failed at `cargo fmt --check` (one rustfmt wrapping in
  a new D1 test); fixed to rustfmt's exact spelling, gate 5 and gate 1
  re-run green afterward. No other intermediate red.
- Adjacent locus (SUPERSEDED by the D1 addendum above): the pre-parse
  `--unit` refusal was recorded here as the same forged-line class left
  outside the original three-item contract; the operator-assigned
  addendum fixed it (plus the same-function `unknown argument` arm) with
  the failing-first tests and gate re-runs documented above.
- D1's non-log consumers kept verbatim (decision recorded above under
  D1); the JSON surface's 500 body is the one place an attacker-shaped
  ticket id still travels outside a debug-escaped sink, transported
  JSON-escaped.
- Sccache's client printed timeout noise at shutdown in two early runs
  (harmless, also observed by r2); all commands completed under the
  lock.
