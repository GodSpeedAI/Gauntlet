# Task 11 — teeth (round 1)

Proof that the P12/deterministic-first tests discriminate: each row neuters
ONE production check, runs the named proof test, watches it FAIL, then
reverts. Every neutering was performed on the working tree, the failing run
was captured verbatim, and the revert was verified byte-identical against a
pre-neuter backup of the file (`diff` clean) — the proof files are new and
untracked, so `git checkout` could not be used (manual inverse restore from
backup). The full proof suite was re-run green (11/11) after the last
revert.

All failing-test line numbers refer to
`crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs` as it stands at
round-1 completion (formatting stable under `cargo fmt --check`).

## Teeth table

| # | Neutered production check (file) | Failing proof test | Failure (verbatim) |
|---|---|---|---|
| 1 | Recency/staleness: §14.3 condition 3 in `GovernedVerification::binding_class` (`crates/gauntlet-app/src/verify/authority.rs`, the `production.iteration_no() < governed iteration` refusal) commented out | `stale_evidence_from_a_prior_iteration_is_inconclusive_stale` (also `a_request_lying_about_recency_producer_and_locator_cannot_make_stale_fresh`) | `thread 'stale_evidence_from_a_prior_iteration_is_inconclusive_stale' panicked at crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs:801:5:` `assertion \`left == right\` failed` `left: Pass` `right: Inconclusive` |
| 2 | Producer: §14.3 condition 2 in `binding_class` (`verify/authority.rs`, the `Producer::Human` refusal) commented out | `instrument_produced_evidence_never_satisfies_a_human_producer` | `thread 'instrument_produced_evidence_never_satisfies_a_human_producer' panicked at crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs:884:5:` `assertion \`left == right\` failed` `left: Pass` `right: Inconclusive` |
| 3 | Deterministic-first: the §14.2 ascending-position sort in `Ladder::new` (`crates/gauntlet-app/src/verify/ladder.rs`) reversed to descending — the model rung dispatches first | `the_deterministic_rung_decides_when_the_model_rung_could_too` | `thread 'the_deterministic_rung_decides_when_the_model_rung_could_too' panicked at crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs:1020:5:` `assertion \`left == right\` failed` `left: ModelJudgment` `right: TestReport` |
| 4 | Resolvability: §14.3 condition 1 in `binding_class` (`verify/authority.rs`, the `resolved.is_none()` refusal) commented out | `absent_store_evidence_is_inconclusive_at_every_layer_never_pass` | `thread 'absent_store_evidence_is_inconclusive_at_every_layer_never_pass' panicked at crates/gauntlet-cli/tests/p12_missing_is_not_passing.rs:700:5:` `assertion \`left == right\` failed` `left: Pass` `right: Inconclusive` |

## What each tooth rules out

1. **Recency.** With the check gone, evidence from a prior iteration is
   verified and the chain mints PASS — exactly the P12-class regression
   (missing/stale evidence becoming passing evidence) the plan's done-when
   tooth requires a test to catch. The lying-request twin fails the same
   way: without the durable recency fact, the lie becomes the decision.
2. **Producer.** Without the producer check, an instrument's test report
   satisfies a HUMAN-producer sign-off requirement (PASS), collapsing the
   §5.3.2 "never: model assertion" class boundary.
3. **Deterministic-first.** With the preference reversed, the model rung
   decides a claim the focused-test rung can decide; the proof catches it
   twice — the ladder-order assertion fires first (`ModelJudgment` where
   `TestReport` must be), and the record-identity assertion plus the
   zero-model-dispatches assertion behind it would fire next.
4. **Resolvability.** Without the store-side resolution check, a production
   record whose ref never landed in the evidence store is verified over a
   present artifact file: PASS from absent evidence — the core P12
   inversion, caught at the first layer assertion.

## Reproduction (one tooth at a time)

```bash
# 1. neuter: comment out the §14.3 condition-3 refusal in
#    crates/gauntlet-app/src/verify/authority.rs::binding_class
cargo nextest run -p gauntlet-cli --test p12_missing_is_not_passing stale
# 2. neuter: comment out the condition-2 (Producer::Human) refusal
cargo nextest run -p gauntlet-cli --test p12_missing_is_not_passing producer
# 3. neuter: wrap the Ladder::new sort key in std::cmp::Reverse
cargo nextest run -p gauntlet-cli --test p12_missing_is_not_passing deterministic
# 4. neuter: comment out the condition-1 (resolved.is_none()) refusal
cargo nextest run -p gauntlet-cli --test p12_missing_is_not_passing absent_store
```

Each command must FAIL while neutered and PASS after the inverse revert.
Ran 2026-08-21 under rustc 1.98.0. Panic line numbers were re-anchored to
the final file after two post-run `clippy::arc_with_non_send_sync` allows
shifted the file by +6 lines (the failing assertions are unchanged; the
cited lines are where a re-run panics today). Verified post-revert:
`cargo nextest run -p gauntlet-cli --test p12_missing_is_not_passing` →
`Summary [ 0.074s] 11 tests run: 11 passed, 0 skipped`.

## Correction round 1 — teeth (2026-08-22)

Same discipline as round 1 (neuter one production check, run the named
test, capture verbatim, revert byte-identical against a /tmp backup,
re-green). Raw captures: `correction-failfirst-app.log`. Two rows are
compile-level (the seal and the C1 mint replay used a temporary re-added
seam — the permanent C1 test cannot call a nonexistent parameter, which
is the point).

| # | Neutered production check (file) | Failing test | Failure (verbatim, abridged — full text in the log) |
|---|---|---|---|
| 5 | C1 seal: `Ladder::new` referenced from non-test lib code, normal build (`verify/mod.rs` probe) | the build itself | `error[E0599]: no associated function or constant named 'new' found for struct 'Ladder'` |
| 6 | C1 mint: the pre-fix per-call ladder seam re-added (`verify_claim_opt(.., Some(&forged), ..)` + temporary replay test) | `c1_failfirst_replay_the_forged_ladder_mints_through_the_per_call_seam` (temporary; permanent pin is `a_hand_built_rung_cannot_mint_a_verified_claim`) | `the forged rung must never decide the claim` `left: Pass` `right: Inconclusive` |
| 7 | C2: `InstrumentFailed` arm in `Ladder::select_and_verify` (`verify/ladder.rs`) reverted to fall-through | `a_supported_broken_rung_terminates_selection_and_never_reaches_the_model` | `got Ok(Decided { rung: LadderRung { kind: ModelJudgment, ... outcome: Pass ... } })` |
| 8 | C3 chain: content-digest comparison in `binding_class` (`verify/authority.rs`) removed | `store_bytes_that_are_not_the_recorded_content_are_refused` | `swapped store bytes must be a typed refusal naming the digests, got Ok(Pass)` |
| 9 | C3 store: sqlite write-once pre-check ignores a differing content digest (`adapter-state-sqlite/src/state_store.rs`) | `the_production_content_digest_is_durable_and_write_once` | `assertion failed: matches!(.. &same_facts_other_digest .., Err(StateStoreError::WriteFailed { .. }))` |
| 10 | C4: the precondition's pin consult (`verify/precondition.rs`) neutered to unconditional success | `a_stooge_store_cannot_satisfy_the_settlement_precondition` | `assertion failed: matches!(.. Err(PreconditionFailure::EvidenceUnpinned { .. }) ..)` |
| 11 | C5: the durable-iteration validation in `for_run` (`verify/authority.rs`) removed | `an_iteration_no_durable_checkpoint_supports_is_refused` | `assertion failed: matches!(.. Err(VerificationAdmissionError::IterationUnsupported { .. }) if durable.get() == 4 && requested.get() == 1)` |
| 12 | C6.1: store-read error mapped back to `EvidenceUnresolvable` in `binding_class` | `an_evidence_store_read_failure_is_a_typed_refusal_not_a_class` | `got Inconclusive with class Some(EvidenceUnresolvable)` |
| 13 | C6.2: the locator equality check in `binding_class` removed (leaking fixture store) | `the_locator_check_bites_a_store_that_leaks_off_locator_productions` | `left: Pass` `right: Inconclusive` |
| 14 | C6.3a: the `result.role() == Verifier` check in `ModelRung::verify` removed | `a_model_result_under_another_role_is_refused` | `got Ok(VerificationRecord { ... outcome: Pass ... })` |
| 15 | C6.3b: the `result.iteration_no()` check in `ModelRung::verify` removed | `a_model_result_at_another_iteration_is_refused` | `got Ok(VerificationRecord { ... outcome: Pass ... })` |
| 16 | C6.3c: `RoleInvocationSpec.iteration_no` hardcoded back to 1 | `the_model_rung_dispatch_carries_the_governed_iteration` | `InstrumentFailed { detail: "the model rung's result names iteration 1, but this verification is governed at iteration 2" }` |
| 17 | C6.4: `verifier_envelope` reverted to raw string interpolation | `the_verifier_envelope_is_serialized_json_even_with_hostile_text` | `json: unexpected byte 0x71 at offset 24` |
| 18 | C6.5: `is_pinned` reverted to the following `exists()` (evidence-fs `store.rs`) | `a_symlinked_object_path_is_not_pinned_membership` | `is_pinned followed the symlink and reported membership over an aliased object` |
| 19 | C6.6: the kind-vs-verifier consistency check in `Ladder::new` removed | `a_rung_whose_kind_and_verifier_disagree_is_refused_at_construction` | `assertion failed: matches!(.. Err(LadderError::KindVerifierMismatch { .. }) ..)` |

What the new teeth rule out, in one line each: (5) any code outside the
audited seam constructing a rung set; (6) a per-call caller choosing the
instrument of record; (7) a dearer rung — least of all the model —
inheriting a mechanically-broken rung's claim; (8) store bytes that are
not the recorded content certifying a PASS; (9) a production record's
content digest being rewritten; (10) a caller-presented oracle satisfying
§16.1 condition 2; (11) an iteration anchor no durable state supports;
(12) an infrastructure failure degrading into a binding class;
(13) a store adapter's locator-filter regression leaking off-locator
productions into a PASS; (14/15/16) a model result bought under another
role, another iteration, or a hardcoded-iteration dispatch;
(17) hostile claim text corrupting the context envelope;
(18) pin membership over an aliased object; (19) a mis-labeled rung
occupying a deterministic position.

Verified post-revert (after the LAST revert, all backups diff-clean):
`verify::` 43/43, sqlite 37/37, fs 9/9 — and the full gates at the end of
the round: `just test` 778/778, boundaries 194/194, ruler PASS.

## Correction round 2 — teeth (2026-08-22)

Same discipline (neuter one production check on the working tree, run the
named test, capture verbatim, restore from a /tmp backup verified
byte-identical with `cmp`, re-green). Raw captures:
`correction-failfirst-round2.log`. One row is a LIVE manifest probe (F2 —
the neuter is the regression itself, added to the real workspace) and one
is a compile-level seal probe (F3).

| # | Neutered production check (file) | Failing test | Failure (verbatim, abridged — full text in the log) |
|---|---|---|---|
| 20 | F1: the locator-half content binding in `GovernedVerification::binding_class` (`verify/authority.rs`, the `read_locator_content` + `LocatorContentMismatch` refusal) commented out | `a_doctored_workspace_file_cannot_ride_the_locator_to_a_pass` (app, `verify::authority::tests`) | `a doctored workspace file must be a typed locator-content refusal naming the digests BEFORE dispatch, got Ok(Pass)` |
| 21 | same neuter, composed layer (REAL sqlite state, REAL fs store, REAL test-report adapter, run-pinned honest evidence) | `a_doctored_workspace_file_cannot_ride_the_locator_to_a_pass` (p12 test 12) | `a doctored workspace file must be refused before dispatch, never mint an outcome; got Pass (settlement readiness granted: true)` |
| 22 | F2: a production composition edge added to the LIVE workspace (`crates/gauntlet-cli/Cargo.toml` `[dependencies] gauntlet-app = { workspace = true, features = ["composition"] }`) | `deps::repo::the_composition_feature_seal_holds` (arch) | `composition-seal: 1 violation(s) — crates/gauntlet-cli/Cargo.toml: gauntlet-cli enables feature 'composition' on 'gauntlet-app' via [dependencies] …` |
| 23 | F3: `ModelRung::new` referenced from non-test lib code (temporary `verify/mod.rs` probe), normal build | the build itself | `error[E0599]: no associated function or constant named 'new' found for struct 'ModelRung' in the current scope` |

What the round-2 teeth rule out, one line each: (20) a doctored workspace
file riding the app-layer chain to a verdict over bytes it is not; (21)
the same through the composed REAL-adapter path — the confirmation
critic's executed attack, now a permanent failing-before capture ending in
`settlement readiness granted: true`; (22) any production (non-dev),
foreign-member, target-scoped, or root-table edge silently enabling the
sealed composition seam; (23) any rung-shaped value constructed outside
the audited §7.1 seam in a normal build.

Reproduction (one tooth at a time):

```bash
# 20/21. neuter: comment out the locator-half binding block in
#    crates/gauntlet-app/src/verify/authority.rs::binding_class
cargo nextest run -p gauntlet-app a_doctored_workspace_file
cargo nextest run -p gauntlet-cli --test p12_missing_is_not_passing a_doctored
# 22. add a production `features = ["composition"]` edge to a real
#     Cargo.toml (revert after)
cargo nextest run -p gauntlet-arch-tests the_composition_feature_seal_holds
# 23. reference ModelRung::new from non-test lib code (feature off)
just check-package gauntlet-app
```

Each command must FAIL while neutered/probed and PASS after the inverse
revert. Ran 2026-08-22 under rustc 1.98.0. Verified post-revert (after the
LAST revert, all backups `cmp`-clean): `verify::` 44/44, ladder 17/17,
arch 197/197, p12 12/12 — and the full round-2 gates: `just test` 783/783,
boundaries 197/197, ruler PASS before and after.
