# Task 22 falsifier — round 1 (blind executing falsifier)

Date: 2026-09-03. Scope: executed attacks against the developmental-memory +
capability-ledger mechanism (`gauntlet-ports/src/capability_ledger.rs`,
`gauntlet-app/src/memory/{developmental,promotion}.rs`,
`gauntlet-adapter-state-sqlite/src/capability_ledger.rs` + migration 0009,
`gauntlet-cli/tests/p10_no_false_learning.rs`, contract suite
`gauntlet-ports/src/contract/capability_ledger.rs` + stubs + suite_runs),
against claims 1–4 of the proof's teeth declarations and charter D1–D11.
Blind: no builder/critic reports read; everything below derived from code,
spec text quoted in the charter, AK-055, and executed commands.

All cargo invocations ran under
`flock /tmp/gauntlet-build.lock sh -c '<cmd>; sccache --stop-server; true'`.

## Executed attacks (one line each)

Baseline (all GREEN before any mutation):

- E0 [baseline] p10 proof — `cargo nextest run -p gauntlet-cli --test p10_no_false_learning` → 4/4 GREEN.
- E1 [baseline] app services — `cargo nextest run -p gauntlet-app memory::` → 24/24 GREEN.
- E2 [baseline/D] ports suite legs — `cargo nextest run -p gauntlet-ports capability_ledger` → 3/3 GREEN (green stub clean; both broken stubs caught).
- E3 [baseline/D] adapter contract — `cargo nextest run -p gauntlet-adapter-state-sqlite --test contract_suite` → 3/3 GREEN (incl. `sqlite_capability_ledger_passes_the_shared_contract_suite`).

Family B — public-API false-promotion search (scratch integration test
`crates/gauntlet-cli/tests/zz_falsifier_scratch.rs`, real sqlite
`EmbeddedCapabilityLedger` + real `EmbeddedStateStore` + authority-minted
records; file temporary, deleted after, never staged):

- B1 [B-i] AK-055 run-id trap fed DIRECTLY through ungated `append_observation`: 3 identical-content settlements differing only in run id + recording instant, otherwise-complete window → `Refused{insufficient_variation}` (the ONLY refusal), `Provisional`. GREEN (repelled).
- B2a [B-ii] 1 full + 2 sparse settlement rows (sparse = no unit/reference/kind/locator; instrument, burden, verification, outcome present), otherwise-complete window → **PROMOTED**; evidence credits variation to ABSENCE; 1 witness for 3 settlements_observed. Confirmed (→ F-1).
- B2b [B-ii] 3 all-sparse rows (identical EMPTY axes) → `Refused{insufficient_variation}`. GREEN.
- B3a [B-iii] honest substrate r1–r3 PASS settlements (3 units) + r4 (newest) FAIL record, in-order import → `Refused{failure_rate_increased}`. GREEN (correct verdict).
- B3b [B-iii] SAME durable substrate, import order 4,3,2,1 (newest first) → **PROMOTED**, decision records `failure_rate_earlier=(1,2)`, `failure_rate_later=(0,2)` — window inversion certified a rising rate as improving. Confirmed (→ F-2).
- B4 [B-iv] fabricated `PromotionEvidence` in `StatusTransition{Observed→Promoted}` applied directly through `apply_transition` (one sparse observation, service never involved) → applied, register durably `Promoted`; the honest service thereafter returns `AlreadyPromoted` (forged status stands; only a revoke clears it). Confirmed (→ F-3).
- B5 [B-v] `record_recovery` for a run that exists NOWHERE (`state.load_run` = None) on a complete-minus-recovery window: first eval refuses `no_recovery_observed`; after the minted row → **PROMOTED**. Confirmed (→ F-4).

Family C — durability/idempotency over the real sqlite adapter:

- C1 [C] re-import the same run twice → second import `appended=0`, `already_present=N`, row count unchanged. GREEN.
- C2 [C] promote → drop connection → reopen file: status `Promoted`, transition history + evidence intact; idempotent replay after reopen = no-op; conflicting re-append after reopen = typed `ObservationConflict`. GREEN.
- C3 [C] same identity re-appended with changed facts → `CapabilityLedgerError::ObservationConflict`, exactly 1 row stands. GREEN.

Family A — mechanism mutations (each: apply → targeted tests → observe RED → restore → sha256 verify):

- A1 [A] `promotion.rs`: `insufficient_settlements` push neutralized (`if false && have < need`) → `fewer_settlements_than_the_minimum_refuse_and_write_provisional` FAIL (assertion: rule absent) AND p10 `one_successful_run_is_observed_and_never_promoted` FAIL. RED — tooth bites. Restored (sha `befc6663…`).
- A2 [A] `promotion.rs`: `insufficient_variation` push neutralized → unit `identical_content_axes_refuse_insufficient_variation` FAIL AND p10 `three_identical_axis_settlements_are_refused` FAIL (observed refusal list with tooth gone: `[NoRecoveryObserved]` — still not promoted, but the named-rule assertion fired). RED — tooth bites. Restored (sha `befc6663…`).
- A3 [A] `capability_ledger.rs` (ports): `is_legal_transition` given a `(Revoked, Promoted)` arm → ports `capability_ledger_green_is_clean` FAIL ("a transition left Revoked toward promoted") AND adapter `sqlite_capability_ledger_passes_the_shared_contract_suite` FAIL. RED — tooth bites on both stub and real adapter. Restored (sha `68fc78d4…`).
- A4 [A] `capability_ledger.rs` (ports): `axes()` mutated to prepend a run-id-derived locator axis (the exact false-variation trap) → unit variation test FAIL, p10 leg-2 FAIL, AND `a_settlement_observation_carries_the_content_axes` FAIL. RED — tooth bites via three detectors. Restored (sha `68fc78d4…`).

Post-restore confirmation: E0–E3 re-run → 4/4, 24/24, 3/3, 3/3 GREEN. `git status --porcelain` after == before (diff empty). `just ruler` → PASS.

## Findings

### F-1 — MAJOR: the variation condition credits ABSENCE as recorded variation (a §19.2-contrary promotion through the service itself)

The gate that must refuse "three settlements with identical content axes"
also promotes windows where two of three settlements carry NO content
axes at all, because `settled_with_real_variation`
(promotion.rs) compares each row's axis SET against the first row's:
empty ≠ populated ⇒ "variation". Reproduction (executed, B2a): real
sqlite ledger; append settlement{unit AK-9801, reference, kind
test_report, locator, instrument deterministic, cost 0}; two
settlements with only {outcome Pass, instrument deterministic,
verification, evidence, cost 0} and NO unit/reference/kind/locator; two
PASS attempts; one recovery. `PromotionService(min=3).evaluate` →
`Promoted`; `evidence.variation_axes` = the one full row's four axes;
`settlements_observed=3` with 1 witness. Reachability boundary: the
developmental import can never emit an axis-less settlement row
(`SettlementRecord` always carries `unit_id`; the import always sets
unit + reference digest), so this requires hand-crafted rows through
the ungated append surface — but that surface is ungated BY DESIGN and
the promotion gate is the only defense. Correction direction: refuse
windows containing settlement rows with an empty axis set
(`insufficient`-evidence class), or compare per-axis presence rather
than set inequality.

### F-2 — BLOCKING: the failure-rate (and burden/erosion window) verdict is a function of import order, not of recorded facts — honest out-of-order imports flip a genuinely rising failure rate into a promotion

Executed (B3a/B3b): real state store; r1–r3 authority-minted PASS
records + §16.3 settlement commits over units AK-9801..03; r4 (newest)
authority-minted FAIL record, no settlement; one recovery observation.
`import_run` in order 1,2,3,4 → `Refused{failure_rate_increased}`. The
SAME durable substrate imported 4,3,2,1 → `Promoted` with recorded
rates `earlier=(1,2)`, `later=(0,2)`. No hand-crafted rows, no bypassed
service — only the call order of the production import service
differs, and nothing constrains that order (runs complete out of
chronological order under parallelism; the import's production call
sites are not yet wired, so this must be fixed before wiring). Root
cause: outcome windows order by ledger append order == import call
order; `ObservationRecord.at` is deliberately the recording instant,
not a substrate fact, so the ledger cannot self-correct. The durable
decision record certified a deteriorating trajectory (true tail rate
1/1) as improving — a false promotion actually reachable with honest
data. Correction direction: order windows by a substrate fact
(verification `produced_at`, run-id/iteration tiebreak) or refuse when
append order disagrees with substrate chronology.

### F-3 — MAJOR: elevation is not an earned write — a fabricated promotion decision applies directly through the port and irreversibly poisons the register

Executed (B4): ledger with one sparse observation (registers
`Observed`); `StatusTransition::new(cap, Observed, Promoted,
TransitionReason::Promoted(fabricated full-satisfaction evidence), vec![], at)`
composes (Observed→Promoted is a legal move) and `apply_transition`
applies it; the register is durably `Promoted` and the honest service
now refuses with `AlreadyPromoted` — the forged status stands as if
earned, clearable only by revoke. Charter D2's LETTER holds (no bare
`set_status`; illegal moves refuse to compose; stale `from` refuses),
but the §19.2 gate over the record's CONTENT is call-site convention
only. The house pattern for earned writes (`AdmittedSettlement`,
`VerifiedClaim`, `GateAdmission` — authority types with no public
constructors) is not applied to elevating transitions, unlike
everywhere else a write must be earned. Correction direction: mint
elevating transition records through an authority the PromotionService
composes.

### F-4 — MAJOR: recovery credit is mintable on an empty substrate — one ungated row completes a false conjunction

Executed (B5): window with 3 varied full settlements + 2 PASS attempts
and no recovery → `Refused{no_recovery_observed}` (durable);
`DevelopmentalImport::record_recovery(cap, run_id_that_exists_nowhere, …)`
→ Ok (the method never reads the StateStore — verified
`state.load_run == None` immediately before); re-evaluate → `Promoted`.
Charter D4's precondition ("appended by the recovery completion call
site AFTER `complete_pending_settlements` returns > 0") is procedural,
not enforced by any type; the p10 proof drives the seam honestly, but
any caller can mint condition 3. Same class as F-3: a
promotion-relevant fact is not an earned write. Correction direction:
`record_recovery` takes a typed receipt from the §18.3 completion
(the house authority pattern).

### I-1 — INFORMATIONAL: teeth declarations overstate single-tooth removal; defense in depth is real

The leg-1/leg-2 declarations say "remove <condition> and the run
promotes". Executed A1 and A2/A4 show removing ONE condition still
refuses (A1: leg 1 refused on variation + recovery + outcome-evidence;
A2/A4: leg 2 refused with `[NoRecoveryObserved]`). The named tests
still turn RED on every mutation (their rule-presence assertions are
real teeth), so the falsification protocol works — but no single
mutation yields an actual promotion through the service. Documentation
honesty note only; no correction required.

### I-2 — INFORMATIONAL: `PromotionEvidence.settlements_observed` can exceed the witness count

Observed 3 vs 1 in F-1: witnesses `filter_map` rows missing
unit/verification/evidence, so the decision record silently carries
fewer witnesses than settlements. The record is honest but easy to
misread; a decision-record consumer should treat `witnesses <
settlements_observed` as a red flag. Feeds F-1's correction.

## Claim scorecard

1. "A single successful run can never promote" — HOLDS (leg 1; and still refuses even with the count tooth removed, A1).
2. "Three identical-content settlements can never promote — run ids/timestamps are not variation" — HOLDS (repelled with direct ungated injection B1 and all-sparse B2b; both weakening mutations A2/A4 caught RED by three detectors).
3. "Only the FULL §19.2 conjunction promotes, durable across reopen" — SERVICE PATH HOLDS and is durable (C2, leg 3), BUT the exclusivity claim does not survive: F-2 promotes a §19.2-contrary capability through the service with honest data; F-1 promotes through the service with hand-crafted rows; F-3 forges elevation at the port.
4. "Revoked is sticky; no transition leaves it" — HOLDS (A3 caught by both the stub leg and the real adapter suite; the service refuses without writing; C3-class conflict rules survive reopen).

## Restoration declaration

- `crates/gauntlet-app/src/memory/promotion.rs` — mutated twice (A1, A2), restored after each; sha256 `befc66632730b5cf56f4138821ee01d1cdf969b95a7afd76bb1055e416b8ea28` verified == pre-attack baseline.
- `crates/gauntlet-ports/src/capability_ledger.rs` — mutated twice (A3, A4), restored after each; sha256 `68fc78d498a45aeae98631c48d7d30bb5c89d2ae0e6d9b42872ea066e00eb368` verified == baseline.
- Created then deleted: `crates/gauntlet-cli/tests/zz_falsifier_scratch.rs` (Family B/C scratch; never staged, never committed).
- Recorded and untouched (sha256 verified unchanged): `crates/gauntlet-ports/src/contract/stubs.rs`, `crates/gauntlet-cli/tests/p10_no_false_learning.rs`, `crates/adapters/gauntlet-adapter-state-sqlite/src/capability_ledger.rs`.
- `.wayfinder/answer-key.md`, `.wayfinder/reference.yaml`, `.gauntlet/reference.lock`: never opened for write, untouched.
- `git status --porcelain` after == pre-recorded delta (diff empty). Nothing staged or committed. `just ruler` PASS after all work.

VERDICT: NOT_CONFIRM
1 blocking / 3 major / 0 minor / 2 informational
