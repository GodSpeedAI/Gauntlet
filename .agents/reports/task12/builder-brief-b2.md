# Task 12 — Builder slice B2 brief — control core (scheduler, selector, diagnosis)

You are a builder agent in /home/sprime01/projects/gauntlet (WSL2, 6 cores,
7.8 GiB RAM). Read `AGENTS.md` first; it binds you. Then
`.agents/reports/task12/recon-spec.md` (spec quotes + line refs),
`.agents/reports/task12/recon-code.md` (inventory; note B1 has since landed:
`Budget::restore`/`Spent::from_counters` in domain, ports
`producer_authority.rs` minting authorities, adapter crate
`gauntlet-adapter-meter-memory` implementing `BudgetMeter`), and
`.agents/reports/task12/builder-b1.md` (what B1 delivered — read the API
signatures from the source, not just the report).

Task 12 (plan): controller — proves P4/P6, targets AK-037 (selection +
rejected alternatives recorded), AK-038 (p4), AK-039 (diagnosis), AK-051
(p6), AK-052 (reserve). Slice B2 is the pure-logic control core as a new
application module `control::` in `crates/gauntlet-app/src/` (add `pub mod
control;` to lib.rs — no other registration needed). The composed
controller loop, policies, and proofs are slices B3/B4 — do NOT build them.

## Hard rules (same as B1)

- Never edit `.wayfinder/**`, `.agents/specs/**`, `.agents/plans/**`,
  `AGENTS.md`, `justfile`, `deny.toml`, `.agents/CURRENT_STATUS.md`,
  `.agents/research/preregistrations/*`, existing task12 recon/adjudication
  files, `.agents/prompts/**`, `.tmp/**`, `.jolli/**`. No commits.
- Single-compiler build lock: one Cargo command at a time; package-scoped
  recipes first; no raised parallelism; no `cargo clean`; `du -sh target`
  stays under ~4 GiB.
- Fail-first where practical; typed errors; no `unwrap`/`expect` in
  production paths; clippy `-D warnings`; fmt; comment density like
  neighbors.
- **Checkpoint discipline: append a dated entry to
  `.agents/reports/task12/builder-b2-checkpoints.md` after EACH numbered
  item.** Files are the contract; the final message is a convenience.
- Result contract: `.agents/reports/task12/builder-b2.md` (per-item summary,
  exact new public APIs, files, fail-first evidence, gate outputs,
  deviations + reasons, open questions for critics).

## Spec anchors (quoted in recon-spec.md — verify locally before relying)

- §11.2 eligibility (ALL must hold): status `open` and not claimed/in
  flight; every `depends_on` Unit `settled` or `out_of_scope`;
  `blast_radius != irreversible` (scheduler-level, unconditional, §17.1 —
  "independent of what any prompt says"); global and per-tier concurrency
  slots available; a runner is configured for the roles the slice needs;
  the budget meter will not breach the recovery reserve. Ordering:
  ascending blast-radius risk (`contained` first), then dependency depth,
  then `UnitId` lexicographic — "a scheduler whose behavior varies run to
  run cannot be compared against itself."
- §11.3: slices for non-separable units; a slice inherits the HIGHEST blast
  radius and MOST RESTRICTIVE authority of its members; slices never mix
  `irreversible` members with anything.
- §15.1 ten dimensions EXACTLY: impact · dependency order · confidence ·
  payment · reversibility · risk · governance status · settlement
  accessibility · recovery cost · expected information gain. Procedure:
  reject governance-blocked paths FIRST (authority, tier, budget,
  irreversibility); among allowed select the next affordable correction
  that most usefully reduces error while preserving recovery capacity;
  prefer `bounded correction → observable consequence → useful settlement`
  over large speculative rewrites; record the selection AND rejected
  alternatives with reasons — "A controller whose choices are unexplained
  cannot be improved."
- §15.2: bounded diagnostic cycle `diagnosis → prediction → cheapest
  discriminating test → disconfirming observation → correction`; sixteen
  loci: bad reference, ambiguous reference, wrong observation, observation
  failure, incorrect comparison, critic error, verifier error,
  route-selection error, implementation defect, tool defect, environment
  drift, insufficient payment, missing capability, integration failure,
  architectural defect, governance conflict. "A loop that only ever
  diagnoses 'implementation defect' is not diagnosing." Spend from
  `diagnostic_budget`; nothing resolves within it ⇒ `INCONCLUSIVE`,
  quarantine the evidence, stabilize, escalate or defer. §11.5: trigger is
  the same residual id N times, `N = GAUNTLET_NONCONVERGENCE_N` (default 3)
  → stop correcting, enter diagnosis; inconclusive diagnosis → escalate.
- §17.4: BudgetMeter consulted before every dispatch, not after.

## Item 1 — `control/scheduler.rs` (UnitScheduler)

Pure decision logic over injected facts (no I/O, no clock, no store reads
inside — B3 supplies fresh facts each tick and §11.4 step 3 freshness is
B3's wiring duty; document that contract on the type).

- `UnitScheduler::eligible(...)` per §11.2. The budget condition CONSULTS a
  `&dyn BudgetMeter` (or takes the meter's `would_breach_reserve` answer
  obtained at call time by the scheduler itself through a closure/trait the
  scheduler owns) — never a precomputed `bool` from the caller.
- Deterministic `plan(...)` ordering: ascending blast-radius risk →
  dependency depth → UnitId lexicographic. Ground the depth direction in
  the spec text and document it; determinism is the invariant (shuffled
  input ⇒ identical output — test it).
- Slice construction per §11.3 with the authority lattice pinned:
  domain `Authority := Critic | LeadAgent | Human` — most-restrictive
  ordering `Human > LeadAgent > Critic` (ground in §5.3.2 coupling:
  `irreversible ⟹ human`, `cross_cutting|irreversible ⟹ authority ≠
  critic`); pin test.
- `depends_on` graph validation (Task-7 F7, adjudicated into Task 12): at
  plan-input load — unknown target ⇒ typed error naming it; cycle ⇒ typed
  error naming the cycle. Fail-first both.
- Tests: open `irreversible` unit NEVER eligible and never sliced into
  anything (§17.1 — regardless of any other fact); depends_on not yet
  settled/out_of_scope blocks; per-tier + global concurrency accounting;
  meter-refusal excludes eligibility; ordering determinism (shuffle);
  slice inheritance both axes.

## Item 2 — `control/select.rs` (CorrectionSelector) + durable selection record

- Inputs are typed facts (`SelectionContext`): reference-derived facts,
  current residuals, verification outcomes, meter `remaining()`/reserve
  facts, unit tier/authority/governance state, attempt history (the
  developmental-memory slice available today). No free-text self-declared
  scores.
- Scoring is EXPLAINABLE BY CONSTRUCTION (pre-empting the plan's redesign
  trigger): an explicit lexicographic priority over the ten §15.1
  dimensions with recorded deterministic tie-breaks — every dimension
  referenced by name in the decision trace, no opaque numeric blend.
- Governance-blocked rejection FIRST, each rejection carrying a class
  (`authority | tier | budget | irreversibility`) and a typed reason with
  the blocking fact (e.g. the meter's numbers for budget blocks).
- Affordability: a candidate whose estimated slice the meter would refuse
  (`would_breach_reserve`) is rejected with the meter's numbers recorded —
  "preserving recovery capacity" must be a computed fact, not prose.
- Preference encoding: bounded-correction-with-observable-consequence over
  speculative rewrites (candidate shape ordering, documented).
- Output: a domain type `CorrectionSelection` (new file in
  `crates/gauntlet-domain/src/` — e.g. `correction.rs`; follow aggregate
  idiom): chosen correction descriptor + `Vec<RejectedAlternative{correction,
  class, reason}>` + decision trace. Constructing it requires the
  alternatives (no "selection without record" state).
- **Durable recording (AK-037 is a recording claim):** add a StateStore
  port method following the `record_role_invocation` precedent (records
  row; adapter `RecordKind` growth is a recorded act — check whether the
  sqlite CHECK constraint enumerates kinds and whether an append-only
  migration is needed; if you can encode within an existing kind without
  conflation, prefer that and justify). SQLite writer + ports contract
  suite coverage + in-app fake-store test. If you believe the event log is
  the better carrier, you may deviate — justify against §16.4 single-writer
  and §18 replay in the report.
- Tests (AK-037 shape — name them so `cargo nextest run -p gauntlet-app
  control::` catches them, e.g. `control::select::controller_records_alternatives`):
  selection + alternatives + reasons durable roundtrip; governance-block
  never chosen even when highest-impact; budget-block records meter
  numbers; tie-break determinism; bounded-over-speculative preference.

## Item 3 — `control/diagnosis.rs` (DiagnosticEngine)

- Trigger detector: same-residual-N-times. **Stable semantic key** (prereg
  concern, now a requirement): key on a projection like
  `(unit, dimension, class)` — NOT raw `ResidualId` values, which are
  minted fresh per comparison. Tests: N recurrences with FRESH ids still
  trigger; N-1 does not; different-dimension same-unit does not.
- Bounded cycle state machine per §15.2 with the 16-locus taxonomy typed;
  locus selection is input-driven with the selecting evidence recorded —
  and a test proves a measuring-apparatus-shaped input (e.g. a residual
  whose facts indicate verifier error) does NOT route to
  `implementation_defect`.
- Diagnostic budget: spend via `Budget::charge_diagnostic` semantics
  (domain; allowance mapping reconciled with config's `diagnostic_budget`
  scalar — check how Task-5's `BudgetSettings` models it; do not invent a
  second unit). Exhaustion ⇒ `INCONCLUSIVE` + quarantine + escalate-or-defer.
- Quarantine is a real inspectable mechanism (a typed record/marker, not a
  doc note) — design within existing store surfaces where possible.
- Escalation via the `HumanEscalation` port: mint an `EscalationItem` (a
  test fake accepts it); no real adapter exists — that's fine.
- G16 honesty: no path re-asks a model until it says yes; one disconfirming
  observation per cycle; bounded by construction (test).
- Tests (AK-039 shape, e.g. `control::diagnosis::nonconvergence`): the
  trigger boundary; budget exhaustion ⇒ INCONCLUSIVE + quarantine marker +
  escalation item; cycle boundedness.

## Item 4 — config knobs (only what Items 1–3 need)

`GAUNTLET_NONCONVERGENCE_N` (spec default 3, §11.5). Read
`crates/gauntlet-app/src/config/` first: Annex E env names exist in
`.env.example`, but the resolution layer may not surface these settings
yet. Add ONLY spec-backed rows: §8.6 defaults table rows are the canonical
literal home (walk.rs `DEFAULTS_TABLE_SETTINGS` pinned inventory — update
it as a recorded act if you add a §8.6-backed row); inline spec defaults
(§11.5 N=3) may back a default with the spec line cited in a comment.
Knobs with NO spec default (retry base/max, stall timeout, attempts per
unit, poll interval for B3) are NOT invented here — leave them for B3 and
note them. `.env.example` already lists them; `env_example_complete` must
stay green.

## Item 5 — Gates (run personally, record)

```
just check-package gauntlet-domain && just test-package gauntlet-domain
just check-package gauntlet-ports && just test-package gauntlet-ports
just check-package gauntlet-adapter-state-sqlite && just test-package gauntlet-adapter-state-sqlite
just check-package gauntlet-app && cargo nextest run -p gauntlet-app control::
just check && just lint && just deny && just boundaries && just ruler
```

Red gate at session end ⇒ say so loudly; never disable/weaken to green.

## Out of scope

The controller loop / iteration engine / policies (B3), observer adapters
(B3), proofs p4/p6 (B4), CLI changes, meter changes, HumanEscalation
adapters, doctor.
