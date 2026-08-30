//! The scripted, deterministic `AgentRunner` (spec §6.1, §6.3, §10.3; plan
//! Task 8): adapter-owned generalization of the ports suite's ScriptedRunner
//! pattern — poll advances a script and never blocks on real time;
//! completion happens ONLY through
//! [`read_result_contract`](gauntlet_ports::role_result::read_result_contract)
//! (strict decode plus the §10.3 rule-6 identity check), with decode
//! failures mapped by [`failure_class`]; terminal statuses are reported
//! stably forever; `cancel` is idempotent with typed
//! `AlreadyTerminal`/`UnknownHandle` refusals; `collect` only after
//! `Completed`, returning the same document on every collect.
//!
//! Spec §6.3: the mock runner is REQUIRED so "the orchestration layer
//! [can] be tested without paying a frontier model". It returns results;
//! it decides nothing — every behavior it replays is child behavior
//! declared by a [`Scenario`] (fixture file or `RunnerScript` mapping),
//! never an orchestration decision.
//!
//! ## Result delivery: the `File` emulation
//!
//! `capabilities()` reports `result_delivery: File`, which is the honest
//! contract the runtime polls (§10.3: "The runtime treats the invocation
//! as `Completed` only when a well-formed result document exists and
//! validates"). This adapter EMULATES the file: the scripted child's raw
//! document text is held in a map keyed by the declared
//! [`ResultContractPath`] — standing in for the result file a real
//! harness's child writes — and completion is admitted only after that
//! text passes [`read_result_contract`] against the dispatch facts. The
//! raw text stays retrievable ([`MockAgentRunner::written_document`]) for
//! forensic assertions about what the child "wrote".
//!
//! ## Containment honesty
//!
//! `capabilities()` reports `isolation: Isolation::None` — the mock is an
//! in-process replay with no containment of any kind. Spec §6.1: "A
//! runner reporting `isolation: none` MUST NOT be selected for a tier
//! requiring containment (§17.3)" — Task 14's scheduler must be able to
//! refuse this adapter for contained tiers, which is only possible if the
//! adapter does not inflate its isolation rung. `max_depth: 0` for the
//! same reason: the mock cannot delegate inside itself at all.

use core::cell::RefCell;
use std::collections::HashMap;
use std::path::PathBuf;

use gauntlet_domain::{EpochSeconds, InvocationId, InvocationState, Payment, RoleId};
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, CancelReason, InvocationFailure, InvocationHandle, InvocationStatus,
    Isolation, ResultDelivery, RoleInvocationSpec, RunnerCapabilities,
};
use gauntlet_ports::contract::agent_runner::RunnerScript;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::role_result::{RoleResult, RoleResultState, read_result_contract};

use crate::scenario::{ChildBehavior, Derailment, PayloadParts, Scenario};

/// The fixed virtual instant every in-flight `Running` report cites. The
/// mock has no clock of its own and no real event stream; a named constant
/// keeps the reports deterministic instead of borrowing wall time.
const SCRIPTED_EVENT_EPOCH: u64 = 1_700_000_000;

/// How many polls a suite-scripted silent child stalls for before the
/// virtual deadline fires (spec §10.3 rule 5). The suite's probe budget
/// allows far more; four keeps the deterministic runs short. Fixture
/// scenarios carry their own `timeout_after_probes`.
const SILENT_CHILD_PROBES: u8 = 4;

/// One admitted invocation's replay state.
struct MockEntry {
    spec: RoleInvocationSpec,
    probes: u32,
    effects_applied: bool,
    terminal: Option<InvocationStatus>,
    result: Option<RoleResult>,
}

impl MockEntry {
    fn in_flight(&self) -> InvocationStatus {
        InvocationStatus::Running {
            last_event_at: EpochSeconds::new(SCRIPTED_EVENT_EPOCH),
            payment_so_far: Payment::zero(),
        }
    }
}

/// The scripted runner. Construct with a [`Scenario`] and the workspace
/// root under which the scenario's file effects resolve (tests provide a
/// tempdir; fixtures never name absolute paths), or through
/// [`MockAgentRunner::from_script`] for suite-driven children.
///
/// Single-threaded by design (`RefCell` interior): one runner replays one
/// child per admitted invocation, and the contract suite drives it from
/// one thread.
pub struct MockAgentRunner {
    scenario: Scenario,
    workspace_root: PathBuf,
    entries: RefCell<HashMap<InvocationId, MockEntry>>,
    /// The `File`-delivery emulation: raw document text keyed by the
    /// declared result-contract path.
    documents: RefCell<HashMap<String, String>>,
}

impl MockAgentRunner {
    /// A runner replaying `scenario`, resolving file effects under
    /// `workspace_root`.
    pub fn new(scenario: Scenario, workspace_root: PathBuf) -> Self {
        Self {
            scenario,
            workspace_root,
            entries: RefCell::new(HashMap::new()),
            documents: RefCell::new(HashMap::new()),
        }
    }

    /// The `RunnerScript` → mock-configuration mapping: the unmodified
    /// Task-4 contract suite drives adapters through
    /// [`RunnerScript`](gauntlet_ports::contract::agent_runner::RunnerScript)
    /// child behaviors; this maps each one to the scenario that models it.
    /// The workspace root is a per-instance scratch under the system temp
    /// directory — suite scripts carry no file effects, so nothing is ever
    /// written there.
    pub fn from_script(script: RunnerScript) -> Self {
        Self::new(Self::scenario_for_script(script), scratch_root())
    }

    /// The raw document text the scripted child "wrote" at the declared
    /// result-contract path — the `File`-delivery emulation's forensic
    /// surface.
    pub fn written_document(
        &self,
        contract: &gauntlet_domain::ResultContractPath,
    ) -> Option<String> {
        self.documents.borrow().get(contract.as_str()).cloned()
    }

    fn scenario_for_script(script: RunnerScript) -> Scenario {
        use RunnerScript::{
            CompleteAfterProbes, CompleteImmediately, ExitWithoutResult, NeverWriteResult,
            WriteForeignEnvelopeDigest, WriteForeignInvocationId, WriteMalformedResult,
            WriteUnknownEnvelopeField, WriteUnknownNestedField,
        };
        match script {
            CompleteAfterProbes {
                probes_before_result,
            } => writing(
                "suite-complete-after-probes",
                Derailment::None,
                Some(probes_before_result),
            ),
            CompleteImmediately => writing("suite-complete-immediately", Derailment::None, None),
            NeverWriteResult => Scenario::assemble(
                "suite-never-write-result".to_owned(),
                RoleId::Builder,
                ChildBehavior::Stall,
                0,
                Some(SILENT_CHILD_PROBES),
                None,
                Derailment::None,
                None,
                Vec::new(),
            ),
            // Plan Task 16, shape 2: the child EXITED without writing —
            // the first probe pins `Failed{missing_result}`, before any
            // deadline (distinct from the Stall deadline verdict above).
            ExitWithoutResult => Scenario::assemble(
                "suite-exit-without-result".to_owned(),
                RoleId::Builder,
                ChildBehavior::ExitWithoutResult,
                0,
                None,
                None,
                Derailment::None,
                None,
                Vec::new(),
            ),
            WriteMalformedResult => writing(
                "suite-write-malformed-result",
                Derailment::RawMalformed,
                None,
            ),
            WriteUnknownEnvelopeField => writing(
                "suite-write-unknown-envelope-field",
                Derailment::UnknownEnvelopeField,
                None,
            ),
            WriteUnknownNestedField => writing(
                "suite-write-unknown-nested-field",
                Derailment::UnknownNestedField,
                None,
            ),
            WriteForeignInvocationId => writing(
                "suite-write-foreign-invocation-id",
                Derailment::ForeignInvocationId,
                None,
            ),
            WriteForeignEnvelopeDigest => writing(
                "suite-write-foreign-envelope-digest",
                Derailment::StaleEnvelopeDigest,
                None,
            ),
        }
    }

    /// Applies one poll's worth of script advance to `entry`.
    fn advance(&self, entry: &mut MockEntry) {
        if entry.terminal.is_some() {
            return;
        }
        entry.probes += 1;
        match self.scenario.behavior() {
            ChildBehavior::WriteResult => {
                if entry.probes <= u32::from(self.scenario.probes_before_write()) {
                    return; // still in flight at admission (spec §6.1)
                }
                if !entry.effects_applied {
                    entry.effects_applied = true;
                    if let Err(detail) = self.apply_effects() {
                        entry.terminal = Some(InvocationStatus::Failed {
                            class: InvocationFailure::Execution,
                        });
                        // The child could not even do its own work: record
                        // the failure locus where a real harness would log.
                        entry.result = None;
                        self.documents.borrow_mut().insert(
                            entry.spec.result_contract_path.as_str().to_owned(),
                            format!("{{effect application failed: {detail}}}"),
                        );
                        return;
                    }
                }
                let text = self.scenario.result_document_text(&entry.spec);
                self.documents.borrow_mut().insert(
                    entry.spec.result_contract_path.as_str().to_owned(),
                    text.clone(),
                );
                match read_result_contract(
                    &text,
                    &entry.spec.invocation_id,
                    entry.spec.context_envelope.digest(),
                ) {
                    Ok(result) => {
                        entry.result = Some(result);
                        entry.terminal = Some(InvocationStatus::Completed {
                            result_ref: entry.spec.result_contract_path.clone(),
                        });
                    }
                    Err(error) => {
                        entry.terminal = Some(InvocationStatus::Failed {
                            class: error.failure_class(),
                        });
                    }
                }
            }
            ChildBehavior::Stall => {
                // A silent child, modeled on probe counts: the deadline
                // fires after `timeout_after_probes` polls (§10.3 rule 5).
                if u32::from(
                    self.scenario
                        .timeout_after_probes()
                        .unwrap_or(SILENT_CHILD_PROBES),
                ) <= entry.probes
                {
                    entry.terminal = Some(InvocationStatus::TimedOut);
                }
            }
            ChildBehavior::ExitWithoutResult => {
                // Plan Task 16, shape 2: the child EXITED without ever
                // writing a valid result document. A dead child can never
                // write, so the verdict lands on the FIRST probe — before
                // any deadline — and is `Failed{missing_result}`, never
                // Completed (exit status is liveness evidence) and never
                // TimedOut (no deadline was reached).
                entry.terminal = Some(InvocationStatus::Failed {
                    class: InvocationFailure::MissingResult,
                });
            }
        }
    }

    /// Applies the scenario's workspace file effects under the runner's
    /// workspace root. The parser already refused absolute/`..` paths;
    /// this re-checks containment at use (two independent guards) and
    /// creates parent directories as a real child process would.
    fn apply_effects(&self) -> Result<(), String> {
        for effect in self.scenario.workspace_effects() {
            let target = self.workspace_root.join(effect.path());
            if !target.starts_with(&self.workspace_root) {
                return Err(format!(
                    "effect path {:?} escapes the workspace root — refused at resolution",
                    effect.path()
                ));
            }
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)
                    .map_err(|error| format!("creating {:?} for an effect: {error}", parent))?;
            }
            std::fs::write(&target, effect.content())
                .map_err(|error| format!("writing effect {:?}: {error}", effect.path()))?;
        }
        Ok(())
    }
}

/// A minimal writing scenario for the suite mapping.
fn writing(name: &'static str, derailment: Derailment, probes: Option<u8>) -> Scenario {
    Scenario::assemble(
        name.to_owned(),
        RoleId::Builder,
        ChildBehavior::WriteResult,
        probes.unwrap_or(0),
        None,
        Some(RoleResultState::Completed),
        derailment,
        Some(PayloadParts::scripted()),
        Vec::new(),
    )
}

/// A per-instance scratch directory for suite-driven runners (no effects
/// are ever applied there; the path only needs to be unique).
fn scratch_root() -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQUENCE: AtomicU32 = AtomicU32::new(0);
    std::env::temp_dir().join(format!(
        "gauntlet-agent-mock-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ))
}

/// The status → coarse state mapping the ports' own stub uses for
/// `ResultNotReady`/`AlreadyTerminal` loci.
fn status_state(status: &InvocationStatus) -> InvocationState {
    match status {
        InvocationStatus::Admitted | InvocationStatus::Running { .. } => InvocationState::Running,
        InvocationStatus::Completed { .. } => InvocationState::Completed,
        InvocationStatus::Failed { .. } => InvocationState::Failed,
        InvocationStatus::TimedOut => InvocationState::TimedOut,
        InvocationStatus::Cancelled => InvocationState::Cancelled,
    }
}

impl AgentRunner for MockAgentRunner {
    fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
        // Admission records the dispatch facts (expected invocation id,
        // envelope digest, result-contract path, workspace ref — the whole
        // spec) and returns promptly; no work happens here (spec §6.1:
        // "invoke MUST return promptly after admission").
        //
        // A known invocation id is never re-admitted: replacing the entry
        // would reset a terminal one and resurrect it to Running — the
        // ports' pinned terminal stability (§6.1) forbids it, so a
        // duplicate admission is a typed refusal and the existing entry
        // is left untouched.
        let invocation_id = spec.invocation_id.clone();
        if self.entries.borrow().contains_key(&invocation_id) {
            return Err(AgentError::AdmissionRefused {
                detail: format!(
                    "invocation {invocation_id} is already admitted to this runner; \
                     duplicate admission is refused and the existing entry is untouched"
                ),
            });
        }
        let session = format!("mock-{}", self.scenario.name());
        let session_ref = gauntlet_domain::SessionRef::new(&session).ok();
        self.entries.borrow_mut().insert(
            invocation_id.clone(),
            MockEntry {
                spec,
                probes: 0,
                effects_applied: false,
                terminal: None,
                result: None,
            },
        );
        Ok(InvocationHandle::new(invocation_id, session_ref))
    }

    fn poll(&self, handle: &InvocationHandle) -> Result<InvocationStatus, AgentError> {
        let mut entries = self.entries.borrow_mut();
        let Some(entry) = entries.get_mut(handle.invocation_id()) else {
            return Err(AgentError::UnknownHandle {
                invocation_id: handle.invocation_id().clone(),
            });
        };
        self.advance(entry);
        Ok(entry.terminal.clone().unwrap_or_else(|| entry.in_flight()))
    }

    fn collect(
        &self,
        handle: &InvocationHandle,
        _deadline: Deadline,
    ) -> Result<RoleResult, AgentError> {
        let entries = self.entries.borrow();
        let Some(entry) = entries.get(handle.invocation_id()) else {
            return Err(AgentError::UnknownHandle {
                invocation_id: handle.invocation_id().clone(),
            });
        };
        match (&entry.terminal, &entry.result) {
            (Some(InvocationStatus::Completed { .. }), Some(result)) => Ok(result.clone()),
            (Some(terminal), _) => Err(AgentError::ResultNotReady {
                invocation_id: handle.invocation_id().clone(),
                state: status_state(terminal),
            }),
            (None, _) => Err(AgentError::ResultNotReady {
                invocation_id: handle.invocation_id().clone(),
                state: InvocationState::Running,
            }),
        }
    }

    fn cancel(
        &self,
        handle: &InvocationHandle,
        _reason: CancelReason,
        _deadline: Deadline,
    ) -> Result<(), AgentError> {
        let mut entries = self.entries.borrow_mut();
        let Some(entry) = entries.get_mut(handle.invocation_id()) else {
            return Err(AgentError::UnknownHandle {
                invocation_id: handle.invocation_id().clone(),
            });
        };
        if let Some(terminal) = &entry.terminal {
            // Idempotent: the typed no-op, never a second effect.
            return Err(AgentError::AlreadyTerminal {
                invocation_id: handle.invocation_id().clone(),
                state: status_state(terminal),
            });
        }
        entry.terminal = Some(InvocationStatus::Cancelled);
        Ok(())
    }

    fn capabilities(&self) -> RunnerCapabilities {
        // Honest by construction — see the module docs: no containment
        // (Task 14's tier check must be able to refuse this adapter), the
        // File-delivery emulation, and no delegation depth at all.
        RunnerCapabilities {
            supports_native_subagents: false,
            supports_streaming_events: false,
            supports_model_selection: false,
            isolation: Isolation::None,
            result_delivery: ResultDelivery::File,
            max_depth: 0,
        }
    }
}

/// The suite-facing factory: builds an isolated runner per case (spec
/// §7.3; the unmodified Task-4 suite runs against it — AK-059's core).
///
/// The runner is TIME-FREE by design: it never consults a clock, virtual
/// or real — probe counts are the only time surrogate. A future change
/// wanting wall behavior must inject the `Clock` port instead of calling
/// the system clock directly.
pub struct MockRunnerFactory;

impl gauntlet_ports::contract::agent_runner::AgentRunnerFactory for MockRunnerFactory {
    fn label(&self) -> &'static str {
        "mock-agent-runner"
    }

    fn build(&self, script: RunnerScript) -> Result<Box<dyn AgentRunner>, String> {
        Ok(Box::new(MockAgentRunner::from_script(script)))
    }
}

#[cfg(test)]
mod tests {
    use super::MockAgentRunner;
    use crate::scenario::parse_scenario;
    use gauntlet_domain::{
        Budget, DiagnosticAllowance, Digest, EpochSeconds, InvocationId, InvocationState, Limits,
        MicroUsd, PermissionSetRef, ResultContractPath, RoleId, RunId, WorkspaceRef,
    };
    use gauntlet_ports::agent_runner::{
        AgentError, AgentRunner, CancelReason, ContextEnvelope, InvocationFailure,
        InvocationStatus, Isolation, ResultDelivery, RoleInvocationSpec,
    };
    use gauntlet_ports::deadline::Deadline;
    use std::path::PathBuf;

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const DIGEST: &str = "sha256:a1b2c3d4e5f6a7b8";

    fn spec() -> RoleInvocationSpec {
        let budget = Budget::new(
            Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("limits usable"),
            20,
            DiagnosticAllowance::new(100, MicroUsd::new(50)),
        )
        .expect("budget usable");
        RoleInvocationSpec {
            invocation_id: InvocationId::new(ULID).expect("valid id"),
            run_id: RunId::new(ULID).expect("valid id"),
            iteration_no: gauntlet_domain::IterationNo::new(1).expect("1-based"),
            role: RoleId::Builder,
            prompt: "run the assigned slice".to_owned(),
            context_envelope: ContextEnvelope::new(
                "allowlisted context".to_owned(),
                Digest::parse(DIGEST).expect("valid digest"),
            ),
            workspace: WorkspaceRef::new("ws-runner").expect("valid ref"),
            permissions: PermissionSetRef::new("perm-readonly").expect("valid ref"),
            result_contract_path: ResultContractPath::new("runner-result-document")
                .expect("valid ref"),
            deadline: Deadline::at_epoch(EpochSeconds::new(1_700_000_090)),
            model_hint: None,
            sandbox_policy: None,
            budget_slice: budget,
        }
    }

    fn happy_scenario() -> crate::scenario::Scenario {
        let text = [
            "scenario: runner-happy",
            "role: builder",
            "behavior: write-result",
            "state: completed",
            "payload:",
            "  attempted: [AK-014]",
            "  changed_paths: []",
            "  commands_run: []",
            "  artifacts: []",
            "  claims:",
            "    - the slice is done",
            "  self_checks: []",
            "  blocked_on: []",
        ]
        .join("\n");
        parse_scenario(&text).expect("the minimal writing scenario parses")
    }

    /// Re-assembles a parsed scenario with a different probe knob (the
    /// parser owns construction; tests adjust one field through the same
    /// validated path).
    fn with_probes(scenario: &crate::scenario::Scenario, probes: u8) -> crate::scenario::Scenario {
        crate::scenario::Scenario::assemble(
            scenario.name().to_owned(),
            scenario.role().clone(),
            scenario.behavior(),
            probes,
            scenario.timeout_after_probes(),
            scenario.state(),
            scenario.derailment(),
            scenario.payload().cloned(),
            scenario.workspace_effects().to_vec(),
        )
    }

    fn scratch(tag: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "gauntlet-agent-mock-test-{tag}-{}-{}",
            std::process::id(),
            tag
        ));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("test scratch is creatable");
        path
    }

    #[test]
    fn invoke_admits_and_first_poll_reports_in_flight_work() {
        // One probe of in-flight work before the child writes.
        let scenario = with_probes(&happy_scenario(), 1);
        let runner = MockAgentRunner::new(scenario, scratch("admit"));
        let handle = runner.invoke(spec()).expect("admission");
        assert_eq!(handle.invocation_id().as_str(), ULID);
        assert!(
            handle.session_ref().is_some(),
            "the session ref names the scenario for forensics"
        );
        match runner.poll(&handle).expect("first poll") {
            InvocationStatus::Running { .. } => {}
            other => panic!("the first poll must report in-flight work, got {other:?}"),
        }
    }

    #[test]
    fn re_invoking_a_completed_invocation_is_refused_and_terminal_state_survives() {
        // Pinned terminal stability (the ports' `poll` contract): a
        // duplicate admission must never reset a terminal entry — without
        // the refusal, a Completed invocation would poll Running again.
        let runner = MockAgentRunner::new(happy_scenario(), scratch("dup-terminal"));
        let handle = runner.invoke(spec()).expect("admission");
        match runner.poll(&handle).expect("completes on the first poll") {
            InvocationStatus::Completed { .. } => {}
            other => panic!("got {other:?}"),
        }
        let error = runner
            .invoke(spec())
            .expect_err("duplicate admission is refused");
        assert!(
            matches!(&error, AgentError::AdmissionRefused { detail } if detail.contains("dupl")),
            "got {error:?}"
        );
        // The existing entry is untouched: still Completed, forever.
        for _ in 0..3 {
            assert!(
                matches!(
                    runner.poll(&handle).expect("poll after the refusal"),
                    InvocationStatus::Completed { .. }
                ),
                "the terminal entry must never resurrect"
            );
        }
        let bound = Deadline::at_epoch(EpochSeconds::new(1_700_000_090));
        let result = runner.collect(&handle, bound).expect("still collectible");
        assert_eq!(
            result.state(),
            gauntlet_ports::role_result::RoleResultState::Completed
        );
    }

    #[test]
    fn re_invoking_while_running_is_refused_too() {
        // The refusal is not terminal-only: a known id is a known id, and
        // re-admitting mid-flight would silently restart the script.
        let scenario = with_probes(&happy_scenario(), 2);
        let runner = MockAgentRunner::new(scenario, scratch("dup-running"));
        let handle = runner.invoke(spec()).expect("admission");
        assert!(matches!(
            runner.poll(&handle).expect("in flight"),
            InvocationStatus::Running { .. }
        ));
        let error = runner
            .invoke(spec())
            .expect_err("duplicate admission is refused mid-flight");
        assert!(
            matches!(&error, AgentError::AdmissionRefused { .. }),
            "got {error:?}"
        );
        // The in-flight entry continues its script undisturbed.
        match runner.poll(&handle).expect("second probe still in flight") {
            InvocationStatus::Running { .. } => {}
            other => panic!("got {other:?}"),
        }
        match runner.poll(&handle).expect("third probe completes") {
            InvocationStatus::Completed { .. } => {}
            other => panic!("got {other:?}"),
        }
    }

    #[test]
    fn collect_before_completion_is_result_not_ready() {
        let runner = MockAgentRunner::new(happy_scenario(), scratch("notready"));
        let handle = runner.invoke(spec()).expect("admission");
        let error = runner
            .collect(
                &handle,
                Deadline::at_epoch(EpochSeconds::new(1_700_000_090)),
            )
            .expect_err("collect is after completed");
        assert!(
            matches!(
                error,
                AgentError::ResultNotReady {
                    state: InvocationState::Running,
                    ..
                }
            ),
            "got {error:?}"
        );
    }

    #[test]
    fn cancel_before_terminal_is_idempotent_and_typed() {
        let runner = MockAgentRunner::new(happy_scenario(), scratch("cancel"));
        let handle = runner.invoke(spec()).expect("admission");
        let reason = CancelReason::new("test: superseded").expect("non-empty reason");
        let bound = Deadline::at_epoch(EpochSeconds::new(1_700_000_090));
        runner
            .cancel(&handle, reason.clone(), bound)
            .expect("the first cancel performs it");
        assert_eq!(
            runner.poll(&handle).expect("poll after cancel"),
            InvocationStatus::Cancelled
        );
        match runner.cancel(&handle, reason, bound) {
            Err(AgentError::AlreadyTerminal {
                state: InvocationState::Cancelled,
                ..
            }) => {}
            other => panic!("the second cancel must be the typed no-op, got {other:?}"),
        }
        // Still Cancelled, forever after.
        assert_eq!(
            runner.poll(&handle).expect("stability"),
            InvocationStatus::Cancelled
        );
        // Cancelled work never yields a result document.
        let error = runner
            .collect(&handle, bound)
            .expect_err("cancelled is not collectible");
        assert!(
            matches!(error, AgentError::ResultNotReady { .. }),
            "got {error:?}"
        );
    }

    #[test]
    fn unknown_handles_are_refused_by_every_operation() {
        let runner = MockAgentRunner::new(happy_scenario(), scratch("stranger"));
        let stranger = gauntlet_ports::agent_runner::InvocationHandle::new(
            InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FZZ").expect("distinct valid id"),
            None,
        );
        assert!(matches!(
            runner.poll(&stranger),
            Err(AgentError::UnknownHandle { .. })
        ));
        assert!(matches!(
            runner.collect(
                &stranger,
                Deadline::at_epoch(EpochSeconds::new(1_700_000_090))
            ),
            Err(AgentError::UnknownHandle { .. })
        ));
        assert!(matches!(
            runner.cancel(
                &stranger,
                CancelReason::new("test: stray").expect("non-empty"),
                Deadline::at_epoch(EpochSeconds::new(1_700_000_090))
            ),
            Err(AgentError::UnknownHandle { .. })
        ));
    }

    #[test]
    fn capabilities_report_honest_containment_and_delivery() {
        let runner = MockAgentRunner::new(happy_scenario(), scratch("caps"));
        let capabilities = runner.capabilities();
        assert_eq!(capabilities.isolation, Isolation::None);
        assert_eq!(capabilities.result_delivery, ResultDelivery::File);
        assert_eq!(capabilities.max_depth, 0);
        assert!(!capabilities.supports_native_subagents);
        assert!(!capabilities.supports_streaming_events);
        assert!(!capabilities.supports_model_selection);
    }

    #[test]
    fn workspace_effects_resolve_under_the_provided_root() {
        let text = [
            "scenario: runner-effects",
            "role: builder",
            "behavior: write-result",
            "state: completed",
            "payload:",
            "  attempted: [AK-014]",
            "  changed_paths: [out/artifact.txt]",
            "  commands_run: []",
            "  artifacts: []",
            "  claims:",
            "    - the artifact is written",
            "  self_checks: []",
            "  blocked_on: []",
            "workspace_effects:",
            "  - path: out/artifact.txt",
            "    content: the artifact body",
        ]
        .join("\n");
        let scenario = parse_scenario(&text).expect("parses");
        let root = scratch("effects");
        let runner = MockAgentRunner::new(scenario, root.clone());
        let handle = runner.invoke(spec()).expect("admission");
        match runner.poll(&handle).expect("completes") {
            InvocationStatus::Completed { .. } => {}
            other => panic!("got {other:?}"),
        }
        let written = std::fs::read_to_string(root.join("out/artifact.txt"))
            .expect("the effect landed under the provided root");
        assert_eq!(written, "the artifact body");
    }

    #[test]
    fn an_unwritable_workspace_maps_to_execution_failure() {
        // The workspace root is a FILE: every effect write fails, and the
        // child's own failure is Failed{agent_execution}, never a panic.
        let text = [
            "scenario: runner-effect-failure",
            "role: builder",
            "behavior: write-result",
            "state: completed",
            "payload:",
            "  attempted: [AK-014]",
            "  changed_paths: []",
            "  commands_run: []",
            "  artifacts: []",
            "  claims: []",
            "  self_checks: []",
            "  blocked_on: []",
            "workspace_effects:",
            "  - path: out/artifact.txt",
            "    content: unreachable",
        ]
        .join("\n");
        let scenario = parse_scenario(&text).expect("parses");
        let root_file = std::env::temp_dir().join(format!(
            "gauntlet-agent-mock-blockfile-{}",
            std::process::id()
        ));
        std::fs::write(&root_file, "this is a file, not a directory")
            .expect("block file is writable");
        let runner = MockAgentRunner::new(scenario, root_file.clone());
        let handle = runner.invoke(spec()).expect("admission");
        match runner.poll(&handle).expect("a typed failure, not a panic") {
            InvocationStatus::Failed {
                class: InvocationFailure::Execution,
            } => {}
            other => panic!("got {other:?}"),
        }
        std::fs::remove_file(&root_file).ok();
    }

    #[test]
    fn the_written_document_is_retrievable_by_contract_path() {
        let runner = MockAgentRunner::new(happy_scenario(), scratch("document"));
        let handle = runner.invoke(spec()).expect("admission");
        runner.poll(&handle).expect("completes");
        let contract = ResultContractPath::new("runner-result-document").expect("valid ref");
        let raw = runner
            .written_document(&contract)
            .expect("the emulation holds the raw text");
        assert!(
            raw.contains("\"schema\":\"role_result@v1\""),
            "the raw document is the envelope text: {raw}"
        );
    }

    #[test]
    fn repeated_collects_return_the_same_document() {
        let runner = MockAgentRunner::new(happy_scenario(), scratch("collect"));
        let handle = runner.invoke(spec()).expect("admission");
        runner.poll(&handle).expect("completes");
        let bound = Deadline::at_epoch(EpochSeconds::new(1_700_000_090));
        let first = runner.collect(&handle, bound).expect("collectible");
        let second = runner.collect(&handle, bound).expect("collectible again");
        assert_eq!(first, second);
    }
}
