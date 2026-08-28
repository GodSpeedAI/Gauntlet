# Task 13 / Correction round 1 checkpoints (COR-1..COR-7)

Pre-start gates (run personally, 2026-08-26):

- `just ruler` → `verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock (/home/sprime01/projects/gauntlet/.gauntlet/reference.lock)`
- Reconcile: `git status` matches the B2/B3 landed state recorded in `IN-FLIGHT.md`
  (controller.rs/settle/mod.rs/doctor/arch-tests modifications are B2 heal + B3;
  `correction-round1.md` does not exist yet; no prior COR checkpoints — this file
  is the start of the correction round). Dispatch ledger says Correction round 1
  DISPATCHED; nothing recorded as done; starting COR-1 fresh.

## COR-1 — capability-gate `record_verification` + test-gate `apply_settlement_row`

STARTED COR-1 (2026-08-26): ports recording authority (durable run + iteration
anchor validation, `AdmittedVerificationRecord` mint, trait write consumes it) +
`apply_settlement_row` becomes `#[cfg(test)]`; fail-first = the probe's C1 attack
asserted-as-refused must FAIL today (it succeeds), then the compile-level closure.

## RECOVERED-AND-VERIFIED (orchestrator, 2026-08-27 08:3x–08:4x EDT — 18th interruption instance)

The correction builder died to the sub-agent usage limit PAST its last
checkpoint write (21:15:44 EDT 08-26), having edited 12 files 21:19–21:26.

**What landed (unrecorded):** COR-1 COMPLETE — ports
`VerificationRecordAuthority` (+233 lines: durable run + iteration-anchor
validation, `AdmittedVerificationRecord` mint) with the trait's
`record_verification` consuming the mint; the adapter's inherent
`apply_settlement_row` now `#[cfg(test)]` (comment names the correction);
earning callers updated (crash_helper, tamper_evidence, export_stack, app
`ensure_verification_recorded`, suite stubs' signature migration).

**Incomplete in the window (fixed by the orchestrator per the standing
heal instruction):** the shared-suite fixture was left incoherent — the
builder lowered `checkpoint()` 2→1 AND `verification_doc()` 2→1, but the
owner-writes case's production anchor and the observation-bundle fixture
still said 2 (green-stub run RED: the production earning refused "declares
iteration 2"). Resolution: `checkpoint()` restored to 2 (the owner-writes
family anchors at 2); `verification_doc()` kept at 1 with an accurate
comment (the three settlement cases run WITHOUT a checkpoint — durable
iteration defaults to 1 — and the now-gated recording authority refuses a
doc declaring 2; the builder's direction was right for that family).
Plus `cargo fmt` over the two drifted files (the builder's fmt never ran).

**Orchestrator verification (personal):** ports **98/98** (green-stub
suite clean again), sqlite **46/46**, app **324/324**, proofs p4/p6/p12/p2
**31/31**, `just lint` exit 0, `just ruler` PASS, `just boundaries`
**215/215**, compile zero warnings.

**Verdict:** COR-1 LANDED (recovered, completed, verified); COR-2..COR-7
UNSTARTED (SettleRequest still carries the portable readiness; no
verification-ref field in the doc; no AlreadyCommittedDivergence; no
run_id comparison in admit; no identity escaping; read defaults still
empty). The builder's COR-1 fail-first evidence is LOST (no DONE entry) —
the C1 attack must be re-derived fail-first by the COR-2..7 finisher,
honestly recorded as recovered, never implied.

## COR-2 — service re-derives readiness internally + claim-outcome entry check

STARTED COR-2 (2026-08-27, finisher): `SettleRequest` drops `readiness`;
`settle()` checks `claim.record().outcome() == Pass` at entry and re-derives
conditions 1-2 through `settlement_precondition(&claim, run)` internally,
all before ANY durable write. Fail-first: probe M3 (FAIL-claim + stale
PASS-readiness through the service) asserted to refuse with ZERO durable
writes — expected RED today (the service never read the claim's outcome and
recorded the FAIL doc + committed before the domain transition refused).

DONE COR-2 (2026-08-27, finisher).

- RED (fail-first, pre-fix run): new test
  `settle::tests::a_fail_claim_with_a_stale_pass_readiness_refuses_with_zero_durable_writes`
  (pre-fix form: readiness still a request field) FAILED exactly as the probe
  predicted — `got Err(DomainTransition(SettledWithoutPassVerification
  { unit_id: "AK-9501", outcome: Fail }))`, i.e. the service never read the
  claim's own outcome and ran the FULL durable commit path (FAIL document
  recorded through the COR-1 recording authority, settlement event appended,
  settlement row written) before the post-commit domain transition caught
  the FAIL. Probe M3 reproduced verbatim. Tail:
  `FAIL ... settle::tests::a_fail_claim_with_a_stale_pass_readiness_refuses_with_zero_durable_writes`
  (`just test-package gauntlet-app` exit 100).
- Fix: `SettleRequest` drops the `readiness` field (claim + actor +
  tracked_residuals only); `settle()` checks `claim.record().outcome() ==
  Pass` at ENTRY (typed `VerificationNotPass`, outcome named — three states
  preserved) and re-derives conditions 1–2 internally through
  `settlement_precondition(&claim, &run_id)` (new `SettleError::ReadinessNotEarned(PreconditionFailure)`);
  both precede every durable write the service performs. The stale-token
  pairing is unrepresentable at the type level. Controller call site: the
  pre-derivation match is gone; non-PASS claims reach the service's entry
  gate and pin forensic evidence through the existing refusal arm (the
  `precondition`-keyed pin is superseded by the `settlement_refused`-keyed
  pin — no test pinned the old key).
- GREEN: `just test-package gauntlet-app` → `Summary [  0.905s] 327 tests
  run: 327 passed, 0 skipped` (+3: entry-refusal FAIL, distinct
  INCONCLUSIVE naming, internal re-derivation unpinned-evidence refusal).
  Deviations (assertions unchanged, signature migration only): the B2-era
  settle tests construct `SettleRequest` without the dead field; the
  RED test was reshaped post-fix into
  `a_fail_claim_refuses_at_service_entry_with_zero_durable_writes` (the
  stale half of the attack no longer has a parameter to travel through).

## COR-3 — verification-ref field on VerificationRecordDoc + admit requires durable-record ref == commit ref

STARTED COR-3 (2026-08-27, finisher): the doc gains the verification-ref
field (strict codec round-trip; the domain record already carries the id —
Annex-B samples are illustrative), and `SettlementCommitAuthority::admit`
requires the durable PASS record's ref to EQUAL the commit's verification
ref (§16.3 key component three becomes durable-moored). NO new migration.
Fail-first: the N-distinct-refs attack — after the first commit, each
further commit must refuse with NO duplicate settlement event.

DONE COR-3 (2026-08-27, finisher).

- RED (fail-first, pre-fix run): new ports test
  `producer_authority::tests::one_pass_record_cannot_settle_n_distinct_verification_refs`
  FAILED with `a commit under invented ref vrf-forged-a must not admit over
  one PASS: Ok(AdmittedSettlement { .. verification: "vrf-forged-a" .. })` —
  probe M1 reproduced: admit filtered unit+evidence only, so N invented
  refs each minted a distinct §16.3 key over ONE durable PASS.
  (`just test-package gauntlet-ports` exit 100.)
- Fix: `VerificationRecordDoc`/`VerificationContent` gain the
  `verification` field (wire key `verification_ref`, first after the schema
  token; strict decode — absent member is `MissingField`, malformed value
  `ValueInvalid`; accessor `verification()`); `SettlementCommitAuthority::
  admit` requires a durable PASS record whose ref EQUALS the commit's
  verification ref (typed `VerificationNotPass` naming both refs). The
  chain's `mint()` takes the ref and every `VerifiedClaim` arm passes the
  SAME ref its domain record carries — the durable doc names the
  verification it IS. NO migration (records rows carry the doc payload).
- GREEN: ports `100 passed` (+1), app `327 passed`, sqlite `46 passed`,
  cli proofs (export_stack/p12/p4/p6/p2) `32 passed`. Workspace
  `cargo check --all-targets` clean.
- Deviations (assertion intent unchanged, recorded): (a) fixture docs gain
  the field — producer_authority `pass_record` ("vrf-000900" = the honest
  commit's), contract-suite `verification_doc` ("vrf-000042" = the suite
  commit's), app `claim_document` (deliberately distinct), scripted model
  payloads in ladder/precondition/p12 (the chain mints the durable doc
  under the dispatch's ref; the payload's own ref is the model's claim),
  sqlite adapter + tamper_evidence + crash_helper + export_stack fixtures
  (deliberately their commits' refs). (b) export_stack's golden line list
  is UNCHANGED (the record line's payload now carries `verification_ref`;
  no exact-byte pin existed). (c) p4/p6 pinned-refusal substrings updated
  from "the verification outcome is FAIL" (the precondition layer's old
  text, pinned via the controller's pre-COR-2 precondition call) to "the
  settling claim's own outcome is FAIL" (the service entry gate's text,
  COR-2) — the pinned-forensic-evidence assertion itself is unchanged.

## COR-4 — AlreadyCommitted compares the stored row; typed divergence refusal

STARTED COR-4 (2026-08-27, finisher): the ports' `admit` reads the stored
row on a §16.3 key hit and requires evidence agreement (the key fixes
run+unit+verification); divergence is the typed
`SettlementAdmissionError::AlreadyCommittedDivergence` refusal, never
idempotent success. The service's re-entry arm then only ever succeeds over
an AGREEING row. Fail-first: probe M2's E2 evidence-divergence shape through
the service — pre-fix it returns Ok(committed_now=false) riding the old
offset while naming different evidence.

DONE COR-4 (2026-08-27, finisher).

- RED (fail-first, pre-fix run): new app test
  `settle::tests::a_re_entry_diverging_on_evidence_is_a_typed_refusal_not_success`
  FAILED with `the E2 divergence must be a typed refusal, not idempotent
  success: Ok(SettlementOutcome { event_log_offset: 2, committed_now:
  false, ... evidence: evd-AK-9501-b ...})` — probe M2 reproduced: the
  service's AlreadyCommitted arm discarded the stored row's fields, so the
  same (run, unit, verification) key re-entered as SUCCESS over DIFFERENT
  evidence, riding the old offset while the in-memory advance named E2 vs
  the row's E1.
- Fix: ports `SettlementAdmissionError` gains `AlreadyCommittedDivergence
  { run_id, unit_id, verification, committed_evidence, offered_evidence,
  event_log_offset }`; on a §16.3 key hit, `admit` compares the STORED
  row's evidence ref to the commit's — agreement is the idempotent
  `AlreadyCommitted` shape, divergence the typed integrity refusal. The
  service's re-entry success arm therefore only ever fires over an agreeing
  row.
- GREEN: ports `101 passed` (+1: the divergence-typed refusal and the
  agreeing-re-entry pin), app `328 passed`, sqlite `46 passed`.

## COR-5 — admit compares record.run_id() to the commit's run

STARTED COR-5 (2026-08-27, finisher): one comparison in `admit`'s unit-record
filter — `record.run_id() == commit.run_id` — plus a divergent fake-store
test (the ports' own `SettlementGateState` ignores run scoping on
`verification_records`, the discriminating instrument: a conforming-but-
sloppy store feeding a FOREIGN-run PASS record to admission). Fail-first:
the foreign-run record admits the commit today.

DONE COR-5 (2026-08-27, finisher).

- RED (fail-first, pre-fix run): new ports test
  `a_foreign_run_pass_record_never_admits_even_under_a_run_blind_store`
  FAILED with `a PASS of another run never admits this run's commit:
  Ok(AdmittedSettlement { ... run_id: 01ARZ...FAV ... })` — probe M5
  reproduced: the fake's run-blind `verification_records` fed a FOREIGN-run
  PASS to admission and `admit` (unit+evidence filtered only) minted the
  admission.
- Fix: one comparison — `admit`'s unit-record filter now also requires
  `record.run_id() == commit.run_id`; the foreign record counts for
  nothing, whichever store serves it.
- GREEN: ports `102 passed`, app `328 passed`, sqlite `46 passed`.

## COR-6 — escape the operator identity through escape_json_text

STARTED COR-6 (2026-08-27, finisher): `record_authority_decision`'s
`operator_identity` member is interpolated raw into the pinned
settlement_authority_decision JSON (probe m2 — `pin_limitation` escapes,
this site didn't). Fix: the shared `escape_json_text` (controller,
pub(crate) for exactly this). Fail-first: a crafted identity containing
quotes/backslashes injects members into (or corrupts) the pinned record.

DONE COR-6 (2026-08-27, finisher).

- RED (fail-first, pre-fix run): new app test
  `the_operator_identity_cannot_inject_members_into_the_decision_record`
  FAILED with the injected member visible in the pinned record:
  `...,"operator_identity":"a\b","injected":"true",...` — probe m2
  reproduced: the identity was interpolated raw into the pinned
  `settlement_authority_decision` JSON (the second leg, a trailing
  backslash, corrupts the record out of JSON entirely).
- Fix: `record_authority_decision` escapes the identity through the shared
  `crate::control::controller::escape_json_text` (the site was pub(crate)'d
  for exactly this; `pin_limitation` already used it).
- GREEN: app `329 passed` (+1: the pinned record parses, the identity
  round-trips as one string member exactly as recorded, no injected member
  — both crafted shapes).

## COR-7 — trait read defaults (settlements, verification_records) fail closed

STARTED COR-7 (2026-08-27, finisher): the two settlement-surface READ
defaults return the typed not-implemented refusal (ReadFailed, mirroring
the write defaults' WriteFailed) instead of silent Ok(empty) — a
conforming-but-sloppy store can no longer under-inform re-entry and the
disagreement checks silently. Fail-first: a write-only stub through the
trait must surface the refusal loudly.

DONE COR-7 (2026-08-27, finisher).

- RED (fail-first, pre-fix run): new ports test
  `state_store::tests::settlement_surface_read_defaults_fail_closed`
  (a write-only stub: required methods only, every defaulted method is the
  default) FAILED with `settlements fails closed, got Ok([])` — probe m1
  reproduced: the read defaults silently under-informed.
- Fix: the `settlements` and `verification_records` trait defaults return
  the typed `StateStoreError::ReadFailed` not-implemented refusal
  (mirroring the write defaults' `WriteFailed` shape); doc comments state
  the fail-closed rule. Scope is exactly the two settlement-surface reads
  the probe named; the other defaulted reads are untouched.
- GREEN: ports `103 passed`, app `329 passed`, sqlite `46 passed`, proofs
  p4/p6/p12/p2 `31 passed` — no composed path relied on the silent
  empties (every conforming store implements both reads).

## COR-1 — re-derivation of the lost fail-first evidence (recovered-honest)

STARTED COR-1-rederive (2026-08-27, finisher): the builder's fail-first run
was lost to the usage-limit interruption (no DONE entry). Re-derivation
method: git HEAD IS the pre-COR-1 state (COR-1 landed uncommitted), so the
probe's C1 attack is rebuilt verbatim against a scratch worktree of HEAD —
raw doc -> ungated record_verification -> admit -> commit must COMPILE and
COMPLETE there (the RED) — and the identical attack against the live tree
must be NON-COMPILING through the landed authority (the closure). Recorded
as recovered evidence; never implied as the original run.

DONE COR-1-rederive (2026-08-27, finisher; recovered-honest — this is a
RE-DERIVATION, not the lost original run).

Method: git HEAD (62d1a5f) IS the pre-COR-1 tree — COR-1 landed uncommitted
— so a scratch detached worktree of HEAD rebuilt the probe's C1 attack
verbatim (hand-built PASS doc via public `VerificationRecordDoc::new` ->
raw `record_verification(&doc)` -> `SettlementCommitAuthority::admit` ->
`commit_settlement`), in a standalone probe crate depending on
gauntlet-ports only. Scratch removed after; the repo tree untouched.

- RED (re-derived at HEAD): `cargo test` in the probe →
  `the_c1_attack_bypasses_every_16_1_gate ... ok. 1 passed` — the attack
  COMPILED and COMPLETED pre-COR-1: a settlement row landed with no §16.1
  gate ever consulted (head's `record_verification` took the raw
  `&VerificationRecordDoc`, confirmed by `git show HEAD:.../state_store.rs`).
- Closure leg 1 (live tree, raw-doc fake): `error[E0053]: method
  'record_verification' has an incompatible type for trait ... expected
  'AdmittedVerificationRecord', found 'VerificationRecordDoc'` — an
  attacker cannot even IMPLEMENT the ungated write.
- Closure leg 2 (live tree, trait signature adopted): `error[E0308]:
  mismatched types ... expected '&AdmittedVerificationRecord', found
  '&VerificationRecordDoc'` at the call site — and `AdmittedVerificationRecord`
  has no constructor outside the minting module (pinned in-crate by
  `the_minted_verification_record_has_no_public_constructor`), so the
  runtime refusal legs (undurable run / stale iteration, the sibling
  COR-1 tests) are reachable only through the authority's governed reads.

The compiled out-of-crate probe the Task-12/13 structural tests deferred
to "the confirmation round's job" is hereby that confirmation.

## Closure (finisher, 2026-08-27)

Result contract written: `.agents/reports/task13/correction-round1.md`.
Gates re-run personally after the last code change (clippy-driven boxing of
the COR-4 variant + fmt + the COR-7 test's literal idiom):

- `just ruler` → PASS
- `just check-package gauntlet-ports` / `just test-package gauntlet-ports`
  → `103 passed`
- `just check-package gauntlet-app` / `just test-package gauntlet-app`
  → `329 passed`
- proofs p4/p6/p12/p2 → `31 passed`
- `just lint` → exit 0; `just boundaries` → `215 passed`
- extra: sqlite `46 passed`, export_stack `1 passed`, tamper_evidence
  `9 passed`

COR-2..COR-7 DONE; COR-1 fail-first evidence re-derived (recovered-honest).
Correction round 1 complete.

## COR-R2 / COR2-1(a) — admission-level one-settlement-per-(run,unit) — STARTED 2026-08-26

F-1's admission half: `SettlementCommitAuthority::admit` refuses a
DIFFERENT-key commit when ANY settlement row exists for the (run, unit);
same-key stays idempotent (AlreadyCommitted / AlreadyCommittedDivergence
shapes unchanged). RED = the falsifier's sequence at the admission
surface (settled AK-900 re-attempted via a fresh honest PASS under a
fresh verification ref) plus the app-level settle re-entry.

## COR-R2 EXECUTION NOTE (orchestrator, 2026-08-27 15:1x EDT)

Three consecutive COR-R2 dispatches cancelled/killed externally (the last
mid-admission). Per the operator's standing heal+continue instruction and
the ESC-002 lineage, the orchestrator executes COR2-1..6 personally with
fail-first discipline. Build lock held; no other compiler.

## COR-R2 COMPLETION (orchestrator-run, 2026-08-27 ~17:xx EDT — DONE entries)

- DONE COR2-1(a-test) `a_fresh_verification_key_cannot_re_settle_a_settled_unit`
  (ports): the F-1 admission half re-derived fail-first — guard
  neutralized (`if false &&`) → RED (`Err(UnitAlreadySettled)` expected,
  admission succeeded instead) → restored → GREEN. Fixture:
  `pass_record_with_ref` via the refactored `verification_doc` helper.
- DONE COR2-1(b) completion-path guard: the per-completion loop processes
  only the flight's OWN unit; a declared-other attempted entry becomes a
  pinned `attempted_unit_mismatch` governance record (evidence.put+pin),
  never verified/settled. Test
  `a_declared_attempted_other_unit_never_settles_and_is_recorded`
  (app): fixture lever `Behavior::CompleteDeclaringOther` (declares
  AK-9999, absent from the ruler) wired through BOTH `poll` and `collect`
  (collect is the path the loop reads — the first draft patched poll only
  and the guard never fired; fixed). Fail-first: guard neutralized → RED
  (no pinned record) → restored → GREEN.
- DONE COR2-2 + COR2-3: PREDECESSOR-LANDED (the cancelled dispatch's
  unrecorded window — arm at settle/mod.rs:1048 refusing
  Operator{identity:None} for every authority; run filter at :1702);
  verified green in the full battery (app 330/330 includes their tests).
- DONE COR2-4 `append_record` gate, rescoped by KIND after export_stack's
  benign Observation call broke under pub(crate): the public fn REFUSES
  `RecordKind::VerificationRecord` for every caller (typed, naming the
  authority); the trait's `record_verification` reroutes through the
  private `append_record_raw`. Fail-first: the out-of-crate forgery is
  now a runtime typed refusal (compile-level for pub(crate) was tried
  first; the kind gate is the honest complete closure).
- DONE COR2-5 replay divergence: `insert_settlement` compares the stored
  row's evidence+digest on an ignored keyed insert → typed `Corrupt`
  naming both sides; same-facts duplicates stay idempotent (offset
  deliberately NOT compared — a second same-key event at a new offset
  with identical facts is legal §16.3 idempotency; the first draft
  compared offsets and broke `commit_settlement_is_the_spec_sequence…`,
  fixed). Tests: `a_same_key_divergent_event_is_loud_on_replay` +
  `a_same_facts_duplicate_event_stays_the_idempotent_no_op` (own fixture
  — a divergent event fails every replay over it). Shared-suite case
  `settlement_replay_is_keyed_idempotent_and_anchored` updated to pin the
  REFUSAL (intended, evidenced behavior change — the old silent
  never-displaces expectation WAS F-6) and the green stub conforms
  (DurableStore returns the typed Corrupt on divergent replay).
- DONE COR2-6: precondition.rs module doc trued (settlement landed; the
  layer's split unchanged).

FINAL BATTERY (orchestrator-run, verbatim counts): ports **105/105**,
app **330/330**, sqlite **48/48**, proofs p7/p11/g10 **12/12**, standing
battery p4/p6/p12/p2/p3 **33/33**, `just lint` **0**, boundaries
**215/215**, deny **ok**, ruler **PASS**.

## COR3 — gate `RunEventKind::SettlementCommitted` on both public append surfaces (2026-08-27)

STARTED COR3 (2026-08-27, correction builder, brief `correction-brief-r3.md` /
falsifier F-1 / adjudication COR3-only):

- Surfaces: `StateStore::append_event` (ports trait doc + sqlite impl) and the
  `EventLog` port's `append` (`SqliteEventLog`) — both refuse the kind with a
  TYPED error naming the earned path (`commit_settlement` consuming
  `AdmittedSettlement`); `commit_settlement` keeps the ungated internal
  `append_to_log` (the COR2-4/`apply_settlement_row` idiom). No
  replay-semantics change.
- Caller audit (pre-implementation, recorded in full in the result contract):
  production append_event callers append only ordinary kinds
  (controller: InvocationTerminal/CheckpointWritten; compare/recording.rs:
  InvocationAdmitted; adapter record_evidence_production: EvidenceProduced).
  Legitimate two-step settlement producers besides commit_settlement:
  crash_helper's `settle` script (real-abort crash injection between §16.3
  steps — to move to an EARNED route) and tests (adapter in-crate unit tests
  → internal path; tamper_evidence.rs:443 → the existing `earn_settlement`
  helper; shared contract suites → reshape + refusal cases; app-side fakes
  model post-crash durable states and stay, with comments).
- Fail-first: two-leg RED as an in-repo integration test over the real
  sqlite surfaces (new tests/settlement_append_gate.rs), mirroring the
  falsifier's executed A/B sequence; observed RED before any implementation.

RED COR3 (2026-08-27, fail-first observed, `cargo nextest run -p
gauntlet-adapter-state-sqlite --test settlement_append_gate` → `Summary
2 tests run: 0 passed, 2 failed`):

- Leg A `the_state_store_append_surface_refuses_the_settlement_kind`
  panicked at settlement_append_gate.rs:148: "the forged settlement event
  must be refused by the generic append surface, but it appended durably
  at offset 1 — the 18.3 replay will mint a settlement row with zero
  verification records (F-1)" — the falsifier's [A] leg reproduced
  in-repo.
- Leg B `the_event_log_append_surface_refuses_the_settlement_kind`
  panicked at :195 with the same reason through the `EventLog` port — the
  falsifier's [B] leg reproduced.
- Expected reason confirmed on both: the append SUCCEEDS today (Ok(offset
  1)); no unrelated failure. Implementation may begin.

## RECONCILE-CHECKPOINTS RUN (orchestrator, 2026-08-27, pre-confirmation)

`scripts/reconcile-checkpoints.sh 13` flagged 3 files modified after this
checkpoint file's last write:

- `.agents/CURRENT_STATUS.md.bak`, `.agents/CURRENT_STATUS.yml`,
  `.agents/CURRENT_STATUS.yml.bak`

Recovered-and-verified finding (NOT implementation work): all three are the
OPERATOR's status-record format migration (CURRENT_STATUS.md → CURRENT_STATUS.yml
plus .bak snapshots; the .md deletion is in git status as a worktree delete).
Zero task13 implementation surface: no crate, test, proof, or report file is
newer than the last checkpoint write. The task13 tree state matches the record
(B1 committed at 62d1a5f; B2/B3/B4/COR-1..7/COR2-1..6 uncommitted working tree,
all CLOSED per IN-FLIGHT). CURRENT_STATUS.yml is on the never-stage list and
stays uncommitted by design. No re-dispatch, no redo.

## COR3 — RECOVERED-AND-VERIFIED (orchestrator, 2026-08-28 08:4x EDT — 20th interruption instance)

The COR3 builder dispatch (2026-08-27 ~21:25 EDT, builder charter) died to
the sub-agent usage limit (window resets 2026-08-28 12:44:33 EDT) at
~21:39 EDT, leaving its work UNRECORDED (no STARTED entry, no result
contract). Tree-vs-record reconciliation (tree outranks record):

LANDED (mtimes 21:29–21:39 EDT, verified by the orchestrator by reading):
- `crates/adapters/gauntlet-adapter-state-sqlite/tests/settlement_append_gate.rs`
  (NEW, 225 lines): both fail-first legs — Leg A the StateStore
  `append_event` surface, Leg B the `SqliteEventLog` append over the same
  DB — with zero-durable-write assertions (export, settlements,
  verification_records, reopen) and honest RED-stage match-arm docs.
- `crates/gauntlet-ports/src/state_store.rs`: typed
  `StateStoreError::SettlementEventNotEarned { run_id, unit_id }` + trait
  docs on `append_event`.
- `crates/gauntlet-ports/src/event_log.rs`: typed
  `EventLogError::SettlementEventNotEarned { run_id, unit_id }` + trait
  docs.
- `crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs`:
  `append_event` impl gate; new earned pub half-step
  `append_settlement_event` (documented for the crash instrument); internal
  test-only `append_settlement_event_unchecked` for crash-state fixtures.
- `crates/adapters/gauntlet-adapter-state-sqlite/src/event_log.rs`:
  `SqliteEventLog::append` gate.

NOT LANDED (this reconciliation's remaining work): compile verification;
fail-first evidence (none recorded — to be re-derived honestly via the
established neuter-and-cmp tooth idiom); shared contract-suite cases +
conforming stub updates; the in-repo caller audit (append_event /
EventLog::append / crash_helper); the full gate battery;
`correction-round3.md`. The orchestrator completes these personally per the
operator's standing heal+continue instruction (IN-FLIGHT B2/B4 precedent)
and the operator's explicit 2026-08-28 "continue" directive (sub-agent
dispatch is usage-limited until 12:44 EDT; the compiler lane is exclusively
the orchestrator's either way).

## COR3 — COMPLETION (orchestrator-run, 2026-08-28 09:0x EDT, per the standing heal+continue instruction)

- DONE gates+typed errors (builder, 21:32–21:39): both public append
  surfaces refuse the settlement kind (StateStoreError/EventLogError
  SettlementEventNotEarned); earned half-step append_settlement_event +
  crash_helper conversion + internal unchecked fixture path — verified by
  the orchestrator by reading (see RECOVERED-AND-VERIFIED above).
- DONE fail-first (orchestrator, recovered-honest): gates neutered →
  settlement_append_gate 0/2 (both legs panic on the forged append
  succeeding at offset 1 — the falsifier's F-1 reproduced); cp+cmp
  byte-identical restore → 2/2 GREEN.
- DONE caller audit (orchestrator): production has zero settlement-kind
  appends through generic surfaces; crash_helper earned; 2 in-crate
  fixtures → unchecked; tamper test → earned commit idiom; dead helper
  removed.
- DONE contract suites (orchestrator): StateStore case renamed+reshaped
  (settlement_events_are_earned_and_replay_never_disturbs_rows — earned
  commits stand, forged append refused typed + zero durable change, replay
  idempotent at anchors 1/2); EventLog case settlement_kind_appends_are_refused;
  seeding de-settlementized; DurableStore/GrowingLog stubs conform (gate +
  private ungated earned append). Teeth: SettlementLax still caught.
  Coverage delegation note + ports-revision trigger recorded in the case
  doc and correction-round3.md.
- DONE gates battery (orchestrator, personal): ruler PASS ×2, ports
  105/105, app 330/330, sqlite 50/50, proofs 12/12, standing 33/33, lint
  0, fmt-check clean, boundaries 215/215 (one comment literal fix — vendor
  name), deny ok, prereg-check 13 PASS.
- Result contract: correction-round3.md written.

**COR3 CLOSED.** Next: fresh blind confirmation round 2 (sub-agent window
resets 12:44 EDT), then closure.
