# Task 20 — closure: CLI surface + first full loop on the mock

**Settled:** 2026-09-01. **Ruler units answered:** AK-060, AK-061 (both
provisional pending the ruler owner's locator reconciliation; see below).
**Plan:** `.agents/plans/gauntlet-impl.md` §Task 20 (phase 5 — self-hosting
begins). **Commission:** charter.md; **slice ledger:** IN-FLIGHT.md.

## What landed

- **Full §20.1 command surface** (crates/gauntlet-cli, thin-lib composition
  root): doctor, init, reference (form/freeze/show/diff/verify/digest/
  validate), run (+ --dry-run), status, inspect, verify, resume,
  escalations (list/resolve/retry), roles (install/verify/list), skills
  (install/verify/list), improve (typed not-wired, plan Task 23), export.
  Every command: unknown-arg exit 2, typed errors, secrets never printed.
- **`gauntlet run`** composes the EXISTING RunController end-to-end:
  doctor preflight → freeze → plan slices → dispatch → observe → compare →
  verify → select → settle → checkpoint → six-category report. Fail-closed
  runner selection over three fronted tokens (prime-agent, cli-agent,
  mock-agent; charter D2 — UNK-003 forced, stays the operator's). The
  plan's own dogfooding instruction now applies: remaining tasks run
  THROUGH `gauntlet run`.
- **Report generator** (gauntlet_app::report): implemented-and-proven /
  provisional / configured-but-untested / proposed / blocked / unknown —
  populated from durable records only (teeth: settlement-row removal moves
  the unit out); the CLI declares its real command table as the
  capability manifest.
- **E2E** (crates/gauntlet-cli/tests/e2e_full_loop_mock.rs, real binary via
  CARGO_BIN_EXE): seeded fixture project tests/fixtures/target (strict-
  parser-valid reference, 3 blocking contained/critic units, real
  instruments: filesystem facet + test facet running scripts/check.sh);
  happy leg settles 3/3 with pinned instrument evidence and record-derived
  report assertions; lying-builder leg (e2e-lying) ends non-settled;
  **report-forging liar leg (e2e-forged-report) ends non-settled** (the
  falsifier's own round-1 blocking attack shipped as a permanent tooth);
  parallel-carry leg (max_parallel=2); kill-9 + resume legs; settled no-op
  legs; dry-run zero-write leg (content-hashed).
- **New adapter crates:** gauntlet-adapter-workspace-fs (R-1b),
  gauntlet-adapter-escalation-fs (production HumanEscalation queue),
  gauntlet-adapter-id-entropy (production ULID id generator); mock gains
  its RUNNER_TOKEN. All std-only, contract-suite-verified.

## Residual obligations triggered by this task — DISPOSITIONS

- **R-1b CLOSED (both halves).** Production FsWorkspaceProvider wired into
  the composition; the gate admits against the FRESH per-unit workspace AND
  execution anchors there (mock effects, CLI cwd/{workspace}, produce
  locator, ladder factory memoized per anchored workspace; COR-D3).
  Distinct-workspaces teeth red-when-bypassed. §17.5.1 invariant 1 holds in
  the composed path (round 2-6 sweeps found no escape).
- **R-3 PARTIALLY DISCHARGED (honest, per charter D3).** The composition
  path calls wrap_builder_command after admission for the process-backed
  token; with a live backend the composition refuses fail-closed
  (LiveContainmentDeferred — recorded Task 25 leg, never a silent unwrap).
  The fully-composed LIVE-containment run remains **Task 25**'s E2E. The
  round-1-6 sweep confirmed every route that could run a builder unwrapped
  under a live posture refused instead.
- **R-2 RE-ADJUDICATED (no change).** The wrap-consuming composition wraps
  process-backed commands; the CLI adapter still consumes only a command
  TEMPLATE; nothing in its path consumes a SandboxedCommand. Carried to
  the operator's re-adjudication queue with the new evidence.
- **R17-F4 CLOSED + HARDENED.** The runtime-provided result-contract
  directory is materialized before being claimed as a writable root
  (the teeth's first live exercise found the missing materialization —
  ContractDirPolicy fix, cor-d-report.md item 3) and is added to the wrap
  policy in the composed world.
- **R17-note-1 CLOSED.** write_atomic carries directory-entry fsync with a
  mutation-tooth sensor (three-state proof, b1).
- **R2-N1 CLOSED.** Roles' context.deny sets reach the comparison envelope
  before construction (ComparisonAuthority::with_role_deny) with a
  composed-path teeth test (compare_deny.rs).
- **UNK-003 FORCED, STILL OPEN (operator's).** `gauntlet run` fails closed
  without runners.default; dry-run needs none. The repo's own GAUNTLET.md
  stays unset by design.
- **Cohort guardrail (D7) HELD.** No cohort source invented; the parser
  stays closed; the pinned compose-time record re-verified by the E2E
  (empty cohorts, plan preview 2 dispatches, irreversible never planned).
- **Arch-suite standing trigger DISCHARGED.** Re-litigation at
  arch-relitigation-task20.md: 4 HOLD / 0 AMEND / 0 REOPEN; both edits
  within documented flows; 3 prose-staleness notes recorded (next-touch
  class).

## Confirmation history (the loop, honestly)

1. Build waves B1 ∥ B2a → B2 → B3 (two builder deaths + one pseudo-death
   and one network cut, all recovered from checkpoint files; zero work
   lost) → COR-B3a (the E2E itself found the evidence-path collision —
   ProducePolicy became a composition decision with the default world
   byte-identical; all p-proofs stayed green UNMODIFIED).
2. **Blind design-as-built critic: NOT_CONFIRM** (0 blocking / 4 major /
   6 minor / 5 informational) → adjudicated design-adjudication.md →
   COR-D1..D10 (interrupted post-landing by a usage limit; recovered and
   verified by the orchestrator — which re-proved everything personally
   and fixed the ContractDirPolicy defect the new teeth exposed).
3. **Falsifier round 1 (blind, executing): NOT_CONFIRM** (1 blocking: a
   report-forging liar settled everything — producer integrity unverified;
   1 major: skills install→verify round-trip broken; 4 minor) → COR-F1..F6
   (producer-corroboration settlement gate in the composed world; the
   forging attack shipped as a tooth).
4. **Falsifier round 2 (fresh blind): NOT_CONFIRM** (1 blocking:
   settled-resume no-op defeated by a production-key-surface split; 1
   major: escalation resolve clobber via decoded-ticket write-back; 1
   minor: loose skills-lock reader) → COR-R2-1..3 (ONE production key
   surface, `ProducePolicy::production_recorded`; requested-id write-back
   + identity-mismatch refusal; strict reader).
5. **Falsifier round 3 (fresh blind): NOT_CONFIRM** (2 major: the
   recovery's MINT half still facet-keyed — false forensics + dishonest
   budget_exhausted + A5 window asymmetry; a shape-valid lock lying about
   content verified clean; 1 minor: no single-writer guard) → COR-R3-1..3
   (per-world mint; skills content-verify vs repo sources; O_EXCL
   writer.lock).
6. **Falsifier round 4 (fresh blind): NOT_CONFIRM** (1 major:
   recovery-completed production on a settled run still burned a round;
   2 minor) → COR-R4-1..3.
7. **Falsifier round 5 (fresh blind): NOT_CONFIRM** (1 major: the
   SETTLEMENT arm of the same seam, reachable from an ordinary crash; 1
   minor: asymmetric forensic pinning) → COR-R5-1..2.
8. **Falsifier round 6 (fresh blind): CONFIRM** — 0 blocking / 0 major /
   0 minor / 3 informational. All prior arms regression-swept; every
   COR-R5 cell held; over-rejection leg clean.

## Settlement evidence (orchestrator, personally run, 2026-09-01)

- `just check` 0 · `just test` **1377/1377** · `just lint` 0 · `just deny` 0
- `just boundaries` 228/228 · `just ruler` PASS (before and after every
  round) · `just doctor` OK
- `cargo nextest run -p gauntlet-cli --test e2e_full_loop_mock --test
  cli_surface` **16/16**
- repo-root dry-run gate: exit 0, **zero writes** (before/after git-status
  diff empty; the leg's own content-hash tooth verified separately by the
  falsifier)
- **Teeth re-verified personally:** the producer-corroboration gate
  bypassed (one-line revert) ⇒ the forged-liar E2E leg RED (exit 100, the
  liar settles) ⇒ byte-identical restore (sha256
  32183f22…ecca8c669c before/after) ⇒ GREEN 12/12. Fail-closed runner
  selection, wrap refusal, dry-run content tooth, escalation durability
  and the denial batteries are shipped tests (compose_teeth.rs,
  escalations_cli.rs, skills_roundtrip.rs, compare_deny.rs) exercised in
  the 1377.
- Logs: settle2-*.log, tooth-corroboration-*.log, final-*.log in this
  directory.

## AK disposition (for the ruler owner; never ruler edits)

- **AK-060** — the CLI exposes all thirteen §20.1 commands; evidence:
  cli_surface snapshot + argument-discipline battery (in-crate target
  `crates/gauntlet-cli/tests/cli_surface.rs` — frozen locator
  `tests/e2e/cli_surface` divergence recorded in
  reference-candidates.md, AK-063/AK-065 class, with the architectural
  reason the exemption is arch-tests-only). PROVISIONAL.
- **AK-061** — the core builds and completes a full loop with operator
  surfaces limited to the CLI; evidence: e2e_full_loop_mock (12 legs) on
  the default CLI-only build, `operator.surfaces: [cli]` in the fixture
  posture. Frozen locator `build/cli-only-profile` divergence recorded in
  the same entry. PROVISIONAL.

## Residuals and triggers (registered; next owner in parentheses)

- **ProcessChildTransport torn-read race — CLOSED this task** (was the
  charter's flaky class): a parse failure is now decidable only once the
  child is terminal; torn-write teeth b6/b9 shipped
  (falsifier_round1_live_attacks.rs). The escalation that named it came
  from COR-R4's battery forensics.
- **Settlement forensics asymmetry (I-1, round 6, dormant):** the
  settlement_row_unrecoverable pin lives only in the certified no-op
  close; a full-path resume discloses the same damage through the report
  classification instead. Decision, not accident (falsifier-round6.md).
  Trigger: next settlement-forensics touch.
- **Resume no-op predicate asymmetry (I-2, round 6, dormant):** no-op
  requires all-Settled; decide_progress closes on Settled|OutOfScope.
  Trigger: first out-of-scope-heavy reference or next resume touch.
- **Escalations resolve/retry success path (F-5, round 1, carried):** the
  positive half is **Task 21**'s commissioned surface (AK-062 positive
  half); on this tree the CLI refuse paths are the honest posture.
- **Checkpoint forged-offset class:** remains the recorded planning-layer
  deferral; F-R4-3's precondition confirmed the class, did not reopen it.
- **Default-world multi-facet recovery asymmetry (round 3, latent):** the
  default produce world records only the first facet's ref; no shipped
  composition fronts >1 observer in the default world. Trigger: any
  multi-observer default-world composition.
- **Manifest/count prose staleness (arch re-litigation, non-blocking):**
  gauntlet-app/Cargo.toml:16-17, gauntlet-cli/Cargo.toml:92-94,
  single_writer.rs:122-134. Next-touch class.
- **Load-flaky charter class note:** the live-process family now has the
  torn-read fix; b5 remains occasionally load-sensitive and passed all
  settled-tree runs. Charter addendum updated.
- Prior task deferral lists (task13-19) are unchanged except where
  explicitly closed above.

## Self-hosting note (plan Task 20 step 6)

From here the plan's dogfooding instruction applies: remaining tasks run
through `gauntlet run` with this plan as the route and the frozen answer
key as the ruler. The composed path has now executed every stage of that
loop on the mock; Task 25 extends it to a real runner and real containment.
