# Task 11 — correction round 1 report

Builder: CORRECTION FINISHER (second session; the first correction builder
died to a usage limit mid-edit — see "Recovery note"). Mandate:
`correction-brief.md` items C1–C6 + C8a against `correction-adjudication.md`.
Raw fail-first captures: `correction-failfirst-app.log` (verbatim panic text
for every item). Gates at the end; `just ruler` PASS before and after.

## Recovery note — reconciling the dead correction builder's footprint

The first correction builder (2026-08-21 16:33–16:45) left the tree
NON-COMPILING and its result contract empty (`correction-failfirst-app.log`
was 0 bytes; nothing appended to `builder-checkpoints.md`). Reconciliation
against the adjudication, file by file:

- **Landed by the dead builder (kept, after review, as sound):** the C3
  ports surface (`EvidenceProduction` gained the `content: Digest` field +
  accessor + docs), sqlite migration `0007_evidence_production_content.sql`
  (DEFAULT '' for ALTER TABLE only; strict decoder refuses empty as
  `Corrupt`), the sqlite `content_digest` column plumbing (write, both
  reads, `production_from_parts`), `record`'s conflict refusal extended to
  the digest, the C1 `composition` cargo feature + `#[cfg(any(test,
  feature = "composition"))]` sealing of `Ladder::new`/`LadderRung::new`/
  `ModelRung::rung`, `for_run(run, iteration, ladder)` + `verify_claim`
  losing the ladder parameter, `GovernedVerification` holding the ladder,
  `VerifiedClaim` carrying its chain's evidence store (`chain_evidence()`,
  crate-private), C4's store-free `settlement_precondition(claim, run,
  deadline)`, C5's `IterationUnsupported` admission check, C6.1's typed
  `VerificationRefusal::Evidence` for store-read failures, C6.3's role +
  iteration result checks and the `iteration_no` constructor param on
  `ModelRung`, C6.4's `JsonValue`-built envelope, C6.5's `is_pinned`
  symlink refusal + test, C6.6's `KindVerifierMismatch` construction
  check + tests, and most C-item tests. C8a's denylist comment was already
  fixed by the orchestrator (mtime 16:25, before dispatch).
- **Broken and repaired by this session (compile):** (1) the C6.4
  `verifier_envelope` had a dropped `),` closing the "requirement" tuple —
  the reported `mismatched closing delimiter: ]`; (2) `enum
  CandidateRejection` sat inside `impl GovernedVerification` — the reported
  `enum is not supported in traits or impls` — moved to module scope;
  (3) `Checkpoint::iteration_no` (a field, not a method); (4) two unused
  imports; (5) a missing `#[allow(clippy::arc_with_non_send_sync)]`.
- **Half-landed and completed by this session:** the C3 authority test
  built its chain from a FRESH fixture pair instead of the recorded one
  (the swapped bytes were invisible — test asserted the wrong failure);
  ladder round-1 tests still pinned pre-C2 behavior (`InstrumentsFailed`
  variant that no longer exists; a fall-through-past-a-broken-rung test
  that C2 forbids); `ModelRung`'s scripted runner hardcoded the payload
  iteration (no dispatch-carry discrimination existed); p12 still called
  the round-1 signatures; no fail-first evidence existed for any item.
- **Landed new by this session:** the two ladder test rewrites, the C6.3c
  dispatch-carry test, `content_digest` exported from `gauntlet-app`
  (the composition root computes the producer-side digest through the
  app's own rule — no second SHA-256 implementation, no CLI external dep),
  the CLI `composition` dev-dependency edge, the p12 adaptation, the
  dead `VerificationRefusal::InstrumentsExhausted` variant removed
  (C2 replaced its terminal role with `LadderError::RungFailed`, which
  reaches callers as `VerificationRefusal::Ladder`), and every fail-first
  capture below.

Design decision to flag for the confirmation critic (C1 letter vs shape):
the adjudication says "sealed `pub(crate)`". The landed shape seals the
constructors with `#[cfg(any(test, feature = "composition"))]` instead —
in a NORMAL build of the crate they do not exist at all (strictly stronger
than `pub(crate)` for external callers), and the `composition` feature is
the audited §7.1 seam at which a composition root (the CLI's proof wiring;
Tasks 12/20 production wiring) builds the rung set. A literal `pub(crate)`
seal alone would make the p12 composition proof uncompilable (an external
crate cannot construct the rung set at all). The ONLY enabling edge in the
workspace is gauntlet-cli's dev-dependency; no production edge enables it.
Seal evidence: referencing `Ladder::new` from non-test lib code fails
E0599 in a normal build (capture below).

---

## C1 — unforgeable ladder (B-M1; prereg R1/A5)

**Fix.** The rung set is earned authority injected at the governed open:
`VerificationAuthority::for_run(run, iteration, LADDER, deadline)`;
`verify_claim(request, deadline)` has no ladder parameter;
`Ladder::new`/`LadderRung::new`/`ModelRung::rung` exist only under
`cfg(test)` or the `composition` feature (the `AdmittedComparison`
precedent — no public constructor in a normal build).

**Failing-before evidence (mint replay).** The pre-fix seam (per-call
ladder argument) re-added temporarily as `verify_claim_opt(request,
Some(&forged), deadline)`; a temporary replay test performs the exact
attack (self-consistent forged rung: identity and evidence echo both
attacker-supplied, recorded production, resolving bytes) and asserts the
post-fix expectation:

```text
assertion `left == right` failed: the forged rung must never decide the claim
  left: Pass
 right: Inconclusive
```

**Failing-before evidence (seal).** `Ladder::new` referenced from non-test
lib code, normal build (`cargo check -p gauntlet-app`, feature off):

```text
error[E0599]: no associated function or constant named `new` found for struct `Ladder` in the current scope
   --> crates/gauntlet-app/src/verify/mod.rs:64:21
```

**The test (permanent).** `verify::authority::tests::
a_hand_built_rung_cannot_mint_a_verified_claim` — the forged one-rung
ladder is constructible in-crate but inert: the governed verification
opens over the honest rung set, the record is INCONCLUSIVE, and the
forged instrument is nowhere on it.

**Bounded claim.** No per-claim caller of a `GovernedVerification` can
choose a rung; a rung set can only enter at `for_run`, i.e. at the
composition root's audited seam. NOT claimed: the composition root itself
is trusted (that is §7.1's position, unchanged), and B-M3's ungated
production write remains the recorded Task-12 residual.

## C2 — no model reachability below a deciding-but-broken rung (A-M1)

**Fix.** In `Ladder::select_and_verify`, `VerifierError::InstrumentFailed`
at a SUPPORTED rung returns `LadderError::RungFailed { kind, instrument,
detail }` — terminal, naming the broken rung. Fall-through past a rung
remains ONLY for `RequirementUnsupported` (support/dispatch
contradiction). The chain surfaces it as `VerificationRefusal::Ladder(..)`;
the dead round-1 `InstrumentsExhausted` refusal variant was removed (its
terminal role is now this path's).

**Failing-before evidence.** InstrumentFailed arm neutered back to
fall-through:

```text
a mechanically-broken deciding rung must terminate selection with the typed
refusal naming it, got Ok(Decided { rung: LadderRung { kind: ModelJudgment,
instrument: "gauntlet-verify-model" }, record: VerificationRecord { ...
outcome: Pass, instrument: InstrumentName { raw: "gauntlet-verify-model" } } })
```

Exactly the §14.2 defect: the model minted a settling PASS for a property
a deterministic rung could have decided.

**The tests (permanent).** `a_supported_broken_rung_terminates_selection_
and_never_reaches_the_model` (typed refusal names SchemaCheck +
"fixture-schema-verifier"; the scripted runner's dispatch count is 0) and
the rewritten `a_rung_failing_as_an_instrument_is_a_typed_refusal_naming_
it_not_an_outcome`. The round-1 test that pinned the old fall-through
(`a_failing_cheapest_rung_falls_through...`) was rewritten to the one
legitimate fall-through — `a_rung_declining_at_dispatch_falls_through_to_
the_next_that_can_decide` (RequirementUnsupported). Recorded act: two
round-1 ladder tests rewritten because the adjudication reverses the
behavior they pinned; no assertion weakened (both now pin the stricter
behavior).

**Bounded claim.** No rung ABOVE a mechanically-failed supported rung is
dispatched, model least of all. NOT claimed: retry/repair policy for the
broken instrument (P12's "improve the instrument" resolution path is the
correction loop's business, upstream of the ladder).

## C3 — bind examined bytes to pinned bytes (B-M4)

**Shape chosen (and why).** The digest-mirror shape the adjudication
prefers: the producer-side `EvidenceProduction` record carries the content
digest of the bytes ingested at that locator (ports `content: Digest`,
mirroring `ObservationBinding`); sqlite migration 0007 persists it
(`content_digest TEXT NOT NULL DEFAULT ''`, DEFAULT only to satisfy ALTER
TABLE — no rows predate it; the strict decoder refuses empty as `Corrupt`);
the chain re-derives the digest of whatever the store serves at the
production's ref and refuses a mismatch as the typed
`VerificationRefusal::EvidenceContentMismatch { evidence, recorded,
served }` — never a binding class, never an outcome. B.5/§5.3.6 field
shapes untouched (the digest lives on the Task-11-owned production
surface, not the frozen record shapes). The alternative (instruments read
through the store) was not chosen: it would move the defect into the
adapters' seam and couple instruments to store credentials; the digest
discipline closes the recorded-content gap at the durable record where
§14.3 already reads.

**Failing-before evidence (chain half).** Content comparison neutered:

```text
swapped store bytes must be a typed refusal naming the digests, got Ok(Pass)
```

**Failing-before evidence (store half).** The sqlite write-once pre-check
neutered to ignore a differing digest:

```text
assertion failed: matches!(store.record_evidence_production(&run_id(),
&same_facts_other_digest, bound()), Err(StateStoreError::WriteFailed { .. }))
```

**The tests (permanent).** `store_bytes_that_are_not_the_recorded_content_
are_refused` (the swapped-bytes attack through the full chain; asserts the
recorded AND served digests on the refusal) and sqlite's
`the_production_content_digest_is_durable_and_write_once` (round-trip +
conflicting-digest refusal). The p12 `Fixture::produce` records the digest
through the app's exported `verify::content_digest` (one hashing rule, no
second implementation at the composition root).

**Bounded claim.** The record certifies a ref whose served bytes are
provably the recorded content, whoever serves them. NOT claimed: the
instrument's locator read is itself store-mediated (the instrument still
examines the locator; binding the INSTRUMENT's read to the store is the
read-through alternative, not chosen), and the producer-side write remains
ungated (B-M3/C7, Task-12 ledger).

## C4 — precondition's store must be the claim's own (B-M5)

**Fix.** `VerifiedClaim` carries the evidence store of the chain that
minted it (`chain_evidence()`, crate-private);
`settlement_precondition(claim, settling_run, deadline)` has no store
parameter. §16.1 condition 2 is re-derived from the claim's OWN store.

**Failing-before evidence.** Pin consult neutered to unconditional
success (the stooge-oracle shape):

```text
assertion failed: matches!(settlement_precondition(&claim, &RunId::new(RUN_ULID)
.expect("run"), bound()), Err(PreconditionFailure::EvidenceUnpinned { reference })
if reference.as_str() == "evd-pinned")
```

**The test (permanent).** `a_stooge_store_cannot_satisfy_the_settlement_
precondition` — the chain's own store resolves the bytes but never pinned
them; pre-correction a per-call store holding the ref AND a pin satisfied
conditions 1–2 here; now the only store consulted refuses.

**Bounded claim.** No caller of the precondition can select the evidence
oracle. NOT claimed: the chain's own store is more honest than its adapter
(adapter substitution remains §7.1's position).

## C5 — validate the iteration coordinate (B-M2 first half)

**Fix.** `for_run` reads the run's latest durable checkpoint (`resume`;
iteration 1 when none exists) and refuses a mismatched anchor with the
typed `VerificationAdmissionError::IterationUnsupported { run_id,
requested, durable }` — an old anchor would re-fresh prior-iteration
evidence; a future one certifies a pass that never happened.

**Failing-before evidence.** The validation block removed:

```text
assertion failed: matches!(authority.for_run(..., IterationNo::new(1), ...),
Err(VerificationAdmissionError::IterationUnsupported { durable, requested, .. })
if durable.get() == 4 && requested.get() == 1)
```

**The test (permanent).** `an_iteration_no_durable_checkpoint_supports_
is_refused` — both directions refused (old anchor 1 vs durable 4; future
anchor 5 vs durable 4) and the durable anchor itself opens.

**Bounded claim.** The anchor is validated against the durable
checkpoint. NOT claimed: the store-side write validation (a production
declaring an iteration beyond the run's durable iteration) — C7/Task-12
seam, unchanged.

## C6 — minors batch

1. **C6.1 store-read errors are a typed refusal** (§21 locus). Fix: the
   dead `VerificationRefusal::Evidence` is now constructed —
   `CandidateRejection::Refused` separates infrastructure failure from
   binding classes (the candidate walk returns it instead of classing
   it). Evidence (neutered back to the class mapping):
   `a store read failure must be a typed refusal, not an outcome; got
   Inconclusive with class Some(EvidenceUnresolvable)`. Test:
   `an_evidence_store_read_failure_is_a_typed_refusal_not_a_class`.
2. **C6.2 the locator check is real with an app-layer tooth** (chosen
   option: real check + tooth, over the by-construction pin). The
   `VerifyingState` test fixture can `leak_off_locator()` (a
   contract-violating store); the check bites:
   `left: Pass / right: Inconclusive` under neuter. Test:
   `the_locator_check_bites_a_store_that_leaks_off_locator_productions`.
   p12 test 7's title already states what it exercises
   (`evidence_at_another_locator_is_unresolvable_not_the_required_evidence`
   — the no-production-at-locator path); the app-layer tooth is the new
   discriminating test, so no retitle was needed.
3. **C6.3 model-rung result validation carries role + iteration; the
   dispatch carries the governed iteration.** Fix: `result.role()` must be
   `Verifier`; `result.iteration_no()` must equal the rung's composed
   iteration; `RoleInvocationSpec.iteration_no` is the composed iteration
   (the `IterationNo::new(1)` hardcode is gone); `ModelRung::new` takes
   the iteration. Evidence: three neuters — role check removed (`got
   Ok(VerificationRecord { ... outcome: Pass ... })`), iteration check
   removed (same), dispatch hardcoded to 1 (`InstrumentFailed { detail:
   "the model rung's result names iteration 1, but this verification is
   governed at iteration 2" }`). Tests:
   `a_model_result_under_another_role_is_refused`,
   `a_model_result_at_another_iteration_is_refused`,
   `the_model_rung_dispatch_carries_the_governed_iteration` (new — the
   scripted runner echoes the dispatch's iteration in BOTH the role
   result envelope and the payload document).
4. **C6.4 envelope via the ports JSON serializer.** Fix:
   `verifier_envelope` builds a `JsonValue` object and serializes.
   Evidence: raw interpolation restored — `the envelope body must be
   valid JSON under hostile claim text: json: unexpected byte 0x71 at
   offset 24`. Test:
   `the_verifier_envelope_is_serialized_json_even_with_hostile_text`
   (quotes, backslash, newline, tab round-trip).
5. **C6.5 `is_pinned` refuses symlinked object paths.** Fix:
   `symlink_metadata` + regular-file discipline, the `get` precedent.
   Evidence: following `exists()` restored — `is_pinned followed the
   symlink and reported membership over an aliased object`. Test:
   `a_symlinked_object_path_is_not_pinned_membership` (unix).
6. **C6.6 `RungKind::deterministic()` wired, not dead.** Fix:
   `Ladder::new` refuses a rung whose kind and instrument disagree about
   determinism (`KindVerifierMismatch`); the flag now decides at
   construction, so "deterministic-first is structural" rests on a
   checked label, not a self-report. Evidence: the check removed —
   `assertion failed: matches!(mislabeled, Err(LadderError::
   KindVerifierMismatch { kind, .. }) if kind == RungKind::FileDigest)`.
   Test: `a_rung_whose_kind_and_verifier_disagree_is_refused_at_
   construction` (both directions).

## C8a — denylist comment fix (critic C MINOR-2)

The overclaiming comment at `denylist.rs` (formerly :105-107) was already
corrected in-tree before this round's dispatch (orchestrator,
2026-08-21 16:25): it now names the derived spellings
`verify-file-digest`/`digest-file-verify`/`verify-schema-check`/
`check-schema-verify` and states explicitly that bare `digest-file`/
`check-schema` literals pass (which is why the PIN test asserts the full
derived spellings). Verified against critic C's finding; no further edit
needed. Comment-only — no runtime fail-first is constructible; the
evidence is the text itself.

---

## Deliverable deltas beyond the item list (recorded acts)

- Round-1 ladder tests rewritten per C2 (see C2 section): the old
  fall-through test contradicted the adjudicated behavior.
- `VerificationRefusal::InstrumentsExhausted` removed (dead under C2's
  first-failure-terminates design).
- `gauntlet_app::verify::content_digest` exported (C3's one hashing rule
  for the composition root; keeps sha2 out of the CLI, whose external
  allowlist is empty by design).
- `gauntlet-cli` dev-dependency `gauntlet-app = { features =
  ["composition"] }` — the audited seam's only enabling edge.
- p12 proof adapted to the corrected signatures (ladder at `for_run`,
  fixture writes the durable checkpoint the anchor validation reads —
  the controller's §18 boundary act; producer records carry content
  digests; precondition takes the claim only; the scripted judgment
  echoes the governed iteration). No assertion weakened; every round-1
  scenario retained.
- The C3 authority test's fixture wiring repaired (it built its chain
  from a fresh store pair — the dead builder's bug, the test asserted
  the wrong failure).

## Gates

- Focused: `-p gauntlet-ports` 80/80; `-p gauntlet-adapter-state-sqlite`
  37/37; `-p gauntlet-app` 219/219 (43 `verify::`); evidence-fs 9/9;
  four verify adapters 15/15; `--test p12_missing_is_not_passing` 11/11.
- Full: `cargo fmt --all --check` clean; `just check` green; `just lint`
  (clippy -D warnings, all targets) green; `just deny` ok (advisories/
  bans/licenses/sources; pre-existing allowlist notes); `just boundaries`
  194/194; `just test` 778/778 (764 round-1 baseline + 1 sqlite content
  + 1 fs symlink + 12 app verify).
- `just ruler` PASS before and after.

## Bounded overall claim

Every FIX-NOW item of the adjudication is landed with demonstrated
fail-first evidence and a permanent discriminating test; the three named
attack proofs (hand-built rung, swapped bytes, stooge store) are
permanent tests. NOT claimed: B-M3/C7 (ungated production write,
store-side iteration validation — Task-12 ledger), crash-window replay
(Task-13), rungs 5–9/11, locator "contract-defined" matching (ports
revision), and the `produced_at` annex placeholder reconciliation (doc
note below).

## Doc note (allowed by the brief)

`produced_at` is an epoch-seconds INTEGER in the codec while the B.5
annex sample shows the string placeholder `""` — a reconciled decision
recorded by critic-a (A-m6/B-m9): the annex samples use `""` placeholders
for all strings; reconcile with `docs/schemas` when that lands
(Task 17/27). No code change.

DONE — Task 11 correction round 1: C1–C6 + C8a landed with fail-first
evidence (verbatim captures in correction-failfirst-app.log; reverts
byte-identical, suites re-green after each). Focused suites green (ports
80, sqlite 37, app 219, fs 9, adapters 15, p12 11); full gates green
(fmt/check/lint/deny/boundaries 194/ruler PASS before and after);
`just test` 778/778.
