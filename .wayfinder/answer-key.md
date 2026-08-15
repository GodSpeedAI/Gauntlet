## AK-001
claim: The domain layer has zero compile-time dependencies on ports, adapters, or infrastructure.
verify_by: static_check
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/architecture_boundaries, producer: instrument}

## AK-002
claim: The application layer depends on port abstractions only; no adapter identifier appears in domain or application source.
verify_by: static_check
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/architecture_boundaries::denylist, producer: instrument}

## AK-003
claim: Every fixed-value-set field (VerifyBy, BlastRadius, Authority, UnitStatus, Verdict, Outcome, InvocationState, TerminalState, Severity, ReviewMode, EvidenceKind) is a distinct type, not a string.
verify_by: static_check
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/typed_enums, producer: instrument}

## AK-004
claim: Parsing an unknown enum value in a Unit produces a parse error rather than a coerced default.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/answer_key_parse::unknown_enum_is_error, producer: instrument}

## AK-005
claim: Outcome is a three-state type with no boolean conversion; INCONCLUSIVE never satisfies a settlement requirement.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p12_missing_is_not_passing, producer: instrument}

## AK-006
claim: A Unit with blast_radius irreversible and authority other than human fails validation.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/answer_key_parse::tier_authority_coupling, producer: instrument}

## AK-007
claim: A Unit with blast_radius cross_cutting or irreversible cannot have authority critic, regardless of verify_by.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/answer_key_parse::tier_authority_coupling, producer: instrument}

## AK-008
claim: AgentRunner::invoke returns an admission handle without blocking on completion.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/agent_runner::invoke_is_non_blocking, producer: instrument}

## AK-009
claim: An invocation is Completed only when a schema-valid result document exists at the declared result-contract path.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/agent_runner::completion_requires_result_contract, producer: instrument}

## AK-010
claim: A child that never writes a result document is reported TimedOut, never Completed.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/agent_runner::no_result_is_timeout, producer: instrument}

## AK-011
claim: A malformed result document yields Failed{malformed_result} and no settlement-bearing state is written from it.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/agent_runner::malformed_result_rejected, producer: instrument}

## AK-012
claim: A result document whose invocation_id or context_envelope_digest does not match the dispatch yields Failed{contract_mismatch}.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/agent_runner::contract_mismatch, producer: instrument}

## AK-013
claim: The full port conformance suite passes against at least two AgentRunner adapters, one of which is not the mock.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p8_adapter_substitution, producer: instrument}
depends_on: [AK-008]

## AK-014
claim: Adapter selection resolves from configuration only; no adapter is chosen by a literal in domain or application code.
verify_by: static_check
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/adapter_registry::selection_is_config_driven, producer: instrument}

## AK-015
claim: Configuration resolves in the order defaults < GAUNTLET.md < environment < CLI flags, and the effective configuration is digested onto the run.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/config::precedence, producer: instrument}

## AK-016
claim: No path, model, provider, command, timeout, or tier policy is a hardcoded literal outside the declared defaults table.
verify_by: static_check
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/no_hardcoded_literals, producer: instrument}

## AK-017
claim: gauntlet doctor performs every preflight check in the specification and exits non-zero on any required failure.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/e2e/doctor, producer: instrument}

## AK-018
claim: Secrets are referenced by indirection, validated for presence without printing, and never appear in logs, prompts, envelopes, evidence, or generated files.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/secrets::never_leaked, producer: instrument}

## AK-019
claim: A .env.example exists listing every recognized GAUNTLET_ variable with empty values and no credentials.
verify_by: static_check
blast_radius: contained
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/env_example_complete, producer: instrument}

## AK-020
claim: The runtime starts a bounded run when optional configuration is absent, using the conservative defaults table.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/e2e/bounded_without_config, producer: instrument}

## AK-021
claim: Reference resolution normalizes deterministically: the same logical reference always produces the same digest.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/reference::normalization_is_deterministic, producer: instrument}

## AK-022
claim: The frozen reference is stored in the evidence store outside every builder-writable path.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p1_reference_integrity::frozen_copy_outside_workspace, producer: instrument}

## AK-023
claim: A builder attempt to modify the active reference fails or is detected and aborts the run with reference_mutated.
verify_by: test
blast_radius: irreversible
authority: human
evidence_required: {kind: test_report, locator: tests/proofs/p1_reference_integrity, producer: instrument}
depends_on: [AK-022]

## AK-024
claim: The reference digest is re-verified at every iteration boundary and immediately before settlement.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p1_reference_integrity::digest_rechecked, producer: instrument}

## AK-025
claim: A run may record a candidate reference revision but cannot adopt one mid-run.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/antigaming/g2_change_reference, producer: instrument}

## AK-026
claim: The comparison envelope is built from an allowlist and contains no builder trajectory, reasoning, identity, attempt count, or prior verdicts.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p2_context_isolation, producer: instrument}

## AK-027
claim: Adding a deny-listed field to the envelope makes the blindness proof fail.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/proofs/p2_context_isolation::teeth, producer: instrument}
depends_on: [AK-026]

## AK-028
claim: Every observation facet records instrument identity, exact command, environment fingerprint, exit status, and a content-addressed evidence ref.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/observer::facet_provenance, producer: instrument}

## AK-029
claim: Builder-authored claims are stored in a claims facet that is excluded from verification inputs by construction.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p3_real_observation, producer: instrument}

## AK-030
claim: Unavailable evidence and observation failures are recorded explicitly and never dropped or defaulted to success.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/antigaming/g6_hide_failed_observations, producer: instrument}

## AK-031
claim: Comparators produce residuals only; a comparator verdict alone never settles a Unit.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p11_critic_is_not_verifier, producer: instrument}

## AK-032
claim: Residual aggregation is deterministic, is not performed by a model, and any blocking residual forces an aggregate fail verdict.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/comparison::deterministic_aggregation, producer: instrument}

## AK-033
claim: Adaptive review runs deterministic probes first and fans out model critics only across dimensions those probes left undecided.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/review_mode::adaptive_skips_settled_dimensions, producer: instrument}

## AK-034
claim: Critic fan-out truncated by budget records the missing dimensions as unavailable evidence rather than omitting them.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/review_mode::truncation_is_recorded, producer: instrument}

## AK-035
claim: Verification selects the cheapest conclusive instrument and prefers deterministic verifiers over model judgment.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/verifier::deterministic_first, producer: instrument}

## AK-036
claim: A verification record is rejected as INCONCLUSIVE when its evidence is unresolvable, stale relative to the transform, or produced by a non-matching producer.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/verifier::evidence_binding, producer: instrument}

## AK-037
claim: The controller records the selected correction together with rejected alternatives and their reasons.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/controller::records_alternatives, producer: instrument}

## AK-038
claim: An injected controlled defect is exposed by observation and comparison and closed by a routed corrective iteration.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/proofs/p4_corrective_loop, producer: instrument}

## AK-039
claim: A repeated identical residual triggers bounded diagnosis rather than another patch, and exhausting the diagnostic budget yields INCONCLUSIVE.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/diagnosis::nonconvergence, producer: instrument}

## AK-040
claim: Settlement requires a PASS verification record, a resolvable pinned evidence ref, satisfied authority, no blocking residual, and an unchanged reference digest.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/settlement::preconditions, producer: instrument}

## AK-041
claim: The atomic commit sequence appends the durable event before any status write and before any in-memory advance.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p7_recovery::commit_order, producer: instrument}

## AK-042
claim: Killing the process mid-settlement and resuming completes the settlement exactly once, with no double commit and no lost evidence.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p7_recovery, producer: instrument}
depends_on: [AK-041]

## AK-043
claim: The runtime is the sole writer of Unit status and evidence fields; a builder or critic write to the answer key is prevented or detected.
verify_by: test
blast_radius: irreversible
authority: human
evidence_required: {kind: test_report, locator: tests/antigaming/g10_builder_writes_answer_key, producer: instrument}

## AK-044
claim: A unit tagged irreversible is never an automatic dispatch candidate at the scheduler level.
verify_by: test
blast_radius: irreversible
authority: human
evidence_required: {kind: test_report, locator: tests/proofs/p5_hard_gate::irreversible_never_dispatched, producer: instrument}

## AK-045
claim: A high-quality result that violates one hard requirement is rejected rather than averaged into a pass.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p5_hard_gate, producer: instrument}

## AK-046
claim: An agent role cannot lower a Unit's blast radius or authority; only an operator can raise it.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/antigaming/g7_tier_downgrade, producer: instrument}

## AK-047
claim: Sandbox policy validation precedes execution, and a validation failure escalates without falling back to unsandboxed execution.
verify_by: test
blast_radius: irreversible
authority: human
evidence_required: {kind: test_report, locator: tests/contract/sandbox::no_unsandboxed_fallback, producer: instrument}

## AK-048
claim: Containment wraps the builder session from process start; no builder command executes before the authority and containment checks.
verify_by: test
blast_radius: irreversible
authority: human
evidence_required: {kind: test_report, locator: tests/contract/sandbox::wraps_from_process_start, producer: instrument}

## AK-049
claim: A sandbox policy file for the irreversible tier causes doctor to fail.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/e2e/doctor::irreversible_policy_is_error, producer: instrument}

## AK-050
claim: A runner reporting isolation none is refused for any tier above contained.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/sandbox::capability_mismatch, producer: instrument}

## AK-051
claim: A non-converging task terminates at the declared budget limit with the recovery reserve unspent.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/proofs/p6_budget_control, producer: instrument}

## AK-052
claim: The budget meter is consulted before every dispatch and refuses any dispatch that would consume the recovery reserve.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/budget::reserve_protected, producer: instrument}

## AK-053
claim: Every iteration boundary writes a durable checkpoint sufficient to resume without any conversational context.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/state::checkpoint_sufficiency, producer: instrument}

## AK-054
claim: Run records export deterministically to JSONL; exporting the same run twice produces byte-identical output.
verify_by: test
blast_radius: contained
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/state::deterministic_export, producer: instrument}

## AK-055
claim: One successful run does not promote a capability; promotion requires the configured minimum settlements under recorded variation plus a recovery event.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/proofs/p10_no_false_learning, producer: instrument}

## AK-056
claim: An optimizer proposal that improves a metric by weakening verification fails promotion.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/proofs/p9_self_improvement_containment, producer: instrument}

## AK-057
claim: The optimizer cannot modify the active loop; its output is a candidate revision requiring isolated evaluation, held-out comparison, and valid rollback.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/promotion::candidate_only, producer: instrument}

## AK-058
claim: Each of the sixteen anti-gaming strategies has a test that fails when its defense is removed.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/antigaming, producer: instrument}

## AK-059
claim: The mock AgentRunner can replay scripted results including malformed output, timeout, and a reference-mutation attempt.
verify_by: test
blast_radius: contained
authority: critic
evidence_required: {kind: test_report, locator: tests/fixtures/mock_runner_scenarios, producer: instrument}

## AK-060
claim: The CLI exposes doctor, init, reference, run, status, inspect, verify, resume, escalations, roles, skills, improve, and export.
verify_by: test
blast_radius: contained
authority: critic
evidence_required: {kind: test_report, locator: tests/e2e/cli_surface, producer: instrument}

## AK-061
claim: The core builds and completes a full loop with operator surfaces limited to the CLI.
verify_by: build
blast_radius: local
authority: critic
evidence_required: {kind: log, locator: build/cli-only-profile, producer: instrument}

## AK-062
claim: Operator resolve and retry actions perform the same atomic commit sequence as automatic settlement and cannot settle a Unit lacking PASS evidence.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/contract/operator_actions::no_bypass, producer: instrument}

## AK-063
claim: Role definitions are repository files installed into the configured harness, and roles verify reports drift between the file and installed harness state.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/roles::install_and_verify, producer: instrument}

## AK-064
claim: The wayfinder-gauntlet skill is installed in-repo with recorded upstream provenance, preserves destination-first, fog-of-war, out-of-scope, and HITL semantics, and emits map.md plus a schema-valid answer-key.md.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/skills::wayfinder_gauntlet_conformance, producer: instrument}

## AK-065
claim: Unattended formation records unresolved questions as fog rather than inventing answers, and no fog entry appears in the answer key.
verify_by: test
blast_radius: local
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/formation::fog_not_in_answer_key, producer: instrument}

## AK-066
claim: An end-to-end run against a seeded bounded task settles with preserved evidence for every blocking Unit, using a real AgentRunner adapter.
verify_by: test
blast_radius: cross_cutting
authority: lead_agent
evidence_required: {kind: test_report, locator: tests/e2e/full_loop_real_runner, producer: instrument}
depends_on: [AK-013, AK-040]

## AK-067
claim: The final report distinguishes implemented-and-proven, implemented-but-provisional, configured-but-untested, proposed, blocked, and unknown.
verify_by: static_check
blast_radius: local
authority: critic
evidence_required: {kind: file, locator: .agents/reports/handoff.md, producer: instrument}

## AK-068
claim: Documentation exists for architecture, conformance, configuration, security, failure/recovery, operator guide, and self-improvement, and each states which parts are proven versus provisional.
verify_by: static_check
blast_radius: contained
authority: critic
evidence_required: {kind: test_report, locator: tests/contract/docs_present, producer: instrument}

## AK-069
claim: Baseline comparison A/B/C/D has been executed on at least one bounded task and its results are recorded, including the case where D does not win.
verify_by: benchmark
blast_radius: local
authority: lead_agent
evidence_required: {kind: metric, locator: .agents/reports/baselines.json, producer: instrument}

## AK-070
claim: Mutation testing at a settlement boundary shows the proof and anti-gaming suites detect deliberately introduced defects in the mechanisms they guard.
verify_by: test
blast_radius: local
authority: lead_agent
evidence_required: {kind: test_report, locator: .agents/reports/mutation.json, producer: instrument}
