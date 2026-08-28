# Reference candidates — dated entries against the frozen ruler

Per `.gauntlet/reference.lock`: ".wayfinder/answer-key.md is never edited for
any reason … A Unit that seems wrong becomes a dated entry in
`.agents/reports/reference-candidates.md` with evidence." The same discipline
applies to the spec source (`.agents/specs/Core-spec.md`): revisions go
through the normal spec process, never a builder's direct edit.

## 2026-08-17 — AK-049 evidence locator names an unmaterializable test target (Task 5 correction K8 / critic-B R8)

- **Unit:** AK-049 (`.wayfinder/answer-key.md` lines 341–346; identically
  `.agents/specs/Core-spec.md` lines 1745–1749).
- **Claim under review (unchanged, satisfied):** "A sandbox policy file for
  the irreversible tier causes doctor to fail."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/e2e/doctor::irreversible_policy_is_error`; no such test exists
  (`tests/e2e/` holds only `.gitkeep`), and the architecture boundary
  suite's Cargo-target-path rule (`gauntlet-arch-tests` `escapes.rs`
  `scan_cargo_target_paths`, exemption list exactly
  `["crates/gauntlet-arch-tests"]`) forbids a core crate from declaring a
  repo-level test target, so the locator cannot be materialized as spelled.
- **Proposed correction:** re-declare the locator to the in-crate test paths
  that prove the claim (all in `crates/gauntlet-app`):
  - `doctor::tests::a_present_top_tier_policy_fails_check_five_and_the_run`
    (canonical spelling `irreversible.json` in the effective dir: check 5
    fail, AK-049 in summary and doctor.json, exit non-zero, removal green);
  - `doctor::tests::forbidden_tier_policies_under_any_spelling_or_link_fail_check_five`
    (K4 hardening: `irreversible.yaml`, `IRREVERSIBLE.JSON`, extensionless
    `irreversible`, and a broken symlink all fail; clean dir green);
  - `doctor::tests::a_forbidden_policy_in_the_default_dir_fails_even_when_policy_dir_is_overridden`
    (K4 hardening: forbidden file in the default `.gauntlet/policies` dir
    fails with both-dir evidence while `sandbox.policy_dir` is overridden).
- **Live-CLI corroboration:** planted `.gauntlet/policies/irreversible.json`
  → `just doctor` exit 1, check 5 fail citing AK-049; removed → exit 0
  (Task 5 teeth, re-run after the K4 hardening in the 2026-08-17 correction
  round; transcript in `.agents/reports/task5/builder-round1.md`).
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (digests still match; `just ruler` green). Settlement of this
  candidate belongs to the ruler's owner, not the builder.

## 2026-08-17 — AK-054 evidence locator names an unmaterializable test target (Task 6, critic-A MINOR-7)

- **Unit:** AK-054 (`.wayfinder/answer-key.md` lines 376–381).
- **Claim under review (unchanged, satisfied at the instrument level):** "Run
  records export deterministically to JSONL; exporting the same run twice
  produces byte-identical output."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/contract/state::deterministic_export`; no such file or target exists,
  and the architecture boundary suite's Cargo-target-path rule (same class as
  the AK-049 entry above) forbids a core crate from declaring a repo-level test
  target, so the locator cannot be materialized as spelled.
- **Proposed correction:** re-declare the locator to the tests that prove the
  claim today:
  - `gauntlet-app state::export::deterministic::export_is_byte_identical_and_canonical_under_reordered_input`
    (the plan's own Task 6 gate filter; byte-identical double export under a
    store that reorders lines between exports);
  - `gauntlet-adapter-state-sqlite state_store::tests::export_is_the_pinned_golden_and_byte_identical`
    (pinned golden bytes, out-of-order checkpoint writes);
  - `gauntlet-cli tests/export_stack.rs the_real_sqlite_export_stack_is_deterministic_and_canonical`
    (the composed stack: real adapter bytes through the canonical re-export,
    twice, byte-identical).
- **Not changed by this entry:** the answer key, the spec, and the frozen lock
  (digests still match; `just ruler` green after every round). Settlement of
  this candidate belongs to the ruler's owner, not the builder.

## 2026-08-18 — AK-021 evidence locator names an unmaterializable test target (Task 7, deliverable D8)

- **Unit:** AK-021 (`.wayfinder/answer-key.md` lines 142–148).
- **Claim under review (unchanged, satisfied at the instrument level):**
  "Reference resolution normalizes deterministically: the same logical
  reference always produces the same digest."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/contract/reference::normalization_is_deterministic`; no such file
  or target exists, and the architecture boundary suite's
  cargo-target-path rule (same class as the AK-049 entry above) forbids a
  core crate from declaring a repo-level test target, so the locator cannot
  be materialized as spelled. This is an **additional instance of the
  existing locator-materialization class (first recorded 2026-08-17,
  AK-049)** — not a new defect class.
- **Proposed correction:** re-declare the locator to the tests that prove
  the claim today (in-crate, under the plan's own gate filter
  `cargo nextest run -p gauntlet-app reference::`):
  - `gauntlet-app reference::normalize::normalization_is_deterministic_across_line_ending_and_final_newline_spellings`
    (one digest across CRLF/LF/lone-CR endings and final-newline
    present/absent/doubled spellings — AK-021's determinism face);
  - `gauntlet-app reference::normalize::normalization_is_sensitive_to_content_edits`
    (the teeth face of the same rule: one trailing space on a content line,
    one interior blank line, and reordering two lines each change the
    digest);
  - corroborating composed-path evidence: the Task 7 gate diffs
    `scripts/reference-digest.sh .wayfinder/answer-key.md` against
    `gauntlet reference digest --path .wayfinder/answer-key.md` and
    requires byte-identical output — two independent implementations of the
    same rules agreeing is the determinism proof.
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (`just ruler` not run in this write-only builder session; the
  orchestrator's post-land gate run is the confirming evidence). Settlement
  of this candidate belongs to the ruler's owner, not the builder.

## 2026-08-18 — AK-022/AK-023/AK-024 evidence locators name an unmaterializable test target (Task 7, deliverable D8)

- **Units:** AK-022 (`.wayfinder/answer-key.md` lines 149–155), AK-023
  (lines 156–163), AK-024 (lines 164–170).
- **Claims under review (unchanged):** AK-022 — "The frozen reference is
  stored in the evidence store outside every builder-writable path.";
  AK-023 — "A builder attempt to modify the active reference fails or is
  detected and aborts the run with reference_mutated."; AK-024 — "The
  reference digest is re-verified at every iteration boundary and
  immediately before settlement."
- **Defect (evidence declaration only):** the three locators are spelled
  under `tests/proofs/p1_reference_integrity…`; no such repo-root target
  can exist under the same cargo-target-path rule as the AK-049 entry
  above. This is an **additional instance of the existing
  locator-materialization class (first recorded 2026-08-17, AK-049)** —
  not a new defect class.
- **Proposed correction:** re-declare the locators to the composed-path
  proof at the composition root, under the target name the plan's gate
  already uses (`cargo nextest run --test p1_reference_integrity`;
  `crates/gauntlet-cli/tests/p1_reference_integrity.rs`, with its
  location-rationale header comment):
  - AK-022 → `gauntlet-cli tests/p1_reference_integrity.rs frozen_copy_outside_workspace`;
  - AK-023 → `gauntlet-cli tests/p1_reference_integrity.rs builder_mutation_of_the_active_reference_aborts_with_reference_mutated`;
  - AK-024 → `gauntlet-cli tests/p1_reference_integrity.rs digest_rechecked`;
  - adjacent composed-path evidence: `the_run_carries_the_frozen_version`
    (freeze binds the `ReferenceVersion` to run identity through the real
    embedded state store).
- **Scope honesty:** AK-023/AK-024 remain substrate-level until plan Tasks
  12–13 wire the real controller iteration boundaries and the settlement
  pre-check; `digest_rechecked`'s harness loop stands in for those
  callsites, and the provider module header names the three real
  strict-admission callsites.
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (`just ruler` not run in this write-only builder session; the
  orchestrator's post-land gate run is the confirming evidence). Settlement
  of these candidates belongs to the ruler's owner, not the builder.

## 2026-08-18 — AK-025 evidence locator names an unmaterializable test target (Task 7 correction C4, critic-A MAJOR-1)

- **Unit:** AK-025 (`.wayfinder/answer-key.md` lines 171–176).
- **Claim under review (unchanged):** "A run may record a candidate reference
  revision but cannot adopt one mid-run."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/antigaming/g2_change_reference`; no such repo-root target can exist
  under the same cargo-target-path rule as the AK-049 entry above. This is an
  **additional instance of the existing locator-materialization class (first
  recorded 2026-08-17, AK-049)** — not a new defect class. Recorded now
  because the Task 7 round's D8 deliverable covered AK-021 and AK-022/023/024
  and stopped one Unit short of the plan's done-when range (critic-A MAJOR-1).
- **Disposition honesty (both halves of the claim):**
  - "cannot adopt one mid-run" — substrate-level today: the P1 tripwire
    (`gauntlet-cli tests/p1_reference_integrity.rs`
    `builder_mutation_of_the_active_reference_aborts_with_reference_mutated`
    and `digest_rechecked`) aborts on any mid-run reference move, and the
    frozen `ReferenceVersion` makes adoption structurally impossible without
    a new version. The runtime-level "record but never adopt" flow (a
    `CandidateRevision` admitted against the frozen ruler) is the anti-gaming
    suite's business.
  - "may record a candidate reference revision" — no substrate yet; the
    domain `CandidateRevision` type exists (Task 3) but no run-path records
    one. Owned by plan Task 22/23 (memory/optimizer, P9/P10) and exercised
    adversarially by plan Task 24 (G2).
- **Proposed correction:** re-declare the locator when Task 24 materializes
  the anti-gaming suite (its tests live under a crate test target per the
  same rule), citing the then-existing G2 test plus the P1 tripwire tests as
  the substrate evidence.
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (`just ruler` PASS verified by the orchestrator in the same session).
  Settlement of this candidate belongs to the ruler's owner, not the builder.

## 2026-08-19 — AK-028 evidence locator names an unmaterializable test target (Task 9, deliverable D5)

- **Unit:** AK-028 (`.wayfinder/answer-key.md` lines 193–198).
- **Claim under review (unchanged):** "Every observation facet records
  instrument identity, exact command, environment fingerprint, exit status,
  and a content-addressed evidence ref."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/contract/observer::facet_provenance`; no such repo-root target can
  exist under the same cargo-target-path rule as the AK-049 entry above.
  This is an **additional instance of the existing
  locator-materialization class (first recorded 2026-08-17, AK-049)** —
  not a new defect class.
- **Proposed correction:** re-declare the locator to the per-adapter
  provenance tests plus the application layer's cross-check tests, all
  existing and green in this round:
  - filesystem → `gauntlet-adapter-observe-filesystem::shared_contract
    a_facet_carries_full_provenance_and_matching_evidence`;
  - git/versioning → `gauntlet-adapter-observe-git::shared_contract
    a_facet_carries_full_provenance_and_matching_evidence`;
  - build/test/command → `gauntlet-adapter-observe-{build,test,command}::shared_contract
    a_facet_carries_full_provenance_and_a_failing_exit_is_still_a_facet`;
  - the evidence document's own codec (instrument identity, command, env
    fingerprint fields) → `gauntlet-ports observer_evidence::*`
    (7 tests, strict-decode teeth);
  - app cross-check (facet vs stored evidence, exit-status agreement) →
    `gauntlet-app observation::assembly::tests::a_provenance_disagreement_is_recorded_not_fatal`
    and
    `observation::assembly::tests::the_assembled_document_round_trips_and_feeds_verification_inputs`;
  - composed-path corroboration → `gauntlet-cli
    tests/p3_real_observation.rs
    the_lying_builders_claim_diverges_from_reality_inside_the_bundle`
    (asserts the command facet's stored evidence strict-decodes with
    matching exit status 1, exact declared command, non-empty env
    fingerprint).
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (`just ruler` runs in the round's gate run; the orchestrator's
  post-land re-run is the confirming evidence). Settlement of this
  candidate belongs to the ruler's owner, not the builder.

## 2026-08-19 — AK-029 evidence locator names an unmaterializable test target (Task 9, deliverable D5)

- **Unit:** AK-029 (`.wayfinder/answer-key.md` lines 200–205).
- **Claim under review (unchanged):** "Builder-authored claims are stored
  in a claims facet that is excluded from verification inputs by
  construction."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/proofs/p3_real_observation`; no such repo-root target can exist
  under the same cargo-target-path rule as the AK-049 entry above.
  **Additional instance of the existing locator-materialization class
  (first recorded 2026-08-17, AK-049)** — not a new defect class.
- **Proposed correction:** re-declare the locator to the composed-path
  proof at the composition root, under the target name the plan's gate
  already uses (`cargo nextest run --test p3_real_observation`;
  `crates/gauntlet-cli/tests/p3_real_observation.rs`, with its
  location-rationale header comment — the AK-022/023/024 p1 precedent):
  - `the_lying_builders_claim_diverges_from_reality_inside_the_bundle`
    (the full P3 story: claims facet carries the lying document's success
    claims verbatim while the re-run self-check's failure is the recorded
    facet reality, and `VerificationInputs::of_bundle` canonical bytes
    contain NONE of the claims text — canary over every claim);
  - `a_claims_swap_moves_the_digest_but_not_the_facet_inputs`
    (claims are digest-covered but never facet inputs);
  - substrate: `gauntlet-app observation::input::tests::claims_text_never_reaches_verification_inputs`,
    `...::the_facet_and_gap_inputs_are_identical_under_a_claims_swap`, and
    `observation::bundle::tests::a_claims_facet_claiming_inclusion_does_not_decode`
    (the "by construction" half: `InstrumentFacets` is built only by
    `of_bundle`, `ClaimsFacet` has no conversion path, and a wire document
    spelling `excluded_from_verification: false` does not decode).
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (`just ruler` runs in the round's gate run; the orchestrator's
  post-land re-run is the confirming evidence). Settlement of this
  candidate belongs to the ruler's owner, not the builder.

## 2026-08-19 — AK-030 evidence locator names an unmaterializable test target (Task 9, deliverable D5)

- **Unit:** AK-030 (`.wayfinder/answer-key.md` lines 207–212).
- **Claim under review (unchanged):** "Unavailable evidence and observation
  failures are recorded explicitly and never dropped or defaulted to
  success."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/antigaming/g6_hide_failed_observations`; no such repo-root target
  can exist under the same cargo-target-path rule as the AK-049 entry
  above. **Additional instance of the existing locator-materialization
  class (first recorded 2026-08-17, AK-049)** — not a new defect class.
- **Proposed correction:** re-declare the locator to the composed-path
  anti-gaming proof at the composition root, under the target name the
  plan's gate already uses (`cargo nextest run --test
  g6_hide_failed_observations`;
  `crates/gauntlet-cli/tests/g6_hide_failed_observations.rs`, with its
  location-rationale header comment — p1 precedent):
  - `a_failed_probe_is_recorded_in_both_arrays_and_covered_by_the_digest`
    (spawn-failure observer: both gap arrays populated exactly once with
    the failed kind, `has_gaps()` true, and a rebuilt document with a
    mutated gap array digests differently — hidden edits are
    tamper-evident);
  - `a_probe_store_write_failure_surfaces_as_gap_entries`
    (store-put failure at the probe: same double gap, bundle still
    durably written);
  - `an_all_healthy_bundle_attests_empty_gap_arrays_explicitly`
    (the positive statement: EMPTY arrays as populated types, never
    `Option`);
  - substrate: `gauntlet-app observation::assembly::tests::observer_failures_populate_both_gap_arrays_exactly_once`,
    `...::an_observer_store_write_failure_surfaces_as_gap_entries`,
    `...::all_healthy_observers_leave_both_arrays_explicitly_empty`, and
    `...::a_provenance_disagreement_is_recorded_not_fatal` (the fourth
    failure shape, recorded as data).
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (`just ruler` runs in the round's gate run; the orchestrator's
  post-land re-run is the confirming evidence). Settlement of this
  candidate belongs to the ruler's owner, not the builder.

## 2026-08-20 — AK-026 and AK-027 evidence locators name unmaterializable test targets (Task 10, deliverable D6)

- **Units:** AK-026 (`.wayfinder/answer-key.md` lines 178–183), AK-027
  (lines 185–190).
- **Claims under review (unchanged, satisfied at the instrument level):**
  "The comparison envelope is built from an allowlist and contains no
  builder trajectory, reasoning, identity, attempt count, or prior
  verdicts" (AK-026); "Adding a deny-listed field to the envelope makes
  the blindness proof fail" (AK-027).
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/proofs/p2_context_isolation` and
  `tests/proofs/p2_context_isolation::teeth`; no such repo-root target can
  exist under the same cargo-target-path rule as the AK-049 entry
  (exemption list exactly `["crates/gauntlet-arch-tests"]`). **Additional
  instances of the existing locator-materialization class (first recorded
  2026-08-17, AK-049)** — not a new defect class.
- **Proposed correction:** re-declare the locators to the composed-path
  proof at the composition root, under the target name the plan's gate
  already uses (`cargo nextest run --test p2_context_isolation`;
  `crates/gauntlet-cli/tests/p2_context_isolation.rs`, with its
  location-rationale header comment — the p1/p3 precedent):
  - `the_envelope_is_structurally_blind_and_carries_instrument_reality`
    (AK-026: six canaries — claims content, builder reasoning, identity,
    attempt count, prior verdicts, operator chat — each present in
    builder-side source material and absent from the envelope's canonical
    bytes; `assert_blind` passes; the failing instrument's exit status and
    evidence ref ARE in the bytes; a tampered bundle is refused at the
    consumption boundary; the real compare-diff adapter derives its
    blocking residual from the instrument failure with all three digests
    coupled to the run's frozen reference digest);
  - `teeth` (AK-027: the designed-to-fail pair — `builder_reasoning` +
    canary injected into the serialization, `assert_blind` FAILS and names
    the field; every deny-listed spelling likewise);
  - substrate: `gauntlet-app compare::envelope::tests::the_envelope_carries_facet_reality_and_none_of_the_claims`,
    `...::a_poisoned_serialization_fails_the_blindness_proof` (every
    deny-listed name), and `...::a_tampered_bundle_is_refused_at_the_consumption_boundary`.
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (`just ruler` runs in the round's gate run; the orchestrator's
  post-land re-run is the confirming evidence). Settlement of this
  candidate belongs to the ruler's owner, not the builder.

## 2026-08-20 — AK-032, AK-033, AK-034 evidence locators name unmaterializable test targets (Task 10, deliverable D6)

- **Units:** AK-032 (`.wayfinder/answer-key.md` lines 221–226), AK-033
  (lines 228–233), AK-034 (lines 235–240).
- **Claims under review (unchanged, satisfied at the instrument level):**
  "Residual aggregation is deterministic, is not performed by a model, and
  any blocking residual forces an aggregate fail verdict" (AK-032);
  "Adaptive review runs deterministic probes first and fans out model
  critics only across dimensions those probes left undecided" (AK-033);
  "Critic fan-out truncated by budget records the missing dimensions as
  unavailable evidence rather than omitting them" (AK-034).
- **Defect (evidence declaration only):** the locators
  `tests/contract/comparison::deterministic_aggregation`,
  `tests/contract/review_mode::adaptive_skips_settled_dimensions`, and
  `tests/contract/review_mode::truncation_is_recorded` spell repo-root
  test targets under `tests/contract/`, which the same cargo-target-path
  rule forbids for core crates; the `tests/contract/` directory itself
  exists only for the Task-2 evidence-locator stubs. **Additional
  instances of the existing locator-materialization class (first recorded
  2026-08-17, AK-049)** — not a new defect class.
- **Proposed correction:** re-declare the locators to the in-crate tests
  that prove the claims today (all in `crates/gauntlet-app`, under the
  module path the plan's own Task-10 gate filter `compare::` already
  hits):
  - `gauntlet-app compare::aggregate::tests::deterministic_aggregation`
    (AK-032: all six input permutations byte-identical; highest-severity
    merge with evidence union; dispute preserved; blocking forces fail;
    no model in the merge path — identity runner
    `gauntlet-app::compare::aggregate`, empty model field; corroborated by
    `...::any_blocking_residual_forces_fail_regardless_of_everything_else`
    and `...::empty_and_incoherent_inputs_are_typed_errors`);
  - `gauntlet-app compare::review::tests::adaptive_skips_settled_dimensions`
    (AK-033: the instrument-settled dimension receives NO model dispatch
    and its probe residual survives the merge; the clean control
    dispatches both);
  - `gauntlet-app compare::review::tests::truncation_is_recorded`
    (AK-034: budget-truncated dimensions recorded as unavailable-evidence
    entries with dimension-name kinds and truncation-citing reasons;
    zero-budget single mode records every dimension with no fabricated
    record; a truncated adversarial pass is recorded too);
  - adapter corroboration: `gauntlet-adapter-compare-diff
    tests::a_failing_facet_yields_a_blocking_residual_from_the_instrument`
    and `gauntlet-adapter-compare-model
    tests::the_critic_composition_couples_the_dispatch_and_the_record`
    (the two §6.3 REQUIRED comparator adapters over the widened envelope,
    both passing the unmodified shared suite).
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (`just ruler` runs in the round's gate run; the orchestrator's
  post-land re-run is the confirming evidence). Settlement of these
  candidates belongs to the ruler's owner, not the builder.

## 2026-08-21 — AK-005, AK-035, AK-036 evidence locators name unmaterializable test targets (Task 11)

- **Units:** AK-005 (three-state `Outcome`; INCONCLUSIVE never satisfies a
  settlement requirement — locator `tests/proofs/p12_missing_is_not_passing`),
  AK-035 (deterministic-first — locator
  `tests/contract/verifier::deterministic_first`), AK-036 (evidence binding —
  locator `tests/contract/verifier::evidence_binding`).
- **Defect class (existing, not new):** locator materialization — first
  recorded 2026-08-17 for AK-049, then AK-021/022/023/024/025 (Task 7),
  AK-026/027/032/033/034 (Task 10), AK-028/029/030 (Task 9). All three
  Task-11 locators spell repo-root test-target paths that the frozen boundary
  suite forbids for core crates. For AK-005 the forbiddance is empirical, not
  inferred: declaring `tests/proofs/p12_missing_is_not_passing.rs` (repo root)
  as a `[[test]]` target of `gauntlet-cli` failed `just boundaries` twice —
  `scope-escape` (target source root outside the crate — spec §24.3, AK-001)
  and `hardcoded-relative-path` (spec §8.1, AK-016) — with the target-path
  exemption list pinned by an existing suite case; verbatim messages in
  `.agents/reports/task11/builder-round1.md` §4. `gauntlet-arch-tests` cannot
  host it either (depends on no internal crate by its own manifest
  constraint). AK-035/AK-036's `::`-form paths are the same
  cargo-target-path shape already adjudicated for AK-049.
- **Proposed correction — AK-005:** re-declare the locator to
  `gauntlet-cli tests/p12_missing_is_not_passing` (the composition-root proof
  whose cargo target name is exactly the plan's Task-11 gate spelling).
  Carrying tests: all eleven (absent evidence → INCONCLUSIVE at every layer,
  never PASS; stale → `evidence_stale`; lying request rejected; producer and
  locator mismatches; happy anchor PASS; deterministic-first with zero model
  dispatches; model flag; three distinct classes).
- **Proposed correction — AK-035:**
  `gauntlet-cli tests::p12_missing_is_not_passing::the_deterministic_rung_decides_when_the_model_rung_could_too`
  (composed: both rungs declare support; the deterministic instrument's
  identity is recorded; the scripted model runner records ZERO dispatches),
  corroborated by
  `gauntlet-app verify::ladder::tests::when_a_deterministic_rung_and_the_model_rung_could_both_decide_the_deterministic_one_does`
  and `...::the_model_rung_decides_only_where_no_deterministic_rung_can`, plus
  the composed flag test
  `every_model_rung_record_is_flagged_non_deterministic`.
- **Proposed correction — AK-036:**
  `gauntlet-cli tests::p12_missing_is_not_passing::the_three_binding_classes_are_reachable_and_distinct`
  with its sibling scenario tests
  (`absent_store_evidence_is_inconclusive_at_every_layer_never_pass`,
  `stale_evidence_from_a_prior_iteration_is_inconclusive_stale`,
  `instrument_produced_evidence_never_satisfies_a_human_producer`,
  `evidence_at_another_locator_is_unresolvable_not_the_required_evidence`,
  `an_absent_artifact_at_a_resolving_production_is_inconclusive_p12`),
  corroborated by `gauntlet-app verify::authority::tests::*`
  (`an_unresolvable_evidence_ref_is_inconclusive_unresolvable`,
  `evidence_from_a_prior_iteration_is_inconclusive_stale`,
  `instrument_produced_evidence_never_satisfies_a_human_requirement`,
  `a_lying_request_cannot_make_stale_evidence_fresh`) and
  `gauntlet-adapter-state-sqlite state_store::tests::evidence_production_records_are_write_once_and_idempotent`.
- **Not changed by this entry:** the answer key, the spec, the frozen lock.
  Settlement of these candidates belongs to the ruler's owner, not the task.

## 2026-08-25 — AK-037, AK-038, AK-039, AK-051, AK-052 evidence locators name unmaterializable test targets (Task 12 closure)

- **Units:** AK-037 (`.wayfinder/answer-key.md` lines 256–262), AK-038
  (lines 263–269), AK-039 (lines 270–276), AK-051 (lines 355–361), AK-052
  (lines 362–368).
- **Claims under review (unchanged):** AK-037 — "The controller records the
  selected correction together with rejected alternatives and their
  reasons." AK-038 — "An injected controlled defect is exposed by
  observation and comparison and closed by a routed corrective iteration."
  AK-039 — "A repeated identical residual triggers bounded diagnosis rather
  than another patch, and exhausting the diagnostic budget yields
  INCONCLUSIVE." AK-051 — "A non-converging task terminates at the declared
  budget limit with the recovery reserve unspent." AK-052 — "The budget
  meter is consulted before every dispatch and refuses any dispatch that
  would consume the recovery reserve."
- **Defect (evidence declaration only):** all five `evidence_required`
  locator spellings name repo-root test targets that cannot exist under the
  architecture boundary suite's cargo-target-path rule (the
  locator-materialization class first recorded 2026-08-17, AK-049; same
  class as AK-021/AK-028 and the Task-10/Task-11 entries above) — not a
  new defect class. AK-038 and AK-051 additionally name a `tests/proofs/`
  root that has never existed: the proofs live at the composition root as
  `crates/gauntlet-cli/tests/{p4_corrective_loop,p6_budget_control}.rs`
  under the target names the plan's own gate already uses.
- **Proposed correction — AK-037:** re-declare to the composed-path
  evidence that proves the record today:
  `gauntlet-cli tests/p4_corrective_loop.rs an_injected_defect_is_exposed_compared_and_closed_by_a_routed_correction`
  (both the correcting and the closing iteration durably record a §15.1
  selection; the correcting record names the bounded re-dispatch and a
  `SpeculativeWhenBoundedAvailable` rejection);
  `gauntlet-cli tests/p6_budget_control.rs a_non_converging_task_exhausts_the_forward_budget_with_the_reserve_intact`
  (one residual-driven record per forward round, every one non-vacuous,
  read back from the durable store); and the vacuity refusal itself,
  `gauntlet-domain correction::tests a_selection_without_rejected_alternatives_is_unrepresentable`.
- **Proposed correction — AK-038:**
  `gauntlet-cli tests/p4_corrective_loop.rs an_injected_defect_is_exposed_compared_and_closed_by_a_routed_correction`
  — the full composed loop (real controller, real diff comparator over the
  real workspace the builder wrote, governed-chain verification settling
  the unit), plus the routing teeth evidence in
  `.agents/reports/task12/teeth.md` T5.
- **Proposed correction — AK-039:**
  `gauntlet-cli tests/p4_corrective_loop.rs a_recurring_residual_through_the_real_diff_comparator_enters_bounded_diagnosis`
  (three REAL-comparator recurrences under the §13.5 key route into one
  bounded five-phase cycle, honestly INCONCLUSIVE with quarantined pinned
  evidence); `gauntlet-app control::controller::tests a_recurring_residual_routes_composed_diagnosis_to_correction_indicated`
  and `a_surviving_hypothesis_ends_inconclusive_with_quarantine_composed`;
  engine-level exhaustion at
  `gauntlet-app control::diagnosis::tests budget_exhaustion_yields_inconclusive_quarantine_and_escalation`.
- **Proposed correction — AK-051:**
  `gauntlet-cli tests/p6_budget_control.rs a_non_converging_task_exhausts_the_forward_budget_with_the_reserve_intact`
  — terminal `budget_exhausted` at exactly the forward rounds ceiling with
  every forward dimension under its ceiling in the durable checkpoint and
  the reserve round unspent, plus teeth T1 (both rounds-authority arms).
- **Proposed correction — AK-052:** the claim's two legs are proven at
  different locators after correction F4 wired the critic-fan-out leg:
  builder leg (composed) —
  `gauntlet-cli tests/p6_budget_control.rs a_non_converging_task_exhausts_the_forward_budget_with_the_reserve_intact`
  (rows == reservations on the meter's own counter; reserve intact) and
  `the_meter_refuses_the_reserve_consuming_slice_and_books_nothing`, plus
  teeth T4 (mint-bypass caught by the admission count); critic leg
  (composed, correction F4) —
  `gauntlet-app control::controller::tests a_meter_refused_critic_dispatch_is_truncated_and_the_run_continues`
  and
  `a_critic_reservation_releases_at_fan_in_so_a_later_iteration_still_dispatches`,
  with the review-level truncation record pinned at
  `gauntlet-app compare::review::tests a_meter_refusal_truncates_the_dimension_with_the_typed_cause`;
  port contract — the shared budget-meter contract suite
  (`gauntlet-adapter-meter-memory`, in-crate and integration) and
  `gauntlet-ports budget_meter::tests remaining_is_forward_ceiling_minus_spent`.
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (`just ruler` PASS verified by the orchestrator in this session).
  Settlement of these candidates belongs to the ruler's owner, not the task.
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
