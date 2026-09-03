# B1 progress — task22 (developmental memory + capability ledger)

Checkpoint file, updated at every milestone (sub-agent death must not lose
work). Final report: `b1-report.md`.

## Checkpoint 0 — 2026-09-03, reconnaissance complete

- Charter read (D1–D11); ruler unit AK-055 read (answer-key.md:383-388);
  `just ruler` PASS at start (digests match the frozen lock).
- IN-FLIGHT.md and CURRENT_STATUS.yml reconciled against `git status`:
  tree clean except orchestrator-owned files (.agents/CURRENT_STATUS.yml,
  .jolli/*) and this task's own report directory. Nothing to recover.
- Patterns studied:
  - Port shape: `gauntlet-ports/src/event_log.rs` (typed errors, Deadline
    on every method), earned-write precedents in
    `producer_authority.rs` (AdmittedSettlement/VerificationRecord mints,
    governed_run durable-anchor rule).
  - Contract suites: `contract/{mod,event_log,suite_runs,stubs}.rs` —
    factory-parameterized, findings-returned, green/broken/caps legs.
  - SQLite adapter: `migrations.rs` append-only set (0001–0008),
    `state_store.rs` write-once/INSERT-only idempotency-by-key patterns,
    `tests/contract_suite.rs` factory wiring.
  - Config: `config/resolve.rs` NonconvergenceN wiring (env-only),
    `config/canonical.rs` complete-tree round-trip,
    `config/defaults.rs` declaration-granular exemption discipline.
  - App service style: `settle/mod.rs` SettlementService (Arc<dyn Port>
    fields, Clock port for time, authorities composed inside new()).
  - Hand-rolled JSON: `gauntlet_ports::json` (JsonValue +
    to_json_text), `verification_record.rs` strict codec house style.
  - Composed proof style: `crates/gauntlet-cli/tests/p7_recovery.rs`
    (named fixture figures, scratch dirs, REAL stores, documented
    deviations in the module header).
- Design decisions locked (to be re-verified against D1–D11 in the final
  report):
  - Port carries both stores: observation log (identity-keyed,
    append-only, ungated) + capability register (status via FIRST-CLASS
    transition records; domain CapabilityStatus enum reused; port
    enforces the state machine; NO set-status method exists).
  - Variation axes typed as the four durable-derivable content classes
    (unit, reference_digest, evidence kind, locator). blast_radius/tier
    (D4's other named axes) are NOT derivable from any StateStore read
    the import projects — omitted rather than invented (recorded as a
    deviation note; fail-closed: fewer axes make variation harder to
    prove, never easier).
  - Revoked is TERMINAL (mirrors domain Capability::restatus: no
    transition out of Revoked). D5's "re-promotion must re-run the FULL
    evaluation, not merely flip a bit" is honored fail-closed: there is
    no flip-back path at all; evaluate() on a Revoked capability is a
    typed refusal that writes nothing.
  - Import (D3) is a service over ports (StateStore read-only +
    CapabilityLedger appends); production wiring into the report path is
    NOT built (charter out-of-scope: "NO doctor/report changes"; first
    callers are the tests + proof; task23 wires evaluation).
- AK-055 locator divergence (tests/proofs/p10_no_false_learning matches
  no file) → dated entry appended to
  `.agents/reports/reference-candidates.md` (same class as
  AK-060/061/062/063); the ruler itself untouched.

## Milestones

### Milestone 1 — port + contract suite landed (2026-09-03)

- `crates/gauntlet-ports/src/capability_ledger.rs`: typed observation log
  (`ObservationIdentity`/`ObservationClass`/`ObservationRecord` incl.
  canonical JSON codec + `axes()`/`facts_agree()`), variation axes
  (`VariationAxis`), refusal vocabulary (`PromotionRefusal`, 9 typed
  rules), decision record (`PromotionEvidence`, `SettlementWitness`),
  transition records (`StatusTransition` + `TransitionReason` +
  `is_legal_transition` state machine: domain restatus table + the one
  documented non-elevating Provisional→Provisional reasons-re-record;
  Revoked terminal), register entry (`CapabilityEntry`), trait
  `CapabilityLedger` (append_observation / observations / capability /
  apply_transition / transitions), typed errors incl.
  `ObservationConflict` + `StaleTransition` + `UnknownCapability`.
- `crates/gauntlet-ports/src/contract/capability_ledger.rs`: suite with
  7 cases (idempotency incl. disagreeing-re-append conflict;
  exact read-back across every §19.1 class incl. FAIL/INCONCLUSIVE;
  ungated appending; first-observation-registers-Observed; legal typed
  durable transitions; illegal/stale refusals write nothing; revoke
  sticky vs all three targets).
- `contract/stubs.rs`: CapabilityLedgerGreen (reference), Broken
  (idempotency inflated), CapsBroken (read-back corruption).
- `contract/suite_runs.rs`: green/broken/caps legs.
- Gates: `just check-package gauntlet-ports` CLEAN; suite legs
  `cargo nextest run -p gauntlet-ports capability_ledger` 3/3 PASS
  (green clean; broken caught on
  `observation_appends_are_idempotent_on_identity`; caps caught on
  `observations_read_back_exactly_as_appended`); full package
  `just test-package gauntlet-ports` 115/115 PASS.

### Milestone 2 — SQLite adapter landed (2026-09-03)

- `migrations/0009_capability_ledger.sql`: two INSERT-only tables —
  `capability_observations` (UNIQUE on the observation identity; `at`
  column fixed in place during development before any shipped baseline —
  the migration has existed only inside this task's unmerged work) and
  `capability_transitions` (no status column anywhere: current status is
  derived from the latest transition's target).
- `src/capability_ledger.rs`: `SqliteCapabilityLedger` (+ capability-named
  re-export `EmbeddedCapabilityLedger`), identity-keyed idempotent
  appends, derived register entries, transactional transition application
  (staleness + unknown-capability checks inside BEGIN IMMEDIATE).
- `tests/contract_suite.rs`: third factory leg over the real adapter.
- Gates: `just check-package gauntlet-adapter-state-sqlite` CLEAN (after
  fixing QueryReturnedNoRows handling in entry_of and the Query alias);
  `cargo nextest run -p gauntlet-adapter-state-sqlite` 51/51 PASS.

### Milestone 3 — config + app memory services landed (2026-09-03)

- Config (D6): `PROMOTION_MIN_SETTLEMENTS: u32 = 3` in defaults.rs
  (inline §19.2 default, §8.6-exempt like NONCONVERGENCE_N); full
  ConfigKey::PromotionMinSettlements wiring in resolve.rs — path
  `promotion.min_settlements`, env `GAUNTLET_PROMOTION_MIN_SETTLEMENTS`
  (already in Annex E vocabulary + .env.example), front-matter section
  `promotion` (the key IS GAUNTLET.md-addressable, unlike
  nonconvergence), EffectiveConfig + ResolvedConfig fields, canonical
  serialization + round-trip + fingerprint coverage, configured-0
  typed-refused (ValueOutOfRange) at every layer. resolve.rs test
  `the_promotion_minimum_defaults_to_the_spec_three_and_overrides`
  follows the nonconvergence pattern (default/env/0-refusal/file/flag/
  fingerprint).
- `gauntlet_app::memory::developmental`: DevelopmentalImport — projects
  load_run (frozen reference digest), resume (payment = checkpoint
  spend), verification_records (attempt/failure incl. INCONCLUSIVE),
  correction_selections, settlements (axes from the matching PASS
  record's requirement + instrument + the run's frozen digest; burden =
  the run's selection count) — all ungated, identity-idempotent,
  `at` = recording instant from the Clock port. record_recovery is the
  recovery-completion call site's ungated append (AFTER
  complete_pending_settlements returns >0; never a settlement-path
  change).
- `gauntlet_app::memory::promotion`: PromotionService — all five §19.2
  conditions with typed per-condition refusals (incl. the two
  fail-closed undecidable-evidence refusals), refusal writes
  Provisional + reasons durable (Provisional→Provisional re-record for
  re-evaluations), Promote carries the durable PromotionEvidence +
  per-settlement evidence refs, revoke sticky (Observed cannot revoke;
  Revoked re-evaluates to RevokedIsSticky), zero minimum refused at
  construction.
- memory/mod.rs testing doubles: MemoryLedger + FixedClock (cfg(test)).
- Port additions during this milestone: ObservationRecord evidence field
  (settlement evidence -> promotion decision's evidence refs);
  SettlementWitness.evidence.
- Gates: `just check-package gauntlet-app` CLEAN;
  `cargo nextest run -p gauntlet-app memory::` 24/24 PASS; full
  `cargo nextest run -p gauntlet-app` 504/504 PASS.


### Milestone 4 — p10 proof + gates (2026-09-03) — SLICE COMPLETE

- `crates/gauntlet-cli/tests/p10_no_false_learning.rs`: 4 legs
  (a-d) over the real sqlite ledger + real earned-path substrate on one
  scratch DB; teeth named per leg in the module docs. 4/4 PASS.
- Boundaries initially RED on 75 literal-scan violations in my test
  fixtures (path-shaped locators, inline fixture figures next to
  duration-shaped identifiers) — fixed with the domain's named-const
  fixture idiom + single-segment locators; boundaries 228/228 PASS.
- clippy (-D warnings) clean across ports/adapter/app/cli after fixing
  clone_on_copy, type_complexity, arc_with_non_send_sync (MemoryLedger
  double made Send+Sync), useless_format, E0408/E0618 compile rounds.
- Final: fmt CLEAN; package tests ports 115/115, adapter 51/51, app
  504/504, p10 4/4, memory:: 24/24; `just boundaries` 228/228;
  `just ruler` PASS (start and final). b1-report.md written with
  sha256sums + verbatim gate tails + 5 explicit deviations.
