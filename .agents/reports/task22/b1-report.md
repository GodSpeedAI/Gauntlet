# B1 final report — task22: developmental memory + capability ledger (P10)

Builder B1, 2026-09-03. Charter: `.agents/reports/task22/charter.md`
(D1–D11 binding). Checkpoints: `b1-progress.md` in this directory.

## What landed

### 1. The `CapabilityLedger` port (D1/D2) — `crates/gauntlet-ports/src/capability_ledger.rs`

One port carrying both §19.1 stores, typed end to end, zero new
dependencies (hand-rolled JSON codecs over `gauntlet_ports::json`, the
`verification_record` house style):

- **Observation log (developmental memory)**: `append_observation` is
  any-caller, never outcome-gated, append-only, and IDEMPOTENT on the
  declared `ObservationIdentity { capability, class, run, fact }` — a
  same-identity re-append with agreeing facts is a no-op; a disagreeing
  re-append is the typed `ObservationConflict` with the stored row
  standing (the `record_run` precedent). The row type
  (`ObservationRecord`) covers every §19.1 left-column class
  (`ObservationClass`: attempt/failure/correction/comparison/payment/
  settlement/recovery) and records exactly the facts available — absent
  facts stay `None`, never invented.
- **Content axes**: `ObservationRecord::axes()` computes the typed
  `VariationAxis` set (unit / reference digest / evidence kind / locator
  class) from the row's own populated fields, so a row can never claim
  axes its facts do not carry. Run ids and timestamps are NOT axes.
- **Capability register**: `CapabilityId` → `CapabilityStatus` (the
  existing domain four-state enum) whose ONLY status writer is
  `apply_transition(StatusTransition)` — a first-class record carrying
  reason (`TransitionReason::Promoted(PromotionEvidence)` /
  `Refused(Vec<PromotionRefusal>)` / `Revoked(detail)`) and evidence
  refs. There is NO set-status method: elevation is earned by composing
  a typed record that refuses to compose what the state machine forbids
  (`StatusTransition::new` + `is_legal_transition`, the domain
  `Capability::restatus` table). Current status is DERIVED from the
  transition history — no mutable status field exists.
- 9 typed §19.2/§19.4 refusal rules incl. the two fail-closed
  undecidable-evidence refusals; `PromotionEvidence` is the durable
  decision record (canonical JSON, strict decode; corrupt payload ⟹
  `Corrupt`, never a degraded record).

### 2. Shared contract suite (D1) — `crates/gauntlet-ports/src/contract/capability_ledger.rs`

Factory-parameterized, findings-returned, 7 cases (identity idempotency
incl. the disagreeing-re-append conflict; exact read-back of every class
incl. FAIL/INCONCLUSIVE; ungated appending; first-observation-registers-
Observed; legal typed durable transitions; illegal/stale refusals write
nothing; revoke sticky against all three targets). Wired into
`suite_runs.rs` with the three legs, pinned by `contract/stubs.rs`:
`CapabilityLedgerGreen` (reference, clean), `CapabilityLedgerBroken`
(idempotency inflated — caught on
`observation_appends_are_idempotent_on_identity`),
`CapabilityLedgerCapsBroken` (read-back corruption — caught on
`observations_read_back_exactly_as_appended`). The sqlite adapter runs
the unmodified suite as a third factory
(`tests/contract_suite.rs::sqlite_capability_ledger_passes_the_shared_contract_suite`).

### 3. SQLite adapter (D7) — `gauntlet-adapter-state-sqlite`

Migration `0009_capability_ledger.sql` (append-only set, 0001–0009):
`capability_observations` (UNIQUE on the observation identity) +
`capability_transitions` (history only — no status column anywhere, so a
bare set-status has no substrate to write through). `SqliteCapabilityLedger`
(= `EmbeddedCapabilityLedger`): INSERT-only, identity-keyed idempotent
appends, transition application inside one BEGIN IMMEDIATE transaction
(unknown-capability + staleness checks in the same snapshot the insert
lands in), strict payload decode. Cross-run by design.

### 4. Config (D6) — `gauntlet_app::config`

`PROMOTION_MIN_SETTLEMENTS: u32 = 3` in `defaults.rs` (inline §19.2
default, §8.6-exempt like `NONCONVERGENCE_N`, citation on the constant);
full `ConfigKey::PromotionMinSettlements` wiring in `resolve.rs`: path
`promotion.min_settlements`, env `GAUNTLET_PROMOTION_MIN_SETTLEMENTS`
(Annex E vocabulary + `.env.example` pre-existing, untouched),
GAUNTLET.md front-matter section `promotion` (the key IS file-addressable
— unlike nonconvergence, per D6's wording), `EffectiveConfig` +
`ResolvedConfig` fields, canonical serialization + strict round-trip +
fingerprint coverage. A configured 0 is a typed `ValueOutOfRange` refusal
at EVERY layer (env, front matter, flags, and the service constructor).
`GAUNTLET_PROMOTION_MARGIN` stays vocabulary-only (verified: no consumer).

### 5. App services (D3/D4/D5) — `gauntlet_app::memory`

- `developmental::DevelopmentalImport` — IMPORT, NOT DUAL-WRITE:
  projects a run's durable substrate through the typed `StateStore` reads
  (`load_run` frozen digest, `resume` checkpoint spend → payment,
  `verification_records` → attempt/failure incl. INCONCLUSIVE,
  `correction_selections` → corrections, `settlements` → the settlement
  window with axes from the matching PASS record's requirement +
  instrument). Ungated (failures/INCONCLUSIVEs recorded with the same
  eagerness as passes), idempotent (re-import appends nothing),
  read-only over the substrate. `record_recovery` is the
  recovery-completion call site's append — AFTER
  `complete_pending_settlements` returns > 0, never inside the
  settlement machinery. Invocation-level and comparison facts have NO
  port reader and are NOT projected (documented; never invented).
- `promotion::PromotionService` — the §19.2 gate: ALL five conditions
  evaluated, each failure its own typed refusal, canonical order;
  refusal writes `Provisional` + reasons durable (§19.4), with the
  documented non-elevating Provisional→Provisional re-record for
  re-evaluations; promotion writes `Promoted` with the durable
  `PromotionEvidence` decision record naming each settlement's run/unit/
  verification/evidence; revoke from Provisional/Promoted only; revoked
  is STICKY (evaluate ⟹ `RevokedIsSticky` writing nothing; a second
  revoke refuses; the state machine composes no move out of Revoked).
  Zero minimum refused at construction. Typed rules recorded in the
  module docs as the typed proxies they are (failure-rate halves with
  decided-only denominators; instrument-class erosion; per-settlement
  correction-selection-count burden) — honestly not semantic proofs.

### 6. Composed proof (D8) — `crates/gauntlet-cli/tests/p10_no_false_learning.rs`

Four legs over the REAL sqlite ledger + REAL state store on ONE scratch
database file, every substrate fact landed through the EARNED path
(`record_run` → checkpoint → `VerificationRecordAuthority::record` →
`SettlementCommitAuthority::admit` + `commit_settlement`), deterministic
clock:

- (a) `one_successful_run_is_observed_and_never_promoted` — import ⟹
  `Observed`, never evaluated; evaluation refuses
  (`insufficient_settlements`) ⟹ `Provisional`, not promoted;
- (b) `three_identical_axis_settlements_are_refused` — three honest
  fully-settled runs over the SAME unit/reference/kind/locator ⟹
  `insufficient_variation` recorded durably, NOT promoted (the plan's
  Done-when teeth);
- (c) `the_positive_path_is_the_only_road_to_promoted` — real variation
  (three units) + recovery driven through the documented seam
  (`test_support::delete_settlement_rows` damage FIRST, then the §18.3
  `complete_pending_settlements` returns 1, THEN `record_recovery`) ⟹
  `Promoted` with a durable decision record that a freshly REOPENED
  ledger still reads (cross-run by design);
- (d) `revoke_is_sticky_and_has_no_flip_back` — promoted ⟹ revoke ⟹
  evaluation refuses without writing; second revoke refuses; history
  ends at the revoke.

Each leg's module-doc names the single mechanism whose removal defeats
it (the falsifier's mutation targets).

### 7. App module tests (D9) — `gauntlet_app::memory::` (24 tests)

Condition-by-condition promotion coverage (each §19.2 refusal isolated
over the in-memory double, incl. both undecidable-evidence refusals),
transition legality + revoke stickiness + no-bare-set-status shape,
import idempotency + ungated-ness + axes + conflict surfacing, recovery
records, zero-minimum refusal. Config resolution incl. the 0-refusal
lives under `config::resolve::tests`
(`the_promotion_minimum_defaults_to_the_spec_three_and_overrides`,
NonconvergenceN pattern). 504/504 gauntlet-app tests green.

### 8. Hygiene

- AK-055's locator divergence (`tests/proofs/p10_no_false_learning`
  matches no cargo target) recorded as a dated entry in
  `.agents/reports/reference-candidates.md`; the proof materializes at
  the composition root with the plan-gate target name (p4/p7
  precedent). The ruler was never touched; `just ruler` PASS at slice
  start and after the final gate.
- `improve_cmd.rs` untouched (D11); no CLI surface, doctor, or report
  changes (D10); the settlement path (`settle/`, `producer_authority.rs`)
  has ZERO diff (verified with `git diff --stat`); `rg` confirms no new
  code calls `commit_settlement`/`record_verification` (one doc-comment
  mention only); no new dependencies anywhere.

## Contract deviations (explicit, not smuggled)

1. **Variation axes omit blast_radius/tier** (D4 names them): no
   `StateStore` read the import projects carries a unit's blast radius or
   tier, and facts the substrate cannot read are never invented (D3's
   own rule). The typed axis set carries the four durable-derivable
   content classes. Fail-closed: fewer axes make variation HARDER to
   prove, never easier.
2. **Revoked is TERMINAL** (D5 says "re-promotion must re-run the FULL
   evaluation, not merely flip a bit"): the domain `Capability::restatus`
   state machine — the frozen ruler for status legality — permits no move
   out of Revoked, so the register mirrors it. The fail-closed reading:
   there is NO flip-back path at all; evaluate() on a revoked capability
   is the typed `RevokedIsSticky` refusal writing nothing; re-earning is
   only possible for a capability that is not revoked. The alternative
   reading (Revoked→Promoted through a full re-evaluation) would require
   a transition the domain forbids.
3. **Provisional→Provisional re-record**: a re-evaluation that refuses
   again re-records its (new) reasons on the unchanged status — one
   non-elevating extension to the domain aggregate's table, required by
   D4 ("a refusal writes Provisional + reasons") for the second refusal
   of a Provisional capability. It cannot raise status; the suite pins
   that demotions and elevations through it are unrepresentable.
4. **Production call-site wiring deferred**: D3 names two production
   call sites (before promotion evaluation; after a run reaches a
   terminal state in the report path). This slice ships the SERVICES and
   their proofs; the wiring is intentionally not built — the charter's
   out-of-scope list forbids doctor/report changes and `improve` wiring,
   and the first declared consumer (task23's evaluation) does not exist
   yet. First callers today are the tests and p10.
5. **`at` is the recording instant** (Clock port), uniform across all
   classes, excluded from identity idempotency; window ordering is
   append order, never `at`. Documented on the port and in the import.

## Open questions

None blocking. (The AK-055 locator divergence is recorded for the
ruler's owner, per the established reference-candidates process.)

## Gates run (verbatim tails; all under the flock'd build protocol)

```
=== ruler (start) ===
verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock (…/.gauntlet/reference.lock)

=== just check-package (each touched crate) ===
check-package gauntlet-ports:                   Finished `dev` profile … in 2m 16s
check-package gauntlet-adapter-state-sqlite:    Finished `dev` profile … in 0.99s
check-package gauntlet-app:                     Finished `dev` profile … in 4.51s
check-package gauntlet-cli:                     Finished `dev` profile … in 7.91s

=== clippy (touched crates, -D warnings) ===
Finished `dev` profile [unoptimized] target(s) in 2m 45s   (zero warnings)

=== cargo fmt --all -- --check ===
FMT-CLEAN

=== PLAN GATE: cargo nextest run --test p10_no_false_learning -p gauntlet-cli ===
        PASS [   0.074s] (1/4) gauntlet-cli::p10_no_false_learning three_identical_axis_settlements_are_refused
        PASS [   0.080s] (2/4) gauntlet-cli::p10_no_false_learning the_positive_path_is_the_only_road_to_promoted
        PASS [   0.081s] (3/4) gauntlet-cli::p10_no_false_learning one_successful_run_is_observed_and_never_promoted
        PASS [   0.081s] (4/4) gauntlet-cli::p10_no_false_learning revoke_is_sticky_and_has_no_flip_back
────────────
     Summary [   0.098s] 4 tests run: 4 passed, 0 skipped

=== PLAN GATE: cargo nextest run -p gauntlet-app memory:: ===
        PASS [   0.011s] (24/24) gauntlet-app memory::promotion::tests::the_full_satisfaction_path_is_the_only_road_to_promoted
────────────
     Summary [   0.082s] 24 tests run: 24 passed, 480 skipped

=== suite teeth (gauntlet-ports capability_ledger legs) ===
        PASS gauntlet-ports contract::suite_runs::capability_ledger_green_is_clean
        PASS gauntlet-ports contract::suite_runs::capability_ledger_broken_is_caught
        PASS gauntlet-ports contract::suite_runs::capability_ledger_caps_broken_is_caught
     Summary: 3 tests run: 3 passed   (suite can fail: broken + caps legs red by design on their intended cases)

=== package tests ===
cargo nextest run -p gauntlet-ports:                  115 tests run: 115 passed
cargo nextest run -p gauntlet-adapter-state-sqlite:    51 tests run: 51 passed
cargo nextest run -p gauntlet-app:                    504 tests run: 504 passed

=== just boundaries ===
        PASS [  45.962s] (228/228) gauntlet-arch-tests::architecture_boundaries scope_integrity
────────────
     Summary [  57.157s] 228 tests run: 228 passed, 0 skipped

=== just ruler (final) ===
verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock (…/.gauntlet/reference.lock)
```

NOT run here (orchestrator owns them): the full workspace
check/test/lint/deny battery and e2e feature-matrix — package-scoped
verification was kept green instead, per the working discipline.

## Files touched (sha256)

```
68fc78d498a45aeae98631c48d7d30bb5c89d2ae0e6d9b42872ea066e00eb368  crates/gauntlet-ports/src/capability_ledger.rs
5c0f26a1b36153615c106cb736cbbfcdde1fb96faeff71e67ffbd7cfa0a66786  crates/gauntlet-ports/src/contract/capability_ledger.rs
2a6f7c443d881ca2a8f331d8f6feb9894d915914a976da895674ec7c5590e67e  crates/gauntlet-ports/src/contract/mod.rs
83b05e8e8232ef24795dc31108ad91422089765f3cb7a5b88476598dd6b25225  crates/gauntlet-ports/src/contract/stubs.rs
2b53d59d67f1bcfdd1420b86bb357ded38029f87baf8c15c989cd8b8d515a9ca  crates/gauntlet-ports/src/contract/suite_runs.rs
8ffe6f8028265a8bd07ca0846706abe662109a122bd9c988ecfbfd0faa3a1a8f  crates/gauntlet-ports/src/lib.rs
b544a46d2d6dda647760bb3de4637f3470c9c119c14fb67d44baea7224e1c5bf  crates/adapters/gauntlet-adapter-state-sqlite/migrations/0009_capability_ledger.sql
c03476e4431fe3b7663e1d0df2dffb8930a18f17a9d78c8a21f0e038fbe58889  crates/adapters/gauntlet-adapter-state-sqlite/src/capability_ledger.rs
0bc02cdabd6e7481ae0d4ee5d7f643f9cd3f78e10acba3b69b4d8128c75d7372  crates/adapters/gauntlet-adapter-state-sqlite/src/lib.rs
6a520f58e0ab05330246b907fdbe8365669488860eced4ae93584ff3d63a13d7  crates/adapters/gauntlet-adapter-state-sqlite/src/migrations.rs
cec21668b18c637ac5aaa7a84f41e71e6249b726f26ef990e784915664caf71c  crates/adapters/gauntlet-adapter-state-sqlite/tests/contract_suite.rs
06567a786c6e63972e6ce571f0c87ab8ff5a1f363b411f3551709c0aee0e6524  crates/gauntlet-app/src/config/canonical.rs
e7ae3579baf3f80beb71ab5e34e989e0387ca1d5d4edf75f024c50355414b5d2  crates/gauntlet-app/src/config/defaults.rs
7c1a8a8d091227ead9fd3fdcc0bf99c3cc4965aade9897ace758ebc7cd834225  crates/gauntlet-app/src/config/resolve.rs
f2f5acabd1da652cfa6de9a0e57d786e7229acff0ca50d2d49cbfec9c6da02cc  crates/gauntlet-app/src/lib.rs
b5c8cda1b18eff17420c85f469ab66cf7014e421f173995200825793e2e0aada  crates/gauntlet-app/src/memory/mod.rs
a0cdb9968260f191877913de649bfc27b0dd28685f3dad800d2f14ab5aad051d  crates/gauntlet-app/src/memory/developmental.rs
befc66632730b5cf56f4138821ee01d1cdf969b95a7afd76bb1055e416b8ea28  crates/gauntlet-app/src/memory/promotion.rs
17029048862efce4c55d2f0a39705aba28d794fca2e759bee3d2b40147e4a604  crates/gauntlet-cli/tests/p10_no_false_learning.rs
fd49c65d334fe2b05a93282f018b91abc16196ac8dd06292df553f4fd7867f0c  docs/reference/configuration.md
```

Plus report files in `.agents/reports/task22/` (charter, IN-FLIGHT,
b1-progress.md, b1-report.md) and the dated AK-055 entry appended to
`.agents/reports/reference-candidates.md`. Nothing staged or committed;
`.wayfinder/` and `.gauntlet/reference.lock` byte-untouched (ruler PASS
both ends).
