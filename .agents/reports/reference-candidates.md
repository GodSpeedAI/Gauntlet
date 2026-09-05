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

## 2026-08-28 — AK-044 evidence locator names an unmaterializable test target (Task 14 closure)

- **Unit:** AK-044 (`.wayfinder/answer-key.md` lines 306–311).
- **Claim under review (unchanged):** "A unit tagged irreversible is never an
  automatic dispatch candidate at the scheduler level."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/proofs/p5_hard_gate::irreversible_never_dispatched`; `tests/proofs/`
  holds only `.gitkeep` (verified on the tree), and no core crate may declare
  a repo-root test target under the architecture boundary suite's
  cargo-target-path rule (`gauntlet-arch-tests` `escapes.rs`
  `scan_cargo_target_paths` at line 456; exemption list
  `TARGET_PATH_EXEMPT_CRATES` at line 91, exactly
  `["crates/gauntlet-arch-tests"]`, pinned by its own test at line 924).
  This is an **additional instance of the existing locator-materialization
  class (first recorded 2026-08-17, AK-049)** — not a new defect class, and
  emphatically NOT an unsupported claim: the claim is materialized, only the
  locator's spelling cannot be.
- **Proposed correction:** re-declare the locator to the composition-root
  proof whose cargo test-target NAME is already exactly `p5_hard_gate` and
  whose test function name is already exactly the locator's suffix, so the
  plan's focused gate (`cargo nextest run --test p5_hard_gate
  irreversible_never_dispatched`) runs verbatim:
  - `crates/gauntlet-cli/tests/p5_hard_gate.rs::irreversible_never_dispatched`
    (line 815; the file's header comment at lines 9–17 records the locator
    divergence in-tree): the run composes and plans over the REAL controller
    and REAL durable stores, reaches `TerminalState::Blocked` at the first
    iteration, the delegating store's DURABLE role-invocation count is ZERO
    (nothing ever reached the runner), and the escalation ticket names the
    §17.1 first line ("irreversible" + "never an automatic dispatch");
  - the dispatch-time second line at unit level:
    `crates/gauntlet-app/src/govern/gate.rs::irreversible_is_refused_before_every_other_conjunct`
    (line 742) — the conjunct is evaluated FIRST, so a forged or drifted plan
    input cannot smuggle the tier past a later conjunct's refusal;
  - the tier-posture table that makes `irreversible` total and monotone
    rather than unrepresented:
    `crates/gauntlet-app/src/govern/posture.rs::the_rung_table_is_the_spec_posture_and_monotone_with_blast_rank`
    (line 137), over `required_isolation` (line 31).
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock. Verification here is read-level only — this session ran under the
  host's single-compiler build lock, so no `cargo`/`just ruler` run backs it;
  the orchestrator's post-land gate run is the confirming instrument.
  Settlement of this candidate belongs to the ruler's owner, not the task.

## 2026-08-28 — AK-045 evidence locator names an unmaterializable test target (Task 14 closure)

- **Unit:** AK-045 (`.wayfinder/answer-key.md` lines 313–318).
- **Claim under review (unchanged):** "A high-quality result that violates one
  hard requirement is rejected rather than averaged into a pass."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/proofs/p5_hard_gate`; `tests/proofs/` holds only `.gitkeep`, and no
  repo-root target can exist under the same cargo-target-path rule as the
  AK-044 entry above. This is an **additional instance of the existing
  locator-materialization class (first recorded 2026-08-17, AK-049)** — not a
  new defect class, and not an unsupported claim.
- **Proposed correction:** re-declare the locator to the composition-root
  integration test whose cargo test-target name is already exactly
  `p5_hard_gate`, and specifically to the merit-vs-conjunct leg:
  - `crates/gauntlet-cli/tests/p5_hard_gate.rs::a_high_quality_dispatch_violating_one_hard_requirement_is_refused_not_averaged`
    (line 845): a dispatch maximal on EVERY other dimension — open status, no
    dependencies, free slots, affordable budget, a live validated backend, a
    resolvable policy — is refused outright because exactly one hard conjunct
    fails (the runner's honest §22.2 `isolation: none` below the local tier's
    §17.3 process rung). The refusal lands as `TerminalState::Blocked` with
    zero durable invocations, and the governance event names both the fact
    ("isolation none") and the rule (AK-050 / spec 17.3). The test carries its
    own CONTROL: the same runner and same unit subject at the contained tier
    (required rung `none`) DISPATCHES — so the refusal is attributable to the
    violated conjunct, never to a broken fixture;
  - the conjunctive-not-scored property at unit level:
    `crates/gauntlet-app/src/govern/gate.rs` — the gate's admission is a
    conjunction of six ordered conjuncts (header lines 41–44), each with its
    own refusal test:
    `irreversible_is_refused_before_every_other_conjunct` (742),
    `authority_above_the_ceiling_escalates_before_any_transform` (764),
    `a_dispatch_fact_below_the_recorded_facts_is_refused_as_lattice_movement`
    (839), `a_none_isolation_runner_is_refused_for_any_tier_above_contained`
    (793), `the_conservative_posture_admits_contained_critic_and_refuses_every_above_contained_tier`
    (703), and `a_validation_failure_escalates_and_never_falls_back` (874) —
    plus the admitting control
    `a_real_backend_posture_admits_above_contained_with_the_validated_policy_carried`
    (910).
- **Scope honesty:** the live leg is environment-conditional — when the
  process-isolation backend is absent from the host the test prints
  "the process-isolation backend is absent — the live leg is skipped" and
  returns green without exercising the refusal. The gate-level conjunct tests
  above are unconditional, so the claim keeps unconditional support; the
  COMPOSED spelling of it is host-dependent. Worth the ruler owner's notice
  when deciding what the locator must guarantee.
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock. Verification here is read-level only — this session ran under the
  host's single-compiler build lock, so no `cargo`/`just ruler` run backs it;
  the orchestrator's post-land gate run is the confirming instrument.
  Settlement of this candidate belongs to the ruler's owner, not the task.

## 2026-08-28 — AK-046 evidence locator names an unmaterializable test target (Task 14 closure)

- **Unit:** AK-046 (`.wayfinder/answer-key.md` lines 320–325).
- **Claim under review (unchanged):** "An agent role cannot lower a Unit's
  blast radius or authority; only an operator can raise it."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/antigaming/g7_tier_downgrade`; `tests/antigaming/` holds only
  `.gitkeep` (verified on the tree), and no repo-root target can exist under
  the same cargo-target-path rule as the AK-044 entry above. This is an
  **additional instance of the existing locator-materialization class (first
  recorded 2026-08-17, AK-049)** — not a new defect class.
- **Proposed correction:** re-declare the locator to the composition-root
  anti-gaming proof whose cargo test-target NAME is already exactly
  `g7_tier_downgrade` (`cargo nextest run --test g7_tier_downgrade` runs
  verbatim; the file's header at lines 29–33 records the divergence
  in-tree) — `crates/gauntlet-cli/tests/g7_tier_downgrade.rs`, three legs, one
  per surface of the claim:
  - `an_agent_role_cannot_move_a_units_lattice_in_any_direction` (line 561):
    the LATTICE review — an agent-originated movement is refused in every
    direction (a raise is operator-only too, the claim's second clause read
    strictly), an operator may raise and may not lower, and the refusal
    CARRIES the recorded-and-proposed values so the governance event is
    reconstructable from the refusal alone;
  - `a_dispatch_fact_below_the_recorded_facts_is_refused_as_a_downgrade`
    (line 651): the DISPATCH gate — plan facts ranking below the recorded
    answer-key facts (the forged/drifted-projection shape) are refused before
    any containment or policy conjunct is consulted, with the AK-046 rule in
    the recorded summary;
  - `a_mid_run_tier_downgrade_attempt_never_dispatches_at_the_lowered_tier`
    (line 712): the FROZEN-RULER defense, composed — a queued local-tier unit
    refuses dispatch under the honest none-isolation runner, an attacker then
    rewrites `blast_radius: local` to `contained` in the answer key between
    runs, and resume re-freeze-verifies FIRST (§18.3 step 2): the run ends
    `TerminalState::Aborted` with `ReferenceBreach::Mutated`, durable
    invocation count still zero, and the lowered tier never dispatches.
  - the pure rule the legs sit on:
    `crates/gauntlet-app/src/govern/posture.rs::review_lattice_movement`
    (line 88), with `an_agent_cannot_move_a_unit_in_any_direction` (line 164)
    and `an_operator_may_raise_and_may_not_lower` (line 202).
- **Runtime-vs-ruler note (not a defect, recorded for the owner):** the
  implementation reads the claim's second clause strictly — through the
  RUNTIME no decrease is honored from anyone, and an operator's path to
  raising a tier is editing the ruler they own, not a runtime API
  (`posture.rs` header lines 14–23). The ruler's phrasing "only an operator
  can raise it" is satisfied by that reading; a future revision may wish to
  say which surface the operator raises it through.
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock. Verification here is read-level only — this session ran under the
  host's single-compiler build lock, so no `cargo`/`just ruler` run backs it;
  the orchestrator's post-land gate run is the confirming instrument.
  Settlement of this candidate belongs to the ruler's owner, not the task.

## 2026-08-28 — AK-047 evidence locator names an unmaterializable test target (Task 14 closure)

- **Unit:** AK-047 (`.wayfinder/answer-key.md` lines 327–332).
- **Claim under review (unchanged):** "Sandbox policy validation precedes
  execution, and a validation failure escalates without falling back to
  unsandboxed execution."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/contract/sandbox::no_unsandboxed_fallback`; `tests/contract/` holds
  only the four boundary-suite stubs owned by `gauntlet-arch-tests`
  (`adapter_registry.rs`, `architecture_boundaries.rs`,
  `env_example_complete.rs`, `no_hardcoded_literals.rs`), no `sandbox.rs`
  among them, and no core crate may declare a repo-root test target under the
  same cargo-target-path rule as the AK-044 entry above. This is an
  **additional instance of the existing locator-materialization class (first
  recorded 2026-08-17, AK-049)** — not a new defect class. The case NAME the
  locator spells does exist verbatim; only its repo-root path cannot.
- **Proposed correction:** re-declare the locator to the shared contract
  suite's own case, which is named exactly `no_unsandboxed_fallback`, plus its
  meta-test and its composed leg:
  - the case:
    `crates/gauntlet-ports/src/contract/sandbox.rs::no_unsandboxed_fallback`
    (line 273; `const CASE` at 274-adjacent, dispatched from `sandbox_suite`
    at line 539). Pinned semantics (module header lines 4–11): a
    findings-carrying or rung-downgraded validation authorizes nothing, and
    `wrap` structurally requires the `PolicyValidation` that `validate`
    produced for the SAME policy — there is no path from a failed validation
    to an executed command to fall back on;
  - the suite's TEETH (proof the case can fail):
    `crates/gauntlet-ports/src/contract/suite_runs.rs::sandbox_fallback_broken_is_caught`
    (line 424) asserts the `SandboxBrokenFallback` stub
    (`crates/gauntlet-ports/src/contract/stubs.rs` line 2050) — a sandbox
    whose `wrap` accepts a findings-carrying validation, laundering a failed
    validation into an executed command — is caught, and on THIS case
    specifically; `sandbox_green_is_clean` (line 406) is the control;
  - conformance of the real adapters:
    `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs::the_shared_contract_suite_is_clean`
    (line 826) and
    `crates/adapters/gauntlet-adapter-sandbox-none/src/lib.rs::the_shared_contract_suite_is_clean`
    (line 401);
  - the composed escalation half of the claim:
    `crates/gauntlet-cli/tests/p5_hard_gate.rs::a_sandbox_validation_failure_escalates_without_falling_back`
    (line 929) — a policy that DECODES but fails the backend's own validation
    escalates carrying the findings, with no admission and no wrapped
    command; the §8.6 conservative-posture leg rides beside it (above
    contained without a backend refuses dispatch with an escalation ticket and
    zero invocations). Unit-level corroboration:
    `crates/gauntlet-app/src/govern/gate.rs::a_validation_failure_escalates_and_never_falls_back`
    (line 874).
- **Scope honesty:** the bwrap adapter's suite run and the p5 composed leg are
  environment-conditional (they skip with a printed note when the backend
  binary is absent from the host). The none-backend adapter's suite run, the
  suite-run meta-tests, and the gate-level test are unconditional, so the
  claim keeps unconditional support on every host.
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock. Verification here is read-level only — this session ran under the
  host's single-compiler build lock, so no `cargo`/`just ruler` run backs it;
  the orchestrator's post-land gate run is the confirming instrument.
  Settlement of this candidate belongs to the ruler's owner, not the task.

## 2026-08-28 — AK-048 evidence locator names an unmaterializable test target (Task 14 closure)

- **Unit:** AK-048 (`.wayfinder/answer-key.md` lines 334–339).
- **Claim under review (unchanged):** "Containment wraps the builder session
  from process start; no builder command executes before the authority and
  containment checks."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/contract/sandbox::wraps_from_process_start`; `tests/contract/` holds
  only the four boundary-suite stubs owned by `gauntlet-arch-tests`, and no
  repo-root target can exist under the same cargo-target-path rule as the
  AK-044 entry above. This is an **additional instance of the existing
  locator-materialization class (first recorded 2026-08-17, AK-049)** — not a
  new defect class; the case name exists verbatim, its repo-root path cannot.
- **Proposed correction:** re-declare the locator to the shared contract
  suite's case of exactly that name, its meta-test, and the executed
  composed leg:
  - the case:
    `crates/gauntlet-ports/src/contract/sandbox.rs::wraps_from_process_start`
    (line 360) — above the `none` rung the FIRST process of the wrapped
    command is the sandbox, never the bare probe (§17.5.4); the case's helper
    `first_word_names_backend` (line 112) is what makes "from process start"
    an observed property of the rendered command rather than a declared one;
  - the TEETH:
    `crates/gauntlet-ports/src/contract/suite_runs.rs::sandbox_process_start_broken_is_caught`
    (line 437) asserts the `SandboxBrokenProcessStart` stub
    (`stubs.rs` line 2139) — a sandbox that CLAIMS the process rung but
    performs an identity wrap, so the bare probe would be the first process —
    is caught, and on THIS case specifically;
  - adapter conformance: the two `the_shared_contract_suite_is_clean` tests
    cited in the AK-047 entry above
    (`gauntlet-adapter-sandbox-bwrap/src/lib.rs` line 826;
    `gauntlet-adapter-sandbox-none/src/lib.rs` line 401);
  - the composed and EXECUTED leg:
    `crates/gauntlet-cli/tests/p5_hard_gate.rs::containment_wraps_from_process_start_and_confines_the_session`
    (line 1022) — the gate's admission carries the resolved policy and its
    CLEAN validation (`validation.is_clean()`, `isolation == Process`), the
    wrapped command's first process is the backend, and RUNNING the wrapped
    command under the real backend confines the session: a write inside the
    writable root succeeds, a write outside fails on the read-only root, and
    the denied path is unreadable. This leg composes the repository's own
    SHIPPED policy artifacts (`.gauntlet/policies/contained.json`,
    `local.json`, `cross_cutting.json` — new in Task 14), which is the
    difference between mechanism existence and composed reachability.
- **Scope honesty:** the executed p5 leg and the bwrap suite run are
  environment-conditional (skipped with a printed note when the backend binary
  is absent). The contract case, its broken-stub meta-test, and the
  none-backend adapter's suite run are unconditional. The "no builder command
  executes before the checks" half is structural — `wrap` cannot be reached
  without the `PolicyValidation` that `validate` minted for the same policy —
  and the gate runs at DISPATCH time before the `RoleInvocationSpec` is built
  (`crates/gauntlet-app/src/govern/gate.rs` header lines 6–12; defect D2's
  correction that a post-turn check is an audit, not a gate).
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock. Verification here is read-level only — this session ran under the
  host's single-compiler build lock, so no `cargo`/`just ruler` run backs it;
  the orchestrator's post-land gate run is the confirming instrument.
  Settlement of this candidate belongs to the ruler's owner, not the task.

## 2026-08-28 — AK-050 evidence locator names an unmaterializable test target (Task 14 closure)

- **Unit:** AK-050 (`.wayfinder/answer-key.md` lines 348–353).
- **Claim under review (unchanged):** "A runner reporting isolation none is
  refused for any tier above contained."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/contract/sandbox::capability_mismatch`; `tests/contract/` holds only
  the four boundary-suite stubs owned by `gauntlet-arch-tests`, and no
  repo-root target can exist under the same cargo-target-path rule as the
  AK-044 entry above. This is an **additional instance of the existing
  locator-materialization class (first recorded 2026-08-17, AK-049)** — not a
  new defect class; the case name exists verbatim, its repo-root path cannot.
- **Proposed correction:** re-declare the locator to the three surfaces that
  materialize the claim, all verified on the tree:
  - the sandbox-side case of exactly that name:
    `crates/gauntlet-ports/src/contract/sandbox.rs::capability_mismatch`
    (line 473, `const CASE: &str = "capability_mismatch"` at line 474) — a
    backend achieving the `none` rung validates no tier above contained, and
    the irreversible tier has no policy at all, the sandbox being the last
    line after the provider's structural refusal (§17.3);
  - the RUNNER-side conjunct, which is where the claim's own wording lives
    (the runner's REPORT is what is refused, and §22.2's honesty rule is what
    makes the mock's `none` refusable):
    `crates/gauntlet-app/src/govern/gate.rs::a_none_isolation_runner_is_refused_for_any_tier_above_contained`
    (line 793), over `posture::required_isolation`
    (`crates/gauntlet-app/src/govern/posture.rs` line 31) and its table test
    at line 137; the refusal surfaces on the composed path as the
    `capability_mismatch` summary token
    (`crates/gauntlet-app/src/control/controller.rs` line 3572);
  - the composed spellings:
    `crates/gauntlet-cli/tests/p5_hard_gate.rs::a_high_quality_dispatch_violating_one_hard_requirement_is_refused_not_averaged`
    (line 845; the governance event names "isolation none" and cites AK-050 or
    spec 17.3, with the contained-tier control dispatching) and
    `crates/gauntlet-cli/tests/g7_tier_downgrade.rs::a_mid_run_tier_downgrade_attempt_never_dispatches_at_the_lowered_tier`
    (line 712, iteration one: the local-tier unit refuses dispatch under the
    honest none-isolation runner);
  - the operator-facing preflight of the same rule: doctor check 4,
    `crates/gauntlet-app/src/doctor/checks.rs::check_capabilities` (line 756;
    rewritten in Task 14 from its "lands with plan Task 14" placeholder),
    emitting `sandbox_capability_mismatch` when the measured backend rung sits
    below a dispatchable tier's required rung (lines 787–826), asserted at
    `crates/gauntlet-app/src/doctor/mod.rs` line 1400.
- **Scope honesty (a real gap in the suite's teeth, not in the claim):** the
  sandbox suite's meta-test file carries broken-stub negatives for three of
  its five cases only — `sandbox_broken_is_caught`
  (`wrap_demands_its_own_validation`, line 411),
  `sandbox_fallback_broken_is_caught` (line 424) and
  `sandbox_process_start_broken_is_caught` (line 437) — and `stubs.rs`
  declares `SandboxGreen`, `SandboxBroken`, `SandboxBrokenFallback` and
  `SandboxBrokenProcessStart` but NO capability-mismatch breaker. So the
  `capability_mismatch` case is run (green stub, both real adapters) but is
  not itself proven able to fail, unlike its two siblings. The CLAIM is
  independently supported by the gate conjunct, the two composed legs, and
  doctor check 4 — this is a suite-teeth observation for the owner and for a
  future task, not a reason to doubt AK-050.
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock. Verification here is read-level only — this session ran under the
  host's single-compiler build lock, so no `cargo`/`just ruler` run backs it;
  the orchestrator's post-land gate run is the confirming instrument.
  Settlement of this candidate belongs to the ruler's owner, not the task.

## 2026-08-28 — AK-049 addendum: Task 14 did NOT change doctor check 5; the locator entry of 2026-08-17 stands unamended

- **Unit:** AK-049 (`.wayfinder/answer-key.md` lines 341–346); addendum to the
  2026-08-17 entry at the head of this file, whose proposed locators stand
  unchanged.
- **Why an addendum at all:** Task 14 is the sandbox/tier-policy task, so a
  reader would reasonably expect it to have moved AK-049. It did not, and
  saying so plainly is the point of this entry. AK-049's claim has been
  satisfied since Task 5 by doctor check 5 (the forbidden-tier scan), and the
  three tests the 2026-08-17 entry proposes as its locators are unchanged by
  Task 14.
- **What was verified (read-level, on the landed tree):** the check-5
  implementation `check_policies`
  (`crates/gauntlet-app/src/doctor/checks.rs` line 881, with the AK-049 scan
  and its any-casing/any-extension/broken-symlink stem rule at lines 1005–1034)
  falls entirely OUTSIDE Task 14's diff to that file — the working-tree diff
  hunks land at the module head and at lines 716–872 (`check_runners` and the
  `check_capabilities` rewrite, i.e. check 4), not in the check-5 region. The
  three proposed locators still exist under their 2026-08-17 names in
  `crates/gauntlet-app/src/doctor/mod.rs`:
  `a_present_top_tier_policy_fails_check_five_and_the_run` (line 518, still
  asserting `AK-049` in both the text and the JSON report),
  `forbidden_tier_policies_under_any_spelling_or_link_fail_check_five`
  (line 688), and `an_unreadable_policy_dir_fails_check_five_and_recovers_when_readable`
  (line 718).
- **The one operational change worth recording (no locator change):** Task 14
  ships real tier-policy artifacts into the default policy dir —
  `.gauntlet/policies/contained.json`, `local.json`, `cross_cutting.json`
  (verified present; each names its tier, backend, network posture, writable
  roots and denied paths). Check 5's negative clause therefore now runs
  against a POPULATED policy dir rather than an empty one: the dir holds three
  dispatchable-tier files and still no file whose stem is `irreversible` in
  any casing, which is exactly the state check 5 requires. This strengthens
  the claim's operational bite (the check now discriminates rather than
  scanning an empty dir); it does not alter the check, its tests, or the
  locator correction proposed on 2026-08-17.
- **Not a new candidate:** no new divergence is recorded here, and the
  2026-08-17 entry is neither superseded nor edited.
- **Not changed by this entry:** the answer key, the spec, the frozen
  reference, and the frozen lock. Verification here is read-level only — this
  session ran under the host's single-compiler build lock, so no
  `cargo`/`just ruler`/`just doctor` run backs it; the orchestrator's
  post-land gate run is the confirming instrument. Settlement of the
  2026-08-17 candidate remains with the ruler's owner.

## 2026-08-29 — AK-013's evidence locator names a repo-root path where the repository's proofs live under a crate (Task 16)

- **Unit:** AK-013 (`.wayfinder/answer-key.md` lines 85–92).
- **Claim under review (unchanged, delivered this task):** "The full port
  conformance suite passes against at least two AgentRunner adapters, one
  of which is not the mock."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/proofs/p8_adapter_substitution` with `kind: test_report`. The
  repository's real proof tests live in `crates/gauntlet-cli/tests/`
  (p4 corrective loop, p5 hard gate, p6 budget control, p7 recovery, p11
  critic-is-not-verifier, p12 missing-is-not-passing, g6/g7/g10) — the
  repo-root `tests/proofs/` directory is an empty `.gitkeep` placeholder.
  The delivered evidence is
  `crates/gauntlet-cli/tests/p8_adapter_substitution.rs::the_full_conformance_suite_passes_against_every_selected_adapter_unchanged`,
  which runs the UNMODIFIED shared `agent_runner_suite` PLUS the
  task-16-specific `exit_without_result_suite` over the mock and cli
  factories through ONE loop body (zero per-adapter test bodies), plus
  `GAUNTLET_P8_ADAPTER` selection with fail-closed behavior on unknown
  values. Same locator-class divergence as AK-049's 2026-08-17 entry:
  the locator names a path convention the repository did not adopt.
- **Proposed resolution (ruler's owner):** re-point AK-013's locator to
  `crates/gauntlet-cli/tests/p8_adapter_substitution` (or declare the
  `tests/proofs/` convention authoritative and relocate the proof — the
  crate location is the established precedent across eleven proof files).
- **Composition note (for whoever settles this):** the "full suite" the
  delivered proof runs is `agent_runner_suite` + the exported sibling
  `exit_without_result_suite` (design adjudication RES-D1, this task):
  the shape-2 clean-exit rule is plan-Task-16-specific (spec §10.6) and
  is deliberately NOT a member of the universal §7.3 suite, because the
  settled prime adapter's Annex-C.4 one-input poll cannot observe
  process exit. Both suites run over BOTH adapters inside the single
  p8 test body.
- **Not changed by this entry:** the answer key, the spec, the frozen
  reference, and the frozen lock (`just ruler` PASS before and after the
  task's phases, most recently on the corrected tree).

## 2026-08-30 — AK-063 evidence locator names a repo-root test path that cannot host the mechanism test without violating the dependency architecture (Task 17, prereg D5)

- **Unit:** AK-063 (`.wayfinder/answer-key.md` lines 439–444).
- **Claim under review (unchanged, delivered this task):** "Role definitions are
  repository files installed into the configured harness, and roles verify
  reports drift between the file and installed harness state."
- **Defect (evidence declaration only):** `evidence_required.locator` is
  `tests/contract/roles::install_and_verify`. The repository's repo-root
  `tests/contract/<name>.rs` files are `[[test]]` cargo targets of
  `gauntlet-arch-tests` (its Cargo.toml registers `architecture_boundaries`,
  `adapter_registry`, `no_hardcoded_literals`, `env_example_complete` with
  repo-root paths, per the frozen answer key's address convention for the
  architecture-boundary work). That crate is test-only and depends on **no
  internal crate** — a rule the dependency-direction suite (`just boundaries`)
  enforces. A roles test needs `gauntlet_app`'s public API, so materializing
  `tests/contract/roles.rs` as spelled would require `gauntlet-arch-tests →
  gauntlet-app`, violating the declared layering. The delivered evidence is
  `crates/gauntlet-app/tests/roles.rs::roles::install_and_verify` — the crate
  that owns the mechanism, the repository's established crate-scoped layout
  for mechanism tests (AK-013's 2026-08-29 entry is the nearest precedent).
  This is an **additional instance of the existing locator-materialization
  class (first recorded 2026-08-17, AK-049)** — not a new defect class — with
  the additional architectural reason the spelled path cannot host this test
  at all (not merely an empty directory).
- **Delivered surface (wider than the locator's single test):** the same file
  pins the full Task-17 tooth families — over-grant/deny-shadow refusals,
  unknown-key and permission-vocabulary refusals, $VAR indirection classes,
  install idempotence (byte-identical lock), drift in all three directions
  (installed/source/extra), naive hand-edit tamper, full lock re-forgery with
  recomputed digests, canonical-closure trap, advisory fallback honesty,
  fail-closed harness ids, and verify-green-immediately-after-install. The
  blind executing falsifier round 1 executes further attacks against the same
  surface (its report is the round's evidence).
- **Proposed resolution (ruler's owner):** re-point AK-063's locator to
  `crates/gauntlet-app/tests/roles` (test id `roles::install_and_verify`), or
  declare the repo-root `tests/contract/` convention authoritative and
  relocate the test — noting relocation requires a dependency-direction
  exception for `gauntlet-arch-tests`, which is a spec-level change, not a
  test move.
- **Not changed by this entry:** the answer key, the spec, the frozen
  reference, and the frozen lock (`just ruler` PASS at round start; the
  post-settlement gate run is the confirming evidence). Settlement of this
  candidate belongs to the ruler's owner, not the task.


## 2026-08-31 — AK-060's locator `tests/e2e/cli_surface` materialized in the owning crate (Task 20, the AK-063/AK-065 class)

- **Unit:** AK-060 (CLI exposes the full §20.1 surface; frozen locator
  `tests/e2e/cli_surface`).
- **Defect (evidence declaration only, claim satisfied):** the locator names a
  repo-root `tests/e2e/` target. The Cargo-target-path rule
  (`gauntlet-arch-tests` `escapes.rs` `scan_cargo_target_paths`, exemption list
  exactly `["crates/gauntlet-arch-tests"]`) forbids any OTHER crate from
  declaring a repo-level test target, and the exemption's justification — the
  arch suite "is not part of the product, depends on no internal crate … is
  outside `walk::CORE_CRATES`" — cannot transfer to `gauntlet-cli`, which is a
  core product crate: exempting it would let a relocated lib/bin root escape
  the scanners. Additionally, hosting the snapshot elsewhere would lose
  `CARGO_BIN_EXE_gauntlet`, the only seam that drives the REAL binary.
- **Delivered evidence:** `crates/gauntlet-cli/tests/cli_surface.rs` — the
  whole-tree `--help` snapshot against the committed golden
  (`crates/gauntlet-cli/tests/cli_help.txt`, single-segment beside the test
  after the boundaries gate's literal rules; update-only-with-a-recorded-change
  procedure; the test file's doc comment still spells the pre-move
  `golden/cli_help.txt` path — a doc nit, the `include_str!` is the binding
  reference),
  whole-tree dispatch + unknown-command smokes, per-command unknown-arg /
  missing-arg exit-2 smokes, and the usage/dispatch/report-manifest agreement
  pin.
- **This is an additional instance of the existing locator-materialization
  class (first recorded 2026-08-17, AK-049)** — not a new defect class — with
  the additional architectural reason the spelled path cannot host this test
  without weakening a product crate's scan guarantee.
- **Proposed resolution (ruler's owner):** re-point AK-060's locator to
  `crates/gauntlet-cli/tests/cli_surface`, or declare a repo-root
  `tests/e2e/` host crate whose sole justification is locator materialization
  (a spec-level decision, not a test move).
- **Not changed by this entry:** the answer key, the spec, the frozen
  reference, and the frozen lock (`just ruler` PASS at slice start and re-run
  at slice close).

## 2026-08-31 — Observation-binding GRANULARITY: §12/§14 key the bundle per (run, iteration) while §11.2 dispatches up to `max_parallel` completions into ONE iteration (Task 20, COR-D2)

- **Unit:** AK-061 (core builds and completes a full loop), exercised at the
  §11.2/§12 seam.
- **Tension (ruler-owner question, NOT implemented):** the observation binding
  is write-once per (run, iteration) — §12/§14's bundle-to-iteration key —
  while §11.2 plans up to `max_parallel` dispatches into ONE iteration. With
  `max_parallel >= 2` and two eligible units, two completions arrive in the
  same iteration and only one can bind; the store's integrity rule turns the
  second into a typed refusal that, pre-correction, escaped the drain as a
  raw run error — not one of §11.6's bounded terminals. The shipped §8.6
  default is `max_parallel: 2`, so the composed surface could not run its own
  default parallelism over a multi-unit reference.
- **Implemented loop-locally (COR-D2, adjudication):** at most one observation
  binding per (run, iteration); surplus completions of one drain CARRY to the
  following iteration and bind at that iteration's own anchor (checkpoint
  first, §18.2 order preserved; reservations and workspaces held until their
  pipeline runs; carries pinned as evidence; §16.3 recovery semantics:
  checkpoint lists carried invocations on the §18.3 re-probe surface, a
  resume re-plans their units, nothing double-binds).
- **The question left to the ruler's owner:** is the binding's
  granularity right? Two candidate revisions: (a) re-key the observation
  binding per INVOCATION (the bundle is already per-completion; the
  iteration remains the comparison/production anchor), or (b) declare
  §11.2 parallelism as across-iterations only (one completion per
  iteration, which is what the loop now enforces anyway). Choosing (a) is
  a ports+spec change via candidate revision — deliberately not done in a
  correction.
- **Not changed by this entry:** the answer key, the spec, the frozen
  reference, and the frozen lock (`just ruler` PASS at slice start;
  re-run at slice close).

## 2026-08-31 — `evidence_required.locator` CONTENT semantics are composition-bound (Task 20, RES-11 record + COR-D3 consequence)

- **Unit:** AK-061 (§14.2/§14.3 evidence binding in the composed loop).
- **Record (ruler-owner question):** the CONTENT a production binds at
  `evidence_required.locator` depends on the composition's produce seam.
  Two worlds ship: the controller's default
  (`ProducePolicy::MaterializeObservation`, the frozen p4/p7 facet-document
  proof rungs decide through it) materializes the first observation facet's
  document AT the locator; the composed CLI world
  (`ProducePolicy::BindLocatorContent`) ingests the locator's ACTUAL bytes —
  the builder-produced artifact — and materializes the facet document only
  at absence. Neither world lets a claim settle on claim bytes alone; a
  mismatched composition fails the §14.3 three-way equality loudly
  (fail-closed); the production's recorded instrument is the observation
  instrument, honestly recorded. The BindLocatorContent world additionally
  means the rung contract (not the ingestion) is what discriminates: it
  ingests builder-produced bytes at the locator verbatim.
- **COR-D3 consequence (recorded here for one decision):** with the composed
  workspace provider wired, builder execution anchors INSIDE the admitted
  per-unit workspace (§17.5.1 invariant 1), so locator reads and writes for
  a completion anchor there too — while a locator is workspace-RELATIVE, so
  the durable production record does not name the directory it resolved
  under. The operator `verify` surface re-derives bindings under the
  canonical root; where the artifact stood only inside a RELEASED per-unit
  workspace, the §22.2 re-read honestly reports absence and the re-decided
  outcome is INCONCLUSIVE, never PASS (missing evidence is not passing
  evidence). Should §14.3 pin ONE world (e.g., require production records to
  carry the anchored workspace path, or require locators to resolve under
  the canonical root for every composition)? That is the ruler owner's
  adjudication; a ports revision (workspace on the Verifier request) was
  recorded in-task as the race-free closure and is deliberately not chosen
  by a correction.
- **Not changed by this entry:** the answer key, the spec, the frozen
  reference, and the frozen lock.

## 2026-09-03 — AK-055 evidence locator names an unmaterializable test target (Task 22)

- **Unit:** AK-055 (`.wayfinder/answer-key.md` lines 383–388): "One
  successful run does not promote a capability; promotion requires the
  configured minimum settlements under recorded variation plus a recovery
  event." (`verify_by: test`, `authority: critic`,
  `evidence_required: {kind: test_report, locator:
  tests/proofs/p10_no_false_learning, producer: instrument}`.)
- **Defect (evidence declaration only):** the locator
  `tests/proofs/p10_no_false_learning` matches no file or cargo target —
  `tests/proofs/` does not exist in the repository, and the architecture
  boundary suite's Cargo-target-path rule forbids a core crate from
  declaring a repo-level test target. Identical class to the recorded
  AK-060/061/062/063 divergences.
- **Materialization:** the proof lives at
  `crates/gauntlet-cli/tests/p10_no_false_learning.rs` (the composed-proof
  convention: integration tests of the composition root, plan gate
  `cargo nextest run --test p10_no_false_learning`), which proves the
  claim's content: single success stays `observed`; ≥N settlements under
  identical content axes are refused (`insufficient_variation` recorded,
  never promoted); the §19.2 positive path is the only road to `promoted`
  with a durable decision record; revoke is sticky.
- **Not changed by this entry:** the answer key, the spec, and the frozen
  lock (digests still match; `just ruler` PASS at slice start; re-run at
  slice close). Settlement of this candidate belongs to the ruler's
  owner, not the builder.

## 2026-09-03 — CandidateRevision question against §19.1/I6: may the N settlements backing a promotion come from a SINGLE run? (Task 22 falsifier round 2, F2-3 — orchestrator adjudication, operator's call)

**The tension (both texts verbatim):** spec §19.1: "Tool access is not
capability. One successful run is not capability. Capability is repeated
reliable settlement under relevant variation, with recovery and
decreasing orchestration burden." Plan Task 22 / charter D4
commissioned the promotion gate's variation condition mechanically:
"≥ N settlements … must differ on at least one CONTENT axis" — with
axes typed as durable content facts (unit, reference digest, evidence
kind, locator class) and run ids/timestamps EXPLICITLY excluded (they
always differ; excluding them is what makes the AK-055 run-id trap
unpassable).

**The executed case (falsifier round 2, NH-5b, real adapters):** three
settlement-class observations from a SINGLE run over three DISTINCT
units (plus a rooted recovery and passing attempts) promote —
witnesses=3, content-only axes, every commissioned refusal silent. This
is charter-conformant: the variation is real content variation, and the
plan's Done-when teeth ("three settlements with *identical* variation
axes also do not promote") are not implicated. But under a run-counted
reading of §19.1's sentence, one successful run should never promote a
capability, whatever its internal variation — the N settlements should
span ≥ 2 distinct runs.

**Disposition:** recorded, NOT patched. The charter is this task's
commission and was implemented as commissioned; tightening the gate to a
run-counted minimum is a product decision about §19.2's meaning (a
CandidateRevision against the spec), the ruler owner's call between
runs. Until adjudicated: the mechanism refuses all-identical content
axes (proven) and promotes genuinely varied single-run windows (proven);
a consumer needing the stricter reading (task23's optimizer is the
likely first) should either wait for the operator's adjudication or
record the stricter requirement at ITS call site. Trigger: task23
promotion consumption, or the next reference revision. Falsifier
evidence: `.agents/reports/task22/falsifier-round2.md` (NH-5b, F2-3).

- **Not changed by this entry:** the answer key, the spec, and the
  frozen lock (`just ruler` PASS at entry time; the entry is a recorded
  question for the ruler's owner, not the builder).

## 2026-09-03 — AK-056 evidence locator names a repo-root test path (Task 23; the AK-049 class)

- **Unit:** AK-056 (`.wayfinder/answer-key.md`; `verify_by: test`,
  `evidence_required.locator: tests/proofs/p9_self_improvement_containment`).
- **Defect (evidence declaration only; the claim itself is satisfied):** the
  locator `tests/proofs/p9_self_improvement_containment` names a repo-root
  test directory. The architecture boundary suite's Cargo-target-path rule
  (`gauntlet-arch-tests` `escapes.rs` `scan_cargo_target_paths`, exemption
  list exactly `["crates/gauntlet-arch-tests"]`) forbids a core crate from
  declaring a repo-level test target, so the locator cannot materialize as
  spelled — the AK-049 class (the p4/p7/p10 precedent).
- **Materialization (recorded, not a ruler edit):** the proof lives in the
  crate that owns the mechanism, with the target name verbatim:
  `crates/gauntlet-cli/tests/p9_self_improvement_containment.rs` (the
  composed-loop proof for the §19.3/§19.4 self-improvement gate). The
  sibling unit AK-057's locator `tests/contract/promotion::candidate_only`
  materializes as the `candidate_only` test of the `promotion` test
  target (`crates/gauntlet-app/tests/promotion.rs`, declared test
  target of the crate that owns the promotion services — the
  `tests/contract/architecture_boundaries` precedent). Invocation note
  (the task17/task18 recorded class): nextest ids are space-joined
  between binary and test, so the literal substring filter
  `promotion::candidate_only` matches zero tests; the correct focused
  invocation is `cargo nextest run -p gauntlet-app --test promotion
  candidate_only` (orchestrator-verified 2026-09-03).
- **Ruler untouched:** `.wayfinder/answer-key.md`,
  `.gauntlet/reference.lock`, and `.wayfinder/reference.yaml` are
  unmodified; this entry is the record.

## 2026-09-04 — Loop-economics: NO candidate key can improve a per-settlement metric between two COMPLETE arms on the current loop (Task 23, COR-1)

- **Unit:** AK-056 (and §19.4's positive road: "target metric improves by
  the declared margin" proven between two complete arms over the REAL
  composed loop).
- **Tension (ruler-owner question, NOT implemented):** the §19.4
  self-improvement gate's promotion path requires two comparable,
  COMPLETE evaluation arms whose per-settlement figures genuinely differ
  via a candidate's INERT change. On the loop as bound, that difference
  is unreachable for ANY key — inert or otherwise:
  - The COR-D2 observation binding is write-once per (run, iteration),
    and the controller's carry absorbs surplus completions one per
    iteration, so N units settle in exactly N iterations at ANY
    `budget.max_parallel` — iterations_per_settlement is pinned at
    N/N (fixed-point 10 000) for every complete arm.
  - The mock loop books ZERO payment, so payment_per_settlement is
    scale-invariant (identically 0) between arms; a real payment
    difference cannot be composed on the mock.
  - Every cost lever that would genuinely move per-settlement economics
    (verification strength, review mode/critic count, observation
    facets) is verification-bearing and is rightly refused by gate
    condition 3 — the refusal is correct; the point is that nothing
    inert remains.
  - The only inert keys are ceilings; a binding ceiling produces an
    INCOMPLETE arm, which COR-1's C1 completion gate rightly adjudicates
    Provisional (never a promotion, never a rejection).
- **Empirical finding (two independent probes over the real binary, real
  composed loop, real sqlite ledger):** baseline `max_parallel: 1` vs
  treatment delta `max_parallel: 2` → REJECTED, `control 10000 vs
  treatment 10000`, all four runs' checkpoints byte-identical; a
  `review.mode` downgrade over complete arms → REJECTED with
  `control 10000 vs treatment 10000` beside AK-056's
  `settlement_access_weakened`.
- **Consequence for the Task 23 proof (recorded deviation from charter
  D7's all-real-loop composition, coordinator decision 2026-09-04):**
  p9's promotion legs (2/3) prove the positive path at the GATE level —
  REAL harness, REAL embedded sqlite revision+capability+run-state
  stores, REAL gate and rollback — with the executor seam authored over
  REAL StateStore EARNED authorities (the p10 substrate-seed class) and
  the payment economy honestly modeled as runner-cost with proof-authored
  per-invocation costs (the mock books nothing to compose a real one
  from). Leg 1 stays on the real loop in its honest CONTAINMENT form;
  the strongest form (genuine improvement AND rejection on the real
  loop) awaits the resolution below. See the p9
  `p9_self_improvement_containment.rs` header for the leg-by-leg
  declaration.
- **What would close it (ruler owner):** the binding-granularity
  candidate revisions in the 2026-08-31 entry above — re-key the
  observation bundle per INVOCATION (multi-settlement iterations), or
  otherwise let one booked iteration carry >1 settlement — plus a
  runner-cost surface the mock loop actually books. Until then the
  §19.4 positive path is loop-unreachable BY CONSTRUCTION, not by gate
  weakness.
- **Ruler untouched:** `.wayfinder/answer-key.md`,
  `.gauntlet/reference.lock`, and `.wayfinder/reference.yaml` are
  unmodified; this entry is the record.

## 2026-09-04 — No inert candidate key can improve per-settlement economics on the current loop (Task 23 COR-1, the C2 finding)

- **Unit:** AK-056/AK-057 (§19.4 self-improvement gate), exercised at the
  loop's economics seam.
- **Finding (empirical, real binary + real composed loop):** NO inert
  candidate key can improve per-settlement economics between two COMPLETE
  arms. The COR-D2 observation binding is write-once per (run, iteration)
  and carried completions absorb ONE per iteration, so N units settle in
  exactly N iterations at ANY `budget.max_parallel` — serial dispatch
  pipelines perfectly at that cap, and higher parallelism only ties
  (probe: baseline `max_parallel: 1` vs candidate delta `max_parallel: 2`
  over complete arms → `control 10000 vs treatment 10000`, all four runs'
  checkpoints byte-identical: 3 iterations / 3 rounds / zero payment).
  Every other inert key is a CEILING (binding ⇒ INCOMPLETE arm, which the
  §19.4 verdict contract rightly rests at Provisional), and the loop's
  only cost levers are verification-bearing keys the gate's condition 3
  rightly refuses. The §19.4 positive path is therefore structurally
  unreachable on the current loop.
- **Demonstrated instead (task23 p9 legs 2/3, COR-1):** the positive path
  is proven at the GATE level — REAL `EvaluationHarness`, REAL embedded
  sqlite revision+capability+state stores, REAL `PromotionGate`, the
  executor seam authored over the REAL StateStore EARNED authorities (the
  p10 composition pattern: durable run identity with `parent_run` on the
  treatment runs, per-iteration boundary checkpoints,
  verification-record authority mints, settlement-commit authority
  admits + §16.3 commits — never hand-built rows), with the
  per-invocation payment economy authored the same way (the mock loop
  itself charges nothing). Inert improvement carrier: `runners.default`
  to a runner whose authored per-settlement cost is genuinely lower —
  control 30 000 000 vs treatment 20 000 000 per-10 000 payment figure,
  both derived from durable checkpoints.
- **The ruler-owner's path:** loop-level promotion demonstration awaits
  the observation-binding re-key recorded above (2026-08-31 entry,
  candidate revision (a): re-key the observation binding per INVOCATION).
  Leg 1 meanwhile proves the CONTAINMENT form over the real loop: the
  verification-weakening candidate is REJECTED with
  `settlement_access_weakened` durably recorded, both arms complete.
- **Not changed by this entry:** the answer key, the spec, the frozen
  reference, and the frozen lock.

## 2026-09-04 — AK-058's locator `tests/antigaming` materializes as the cargo target `antigaming`; the task-24 plan-gate spellings diverge from the real tools (Task 24; the AK-049 class + the task-18 nextest class)

- **Unit:** AK-058 (`.wayfinder/answer-key.md` lines 404–409).
- **Claim under review (unchanged):** "Each of the sixteen anti-gaming
  strategies has a test that fails when its defense is removed."
- **Defect (evidence declaration only):** `evidence_required.locator`
  is `tests/antigaming`; the repo-root directory holds only `.gitkeep`,
  and no repo-root test target can exist under the same
  cargo-target-path rule as the AK-049/AK-025/AK-030/AK-046 entries.
  **Additional instance of the existing locator-materialization class
  (first recorded 2026-08-17, AK-049)** — not a new defect class.
- **Materialization:** the composition-root integration test target
  `antigaming` — `crates/gauntlet-cli/tests/antigaming.rs`, 19 tests
  (one per strategy G1–G16, a two-leg G16 split across two fns' legs,
  the E-2 order-independence regression, and the manifest validator).
  The machine-readable teeth map is
  `.agents/reports/defended-mechanisms.txt`
  (test_id | G-ID | mechanism | source_location), enforced by the
  in-suite validator; `scripts/run-mutation.sh` scopes cargo-mutants
  to the manifest's loci and produces `.agents/reports/mutation.json`.
- **Plan-gate spelling divergences (both recorded classes, neither a
  silent weakening):**
  1. `cargo mutants --file-list .agents/reports/defended-mechanisms.txt`
     — cargo-mutants 27.1.0 has **no `--file-list` option**; the real
     surface is repeatable `-f` file globs plus `-F` span / `-E`
     justified-exclusion regexes, driven by `scripts/run-mutation.sh`
     from the manifest's loci. `--no-default-features` is a first-class
     cargo-mutants flag (before `--`), not a cargo-test arg.
  2. `cargo nextest run --test-threads 4 antigaming::` — nextest parses
     a `::`-containing positional as the filter DSL's PACKAGE selector;
     it matches zero tests and exits 4 (fails closed, never passes
     vacuously — same mechanism class as the task-18 "space-joined
     nextest filters" note). The working suite gate is
     `cargo nextest run --test-threads 4 --test antigaming`.
- **Mutation evidence:** `.agents/reports/mutation.json` (regenerated
  by `scripts/run-mutation.sh` on the final tree at closure;
  `missed == 0` asserted by the plan's python check). AK-070's
  locator `.agents/reports/mutation.json` is materialized verbatim.
- **Not changed by this entry:** the answer key, the spec, the frozen
  reference, and the frozen lock.
