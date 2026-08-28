# Task 13 / B4 — teeth record (p7 / p11 / g10)

Every tooth: break → RED test name + output tail → revert → `cmp`
byte-identical → green. All teeth run by the B4-completion finisher
(2026-08-28) under the exclusive build lock; the original builder's own
fail-first evidence for Item 1 was lost to the 19th interruption, so the
p7 tooth below is the finisher's re-derivation (recovered-honest).

## Tooth 1 — p7 `commit_order` (the reorder tooth; prereg D1)

- **Break** (`crates/gauntlet-app/src/settle/mod.rs`, the admit arm of
  `settle()`): the durable `commit_settlement` replaced with a no-op —
  the pre-B2 in-memory-only settle shape:
  ```rust
  Ok(admitted) => {
      let _ = &admitted; // B4 TOOTH: pre-B2 in-memory-only settle
      (gauntlet_domain::EventLogOffset::new(0), true)
  }
  ```
- **RED**:
  ```text
  thread 'commit_order' panicked at crates/gauntlet-cli/tests/p7_recovery.rs:1325:9:
  assertion `left == right` failed: each unit's settlement ran the durable sequence once
    left: 0
    right: 2
  Summary [   0.058s] 1 test run: 0 passed, 1 failed
  ```
  The proof discriminates the ORDER claim: with the durable sequence
  removed, the decorator observed zero commits and the proof fails.
- **Revert**: `cp` from the pre-tooth backup; `cmp` clean
  (byte-identical).
- **Green**: `Summary [   0.235s] 5 tests run: 5 passed, 0 skipped`.

## Tooth 2 — p11 (the reads-verification-records gate; "settle admits on comparator cleanliness")

Four coordinated neuters, one tooth — the defense is layered by design
(entry outcome → precondition outcome → PASS-only pin → admission's
durable-record read), and "admit on comparator cleanliness" means every
settlement-side verification-record read is disabled:

1. `crates/gauntlet-app/src/settle/mod.rs` — the COR-2 entry gate:
   `if record.outcome() != gauntlet_domain::Outcome::Pass {` →
   `if false && record.outcome() != ... { // B4 P11 TOOTH`
2. `crates/gauntlet-app/src/verify/precondition.rs` — the precondition's
   NotPass gate: `if record.outcome() != Outcome::Pass {` →
   `if false && record.outcome() != Outcome::Pass { // B4 P11 TOOTH`
3. `crates/gauntlet-app/src/control/controller.rs` — the PASS-only
   evidence pin: `if verified.record().outcome() == Outcome::Pass {` →
   `if true || verified.record().outcome() == Outcome::Pass { // B4 P11 TOOTH`
4. `crates/gauntlet-ports/src/producer_authority.rs` — `admit`'s
   `verification_records` read replaced by a fabricated single PASS doc
   carrying the commit's own run/unit/evidence/verification (so every
   downstream COR-3/COR-5 check passes naturally).

- **RED**:
  ```text
  thread 'an_inconclusive_verdict_never_settles_and_names_itself' panicked at
  crates/gauntlet-cli/tests/p11_critic_is_not_verifier.rs:836:5:
  no SettlementCommitted event exists for the comparator-clean unit: ...
  Summary [   0.080s] 1 test run: 0 passed, 1 failed
  ```
  The neutered service SETTLED the comparator-clean INCONCLUSIVE claim;
  the proof's core assertion caught it.
- **Revert**: all four files `cp` from pre-tooth backups; `cmp` clean on
  each (byte-identical).
- **Green**: `Summary [   0.059s] 5 tests run: 5 passed, 0 skipped`.

## Tooth 3 — g10 detection (the D4 tautology)

- **Break** (`crates/gauntlet-app/src/settle/mod.rs`,
  `settle_time_verify`): the reference provider's file-derived
  `verify_unchanged` consult replaced with the controller's own frozen
  digest compared against itself:
  ```rust
  match (|| {
      let _ = deadline; // B4 G10 TOOTH: the D4 tautology
      let expected = run.reference_version().digest().clone();
      let actual = expected.clone();
      ...
  })() {
  ```
- **RED**:
  ```text
  thread 'a_midrun_key_tamper_aborts_with_both_digests_pinned' panicked at
  crates/gauntlet-cli/tests/g10_builder_writes_answer_key.rs:952:5:
  no SettlementCommitted event exists over the mutated ruler: ...
  Summary [   0.047s] 1 test run: 0 passed, 1 failed
  ```
  The tautologized consult let the settle COMMIT over the mutated ruler
  (the later boundary tripwire still aborted the run afterward, so the
  leg's settle-time-specific assertion — no settlement over a mutated
  ruler — is exactly the discriminator for the §9.4 "immediately before
  settlement" consult).
- **Revert**: `cp` from the pre-tooth backup; `cmp` clean
  (byte-identical).
- **Green**: `Summary [   0.047s] 2 tests run: 2 passed, 0 skipped`.

## Post-tooth state

All production files byte-identical to their pre-tooth state (`cmp`
verified per file); the three proof files pass
(`p7_recovery` 5/5, `p11_critic_is_not_verifier` 5/5,
`g10_builder_writes_answer_key` 2/2) with clippy reporting zero
warnings on each target.
