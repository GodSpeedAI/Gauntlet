# Task 13 — Final confirmation, round 2, Role A (spec critic)

Fresh blind confirmation round. Written by the spec critic under the critic
charter (`.agents/agents/critic.agent.md`), per the round-2 brief delivered by
the orchestrator (the brief on disk names the round-1 file; the round-2
instruction names this file and makes the coordinator's constraints
controlling: static-only, one report file).

**Blindness declaration.** I read, in full or in targeted part, only: the
charter, the brief, `.agents/specs/Core-spec.md` (§3, §9, §10.2, §11, §14.3,
§16, §17.5, §18, §20.2, §21, §23), `.wayfinder/answer-key.md` (the AK units
under review), `.agents/plans/gauntlet-impl.md` (Task 13 section only),
`AGENTS.md` (supplied), and the artifact source listed below. I opened
**nothing** under `.agents/reports/task13/**` except the brief, and nothing
under `.agents/reports/taskN/**`, `CURRENT_STATUS*`, `lessons/**`,
`research/**`, `reference-candidates.md`, or `escalations.md`. No accidental
exposure occurred. Source comments narrating past corrections were read as
part of the authorized artifact; every verdict below is my own construction
from the code as it stands now.

**Method and static bound.** The coordinator's constraint made this round
STATIC-ONLY: no `cargo`, no `nextest`, no `just` target was run (a sibling
falsifier holds the single build lock; the host is memory-constrained). My
evidence base is reading source, spec, answer key, plan, and read-only
structural search (`rg`/`sed`-free targeted reads). Where a claim can only be
settled by executing a test, I record it as an open execution question for the
orchestrator. Nothing below relies on a green test run.

**Artifact examined (principally).** `crates/gauntlet-app/src/settle/mod.rs`
(138 KB), `crates/gauntlet-app/src/control/controller.rs` (326 KB, targeted),
`crates/gauntlet-app/src/verify/{precondition,authority,ladder}.rs`,
`crates/gauntlet-ports/src/{state_store,producer_authority,verification_record}.rs`,
`crates/gauntlet-ports/src/contract/{state_store,stubs}.rs` (outline +
settlement cases), `crates/adapters/gauntlet-adapter-state-sqlite/src/{state_store,event_log,encode}.rs`
and `src/bin/crash_helper.rs`, `crates/gauntlet-arch-tests/src/{lib,single_writer}.rs`,
`crates/gauntlet-cli/tests/{p7_recovery,p11_critic_is_not_verifier,g10_builder_writes_answer_key}.rs`,
`crates/gauntlet-app/src/reference/provider.rs`, `crates/gauntlet-cli/src/main.rs`,
`justfile`.

---

## 1. Section-by-section verification

### §16.1 — the five unit-level preconditions (AK-040)

`SettlementService::settle` (`crates/gauntlet-app/src/settle/mod.rs:631-731`)
enforces, in order, before any durable write of its own:

1. **PASS outcome** — twice, independently: the settling claim's own outcome
   at service entry (`settle/mod.rs:646-656`, refuses `VerificationNotPass`
   naming the exact outcome so INCONCLUSIVE/FAIL stay distinct), and again
   against **durable** state by `SettlementCommitAuthority::admit`
   (`gauntlet-ports/src/producer_authority.rs:676-824`): a PASS record for the
   commit's (run, unit) whose evidence ref AND verification ref equal the
   commit's (the COR-3 mooring — the §16.3 key's third component is the
   durable record's own ref, never caller-invented). The run-scoping of the
   records is re-filtered by the record's OWN run id, not the store's
   filtering (COR-5).
2. **Evidence resolves and is pinned** — re-derived INSIDE the service over
   the claim itself through `settlement_precondition`
   (`verify/precondition.rs:137-184`): outcome PASS, record run = settling
   run, `get()` resolves, `is_pinned(settling_run, …)` true; pin-state
   unanswerable fails closed (`PinMembershipUnknown`, I8). The request type
   (`SettleRequest`, `settle/mod.rs:232-236`) carries **no readiness
   parameter**, and `SettlementReadiness` has no public constructor — a token
   minted over one claim has no parameter to travel to another's settlement.
   The consulted store is the claim's own chain store (`chain_evidence`,
   crate-private), so a stooge store cannot be presented.
3. **Authority chain** — `check_authority` (`settle/mod.rs:1007-1071`) over
   the **frozen** unit's `authority` (loaded by the service from the durable
   run's `ReferenceVersion` through the digest-verifying
   `FrozenReferenceProjection`, `settle/mod.rs:963-1002` — never a caller
   snapshot). Table: critic closable by any actor (§16.2 "minimum bar");
   lead_agent requires orchestrator + non-blank recorded rationale (operator
   also); human requires operator + recorded identity; an orchestrator can
   never close human work; automatic closure of lead_agent/human REFUSES and
   ROUTES to the escalation surface with pinned evidence
   (`escalate_authority_gap`, §11.5 row 6) before returning the typed error.
4. **No blocking residual** — tracked half
   (`check_blocking_residuals:1118-1123`) plus a durable half: the checkpoint
   projection holding `Blocked` while the tracked view is clear is the typed
   `DurableBlockDiverged` refusal (durable wins). The durable residual-ROW
   read is an explicitly recorded ports gap, documented in the module doc —
   not silently papered over.
5. **Digest unchanged** — `settle_time_verify` (`settle/mod.rs:1145-1183`)
   calls the reference PROVIDER's `verify_unchanged` over the durable run's
   version; see §9.4 below for the no-tautology analysis.

The commit's inputs (`readiness.evidence()`, `readiness.verification()`,
`run.reference_version().digest()`) are all service-derived from durable or
chain-minted facts. **Verdict: §16.1 conforms on the composed path.**

### §16.3 — atomic commit sequence, keyed idempotent replay (AK-041, AK-042)

- **Order.** The sqlite adapter's `commit_settlement`
  (`gauntlet-adapter-state-sqlite/src/state_store.rs:1937-1980`) appends the
  `SettlementCommitted` event first (`append_to_log`, own INSERT on a
  `synchronous=FULL` connection — `event_log.rs:9-12, 64-74, 78-103` — so
  step 1 is genuinely durable before step 2 runs), then applies the keyed row.
  A step-2 failure returns an error that names the durable offset and states
  that re-running the unit would append a duplicate settlement event. The
  service returns the in-memory advance only after both steps
  (`settle/mod.rs:714-730`), and the controller updates its projection only
  in the `Ok` arm (`controller.rs:2523-2529`) — the advance is observably last
  (the P7 `commit_order` proof brackets exactly this with a global write-order
  stamp over the real store).
- **Cross-validation.** `insert_settlement` → `verify_settlement_event_at`
  (`state_store.rs:1091-1310`): the row's claimed offset must name a real
  `SettlementCommitted` event of the same run with **every field and the
  stored timestamp agreeing** (§5.3.6); the row's `settled_at` is the event's
  own stored timestamp, not the commit's constructed one. A row therefore
  cannot precede or misattribute its event.
- **Keyed idempotence, loud divergence.** Key `(run_id, unit_id,
  verification_ref)`: same-key same-facts re-entry is the idempotent
  `AlreadyCommitted` success returning the existing offset
  (`settle/mod.rs:696-704`); same-key divergent evidence is the typed
  `AlreadyCommittedDivergence` integrity refusal (admission) AND the store's
  `Corrupt` refusal on divergent facts (COR2-5); a **different** key over an
  already-settled unit is `UnitAlreadySettled` (COR2-1) — one settlement per
  unit per run however honest the fresh PASS.
- **Replay.** `complete_pending_settlements` (adapter `state_store.rs:2004-2045`)
  rebuilds the commit **from the durable event itself** (event's own facts and
  timestamp), keyed-inserts idempotently, anchored at the checkpoint offset.
  The spec's recovery semantics (crash before 1 → retry as unverified; between
  1 and 2 → complete on replay; after 2 → no-op) are exactly what
  `resume_run` composes (§18.3 below) and what P7's crash legs exercise.

**Verdict: §16.3 conforms.** The single deviation I found is in the resume
order relative to §18.3's numbered steps — recorded as residual FC2-A1 below
(minor).

### §16.4 — single-writer discipline (AK-043)

- **No production writer of the ruler exists in the core.** The mechanical
  rule (`gauntlet-arch-tests/src/single_writer.rs`) pins: (A) the exact
  inventory of core modules that name the answer-key surface; (B) no
  unguarded write-API token in a surface module; (C) EVERY production
  write-API token in every core crate, each with a containment-guard needle
  that must remain present in the same file (the controller's two builder-side
  writes are guarded by `contained_locator_path`; doctor's writes by their own
  functions). The scan blanks `#[cfg(test)]` modules with brace balancing and
  **fails closed on anything it cannot classify**; it carries an anti-vacuity
  guard (a rule that matches nothing is itself a violation) and an honestly
  declared lexical threat model (recorded escapes: `format!`-constructed
  surface mentions, aliased imports, adapter-side indirection — the last
  delegated to §8.5/§17.5/g10). Run through `just boundaries` (in `ci`).
- **Runtime never writes Unit status to the key.** The loop's unit status
  lives in the durable store (settlements rows + checkpoint projection); the
  answer key is re-read strictly (`strict_units`) and never written.
- **Placement.** In every composed proof the key lives in its own `ruler/`
  directory outside the workspace root (§17.5.5), and the loop's only
  builder-side write surface refuses locators that escape the canonical
  workspace root (`ControlError::EvidenceLocatorEscapes`, pinned by the g10
  prevention leg). There is **no literal read-only mount** — §16.4 is a
  disjunction ("lives outside builder-writable workspaces **or** is mounted
  read-only"); the landed tree satisfies the first leg plus the write
  boundary plus detection. Recorded as FC2-A5 (minor, plan-wording vs
  landed mechanism; real mounts are Task 14's sandbox scope).

### §18.2–§18.4 — checkpoints, resume (AK-053 support, P7)

- **§18.2.** `controller.rs:3047-3089` writes the checkpoint at the TOP of
  every iteration and at close-out, carrying exactly the required fields: run
  id, reference digest, iteration number, unit-state projection, budget
  spent, in-flight invocation ids **with their result-contract paths**, and
  the event-log offset as a store-returned water-mark threaded from every
  append (C8.1). Resume reads only these durable facts — nothing is
  reconstructed from conversational memory (I11).
- **§18.3.** `resume_run` (`controller.rs:842-979`): re-freeze-verifies the
  reference FIRST (mutated → terminal `aborted` with both digests; unavailable
  → typed error, not a silent pass); loads the checkpoint; calls
  `SettlementService::complete_pending_after_checkpoint` (anchored settlement
  replay + evidence-production orphan completion re-minted from durable facts
  only, with unrecoverable orphans pinned as recorded limitations — never
  fabricated); reconciles the projection so settlements strictly beyond the
  checkpoint anchor are Settled (the §16.3 crash-window shape) while
  at/below-anchor state defers to the later projection word; restores budget
  through the domain's own gate; reconstructs per-unit attempts from durable
  producer writes; re-probes in-flight invocations (valid result contract →
  collected; else terminal `Failed` event → re-planned). The P7 crash legs
  prove exactly-once completion and zero re-dispatch on this path.
- **§18.4.** The assume-failure set is dispositioned (§21.3 below; verifier
  breakage and absence are typed refusals in the chain, P11 legs 1–2).

Deviations from §18.3's full numbered procedure are recorded as FC2-A1
(step order 1↔2) and FC2-A4 (steps 4–5 absent). Both are outside the
Task-13 essential claim ("completes half-applied settlements exactly once and
never re-runs settled work"), which holds.

### §9.4 — freezing, digest re-verification (AK-024 settle-time half)

`FileReferenceProvider::verify_unchanged` (`reference/provider.rs:292-326`)
**re-reads the answer-key file from disk and re-digests the bytes**, comparing
against the version's pinned digest. It is the provider's own file-derived
answer; the settle service passes it the durable run's version
(`settle/mod.rs:1151-1154`) — never a digest the controller froze in the same
process. **No tautology.** A mutation yields `ReferenceMutated{expected,
actual}`; the service pins BOTH digests as forensic evidence before refusing
with the typed abort error the controller maps to terminal
`aborted{reference_mutated}`. Unavailability refuses the settlement without
aborting (I8 + §21.3). The iteration-boundary half is also composed:
`boundary_verify` runs at the top of every iteration and again in
`decide_progress` before the terminal `Settled` is declared
(`controller.rs:1088, 3025`), and `resume_run` verifies before trusting
anything. AK-024's two legs ("every iteration boundary" + "immediately before
settlement") are both load-bearing on the production path.

### §21.3 — degradation rules

Only the integrity violation aborts: `ReferenceMutated` → terminal abort;
`ReferenceUnverifiable`/boundary `Unavailable` → pinned gap, settlement
refused, loop alive; a §16.1 gate refusal on one unit is pinned forensic
evidence with the unit left open (the loop continues, `controller.rs:2562-2574`);
environment/store failures propagate typed, never swallowed. `record_selection`'s
answer-key read failure pins the gap and continues (row 1). This matches the
spec table.

### §10.2 / §14.3 / §11.5 — admission and disposition context

The verification chain that mints the settling claim
(`GovernedVerification::verify_claim`, `verify/authority.rs:413-…`) derives
the requirement from the **frozen** unit, the evidence candidates from
**durable production records at the frozen locator**, validates §14.3 binding
(recency anchored on the governed iteration; content digests store- and
locator-checked), dispatches the §14.2 ladder the governed verification was
OPENED with, and rejects instruments that contradict their dispatch. The
caller's request fields are claim pointers only — the P12 discipline. The
`VerifiedClaim` has no constructor outside the module. Authority escalation
for lead_agent/human automatic closure routes per §11.5 row 6 with pinned
evidence and a ticket.

### §20.2 — operator-action surfaces (AK-062 service-level half)

`settle_operator_action` (`settle/mod.rs:742-793`) hits the same durable-PASS
gate first — a run lacking a PASS record for the unit is refused with the
IDENTICAL `VerificationNotPass` variant the automatic path's admission
produces — and even with PASS evidence present it still cannot settle
(`OperatorActionRequiresEarnedReadiness`): settlement requires the
chain-minted readiness this request shape cannot carry. The service-level
half of AK-062 is therefore satisfied **structurally** (no bypass exists at
this layer because the operator path cannot commit at all); the positive
operator surface is Task 21 by plan. The controller's production path calls
`settle`/`complete_pending_after_checkpoint` only — its test fixtures aside
(all `#[cfg(test)]`, verified at `controller.rs:3498+`).

---

## 2. Durable-substrate audit — who can write settlement facts

Every public surface that writes settlement facts (events, rows,
verification records), and its gate:

| Surface | Gate | Assessment |
|---|---|---|
| `StateStore::commit_settlement` (trait) | accepts only `AdmittedSettlement`; no constructor outside `producer_authority` | Earned. |
| `SettlementCommitAuthority::admit` | durable PASS record for (run, unit) naming the commit's evidence AND verification refs; same-key idempotence; same-key divergence; one-per-unit | Earned and cross-checked. Does NOT validate `commit.reference_digest`/`at` against the durable run — FC2-A2 (minor). |
| `StateStore::append_event` (trait) | refuses `RunEventKind::SettlementCommitted` with typed `SettlementEventNotEarned` (COR3) | Earned-only kind. |
| `EventLog::append` (adapter) | same refusal (COR3 mirrored) | Earned-only kind. |
| `SqliteStateStore::append_settlement_event` (inherent, pub) | consumes `&AdmittedSettlement` (authority-minted); exists for the crash instrument's earned half-step | Earned; benign (an event-without-row is completed by the next anchored replay). |
| `SqliteStateStore::apply_settlement_row` (inherent) | `#[cfg(test)]` — no production route | Test-gated. |
| `StateStore::record_verification` (trait) | accepts only `AdmittedVerificationRecord`, minted by `VerificationRecordAuthority::record` (run durable + iteration = durable anchor) | Content is the caller's at this layer by documented design (the chain's §14 checks are the earning). Authority constructor is public → FC2-A3 (minor, documented boundary; in-process only). |
| `SqliteStateStore::append_record` (inherent, pub) | refuses `RecordKind::VerificationRecord` for every caller (COR2-4); no `Settlement` record kind exists (settlements live in their own gated table + log) | Earned-only substrate. |
| `complete_pending_settlements` | rebuilds the commit from the durable event's own facts | Recovery-only, idempotent. |

Within the declared threat model (builders/critics are external processes
that never hold `Arc<dyn StateStore>`; §16.4's single writer is the runtime),
this surface set is coherently gated. The two residual observations (FC2-A2,
FC2-A3) are defense-in-depth gaps inside the trusted process, both
documented in the code as deliberate scope lines.

---

## 3. Proof judgments (§23.1 discrimination honesty)

All three proofs compose the REAL production controller
(`RunController::compose` over `ControllerDeps`) with the real
`EmbeddedStateStore` (sqlite), real `FsEvidenceStore`, real
`FileReferenceProvider`, real `DiffComparator`, and the real ports
authorities. The proof-local pieces are the scripted builder runner
(`Isolation::None`, the documented deviation shared by every composed proof
in this tree), the never-admitted critic, the recording escalation surface,
and delegating recorder decorators that forward verbatim and only OBSERVE.
Assertions read durable surfaces (`export_jsonl`, `settlements`,
`verification_records`, checkpoints, on-disk bytes) — not in-memory state of
the system under test.

- **`p7_recovery.rs`** — five legs. `commit_order` brackets each
  `commit_settlement` with before/after probes over the store's own surfaces
  plus checkpoint write-order brackets: no event/row before, exactly one
  after, the row cites the returned offset, the last checkpoint before the
  commit does NOT carry the unit settled, the first after does. The crash
  legs use a REAL child process (`crash_helper`) that earns its admission
  through the genuine ports authorities and `std::process::abort()`s at named
  points; the restart is an honest drop-and-recompose, and the
  between-event-and-row leg arms the restarted world with **zero** scripted
  builder steps — any dispatch would fail the assertion. The anchored-replay
  and crash-after legs pin idempotence. Honest. The one soft spot: the
  crash legs' injected settlement row carries the helper's fixture evidence
  ref/digest — the proof itself flags this and scopes its evidence-survival
  walk to the run's own units. Discriminates the claims it names.
- **`p11_critic_is_not_verifier.rs`** — a PASS control leg plus four
  no-settle legs (broken rung, absent rung, INCONCLUSIVE, FAIL) over one
  composition with only the verdict rung swapped; asserts no event, no row,
  no verification record, projection open, unit dispatched exactly once, and
  the pinned forensic record naming the exact outcome (INCONCLUSIVE never
  spelled FAIL). The PASS control is what makes the no-settle outcomes
  attributable to the verification gap rather than a broken fixture.
  Discriminates: with the settlement gate removed, the FAIL/INCONCLUSIVE
  legs' "no settlements row" assertions fail. Note FC2-A7: the "comparator
  pass" is instantiated as a clean diff comparison (zero residuals) rather
  than a literal `verdict: pass` record — the proof documents the
  position-based reasoning; acceptable, recorded.
- **`g10_builder_writes_answer_key.rs`** — prevention leg: the ruler's
  evidence locator TARGETS the key via a built `../ruler/…` escape (built,
  not spelled); the loop's effect surface refuses it typed at the write
  boundary; key bytes byte-identical after; nothing settled. Detection leg:
  the builder raw-writes the key mid-dispatch through plain filesystem
  access; the run ends terminal `Aborted` with both digests pinned, no
  settlement over the mutated ruler, tampered bytes preserved as evidence.
  Both legs are the composed AK-043 "prevented **or** detected" disjunction.

**Teeth records** (reordering the commit steps / disabling a defense makes
the proof fail) are the orchestrator's to re-run; the proof CODE I read
supports them: the store's cross-validation structurally refuses a row
before its event, and the order/bracket assertions fail under the pre-B2
in-memory-only shape the doc names.

---

## 4. Residuals

All findings are **minor**. No blocking or major residual was found within
the declared bound.

## Residual FC2-A1
- **Dimension:** functional
- **Severity:** minor
- **Locus:** `crates/gauntlet-app/src/control/controller.rs:842-882`
- **Contract Ref:** spec §18.3 steps 1–2 (numbered resume procedure)
- **Defect:** `resume_run` performs step 2 (re-freeze-verify) BEFORE step 1
  (anchored replay completion of half-applied settlements). In the
  intersection "reference mutated AND settlement half-applied" the run aborts
  without completing the durable row. Defensible (abort preserves everything;
  the settlement happened under the pre-mutation ruler and its event carries
  the frozen digest), but it is a strict-ordering divergence from the
  numbered procedure, and the intersection is untested (no P7 leg combines a
  mutation with a crash window).
- **Reproduction sketch:** crash between §16.3 steps 1 and 2 (the P7
  "after-append" shape), then mutate the answer key, then `resume_run`:
  expected per strict §18.3 order — row completed, then abort; actual —
  abort with the row still missing (event durable, row absent).

## Residual FC2-A2
- **Dimension:** security (defense in depth)
- **Severity:** minor
- **Locus:** `crates/gauntlet-ports/src/producer_authority.rs:676-824`;
  demonstrated by `crates/adapters/gauntlet-adapter-state-sqlite/src/bin/crash_helper.rs:146-158`
- **Contract Ref:** §16.3 (the SettlementEvent's digest field), §5.3.6
- **Defect:** `SettlementCommitAuthority::admit` validates the PASS record,
  evidence ref, verification ref, and key uniqueness, but does NOT validate
  `commit.reference_digest` (nor `at`) against the durable run's frozen
  reference. The p7 crash helper in fact commits digest
  `sha256:c4d3e2f1a0b1c2d3` over a run whose recorded frozen digest is
  different, and the admission succeeds — the field is caller-attested inside
  the minted admission. The production path derives it from the durable run
  (`settle/mod.rs:693`), so only a caller that already defeated the
  PASS-record gate can exploit it.
- **Reproduction sketch:** with a durable run of frozen digest D and a
  durable PASS record for (run, unit), `admit(SettlementCommit{ digest: X≠D,
  …})` → `Ok(_)`; the event and row record X, not D.

## Residual FC2-A3
- **Dimension:** architecture
- **Severity:** minor
- **Locus:** `crates/gauntlet-ports/src/producer_authority.rs:426-457`
  (`VerificationRecordAuthority::new` public; `record` accepts any
  well-formed `VerificationRecordDoc`)
- **Contract Ref:** §16.1 conditions 2–5; §16.4
- **Defect:** the durable-PASS substrate's earning is admission-shaped, not
  chain-shaped: any in-process holder of `Arc<dyn StateStore>` can construct
  the (public) authority and record a forged PASS document whose run is
  durable and iteration anchored, then admit+commit a settlement that never
  passed the service's conditions 2–5. This is the code's own documented
  boundary ("Ports cannot reach those; minting here is the store-side half of
  the gate, not the whole of §16.1"), and the doc's "The caller is the
  verified chain itself … never an arbitrary store holder" is aspirational
  rather than structural. Within the threat model (builders/critics never
  hold the store; the runtime is the trusted single writer) this holds.
- **Reproduction sketch:** in-process: `VerificationRecordAuthority::new(state)
  .record(&pass_doc)` → `SettlementCommitAuthority::new(state).admit(commit)`
  → `state.commit_settlement(&admitted)` — settles with no authority-chain,
  residual, or digest consult. (Same route the crash helper legitimately
  uses as an instrument.)

## Residual FC2-A4
- **Dimension:** functional
- **Severity:** minor
- **Locus:** `crates/gauntlet-app/src/control/controller.rs:842-979`
- **Contract Ref:** spec §18.3 steps 4–5
- **Defect:** `resume_run` implements steps 1–3 but not step 4 (sweep
  workspaces for settled/out-of-scope units) nor step 5 (re-run preflight
  §8.5). The doc comment scopes resume to "budget/iteration scope". The
  Task-13 essential claim is unaffected.
- **Reproduction sketch:** read `resume_run` end-to-end; no workspace sweep
  or preflight invocation exists on the path.

## Residual FC2-A5
- **Dimension:** evidence
- **Severity:** minor
- **Locus:** plan Task 13 step 3 vs `crates/gauntlet-cli/tests/g10_builder_writes_answer_key.rs:690-696, 841-900`
- **Contract Ref:** §16.4; plan Task 13 step 3
- **Defect:** the plan's "runtime mounts the answer key read-only into
  workspaces" is not implemented as a mount. §16.4's normative disjunction is
  satisfied by the other leg (the key lives outside every workspace, the
  loop's write boundary refuses escapes, and a raw write is detected and
  aborts the run); real mounts belong to Task 14's sandbox port, and no
  runner in this tree yet reports isolation above `None`.
- **Reproduction sketch:** grep for mount/read-only surface in the runtime —
  none; the g10 prevention/detection legs are the landed mechanism.

## Residual FC2-A6
- **Dimension:** evidence
- **Severity:** minor
- **Locus:** `crates/gauntlet-app/src/settle/mod.rs:707-713`
- **Contract Ref:** §16.1 condition 3 ("recorded rationale"); B.6 mapping
- **Defect:** the `settlement_authority_decision@v1` accompanying record is
  pinned AFTER the durable commit; a crash in that window leaves a committed
  settlement with no authority-decision record, and nothing on resume
  re-mints it. Today's production path closes only `critic`-authority units
  automatically (no rationale leg exercised); the gap becomes live when the
  orchestrator/operator closure routes land.
- **Reproduction sketch:** abort the process between `commit_settlement` and
  `record_authority_decision`; resume — the settlement stands complete, the
  decision record never exists.

## Residual FC2-A7
- **Dimension:** evidence
- **Severity:** minor
- **Locus:** `crates/gauntlet-cli/tests/p11_critic_is_not_verifier.rs:818-831`
- **Contract Ref:** P11 wording ("a comparator returning `verdict: pass`");
  §23.1
- **Defect:** the proof instantiates "comparator pass" as a clean diff
  comparison (zero residuals) rather than a literal comparator verdict
  record, with position-based reasoning documented in the proof itself. The
  PASS control leg and the durable-surface assertions keep the proof
  discriminating, but a literal verdict-record leg would bind the wording
  exactly.
- **Reproduction sketch:** read the leg setup — no `ComparisonRecord` with a
  pass verdict is asserted anywhere; cleanliness is inferred from the
  ladder/settle positions downstream.

---

## 5. Open execution questions for the orchestrator

1. Re-run the Task-13 gate and teeth records (I was static-only):
   `cargo nextest run --test p7_recovery --test p11_critic_is_not_verifier`,
   `cargo nextest run -p gauntlet-app settle::`, `just boundaries`, and the
   commit-order tooth (restore an in-memory-first settle shape; the
   `commit_order` proof must fail).
2. FC2-A1's intersection (mutation + half-applied settlement on resume) has
   no test leg in either direction; whichever reading is ruled canonical
   (complete-then-abort vs abort-with-row-missing) deserves a pinned test.
3. The `Isolation::None` proof-runner deviation is documented and shared;
   when Task 14 lands real containment, the composed proofs should gain a
   leg under a real sandbox backend.

## 6. Search bound and what I did not examine

- **Bound:** static reading of the files named above, on the current working
  tree, on 2026-08-28. No builds, tests, mutation runs, or dynamic probes
  (coordinator constraint). Structural search by exact-token grep; I did not
  attempt masked-spelling fuzzing of the lexical arch rule (the rule's own
  documented battery covers the shapes I would have tried; verifying its
  claims by execution was out of bound).
- **Not examined:** git history; `.agents/` anything except the brief;
  crates/modules outside the Task-13 surface except where the settle path
  crosses them (compare/, observation/, diagnosis/, select/, scheduler/ were
  read only at the specific call sites the settle path touches); the
  remaining integration proofs (p1–p6, p12, export, g6); adapter crates other
  than state-sqlite and those the proofs wire (evidence-fs, observe-command,
  compare-diff/model, meter-memory, agent-mock) which I accepted as
  proof-wiring, not review targets; `encode.rs` beyond its call sites; the
  domain crate's `Unit::settle` transition body; runtime behavior of any
  kind.
- **Claim discipline:** "no bypass found" above means none found within this
  declared static bound, against the in-process/external-role threat model
  the code itself declares — not absence in any absolute sense.

---

## 7. Verdict

Within my declared bound, the Task-13 surface holds: §16.1's five
preconditions are enforced at the service entry against durable/frozen state
on the composed controller path; §16.3's sequence is durable-first with
genuine fsync, keyed idempotent replay, and loud divergence; §16.4 is
mechanically pinned with no production writer of the ruler; §18.3's
essential resume claim (exactly-once completion, never re-run settled work)
is composed and proven; AK-024's settle-time consult reads the provider's
file-derived digest (no tautology); AK-031/I3 hold (comparator verdicts
never mint verification records); AK-062's service-level half holds
structurally; AK-053's supporting checkpoint is field-complete. The three
proofs are composed on the production path and discriminate their claims.
Everything I found is minor.

**VERDICT: CONFIRM**
