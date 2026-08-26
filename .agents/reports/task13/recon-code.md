# Task 13 recon — code substrate map (read-only agent, 2026-08-25)

## 0. Workspace

Members: gauntlet-{domain,ports,app,cli}, adapters
{agent-mock, agent-prime*, agent-cli*, state-sqlite, evidence-fs, meter-memory,
observe-{filesystem,git,build,test,command}, compare-{diff,model},
verify-{command-exit-code,test-report,file-digest,schema-check}},
gauntlet-arch-tests. Layering domain→none, ports→domain, app→domain+ports,
cli→composition root. `unsafe_code = "forbid"`. Externals: toml (arch-tests),
sha2 (app). (* prime/cli adapters may be stubs from Task 2.)

## 1. Domain

- `src/settlement.rs`: `EventLogOffset{offset: u64}`; `SettlementRecord{run_id,
  unit_id, evidence, verification, event_log_offset}` + `new` + getters;
  append-only, no mutation. Doc: "The atomic commit sequence around it is plan
  Task 13." **Missing**: `reference_digest`, `settled_at`, authority_decision
  (B.6 fields); no §16-condition aggregate.
- `src/verification.rs`: `VerificationRecord{id: VerificationRef, run_id,
  unit_id, evidence, outcome, instrument}`; doc: "a PASS record from a foreign
  run is stale evidence (§16.1)".
- `src/unit.rs` `Unit::settle(&mut self, run, evidence, verification)` —
  checks unit_id, run_id, `Outcome::Pass` (FAIL and INCONCLUSIVE refuse,
  `SettledWithoutPassVerification`), double-settle refused
  (`StatusTransitionInvalid`; "the durable layer owns replay idempotency (plan
  Task 13)"); sets `verification: Option<VerificationRef>`. `transition`
  refuses bare moves into `Settled`.
- `src/values.rs`: `UnitStatus := Open|InProgress|Settled|Blocked|OutOfScope`;
  `Outcome := Pass|Fail|Inconclusive` (`as_pass() -> Option<bool>`, no bool
  From); newtypes RunId/UnitId/EvidenceRef/VerificationRef.
- `src/run.rs`: `Run` carries frozen `ReferenceVersion` (`.digest()`).
- `tests/settle_public_api.rs` — public-API freeze test.

## 2. State store (state-sqlite)

Migrations 0001–0008 (8 pinned; 0008 = correction_selection_records).
`settlements` table (0002):

```sql
CREATE TABLE settlements (
    run_id           TEXT    NOT NULL,
    unit_id          TEXT    NOT NULL,
    verification_ref TEXT    NOT NULL,
    evidence_ref     TEXT    NOT NULL,
    reference_digest TEXT    NOT NULL,
    event_log_offset INTEGER NOT NULL,
    settled_at       INTEGER NOT NULL,
    PRIMARY KEY (run_id, unit_id, verification_ref)
);
```

- `commit_settlement(commit: SettlementCommit, deadline) -> EventLogOffset`
  (state_store.rs:768): step 1 `append_to_log` (fsync, synchronous=FULL) of
  `RunEventKind::SettlementCommitted{unit_id, evidence, verification,
  reference_digest}`; step 2 `apply_settlement_row_with` → error names the
  durable offset + `complete_pending_settlements` + "re-running the unit is
  wrong". `SettlementCommit{run_id, unit_id, evidence, verification,
  reference_digest, at: EpochSeconds}` — **caller-supplied `at` lands in
  `settled_at` (R1)**.
- `apply_settlement_row` (:817) — step 2 alone, for replay.
- `insert_settlement` (:1126) — `verify_settlement_event_at` then
  `INSERT OR IGNORE`.
- `verify_settlement_event_at` (:1164) — offset cross-validation: event at
  offset must exist, decode, be SettlementCommitted, and agree on
  run/unit/evidence/verification/reference_digest. **Decodes using
  `commit.at`, not the stored `at` column** (event timestamp not compared).
- `complete_pending_settlements(run_id)` (:845) — §18.3 step-1 replay:
  `read_entries(offset 0)` — **replays from 0, not the checkpoint offset** —
  filters run+SettlementCommitted, `insert_settlement` per entry; second call
  adds 0.
- `settlements(run_id)` (:889→:1273): `SELECT … ORDER BY unit_id,
  verification_ref`; rebuilds domain records with
  `EventLogOffset::new(u64::try_from(offset).unwrap_or(0))` — **R3 coercion to
  0**; undecodable refs → `Corrupt` (loud).
- Checkpoints: plain INSERT; `resume` selects latest by (iteration_no, seq);
  `event_log_offset` decode strict → `Corrupt` (Task-12 N3). `load_run` has a
  checkpoint-only projection fallback.
- `record_run`: identity re-read + full-column diff on constraint violation.
- Errors: `StateStoreError := WriteFailed|ReadFailed|Corrupt|Unavailable`;
  EventLog `:= AppendFailed|ReadFailed|Damaged` (Damaged: non-decoding,
  engine-corrupt, negative ts, offset holes).
- Exports: `EmbeddedStateStore`/`EmbeddedEventLog` capability re-exports;
  export pins settlements segment by (unit, verification).

## 3. Event log + evidence

- `RunEventKind` (ports state_store.rs:68): `SettlementCommitted{unit_id,
  evidence, verification, reference_digest}`, `InvocationAdmitted`,
  `InvocationTerminal`, `CheckpointWritten{iteration_no, event_log_offset}`,
  `EvidenceProduced{evidence, locator}`. Wire token `settlement_committed`.
- Event log adapter: journal_mode=DELETE, **synchronous=FULL** (fsync per
  append), busy_timeout 5s; INSERT/SELECT only.
- Evidence-fs: content-addressed write-once, kind sidecars, `pins/` markers;
  `is_pinned` fails closed, refuses symlinks/non-regular.
- **Producer authorities (ports `producer_authority.rs`, Task 12 B1)**:
  `EvidenceProductionAuthority::new(state, evidence)` + `record(run,
  iteration, evidence_ref, producer, locator, produced_at, deadline)` —
  validates durable run + iteration anchor, **derives** the content digest
  from store bytes, mints `AdmittedEvidenceProduction`, calls
  `StateStore::record_evidence_production` (appends `EvidenceProduced` event,
  mints ordering offset server-side). `ObservationBindingAuthority` sibling.
  Held today by `RunController` (controller.rs:603-604, composed :697-700).

## 4. Verifier (app verify::, Task 11)

- `VerificationAuthority::for_run(run_id, iteration_no, ladder,
  workspace_root, deadline) -> GovernedVerification` — loads durable run,
  validates iteration anchor vs `state.resume`, loads+digest-verifies frozen
  projection.
- `GovernedVerification::verify_claim(request) -> VerifiedClaim` — requirement
  from frozen unit's `evidence_required`; candidates from
  `evidence_productions_at`; freshest-first; three-way digest equality
  (store bytes + workspace locator + recorded production digest);
  §14.3 failures → `Inconclusive` + typed `BindingClass`.
- `VerifiedClaim{document: VerificationRecordDoc, domain_record:
  VerificationRecord, evidence}` — no public constructor outside module;
  `record()`, `domain_record()`, `chain_evidence()` (crate-private).
- **`verify/precondition.rs`**: `settlement_precondition(claim, settling_run,
  deadline) -> SettlementReadiness | PreconditionFailure`
  (`NotPass{outcome}|ForeignRun|EvidenceUnresolvable|EvidenceUnpinned|
  PinMembershipUnknown`) — §16.1 conditions 1–2 only; module doc lists
  conditions 3/4/5 as "Task 13's scope". `SettlementReadiness` has **no public
  constructor**.
- `Ladder::new` feature-gated (no production ladder builder yet).

## 5. Controller (app control::, Task 12)

`RunController` (controller.rs:581) holds state/evidence/ids/clock/runner/
observers/comparison deps/ladder/reference/answer_key/escalation/meter_factory/
budget_shape/policy/one canonical workspace_root + prebuilt authorities
(observation_binding, evidence_production, comparison_authority,
verification_authority). `compose(ControllerDeps)`; `start_run` (:744,
freeze-time verify_unchanged + durable run); `resume_run` (:817, §18.3:
re-verify FIRST, `state.resume`, `Budget::restore`, restore in-flight).

Loop `drive` (:994): C8.2 top-of-iteration checkpoint (:1020) →
`boundary_verify` (verify_unchanged every boundary; ReferenceMutated ⇒
terminal aborted) → plan_and_dispatch → drain (charge → observe → bind →
compare → produce evidence → **verify** → dispose → select) →
`book_iteration_round` → decide_progress. `checkpoint()` (:2730) writes
Checkpoint + CheckpointWritten event, threads `state.last_offset`.

**On PASS today** (controller.rs:2239–2282): `verify_claim` →
`settlement_precondition` asserted (failures → pinned forensic evidence, loop
continues) → **readiness token DISCARDED on success** → `Unit::settle` →
`unit_states.insert(Settled)` — **in-memory advance ONLY; no durable
settlement event, no settlements row.**

**commit_settlement callers**: state_store own tests, crash_helper
(`settle-via-commit`), crash_recovery/tamper_evidence tests, cli
export_stack.rs:173. **Zero production callers.**
`commit_settlement`/`apply_settlement_row`/`complete_pending_settlements`/
`settlements` are inherent methods on `SqliteStateStore`, **NOT on the
`StateStore` trait** — unreachable from `Arc<dyn StateStore>`.

In-app crash test: `a_crash_after_a_settlement_never_re_runs_settled_work`
(controller.rs:4949) — asserts via checkpoint-resumed prompts, not durable
settlement rows.

## 6. Reference path

`FileReferenceProvider` (app reference/provider.rs) over `.wayfinder/
{answer-key.md,reference.yaml}`; `AnswerKeySource := Path | Shared`; read-only
per-tick re-read. `verify_unchanged(version)` = I1 tripwire. Production
callers: start_run (:762), resume_run (:829), boundary_verify (:1071).
`frozen_membership`/`frozen_units` (:966/:979) via FrozenReferenceProjection
(F2 item 0). Scripts read-only.

## 7. Answer-key writing surface

**No writer exists anywhere besides git** — provider never writes the ruler
(freeze writes normalized bytes into the evidence store). `.wayfinder` in core
sources only at `config/defaults.rs:195` (const + default path). Arch-tests
crate: modules deps/walk+escapes/denylist/literals/envfile/text; entry
`check_*` fns in lib.rs driven from repo-root `tests/contract/*.rs` targets;
scope constants in walk.rs (CORE_CRATES, INNER_CRATES, COMPOSITION_ROOT,
DEFAULTS_MODULE, DECLARED_SCAN_EXEMPTIONS); anti-vacuity guards; central
exemption lists. **A "single writer / no writer" rule = new check module +
contract target** (only gauntlet-arch-tests may declare repo-root targets).

## 8. Proofs harness (gauntlet-cli/tests/)

Existing: p1, p2, p3, p4, p6, p12, g6, export_stack. **No p7/p11/g10 yet.**
Proofs build real stores in temp Scratch dirs; p12 composes the ladder +
settlement_precondition.

**Crash helper** (state-sqlite `src/bin/crash_helper.rs`, real
`std::process::abort()`):
- `append-events <db> <run-id> <count> [abort-after]`
- `settle <db> <run-id> <unit-id> <abort-point>` — abort points
  `before-append` / `after-append` (half-settled) / `after-apply` / `none`.
- `settle-via-commit <db> <run-id> <unit-id> <before|after|none>`
- `corrupt-tail <db>`
crash_recovery.rs covers: mid-sequence crash readability, half-settled replay
exactly once, before-append retryability, damaged tail ⇒ Damaged.
tamper_evidence.rs:387 asserts the half-settled error wording + replay.

## 9. `-p gauntlet-app settle::` — matches ZERO tests today

No `settle` module exists in gauntlet-app (`rg "mod settle"` empty; `settle::`
substring nowhere under crates/). A new `settle` module makes the plan's gate
non-vacuous.

## Gap list (plan step → substrate → missing)

| Step | Present | Missing |
|---|---|---|
| Domain types | SettlementRecord, Unit::settle, VerificationRecord | record lacks reference_digest/settled_at/authority_decision; no §16.1 cond-3/4/5 aggregate |
| §16.3 durable commit | commit_settlement + apply + replay + cross-validation (tested) | not on StateStore trait; zero production callers; settled_at caller-attested; event `at` not compared |
| Idempotent replay | settlements PK + OR IGNORE + complete_pending_settlements | replay from offset 0 not checkpoint offset; settlements_of offset coercion to 0 |
| In-memory after durable | Unit::settle + unit_states.insert | ordering INVERTED vs AK-041 (in-memory settle with NO durable event; readiness discarded) |
| §16.1 cond 3–5 | cond 1–2 in settlement_precondition; cond-5 mechanism at boundaries; residuals in state.open_residuals | authority chain; blocking-residual check; settle-time explicit re-verify; readiness unused |
| VerifiedClaim consumption | VerifiedClaim + VerificationRecordDoc + records table | no service readiness→commit→advance; no verification_record durable record written from the loop |
| Single-writer + read-only | read-only by construction; verify_unchanged; just ruler | no mechanical arch rule; no g10 proof |
| p7 | crash_helper abort points; crash_recovery patterns | no p7 target; controller-level order + kill/resume unproven |
| p11 | chains separate; settle gated on verify_claim Pass | no negative proof (comparator pass, no record ⇒ no settle) |
| Offset discipline | last_offset threaded; CheckpointWritten | settle appends nothing today; complete_pending ignores checkpoint anchor |
| Export | settlements segment pinned | new rows will alter export content tests (update pinned state per discipline) |

**Composition facts**: insertion point controller.rs:2268-2282; readiness
token computed-and-dropped; `run.reference_version().digest()` is the commit's
reference_digest; the §18.3 anchor rule is duplicated in verify::authority +
producer_authority (settle authority = third mirror — decide dedup);
RunController::compose precedent block :697-704.
