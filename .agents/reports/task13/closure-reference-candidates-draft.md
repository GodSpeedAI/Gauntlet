# Task 13 closure — reference-candidates draft (closure-research verifier, 2026-08-28)

Paste-ready dated entries for `.agents/reports/reference-candidates.md`, in the
established format. Everything from the first `## 2026-08-28` heading below is
the paste; this header block is not. Every locator cited was verified to exist
on the current working tree by direct search (test names grepped, bodies read
where the description depends on them). Ruler line ranges re-read from
`.wayfinder/answer-key.md` (494 lines) on 2026-08-28.

Provenance notes for the closure owner:

- The M-1 disposition paragraph lives in
  `.agents/reports/task13/critic-round1-adjudication.md` (LATER-SCOPE section,
  first paragraph) — not in `final-confirmation-round1-adjudication.md`, which
  contains no M-1 text.
- The AK-024 addendum complements the existing 2026-08-18 entry; it adds no
  new defect, only the composed-status update that entry's scope-honesty
  paragraph anticipated ("substrate-level until plan Tasks 12–13 wire the real
  controller iteration boundaries and the settlement pre-check").
- `just ruler` was NOT run in this verifier session (it is the orchestrator's
  gate); no file outside this draft was written.

---

## 2026-08-28 — AK-031 evidence locator names an unmaterializable test target (Task 13 closure, B4)

- **Unit:** AK-031 (`.wayfinder/answer-key.md` lines 214–219).
- **Claim under review (unchanged):** "Comparators produce residuals only; a
  comparator verdict alone never settles a Unit."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/proofs/p11_critic_is_not_verifier`; `tests/proofs/` exists but is
  empty (no materialized target), and no core crate may declare a repo-root
  test target under the architecture boundary suite's cargo-target-path rule
  (`gauntlet-arch-tests` `escapes.rs` `scan_cargo_target_paths`, exemption
  list exactly `["crates/gauntlet-arch-tests"]`). This is an **additional
  instance of the existing locator-materialization class (first recorded
  2026-08-17, AK-049)** — not a new defect class.
- **Proposed correction:** re-declare the locator to the composed-path proof
  at the composition root, under the target name the plan's gate already uses
  (`cargo nextest run --test p11_critic_is_not_verifier`;
  `crates/gauntlet-cli/tests/p11_critic_is_not_verifier.rs`, with its
  location-rationale header comment — the p1/p3/p7 precedent). Five legs, all
  through the REAL composed controller and REAL durable stores, every
  no-settle leg pinned as forensic evidence with zero `SettlementCommitted`
  events and zero settlements rows:
  - `a_broken_verifier_refuses_and_nothing_settles` (the §18.4
    instrument-failure shape: a supported rung breaks mechanically, the run
    errors with the typed ladder refusal, nothing settles);
  - `no_rung_can_decide_and_nothing_settles` (a rung that declines the
    requirement — the honest "ladder absent" spelling — same typed refusal,
    nothing settles);
  - `an_inconclusive_verdict_never_settles_and_names_itself` (AK-005's
    three-state rule at this seam: refused at the settlement entry gate,
    INCONCLUSIVE named distinctly);
  - `a_fail_verdict_never_settles_and_names_the_real_cause` (a FAIL verdict
    over the same clean comparison — the comparator's pass never closes the
    unit);
  - `a_pass_verdict_settles_the_same_composition` (the PASS control leg: the
    no-settle outcomes above are attributable to the verification gap, never
    to a broken fixture).
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (`just ruler` not run in this closure-research verifier session; the
  orchestrator's post-land gate run is the confirming evidence). Settlement
  of this candidate belongs to the ruler's owner, not the task.

## 2026-08-28 — AK-040 evidence locator names an unmaterializable test target (Task 13 closure, B2)

- **Unit:** AK-040 (`.wayfinder/answer-key.md` lines 277–282).
- **Claim under review (unchanged):** "Settlement requires a PASS
  verification record, a resolvable pinned evidence ref, satisfied authority,
  no blocking residual, and an unchanged reference digest."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/contract/settlement::preconditions`; `tests/contract/` holds only
  the four boundary-suite stubs owned by `gauntlet-arch-tests`
  (`adapter_registry`, `architecture_boundaries`, `env_example_complete`,
  `no_hardcoded_literals`), and no core crate may declare a repo-root test
  target under the same cargo-target-path rule as the AK-049 entry above.
  This is an **additional instance of the existing locator-materialization
  class (first recorded 2026-08-17, AK-049)** — not a new defect class.
- **Proposed correction:** re-declare the locator to the in-module
  precondition matrix plus the composed service path and the append gate that
  prove the claim today (all verified on the tree):
  - the matrix: `gauntlet-app settle::tests
    each_refused_condition_is_a_distinct_typed_refusal`
    (`crates/gauntlet-app/src/settle/mod.rs`) — every refused §16.1
    condition names its own `SettleError` variant (rationale, operator
    identity, authority, blocking residual, durable-block divergence,
    reference mutated, reference unverifiable, undurable run), all distinct;
  - per-condition tests in the same module, grouped under §16.1 section
    banners: condition 1 — `a_fail_claim_refuses_at_service_entry_with_zero_durable_writes`
    and `an_inconclusive_claim_is_a_distinct_entry_refusal_from_fail`;
    conditions 1–2 re-derived at service entry —
    `the_service_re_derives_conditions_one_and_two_over_the_claim_itself`
    (an earned PASS claim whose evidence pin is not durable refuses
    `EvidenceUnpinned` with zero durable writes); condition 3 —
    `a_critic_unit_settles_durably_under_automatic_closure`,
    `a_lead_agent_unit_requires_a_recorded_rationale`, and
    `a_human_authority_unit_refuses_automatic_closure_and_escalates`;
    condition 4 — `a_blocking_residual_referencing_the_unit_refuses_settlement`
    and `a_durably_blocked_projection_wins_over_a_clear_tracked_view`;
    condition 5 —
    `a_settle_time_reference_mutation_aborts_with_both_digests_as_pinned_evidence`
    and
    `a_settle_time_unavailability_refuses_without_aborting_or_escalating`;
    the durability precondition — `an_undurable_run_refuses_every_settlement`;
  - the composed service path through the real controller and real durable
    stores: `gauntlet-cli tests/p7_recovery.rs` (the §16.3 durable legs) and
    `gauntlet-cli tests/p11_critic_is_not_verifier.rs
    a_pass_verdict_settles_the_same_composition` (settlement through the
    governed chain, proving the gates bind the composed path, not just the
    unit-level fixture);
  - the settlement-event append gate (Task 13 COR3, confirmation round 1
    F-1): `gauntlet-adapter-state-sqlite tests/settlement_append_gate.rs`,
    both legs — `the_state_store_append_surface_refuses_the_settlement_kind`
    and `the_event_log_append_surface_refuses_the_settlement_kind`
    (`SettlementCommitted` is earned through `commit_settlement`'s
    authority-minted admission only; every public append surface refuses the
    kind and the durable substrate stays clean).
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (`just ruler` not run in this closure-research verifier session; the
  orchestrator's post-land gate run is the confirming evidence). Settlement
  of this candidate belongs to the ruler's owner, not the task.

## 2026-08-28 — AK-041 evidence locator names an unmaterializable test target (Task 13 closure, B4)

- **Unit:** AK-041 (`.wayfinder/answer-key.md` lines 284–289).
- **Claim under review (unchanged):** "The atomic commit sequence appends the
  durable event before any status write and before any in-memory advance."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/proofs/p7_recovery::commit_order`; `tests/proofs/` is empty and no
  repo-root target can exist under the same cargo-target-path rule as the
  AK-049 entry above. This is an **additional instance of the existing
  locator-materialization class (first recorded 2026-08-17, AK-049)** — not
  a new defect class.
- **Proposed correction:** re-declare the locator to the tests that prove the
  claim today:
  - `gauntlet-cli tests/p7_recovery.rs commit_order` (the locator's own
    test name, materialized at the composition root): the ordering is
    OBSERVED, not inferred — the composition-root store decorator brackets
    every `commit_settlement` with before/after probes over the store's own
    export, and brackets it with the checkpoints that mirror the in-memory
    projection (the checkpoint written before the commit must not carry the
    unit Settled; the first one after must);
  - `gauntlet-adapter-state-sqlite state_store::tests
    commit_settlement_is_the_spec_sequence_and_idempotent_by_key`
    (`crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs`):
    the adapter's own sequence and key-idempotence;
  - the event/row cross-validation tests behind
    `verify_settlement_event_at` (production cross-check in the same file):
    `a_commit_timestamp_disagreeing_with_the_durable_event_is_refused`,
    `a_settlement_row_pointing_at_a_foreign_event_is_refused`,
    `a_settlement_row_pointing_past_the_log_is_refused`, and
    `a_settlement_row_at_its_own_event_offset_lands`.
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (`just ruler` not run in this closure-research verifier session; the
  orchestrator's post-land gate run is the confirming evidence). Settlement
  of this candidate belongs to the ruler's owner, not the task.

## 2026-08-28 — AK-042 evidence locator names an unmaterializable test target (Task 13 closure, B4)

- **Unit:** AK-042 (`.wayfinder/answer-key.md` lines 291–297).
- **Claim under review (unchanged):** "Killing the process mid-settlement and
  resuming completes the settlement exactly once, with no double commit and
  no lost evidence."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/proofs/p7_recovery`; `tests/proofs/` is empty and no repo-root
  target can exist under the same cargo-target-path rule as the AK-049 entry
  above. This is an **additional instance of the existing
  locator-materialization class (first recorded 2026-08-17, AK-049)** — not
  a new defect class.
- **Proposed correction:** re-declare the locator to the tests that prove the
  claim today:
  - the crash legs, `gauntlet-cli tests/p7_recovery.rs`: for each §16.3
    crash window a REAL child process (`crash_helper`,
    `std::process::abort()`) dies against the same database file the
    composed run uses, and a FRESH controller composition drives the
    production `resume_run` path —
    `a_crash_before_the_settlement_event_retries_as_if_never_verified`,
    `a_crash_between_event_and_row_completes_exactly_once_through_resume`,
    `a_crash_after_the_durable_row_never_reruns_settled_work`, and
    `a_settled_run_reresumes_over_anchored_settlements_without_rescanning`;
  - the admission half (one settlement per unit per run):
    `gauntlet-ports producer_authority::tests`
    (`crates/gauntlet-ports/src/producer_authority.rs`) —
    `an_already_committed_settlement_refuses_re_admission`,
    `a_fresh_verification_key_cannot_re_settle_a_settled_unit`,
    `a_settled_unit_re_attempted_under_a_fresh_honest_pass_refuses`,
    `one_pass_record_cannot_settle_n_distinct_verification_refs`, and
    `a_hand_built_pass_document_cannot_become_the_admission_substrate`
    (the admission mints only over a durable PASS record; there is no public
    constructor for the minted types);
  - the divergence teeth (Task 13 COR2-5),
    `gauntlet-adapter-state-sqlite tests/tamper_evidence.rs`:
    `a_same_key_divergent_event_is_loud_on_replay` (a same-key event with
    different facts is corruption on replay, never a silent zero),
    `record_run_refuses_a_changed_intent_on_the_same_digest`, and
    `a_same_facts_duplicate_event_stays_the_idempotent_no_op` (the honest
    replay stays a no-op);
  - service-level re-entry:
    `gauntlet-app settle::tests
    a_half_applied_settlement_completes_exactly_once_from_the_anchor`.
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (`just ruler` not run in this closure-research verifier session; the
  orchestrator's post-land gate run is the confirming evidence). Settlement
  of this candidate belongs to the ruler's owner, not the task.

## 2026-08-28 — AK-043 evidence locator names an unmaterializable test target (Task 13 closure, B3)

- **Unit:** AK-043 (`.wayfinder/answer-key.md` lines 299–304).
- **Claim under review (unchanged):** "The runtime is the sole writer of Unit
  status and evidence fields; a builder or critic write to the answer key is
  prevented or detected."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/antigaming/g10_builder_writes_answer_key`; `tests/antigaming/`
  exists but is empty, and no repo-root target can exist under the same
  cargo-target-path rule as the AK-049 entry above. This is an **additional
  instance of the existing locator-materialization class (first recorded
  2026-08-17, AK-049)** — not a new defect class.
- **Proposed correction:** re-declare the locator to the prevention leg and
  the detection leg that prove the claim today:
  - prevention (structural, §16.4): the single-writer rule of the boundary
    suite — `gauntlet-arch-tests single_writer`
    (`crates/gauntlet-arch-tests/src/single_writer.rs`, exposed as
    `check_answer_key_single_writer` and run by the boundary-suite case
    `tests/contract/architecture_boundaries.rs::answer_key_single_writer`,
    a cargo target `gauntlet-arch-tests` legitimately owns): the core
    modules that own or derive the answer-key surface are pinned, and none
    contains an unguarded write-shaped filesystem operation in production
    code — no writer of the ruler exists anywhere in the core besides git;
    a future `std::fs::write` at the surface fails the boundary suite;
  - both composed legs, `gauntlet-cli tests/g10_builder_writes_answer_key.rs`:
    `a_builder_context_write_at_the_key_is_refused_by_containment` (the
    prevented half at runtime: a builder-context write at the key is refused
    by containment) and `a_midrun_key_tamper_aborts_with_both_digests_pinned`
    (the detected half: a mid-run tamper ends the run terminal `aborted`
    with both digests pinned as forensic evidence and no settlement over the
    mutated ruler).
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (`just ruler` not run in this closure-research verifier session; the
  orchestrator's post-land gate run is the confirming evidence). Settlement
  of this candidate belongs to the ruler's owner, not the task.

## 2026-08-28 — AK-053 evidence locator names an unmaterializable test target (Task 13 closure; partial support, honestly scoped)

- **Unit:** AK-053 (`.wayfinder/answer-key.md` lines 369–374).
- **Claim under review (unchanged):** "Every iteration boundary writes a
  durable checkpoint sufficient to resume without any conversational
  context."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/contract/state::checkpoint_sufficiency`; `tests/contract/` holds
  only the boundary-suite stubs owned by `gauntlet-arch-tests`, and no
  repo-root target can exist under the same cargo-target-path rule as the
  AK-049 entry above. This is an **additional instance of the existing
  locator-materialization class (first recorded 2026-08-17, AK-049)** — not
  a new defect class.
- **Proposed correction:** re-declare the locator to the tests that prove the
  claim today:
  - §18.2 field coverage at the loop: `gauntlet-app control::controller
    a_fresh_run_checkpoint_carries_a_real_event_log_offset`
    (`crates/gauntlet-app/src/control/controller.rs`; the controller's
    `checkpoint()` writes the full §18.2 field set — run id, reference
    digest, iteration number, unit-state projection, budget spent, in-flight
    ids with result-contract paths, event-log offset — and C8.1 makes the
    offset the store-returned water-mark the §18.3 anchor replays from, not
    a constant);
  - resume fidelity from durable facts only (same file):
    `a_resume_completes_half_applied_settlements_and_reconciles_the_projection`,
    `resume_restores_per_unit_attempts_from_durable_productions`,
    `resume_restores_the_budget_from_the_checkpoint_watermark`, and
    `an_answer_key_read_failure_degrades_the_iteration_and_stays_resumeable`;
  - substrate: `gauntlet-adapter-state-sqlite state_store::tests
    resume_returns_the_latest_checkpoint_exactly`,
    `resume_admits_charged_checkpoints_through_the_durable_run_budget`,
    `a_projected_run_still_refuses_charged_checkpoints_loudly`,
    `a_tampered_checkpoint_offset_is_corrupt_never_a_silent_zero`, and
    `anchored_replay_skips_events_below_the_anchor_and_completes_the_rest`;
    plus `encode checkpoint_projections_round_trip`;
  - the crash/resume legs of `gauntlet-cli tests/p7_recovery.rs` (see the
    AK-042 entry above): a fresh composition resumes from the durable
    checkpoint with no in-memory state carried over — the operational
    spelling of "without any conversational context".
- **Scope honesty:** support is partial at the resume-procedure level: §18.3
  steps 1–3 are exercised, but steps 4–5 (workspace sweep for
  settled/out-of-scope units; preflight re-run) are absent by plan scope —
  recorded as RES-A4 in the Task 13 final-confirmation adjudication. The
  checkpoint-write half of the claim is fully covered; "sufficient to
  resume" holds for the implemented resume steps.
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (`just ruler` not run in this closure-research verifier session; the
  orchestrator's post-land gate run is the confirming evidence). Settlement
  of this candidate belongs to the ruler's owner, not the task.

## 2026-08-28 — AK-062 evidence locator names an unmaterializable test target (Task 13 closure; service-level half only, honestly scoped)

- **Unit:** AK-062 (`.wayfinder/answer-key.md` lines 432–437).
- **Claim under review (unchanged):** "Operator resolve and retry actions
  perform the same atomic commit sequence as automatic settlement and cannot
  settle a Unit lacking PASS evidence."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/contract/operator_actions::no_bypass`; `tests/contract/` holds only
  the boundary-suite stubs owned by `gauntlet-arch-tests`, and no repo-root
  target can exist under the same cargo-target-path rule as the AK-049 entry
  above. This is an **additional instance of the existing
  locator-materialization class (first recorded 2026-08-17, AK-049)** — not
  a new defect class.
- **Proposed correction (service-level half):** re-declare the locator to the
  operator-action tests of the settlement service, all in
  `crates/gauntlet-app/src/settle/mod.rs` under the AK-062 section banner:
  - `an_operator_action_without_pass_evidence_hits_the_same_gate_as_the_loop`
    (the no-bypass half: the operator route refuses identically to the
    automatic route);
  - `operator_action_refusals_name_the_three_states_and_the_shared_gate_variant`
    (FAIL / INCONCLUSIVE / missing-evidence refusals name themselves through
    the shared gate variant);
  - `the_operator_identity_cannot_inject_members_into_the_decision_record`
    (§16.1 condition 3's human route: recorded operator identity is a
    minimum bar, not an authority elevation).
- **Scope honesty:** support is the service-level half only. The positive
  half — operator resolve/retry actions performing an actual commit through
  the composed path, and an operator CLI reaching it — is plan-scoped Task
  21 (the adjudication's M-2/F-2 disposition; the plan's own done-when says
  "AK-062 partially", and no operator CLI reaches the composed path yet,
  RES-A5). Revisit this locator when Task 21 lands.
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (`just ruler` not run in this closure-research verifier session; the
  orchestrator's post-land gate run is the confirming evidence). Settlement
  of this candidate belongs to the ruler's owner, not the task.

## 2026-08-28 — AK-024 addendum: the settle-time re-verify is now composed through the real provider (Task 13 closure)

- **Unit:** AK-024 (`.wayfinder/answer-key.md` lines 164–170); addendum to
  the 2026-08-18 entry, which already re-declared this locator and whose
  locators stand unchanged.
- **Claim under review (unchanged):** "The reference digest is re-verified
  at every iteration boundary and immediately before settlement."
- **Update recorded:** the 2026-08-18 entry's scope-honesty paragraph held
  AK-024 "substrate-level until plan Tasks 12–13 wire the real controller
  iteration boundaries and the settlement pre-check". Task 13 landed both
  halves; no new defect, no locator change — this addendum records the
  composed status with verified locators:
  - the settlement pre-check: `gauntlet-app settle
    settle_time_verify` (`crates/gauntlet-app/src/settle/mod.rs`) consults
    the run's frozen `ReferenceVersion` through the real
    `ReferenceProvider::verify_unchanged`
    (`crates/gauntlet-app/src/reference/provider.rs`) — a well-formed
    current digest that differs from the frozen one fires the
    `ReferenceMutated` tripwire with both digests pinned as forensic
    evidence; a broken/missing artifact is the typed `Unavailable` class,
    never conflated with mutation;
  - the iteration-boundary half: the composed controller re-verifies at
    every boundary (`crates/gauntlet-app/src/control/controller.rs`,
    `verify_unchanged` at each boundary consult);
  - the composed detection leg: `gauntlet-cli
    tests/g10_builder_writes_answer_key.rs
    a_midrun_key_tamper_aborts_with_both_digests_pinned` — the real
    controller over the real provider ends terminal `aborted`, the breach
    carries the frozen and current digests, both are pinned as forensic
    evidence, and no settlement lands over the mutated ruler; service-level
    corroboration at `gauntlet-app settle::tests
    a_settle_time_reference_mutation_aborts_with_both_digests_as_pinned_evidence`.
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (`just ruler` not run in this closure-research verifier session; the
  orchestrator's post-land gate run is the confirming evidence). Settlement
  of this candidate belongs to the ruler's owner, not the task.

## 2026-08-28 — settlement.minimum_reliability is §19.2 promotion prose, not a machine-consumable count (Task 13 closure; distinct defect class, NOT the locator class)

- **Unit:** the run-level settlement condition of `.agents/specs/Core-spec.md`
  §16.1 ("A run settles only when every blocking Unit is settled and the
  reference's `settlement.minimum_reliability` condition is met"), read
  against the frozen ruler `.wayfinder/reference.yaml` lines 131–146.
- **Defect (frozen-reference shape divergence, not an evidence locator):**
  the frozen reference's `settlement.minimum_reliability` (lines 143–146) is
  §19.2 promotion prose — "spec §19.2 default: >= 3 settlements under
  recorded variation plus >= 1 handled recovery event before any capability
  is promoted (I6); a single clean run never promotes" — while spec §5.3.1's
  example shape (`minimum_reliability: # e.g. "3 consecutive clean
  verification passes"`) suggests a countable field. A typed
  interpretation of the prose does not exist for THIS ruler, and the I5
  rule forbids settlement gates reading model/free prose: §16.1's run-level
  condition therefore cannot be mechanically consumed as written. This is a
  distinct defect class from the locator-materialization class first
  recorded 2026-08-17 (AK-049) — the locator spellings are not at issue
  here; the field's shape is.
- **Disposition (per the Task 13 critic-round-1 adjudication, M-1):** record
  the divergence here; defer until the first reference revision OR Task 20's
  promotion surface, whichever gives the field a typed meaning first. NOT
  silently dropped: run-settled today requires all-blocking-units settled
  plus the boundary re-verify (the adjudication's m-8 stricter shape —
  conservative direction, so no run settles on weaker grounds than the
  prose intends).
- **Not changed by this entry:** the answer key, the spec, the frozen
  reference, and the frozen lock (`just ruler` not run in this
  closure-research verifier session; the orchestrator's post-land gate run
  is the confirming evidence). Settlement of this candidate belongs to the
  ruler's owner, not the task.
