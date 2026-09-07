# COR-1 progress — task 26 corrections round

Builder COR-1. Scope: `tests/fixtures/baseline/**` + this file + cor1-report.md.
D4 diagnosis runs against SCRATCH copies and the real binary; nothing in
`crates/**` is ever edited by this round.

## Checkpoint 1 — context read (2026-09-06)

Read: charter, b1/b2/b3 reports. Key extracted facts:

- D1 shape: mirror `tests/fixtures/target-real/.wayfinder/reference.yaml`
  `minimum_reliability` (`>= 2 settlements` inside the `>` block scalar).
  Manifest parser: `crates/gauntlet-app/src/reference/manifest.rs`.
- D2 schema (from `gauntlet-adapter-verify-test-report/src/lib.rs`): the
  report at the locator must be a JSON object with whole-number `tests`
  and `failures`; PASS iff tests>0 && failures==0; FAIL iff tests>0 &&
  failures>0; tests==0 or a missing total = `InstrumentFailed` (the rung
  refuses as an instrument → shipped controller turns it into
  `ControlError::VerificationRefused`, a hard run error). So every check
  outcome must emit a DECIDING report where a decision is honest:
  pass → tests=1/failures=0; fail (source present, Y absent) →
  tests=1/failures=1; missing source → the check's one criterion ("does
  the deliverable exist") was asked and answered NO → tests=1/failures=1
  is the honest deciding shape (tests=0 would re-create the hard-error
  world D2 exists to fix).
- D3 shape: mirror target-real `agent/child.sh` — `cat >` the FULL
  deliverable fresh into every workspace; honest-success carries X then Y;
  subtle-defect carries Y only; lying/compromised write nothing.
- D4 leads (from controller.rs reading):
  - produce (`BindLocatorContent`) runs AFTER observe+bind+compare inside
    `process_completion`, so the b3-observed `evidence_produced` events
    imply those stages ran; the export stream shows EVENTS only —
    observation bindings / comparisons / §15.1 selections may be keyed
    rows, so "absent from export" is not yet "absent from the store".
    The store-level truth is what the diagnosis must establish.
  - The settlement corroboration gate (`produced_artifact_corroborated`,
    controller.rs ~line 3648) requires THIS (run, iteration)'s bundle to
    carry a test facet (kind=test, command+exit_status present,
    exit_status==0). GAUNTLET_TOOL_TEST is bound per leg by the CLI
    (`improve_cmd.rs` re-exec), the observer spawns it with
    cwd = the dispatched workspace.
  - `GAUNTLET_BASELINE_SCRATCH_ROOT` (improve_cmd.rs ~1144) retains the
    leg worlds — the diagnosis lever.
  - `target/debug/gauntlet` exists (built 2026-09-06 16:46).

## Checkpoint 2 — D1 fix + refreeze

DONE. `minimum_reliability` reworded to carry `>= 2 settlements`
(mirroring target-real's working shape); lock re-frozen; digests verified
against the script. (Superseded the same round by the D4 edits below —
the lock was re-frozen once more over the final artifact bytes.)

## Checkpoint 3 — D4 diagnosis (complete; all fixture-side)

Reproduced b3's D4 world on a scratch copy (D1 fix only, two-scenario
variant) via the real binary + `GAUNTLET_BASELINE_SCRATCH_ROOT` retention.
Findings, from the leg's own durable store:

- **D4-a (AK-962 never dispatches — governance gate).** Pinned
  `gate_refusal` evidence: "runner reports isolation none, below the
  local tier's required rung process — refused for unit AK-962 (spec 17.3,
  AK-050)". The answer key declared AK-962 `blast_radius: local`; the
  charter-fixed `sandbox.backend: none` reports isolation `none`; the §17.3
  gate refuses local-tier dispatches outright. A permanent governance block:
  the run could never close `settled` even with everything else fixed.
  FIX (fixture-side): AK-962 -> `contained` (+ claim text drops
  "local-tier"), manifest prose/constraints updated, lock re-frozen.
- **D4-b (the test facet never ran — spawn failure).** Observation bundle
  (iter 1): `"unavailable_evidence":[{"kind":"test","reason":"spawning the
  test instrument failed: Permission denied (os error 13)"}]`.
  `GAUNTLET_TOOL_TEST` is bound to the fixture's own canonicalized
  `scripts/check.sh` (improve_cmd.rs:1299), spawned DIRECTLY by the test
  observer (gauntlet-adapter-observe-test/src/lib.rs:85) — and the fixture
  scripts shipped mode 0644 (B1 never needed exec bits; the e2e chmods its
  copies, `improve_cmd.rs` chmods only materialized copies, never the
  binding target). With no executed test facet, the corroboration gate
  (`produced_artifact_corroborated`, controller.rs:3648) honestly refused
  settlement every iteration (pinned `producer_uncorroborated`,
  object baef0ea4...), the unit stayed open, §11.5 row-1 corrections
  re-dispatched, rounds exhausted: `budget_exhausted`, zero settlements —
  exactly b3's "corrections with no durable residual" loop (the residuals
  exist; the export stream just does not carry keyed rows: 2 observation
  bindings, 2 evidence productions, 2 correction_selection records were
  all present in the store).
  FIX (fixture-side): `chmod 755` on check.sh / held-out.sh / child.sh
  (target-real's modes; git records 100755 at the orchestrator's commit).
- **D4-c (absent-artifact path hard-errors — the D2 mechanism, corrected).**
  Reproduced b3's lying-defect leg: exit 1, "the test-report rung
  ... failed as an instrument ... the test report carries no tests total".
  The bytes at the locator were the FIRST OBSERVATION FACET's document
  (`facet_evidence@v1`, filesystem): `produce_unit_evidence`'s absence arm
  materializes the first facet's evidence doc, NOT the declared check's
  stdout (controller.rs:3799-3807). b3's D2 story ("the check's output is
  TEXT") was wrong about the byte path — stdout JSON alone cannot fix this
  path. FIX (fixture-side, on top of the adjudicated D2 letter): the
  declared check — the run's own test instrument — writes its rung-valid
  report at BOTH unit locators when and only when no produced artifact
  stands there; the produce step then binds an honest instrument-produced
  report and the rung DECIDES (lying: tests=1/failures=1 -> FAIL ->
  residual -> bounded corrections -> zero settlements). Builder-produced
  artifacts are never overwritten.

## Checkpoint 4 — fixes landed (fixture files)

- check.sh: rewritten (verdict + greps unchanged; JSON stdout report in
  every outcome; locator report when absent). `sh -n` clean.
- child.sh: honest-success writes the COMPLETE deliverable fresh (X then
  Y; the base-source refusal removed); subtle-defect comment updated
  (fresh full write, Y only); lying/compromised unchanged (write nothing);
  reviewer/critic unchanged (inspection-only). `sh -n` clean.
- answer-key.md: AK-962 contained-tier, claim drops "local-tier".
- reference.yaml: reliability prose (both units contained-tier) +
  constraints (contained-tier only under none).
- GAUNTLET.md body + README.md: output contract, fresh-write wording,
  tier statements, lock re-freeze note.
- reference.lock: re-frozen over the final artifacts:
  answer_key sha256:a179a9d9..., manifest sha256:f3958d81...
- Exec bits 755 on check.sh / held-out.sh / child.sh.

## Checkpoint 5 — verification COMPLETE, report written

- Script matrix re-run by hand in scratch worlds (both procedures:
  per-role fresh worlds + b1's one-world-per-scenario sequential roles).
  Every discrimination cell matches b1's observed table; the locator
  column now shows the instrument-authored
  `baseline-declared-check-report` exactly where no builder artifact
  was produced (the D4-c mechanism, visible at script level).
  Fail-closed negatives re-verified (all exit 2, no result docs).
- Profile D over the real binary, five scenarios: honest-success and
  subtle-defect SETTLE (2 units each, terminal `settled`, reliability
  10000); lying-defect and compromised-critic end `budget_exhausted`
  with ZERO settlements and the instrument's own FAIL report bound at
  the locator; silent ends `escalated` (bounded, zero settlements,
  reserve intact). No leg aborts the run.
- FULL plan-gate shape over A,B,C,D: exit 0, report written, keys
  exactly A/B/C/D/_meta, the charter's A/B/C/D matrix derived.
- Batteries: gate test 4/5 (the one failure = the b3-recorded deviation
  pin at improve_baseline.rs:364, superseded by this round's fix — the
  test file is outside this round's scope); `just test-package
  gauntlet-app` 618/618 exit 0; `just test-package gauntlet-cli`
  138/139 (the only red the same gate-test pin; the e2e_recovery
  kill-window flake fired once under load, passed in isolation and in
  the second full run; b1_contract_wins_over_exit7 did not fire).
- Lock digests re-verified against the final artifact bytes; fixture
  integrity verified against b1's sha256 ledger (src/gadget.rs,
  task.yaml, held-out.sh content unchanged); scratch worlds removed.
- cor1-report.md written (the completion contract).

