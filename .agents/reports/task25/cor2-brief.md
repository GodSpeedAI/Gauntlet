# Task 25 — COR-2 brief (corrections round 2)

You are the corrections builder for task 25's second corrections round
in /home/sprime01/projects/gauntlet. The blind EXECUTING falsifier
round 1 returned NOT_CONFIRM with 0 BLOCKING / 2 MAJOR / 2 minor / 3
informational across 12 executed attacks
(`.agents/reports/task25/falsifier-round1.md` — read it FIRST; it is
your findings ledger, with verbatim reproductions). Orchestrator
dispositions below; every fix is RED-FIRST.

## Dispositions (orchestrator-owned, binding)

- **F-R1 (MAJOR — FIX). The no-seam fail-closed arm has no executed
  evidence.** `CliAgentRunner`'s `(None, Some(policy_ref))` →
  `AdmissionRefused` arm survives its own disabling mutant across the
  entire battery (falsifier attack 1b: three E2E targets, p8, adapter
  package 60/60 all green with the arm disabled). Fix: a TEETH test at
  the adapter level — construct the runner WITHOUT the containment
  seam, drive an invoke whose dispatch carries a sandbox policy ref,
  and assert the typed `AdmissionRefused` (and that nothing spawns /
  no workspace effect lands). RED-FIRST: apply the falsifier's exact
  mutant (arm → return argv verbatim), show the NEW test RED and state
  plainly that every pre-existing suite stays green under the same
  mutant (that is the gap), restore byte-identically (sha256 both
  sides), then green on the real tree. Home: the adapter package's own
  test surface (it needs no bwrap — the refusal must fire before any
  spawn). The arm's lib.rs doc claim then has its binding evidence;
  say so in the doc comment (one line, "teeth: <test name>").
- **F-R4 (MAJOR — FIX). The report's classification basis lies.** On
  the budget-terminal path the report JSON says units are
  `configured-but-untested` with basis "the latest checkpoint names
  the unit open — configured into the run, **never attempted on
  record**" while the run's export holds `invocation_admitted` /
  `invocation_terminal state=timed_out` records for those units. Fix:
  the classification basis must be RECORD-DERIVED — a unit with any
  invocation attempt in the durable record is never described as
  never-attempted; render the honest basis (attempted-and-timed-out /
  attempted-and-escalated / attempted-and-open, from the records).
  Also fold in the informational items: (a) `terminal_state:null` in
  the report JSON while stdout prints the terminal — the report must
  carry the real terminal; (c) the generic open-unit basis text that
  renders for operator-routed units — give operator-routed closure its
  own honest basis. RED-FIRST: extend `e2e_budget` to parse the run's
  printed report JSON and assert the units array's classification
  bases against the durable export (AK-951: attempted/timed-out basis
  present, the string "never attempted on record" ABSENT; terminal
  field consistent with stdout). Show that assertion RED on the
  current tree (the falsifier's world shape), then fix the report
  generator (`crates/gauntlet-cli/src/report*.rs` — find the real
  locus), then green. Regression: the mock E2E's report legs
  (`e2e_full_loop_mock` asserts report categories from durable
  records) must stay green — a report change that breaks them is a
  defect in the change.
- **F-R3 (minor — FIX). The recording-refusal cancel can leak a
  just-spawned wrapped child.** Executed under falsifier mutant 2b:
  the invoke path's cancel-for-recovery arm (post-spawn admission
  refusal) left a live bwrap pair + in-sandbox shell after exit;
  `--die-with-parent` does not reap it. Fix: the cancel-for-recovery
  arm must actually RETIRE the spawned child before returning the
  refusal — through the transport's own stop path (SIGTERM → bounded
  grace → SIGKILL + reap, the ProcessChildTransport mechanics), within
  the bounded cancel span, never an unwaited fire-and-forget signal.
  RED-FIRST: an adapter-level test that triggers a post-spawn refusal
  (the no-tier-registration shape or an equivalent constructed refusal)
  and asserts — bounded poll on process liveness — that no child
  outlives the call. Demonstrate it RED against the current arm (a
  sleep-guarded child survives), then fix, then green.
- **F-R5 (minor — FIX the ticket dedup; PIN the re-entry semantics;
  NO flow redesign).** `gauntlet resume` of a non-settled (escalated)
  terminal run re-enters the loop, re-spends a booked round, rewrites
  the terminal (escalated → budget_exhausted), and mints a DUPLICATE
  open AK-953 ticket (observed in two independent worlds; the F9
  one-ticket ledger is in-memory per-pass). Dispositions:
  1. **Ticket dedup (required):** open escalation tickets for the same
     unit + fact class are identified durably (per-run), so a resumed
     pass re-observing the same unresolved fact does NOT mint a second
     open ticket — it re-affirms the existing one (and the escalation
     listing shows ONE). RED-first: a leg/test that resumes an
     escalated world and asserts exactly one open AK-953 ticket
     (RED: two).
  2. **Re-entry semantics (pin, don't redesign):** resume-after-resolve
     is the designed operator flow (task21) and must keep working;
     resume-without-resolve re-spending is budget-honest (spent rounds
     are recorded). The objective defect is SILENT rewrite: the
     re-terminaling must be visible — assert in the same leg that the
     run's report/export reflects the re-entry honestly (the new
     terminal is the run's real final state and the re-spent round is
     in the checkpoint facts). Whatever the code already does that is
     honest, PIN it; if the rewrite is silent anywhere a reader can
     see, make it visible in the report (smallest change).
  3. Do NOT redesign the escalation flow in this task; the deeper
     question (terminal-refusal posture, ticket↔unit binding as a
     ports revision) is a recorded residual, not a change.
- **Informational (a) — RECORD, no fix:** the resumed pass minted an
  invocation id identical to the run id (id-space collision across
  kinds, seeded-generator shape). Record in cor2-report.md +
  e2e-real.md's residual section; the deterministic generator is
  fixture/test surface, the real IdGenerator is ULID; if a one-line
  typed namespace fix is NOT trivial, record it.

## Constraints (unchanged, binding)

- Never touch: repo-root `.wayfinder/**`, repo `.gauntlet/**`,
  repo-root `GAUNTLET.md`, settled `p*.rs`/`g*.rs`/`antigaming.rs`/
  `e2e_full_loop_mock.rs` (RUN as regression, never edit),
  `.agents/CURRENT_STATUS.yml`. The fixture under
  `tests/fixtures/target-real/` and its docs stay as delivered unless
  a fix requires a fixture change (prefer not).
- No new dependencies; no network; no sleep-and-hope (bounded polls,
  typed failures). No shell-out to cargo outside the flock.
- RED-first evidence verbatim in your report for F-R1, F-R4, F-R3, and
  the F-R5 dedup leg.
- Checkpoint: `cor2-progress.md`. Contract: `cor2-report.md` (mission
  echo, per-finding disposition + evidence, deliverable paths +
  sha256, command ledger, deviations, residuals for the record).
  Final message is a signal only.
- Nothing staged or committed.

## Battery (all green before reporting)

```bash
just ruler
just check-package gauntlet-cli
just check-package gauntlet-app
cargo nextest run -p gauntlet-adapter-agent-cli
cargo nextest run --test e2e_full_loop_real_runner
cargo nextest run --test e2e_recovery
cargo nextest run --test e2e_budget
cargo nextest run --test e2e_full_loop_mock
cargo nextest run --test p5_hard_gate
cargo nextest run --test p7_recovery --test p6_budget_control
cargo nextest run --test p4_corrective_loop --test p10_no_false_learning --test p9_self_improvement_containment
cargo nextest run --test operator_actions_no_bypass --test escalations_cli
just lint
just boundaries
```

(`escalations_cli` / `operator_actions_no_bypass` join because of the
F-R5 dedup surface.)

## Operational addenda (binding — identical to prior rounds)

- ONE compile at a time: EVERY cargo/just invocation is
  `flock .agents/reports/task25/compile.lock -c '<command>'`, flock
  always its OWN top-level command, never nested; `sccache
  --stop-server` inside the same flock'd command after the cargo
  command. Default threading only (WSL2, ~1.6 GiB).
- `just ruler` before and after every phase; STOP if it fails.
- Probe hygiene: nothing planted under repo `.gauntlet/policies/`;
  `ls` proof in the report. API-outage discipline: checkpoint the
  report file after every landed fix; retry connection errors a few
  times; on persistent outage write resume state and stop cleanly.
