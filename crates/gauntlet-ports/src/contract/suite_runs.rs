//! The suite-run teeth tests (plan Task 4, gate G2).
//!
//! Two families of tests, one per port:
//!
//! - `*_green_is_clean`: the reference in-memory stub of the port runs the
//!   shared suite and MUST report no findings. Green is the floor every
//!   adapter must clear — a suite that flags its own reference stub is
//!   itself broken.
//! - `*_broken_is_caught`: a deliberately defective stub runs the SAME
//!   suite and MUST produce at least one finding — and specifically the
//!   case the defect targets. "A suite no adapter can fail is not a suite."
//!
//! Every broken stub violates exactly one pinned contract rule; the
//! intended case is asserted by name so a suite that merely errors out, or
//! reports a different violation, is caught as a regression. The green and
//! broken runs share the suite function — there is no bespoke assertion
//! path.
//!
//! These tests are the proof that the suites discriminate: green passes,
//! broken fails, both through the same door.

use super::ContractFinding;
use super::agent_runner::ProbeBudget;
use super::stubs::*;

fn assert_green(label: &'static str, findings: Vec<ContractFinding>) {
    assert!(
        findings.is_empty(),
        "{label} is the reference stub and must run its suite clean, but reported:\n{}",
        findings
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// A broken stub must trip the suite — and trip the intended case, not an
/// incidental one.
fn assert_caught(label: &'static str, intended: &'static str, findings: Vec<ContractFinding>) {
    assert!(
        !findings.is_empty(),
        "{label} is deliberately broken but ran its suite clean — the suite \
         cannot fail this adapter; a suite no adapter can fail is not a suite"
    );
    assert!(
        findings.iter().any(|f| f.is_case(intended)),
        "{label} was caught, but not on the intended case {intended}; reported:\n{}",
        findings
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn agent_runner_green_is_clean() {
    assert_green(
        "AgentRunnerGreen",
        super::agent_runner::agent_runner_suite(&AgentRunnerGreen, ProbeBudget::INSTANT),
    );
}

#[test]
fn agent_runner_broken_is_caught() {
    assert_caught(
        "AgentRunnerBroken",
        "admission_returns_before_completion",
        super::agent_runner::agent_runner_suite(&AgentRunnerBroken, ProbeBudget::INSTANT),
    );
}

#[test]
fn agent_runner_caps_broken_is_caught() {
    assert_caught(
        "AgentRunnerCapsBroken",
        "capabilities_are_stable_and_typed",
        super::agent_runner::agent_runner_suite(&AgentRunnerCapsBroken, ProbeBudget::INSTANT),
    );
}

#[test]
fn budget_meter_green_is_clean() {
    assert_green(
        "BudgetMeterGreen",
        super::budget_meter::budget_meter_suite(&BudgetMeterGreen),
    );
}

#[test]
fn budget_meter_broken_is_caught() {
    assert_caught(
        "BudgetMeterBroken",
        "reserve_consuming_the_recovery_reserve_is_refused",
        super::budget_meter::budget_meter_suite(&BudgetMeterBroken),
    );
}

#[test]
fn clock_green_is_clean() {
    assert_green("ClockGreen", super::clock::clock_suite(&ClockGreen));
}

#[test]
fn clock_broken_is_caught() {
    assert_caught(
        "ClockBroken",
        "now_never_moves_backwards",
        super::clock::clock_suite(&ClockBroken),
    );
}

#[test]
fn comparator_green_is_clean() {
    assert_green(
        "ComparatorGreen",
        super::comparator::comparator_suite(&ComparatorGreen),
    );
}

#[test]
fn comparator_broken_is_caught() {
    assert_caught(
        "ComparatorBroken",
        "record_digests_couple_the_envelope",
        super::comparator::comparator_suite(&ComparatorBroken),
    );
}

#[test]
fn event_log_green_is_clean() {
    assert_green(
        "EventLogGreen",
        super::event_log::event_log_suite(&EventLogGreen),
    );
}

#[test]
fn event_log_broken_is_caught() {
    assert_caught(
        "EventLogBroken",
        "append_returns_monotonic_offsets",
        super::event_log::event_log_suite(&EventLogBroken),
    );
}

#[test]
fn evidence_store_green_is_clean() {
    assert_green(
        "EvidenceStoreGreen",
        super::evidence_store::evidence_store_suite(&EvidenceStoreGreen),
    );
}

#[test]
fn evidence_store_broken_is_caught() {
    assert_caught(
        "EvidenceStoreBroken",
        "get_round_trips_bytes",
        super::evidence_store::evidence_store_suite(&EvidenceStoreBroken),
    );
}

#[test]
fn human_escalation_green_is_clean() {
    assert_green(
        "HumanEscalationGreen",
        super::human_escalation::human_escalation_suite(&HumanEscalationGreen),
    );
}

#[test]
fn human_escalation_broken_is_caught() {
    assert_caught(
        "HumanEscalationBroken",
        "escalate_returns_an_open_ticket",
        super::human_escalation::human_escalation_suite(&HumanEscalationBroken),
    );
}

#[test]
fn id_generator_green_is_clean() {
    assert_green(
        "IdGeneratorGreen",
        super::id_generator::id_generator_suite(&IdGeneratorGreen),
    );
}

#[test]
fn id_generator_broken_is_caught() {
    assert_caught(
        "IdGeneratorBroken",
        "ids_never_collide",
        super::id_generator::id_generator_suite(&IdGeneratorBroken),
    );
}

#[test]
fn observer_green_is_clean() {
    assert_green(
        "ObserverGreen",
        super::observer::observer_suite(&ObserverGreen),
    );
}

#[test]
fn observer_broken_is_caught() {
    assert_caught(
        "ObserverBroken",
        "facets_name_their_own_kind",
        super::observer::observer_suite(&ObserverBroken),
    );
}

#[test]
fn operator_surface_green_is_clean() {
    assert_green(
        "OperatorSurfaceGreen",
        super::operator_surface::operator_surface_suite(&OperatorSurfaceGreen),
    );
}

#[test]
fn operator_surface_broken_is_caught() {
    assert_caught(
        "OperatorSurfaceBroken",
        "snapshot_echoes_the_runtime_state",
        super::operator_surface::operator_surface_suite(&OperatorSurfaceBroken),
    );
}

#[test]
fn policy_provider_green_is_clean() {
    assert_green(
        "PolicyProviderGreen",
        super::policy_provider::policy_provider_suite(&PolicyProviderGreen),
    );
}

#[test]
fn policy_provider_broken_is_caught() {
    assert_caught(
        "PolicyProviderBroken",
        "irreversible_is_refused_structurally",
        super::policy_provider::policy_provider_suite(&PolicyProviderBroken),
    );
}

#[test]
fn reference_provider_green_is_clean() {
    assert_green(
        "ReferenceProviderGreen",
        super::reference_provider::reference_provider_suite(&ReferenceProviderGreen),
    );
}

#[test]
fn reference_provider_broken_is_caught() {
    assert_caught(
        "ReferenceProviderBroken",
        "the_tripwire_watches_the_artifact",
        super::reference_provider::reference_provider_suite(&ReferenceProviderBroken),
    );
}

#[test]
fn sandbox_green_is_clean() {
    assert_green("SandboxGreen", super::sandbox::sandbox_suite(&SandboxGreen));
}

#[test]
fn sandbox_broken_is_caught() {
    assert_caught(
        "SandboxBroken",
        "wrap_demands_its_own_validation",
        super::sandbox::sandbox_suite(&SandboxBroken),
    );
}

#[test]
fn state_store_green_is_clean() {
    assert_green(
        "StateStoreGreen",
        super::state_store::state_store_suite(&StateStoreGreen),
    );
}

#[test]
fn state_store_broken_is_caught() {
    assert_caught(
        "StateStoreBroken",
        "export_is_byte_identical_and_faithful",
        super::state_store::state_store_suite(&StateStoreBroken),
    );
}

#[test]
fn tool_provider_green_is_clean() {
    assert_green(
        "ToolProviderGreen",
        super::tool_provider::tool_provider_suite(&ToolProviderGreen),
    );
}

#[test]
fn tool_provider_broken_is_caught() {
    assert_caught(
        "ToolProviderBroken",
        "available_reports_exactly_the_resolvable_set",
        super::tool_provider::tool_provider_suite(&ToolProviderBroken),
    );
}

#[test]
fn verifier_green_is_clean() {
    assert_green(
        "VerifierGreen",
        super::verifier::verifier_suite(&VerifierGreen),
    );
}

#[test]
fn verifier_broken_is_caught() {
    assert_caught(
        "VerifierBroken",
        "missing_evidence_is_an_error_not_silence",
        super::verifier::verifier_suite(&VerifierBroken),
    );
}

#[test]
fn workspace_provider_green_is_clean() {
    assert_green(
        "WorkspaceProviderGreen",
        super::workspace_provider::workspace_provider_suite(&WorkspaceProviderGreen),
    );
}

#[test]
fn workspace_provider_broken_is_caught() {
    assert_caught(
        "WorkspaceProviderBroken",
        "create_isolates_parallel_subjects",
        super::workspace_provider::workspace_provider_suite(&WorkspaceProviderBroken),
    );
}
