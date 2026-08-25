# Task 12 — Builder slice B4 brief — proofs p4/p6 + teeth (final slice)

You are a builder agent in /home/sprime01/projects/gauntlet. Read `AGENTS.md`
first; it binds you. Then `.agents/reports/task12/builder-brief-b3.md` and
`.agents/reports/task12/builder-b3.md` (the controller you are proving),
`.agents/reports/task12/recon-answerkey.md` (AK-037/038/039/051/052
verbatim), and the existing proof patterns:
`crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs` (Scratch/Fixture
composition pattern, authority-routed produce, ladder through the
composition seam) and `p3_real_observation.rs` (mock scenario + real
observers over the same workspace).

Task 12 plan gate (must pass exactly):

```
cargo nextest run --test p4_corrective_loop --test p6_budget_control && \
cargo nextest run -p gauntlet-app control::
```

Hard rules: same as previous slices (no frozen/operator file edits —
`.wayfinder/**`, `.agents/specs/**`, `.agents/plans/**`, `AGENTS.md`,
`justfile`, `deny.toml`, `.agents/CURRENT_STATUS.md`, prereg files, existing
task12 recon/adjudication/brief/report files, `.agents/prompts/**`,
`.tmp/**`, `.jolli/**`; no commits; single-compiler lock; package-scoped
recipes first; fail-first; checkpoint entry appended to
`.agents/reports/task12/builder-b4-checkpoints.md` after EACH item; result
contract `.agents/reports/task12/builder-b4.md`).

## Item 1 — `crates/gauntlet-cli/tests/p4_corrective_loop.rs` (P4, AK-038)

Composition-root proof of the corrective loop through the REAL controller:

- Fixture answer key (scratch tree, frozen through the reference provider
  path) containing a Unit whose `evidence_required` is decidable by a REAL
  deterministic verifier (test-report rung over the workspace) — follow the
  p12 ladder pattern; the controller receives the pre-built Ladder through
  the CLI dev-dep composition seam.
- Iteration 1: a builder that produces a DEFECTIVE artifact (lying-builder
  mock scenario or scripted defect — prefer the real `MockAgentRunner`
  scenario fixture; if one runner instance cannot script fail-then-fix, a
  proof-local scripted `AgentRunner` implementing the port is acceptable
  AND MUST be documented in the report with why the mock could not serve).
- REAL observation exposes the defect (CommandObserver running the failing
  check over the same workspace), comparison emits the residual,
  verification FAILs/records it.
- Assert the controller ROUTED a corrective iteration: durable
  `CorrectionSelection` read back from the store with rejected alternatives
  + reasons (AK-037 at the composed level); re-dispatch prompt carries
  residual TEXT ONLY (not critic trajectory) if the scenario surfaces it.
- Iteration 2: the corrected builder closes the defect — verification
  outcome PASS through the real chain.
- Boundary honesty: settlement COMMIT is Task 13 — assert the settlement
  precondition (or its honest absence) but do NOT call commit_settlement.
- Assert both gated writes flowed through the authorities (no direct store
  write path executed — e.g. a counting wrapper, or assert the authorities'
  typed refusals trigger for a forged attempt).

## Item 2 — `crates/gauntlet-cli/tests/p6_budget_control.rs` (P6, AK-051/052)

Composition-root proof of budget control:

- Deliberately NON-CONVERGING task: the same residual recurs every
  iteration (script it), budget tight (e.g. `max_rounds` small, reserve
  15%).
- Assert: run terminates at the declared limit with terminal
  `budget_exhausted`; the recovery reserve is INTACT — forward spend never
  exceeded the forward ceiling (`limit × (100−pct)/100`); every recorded
  invocation has a corresponding meter reservation (AK-052 — count
  RecordingAgentRunner rows vs reservations); a checkpoint exists for every
  iteration boundary; final state exportable.
- Design note: with `nonconvergence_n` default 3 and a small `max_rounds`,
  decide and document which terminus fires (exhaustion vs diagnosis) and
  assert the OTHER path separately is already covered in-crate
  (`control::diagnosis` tests, AK-039) — both behaviors must be pinned,
  neither vacuously.

## Item 3 — Teeth (orchestrator will re-run them; you prove each bites)

Record in `.agents/reports/task12/teeth.md` (append; format per the file if
it exists): for each, break the mechanism → run the named test/probe →
observe the FAILURE → revert byte-identical (`git diff` empty for tracked;
manual inverse for untracked) → re-run green:

- T1: neuter the meter's reserve refusal (or the controller's summed-slice
  re-consult) → p6 must FAIL (spend would pass the reserve).
- T2: delete the iteration-boundary `verify_unchanged` call → the boundary
  abort test (in-crate or composed mid-run-edit test) must FAIL.
- T3: delete the per-tick answer-key re-read (cache once) → the mid-run
  operator-edit test must FAIL.
- T4: delete the reservation requirement on the dispatch path (or make
  reserve a no-op pass-through) → p6's every-dispatch-reserved assertion
  must FAIL.
- T5 (if p4 uses the correction-record path): delete the
  `record_correction_selection` call → p4's alternatives read-back must
  FAIL.
- T6: observer deadline enforcement removal → the already-expired-deadline
  refusal test must FAIL.

## Item 4 — Gates

```
cargo nextest run --test p4_corrective_loop --test p6_budget_control
cargo nextest run -p gauntlet-app control::
just check-package gauntlet-cli && just test-package gauntlet-cli
just check && just test && just lint && just deny && just boundaries && just ruler
```

Record exact counts. In the report, list the REAL locator spellings you
created (CLI test targets + in-crate `control::` test names) so the
orchestrator can record reference-candidates entries for the answer key's
unmaterializable `tests/contract/`-shaped locators.

## Out of scope

Scheduler/selector/diagnosis internals (B2), controller logic (B3),
reference-candidates entries, CURRENT_STATUS, commits, Task-13 surfaces.
