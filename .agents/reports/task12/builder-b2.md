# Task 12 — Builder B2 result contract

Slice B2 of 4 (brief: `.agents/reports/task12/builder-brief-b2.md`).
Status: **complete — Items 1–4 landed, every gate green** (see Recovery
note: B2 spanned two builder sessions; the second reconciled the tree,
fixed two defects the first session left, and ran every gate personally).
Checkpoints: `.agents/reports/task12/builder-b2-checkpoints.md`.
No commit made (orchestrator commits). Frozen paths untouched
(`git status` shows no edits under `.wayfinder/**`, `.agents/specs/**`,
`.agents/plans/**`, AGENTS.md, justfile, deny.toml, CURRENT_STATUS.md,
prereg files, task12 recon/adjudication files, `.agents/prompts/**`,
`.tmp/**`, `.jolli/**`, gauntlet-arch-tests).

## Item summaries

### 1 — `control/scheduler.rs` (UnitScheduler, spec §11.2/§11.3 + Task-7 F7)

Pure decision logic over injected facts (`PlanInput`); the purity contract
is documented on the type (B3 supplies fresh facts per §11.4 tick). All
six §11.2 conjuncts checked in spec bullet order, each refusal a typed
`Ineligible` variant carrying the blocking fact: status/claim, every
`depends_on` settled-or-out_of_scope, `blast_radius != irreversible`
(scheduler-level, unconditional, §17.1), global + per-tier concurrency (a
tier with NO capacity entry is unadmitted per §17.3, never unlimited), a
configured builder runner, and the budget conjunct decided by asking the
injected `&dyn BudgetMeter.would_breach_reserve` AT CALL TIME — never a
caller-precomputed bool. Missing dispatch estimate is a typed input error
(`SchedulerError::MissingEstimate`), never an assumed zero. `plan(...)`
validates input shape first (duplicate units, estimates, and the Task-7
F7 depends_on graph: unknown target ⇒ `UnknownDependency` naming it;
cycle ⇒ `DependencyCycle` naming the path start-to-start via three-color
DFS), then orders by ascending blast-radius risk → dependency depth
ASCENDING (dependencies strictly shallower than dependents, so ascending
respects `depends_on` "ordering only" while keeping the spec's leading
"ascending" key; grounded in the code comment) → UnitId lexicographic;
permutation determinism pinned by test. §11.3 slicing: cohort-keyed
eligible Units inherit the HIGHEST blast radius and MOST RESTRICTIVE
authority via the domain's public `blast_rank`/`authority_rank` (the one
lattice, also used by `Unit::retier` and pinned against the §5.3.2
coupling matrix); never mix irreversible members — unreachable through
`plan` (eligibility refuses them first) and guarded anyway
(`IrreversibleSliceMember`). 13 tests incl.: open irreversible unit never
eligible and never sliced (with a cohort partner); unsettled dependency
blocks and settled/out_of_scope releases; per-tier + global concurrency
accounting; meter refusal excludes eligibility (RealMeter over a real
domain `Budget`, 800-token boundary exact); shuffle determinism; slice
inheritance both axes.

### 2 — `control/select.rs` (CorrectionSelector) + domain `correction.rs` + durable record (AK-037)

Domain aggregate `crates/gauntlet-domain/src/correction.rs`:
`SelectionDimension` (the ten §15.1 dimensions exactly, `ALL` const,
test-pinned to the spec list), `CorrectionShape {Bounded, Speculative}`,
`BudgetFacts` (requested+remaining, four dimensions),
`CorrectionDescriptor::new` (non-empty summary, ≥1 unit ref, shape,
estimate), `RejectionClass {Authority, Tier, Budget, Irreversibility,
NotSelected}` + `is_governance()`, `RejectionReason` (typed facts:
`AuthorityAboveCeiling{required, exercisable}`, `TierNotDispatchable`,
`MeterRefusedReserve{facts: BudgetFacts}`, `IrreversibleTier`,
`LowerPriority{decided_by}`, `SpeculativeWhenBoundedAvailable`),
`RejectedAlternative::new` + `class_matches_reason`, `DecisionStep
{dimension, note}`, and `CorrectionSelection::new` which REFUSES: a trace
not naming all ten dimensions (`TraceIncomplete{missing}`), and any
(class, reason) mismatch (`ClassReasonMismatch`). Constructing a selection
without alternatives-with-reasons is unrepresentable — AK-037 is structural.

`CorrectionSelector::select(&SelectionContext)` — inputs are typed facts
only (candidates carry `required_authority`, `tier`, `dependency_ready`,
`settlement`, `Impact{worst_severity, residuals_addressed}`,
`InformationGain`, `Confidence`, `units_touched`, `rollback_units`;
context carries run/iteration, `&dyn BudgetMeter`,
`exercisable_authority`, `dispatchable_tiers`). Step 1 rejects governance
blocks FIRST in the spec's parenthetical order (authority → tier → budget
→ irreversibility); budget blocks ask `would_breach_reserve` at call time
and record the meter's own `remaining()` numbers. Step 2/3 rank the
admissible by an explicit lexicographic priority — shape (bounded first,
recorded under risk) → dependency order → settlement accessibility →
impact (severity then count; "it does not simply select the largest
residual") → information gain → confidence → reversibility → risk →
payment → recovery cost, with deterministic lead-unit-id then summary
tie-breaks; every dimension referenced by name in the trace (constructor
enforced). `SelectionError::NoAdmissibleCandidate{rejected}` carries
every rejection so a fully-blocked selection escalates WITH its evidence.
6 tests incl. `controller_records_alternatives` (in-app FakeStore
round-trip through the port), governance-block never chosen even when
highest-impact, budget-block records meter numbers (pinned equal to
`meter.remaining()`; 800/801 boundary), bounded-over-speculative,
tie-break determinism/priority order, empty/fully-blocked typed errors.

**Durable half:** `StateStore::record_correction_selection` +
`correction_selections` (ports), following the `record_role_invocation`
precedent — the argument is the exact domain aggregate; §16.4 single-writer
justification on the port doc (the RUNTIME is the writer; no minting
authority to earn, unlike the role-owned evidence/observation writes).
Adapter `RecordKind::CorrectionSelection` ("correction_selection");
SQLite CHECK cannot be ALTERed, so migration `0008` rebuilds the records
table with the CHECK widened by exactly one token, rows copied with seq
preserved (append-only migration; migrations pin updated 1..=8 — a
recorded act). Strict JSON codec decodes only through the domain
constructors (damaged row ⇒ `Corrupt`, tested). Shared-suite case
`correction_selection_round_trip` runs for every StateStore adapter;
green stub stores values. Event log NOT chosen as carrier (see
Deviations).

### 3 — `control/diagnosis.rs` (DiagnosticEngine, spec §15.2 + §11.5, AK-039)

`FailureLocus` — the sixteen §15.2 loci verbatim (`ALL`), with derived
`APPARATUS` (the six measuring-apparatus loci) and `is_apparatus()`.
`NonconvergenceDetector` — trigger keyed on the semantic projection
`ResidualKey {unit, dimension, severity}`, NOT the raw `ResidualId`
(fresh per comparison; the literal reading would never fire —
preregistered concern, resolved as a requirement): `observe` counts per
key and fires at threshold; N-1 does not fire; fresh ids under the same
projection DO; different dimension/class does not accumulate;
idempotent past threshold; `threshold()==0` refused
(`DiagnosisError::ThresholdZero` — counting cannot reach zero).
`DiagnosticEngine::run_cycle(evidence, &mut Budget, observation,
&dyn HumanEscalation, ctx)` — one bounded cycle, five phases (diagnosis →
prediction → cheapest discriminating test → disconfirming observation →
correction), each phase charging `DiagnosticSpend` via the domain
`Budget::charge_diagnostic` (no second unit invented; config's
`diagnostic_budget` scalar maps to the domain allowance at composition —
B3). Exhaustion at ANY phase ⇒ `DiagnosisOutcome::Inconclusive` with a
typed `QuarantineRecord {run_id, evidence_refs, locus, reason}` AND an
`EscalationItem` submitted through the `HumanEscalation` port (recording
fake in tests; escalation refusal ⇒ `DiagnosisError::EscalationFailed`
carrying the quarantine — neither fact dropped). A hypothesis that
survives its ONE disconfirming observation is INCONCLUSIVE the same way
(G16: no re-ask path exists; boundedness by construction — the engine's
one-call shape has no loop; `CyclePhase::ALL` pins five phases).
Locus selection (`DiagnosticEvidence::selected_locus`) is input-driven
with the selecting evidence recorded, precedence: reference integrity →
apparatus (six) → tooling/environment/payment/capability/integration/
architecture/governance → implementation LAST; empty evidence selects no
locus (nothing invented). 14 tests incl. the three trigger-boundary
tests, apparatus-shaped input not routing to implementation_defect, all
seven non-implementation loci outranking implementation (added this
session — see Recovery), exhaustion ⇒ INCONCLUSIVE + quarantine +
escalation, surviving-hypothesis ⇒ INCONCLUSIVE, zero-spend engine
refused, cycle boundedness.

### 4 — config knob (§11.5 only)

`defaults.rs`: `pub const NONCONVERGENCE_N: u32 = 3` (inline §11.5
default, spec line cited in comment; deliberately NOT in
`DEFAULTS_TABLE_SETTINGS` — it is not a §8.6 defaults-table row, so no
walk.rs inventory change was warranted). `resolve.rs`:
`ConfigKey::NonconvergenceN` (env `GAUNTLET_NONCONVERGENCE_N`, env-only
like `mcp_required`), zero refused as `ValueOutOfRange` (a counting
trigger cannot fire at zero), surfaced on `EffectiveConfig` and
`ResolvedConfig`. `canonical.rs` carries `"nonconvergence_n"` both ways
(round-trip exact). Retry base/max, stall timeout, attempts-per-unit,
poll interval NOT invented — left for B3 as the brief directs;
`env_example_complete` green (arch envfile suite 14/14, part of
boundaries).

### 5 — gates

All green; outputs under "Gate outputs" below. Boundaries at the exact
baseline 197/197.

## New / changed public APIs (exact signatures)

`gauntlet_app::control::scheduler`:

```rust
pub struct Concurrency { pub global_capacity: u32, pub global_in_flight: u32,
    pub tier_capacity: Vec<(BlastRadius, u32)>, pub tier_in_flight: Vec<(BlastRadius, u32)> }
impl Concurrency { pub fn tier_capacity(&self, tier: BlastRadius) -> Option<u32>
                   pub fn tier_in_flight(&self, tier: BlastRadius) -> u32 }
pub struct PlanInput { pub units: Vec<Unit>, pub claimed: Vec<(UnitId, InvocationId)>,
    pub concurrency: Concurrency, pub configured_roles: Vec<RoleId>,
    pub cohorts: BTreeMap<UnitId, CohortId>, pub estimates: BTreeMap<UnitId, BudgetSlice> }
pub struct CohortId; impl CohortId { pub fn new(cohort: &str) -> Result<Self, SchedulerError>
                                          pub fn as_str(&self) -> &str }
pub enum Ineligible { StatusNotOpen{unit,status} | Claimed{unit,invocation}
  | DependencyUnresolved{unit,dependency,status:Option<UnitStatus>} | Irreversible{unit}
  | ConcurrencyGlobal{unit,capacity,in_flight} | ConcurrencyTier{unit,tier,capacity,in_flight}
  | TierConcurrencyUnconfigured{unit,tier} | RunnerUnconfigured{unit,role}
  | EstimateMissing{unit} | BudgetReserveBreach{unit,slice:BudgetSlice} } // + Display
pub enum SchedulerError { UnknownDependency{unit,dependency} | DependencyCycle{cycle:Vec<UnitId>}
  | DuplicateUnit{unit} | MissingEstimate{unit} | CohortInvalid{cohort:String}
  | IrreversibleSliceMember{unit} } // + Display + Error
pub struct PlannedDispatch { pub units: Vec<UnitId>, pub blast_radius: BlastRadius,
    pub authority: Authority, pub estimated_slice: BudgetSlice, pub depth: u32 }
impl PlannedDispatch { pub fn single(...) -> Self; pub fn is_slice(&self) -> bool }
pub struct UnitScheduler;
impl UnitScheduler {
    pub fn new() -> Self;
    pub fn eligible(&self, unit: &Unit, input: &PlanInput, meter: &dyn BudgetMeter)
        -> Result<(), Ineligible>;
    pub fn plan(&self, input: &PlanInput, meter: &dyn BudgetMeter)
        -> Result<Vec<PlannedDispatch>, SchedulerError>;
}
```

`gauntlet_app::control::select`:

```rust
pub struct Impact { pub worst_severity: Severity, pub residuals_addressed: u32 }
pub enum SettlementAccessibility { Instrument, HumanSignoff, Missing }
pub enum Confidence { LastSucceeded, NeverAttempted, FailedOnce, RepeatedFailure }
pub enum InformationGain { None, Incremental, Decisive }
pub struct Candidate { pub correction: CorrectionDescriptor,
    pub required_authority: Authority, pub tier: BlastRadius, pub dependency_ready: bool,
    pub settlement: SettlementAccessibility, pub impact: Impact,
    pub information_gain: InformationGain, pub confidence: Confidence,
    pub units_touched: u32, pub rollback_units: u32 }
pub struct SelectionContext<'a> { pub run_id: RunId, pub iteration_no: IterationNo,
    pub meter: &'a dyn BudgetMeter, pub exercisable_authority: Authority,
    pub dispatchable_tiers: Vec<BlastRadius>, pub candidates: Vec<Candidate> }
pub enum SelectionError { NoCandidates, NoAdmissibleCandidate{rejected: Vec<RejectedAlternative>},
    Domain(gauntlet_domain::CorrectionError) } // + Display + Error
pub struct CorrectionSelector;
impl CorrectionSelector { pub fn new() -> Self;
    pub fn select(&self, context: &SelectionContext<'_>) -> Result<CorrectionSelection, SelectionError> }
```

`gauntlet_app::control::diagnosis`:

```rust
pub enum FailureLocus { /* sixteen, §15.2 verbatim */ ..=GovernanceConflict }
impl FailureLocus { pub const ALL: &'static [Self]; pub const APPARATUS: &'static [Self];
    pub fn is_apparatus(self) -> bool } // + Display
pub struct ResidualKey { pub unit: UnitId, pub dimension: String, pub severity: Severity } // Ord
pub struct RecurringResidual { pub id: ResidualId, pub key: ResidualKey }
pub struct Nonconvergence { pub key: ResidualKey, pub occurrences: u32 }
pub struct NonconvergenceDetector;
impl NonconvergenceDetector {
    pub fn new(threshold: u32) -> Result<Self, DiagnosisError>;
    pub fn threshold(&self) -> u32;
    pub fn observe(&mut self, residual: &RecurringResidual) -> Option<Nonconvergence>;
    pub fn occurrences(&self, key: &ResidualKey) -> u32;
}
pub struct DiagnosticSpend { pub tokens: u64, pub cost_usd: MicroUsd }
pub struct DiagnosticEvidence { /* sixteen bool facts + discriminating_test: Option<String>
                                  + quarantined_evidence: Vec<EvidenceRef> */ }
impl DiagnosticEvidence { pub fn selected_locus(&self) -> Option<(FailureLocus, Vec<String>)> }
pub struct DisconfirmingObservation { pub disconfirmed: bool }
pub struct QuarantineRecord { pub run_id: RunId, pub evidence_refs: Vec<EvidenceRef>,
    pub locus: Option<FailureLocus>, pub reason: String }
pub enum DiagnosisOutcome { CorrectionIndicated{locus, correction_hint:String},
    Inconclusive{quarantine: QuarantineRecord} }
pub struct DiagnosisReport { pub locus: Option<FailureLocus>,
    pub selecting_evidence: Vec<String>, pub prediction: Option<String>,
    pub discriminating_test: Option<String>, pub phases_completed: u32,
    pub spend_booked: DiagnosticSpend, pub outcome: DiagnosisOutcome }
pub struct EscalationContext { pub run_id: RunId, pub context_ref: EvidenceRef, pub deadline: Deadline }
pub enum DiagnosisError { ThresholdZero, SpendZero,
    EscalationFailed{quarantine: QuarantineRecord, cause: HumanEscalationError} } // + Display + Error
pub enum CyclePhase { Diagnosis, Prediction, CheapestDiscriminatingTest,
    DisconfirmingObservation, Correction } // ALL const
pub struct DiagnosticEngine;
impl DiagnosticEngine { pub fn new(spend: DiagnosticSpend) -> Result<Self, DiagnosisError>;
    pub fn run_cycle(&self, evidence: &DiagnosticEvidence, budget: &mut Budget,
        observation: &DisconfirmingObservation, escalation: &dyn HumanEscalation,
        escalation_ctx: &EscalationContext) -> Result<DiagnosisReport, DiagnosisError> }
```

`gauntlet_domain::correction` (new module, re-exported at domain root):

```rust
pub enum SelectionDimension { Impact, DependencyOrder, Confidence, Payment, Reversibility,
    Risk, GovernanceStatus, SettlementAccessibility, RecoveryCost, InformationGain }
impl SelectionDimension { pub const ALL: &'static [Self]; pub fn name(self) -> &'static str }
pub enum CorrectionShape { Bounded, Speculative } // + Display
pub struct BudgetFacts { pub requested_rounds: u32, pub requested_wall_seconds: u64,
    pub requested_tokens: u64, pub requested_cost_usd: MicroUsd, pub remaining_rounds: u32,
    pub remaining_wall_seconds: u64, pub remaining_tokens: u64, pub remaining_cost_usd: MicroUsd }
pub struct CorrectionDescriptor; // accessors: summary()/shape()/unit_refs()/estimated()
impl CorrectionDescriptor { pub fn new(summary: String, shape: CorrectionShape,
    unit_refs: Vec<UnitId>, estimated: BudgetFacts) -> Result<Self, CorrectionError> }
pub enum RejectionClass { Authority, Tier, Budget, Irreversibility, NotSelected }
impl RejectionClass { pub fn is_governance(self) -> bool } // + Display
pub enum RejectionReason { AuthorityAboveCeiling{required: Authority, exercisable: Authority}
    | TierNotDispatchable{tier: BlastRadius} | MeterRefusedReserve{facts: BudgetFacts}
    | IrreversibleTier{tier: BlastRadius} | LowerPriority{decided_by: SelectionDimension}
    | SpeculativeWhenBoundedAvailable }
pub struct RejectedAlternative; // accessors correction()/class()/reason()/class_matches_reason()
impl RejectedAlternative { pub fn new(correction: CorrectionDescriptor, class: RejectionClass,
    reason: RejectionReason) -> Self }
pub struct DecisionStep; // dimension()/note()
impl DecisionStep { pub fn new(dimension: SelectionDimension, note: String) -> Self }
pub enum CorrectionError { TraceIncomplete{missing: Vec<SelectionDimension>}
    | ClassReasonMismatch{class: RejectionClass, summary: String}
    | DescriptorIncomplete{field: &'static str, detail: String} } // + Display + Error
pub struct CorrectionSelection; // run_id()/iteration_no()/chosen()/rejected()/trace()
impl CorrectionSelection { pub fn new(run_id: RunId, iteration_no: IterationNo,
    chosen: CorrectionDescriptor, rejected: Vec<RejectedAlternative>, trace: Vec<DecisionStep>)
    -> Result<Self, CorrectionError> }
```

Changed trait `gauntlet_ports::state_store::StateStore`:

```rust
fn record_correction_selection(&self, selection: &CorrectionSelection, deadline: Deadline)
    -> Result<(), StateStoreError>;   // default: WriteFailed("does not implement...")
fn correction_selections(&self, run_id: &RunId, deadline: Deadline)
    -> Result<Vec<CorrectionSelection>, StateStoreError>;  // default: Ok(Vec::new())
```

Domain `unit.rs`: `pub fn blast_rank(BlastRadius) -> u8` and
`pub fn authority_rank(Authority) -> u8` made public (one lattice for
retier + §11.2 ordering + §11.3 inheritance; §5.3.2 coupling pinned);
re-exported in domain lib.

Config: `pub const NONCONVERGENCE_N: u32` (defaults),
`ConfigKey::NonconvergenceN`, `EffectiveConfig::nonconvergence_n`,
`ResolvedConfig::nonconvergence_n` (env `GAUNTLET_NONCONVERGENCE_N`),
canonical key `"nonconvergence_n"`.

## Files touched (B2 total, both sessions)

- `crates/gauntlet-app/src/control/{mod,scheduler,select,diagnosis}.rs` (new)
- `crates/gauntlet-app/src/lib.rs` (`pub mod control;`, re-export NONCONVERGENCE_N)
- `crates/gauntlet-domain/src/correction.rs` (new), `src/unit.rs` (ranks public), `src/lib.rs`
- `crates/gauntlet-ports/src/state_store.rs` (two methods + docs),
  `src/contract/state_store.rs` (suite case), `src/contract/stubs.rs` (green stub)
- `crates/adapters/gauntlet-adapter-state-sqlite/src/{state_store.rs,migrations.rs,encode.rs}` + `migrations/0008_correction_selection_records.sql` (new)
- `crates/gauntlet-app/src/config/{defaults,resolve,canonical}.rs`
- Root `Cargo.toml`/`Cargo.lock` are B1's meter-crate changes (unchanged this slice)

## Fail-first evidence

1. **Prior session** (from checkpoints, verified against the tree):
   domain item — 24 compile errors before implementing restore-adjacent
   types (B1) / matrix test initially wrong expectation (B2 domain base);
   scheduler pass 1 without graph validation/depth — exactly 4 red
   (dangling depends_on, cycle, depth ordering, missing-estimate);
   diagnosis raw-ResidualId keying — exactly the 3 trigger tests red;
   sqlite contract suite RED before the writer ("this state store does
   not implement durable correction selections"), green after.
2. **This session**:
   - Locus precedence: new test failed `left: ImplementationDefect,
     right: ToolDefect` before the array reorder; green after.
   - Arch gates: `just test` 840/844 and `just boundaries` 195/197 RED on
     the prior session's tree (denylist: cohere/sqlite; literals: 68)
     — evidence the prior session never completed Item 5 — green 844/844
     and 197/197 after fixing only B2 files.

## Gate outputs (command → tail; run this session, post-fix)

- `just check-package` × {gauntlet-domain, gauntlet-ports,
  gauntlet-adapter-state-sqlite, gauntlet-app} → `Finished dev profile`
  each.
- `just test-package` → gauntlet-domain `85 tests run: 85 passed`;
  gauntlet-ports `88 … 88 passed`; gauntlet-adapter-state-sqlite
  `38 … 38 passed`.
- `cargo nextest run -p gauntlet-app control::` → `33 tests run: 33
  passed` (32 prior + 1 added this session); whole app package `256 …
  256 passed`.
- `just check` → `Finished dev profile [unoptimized] target(s) in 9.08s`.
- `just test` → `Summary [15.548s] 844 tests run: 844 passed, 0 skipped`.
- `just lint` → exit 0 (clippy `--workspace --all-targets -D warnings` +
  `cargo fmt --check`). `just fmt-check` → exit 0.
- `just deny` → exit 0 (`advisories ok, bans ok, licenses ok, sources ok`).
- `just boundaries` → `Summary [11.755s] 197 tests run: 197 passed` —
  exact baseline count (includes the denylist and literals arch tests
  this session fixed, plus envfile 14/14).
- `just ruler` → `verify-ruler.sh: PASS — answer key and manifest digests
  match the frozen lock`.
- `du -sh target` → 2.8G (under the ~4 GiB build-cache budget).

## Recovery note (two sessions, honestly recorded)

The first B2 session (2026-08-24) landed Items 1–4 and died mid-Item-5
with NO gate results. The second session (this contract's author)
reconciled `git status` against the checkpoints (all claims matched;
nothing redone), then found and fixed two latent defects before gating:
(1) `selected_locus` ordered the loci in §15.2 list order, betraying its
own documented "implementation LAST" precedence — fixed fail-first;
(2) the landed code had never passed the workspace arch gates (26
denylist violations: `cohere`/`sqlite` substrings; 68 hardcoded-literal
violations: inline duration values, port-valued 3000, epoch ticks next
to `deadline`, plus scanner statement-joins pulling bare numbers into
hinted windows) — fixed with the repository's named-const idiom in the
B2 files only; no gate, list, or threshold was touched.

## Deviations from the brief, with reasons

1. **Records table, not the event log, carries the selection** (brief
   allowed event-log deviation with justification): kept the
   `record_role_invocation` precedent — one row per immutable selection,
   read side returns them in append order; §18 replay reads the event
   LOG for run state, while §15.1 step 4's obligation is a per-iteration
   RECORD, and §16.4's single-writer discipline is satisfied by the
   runtime being the only writer (documented on the port). The event log
   remains free for B3 to ALSO append a projection if the loop needs
   replay visibility; no conflation today.
2. **The §15.1 parenthetical order means the irreversible-vs-authorality
   class attribution follows the FIRST refusing gate**: an irreversible
   candidate needing human authority over a lead-agent run records
   `Authority` (authority is checked first), not `Irreversibility`.
   Tests pin both behaviors; the order is the spec's own parenthetical
   "(authority, tier, budget, irreversibility)".
3. **Migration 0008 rebuilds the records table** (CHECK cannot be
   ALTERed in SQLite): identical columns, CHECK widened by exactly
   `'correction_selection'`, rows copied with seq preserved — the
   append-only discipline is preserved by the copy, and the migrations
   count pin moved 1..=7 → 1..=8 as a recorded act (test updated with
   the migration, per the suite's own precedent).
4. **Quarantine persistence is composition's write (B3)**: §15.2's
   quarantine is a typed `QuarantineRecord` handed to the escalation
   path (and carried inside `EscalationFailed`), not a store row — the
   engine has no store by purity contract; B3 wires persistence beside
   the escalation. Recorded residual, not silently dropped.
5. **`NONCONVERGENCE_N` not added to walk.rs
   DEFAULTS_TABLE_SETTINGS**: it is not a §8.6 defaults-table row (§11.5
   inline default); adding it would have been an inventory drift without
   spec backing. The const's comment cites the §11.5 line.
6. **Test-literal rewrites this session** (the named-const idiom) changed
   one value: `remaining_wall_seconds` 3000→2700 in test facts (3000 is
   a well-known-port literal the arch gate forbids); all round-trip
   tests are value-agnostic, and the load-bearing meter arithmetic
   (1000-token ceiling, 20% reserve ⇒ 800 forward; 800/801 boundary) is
   unchanged.

## Residuals / open questions for critics

1. **Scheduler concurrency input is caller-assembled** (`Concurrency`
   with tier lists keyed by tier): B3 must assemble it from policy
   files; a missing tier entry is "unadmitted" (§17.3) — confirm the
   composition maps §17.3 policy presence correctly (no policy for
   `irreversible` is REQUIRED; the test fixture deliberately carries an
   irreversible tier entry to exercise non-mixing, not to admit it).
2. **`PlannedDispatch.estimated_slice` sums member estimates with
   saturating arithmetic** — a sum that saturates silently understates;
   the meter conjunct decided per-Unit BEFORE slicing keeps the dispatch
   decision conservative, but B3's dispatch admission should re-consult
   the meter on the summed slice (§17.4) before starting the process.
3. **Selector's payment comparison uses the descriptor's requested
   estimate only** (tokens then rounds/wall/cost lexicographic within
   the payment dimension — see `compare`); §17.4's "payment includes
   verification cost and orchestration burden" wants B3 to fold those
   into the estimates it assembles, not into the selector.
4. **DiagnosticEvidence is a flat fact struct** (sixteen booleans + two
   optionals): input-driven and inspectable, but the caller (B3) decides
   which facts to set from real instrument outcomes; a wrong fact map
   yields a wrong locus — the precedence at least guarantees
   implementation is never chosen while any other fact is set.
5. **`confidence::RepeatedFailure` ranks below NeverAttempted but does
   not itself trigger diagnosis** — the §11.5 non-convergence detector
   is the routing mechanism; B3 must wire detector output to bypass
   selection (both mechanisms exist; the wiring is composition).
6. **One residual from B1 persists here**: the §17.4 tool-call/retry
   dimensions remain adapter-counters (B1 Residual 2); the selector and
   scheduler express affordability in the port's `BudgetSlice` shape
   only.
