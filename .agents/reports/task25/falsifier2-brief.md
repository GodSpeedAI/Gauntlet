# Task 25 — blind EXECUTING falsifier brief (round 2, focused)

You are the BLIND EXECUTING falsifier, round 2, for task 25 in
/home/sprime01/projects/gauntlet. Fresh context. Same rules as every
falsifier: you ATTACK and a finding counts only when EXECUTED (RED
output, a surviving mutant, a live process, a false record) with exact
reproduction. Blindness: no builder/correction/critic history — do not
open `b1-report.md`, `b1-progress.md`, `cor1-*.md`, `cor2-*.md`,
`critic-*.md`, `IN-FLIGHT.md`, `falsifier-round1.md` (your own prior
round's file is off-limits too; you may build on its published
not-attacked list, which is quoted below). Inputs: the charter
(`charter.md` — your contract), the spec, the ruler, the delivered
tree.

**Context for focus (orchestrator-issued, mechanism-level only):** the
round-1 falsifier's findings were corrected. Your job is to try to
break THE CORRECTIONS and to execute the prior round's not-attacked
list. The corrected mechanisms and their loci:

1. **Adapter no-seam refusal teeth** —
   `crates/adapters/gauntlet-adapter-agent-cli/` (the
   `(None, Some(policy_ref))` arm and its new teeth test in
   `tests/cor2_seam_and_recovery_attacks.rs`). Attack: does the teeth
   test die when its defense is disabled (try the mutant AND variants
   — e.g. refuse-with-spawn, refuse-with-workspace-effect)? Does any
   OTHER suite keep the arm honest if the teeth test is deleted?
2. **Run lifecycle facts** — `crates/gauntlet-ports/src/state_store.rs`,
   `crates/adapters/gauntlet-adapter-state-sqlite/{state_store,encode}.rs`,
   `crates/gauntlet-app/src/control/controller.rs` (terminal sealing),
   `crates/gauntlet-app/src/report/mod.rs`. NEW append-only event
   kinds + a derived read feeding report classification. Attack:
   export determinism (AK-054 — same run exported twice, byte
   -identical, with the new events present); kill windows AROUND the
   new event appends (resume must still be exactly-once — the new
   events must not create a second settlement/recovery hole);
   classification lies by another route (attempted unit rendered
   never-attempted; an aborted/escalated run misclassified; a resumed
   run's lifecycle facts stale); a terminal that ends without a
   sealed terminal event anywhere; the fail-closed shape of the read
   (missing/corrupt lifecycle rows must not become a false basis).
3. **Group-wide child retirement** —
   `crates/adapters/gauntlet-adapter-agent-cli/src/transport.rs`.
   Attack: blast radius of the group kill — is the spawned child in
   its OWN process group (if not, the stop could reach the run itself
   — test with a child that inspects/kills group members); TERM
   -ignoring children; grandchildren; the driver-dies-of-TERM case;
   verify the bounded claim in the code/docs is the true boundary and
   nothing broader breaks (the run survives cancel; the port contract
   suites still pass).
4. **Escalation dedup** —
   `crates/adapters/gauntlet-adapter-escalation-fs/src/queue.rs` +
   contract suite. Attack: FALSE dedup — two genuinely different facts
   whose summaries coincide byte-for-byte in one run (new fact
   suppressed = a dropped hard residual, G8-adjacent); dedup across
   runs (must NEVER — try the same summary in two run ids); the
   quarantine/corruption defense vs the new scan; resolve-then
   -reobserve (must mint anew); listing totality/order with
   re-affirmed tickets; concurrent escalate+list races.
5. **The r1 not-attacked list, executed:**
   - child retirement on the remaining disposition paths: diagnostic
     exhaustion, reference-mutated abort, governance refusals
     mid-flight (/proc sweeps after each, bounded);
   - cancel-during-retry window races (fine grain);
   - `gauntlet improve`'s composition: EXECUTE a probe that a policy
     -carrying dispatch is impossible there (fail-closed), not just
     static reading;
   - the DESIGNED operator flow end to end: resolve the escalated
     ticket, resume, and judge whether the run continues to an honest
     terminal (this flow MUST work — breaking it is a finding).

## Hard rules (identical to round 1)

- Mutants restored byte-identically, sha256 both sides, in the report.
  Never mutate the repo ruler (`.wayfinder/**`, `.gauntlet/**`),
  `GAUNTLET.md`, `CURRENT_STATUS.yml`, settled `p*.rs`/`g*.rs`/
  `antigaming.rs`/`e2e_full_loop_mock.rs`. All scratch under /tmp.
- EVERY cargo/just invocation: `flock .agents/reports/task25/
  compile.lock -c '<command>'`, flock its OWN top-level command, never
  nested; `sccache --stop-server` inside the same flock'd command.
  Default threading only.
- `just ruler` at start and end; STOP if it fails.
- Bounded waits everywhere; each of the three E2E targets run at least
  twice; nothing staged or committed; probe-hygiene ls proof at the
  end.
- API-outage discipline: checkpoint the report after every executed
  attack; retry connection errors; on persistent outage write resume
  state and stop cleanly.

## Report

`.agents/reports/task25/falsifier-round2.md`, ending with VERDICT:
CONFIRM / NOT_CONFIRM, counts per severity, the executed-attacks
table, and the not-attacked list. Final message: one-paragraph signal.
