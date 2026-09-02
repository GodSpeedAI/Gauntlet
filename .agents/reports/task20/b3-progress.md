# B3 progress — E2E fixture project + e2e_full_loop_mock (declare-first checkpoints)

STARTED 2026-08-31 — context reading (AGENTS.md, charter, b3-brief, b1/b2a/b2 reports,
compose.rs, context.rs, run_cmd.rs, mock adapter/scenario, controller verification path,
verifier adapters, doctor checks). No source files touched yet.

Plan:
- Item 1: tests/fixtures/target/ (GAUNTLET.md runners.default=mock-agent, roles tree,
  comparison contract, .wayfinder reference.yaml + answer-key.md with 2-3 blocking local
  units, check script, .gauntlet/reference.lock for the dry-run leg).
- Item 2: crates/gauntlet-cli/tests/e2e_full_loop_mock.rs (run leg, status/inspect/
  export/verify legs, dry-run leg, lying-builder discrimination tooth).
- Gates: flock + sccache --stop-server for every cargo/just call; background+poll.

FINDING UNDER INVESTIGATION (pre-implementation, from source reading):
The composed verification path (controller.rs process_completion) writes the FIRST
observation facet's evidence bytes (a facet_evidence@v1 JSON document) at the unit's
evidence_required.locator under the canonical workspace root, records the production
binding over those bytes, and then dispatches the composed ladder. Every shipped
deterministic rung expects a DIFFERENT artifact at that locator:
- FileDigest rung: two-line sha256 manifest (sha256:<64hex> + relative path)
- CommandExitCode rung: two-line transcript (command + integer exit)
- TestReport rung: JSON object with whole-number `tests` and `failures`
A facet_evidence@v1 JSON satisfies none of these shapes -> VerifierError::InstrumentFailed
-> LadderError::RungFailed -> ControlError::VerificationRefused -> the whole run errors.
No requirement kind avoids this (unsupported kinds hit NoRungCanDecide, also a run error).
If confirmed empirically, this is a composition defect to record per the brief (no CLI
source patches from B3). Empirical check follows once the fixture + binary are ready.

STARTED Item 1 — fixture target project under tests/fixtures/target/.
Design constraints discovered (recorded for b3-report):
- The shipped none posture admits ONLY contained-tier, critic-authority dispatches
  (govern/gate.rs admit(): NoBackendForTier for local+; none_backend() is
  "critic-exercisable"). The brief's "tier local" premise is contradicted by the
  gate; fixture units are tier contained / authority critic. DEVIATION recorded.
- GAUNTLET_MOCK_SCENARIO names ONE scenario for every dispatch (builder AND critic
  fall back to the same token), so the fixture carries its own scenario files with
  attempted: [] (the controller's empty-attempted fallback processes the flight's
  OWN unit; a non-empty attempted list naming another unit is mismatch-skipped).

DONE Item 1 — fixture target project LANDED and validated through the real binary:
- tests/fixtures/target/{GAUNTLET.md,.wayfinder/reference.yaml,.wayfinder/answer-key.md,
  .agents/roles/{critic,builder}.md,.gauntlet/comparison-contract.yaml,
  .gauntlet/reference.lock,src/gadget.rs,scripts/check.sh}
- tests/fixtures/mock_runner_scenarios/{e2e-happy.yaml,e2e-lying.yaml} (scenario fixtures
  must live where the mock adapter's load_scenario looks; DEVIATION vs "tests/fixtures/target/**" recorded)
- Probe evidence (scratch copies, never the fixture): doctor exit OK; `reference validate
  --path .wayfinder/answer-key.md` => OK: 3 units; `run --dry-run` => exit 0 (3 dispatches
  planned, lock verified, zero writes).

DONE (probe) Item 2 — EMPIRICAL FINDINGS, all reproducible, logs under
.agents/reports/task20/b3-probe-*.log:
1. b3-probe-run-stall-escalated.log: with the wall clock (the composition ALWAYS uses
   SystemClock; compose.rs never consults the Annex-E GAUNTLET_CLOCK hook), every mock
   dispatch is stall-caught (the mock reports last_event_at = fixed epoch 1_700_000_000),
   retried, and the run ends `escalated` ("AK-901 timed out twice"). Workaround via env:
   GAUNTLET_STALL_TIMEOUT_MS=200000000000.
2. b3-probe-run-critic-isolation-refused.log: with max_critics >= 1 the governed comparison
   refuses at the critic dispatch: "a model critic requires process-or-stronger runner
   isolation; isolation:none cannot establish fresh-context containment". The composition
   fronts no critic runner the fixture can select (prime/cli need external binaries).
   Workaround via config: review.max_critics: 0 (count-truncated review, honest records).
3. b3-probe-run-verification-rung-refused.log (THE BLOCKER): with both workarounds, the
   run dies at the FIRST completed dispatch's verification:
   "the command-exit-code rung failed as an instrument ... the transcript at
   evidence/check-transcript.txt is not the two-line command/exit artifact".
   Mechanism: process_completion writes the FIRST observation facet's facet_evidence@v1
   JSON at the unit's evidence_required.locator, records the production binding over those
   bytes, and dispatches B2's composed_ladder — whose three rungs expect a two-line sha256
   manifest (File), a two-line command/exit transcript (Log), or a tests/failures JSON
   (TestReport). facet_evidence@v1 satisfies none; InstrumentFailed => RungFailed =>
   ControlError::VerificationRefused => the run errors. No fixture configuration can settle
   a unit: file/log/test_report kinds => RungFailed; other kinds => NoRungCanDecide (same
   run error); no facets => composition refuses; all-failed observers => INCONCLUSIVE
   forever. `verify --run` leg shows the same refusal (b3-report.md quotes it).
4. status/inspect/export/dry-run legs WORK (record-derived) — but the brief's legs are
   specified against the SETTLED run, which items 1-3 make unreachable.

DECISION (per brief): the composition defect blocks the run leg, hence every settlement-
dependent leg and the discrimination tooth. B3 does NOT patch CLI source (brief boundary),
does NOT write a partial e2e_full_loop_mock.rs that would either fail or freeze the
defective behavior as expected, and STOPS after recording. Fixture (Item 1) is complete
and green; correction round can resume Item 2 from this state.

DONE Gate battery (final tree): ruler 0 (before+after) · doctor 0 · check 0 ·
repo-root dry-run gate exit 0 zero-writes (b3-dryrun-root.log) · boundaries 0
(228/228) · deny 0 · lint 0 · test 1319/1319 PASS (b3-gate-test2.log). First
battery run had 3 known-class load-flaky live-process failures
(gauntlet-adapter-agent-cli), 56/56 in isolation, 1319/1319 on re-run.

DONE b3-report.md written. SLICE STATE: Item 1 landed and validated; Item 2
blocked by composition defects (findings 1-4 in b3-report.md), recorded and
stopped per the brief's blocked path. No CLI source, control logic, spec, or
ruler file touched. Working tree holds B3's files uncommitted for the
orchestrator.

DEVIATION 4 (post-battery catch): .gitignore gained one scoped line
`!tests/fixtures/target/` — the `**/target/` build-artifact rule was silently
ignoring the ENTIRE fixture tree (git check-ignore confirmed); without the
negation the fixture could never be committed. Recorded in b3-report.md.
Final tree check: tests/fixtures/target/ now visible to git; all gates remain
green (no Rust or gate-relevant file changed by this edit).
