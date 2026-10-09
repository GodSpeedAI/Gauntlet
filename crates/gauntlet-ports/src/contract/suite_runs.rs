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

/// The `CapabilityLedger` reference stub runs its suite clean — the floor
/// every adapter must clear (plan Task 22, charter D1).
#[test]
fn capability_ledger_green_is_clean() {
    assert_green(
        "CapabilityLedgerGreen",
        super::capability_ledger::capability_ledger_suite(&CapabilityLedgerGreenFactory),
    );
}

/// The broken stub inflates the observation log on re-append — exactly the
/// defect that would let a replayed import inflate a promotion's
/// settlement count. Caught on the idempotency case specifically.
#[test]
fn capability_ledger_broken_is_caught() {
    assert_caught(
        "CapabilityLedgerBroken",
        "observation_appends_are_idempotent_on_identity",
        super::capability_ledger::capability_ledger_suite(&CapabilityLedgerBrokenFactory),
    );
}

/// The caps-broken stub hands back observations whose typed facts differ
/// from the rows appended — the facts a §19.2 decision reads are not the
/// facts recorded. Caught on the exactness case specifically.
#[test]
fn capability_ledger_caps_broken_is_caught() {
    assert_caught(
        "CapabilityLedgerCapsBroken",
        "observations_read_back_exactly_as_appended",
        super::capability_ledger::capability_ledger_suite(&CapabilityLedgerCapsBrokenFactory),
    );
}

/// The `RevisionLedger` reference stub runs its suite clean — the floor
/// every adapter must clear (plan Task 23, charter D2).
#[test]
fn revision_ledger_green_is_clean() {
    assert_green(
        "RevisionLedgerGreen",
        super::revision_ledger::revision_ledger_suite(&RevisionLedgerGreenFactory),
    );
}

/// The broken stub duplicates the proposal admission on re-admit — exactly
/// the defect that would let a replayed admission multiply a revision's
/// durable subject. Caught on the idempotency case specifically.
#[test]
fn revision_ledger_broken_is_caught() {
    assert_caught(
        "RevisionLedgerBroken",
        "proposals_are_idempotent_on_revision",
        super::revision_ledger::revision_ledger_suite(&RevisionLedgerBrokenFactory),
    );
}

/// The caps-broken stub hands back proposals whose declared facts differ
/// from the rows admitted — the record the §19.4 gate adjudicates is not
/// the record the optimizer declared. Caught on the exactness case
/// specifically.
#[test]
fn revision_ledger_caps_broken_is_caught() {
    assert_caught(
        "RevisionLedgerCapsBroken",
        "proposals_read_back_exactly_as_admitted",
        super::revision_ledger::revision_ledger_suite(&RevisionLedgerCapsBrokenFactory),
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
fn comparator_facet_blind_is_caught() {
    // The widened-envelope case's teeth: a comparator that copies every
    // digest faithfully but clean-passes over a recorded instrument
    // failure is caught (plan Task 10, P3 comparison-consumption
    // coupling).
    assert_caught(
        "ComparatorFacetBlind",
        "facet_reality_is_visible_to_the_comparator",
        super::comparator::comparator_suite(&ComparatorFacetBlind),
    );
}

/// The F1 tooth (Task 10, round-9 correction). The comparator suite is a
/// PUBLIC entry point of this crate: whatever context it dispatches, it
/// dispatches into an adapter-supplied `Comparator`, which any downstream
/// crate can implement and which can retain what it is handed. So the suite
/// must not be able to fabricate one. This asserts the dispatched context is
/// the product of the real chain — a digest verified against the fixture's
/// own frozen bytes, an envelope digest computed over the canonical bytes,
/// and an observation PROJECTED from the durably bound bundle — none of
/// which a struct-literal fixture would satisfy.
#[test]
fn the_comparator_suite_dispatches_only_contexts_it_earned() {
    use std::sync::{Arc, Mutex};

    use gauntlet_domain::{ComparisonRecord, Digest, Verdict};
    use sha2::{Digest as _, Sha256};

    use crate::comparator::{
        Comparator, ComparatorError, TrustedComparisonContext, assert_blind,
        calculate_envelope_digest,
    };
    use crate::contract::comparator::ComparatorFactory;
    use crate::deadline::Deadline;

    #[derive(Default)]
    struct Seen {
        reference_digests: Vec<Digest>,
        digest_self_consistent: Vec<bool>,
        blind: Vec<bool>,
        failing_facets: usize,
    }

    struct Capturing(Arc<Mutex<Seen>>);

    impl Comparator for Capturing {
        fn compare(
            &self,
            context: &TrustedComparisonContext,
            _deadline: Deadline,
        ) -> Result<ComparisonRecord, ComparatorError> {
            let envelope = context.envelope();
            {
                let mut seen = self.0.lock().expect("the capture lock is uncontended");
                seen.reference_digests
                    .push(envelope.reference_digest().clone());
                seen.digest_self_consistent
                    .push(calculate_envelope_digest(envelope) == *envelope.envelope_digest());
                seen.blind.push(assert_blind(&envelope.to_json()).is_ok());
                seen.failing_facets += envelope
                    .observation()
                    .facets()
                    .iter()
                    .filter(|facet| facet.exit_status().is_some_and(|status| status != 0))
                    .count();
            }
            ComparisonRecord::new(
                envelope.reference_digest().clone(),
                envelope.observation().observation_digest().clone(),
                envelope.envelope_digest().clone(),
                Vec::new(),
                Verdict::Pass,
            )
            .map_err(|error| ComparatorError::ComparisonFailed {
                detail: error.to_string(),
            })
        }
    }

    struct CapturingFactory(Arc<Mutex<Seen>>);

    impl ComparatorFactory for CapturingFactory {
        fn label(&self) -> &'static str {
            "capturing-comparator"
        }

        fn supported_version(&self) -> String {
            "1".to_owned()
        }

        fn build(&self) -> Result<Box<dyn Comparator>, String> {
            Ok(Box::new(Capturing(Arc::clone(&self.0))))
        }
    }

    let seen = Arc::new(Mutex::new(Seen::default()));
    let _ = super::comparator::comparator_suite(&CapturingFactory(Arc::clone(&seen)));
    let seen = seen.lock().expect("the capture lock is uncontended");

    // The fixture's frozen reference bytes, hashed the way
    // `FrozenReferenceProjection::load` hashes them. A context minted from a
    // struct literal would carry whatever digest the literal named.
    let earned = Digest::new(
        "sha256",
        &Sha256::digest(b"the runtime-rendered view of the frozen standard\n")
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    )
    .expect("sha-256 output is a valid digest");

    assert!(
        !seen.reference_digests.is_empty(),
        "the suite dispatched no comparison at all"
    );
    for digest in &seen.reference_digests {
        assert_eq!(
            digest, &earned,
            "the suite dispatched a context whose reference digest is not the verified digest \
             of its own frozen reference bytes — it was fabricated, not earned"
        );
    }
    assert!(
        seen.digest_self_consistent.iter().all(|ok| *ok),
        "the suite dispatched an envelope whose digest was not computed over its canonical bytes"
    );
    assert!(
        seen.blind.iter().all(|ok| *ok),
        "the suite dispatched an envelope that does not survive the blindness assertion"
    );
    assert_eq!(
        seen.failing_facets, 1,
        "the failing instrument facet must reach the comparator by projection from the durably \
         bound bundle bytes"
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

/// The no-unsandboxed-fallback teeth (plan Task 14, AK-047): a sandbox
/// whose `wrap` accepts a findings-carrying validation — laundering a
/// failed validation into an executed command — is caught, and on the
/// fallback case specifically.
#[test]
fn sandbox_fallback_broken_is_caught() {
    assert_caught(
        "SandboxBrokenFallback",
        "no_unsandboxed_fallback",
        super::sandbox::sandbox_suite(&SandboxBrokenFallback),
    );
}

/// The process-start teeth (plan Task 14, AK-048): a sandbox that CLAIMS
/// the process rung but performs an identity wrap — the bare probe would
/// be the first process — is caught, and on the process-start case
/// specifically.
#[test]
fn sandbox_process_start_broken_is_caught() {
    assert_caught(
        "SandboxBrokenProcessStart",
        "wraps_from_process_start",
        super::sandbox::sandbox_suite(&SandboxBrokenProcessStart),
    );
}

/// The policy-interpolation teeth (plan Task 14, AK-047): a sandbox that
/// splices a policy's writable roots and denied paths RAW into the command
/// it renders — so a path carrying a flag after a space smuggles that flag
/// into the backend's own argument list, and an honest path containing a
/// space arrives split — is caught, and on the interpolation case
/// specifically.
#[test]
fn sandbox_shell_words_broken_is_caught() {
    assert_caught(
        "SandboxBrokenShellWords",
        "policy_paths_are_not_shell_words",
        super::sandbox::sandbox_suite(&SandboxBrokenShellWords),
    );
}

/// The nested-denial teeth (plan Task 14, AK-047): a sandbox that quotes
/// every interpolated path correctly but emits its denied-path masks
/// BEFORE the binds granting its writable roots — so a later bind of an
/// enclosing root re-exposes the denied subtree — is caught, and on the
/// nested-denial case specifically.
#[test]
fn sandbox_mask_order_broken_is_caught() {
    assert_caught(
        "SandboxBrokenMaskOrder",
        "a_nested_denial_survives_the_writable_bind",
        super::sandbox::sandbox_suite(&SandboxBrokenMaskOrder),
    );
}

/// The relocatable-denial teeth (plan Task 14, AK-047): a sandbox that
/// quotes every interpolated path AND orders its masks after its binds —
/// clean on every earlier case — but expresses a nested denial as nothing
/// but a path mask, so a denial reachable through a directory the contained
/// process is permitted to rename moves out from under its own mask, is
/// caught, and on the relocatable-denial case specifically.
#[test]
fn sandbox_relocatable_denial_broken_is_caught() {
    assert_caught(
        "SandboxBrokenRelocatableDenial",
        "a_relocatable_nested_denial_is_refused",
        super::sandbox::sandbox_suite(&SandboxBrokenRelocatableDenial),
    );
}

/// The process-space teeth (plan Task 14, AK-050): a sandbox that claims the
/// process rung and really wraps — its own invocation is the first process,
/// with the payload inside it — but confines nothing about the PROCESS space,
/// so the contained payload can still reach a process outside the sandbox, is
/// caught, and on the process-space case specifically.
#[test]
fn sandbox_process_space_broken_is_caught() {
    assert_caught(
        "SandboxBrokenProcessSpace",
        "the_process_space_is_confined",
        super::sandbox::sandbox_suite(&SandboxBrokenProcessSpace),
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

/// The settlement-surface teeth (plan Task 13): a store that FAKES the
/// surface — the commit reports success having done nothing, the replay
/// always claims an empty window — must be caught, and on the settlement
/// cases, not incidentally elsewhere.
#[test]
fn state_store_settlement_lax_is_caught() {
    assert_caught(
        "StateStoreSettlementLax",
        "settlements_are_earned_committed_and_read_back",
        super::state_store::state_store_suite(&StateStoreSettlementLax),
    );
    assert_caught(
        "StateStoreSettlementLax",
        "settlement_events_are_earned_and_replay_never_disturbs_rows",
        super::state_store::state_store_suite(&StateStoreSettlementLax),
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

/// A runtime that drops the host's record and reports an empty trace. The
/// trace must come from the host, so the suite has to catch it.
struct TraceForgingRuntime;

impl crate::tool_program::ToolProgramRuntime for TraceForgingRuntime {
    fn run(
        &self,
        request: &crate::tool_program::ToolProgramRequest,
        host: &dyn crate::tool_program::ProgramHost,
    ) -> crate::tool_program::ProgramOutcome {
        if let Ok(name) = crate::tool_catalog::ToolName::new("tools.read") {
            host.call(&name, &crate::json::JsonValue::Object(Vec::new()));
        }
        crate::tool_program::finish_outcome(
            crate::tool_program::ProgramStatus::Completed(crate::json::JsonValue::Null),
            crate::tool_program::HostRecord {
                trace: Vec::new(),
                breach: None,
            },
            request,
            core::time::Duration::ZERO,
        )
    }
}

struct TraceForgingFactory;

impl super::tool_program::ToolProgramFactory for TraceForgingFactory {
    fn label(&self) -> &'static str {
        "trace-forging"
    }

    fn build(&self) -> Result<Box<dyn crate::tool_program::ToolProgramRuntime>, String> {
        Ok(Box::new(TraceForgingRuntime))
    }
}

/// A runtime that issues one more call after a `Completed` program, as if
/// completion marked work done. The suite must catch the extra call.
struct CompletionLeakRuntime;

impl crate::tool_program::ToolProgramRuntime for CompletionLeakRuntime {
    fn run(
        &self,
        request: &crate::tool_program::ToolProgramRequest,
        host: &dyn crate::tool_program::ProgramHost,
    ) -> crate::tool_program::ProgramOutcome {
        let outcome = crate::tool_program::ToolProgramRuntime::run(
            &super::tool_program_reference::MockProgramRuntime,
            request,
            host,
        );
        if let (crate::tool_program::ProgramStatus::Completed(_), Ok(name)) = (
            &outcome.status,
            crate::tool_catalog::ToolName::new("tools.read"),
        ) {
            host.call(&name, &crate::json::JsonValue::Object(Vec::new()));
        }
        let record = host.record();
        crate::tool_program::finish_outcome(outcome.status, record, request, outcome.duration)
    }
}

struct CompletionLeakFactory;

impl super::tool_program::ToolProgramFactory for CompletionLeakFactory {
    fn label(&self) -> &'static str {
        "completion-leak"
    }

    fn build(&self) -> Result<Box<dyn crate::tool_program::ToolProgramRuntime>, String> {
        Ok(Box::new(CompletionLeakRuntime))
    }
}

#[test]
fn tool_program_green_is_clean() {
    assert_green(
        "MockProgramRuntime",
        super::tool_program::tool_program_suite(
            &super::tool_program_reference::MockProgramRuntimeFactory,
        ),
    );
}

#[test]
fn tool_program_trace_forging_is_caught() {
    assert_caught(
        "TraceForgingRuntime",
        "trace_is_the_host_record",
        super::tool_program::tool_program_suite(&TraceForgingFactory),
    );
}

#[test]
fn tool_program_completion_leak_is_caught() {
    assert_caught(
        "CompletionLeakRuntime",
        "completed_is_data_only",
        super::tool_program::tool_program_suite(&CompletionLeakFactory),
    );
}
