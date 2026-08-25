# Task 12 — Builder B1 checkpoints

Working log for slice B1 (domain Spent-restore, ports owner-write capability
gating, BudgetMeter adapter). One dated entry per completed numbered brief
item; the brief mandates this because sessions die to usage limits.

## Session 2026-08-23

* Bootstrapped this file after code recon (brief, ports/domain/adapter/cli
  surfaces; `recon-code.md` named by the brief does not exist — self-served
  recon from source, noted for the report).

### Item 2 — ports owner-write capability gating (2026-08-23) — LANDED

* New `crates/gauntlet-ports/src/producer_authority.rs`:
  - `evidence_content_digest(bytes) -> Digest` (SHA-256 over raw bytes; the
    ONE production-content rule; equality with app `verify::content_digest`
    pinned by a new test in `gauntlet-app/src/verify/mod.rs` —
    `the_ports_production_content_digest_is_this_layers_rule`).
  - `ProducerAuthorityError` (typed: RunNotDurable, IterationUnsupported,
    EvidenceAbsent, EvidenceUnreadable, BundleInvalid, State passthrough).
  - `AdmittedEvidenceProduction` / `AdmittedObservationBinding` — minted
    types, NO public constructor; accessors expose the payloads.
  - `EvidenceProductionAuthority::record(run, iteration, ref, producer,
    locator, produced_at, deadline)` and
    `ObservationBindingAuthority::bind(run, iteration, bundle_ref,
    deadline)` — run-durable + declared-iteration-vs-durable-anchor
    validation (anchor rule MIRRORS app `VerificationAuthority::for_run`;
    ports cannot depend on app — cross-referenced in doc comments),
    digest DERIVED from served bytes (§12.4 canonical derivation reused for
    bundles via `crate::observation_digest`).
  - 8 in-module tests (undurable run, stale/ahead iteration + no-checkpoint
    anchor default, digest-derived-from-bytes, swapped-bytes-after-the-fact
    refused via store write-once, absent ref, canonical §12.4 binding
    digest, non-JSON bundle, minted-type payload wiring).
* `StateStore` trait: both owner writes now take the minted types;
  the recorded residual doc replaced by the gating description.
  lib.rs exports added.
* Shared `contract/state_store.rs`: new case
  `owner_writes_are_earned_through_the_authorities` (earned through the
  authorities against the adapter store; independent SHA-256 recompute;
  write-once conflicting re-record; §12.4 readback). Green stub
  `DurableStore` extended to implement both writes + readbacks (in
  `contract/stubs.rs`); broken stub unchanged (still caught on its
  intended case).
* sqlite adapter: signatures updated (payload via accessors); the four
  evidence-production tests rerouted through the authority (new
  `SwappableEvidence` test fake with after-the-fact `replace`; the
  different-digest write-once tooth now constructed by SWAPPING the bytes
  at the ref — a caller-chosen digest is no longer representable).
* CLI reroutes: p12 `Fixture::produce` → `EvidenceProductionAuthority`
  (fixture checkpoints the anchor iteration first; derived digest asserted
  equal to `content_digest(payload)`); p12 absent-store scenario
  reconstructed honestly (produce then remove the object through the fs
  adapter's documented layout); p1 + p2 (5 sites) bind through
  `ObservationBindingAuthority`; p2 rebind tooth matches
  `ProducerAuthorityError::State(WriteFailed)`; p2 absent-bundle tooth =
  bind-then-delete. Dead `binding_of` removed.
* Fail-first evidence (Item 2): after the trait change,
  `just check-package gauntlet-adapter-state-sqlite` → E0053 on both
  methods (direct-signature bypass unrepresentable) + E0308 at every
  direct call site (adapter tests, p12) — the "direct-store-call bypass is
  a compile error" confirmation, captured before rerouting.

### Item 3 — BudgetMeter adapter crate (2026-08-23) — LANDED

* New crate `crates/adapters/gauntlet-adapter-budget-meter` (registered in
  root Cargo.toml members + workspace.dependencies; deps: gauntlet-domain +
  gauntlet-ports only).
* `MeteredBudget::new(budget: Budget)`, thread-safe `Arc<Mutex<Inner>>`
  holding the domain Budget + outstanding-reservation count + §17.4
  adapter-level counters (`tool_calls_total()`, `note_retry()`,
  `retries_total()` — tracked, enforced nowhere; recorded residual).
* `reserve`: domain `would_breach_reserve` gate with typed cause derived by
  a dry charge over a CLONE; then `check_dispatch_scope(outstanding+1, 1)`
  for the parallel ceiling (domain's own scope error). `charge`:
  `ReservationExceeded` when tokens exceed the reserved slice (the port's
  refusal carries token fields — documented); books via `budget.charge`
  with rounds=0 (payments carry no rounds); releases the slot on success
  (zero-payment charge = documented release path); refused charge holds
  the slot; `tool_calls` accumulate. `remaining()`: `remaining_of`
  verbatim (the UNMODIFIED suite pins that reserve moves nothing there —
  slot availability observable only in reserve's scope refusal).
  `would_breach_reserve`: domain delegation (same formula both sides).
* Tests (7 total): the UNMODIFIED shared `budget_meter_suite` (both in-crate
  and via `tests/contract_suite.rs`), the reserve↔charge agreement pin
  (13 boundary-exact probes pre-booking + 4 post-booking, fresh meter
  pairs per probe), parallel slot hold/release/refused-charge-holds,
  multi-charge reservation with saturating release, §17.4 counter
  tracking, refused-charges-book-nothing.
* Fail-first: the UNMODIFIED suite caught my first `remaining()` design
  ("remaining changed at reserve time — only charges book spend") —
  recorded as the discriminating-suite evidence; fixed to the pinned
  projection.
* Gate: `just check-package` clean; `just test-package
  gauntlet-adapter-budget-meter` → 7 passed.
* Gates: `just test-package gauntlet-ports` → 88 passed (incl. green/broken
  suite runs); `just test-package gauntlet-adapter-state-sqlite` → 37
  passed; `just test-package gauntlet-app` → 221 passed (incl. the new
  digest pin); `just test-package gauntlet-cli` → 36 passed (p1/p2/p12
  proofs green under the gate).
* Negative-compile probe note: out-of-crate minted-value construction is a
  compile error (no public constructor) — an explicitly compiled negative
  probe is left to the confirmation round, per the brief.

### Item 1 — domain `Spent` reconstruction (2026-08-23) — LANDED

* `crates/gauntlet-domain/src/budget.rs`:
  - `Spent::from_counters(rounds, wall_seconds, tokens, cost_usd,
    diagnostic_tokens, diagnostic_cost_usd) -> Spent` (public
    value-carrying constructor, NO validation — doc names
    `Budget::restore` as the admission path).
  - `Budget::restore(limits, recovery_reserve_pct, diagnostic, spent)
    -> Result<Budget, BudgetError>` — enforces reserve range + every
    component ≤ raw limit AND ≤ forward ceiling (forward dims) / ≤
    allowance (diagnostic); typed `BudgetError::RestoreUnreachable
    {dimension, ceiling, spent}` refusal.
  - Tests added (4): `a_zero_spent_restores_to_a_fresh_budget`,
    `restore_refuses_spend_no_charge_sequence_can_produce`,
    `restore_reuses_the_new_reserve_range_rule`,
    `restore_agrees_with_replaying_the_charges` (equivalence: counters,
    `would_breach_reserve` incl. boundary, follow-up charge admission,
    diagnostic accounting).
* Fail-first: tests written first; `just check-package gauntlet-domain`
  failed with 24 errors (`restore`, `from_counters`, `RestoreUnreachable`
  absent) before implementation.
* Gate: `just test-package gauntlet-domain` → `Summary 80 tests run:
  80 passed` (76 prior + 4 new).
* Adapter note (brief allowed "switch only if strictly simpler"):
  `spent_or_refuse` non-zero refusal KEPT (resume has no ceilings in
  scope; wiring `Budget::restore` into resume = B3 controller
  composition — recorded as residual). `zero_spent()` simplified to
  `Spent::from_counters(0,…)` (strictly simpler, behavior-identical);
  module doc + refusal message updated to name the new domain path.
  `just check-package` + `just test-package gauntlet-adapter-state-sqlite`
  → 37 passed (no behavior change).

### Item 3 rename note (2026-08-23)

* The brief's crate name `gauntlet-adapter-budget-meter` trips the
  repository's own adapter-identifier denylist (`just boundaries`): the
  derived spellings `budget_meter`/`meter_budget` are the PORT's own §6.2
  vocabulary (module `budget_meter.rs`, suite name "budget_meter", and a
  pre-existing `meter_budget` local in the port's tests), so naming an
  adapter after the port makes the port's own module name a denied
  identifier — 52 violations, gate red. Weakening the derivation is a gate
  change (not mine to make); the crate is renamed to
  `gauntlet-adapter-meter-memory` (capability-substrate convention; the
  meter is "pure arithmetic over in-memory counters" per the port's own
  doc). Boundaries return to the exact baseline: 197/197.

### Item 4 — gates (2026-08-23) — ALL GREEN

* Package-scoped:
  - `just check-package` gauntlet-domain / gauntlet-ports /
    gauntlet-adapter-meter-memory / gauntlet-adapter-state-sqlite /
    gauntlet-cli — all Finished.
  - `just test-package`: gauntlet-domain 80/80, gauntlet-ports 88/88,
    gauntlet-adapter-meter-memory 7/7, gauntlet-adapter-state-sqlite 37/37,
    gauntlet-cli 36/36 (plus gauntlet-app 221/221 — the digest pin).
* Workspace: `just check` Finished; `just test` → 803/803 passed;
  `just lint` exit 0; `just fmt-check` exit 0; `just deny` exit 0
  (advisories/bans/licenses/sources ok); `just boundaries` → 197/197
  (baseline count preserved); `just ruler` → PASS (answer key and manifest
  digests match the frozen lock).
* `target` at 2.8G (ceiling ~4G). No test disabled, no assertion weakened.
