# Task 13 recon — frozen ruler text + inherited-deferral audit (read-only agent, 2026-08-25)

Sources: `.wayfinder/answer-key.md`, `.agents/reports/reference-candidates.md`,
`.agents/CURRENT_STATUS.md`, `task12/{residual-readjudication,final-confirmation-round2,
residual-adjudication,correction-adjudication,final-confirmation-round1,
confirmation-adjudication-round1,builder-b1,builder-b3,recon-residuals}.md`,
the plan's Task-13 section, live test layout.

## (a) Target units verbatim

**AK-040**: "Settlement requires a PASS verification record, a resolvable pinned
evidence ref, satisfied authority, no blocking residual, and an unchanged
reference digest." — test, cross_cutting, lead_agent, locator
`tests/contract/settlement::preconditions`.

**AK-041**: "The atomic commit sequence appends the durable event before any
status write and before any in-memory advance." — test, cross_cutting,
lead_agent, locator `tests/proofs/p7_recovery::commit_order`.

**AK-042**: "Killing the process mid-settlement and resuming completes the
settlement exactly once, with no double commit and no lost evidence." — test,
cross_cutting, lead_agent, locator `tests/proofs/p7_recovery`,
`depends_on: [AK-041]`.

**AK-043**: "The runtime is the sole writer of Unit status and evidence fields;
a builder or critic write to the answer key is prevented or detected." — test,
**irreversible**, **human**, locator `tests/antigaming/g10_builder_writes_answer_key`.

**AK-062**: "Operator resolve and retry actions perform the same atomic commit
sequence as automatic settlement and cannot settle a Unit lacking PASS
evidence." — test, cross_cutting, lead_agent, locator
`tests/contract/operator_actions::no_bypass`.

Adjacent units: **AK-031** (P11's unit): "Comparators produce residuals only; a
comparator verdict alone never settles a Unit." — locator
`tests/proofs/p11_critic_is_not_verifier`. **AK-024** (settle-time re-verify
half is Task 13's). **AK-053**: "Every iteration boundary writes a durable
checkpoint sufficient to resume without any conversational context." — locator
`tests/contract/state::checkpoint_sufficiency` (CURRENT_STATUS groups it with
the Task-13 proofs; the plan does not name it — resolve at closure). AK-011
(malformed result ⇒ no settlement-bearing state), AK-005 (INCONCLUSIVE never
settles — provisional), AK-055/066/068/070 mention settlement only in passing.
The ruler never uses "single-writer"/"read-only"/"mount" — its phrase is
AK-043's "sole writer"; read-only mounting is plan/spec language feeding
AK-043's "prevented or detected".

## (b) reference-candidates state

**No entry exists for AK-040/041/042/043/062 (nor AK-031/AK-053).** 13 dated
entries total; relevant precedents: AK-049 (2026-08-17, class origin),
AK-022/023/024 (2026-08-18, carries the scope-honesty clause: "AK-023/AK-024
remain substrate-level until plan Tasks 12–13 wire the real controller
iteration boundaries and the settlement pre-check"), AK-030 (2026-08-19,
antigaming locator re-declared to `crates/gauntlet-cli/tests/g6_*.rs`),
AK-005/035/036 (2026-08-21, empirical repo-root-target forbiddance),
AK-037–039/051/052 (2026-08-25, direct shape precedent).

## (c) Inherited-trigger list (15 items; exhaustive sweep)

1. **Task-3**: `SettlementRecord` store-binding at Task 13 (SettlementService
   enforces §16.1; the domain type cannot see the store).
2. ~~Task-6 MAJOR-2 non-zero budget resume~~ — **CLOSED by Task 12 B1/B3**
   (`Budget::restore`/`Spent` reconstruction composed into `resume_run`).
   NOT inherited.
3. **Task-6 intra-call §16.3 swap discrimination**: naive swap structurally
   refused (offset cross-validation), but defeating the cross-check needs
   mid-function abort injection. "Trigger: **Task 13** P7
   (`p7_recovery::commit_order`, AK-041's declared locator)."
4. **Task-6 MINOR-4 replay from offset 0**: §18.3 names the checkpoint offset;
   superset replay idempotent-safe today. "Trigger: **Task 13** resume
   composition." Cross-referenced as the same obligation as Task-12 L2.
5. **Task-6 R1/R3 settlement-side coercions**: `settled_at` caller-attested;
   tampered `settlements.event_log_offset` reads 0 not Corrupt
   (`unwrap_or(0)`). Task-12 readjudication: "**the settlement-row half is
   Task 13's own surface, where P7 will exercise it**."
6. **Task-7 m1**: `ReferenceVersion` lacks the spec's `evidence_ref` field;
   linkage by `sha256:<hex>` convention. "Trigger: **Task 13** settlement
   persistence or the ports revision."
7. **Task-7 M4 settlement half**: no production settle-time `verify_unchanged`
   caller; digests not persisted as evidence. "Trigger: Tasks 12–13";
   Task-12 adjudication: "**Settlement pre-check half stays Task 13.**"
8. **Task-11 unimplemented-3 production-row crash-window replay**: window
   between durable `EvidenceProduced` event append and row insert named in the
   error string; no replay code. "Trigger: **Task 13** recovery composition."
9. **Task-12 L2**: full §18.3 resume (log replay, workspace sweep, preflight,
   collected-results refeeding) + retry-ledger checkpoint persistence (§18.2
   field set spec-fixed; shape change is a ports revision). "Trigger: Task 13
   P7."
10. **Task-12 L3**: mid-window crash replay of the settlement commit — "Task
    13's §16.3 discipline. The Task-12 checkpoint ORDER is fixed now (C8); the
    replay machinery is not built here."
11. **Task-12 §21.3 non-integrity mid-run failures**: answer-key read failure
    (hard `ControlError::Reference` at controller.rs:1069) + runner poll
    transport error (Err at controller.rs:1698) vs §21.3's "stop new dispatch;
    keep the process alive" and §11.5 dispositions. "Trigger: Task 13 recovery
    composition, or the next §21.3 degradation-policy touch."
12. **Task-12 confirmation MINOR-6**: evidence production binds the FIRST
    facet's bytes to every attempted unit (controller.rs:2162-2220) — coarse
    per-unit binding. Trigger: Task 13/20.
13. **Task-12 conditional**: `BudgetMeter::mint` type-level seal — "later-scope,
    trigger: the next `BudgetMeter` ports revision (Task 13 P7 resume surface
    or Task 15 real runner, whichever touches the port first)."
14. Task-12 umbrella: all ledger items with Task-13 triggers untouched.
15. Status notes: AK-041/042/053 have substrate-level support (commit-order
    primitive, exactly-once keyed replay, §18.2 checkpoint durability, real
    process-kill proven) but declared locators are Task-13 proofs — not
    claimed. CURRENT_STATUS Next-action names the inherited surface set:
    "L2/L3 above plus the settlement-side sqlite coercions (`settled_at`,
    non-checkpoint `unwrap_or(0)` legs)".

## (d) Locator materialization classification

Live layout: repo-root `tests/{antigaming,contract,e2e,proofs}` exist but hold
only the four Task-2 contract stubs + fixtures; composition proofs live at
`crates/gauntlet-cli/tests/*.rs`. **All five targets are materialization-class,
none materializable as-written** (cargo-target-path escape rule; empirically
verified 2026-08-21):

| Unit | Locator as written | Realizable shape |
|---|---|---|
| AK-040 | `tests/contract/settlement::preconditions` | in-crate `gauntlet-app settle::` (the plan's own gate filter) |
| AK-041 | `tests/proofs/p7_recovery::commit_order` | `crates/gauntlet-cli/tests/p7_recovery.rs` test `commit_order` |
| AK-042 | `tests/proofs/p7_recovery` | `crates/gauntlet-cli/tests/p7_recovery.rs` |
| AK-043 | `tests/antigaming/g10_builder_writes_answer_key` | `crates/gauntlet-cli/tests/g10_builder_writes_answer_key.rs` (g6 precedent) |
| AK-062 | `tests/contract/operator_actions::no_bypass` | in-crate/service-level no-bypass (full CLI half is Task 21) |

Also: AK-031 → `crates/gauntlet-cli/tests/p11_critic_is_not_verifier.rs`;
AK-053 → same class as AK-054.

## (e) Gaps and ambiguities

1. **AK-062 "partially"**: plan done-when names it; full operator-action closure
   is Task 21; Task 13 owns the service-level no-bypass half.
2. No reference-candidates entries for any Task-13 target — create at closure.
3. Ruler-vs-plan terminology: "sole writer" vs plan's "single-writer … confined
   to one module … read-only mounting" — the mounting behavior has no ruler
   sentence of its own; it feeds AK-043 "prevented or detected".
4. **AK-043 is irreversible+human** — the only target at that posture; HITL
   rule 9 and Task-14 L1 adjacency apply ("`Unit::settle` authority-blindness is
   the §16.1 SettlementService (Task 13)" with trigger Tasks 13/14).
5. L2's retry-ledger half may force a ports revision (spec-fixed §18.2 field
   set) — inherited tension, not owned.
6. The §16.3 swap deferral expects abort injection — Task 6's process-abort
   crash helper is the designated instrument (with the recorded honest limit:
   `synchronous=OFF` not demonstrable via process abort).
7. Substrate already exists from Tasks 6+12 — compose, don't rebuild. The
   controller asserts a "settlement precondition assertion (Task-13 boundary)"
   placeholder in the loop (builder-b3.md:91).
8. Task-12 already consumed the budget-resume deferral — do not redo.
9. AK-053 is a quiet sixth unit — resolve when claiming at closure.
