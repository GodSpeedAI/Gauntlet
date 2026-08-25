# Task 12 — Builder slice B1 brief

You are a builder agent in /home/sprime01/projects/gauntlet (WSL2, 6 cores,
7.8 GiB RAM — memory-constrained). Read `AGENTS.md` first; it binds you.
Then read `.agents/CURRENT_STATUS.md` ("TL;DR for the next agent" + Task-10/11
closure records), `.agents/reports/task12/recon-code.md` (code inventory with
file:line refs — trust it, verify locally), and this brief.

Task 12 overall (`.agents/plans/gauntlet-impl.md` Task 12): controller —
UnitScheduler, CorrectionSelector, BudgetMeter adapter, non-convergence
diagnosis, proofs p4/p6 (AK-037/038/039/051/052). You are building slice B1
of four (B2 control core, B3 controller composition, B4 proofs follow).
Do NOT implement B2–B4 surfaces.

## Hard rules

- **Never edit**: `.wayfinder/**`, `.agents/specs/**`, `.agents/plans/**`,
  `AGENTS.md`, `justfile`, `deny.toml`, `.agents/CURRENT_STATUS.md`,
  `.agents/research/preregistrations/*` (frozen),
  `.agents/reports/task12/{recon-*,residual-adjudication,IN-FLIGHT}.md`,
  `.agents/prompts/**`, `.tmp/**`, `.jolli/**`. Do not commit — the
  orchestrator commits.
- **You hold the single-compiler build lock.** Run one Cargo command at a
  time; package-scoped recipes before workspace ones; do NOT raise build
  jobs/test threads; do NOT `cargo clean`; watch `du -sh target` (ceiling
  ~4 GiB).
- Use `just check-package <crate>` / `just test-package <crate>` /
  `just check` / `just lint` / `just deny` / `just boundaries` — never raw
  cargo when a recipe exists.
- Fail-first where practical: for each behavioral change, add/adjust the
  discriminating test, see it fail for the expected reason, then implement.
  Record the fail-first observation in your report.
- Match repo idiom: typed errors (no bare strings), no `unwrap`/`expect` in
  production paths, `clippy -D warnings` clean, `cargo fmt` applied, comment
  density like neighboring modules.
- **Checkpoint discipline (mandatory — sessions here die to usage limits):**
  after completing EACH numbered item below, append a dated entry to
  `.agents/reports/task12/builder-b1-checkpoints.md` (what landed, files,
  test names, gate output tail). Your final message is a convenience; the
  files are the contract.

## Result contract

Write `.agents/reports/task12/builder-b1.md` (schema): per-item summary;
exact new/changed public APIs (signatures); files touched; fail-first
evidence; gate outputs (command + tail); deviations from this brief with
reasons; open questions / residuals for critics. Your final message: a short
self-contained summary pointing at both files.

## Scope

### Item 1 — Domain: `Spent` reconstruction (closes Task-6 critic-A MAJOR-2)

`.agents/CURRENT_STATUS.md` deferral ledger: "Non-zero `budget_spent` cannot
resume … domain `Spent` has no value-carrying constructor … needs a domain
reconstruction slice (constructor + tests), not an adapter patch."

In `crates/gauntlet-domain/src/budget.rs`:

- Add a reconstruction path, e.g. `Spent` private value-carrying constructor
  plus `Budget::restore(limits: Limits, recovery_reserve_pct: u8,
  diagnostic: DiagnosticAllowance, spent: Spent) -> Result<Budget,
  BudgetError>` (exact naming yours; follow module idiom).
- Invariants restore must enforce (typed refusals, each tested): every
  component ≤ its limit (rounds/wall/tokens/cost; diagnostic tokens/cost ≤
  allowance); FORWARD spend must not exceed the forward ceiling the charge
  path enforces (the reserve watermark). A `Spent` unreachable via `charge`
  must be refused as corrupt, not accepted.
- Behavioral-equivalence test: for a sequence of legal charges,
  `restore(spent_after_charges)` agrees with the budget built by replaying
  those charges (`would_breach_reserve`, `remaining`-equivalents, subsequent
  charge admission all agree).
- The sqlite adapter's checkpoint decode (`spent_or_refuse`,
  `crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs:~1881`)
  currently reconstructs by re-charging. You may switch it to `restore` ONLY
  if that is strictly simpler and covered by existing tests; otherwise leave
  it and note the equivalence. No behavior change either way.

### Item 2 — Ports: capability-gate the two owner writes (closes Task-11 C7 + Task-10 MINOR-2)

Today any holder of `Arc<dyn StateStore>` can call
`record_evidence_production` / `bind_observation_bundle` for any
store-resolvable bytes, and the declared iteration is not validated
(`crates/gauntlet-ports/src/state_store.rs` doc ~lines 340–376 records this
residual). The established pattern is `AdmittedComparison`: a ports-level
minting service does the governed reads, mints an unforgeable value
(`pub(crate)` constructor), and the store trait accepts only the minted
value (see `MandatoryComparisonAcceptance` in
`crates/gauntlet-ports/src/comparator.rs:1336`, and Task-10 round 8–10
closure records).

Add to ports (placement yours; a new `producer_authority.rs` module is
fine):

1. `EvidenceProductionAuthority::new(state, evidence)` with
   `record(&self, run_id, iteration_no, evidence_ref, producer,
   locator, produced_at, deadline) -> Result<EvidenceProductionBinding, …>`
   which:
   - loads the durable run (typed refusal if absent);
   - validates the declared iteration against the SAME durable anchor rule
     `VerificationAuthority::for_run` uses (latest durable checkpoint's
     iteration, else 1) — factor that anchor read if it is duplicable, else
     mirror it exactly and cross-reference in a doc comment;
   - reads the evidence bytes at `evidence_ref` from the evidence store and
     derives the content digest FROM BYTES (never accepts a caller-supplied
     digest — Task-11's F1 lesson: doctored locator bytes must not ride a
     real adapter to PASS);
   - mints `AdmittedEvidenceProduction` (no public constructor; accessors
     expose the `EvidenceProduction` payload for the adapter) and calls
     `state.record_evidence_production(run, &admitted, deadline)`.
2. `ObservationBindingAuthority::new(state, evidence)` with
   `bind(&self, run_id, iteration_no, bundle_ref, deadline)` which performs
   the same run-exists + iteration-anchor validations, reads the bundle
   bytes, re-derives the §12.4 observation content digest from bytes (the
   canonical derivation already lives in ports — Task-10 round 10 moved it
   there; find and reuse it, do not write a second one), mints
   `AdmittedObservationBinding`, and calls
   `state.bind_observation_bundle(run, iteration, &admitted, deadline)`.
3. Change the `StateStore` trait signatures to take the minted types;
   update the sqlite adapter (payload via accessors) and the shared
   StateStore contract suite. The suite must EARN the capability through the
   authorities against an in-suite store (the Task-10 round-9 pattern that
   deleted `contract_fixture`) — no public minting path may remain anywhere.
4. Digest-rule discipline: ONE hashing rule per digest kind. Find the
   existing ports-side content-digest/observation-digest helpers and reuse
   them; if `gauntlet-app`'s `verify::content_digest` is the same algorithm,
   keep both sides pinned to equality with a test (there may already be a
   pin — look). Do not introduce a third derivation.
5. Update `crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs`
   `Fixture::produce` to route through `EvidenceProductionAuthority`
   (ports-public API; the CLI dev-dependency seam is legal).
6. Tests (fail-first): wrong declared iteration (stale/ahead) refused;
   unknown run refused; content digest derived-from-bytes (a production
   whose bytes at the ref change after the fact is refused at the authority
   — simulate via a mutable evidence fake in the ports test module);
   direct-store-call bypass is a compile error (verify by absence of public
   constructor; a compiled negative probe is the confirmation round's job,
   note it).

### Item 3 — BudgetMeter adapter (plan step 3)

New crate `crates/adapters/gauntlet-adapter-budget-meter` implementing
`gauntlet_ports::BudgetMeter` (trait at
`crates/gauntlet-ports/src/budget_meter.rs:105`; shared suite at
`crates/gauntlet-ports/src/contract/budget_meter.rs` — the adapter must
pass the UNMODIFIED suite; see how `gauntlet-adapter-state-sqlite` wires
suite execution).

Design:

- `MeteredBudget::new(budget: Budget)` (or factory per suite needs),
  thread-safe (`Arc<Mutex<…>>`), holding the domain `Budget` plus
  reservation bookkeeping (outstanding reservations).
- `reserve(slice)`: refuse `BudgetMeterError::Reserve{cause}` when the
  slice would breach the recovery reserve (delegate to the domain's
  `would_breach_reserve` — SAME formula both sides, no re-implementation
  drift); enforce outstanding-reservation parallelism ≤ `max_parallel`
  (documented zero-payment `charge` as the release path for
  cancelled/failed invocations — charge(Payment::zero()) books nothing and
  releases the slot); return `Reservation`.
- `charge(reservation, payment)`: `ReservationExceeded` if payment exceeds
  the reserved slice per-dimension; book via `budget.charge` (typed refusal
  passthrough); consume the reservation slot.
- `remaining()`: from the domain budget + parallel/depth availability.
- §17.4 also names tool-invocation and retry dimensions. The domain Limits
  (§5.3.7) has NO tool-call/retry ceilings and the ports `Remaining` shape
  is fixed — enforce every dimension that exists; track tool-call/retry
  totals as adapter-level counters (exposed on the adapter type, doc
  comment referencing §17.4) and RECORD THE GAP in your report as a
  residual for the ports revision (do not widen the port).
- Reserve↔charge agreement pin: property/exhaustive-boundary test that for
  any slice, `would_breach_reserve(slice) == matches!(reserve(slice),
  Err(Reserve{..}))` — boundary-exact values included (== ceiling).
- Workspace registration: add the crate to the workspace members list in
  the root `Cargo.toml`; its deps are `gauntlet-domain` + `gauntlet-ports`
  only (no external crates — boundaries deny-by-default).

### Item 4 — Gates (run personally, record outputs)

```
just check-package gauntlet-domain && just test-package gauntlet-domain
just check-package gauntlet-ports && just test-package gauntlet-ports
just check-package gauntlet-adapter-budget-meter && just test-package gauntlet-adapter-budget-meter
just check-package gauntlet-adapter-state-sqlite && just test-package gauntlet-adapter-state-sqlite
just check-package gauntlet-cli        # p12 proof affected
just check && just lint && just deny && just boundaries && just ruler
```

Expect boundaries to stay at its current count (197) plus/minus your suite
additions — any unexpected boundary failure is a finding, not something to
paper over. If a gate is red at the end of your session, say so loudly in
the report; never leave a failing test disabled or an assertion weakened.

## Out of scope (do not touch)

App-layer `control` module (B2/B3), observer adapters (B3), proofs p4/p6
(B4), CLI composition root changes beyond the p12 fixture reroute, any
scheduler/selector/diagnosis logic, HumanEscalation, doctor.
