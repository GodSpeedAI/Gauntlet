# Task 20 / COR-B3a brief — composition correction driven by live E2E failure

You are the correction builder for plan Task 20 in
/home/sprime01/projects/gauntlet. Read first, in order: `AGENTS.md`,
`.agents/reports/task20/charter.md` (binding), `b3-report.md` (the precise
live-failure evidence you are correcting — read its quoted errors and case
analysis), `b3-brief.md` (what the E2E must become), `b2-report.md` (the
composition you are correcting). This correction is triggered by an honest
live failure, the strongest trigger there is. Fix the mechanism, never the
symptom; never weaken an assertion, gate, or proof.

## Corrected item 1 — the evidence-path collision (the blocker)

Live failure: the observation pipeline writes the facet's
`facet_evidence@v1` JSON at the unit's `evidence_required.locator`, and the
composed ladder rungs then require their OWN artifact shape at that exact
locator ⇒ first dispatch ends `RungFailed`. No fixture configuration can
settle a unit (B3 closed the case analysis).

Correct the SEMANTICS, not the symptom: facet/observation evidence belongs
in the content-addressed EvidenceStore (with provenance), and the unit's
`evidence_required.locator` remains the location of the VERIFICATION
artifact a rung produces/consumes (workspace artifact, transcript, report).
Trace the actual write path first (controller's observe → bind → produce
evidence steps, `crates/gauntlet-app/src/control/controller.rs` +
`observation::`, and B2's compose.rs observer wiring) — identify WHERE the
locator got conflated with facet-evidence storage, fix it there. The
pre-adjudicated task20 deferral "evidence production binding first-facet
bytes" is adjacent: if your fix resolves or narrows it, record that in the
report; if not, leave the deferral standing. CONSTRAINTS: every proof test
p1-p12/g6/g7/g10 and the contract suites stay green UNMODIFIED (a proof
that must change to keep passing is a design change — stop and record
instead); the fixture-side fix in B3's files is allowed ONLY if the defect
turns out to be fixture-facing configuration rather than composition.

## Corrected item 2 — Annex-E clock hook reaches the composition

Live failure: `compose.rs` never consults the `GAUNTLET_CLOCK` hook, so the
mock's fixed-2023 event epoch stall-escalates under the wall clock. The
hooks module (`gauntlet_adapter_agent_mock::hooks`, `clock_from_env`,
`id_generator_from_env`) exists exactly for this. Wire the composition:
clock selection through `clock_from_env()` (production default = the wall
clock when unset — behavior-identical when the var is absent), and the id
generator through `id_generator_from_env()` (None ⇒ the composition's
existing generator). Do not invent new env spellings.

## Corrected item 3 — max_critics posture (verify only)

B3's workaround (`max_critics: 0` in the fixture) suggests the fail-closed
refusal of an isolation-none model critic is CORRECT governance (§13.2
fresh-context + posture). Verify by reading the refusal site: if it is
honest fail-closed governance, no change — record the verification; if it
is an accidental conflation, fix it minimally.

## Then — complete the E2E (B3's brief, B3's files)

With the composition corrected, create
`crates/gauntlet-cli/tests/e2e_full_loop_mock.rs` per `b3-brief.md` Item 2:
run leg (settles every blocking unit, record-derived report assertions,
pinned evidence inspected back through status/inspect/verify/export),
dry-run leg, and the lying-builder DISCRIMINATION TOOTH (non-settlement).
B3's fixture (`tests/fixtures/target/`) and scenario fixtures are landed —
use them; fixture fixes are yours if the corrected composition shifts
requirements (e.g. locator spelling in the fixture reference must now name
the verification artifact the rung reads).

## Verification (personally, all must pass)

`just check`; `just test` (expect ≥1319 + your new tests); `just lint`;
`just deny`; `just boundaries` (228/228); `just ruler` (before/after);
`just doctor`; `cargo nextest run --test e2e_full_loop_mock --test
cli_surface`; repo-root `cargo run -q -p gauntlet-cli -- run --dry-run
"seeded bounded task"` exit 0 zero-writes.

## Constraints + protocol

Your files: the composition/correction surfaces you trace (record every
file you touch and why), `crates/gauntlet-cli/tests/e2e_full_loop_mock.rs`,
fixture fixes. Never: `.wayfinder/**` (repo root), `.agents/specs/**`,
frozen locks, weakened assertions. Operational: flock +
`sccache --stop-server` inside the lock; background+poll cadence; declare-first
checkpointing in `.agents/reports/task20/cor-b3a-progress.md`; final report
`.agents/reports/task20/cor-b3a-report.md` (root cause per corrected item,
files touched, proof battery results, deferral disposition, deviations).
If your window runs short, the progress file is the contract — leave it
resumable.
