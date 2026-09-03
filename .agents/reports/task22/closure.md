# Task 22 closure — Developmental memory and the capability ledger (P10)

**Date:** 2026-09-03. **Plan:** `.agents/plans/gauntlet-impl.md` §Task 22.
**Spec:** §19.1–19.2 (verbatim in charter.md), I6. **Ruler unit:**
AK-055. **Commit:** the task22 closure commit (one task one commit; see
the git log line for the sha).

## What shipped

Two durable stores with opposite write policies (§19.1), zero new
external dependencies, zero new CLI surface, zero settlement-path
changes (verified again at closure: `git diff HEAD --stat` over
`crates/gauntlet-app/src/settle/` and `producer_authority.rs` is empty):

- **Port** `crates/gauntlet-ports/src/capability_ledger.rs`: the
  `CapabilityLedger` port carrying (a) the append-only, ungated,
  identity-idempotent developmental observation log (all seven §19.1
  classes; disagreeing re-append is a typed `ObservationConflict`) and
  (b) the capability register — status derived from first-class
  transition history (no status column, no set-status method anywhere),
  state machine from domain `Capability::restatus`, Revoked terminal.
  Typed refusal vocabulary and the durable `PromotionEvidence` decision
  record.
- **Contract suite** `contract/capability_ledger.rs` with
  green/broken/caps-broken legs in `suite_runs.rs`; grown by COR-1/COR-2
  (`elevating_transitions_are_log_rooted`,
  `a_zero_reason_refusal_does_not_compose`).
- **SQLite adapter** `SqliteCapabilityLedger`/`EmbeddedCapabilityLedger`
  over migration `0009_capability_ledger.sql` (INSERT-only tables,
  cross-run by design), passing the unmodified shared suite; promotion
  transitions log-rooted inside `BEGIN IMMEDIATE`
  (`UnrootedPromotion`).
- **App services** `gauntlet_app::memory`: the developmental import
  (read-only over the StateStore substrate, ungated, idempotent,
  `occurred_at` populated from the verification records'
  `produced_at`; `record_recovery` refuses `RecoveryNotEarned`
  (completed=0) and `RunNotDurable`, appending nothing) and the
  `PromotionService` (all five §19.2 conditions, typed per-condition
  refusals; chronology-ordered windows; fact-uniformity pre-validation;
  complete witnesses; refusal writes `Provisional` + durable reasons;
  revoke sticky).
- **Config** `promotion.min_settlements` (spec inline default 3),
  full wiring per the NonconvergenceN pattern; configured 0 is a typed
  `ValueOutOfRange` at every layer; `GAUNTLET_PROMOTION_MARGIN` remains
  vocabulary-only (task23's declared margin).
- **Proof** `crates/gauntlet-cli/tests/p10_no_false_learning.rs` — four
  legs over the REAL sqlite ledger + REAL state store, every substrate
  fact earned through the authorities, the recovery seam driven through
  real §18.3 damage + replay-completion: single success stays
  `Observed` (AK-055 half 1); three identical-content-axis settlements
  refused `insufficient_variation` (the plan's Done-when teeth); the
  full conjunction promotes with a durable decision record that
  survives a ledger reopen; revoke sticky with no flip-back.

## Confirmation history

1. **Blind design-as-built critic round 1: CONFIRM** (0 blocking /
   0 major / 4 minor / 4 informational; `critic-design.md`). Verified
   independently: no set-status escape, settlement path zero diff,
   suite-can-fail legs on distinct cases, idempotency without
   double-count, config-0 refusal at every layer, five declared
   deviations acceptable-and-recorded.
2. **Blind executing falsifier round 1: NOT_CONFIRM** (1 blocking /
   3 major; `falsifier-round1.md`). All four named teeth bite; the
   AK-055 run-id trap repelled with direct ungated injection; but:
   **F-2 (blocking)** honest out-of-order imports flipped a rising
   failure rate into a PROMOTION (window verdict was a function of
   import order); **F-1** variation credited to ABSENT facts; **F-3** a
   fabricated promotion evidence applied directly through the port;
   **F-4** recovery credit minted on an empty substrate.
3. **COR-1** (`cor1-brief.md`, `cor1-report.md`): C1 chronology-ordered
   windows (`occurred_at` substrate fact; undecidable windows refuse),
   C2 settlement fact-uniformity + complete witnesses, C3 log-rooted
   elevations (`UnrootedPromotion` at the port + suite, all legs),
   C4 rooted recovery records, C5 teeth-wording honesty. Every
   correction RED-first proven against the falsifier's verbatim
   reproduction.
4. **Fresh blind executing falsifier round 2: CONFIRM** (0 blocking /
   0 major / 3 minor / 3 informational; `falsifier-round2.md`). All
   four round-1 holes CLOSED over the real adapters (B3b closed across
   five import orders; B2a closed across all nine single-missing-fact
   classes; B4 closed for fabricated and foreign-capability witnesses;
   B5 closed), every correction tooth proven load-bearing by
   neutralization (2–6 detectors RED each). Round-2 agent instance died
   on an API outage mid-round; the re-dispatched fresh agent recovered
   its unrun scratch attacks, executed, judged, and extended them
   (task16 precedent; recorded in the round-2 report).
5. **COR-2** (`cor2-brief.md`, `cor2-report.md`): C6 zero-reason
   refusals refuse to compose (`EmptyRefusal`, RED-first, suite case on
   all legs, decode re-validation fails closed), C7 the chronology docs
   claim softened to the seq-tiebreak truth (docs-only).
6. **Focused fresh executing falsifier round 3: CONFIRM** (0 findings /
   0 regressions; `falsifier-round3.md`): F2-2 stays dead over the real
   adapter (compose, decode, and stored-read-back all fail closed with
   an honest-payload control), C6's tooth bites on three detectors,
   C7's docs match executed behavior, full regression sweep green.

## Orchestrator personal verification (never settled on sub-agent say-so)

- Gate battery at every phase transition, and the closure battery on
  the final tree: `just check` clean; `just test` 1436/1436 THREE
  consecutive full runs; `just lint` clean; `just deny` ok;
  `just boundaries` 228/228; `just ruler` PASS before, between, and
  after every round and at closure; plan gates `p10_no_false_learning`
  4/4 and `gauntlet-app memory::` 34/34.
- **Teeth, personally executed:** neutralized the minimum-settlements
  condition → p10 leg 1 RED; restored byte-identical (sha256
  `39d7709d…` verified) → GREEN. Neutralized the identical-variation
  condition → p10 leg 2 RED; restored byte-identical (same sha) →
  plan gates GREEN. Both of the plan's Done-when teeth bite in my own
  hands.
- One battery incident, recorded honestly: the first post-COR-2 full
  `just test` run failed with the failing test name lost to my own
  output truncation; three consecutive full re-runs and three targeted
  package re-runs were 100% green with zero intervening changes. The
  class matches the recorded task20 residual ("load-flaky b5 remains
  occasionally load-sensitive"); no assertion was touched between
  runs; nothing was weakened to reach green.

## Honest boundaries and residuals (all trigger-recorded)

- **Append-only-log forgery class (documented, not sealed):** elevation
  is log-rooted (every promotion witness must exist in the ledger's own
  log with matching identity) but NOT unforgeable against a writer who
  forges the log itself — the developmental log is append-only and
  auditable by design; the sealed-mint alternative (authority types
  with no public constructors, the `AdmittedSettlement` house pattern)
  is a recorded ports-revision candidate (same class as
  `BudgetMeter::mint`'s recorded candidate). Composition roots hand
  ledgers only to trusted code. Same honest boundary shape as task21's
  raw-store-write disclosure.
- **`occurred_at` lies are undetectable in-ledger** (F2-5): the
  chronology is only as honest as the appended fact; cross-checking
  against the substrate would be a new mechanism (out of scope).
- **`occurred_at` ties are order-dependent by construction** (F2-1/C7):
  the `(occurred_at, seq)` tiebreak is append order; documented as a
  recorded residual, not hidden.
- **Single-run promotion tension (F2-3):** three settlements from ONE
  run over distinct units promote — charter-conformant, but in tension
  with §19.1's verbatim "One successful run is not capability."
  Recorded in `.agents/reports/reference-candidates.md` (2026-09-03
  CandidateRevision entry) for the ruler owner; NOT patched.
- **Evolving-run re-import wedge (critic RES-03):** mid-run imports of
  an evolving run hit `ObservationConflict` (fail-closed — memory goes
  missing, never false); latent until production call sites exist.
  Trigger: task23 wiring.
- **Tracked-dimension coverage (critic RES-05):** decided-on today:
  variation, recovery, failure rate, gate erosion, orchestration
  burden. Recorded but never decided on: performance, settlement
  reliability. Absent: transfer, stability. Trigger: task23.
- **AK-055 locator divergence:** `tests/proofs/p10_no_false_learning`
  materializes as `crates/gauntlet-cli/tests/p10_no_false_learning.rs`
  (the p4/p7 precedent; recorded in reference-candidates, 2026-09-03).
- **Production call sites deferred (declared deviation, charter D10):**
  first callers are the tests + proof; task23 wires the import/evaluate
  surfaces into the loop. Until then the composition is
  proof-composition, honestly labeled.

## Answer-key unit status

48 → **49 provisional through Task 22** (AK-055 now delivered: single
success does not promote; promotion requires the configured minimum
settlements under recorded variation plus a handled recovery event —
both halves proven over real adapters with personally verified teeth).
39 proposed, 0 blocked.

## Battery (final tree)

check clean · test 1436/1436 ×3 · lint clean · deny ok · boundaries
228/228 · ruler PASS · p10 4/4 · memory:: 34/34 · ports suite legs 3/3
· sqlite contract suite 3/3.
