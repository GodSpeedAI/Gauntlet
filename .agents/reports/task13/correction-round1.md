# Task 13 / Correction round 1 — result contract (COR-1..COR-7)

Dispatch: correction round 1 against the early-probe findings
(`early-adversarial-probe.md`, adjudicated in `early-probe-adjudication.md`).
COR-1 was landed by the correction builder and recovered/verified by the
orchestrator (see `correction-checkpoints.md`, RECOVERED-AND-VERIFIED).
This document is the COR-2..COR-7 finisher slice plus the re-derived COR-1
fail-first evidence. Per-item logs with verbatim RED/GREEN observations:
`correction-checkpoints.md`.

## COR-1 — recovered, verified, and re-derived (not re-implemented)

Landed state (verified by the orchestrator before this slice; re-verified
green at this slice's closure): ports `VerificationRecordAuthority`
(durable-run + iteration-anchor validation, `AdmittedVerificationRecord`
mint) with the trait's `record_verification` consuming the mint; the
adapter's inherent `apply_settlement_row` is `#[cfg(test)]`; earning
callers updated.

**Re-derived fail-first evidence (recovered-honest — the builder's original
RED run was lost to the usage-limit interruption):** git HEAD (`62d1a5f`)
IS the pre-COR-1 tree (COR-1 landed uncommitted). A scratch detached
worktree of HEAD rebuilt the probe's C1 attack verbatim in a standalone
probe crate (path dep on gauntlet-ports only; no repo-tree changes):

- RED at HEAD: `the_c1_attack_bypasses_every_16_1_gate ... ok. 1 passed` —
  hand-built PASS doc (public `VerificationRecordDoc::new`) → raw
  `record_verification(&doc)` → `SettlementCommitAuthority::admit` →
  `commit_settlement` → settlement row, with no §16.1 gate consulted.
  HEAD's trait signature confirmed by `git show HEAD:` (`record_verification
  (&self, _record: &VerificationRecordDoc, ...)`).
- Closure, live tree, leg 1 (raw-doc fake): `error[E0053]: method
  'record_verification' has an incompatible type for trait ... expected
  'AdmittedVerificationRecord', found 'VerificationRecordDoc'`.
- Closure, live tree, leg 2 (trait signature adopted): `error[E0308]:
  mismatched types ... expected '&AdmittedVerificationRecord', found
  '&VerificationRecordDoc'` at the attack's call site — and the minted type
  has no constructor outside `producer_authority` (pinned in-crate by
  `the_minted_verification_record_has_no_public_constructor`), so only the
  authority's governed reads (undurable run / stale iteration refusals,
  pinned by the COR-1 sibling tests) can produce one.

This is also the compiled out-of-crate probe the Task-12/13 structural
tests deferred to "the confirmation round's job".

## COR-2 — pairing + claim-outcome at service entry (probe M3)

- RED: `settle::tests::a_fail_claim_with_a_stale_pass_readiness_refuses_
  with_zero_durable_writes` (pre-fix form, readiness still a request field)
  failed with `got Err(DomainTransition(SettledWithoutPassVerification
  { unit_id: "AK-9501", outcome: Fail }))` — the service never read the
  settling claim's outcome; the FAIL document, settlement event, and
  settlement row all landed before the post-commit domain transition
  refused. Probe M3 reproduced verbatim.
- Fix: `SettleRequest` drops the `readiness` field; `settle()` checks
  `claim.record().outcome() == Pass` at entry (typed `VerificationNotPass`,
  outcome named — three states preserved) and re-derives conditions 1–2
  internally via `settlement_precondition(&claim, &run_id)` (new
  `SettleError::ReadinessNotEarned(PreconditionFailure)`); both precede
  every durable write. The stale-token pairing is unrepresentable at the
  type level. Controller: the pre-derivation match is gone; non-PASS
  claims reach the service's entry gate and pin forensic evidence through
  the existing refusal arm.
- GREEN: app 327/327 (+3: FAIL entry refusal with zero writes; distinct
  INCONCLUSIVE naming; internal re-derivation refusing unpinned evidence).

## COR-3 — verification-ref moored in the durable record (probe M1)

- RED: ports `one_pass_record_cannot_settle_n_distinct_verification_refs`
  failed with `a commit under invented ref vrf-forged-a must not admit over
  one PASS: Ok(AdmittedSettlement { ... "vrf-forged-a" ... })`.
- Fix: `VerificationRecordDoc`/`VerificationContent` gain `verification`
  (wire key `verification_ref`, strict decode: absent = `MissingField`,
  malformed = `ValueInvalid`; accessor `verification()`; round-trip and
  negative codec tests added). `admit` requires a durable PASS record whose
  ref EQUALS the commit's verification ref (typed `VerificationNotPass`
  naming both). The chain's `mint()` takes the ref; every `VerifiedClaim`
  arm passes the SAME ref its domain record carries. NO migration (records
  rows carry the doc payload).
- GREEN: ports 100/100 (+1), app 327/327, sqlite 46/46, cli batteries
  32/32.

## COR-4 — AlreadyCommitted compares the stored row (probe M2)

- RED: app `a_re_entry_diverging_on_evidence_is_a_typed_refusal_not_success`
  failed with `Ok(SettlementOutcome { committed_now: false, ... evidence:
  evd-AK-9501-b ... })` — the same §16.3 key re-entered as SUCCESS over
  DIFFERENT evidence, riding the old offset.
- Fix: new `SettlementAdmissionError::AlreadyCommittedDivergence(Box<
  AlreadyCommittedDivergenceRefusal>)` (boxed per the `ReferenceMutated`
  large-Err precedent); on a key hit `admit` compares the STORED row's
  evidence ref — agreement is the idempotent shape, divergence the typed
  integrity refusal.
- GREEN: ports 101/101 (+1 mechanism pin), app 328/328, sqlite 46/46.

## COR-5 — admit compares record.run_id() (probe M5)

- RED: ports `a_foreign_run_pass_record_never_admits_even_under_a_run_
  blind_store` failed with `Ok(AdmittedSettlement ...)` — the fake's
  run-blind `verification_records` fed a FOREIGN-run PASS to admission.
- Fix: one comparison — the unit-record filter also requires
  `record.run_id() == commit.run_id`.
- GREEN: ports 102/102, app 328/328, sqlite 46/46.

## COR-6 — operator identity escaped (probe m2)

- RED: app `the_operator_identity_cannot_inject_members_into_the_decision_
  record` failed with the injected member visible in the pinned record:
  `...,"operator_identity":"a\b","injected":"true",...`.
- Fix: `record_authority_decision` escapes the identity through the shared
  `crate::control::controller::escape_json_text`.
- GREEN: app 329/329 (+1: record parses, identity round-trips as one string
  member, no injected member — both crafted shapes).

## COR-7 — read defaults fail closed (probe m1)

- RED: ports `settlement_surface_read_defaults_fail_closed` (write-only
  stub) failed with `settlements fails closed, got Ok([])`.
- Fix: the `settlements` and `verification_records` trait defaults return
  the typed `ReadFailed` not-implemented refusal (mirroring the write
  defaults); scope is exactly the two probe-named reads.
- GREEN: ports 103/103 (+1), app 329/329, sqlite 46/46, proofs 31/31 — no
  composed path relied on the silent empties.

## Deviations (assertions unchanged; recorded)

1. B2-era settle tests and the COR-2 RED test were migrated to the
   `SettleRequest` shape without the dead `readiness` field; the RED test's
   post-fix form is `a_fail_claim_refuses_at_service_entry_with_zero_
   durable_writes` (the stale half of the attack has no parameter left).
2. Fixture docs gain the COR-3 field: ports `pass_record` and the
   contract-suite `verification_doc` carry their commits' refs
   deliberately; app `claim_document` carries a deliberately distinct ref;
   scripted model payloads (ladder, precondition, p12) carry their own
   claim's ref (the chain mints the durable doc under the dispatch's ref);
   sqlite adapter, tamper_evidence, crash_helper, export_stack fixtures
   carry their commits' refs.
3. export_stack's golden line list is UNCHANGED (the record line's payload
   now carries `verification_ref`; no exact-byte pin existed).
4. p4/p6 pinned-refusal substrings: "the verification outcome is FAIL"
   (the precondition layer's text, pinned via the controller's pre-COR-2
   precondition call) → "the settling claim's own outcome is FAIL" (the
   service entry gate's text). The pinned-forensic-evidence assertions
   themselves are unchanged.
5. Clippy-driven: the COR-4 variant is boxed (large-Err bar), and the
   COR-7 test's bound uses the crate's BASE_INSTANT + BOUND_SPAN idiom
   (hardcoded-timeout arch rule).

## Files touched (this slice)

- `crates/gauntlet-ports/src/verification_record.rs` — COR-3 field, codec,
  accessor, tests.
- `crates/gauntlet-ports/src/producer_authority.rs` — COR-3/4/5 admit
  checks + variant, fixtures, tests.
- `crates/gauntlet-ports/src/state_store.rs` — COR-7 read defaults + test.
- `crates/gauntlet-ports/src/contract/state_store.rs` — COR-3 suite fixture.
- `crates/gauntlet-app/src/settle/mod.rs` — COR-2 service/request/error,
  COR-6 escape, COR-3/4 fixtures and tests.
- `crates/gauntlet-app/src/control/controller.rs` — COR-2 call site.
- `crates/gauntlet-app/src/verify/authority.rs` — COR-3 mint wiring.
- `crates/gauntlet-app/src/verify/ladder.rs`,
  `crates/gauntlet-app/src/verify/precondition.rs` — COR-3 test payloads.
- `crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs`,
  `.../src/bin/crash_helper.rs`, `.../tests/tamper_evidence.rs` — COR-3
  fixtures.
- `crates/gauntlet-cli/tests/export_stack.rs`,
  `.../p12_missing_is_not_passing.rs` — COR-3 fixtures/payload.
- `crates/gauntlet-cli/tests/p4_corrective_loop.rs`,
  `.../p6_budget_control.rs` — COR-2 pinned-text deviation.

(The remaining modified files in `git status` — doctor, arch-tests,
ports `lib.rs`/`stubs.rs`, `single_writer.rs`, and the task reports — are
the pre-existing recovered COR-1/B2/B3 state, untouched by this slice.)

## Gates (verbatim tails, run personally 2026-08-27)

```text
just ruler
  verify-ruler.sh: PASS — answer key and manifest digests match the frozen
  lock (/home/sprime01/projects/gauntlet/.gauntlet/reference.lock)

just check-package gauntlet-ports && just test-package gauntlet-ports
  Finished `dev` profile [unoptimized] target(s) in 2.32s
  Summary [   0.148s] 103 tests run: 103 passed, 0 skipped

just check-package gauntlet-app && just test-package gauntlet-app
  Finished `dev` profile [unoptimized] target(s) in 5.07s
  Summary [   0.644s] 329 tests run: 329 passed, 0 skipped

cargo nextest run --test p4_corrective_loop --test p6_budget_control \
  --test p12_missing_is_not_passing --test p2_context_isolation
  Summary [   0.424s] 31 tests run: 31 passed, 0 skipped

just lint && just boundaries
  Finished `dev` profile [unoptimized] target(s) in 14.05s
  cargo fmt --check            (lint exit 0)
  Summary [  26.878s] 215 tests run: 215 passed, 0 skipped
```

Additional evidence beyond the required gates: sqlite adapter package
`46 passed`; `export_stack` `1 passed`; `tamper_evidence` `9 passed`;
workspace `cargo check --all-targets` clean.

Untouched per constraints: `.wayfinder/`, `.gauntlet/reference.lock`,
`deny.toml`, nextest config, `AGENTS.md`, `.jolli/`, `.tmp/`,
`.agents/prompts/`, applied migrations (COR-3 needed none). No sub-agents;
no parallelism increases; `just ruler` PASS before starting and after
finishing.
