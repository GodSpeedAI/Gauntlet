//! The shared `AgentRunner` contract suite (spec §7.3) run against the
//! prime-agent adapter — "Adding an adapter means implementing the trait
//! and running the existing suite, not writing a bespoke one." The suite
//! lives, UNMODIFIED, in `gauntlet-ports::contract::agent_runner`; this
//! file only supplies the factory and asserts the findings are empty.
//!
//! The harness (IPython kernel + model child) is what the doubles stand
//! in for; the ADAPTER's real logic is what the suite exercises (spec
//! §6.3's own design: "a real adapter maps it to a probe child instructed
//! to behave that way"):
//!
//! - the transport double performs the rendered program's side effects —
//!   it writes the admission-handle document at the submission's handle
//!   path and reads it back through the REAL strict
//!   [`read_admission`](gauntlet_adapter_agent_prime::read_admission) —
//!   and returns the ADMISSION handle only;
//! - the child's result-document writing is driven by the virtual clock
//!   the runner itself polls (one tick per `now()`), so "the child
//!   finishes between polls" is simulated deterministically, with NO
//!   process spawned by the default battery (R-1b: no real builder);
//! - the control double records stops and answers listings;
//! - the result document lands as a REAL FILE at the real anchored path
//!   (`<state_dir>/runs/<run_id>/inv/<invocation_id>/<leaf>`, correction
//!   RES-D3), so poll/collect classify through actual file IO and the
//!   actual strict `read_result_contract`.

// The suite doubles are deliberately single-threaded (the mock runner's
// own posture: one runner replays one child per admitted invocation, and
// the shared contract suite drives it from one thread), so the `Arc`s
// that hand them to the runner are knowingly non-Send/Sync.
#![allow(clippy::arc_with_non_send_sync)]

use std::cell::Cell;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use gauntlet_adapter_agent_prime::transport::{
    AdmissionHandle, ControlError, HarnessControl, KernelSubmission, KernelTransport,
    NullParentHandle, ParentHandle, SubagentRecord, TransportError, read_admission,
};
use gauntlet_adapter_agent_prime::{PrimeAdapterConfig, PrimeAgentRunner};
use gauntlet_domain::EpochSeconds;
use gauntlet_ports::agent_runner::{AgentError, AgentRunner, CancelReason, InvocationStatus};
use gauntlet_ports::clock::Clock;
use gauntlet_ports::contract::agent_runner::{
    AgentRunnerFactory, ProbeBudget, RunnerScript, agent_runner_suite,
};
use gauntlet_ports::deadline::Deadline;

/// The dispatch facts the shared suite pins (its `dispatch_spec` uses the
/// same run/invocation ids); the doubles key the child's document off
/// them.
const SUITE_ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const SUITE_DIGEST: &str = "sha256:e5f6a7b8c4d3e2f1a0b1c2d3";
const SUITE_CONTRACT_LEAF: &str = "suite-result-document";
/// The virtual clock's base instant — the suite's own pinned base.
const BASE_INSTANT: u64 = 1_700_000_000;
/// Virtual seconds per `now()` call. Two steps per poll-place: invoke
/// reads the clock once (admission), each poll once, so the deadline
/// (BASE+30) fires within the suite's 24-probe ceiling.
const CLOCK_STEP_SECONDS: u64 = 2;
/// The child writes its document once this many ticks have passed. The
/// tick budget: invoke reads the clock once (the RES-D12 admission
/// instant), then each poll once. The suite's probe-count case is
/// `probes_before_result: 1`: the write must land AFTER that poll's file
/// read but DURING its clock read, so the first poll reports in-flight
/// work and the next completes — exactly "work finished between polls".
const CHILD_WRITE_TICK: u64 = 4;

/// The anchored contract file path for the suite's dispatch facts — the
/// runner's RES-D3 rule, mirrored here so the "child" writes where the
/// runner will read.
fn contract_file_for(state_dir: &Path) -> PathBuf {
    state_dir
        .join("runs")
        .join(SUITE_ULID)
        .join("inv")
        .join(SUITE_ULID)
        .join(SUITE_CONTRACT_LEAF)
}

/// The admission handle the harness's program would record.
fn suite_admission() -> AdmissionHandle {
    AdmissionHandle {
        rlm_child_id: "sub-suite01".to_owned(),
        name: format!("builder-{SUITE_ULID}"),
        session_dir: "/sessions/suite-01".to_owned(),
        model: "suite-model".to_owned(),
    }
}

/// A minimal valid `role_result@v1` document for the dispatch facts.
fn valid_document() -> String {
    format!(
        "{{\"schema\":\"role_result@v1\",\"invocation_id\":\"{SUITE_ULID}\",\
         \"run_id\":\"{SUITE_ULID}\",\"iteration_no\":1,\"role\":\"builder\",\
         \"context_envelope_digest\":\"{SUITE_DIGEST}\",\"state\":\"completed\",\
         \"payload_schema\":\"builder_result@v1\",\
         \"payload\":{{\"attempted\":[],\"changed_paths\":[],\"commands_run\":[],\
         \"artifacts\":[],\"claims\":[],\"self_checks\":[],\"blocked_on\":[]}},\
         \"payment\":{{\"input_tokens\":0,\"output_tokens\":0,\"tool_calls\":0,\
         \"wall_seconds\":0,\"cost_usd\":0}},\"unavailable_evidence\":[],\
         \"errors\":[]}}"
    )
}

/// A well-formed envelope carrying ONE unknown envelope field (strict
/// decode rejects it — spec §10.3 rule 4).
fn unknown_envelope_field_document() -> String {
    valid_document().replace("\"errors\":[]", "\"errors\":[],\"fabricated\":true")
}

/// A well-formed envelope carrying an unknown field nested INSIDE
/// `self_checks[0]` (strict decode rejects it at every object layer).
fn unknown_nested_field_document() -> String {
    valid_document().replace(
        "\"self_checks\":[]",
        "\"self_checks\":[{\"command\":\"x\",\"exit_code\":0,\"fabricated\":true}]",
    )
}

/// A valid document carrying a foreign `invocation_id` (spec §10.3 rule 6).
fn foreign_invocation_document() -> String {
    valid_document().replace(SUITE_ULID, "01ARZ3NDEKTSV4RRFFQ69G5FAB")
}

/// A valid document carrying a stale `context_envelope_digest`.
fn foreign_digest_document() -> String {
    valid_document().replace(SUITE_DIGEST, "sha256:a1b2c3d4e5f6a7b8")
}

/// A malformed document (not JSON at all).
fn malformed_document() -> String {
    "this is not json {{{".to_owned()
}

/// The document a script's child writes, or `None` when it never does.
fn document_for(script: RunnerScript) -> Option<String> {
    use RunnerScript::*;
    match script {
        CompleteAfterProbes { .. } | CompleteImmediately => Some(valid_document()),
        NeverWriteResult | ExitWithoutResult => None,
        WriteMalformedResult => Some(malformed_document()),
        WriteUnknownEnvelopeField => Some(unknown_envelope_field_document()),
        WriteUnknownNestedField => Some(unknown_nested_field_document()),
        WriteForeignInvocationId => Some(foreign_invocation_document()),
        WriteForeignEnvelopeDigest => Some(foreign_digest_document()),
    }
}

/// The virtual clock: one step per `now()`, with the child's write
/// hooked to the tick (time passing is the child working).
struct SuiteClock {
    current: Cell<u64>,
    /// The child's per-tick write, or none. The boxed-callback type is
    /// spelled once here; a type alias would obscure the single use.
    #[allow(clippy::type_complexity)]
    on_tick: RefCell<Option<Box<dyn Fn(u64)>>>,
}

impl std::fmt::Debug for SuiteClock {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SuiteClock")
            .field("current", &self.current.get())
            .finish()
    }
}

impl SuiteClock {
    fn new(on_tick: Box<dyn Fn(u64)>) -> Self {
        Self {
            current: Cell::new(0),
            on_tick: RefCell::new(Some(on_tick)),
        }
    }
}

impl Clock for SuiteClock {
    fn now(&self) -> EpochSeconds {
        let next = self.current.get() + CLOCK_STEP_SECONDS;
        self.current.set(next);
        if let Some(fire) = self.on_tick.borrow().as_deref() {
            fire(next);
        }
        EpochSeconds::new(BASE_INSTANT + next)
    }

    fn sleep_until(&self, at: EpochSeconds) -> Result<(), gauntlet_ports::clock::ClockError> {
        self.current.set(
            at.get()
                .saturating_sub(BASE_INSTANT)
                .max(self.current.get()),
        );
        Ok(())
    }
}

/// The transport double: performs the rendered program's ADMISSION side
/// effects and nothing else — writes the handle document (through the
/// same serialization the production program emits), reads it back
/// through the real strict parse, returns the admission only. For
/// `CompleteImmediately` the "child" has already written its result
/// before admission returns.
#[derive(Debug)]
struct ScriptedTransport;

impl KernelTransport for ScriptedTransport {
    fn submit(
        &self,
        submission: &KernelSubmission,
        _deadline: Deadline,
    ) -> Result<(AdmissionHandle, Box<dyn ParentHandle>), TransportError> {
        if let Some(parent) = submission.handle_path.parent() {
            std::fs::create_dir_all(parent).expect("the handle parent dir is creatable");
        }
        std::fs::write(&submission.handle_path, suite_admission().to_document())
            .expect("the handle document is writable");
        let handle = read_admission(&submission.handle_path)?;
        Ok((handle, Box::new(NullParentHandle) as Box<dyn ParentHandle>))
    }
}

/// A transport double that RECORDS the parent-run workspace every
/// submission carries (task26-dcritic-apparatus-repair, RC4 teeth): the
/// admission side effects are the same as [`ScriptedTransport`], but the
/// resolved `submission.workspace` is captured so a test can assert what
/// `invoke` actually anchored the parent's cwd at.
#[derive(Debug)]
struct RecordingTransport {
    workspaces: std::rc::Rc<RefCell<Vec<PathBuf>>>,
}

impl KernelTransport for RecordingTransport {
    fn submit(
        &self,
        submission: &KernelSubmission,
        _deadline: Deadline,
    ) -> Result<(AdmissionHandle, Box<dyn ParentHandle>), TransportError> {
        self.workspaces
            .borrow_mut()
            .push(submission.workspace.clone());
        if let Some(parent) = submission.handle_path.parent() {
            std::fs::create_dir_all(parent).expect("the handle parent dir is creatable");
        }
        std::fs::write(&submission.handle_path, suite_admission().to_document())
            .expect("the handle document is writable");
        let handle = read_admission(&submission.handle_path)?;
        Ok((handle, Box::new(NullParentHandle) as Box<dyn ParentHandle>))
    }
}

/// The control double: records stops, answers listings.
#[derive(Debug, Default)]
struct ScriptedControl {
    stopped: RefCell<Vec<String>>,
}

impl HarnessControl for ScriptedControl {
    fn stop(&self, child: &str, _deadline: Deadline) -> Result<(), ControlError> {
        self.stopped.borrow_mut().push(child.to_owned());
        Ok(())
    }

    fn list(&self, _deadline: Deadline) -> Result<Vec<SubagentRecord>, ControlError> {
        Ok(Vec::new())
    }
}

/// Per-build scratch: a fresh state root the runner anchors IO under.
fn scratch_dir(tag: &str) -> PathBuf {
    use std::sync::atomic::{AtomicU32, Ordering};
    static SEQUENCE: AtomicU32 = AtomicU32::new(0);
    let dir = std::env::temp_dir().join(format!(
        "gauntlet-prime-suite-{tag}-{}-{}",
        std::process::id(),
        SEQUENCE.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::remove_dir_all(&dir).ok();
    std::fs::create_dir_all(&dir).expect("the suite scratch is creatable");
    dir
}

/// The suite-facing factory: builds an ISOLATED runner per case (spec
/// §7.3), each with its own scratch root, virtual clock, and doubles.
pub struct PrimeRunnerFactory;

impl AgentRunnerFactory for PrimeRunnerFactory {
    fn label(&self) -> &'static str {
        "prime-agent-adapter"
    }

    fn build(&self, script: RunnerScript) -> Result<Box<dyn AgentRunner>, String> {
        let state_dir = scratch_dir("contract");
        let contract_file = contract_file_for(&state_dir);
        // The child's write, driven by the runner's own clock reads.
        let writer_state = contract_file.clone();
        let clock = SuiteClock::new(Box::new(move |tick: u64| {
            if tick >= CHILD_WRITE_TICK
                && let Some(document) = document_for(script)
                && !writer_state.exists()
            {
                if let Some(parent) = writer_state.parent() {
                    std::fs::create_dir_all(parent).expect("the inv dir is creatable");
                }
                std::fs::write(&writer_state, document).expect("the result file is writable");
            }
        }));
        let runner = PrimeAgentRunner::new(
            PrimeAdapterConfig::default(),
            state_dir,
            scratch_dir("cwd"),
            ScriptedTransport,
            ScriptedControl::default(),
            Arc::new(clock),
        );
        Ok(Box::new(runner))
    }
}

/// The unmodified Task-4 `AgentRunner` contract suite against the
/// prime-agent adapter with the in-repo instant probe budget. Empty
/// findings = pass.
#[test]
fn the_prime_runner_passes_the_unmodified_shared_contract_suite() {
    // AK-009..AK-012 re-evidenced on a second, real adapter: admission
    // before completion, terminal-once stability, the TimedOut /
    // malformed_result / contract_mismatch classifications, idempotent
    // cancellation, and stable typed capabilities.
    let findings = agent_runner_suite(&PrimeRunnerFactory, ProbeBudget::INSTANT);
    assert!(
        findings.is_empty(),
        "the prime-agent runner must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

// ---------------------------------------------------------------------------
// RES-D4 evidence: the admission-failure taxonomy and the orphan sweep.
// ---------------------------------------------------------------------------

/// A transport double that fails admission the way a parent run does when
/// the program dies before writing the handle: handle absent.
#[derive(Debug)]
struct HandlelessTransport;
impl KernelTransport for HandlelessTransport {
    fn submit(
        &self,
        _submission: &KernelSubmission,
        _deadline: Deadline,
    ) -> Result<(AdmissionHandle, Box<dyn ParentHandle>), TransportError> {
        Err(TransportError::HandleAbsent {
            detail: "the parent run ended without a handle".to_owned(),
        })
    }
}

/// A control double that reports one live child under the invocation's
/// deterministic name, so the sweep has something to find and stop.
#[derive(Debug)]
struct OrphanedChildControl {
    stops: RefCell<Vec<String>>,
}
impl HarnessControl for OrphanedChildControl {
    fn stop(&self, child: &str, _deadline: Deadline) -> Result<(), ControlError> {
        self.stops.borrow_mut().push(child.to_owned());
        Ok(())
    }
    fn list(&self, _deadline: Deadline) -> Result<Vec<SubagentRecord>, ControlError> {
        // The deterministic invocation name (role + run short + invocation
        // short) — the exact name the sweep searches by.
        Ok(vec![SubagentRecord {
            session_name: "builder-9G5FAV-9G5FAV".to_owned(),
            status: "running".to_owned(),
            session_dir: "/sessions/orphan".to_owned(),
        }])
    }
}

/// A handle-absent admission failure is an AdmissionRefused carrying the
/// orphan sweep's outcome, and the sweep stops the discovered child.
#[test]
fn a_handle_absent_admission_sweeps_the_orphaned_child() {
    let runner = PrimeAgentRunner::new(
        PrimeAdapterConfig::default(),
        scratch_dir("sweep-state"),
        scratch_dir("sweep-cwd"),
        HandlelessTransport,
        OrphanedChildControl {
            stops: RefCell::new(Vec::new()),
        },
        Arc::new(SuiteClock::new(Box::new(|_| {}))),
    );
    let spec = crate_spec();
    let error = runner.invoke(spec).expect_err("admission fails");
    let AgentError::AdmissionRefused { detail } = &error else {
        panic!("handle-absent must be AdmissionRefused, got {error:?}")
    };
    assert!(detail.contains("orphan sweep"), "{detail}");
    // The swept child is the one the listing reported under the
    // deterministic invocation name (role-run-invocation shorts).
    assert!(detail.contains("builder-9G5FAV-9G5FAV"), "{detail}");
    assert!(detail.contains("stopped discovered child"), "{detail}");
}

/// A transport-level (binary/parent spawn) failure is the port's
/// `Transport` class — the caller may retry against a fixed environment.
#[test]
fn a_spawn_failure_is_the_transport_class() {
    #[derive(Debug)]
    struct UnspawnableTransport;
    impl KernelTransport for UnspawnableTransport {
        fn submit(
            &self,
            _submission: &KernelSubmission,
            _deadline: Deadline,
        ) -> Result<(AdmissionHandle, Box<dyn ParentHandle>), TransportError> {
            Err(TransportError::SpawnFailed {
                binary: PathBuf::from("binary"),
                detail: "no such file".to_owned(),
            })
        }
    }
    let runner = PrimeAgentRunner::new(
        PrimeAdapterConfig::default(),
        scratch_dir("spawn-state"),
        scratch_dir("spawn-cwd"),
        UnspawnableTransport,
        ScriptedControl::default(),
        Arc::new(SuiteClock::new(Box::new(|_| {}))),
    );
    let error = runner.invoke(crate_spec()).expect_err("admission fails");
    assert!(
        matches!(error, AgentError::Transport { .. }),
        "spawn failures are Transport, got {error:?}"
    );
}

/// The dispatch spec the direct (non-suite) tests dispatch — the same
/// facts the suite pins.
fn crate_spec() -> gauntlet_ports::agent_runner::RoleInvocationSpec {
    use gauntlet_domain::{
        Budget, DiagnosticAllowance, Digest, InvocationId, Limits, MicroUsd, PermissionSetRef,
        ResultContractPath, RoleId, RunId, WorkspaceRef,
    };
    use gauntlet_ports::agent_runner::{ContextEnvelope, RoleInvocationSpec};
    let budget = Budget::new(
        Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("limits usable"),
        20,
        DiagnosticAllowance::new(100, MicroUsd::new(50)),
    )
    .expect("budget usable");
    RoleInvocationSpec {
        invocation_id: InvocationId::new(SUITE_ULID).expect("valid id"),
        run_id: RunId::new(SUITE_ULID).expect("valid id"),
        iteration_no: gauntlet_domain::IterationNo::new(1).expect("1-based"),
        role: RoleId::Builder,
        prompt: "run the assigned slice".to_owned(),
        context_envelope: ContextEnvelope::new(
            "allowlisted context".to_owned(),
            Digest::parse(SUITE_DIGEST).expect("valid digest"),
        ),
        workspace: WorkspaceRef::new("ws-runner").expect("valid ref"),
        permissions: PermissionSetRef::new("perm-readonly").expect("valid ref"),
        result_contract_path: ResultContractPath::new(SUITE_CONTRACT_LEAF).expect("valid ref"),
        deadline: Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + 30)),
        model_hint: None,
        sandbox_policy: None,
        budget_slice: budget,
    }
}

/// `crate_spec` with a distinct dispatch identity and workspace ref — for
/// the RC4 anchoring teeth below, which need two admittable invocations
/// (duplicate `invocation_id` is refused).
fn spec_with(id: &str, workspace: &str) -> gauntlet_ports::agent_runner::RoleInvocationSpec {
    let mut spec = crate_spec();
    spec.invocation_id = gauntlet_domain::InvocationId::new(id).expect("valid id");
    spec.run_id = gauntlet_domain::RunId::new(id).expect("valid id");
    spec.workspace = gauntlet_domain::WorkspaceRef::new(workspace).expect("valid ref");
    spec
}

// ---------------------------------------------------------------------------
// RC4 teeth (task26-dcritic-apparatus-repair): the parent's cwd is the
// admitted PER-UNIT workspace, never a fixed root; distinct workspaces get
// distinct cwds; a hostile/escaping segment is a typed refusal with zero
// spawn.
// ---------------------------------------------------------------------------

/// The parent run's cwd is `<workspace_root>/<spec.workspace>/` — never the
/// fixed root the runner used to start every parent in before this
/// correction (which left the builder writing into the project root
/// instead of its admitted workspace).
#[test]
fn the_parent_cwd_is_the_per_unit_workspace_not_a_shared_root() {
    let workspaces = std::rc::Rc::new(RefCell::new(Vec::new()));
    let transport = RecordingTransport {
        workspaces: std::rc::Rc::clone(&workspaces),
    };
    let ws_root = scratch_dir("rc4-anchor-ws-root");
    let runner = PrimeAgentRunner::new(
        PrimeAdapterConfig::default(),
        scratch_dir("rc4-anchor-state"),
        ws_root.clone(),
        transport,
        ScriptedControl::default(),
        Arc::new(SuiteClock::new(Box::new(|_| {}))),
    );
    runner.invoke(crate_spec()).expect("admission");
    let expected = ws_root.join("ws-runner");
    let recorded = workspaces.borrow();
    assert_eq!(recorded.len(), 1, "exactly one submission");
    assert_eq!(
        recorded[0], expected,
        "the parent run must start inside the admitted per-unit workspace, not a shared root"
    );
    assert!(expected.is_dir(), "the execution anchor exists");
}

/// Two invocations dispatched against different `spec.workspace` refs get
/// two DIFFERENT parent cwds, both under the same canonical root.
#[test]
fn two_invocations_with_different_workspace_refs_get_different_parent_cwds() {
    let workspaces = std::rc::Rc::new(RefCell::new(Vec::new()));
    let transport = RecordingTransport {
        workspaces: std::rc::Rc::clone(&workspaces),
    };
    let ws_root = scratch_dir("rc4-two-inv-ws-root");
    let runner = PrimeAgentRunner::new(
        PrimeAdapterConfig::default(),
        scratch_dir("rc4-two-inv-state"),
        ws_root.clone(),
        transport,
        ScriptedControl::default(),
        Arc::new(SuiteClock::new(Box::new(|_| {}))),
    );
    runner
        .invoke(spec_with(SUITE_ULID, "ws-one"))
        .expect("first admission");
    runner
        .invoke(spec_with("01ARZ3NDEKTSV4RRFFQ69G5FAB", "ws-two"))
        .expect("second admission");
    let recorded = workspaces.borrow();
    assert_eq!(recorded.len(), 2, "exactly two submissions");
    assert_eq!(recorded[0], ws_root.join("ws-one"));
    assert_eq!(recorded[1], ws_root.join("ws-two"));
    assert_ne!(
        recorded[0], recorded[1],
        "distinct workspace refs must resolve to distinct parent cwds"
    );
}

/// A workspace ref outside the single-component key class is a typed
/// ADMISSION refusal — the anchor never joins a separator or traversal
/// segment onto the root, nothing is left behind, and the transport is
/// NEVER reached (zero spawn).
#[test]
fn a_hostile_workspace_ref_refuses_admission_with_zero_spawn() {
    let workspaces = std::rc::Rc::new(RefCell::new(Vec::new()));
    let transport = RecordingTransport {
        workspaces: std::rc::Rc::clone(&workspaces),
    };
    let state_dir = scratch_dir("rc4-hostile-state");
    let runner = PrimeAgentRunner::new(
        PrimeAdapterConfig::default(),
        state_dir.clone(),
        scratch_dir("rc4-hostile-ws-root"),
        transport,
        ScriptedControl::default(),
        Arc::new(SuiteClock::new(Box::new(|_| {}))),
    );
    // A slashed traversal AND the bare dot segments: `..` and `.` are
    // single, slash-free components made only of class-legal bytes, so a
    // character-class check alone admits them — yet `root.join("..")`
    // escapes. Each must be refused by name.
    for reference in ["seg/../escape", "..", "."] {
        let mut hostile = crate_spec();
        hostile.workspace = gauntlet_domain::WorkspaceRef::new(reference)
            .expect("the ref grammar admits the string");
        let error = runner.invoke(hostile).expect_err("admission refuses");
        let AgentError::AdmissionRefused { detail } = &error else {
            panic!("hostile workspace ref {reference:?} must be AdmissionRefused, got {error:?}")
        };
        assert!(detail.contains("key class"), "{reference:?}: {detail}");
    }
    assert!(
        workspaces.borrow().is_empty(),
        "a refused workspace ref must never reach the transport — zero spawn"
    );
    assert!(
        !state_dir.join("runs").exists(),
        "a ref refusal leaves nothing behind"
    );
}

/// A cancel on an invocation this runner never admitted is the typed
/// refusal (the shared suite already covers admitted handles; this pins
/// the runner's own path end to end over the doubles).
#[test]
fn an_unknown_handle_is_refused_by_every_operation() {
    let runner = PrimeAgentRunner::new(
        PrimeAdapterConfig::default(),
        scratch_dir("stranger-state"),
        scratch_dir("stranger-cwd"),
        ScriptedTransport,
        ScriptedControl::default(),
        Arc::new(SuiteClock::new(Box::new(|_| {}))),
    );
    let stranger = gauntlet_ports::agent_runner::InvocationHandle::new(
        gauntlet_domain::InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FZZ").expect("distinct id"),
        None,
    );
    let bound = Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + 30));
    assert!(matches!(
        runner.poll(&stranger),
        Err(AgentError::UnknownHandle { .. })
    ));
    assert!(matches!(
        runner.collect(&stranger, bound),
        Err(AgentError::UnknownHandle { .. })
    ));
    assert!(matches!(
        runner.cancel(
            &stranger,
            CancelReason::new("test: stray").expect("non-empty"),
            bound
        ),
        Err(AgentError::UnknownHandle { .. })
    ));
    assert!(runner.forensic_record(&stranger).is_none());
}

// ---------------------------------------------------------------------------
// COR-D3 teeth (falsifier round 1: F1 end-to-end, F2, F3). Tests only:
// every mechanism here is already correct; these are the committed tests
// that must FAIL when the guarantees regress.
// ---------------------------------------------------------------------------

/// F1 end-to-end: a transport double whose parent run wrote an
/// ANSWER-SHAPED unknown field into the handle document. The strict
/// parse must refuse it, and `invoke` must surface the typed
/// `AdmissionRefused` (mutation M2 admitted this end-to-end).
#[derive(Debug)]
struct AnswerShapedHandleTransport;
impl KernelTransport for AnswerShapedHandleTransport {
    fn submit(
        &self,
        submission: &KernelSubmission,
        _deadline: Deadline,
    ) -> Result<(AdmissionHandle, Box<dyn ParentHandle>), TransportError> {
        if let Some(parent) = submission.handle_path.parent() {
            std::fs::create_dir_all(parent).expect("the handle parent dir is creatable");
        }
        std::fs::write(
            &submission.handle_path,
            r#"{"rlm_child_id":"sub-1","name":"probe","session_dir":"/s","model":"m","answer":"the result is 42"}"#,
        )
        .expect("the hostile handle document is writable");
        let handle = read_admission(&submission.handle_path)?;
        Ok((handle, Box::new(NullParentHandle) as Box<dyn ParentHandle>))
    }
}

#[test]
fn an_answer_shaped_handle_field_refuses_admission_end_to_end() {
    let runner = PrimeAgentRunner::new(
        PrimeAdapterConfig::default(),
        scratch_dir("f1-state"),
        scratch_dir("f1-cwd"),
        AnswerShapedHandleTransport,
        ScriptedControl::default(),
        Arc::new(SuiteClock::new(Box::new(|_| {}))),
    );
    let error = runner.invoke(crate_spec()).expect_err("admission refuses");
    let AgentError::AdmissionRefused { detail } = &error else {
        panic!("the hostile handle must be an admission refusal, got {error:?}")
    };
    assert!(detail.contains("answer"), "{detail}");
}

/// F2: cancel's "never a second effect" guarantee, seen through a
/// control double that COUNTS stops. The frozen suite cannot see side
/// effects (mutation M7 re-issued the harness stop on the second cancel
/// and the whole battery stayed green); this test is the only teeth.
#[derive(Debug)]
struct CountingControl {
    /// The stop log, shared with the test (the control itself is moved
    /// into the runner).
    stops: std::rc::Rc<RefCell<Vec<String>>>,
}
impl HarnessControl for CountingControl {
    fn stop(&self, child: &str, _deadline: Deadline) -> Result<(), ControlError> {
        self.stops.borrow_mut().push(child.to_owned());
        Ok(())
    }
    fn list(&self, _deadline: Deadline) -> Result<Vec<SubagentRecord>, ControlError> {
        Ok(Vec::new())
    }
}

#[test]
fn cancel_issues_exactly_one_stop_ever() {
    let stops = std::rc::Rc::new(RefCell::new(Vec::new()));
    let control = CountingControl {
        stops: std::rc::Rc::clone(&stops),
    };
    let runner = PrimeAgentRunner::new(
        PrimeAdapterConfig::default(),
        scratch_dir("f2-state"),
        scratch_dir("f2-cwd"),
        ScriptedTransport,
        control,
        Arc::new(SuiteClock::new(Box::new(|_| {}))),
    );
    let handle = runner.invoke(crate_spec()).expect("admission");
    let reason = CancelReason::new("teeth: superseded").expect("non-empty");
    let bound = Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + 30));
    runner
        .cancel(&handle, reason.clone(), bound)
        .expect("first cancel");
    // The second cancel is the TYPED NO-OP with ZERO control effects.
    match runner.cancel(&handle, reason, bound) {
        Err(AgentError::AlreadyTerminal { .. }) => {}
        other => panic!("the second cancel must be the typed no-op, got {other:?}"),
    }
    // And a third, for good measure: still zero additional effects.
    let _ = runner.cancel(
        &handle,
        CancelReason::new("teeth: again").expect("non-empty"),
        bound,
    );
    let log = stops.borrow();
    assert_eq!(
        log.len(),
        1,
        "exactly ONE harness stop may ever be issued: {log:?}"
    );
    assert_eq!(log[0], "sub-suite01");
}

/// F3: the late-writer resurrection shape (mutation M1's exact attack):
/// a schema-valid, identity-MATCHING document arriving AFTER the
/// TimedOut verdict must never resurrect the invocation.
#[test]
fn a_late_identity_matching_writer_never_resurrects_a_timed_out_invocation() {
    let state_dir = scratch_dir("f3-state");
    let clock = SuiteClock::new(Box::new(|_| {}));
    let runner = PrimeAgentRunner::new(
        PrimeAdapterConfig::default(),
        state_dir.clone(),
        scratch_dir("f3-cwd"),
        ScriptedTransport,
        ScriptedControl::default(),
        Arc::new(clock),
    );
    let handle = runner.invoke(crate_spec()).expect("admission");
    // Advance the virtual clock past the deadline by polling: the
    // deadline (BASE+30) fires at tick 32 — poll 15.
    let mut timed_out = false;
    for _ in 0..20 {
        if let InvocationStatus::TimedOut = runner.poll(&handle).expect("poll while pending") {
            timed_out = true;
            break;
        }
    }
    assert!(timed_out, "the deadline must fire within the poll bound");
    // NOW the late writer lands a schema-valid, identity-MATCHING
    // document at the declared contract path.
    let contract_file = contract_file_for(&state_dir);
    std::fs::write(&contract_file, valid_document()).expect("the late document is writable");
    // The verdict stays TimedOut — verbatim, forever.
    for _ in 0..3 {
        match runner.poll(&handle).expect("poll after the late write") {
            InvocationStatus::TimedOut => {}
            other => panic!("a late writer resurrected the verdict: {other:?}"),
        }
    }
    // And the late document is never collectible.
    let error = runner
        .collect(
            &handle,
            Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + 30)),
        )
        .expect_err("a TimedOut invocation is not collectible");
    assert!(
        matches!(
            error,
            AgentError::ResultNotReady {
                state: gauntlet_domain::InvocationState::TimedOut,
                ..
            }
        ),
        "got {error:?}"
    );
}

// ---------------------------------------------------------------------------
// RC1 teeth (task26-dcritic-apparatus-repair): the invocation, never the
// transport, owns the parent process's lifetime. These are the committed
// tests that must FAIL if the runner regresses to disposing of the parent
// at admission, or fails to dispose of it at a terminal transition.
// ---------------------------------------------------------------------------

/// A [`ParentHandle`] double that records whether `terminate` was called
/// and can be told to report an early exit through `try_reap` — for the
/// RC1 teeth below.
#[derive(Debug)]
struct RecordingParentHandle {
    terminated: std::rc::Rc<Cell<bool>>,
    early_exit_note: Option<&'static str>,
    reaped: bool,
}

impl ParentHandle for RecordingParentHandle {
    fn try_reap(&mut self) -> Option<String> {
        if self.reaped {
            return None;
        }
        let note = self.early_exit_note?;
        self.reaped = true;
        Some(note.to_owned())
    }

    fn terminate(&mut self) -> String {
        self.terminated.set(true);
        "terminated (recording double)".to_owned()
    }

    fn telemetry(&self) -> gauntlet_adapter_agent_prime::transport::ParentTelemetry {
        gauntlet_adapter_agent_prime::transport::ParentTelemetry::default()
    }
}

/// A transport whose admission handle is the suite's, but whose parent
/// handle is the recording double above.
#[derive(Debug)]
struct RecordingParentTransport {
    terminated: std::rc::Rc<Cell<bool>>,
    early_exit_note: Option<&'static str>,
}

impl KernelTransport for RecordingParentTransport {
    fn submit(
        &self,
        submission: &KernelSubmission,
        _deadline: Deadline,
    ) -> Result<(AdmissionHandle, Box<dyn ParentHandle>), TransportError> {
        if let Some(parent) = submission.handle_path.parent() {
            std::fs::create_dir_all(parent).expect("the handle parent dir is creatable");
        }
        std::fs::write(&submission.handle_path, suite_admission().to_document())
            .expect("the handle document is writable");
        let handle = read_admission(&submission.handle_path)?;
        Ok((
            handle,
            Box::new(RecordingParentHandle {
                terminated: std::rc::Rc::clone(&self.terminated),
                early_exit_note: self.early_exit_note,
                reaped: false,
            }),
        ))
    }
}

/// The runner must NEVER dispose of the parent at admission — that was
/// the RC1 defect: the transport's earlier "detached bounded reaper"
/// stopped the parent shortly after `invoke`, aborting real child work.
#[test]
fn invoke_does_not_dispose_of_the_parent_at_admission() {
    let terminated = std::rc::Rc::new(Cell::new(false));
    let transport = RecordingParentTransport {
        terminated: std::rc::Rc::clone(&terminated),
        early_exit_note: None,
    };
    let runner = PrimeAgentRunner::new(
        PrimeAdapterConfig::default(),
        scratch_dir("rc1-invoke-state"),
        scratch_dir("rc1-invoke-cwd"),
        transport,
        ScriptedControl::default(),
        Arc::new(SuiteClock::new(Box::new(|_| {}))),
    );
    let _handle = runner.invoke(crate_spec()).expect("admission");
    assert!(
        !terminated.get(),
        "invoke must hand the parent's lifetime to the runner, never dispose of it at admission"
    );
}

/// `cancel` terminates the parent process, and an already-gone harness
/// child (`NoSuchChild`, RC3's routine case for prime-agent 0.8.0's
/// non-daemon-registered rlm children) still counts as a successful
/// cancel.
#[test]
fn cancel_terminates_the_parent_and_treats_an_already_gone_child_as_success() {
    let terminated = std::rc::Rc::new(Cell::new(false));
    let transport = RecordingParentTransport {
        terminated: std::rc::Rc::clone(&terminated),
        early_exit_note: None,
    };
    #[derive(Debug)]
    struct AlreadyGoneControl;
    impl HarnessControl for AlreadyGoneControl {
        fn stop(&self, child: &str, _deadline: Deadline) -> Result<(), ControlError> {
            Err(ControlError::NoSuchChild {
                child: child.to_owned(),
            })
        }
        fn list(&self, _deadline: Deadline) -> Result<Vec<SubagentRecord>, ControlError> {
            Ok(Vec::new())
        }
    }
    let runner = PrimeAgentRunner::new(
        PrimeAdapterConfig::default(),
        scratch_dir("rc1-cancel-state"),
        scratch_dir("rc1-cancel-cwd"),
        transport,
        AlreadyGoneControl,
        Arc::new(SuiteClock::new(Box::new(|_| {}))),
    );
    let handle = runner.invoke(crate_spec()).expect("admission");
    let reason = CancelReason::new("teeth: already-gone child").expect("non-empty");
    let bound = Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + 30));
    runner
        .cancel(&handle, reason, bound)
        .expect("an already-gone harness child is a successful cancel");
    assert!(
        terminated.get(),
        "cancel must terminate the parent process even when the harness child is already gone"
    );
}

/// A parent that exited EARLY (before any result document exists and
/// before the deadline has passed) must never be read as `Completed` or
/// `Failed` — completion derives ONLY from the result contract, and
/// failure ONLY from a malformed/mismatched one (Annex C.2/C.4).
#[test]
fn a_parent_exiting_early_never_produces_completed_or_failed_before_the_contract_or_deadline() {
    let transport = RecordingParentTransport {
        terminated: std::rc::Rc::new(Cell::new(false)),
        early_exit_note: Some("parent exited unexpectedly (test double)"),
    };
    let runner = PrimeAgentRunner::new(
        PrimeAdapterConfig::default(),
        scratch_dir("rc1-early-exit-state"),
        scratch_dir("rc1-early-exit-cwd"),
        transport,
        ScriptedControl::default(),
        // No document is ever written by this clock's on_tick (a no-op),
        // so only the parent's early exit and the (far-off) deadline are
        // in play.
        Arc::new(SuiteClock::new(Box::new(|_| {}))),
    );
    let handle = runner.invoke(crate_spec()).expect("admission");
    // invoke's clock read landed at tick 2; this poll's lands at tick 4
    // (BASE_INSTANT+4) — nowhere near the deadline (BASE_INSTANT+30).
    match runner.poll(&handle).expect("poll must not error") {
        InvocationStatus::Running { .. } => {}
        other => panic!(
            "a parent that exited early must never produce a terminal status before the \
             contract or the deadline decides, got {other:?}"
        ),
    }
}
