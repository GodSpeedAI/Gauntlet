# Task 12 — Builder B1 result contract

Slice B1 of 4 (brief: `.agents/reports/task12/builder-brief-b1.md`).
Status: **complete — all four items landed; every gate green.**
Checkpoints: `.agents/reports/task12/builder-b1-checkpoints.md`.
No commit made (orchestrator commits). Frozen paths untouched.

## Item summaries

### 1 — Domain `Spent` reconstruction (closes Task-6 critic-A MAJOR-2's domain half)

`crates/gauntlet-domain/src/budget.rs`: a value-carrying `Spent`
constructor plus `Budget::restore`, which refuses — with the new typed
`BudgetError::RestoreUnreachable` — any water-mark no legal charge
sequence can leave (past a raw limit, inside the recovery reserve, past
the diagnostic allowance), and restores everything else under the
constructor's own reserve-range rule. Behavioral equivalence with charge
replay is pinned (counters, `would_breach_reserve` including boundary
values, follow-up charge admission, diagnostic accounting).

### 2 — Ports capability-gating of the two owner writes (closes Task-11 C7 + Task-10 MINOR-2)

New `crates/gauntlet-ports/src/producer_authority.rs` mints
`AdmittedEvidenceProduction` / `AdmittedObservationBinding` (no public
constructor anywhere) behind `EvidenceProductionAuthority::record` and
`ObservationBindingAuthority::bind`, which perform the governed reads
(durable run; declared iteration validated against the durable anchor —
latest checkpoint's iteration, else 1) and DERIVE digests from the
evidence store's own bytes. The `StateStore` trait methods now accept
only the minted types. Shared suite, green stub, sqlite adapter, and the
p1/p2/p12 CLI proofs all earn the capability through the authorities.

### 3 — BudgetMeter adapter (plan step 3)

New crate `crates/adapters/gauntlet-adapter-meter-memory` (see
Deviation 1 for the name): `MeteredBudget` wraps the domain `Budget`
under `Arc<Mutex<…>>` with reservation-slot bookkeeping and §17.4
adapter-level counters. All arithmetic is the domain's (reserve refuses
exactly what the domain's charge would refuse; `remaining` is the port's
projection of the domain counters). Passes the UNMODIFIED shared
`budget_meter` suite.

### 4 — Gates

All green (below). Boundaries at the exact baseline count (197/197).

## New / changed public APIs (exact signatures)

Domain (`gauntlet-domain::budget`):

```rust
impl Spent {
    pub fn from_counters(rounds: u32, wall_seconds: u64, tokens: u64,
        cost_usd: MicroUsd, diagnostic_tokens: u64, diagnostic_cost_usd: MicroUsd) -> Spent
}
impl Budget {
    pub fn restore(limits: Limits, recovery_reserve_pct: u8,
        diagnostic: DiagnosticAllowance, spent: Spent) -> Result<Budget, BudgetError>
}
// BudgetError new variant:
RestoreUnreachable { dimension: BudgetDimension, ceiling: u64, spent: u64 }
```

Ports (`gauntlet_ports::producer_authority`):

```rust
pub fn evidence_content_digest(bytes: &[u8]) -> gauntlet_domain::Digest

pub enum ProducerAuthorityError {
    RunNotDurable { run_id: RunId },
    IterationUnsupported { run_id: RunId, requested: IterationNo, durable: IterationNo },
    EvidenceAbsent { evidence: EvidenceRef },
    EvidenceUnreadable { evidence: EvidenceRef, detail: String },
    BundleInvalid { detail: String },
    State(StateStoreError),
}

pub struct AdmittedEvidenceProduction { /* private */ }
impl AdmittedEvidenceProduction { pub fn production(&self) -> &EvidenceProduction }

pub struct AdmittedObservationBinding { /* private */ }
impl AdmittedObservationBinding { pub fn binding(&self) -> &ObservationBinding }

pub struct EvidenceProductionAuthority { /* Clone */ }
impl EvidenceProductionAuthority {
    pub fn new(state: Arc<dyn StateStore>, evidence: Arc<dyn EvidenceStore>) -> Self
    pub fn record(&self, run_id: &RunId, iteration_no: IterationNo,
        evidence_ref: &EvidenceRef, producer: &InstrumentIdentity,
        locator: &EvidenceLocator, produced_at: EpochSeconds, deadline: Deadline)
        -> Result<EvidenceProductionBinding, ProducerAuthorityError>
}

pub struct ObservationBindingAuthority { /* Clone */ }
impl ObservationBindingAuthority {
    pub fn new(state: Arc<dyn StateStore>, evidence: Arc<dyn EvidenceStore>) -> Self
    pub fn bind(&self, run_id: &RunId, iteration_no: IterationNo,
        bundle_ref: &EvidenceRef, deadline: Deadline)
        -> Result<(), ProducerAuthorityError>
}
```

Changed trait signatures (`gauntlet_ports::state_store::StateStore`):

```rust
fn record_evidence_production(&self, run_id: &RunId,
    production: &AdmittedEvidenceProduction, deadline: Deadline)
    -> Result<EvidenceProductionBinding, StateStoreError>;
fn bind_observation_bundle(&self, run_id: &RunId, iteration_no: IterationNo,
    binding: &AdmittedObservationBinding, deadline: Deadline)
    -> Result<(), StateStoreError>;
```

Adapter (`gauntlet_adapter_meter_memory`):

```rust
pub struct MeteredBudget { /* private inner */ }
impl MeteredBudget {
    pub fn new(budget: Budget) -> Self
    pub fn tool_calls_total(&self) -> u64
    pub fn note_retry(&self)
    pub fn retries_total(&self) -> u64
}
impl gauntlet_ports::budget_meter::BudgetMeter for MeteredBudget { /* reserve/charge/remaining/would_breach_reserve */ }
```

Suite additions (`gauntlet_ports::contract::state_store`): new case
`owner_writes_are_earned_through_the_authorities` (run by
`state_store_suite`, so every StateStore adapter now runs it).

## Files touched

- `crates/gauntlet-domain/src/budget.rs` (Item 1 + tests)
- `crates/gauntlet-ports/src/producer_authority.rs` (new; Item 2 + tests)
- `crates/gauntlet-ports/src/state_store.rs` (signatures, docs)
- `crates/gauntlet-ports/src/lib.rs` (module + re-exports)
- `crates/gauntlet-ports/src/contract/state_store.rs` (new suite case + in-suite evidence store)
- `crates/gauntlet-ports/src/contract/stubs.rs` (green DurableStore implements the two writes + readbacks)
- `crates/gauntlet-app/src/verify/mod.rs` (digest-equality pin test only)
- `crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs` (signatures; tests rerouted via authority; zero-spent simplification; docs) and `src/encode.rs` (doc only)
- `crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs` (produce reroute; honest absent-store restructure)
- `crates/gauntlet-cli/tests/p1_reference_integrity.rs`, `p2_context_isolation.rs` (binds earned via authority; absent-bytes teeth reconstructed bind-then-delete)
- `crates/adapters/gauntlet-adapter-meter-memory/{Cargo.toml,src/lib.rs,tests/contract_suite.rs}` (new)
- Root `Cargo.toml` (workspace members + workspace.dependencies), `Cargo.lock`

## Fail-first evidence

1. Item 1: tests written first; `just check-package gauntlet-domain` →
   24 errors (`Budget::restore`, `Spent::from_counters`,
   `RestoreUnreachable` absent) before implementing.
2. Item 2 gate: after the trait change, `just check-package
   gauntlet-adapter-state-sqlite` → E0053 on both methods + E0308 at
   every direct call site (adapter tests, p12) — the direct-store-call
   bypass is now a compile error, captured before rerouting.
3. Item 3: the UNMODIFIED shared suite caught my first `remaining()`
   design ("remaining changed at reserve time — only charges book
   spend") — fixed to the pinned projection; the suite discriminated.
4. Item 4: `just boundaries` caught the brief's crate name (Deviation 1)
   with 52 adapter-identifier violations — fixed by rename, gate untouched.

## Gate outputs (command → tail)

- `just check-package` × {gauntlet-domain, gauntlet-ports,
  gauntlet-adapter-meter-memory, gauntlet-adapter-state-sqlite,
  gauntlet-cli} → `Finished dev profile …` each.
- `just test-package` → gauntlet-domain `80 tests run: 80 passed`;
  gauntlet-ports `88 … 88 passed`; gauntlet-adapter-meter-memory
  `7 … 7 passed`; gauntlet-adapter-state-sqlite `37 … 37 passed`;
  gauntlet-cli `36 … 36 passed`; (extra) gauntlet-app `221 … 221 passed`.
- `just check` → `Finished dev profile [unoptimized] target(s) in 16.96s`.
- `just test` → `Summary [14.717s] 803 tests run: 803 passed, 0 skipped`.
- `just lint` → exit 0. `just fmt-check` → exit 0.
- `just deny` → exit 0 (`advisories ok, bans ok, licenses ok, sources ok`).
- `just boundaries` → `Summary [13.231s] 197 tests run: 197 passed` —
  the exact baseline count, no suite-count drift (the arch tests count
  their own cases, not suite cases).
- `just ruler` → `verify-ruler.sh: PASS — answer key and manifest
  digests match the frozen lock`.

## Deviations from the brief, with reasons

1. **Crate renamed**: brief said `gauntlet-adapter-budget-meter`; landed
   as `gauntlet-adapter-meter-memory`. Reason: the brief's name trips the
   repository's adapter-identifier denylist (derived `budget_meter`/
   `meter_budget` are the PORT's own §6.2 vocabulary — the ports module
   file, the suite name, and a pre-existing test local), 52 violations,
   `just boundaries` red. Renaming the adapter (capability-substrate
   convention; the port's own doc calls the meter "pure arithmetic over
   in-memory counters") preserves the gate; weakening the derivation
   would have been papering over. Plan text does not pin the name.
2. **`Spent::from_counters` is PUBLIC, not private** as the brief's
   sketch suggested: the sqlite decode path (and any resume composition)
   must build a `Spent` from durable counters before `Budget::restore`
   can admit it. Unforgeability is not `Spent`'s invariant (it is inert
   data); the admission gate is `restore`, which is the only public path
   from raw numbers to a non-zero-spend `Budget`.
3. **Adapter non-zero-spend refusal kept** (brief allowed switching
   `spent_or_refuse` to `restore` "only if strictly simpler"): `resume`
   has no ceilings in scope, so admission via `restore` requires loading
   the run's durable budget — that is B3's controller composition, not a
   simplification. Zero path simplified to the new constructor
   (behavior-identical); docs updated to name the domain path.
4. **p1/p2 CLI tests updated beyond the p12-only reroute the brief
   named**: the trait signature change makes their direct calls compile
   errors; earning via the authorities is the minimal compliant fix. Two
   adversarial substrates were reconstructed honestly (bind-then-delete
   the stored object through the fs adapter's documented content-addressed
   layout) because binding bytes that never existed is now
   unrepresentable — which is the point of the gate.
5. **`gauntlet-app` touched (one test)**: the brief's digest-rule
   discipline (item 2.4) mandates pinning ports↔app digest equality with
   a test; ports cannot see app, so the pin lives in
   `gauntlet-app/src/verify/mod.rs` (test-only, no production change).
6. **`recon-code.md` does not exist** (the brief and IN-FLIGHT reference
   it); recon was self-served from source. No blocker resulted.

## Residuals / open questions for critics

1. **Anchor-rule duplication**: `producer_authority::durable_iteration`
   mirrors `VerificationAuthority::for_run`'s anchor (ports cannot depend
   on app). Cross-referenced in doc comments on both sides' merit; a
   future anchor-rule change must move both. A shared home would need the
   rule to live in ports and app to delegate (app-layer change, out of
   B1 scope).
2. **§17.4 tool-call/retry dimensions unenforceable at this layer**
   (brief-mandated residual): domain `Limits` has no tool-call/retry
   ceilings and the ports `Remaining` shape is frozen; the adapter tracks
   both as counters (`tool_calls_total`, `note_retry`/`retries_total`)
   and enforces nothing. Needs a ports/domain revision; not widened here.
3. **Reservation identity unprovable**: the port's `Reservation` is
   publicly constructible, so `BudgetMeterError::UnknownReservation` is
   not detectable by this (or any honest) adapter; the reservation's
   token bound and the domain's charge arithmetic are what actually bind.
   The port's `ReservationExceeded` carries only token fields, so the
   reservation bound is expressed where the port can express it
   (tokens); wall/cost ride the domain's cumulative arithmetic (same
   choice as the suite's reference stub).
4. **Repeated charges against one reservation** release at most one
   parallelism slot (the counter saturates); the port offers no
   reservation identity to make release once-only. Documented in the
   adapter module doc.
5. **Non-zero `budget_spent` resume still gated in the adapter**
   (Deviation 3): composing `Budget::restore` with the run's durable
   budget inside `resume` belongs to B3's controller composition; the
   domain path now exists and is tested.
6. **Out-of-crate negative compile probe not run**: constructing
   `AdmittedEvidenceProduction`/`AdmittedObservationBinding` outside
   `gauntlet-ports` is a compile error by construction (no public
   constructor); an explicitly compiled negative probe is the
   confirmation round's job (the brief says as much). The in-crate E0053/
   E0308 evidence above is the builder-side confirmation.
7. **`p12` produce now writes an anchor checkpoint at the production's
   iteration** (the controller's own discipline) before the authority
   call — required because the authority validates the declared iteration
   against the durable anchor. Test outcomes unchanged (37/36 suites
   green); critics may want to confirm the stale-evidence scenarios still
   construct iteration-1-then-anchor-2 histories, which they do.
