//! Shared deterministic doubles and builders for the delegation tests and the
//! agents service tests: the scripted runner, the probe graph, the lab wiring,
//! and the request, policy, and envelope builders.

use std::cell::{Cell, RefCell};
use std::collections::BTreeSet;
use std::rc::Rc;

use gauntlet_domain::{
    AdmissionKey, Budget, DiagnosticAllowance, Digest, EpochSeconds, HarnessId, InvocationId,
    IterationNo, Limits, MicroUsd, Payment, PermissionSetRef, ResultContractPath, RoleId, RunId,
    RunnerProfileId, SandboxPolicyRef, SessionRef, WorkspaceRef,
};
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, CancelReason, InvocationFailure, Isolation, ResultDelivery,
    RoleInvocationSpec, RunnerCapabilities,
};
use gauntlet_ports::contract::invocation_graph_reference::InMemoryInvocationGraph;
use gauntlet_ports::invocation_graph::{
    AdmissionOutcome, AdmissionRequest, GraphError, GraphLimits, InvocationGraph, InvocationNode,
    NativeSessionBinding, NodeStatus, ResultBinding,
};
use gauntlet_ports::lifecycle::{
    HarnessObservation, LifecycleView, ObservationOutcome, ObservationRecord,
};
use gauntlet_ports::role_result::RoleResult;
use gauntlet_ports::{BudgetSlice, Deadline, InvocationHandle, InvocationStatus};

use super::DelegationBroker;
use super::broker::{DelegationRequest, RunnerSet};
use super::context::{BODY_HEADER, ContextItem, ContextItemKind, ContextPolicy};
use crate::fabric::selection::{Preference, Requirements};
use crate::fabric::{
    Availability, FabricCapabilities, LifecycleSource, ModelFacts, RunnerInstance, RunnerProfile,
    RunnerRegistry, SupportLevel,
};

// Identifiers: 26-character Crockford tags, left-padded with zeros.
pub(crate) const RUN_TAG: &str = "RNA1";
pub(crate) const ROOT_TAG: &str = "RT01";
pub(crate) const CHILD_TAG: &str = "CD01";
pub(crate) const SECOND_CHILD_TAG: &str = "CD02";
pub(crate) const THIRD_CHILD_TAG: &str = "CD03";
pub(crate) const GRANDCHILD_TAG: &str = "GR01";
pub(crate) const GREAT_GRANDCHILD_TAG: &str = "GR02";
pub(crate) const UNKNOWN_TAG: &str = "XX99";

pub(crate) const KEY_ROOT: &str = "key-root";
pub(crate) const KEY_CHILD: &str = "key-child";

// Budget slices. Each child slice fits its parent; sums stay within the parent.
pub(crate) const ROOT_ROUNDS: u32 = 8;
pub(crate) const ROOT_WALL: u64 = 3_600;
pub(crate) const ROOT_TOKENS: u64 = 80_000;
pub(crate) const ROOT_COST: u64 = 800_000;
pub(crate) const CHILD_ROUNDS: u32 = 4;
pub(crate) const CHILD_WALL: u64 = 1_800;
pub(crate) const CHILD_TOKENS: u64 = 40_000;
pub(crate) const CHILD_COST: u64 = 400_000;
pub(crate) const GRAND_ROUNDS: u32 = 2;
pub(crate) const GRAND_WALL: u64 = 900;
pub(crate) const GRAND_TOKENS: u64 = 20_000;
pub(crate) const GRAND_COST: u64 = 200_000;

pub(crate) const MAX_DEPTH: u32 = 2;
pub(crate) const MAX_CHILDREN: u32 = 2;
pub(crate) const PARALLEL: u32 = 2;
pub(crate) const RESERVE_PCT: u8 = 20;
pub(crate) const DIAG_TOKENS: u64 = 1_000;
pub(crate) const DIAG_COST: u64 = 50;
pub(crate) const ITERATION: u32 = 1;
pub(crate) const ADMIT_AT: u64 = 100;
pub(crate) const FAR_AT: u64 = 1_000_000;

pub(crate) const TASK_TEXT: &str = "review the change against the reference";
pub(crate) const REF_TEXT: &str = "the reference excerpt";
pub(crate) const OTHER_REF_TEXT: &str = "a different reference excerpt";
pub(crate) const CONSTRAINT_TEXT: &str = "do not touch the network";
pub(crate) const ROOT_MARKER: &str = "root-marker";
pub(crate) const CHILD_MARKER: &str = "child-marker";
pub(crate) const GRAND_MARKER: &str = "grandchild-marker";

pub(crate) fn ulid(tag: &str) -> String {
    format!("{tag:0>26}")
}

pub(crate) fn run_id() -> RunId {
    RunId::new(&ulid(RUN_TAG)).expect("valid run id")
}

pub(crate) fn inv(tag: &str) -> InvocationId {
    InvocationId::new(&ulid(tag)).expect("valid invocation id")
}

pub(crate) fn profile_id(profile: &str) -> RunnerProfileId {
    RunnerProfileId::new(profile).expect("valid profile id")
}

pub(crate) fn deadline() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(FAR_AT))
}

pub(crate) fn root_slice() -> BudgetSlice {
    BudgetSlice {
        rounds: ROOT_ROUNDS,
        wall_seconds: ROOT_WALL,
        tokens: ROOT_TOKENS,
        cost_usd: MicroUsd::new(ROOT_COST),
    }
}

pub(crate) fn child_slice() -> BudgetSlice {
    BudgetSlice {
        rounds: CHILD_ROUNDS,
        wall_seconds: CHILD_WALL,
        tokens: CHILD_TOKENS,
        cost_usd: MicroUsd::new(CHILD_COST),
    }
}

pub(crate) fn grand_slice() -> BudgetSlice {
    BudgetSlice {
        rounds: GRAND_ROUNDS,
        wall_seconds: GRAND_WALL,
        tokens: GRAND_TOKENS,
        cost_usd: MicroUsd::new(GRAND_COST),
    }
}

pub(crate) fn spec_budget() -> Budget {
    Budget::new(
        Limits::new(
            ROOT_ROUNDS,
            PARALLEL,
            MAX_DEPTH,
            ROOT_WALL,
            ROOT_TOKENS,
            MicroUsd::new(ROOT_COST),
        )
        .expect("test limits are usable"),
        RESERVE_PCT,
        DiagnosticAllowance::new(DIAG_TOKENS, MicroUsd::new(DIAG_COST)),
    )
    .expect("test budget is usable")
}

pub(crate) fn all_kinds() -> BTreeSet<ContextItemKind> {
    [
        ContextItemKind::TaskStatement,
        ContextItemKind::ReferenceExcerpt,
        ContextItemKind::FileExcerpt,
        ContextItemKind::EvidenceReference,
        ContextItemKind::ConstraintStatement,
        ContextItemKind::ResultSchema,
    ]
    .into_iter()
    .collect()
}

pub(crate) fn full_policy() -> ContextPolicy {
    ContextPolicy {
        allow: all_kinds(),
        deny: BTreeSet::new(),
    }
}

pub(crate) fn item(kind: ContextItemKind, label: &str, content: &str) -> ContextItem {
    ContextItem {
        kind,
        label: label.to_owned(),
        content: content.to_owned(),
    }
}

pub(crate) fn task(content: &str) -> ContextItem {
    item(ContextItemKind::TaskStatement, "task", content)
}

pub(crate) fn reference(label: &str, content: &str) -> ContextItem {
    item(ContextItemKind::ReferenceExcerpt, label, content)
}

/// The items a delegation carries for a given marker. Both items mention the
/// marker, so a spec's body can be attributed to its own delegation.
pub(crate) fn items_for(marker: &str) -> Vec<ContextItem> {
    vec![
        task(&format!("task for {marker}")),
        reference("notes", marker),
    ]
}

pub(crate) fn request(
    invocation: &str,
    parent: Option<&str>,
    key: &str,
    items: Vec<ContextItem>,
) -> DelegationRequest {
    DelegationRequest {
        run_id: run_id(),
        parent: parent.map(inv),
        role: RoleId::Builder,
        items,
        policy: full_policy(),
        requirements: Requirements::default(),
        preference: Preference::default(),
        workspace: WorkspaceRef::new("workspace-a").expect("valid workspace"),
        permissions: PermissionSetRef::new("perm-builder").expect("valid permissions"),
        sandbox_policy: SandboxPolicyRef::new("sandbox-readonly").expect("valid sandbox"),
        budget: child_slice(),
        spec_budget: spec_budget(),
        limits: GraphLimits {
            max_depth: MAX_DEPTH,
            max_children_per_parent: MAX_CHILDREN,
        },
        admission_key: AdmissionKey::new(key).expect("valid admission key"),
        invocation_id: inv(invocation),
        result_contract: ResultContractPath::new("delegate-result-document")
            .expect("valid contract path"),
        deadline: deadline(),
        iteration_no: IterationNo::new(ITERATION).expect("1-based iteration"),
        at: EpochSeconds::new(ADMIT_AT),
    }
}

pub(crate) fn root_request(
    invocation: &str,
    key: &str,
    items: Vec<ContextItem>,
) -> DelegationRequest {
    let mut root = request(invocation, None, key, items);
    root.role = RoleId::Orchestrator;
    root.budget = root_slice();
    root
}

pub(crate) fn pinned(mut request: DelegationRequest, profile: &str) -> DelegationRequest {
    request.preference.pin = Some(profile_id(profile));
    request
}

pub(crate) fn capabilities() -> FabricCapabilities {
    FabricCapabilities {
        support_level: SupportLevel::Structured,
        lifecycle_source: LifecycleSource::Process,
        resumable: true,
        model_selection: true,
        native_subagents: true,
        streaming_events: true,
        structured_lifecycle: true,
        fresh_context: true,
        workspace_mutation: true,
        isolation: Isolation::Process,
        max_depth: 8,
    }
}

pub(crate) fn instance(profile: &str, harness: &str) -> RunnerInstance {
    RunnerInstance {
        profile: RunnerProfile {
            id: profile_id(profile),
            harness: HarnessId::new(harness).expect("valid harness id"),
            model: ModelFacts {
                provider: None,
                model: None,
            },
            label: format!("{profile} label"),
        },
        availability: Availability::Available,
        capabilities: capabilities(),
    }
}

/// The order of runner cancels, shared across doubles to observe global order.
pub(crate) type CancelLog = Rc<RefCell<Vec<InvocationId>>>;

/// A runner double. Counts every call and records each spec it was handed. It
/// keeps the status of every handle it admitted, so a fresh double can stand in
/// for a restarted runner that forgot them. It fails the next N invokes after
/// recording them, can drop the session identity, and can fail cancels.
pub(crate) struct ScriptedRunner {
    session: SessionRef,
    invoke_calls: Cell<u32>,
    poll_calls: Cell<u32>,
    collect_calls: Cell<u32>,
    cancel_calls: Cell<u32>,
    failures_left: Cell<u32>,
    omit_session: Cell<bool>,
    fail_cancels: Cell<bool>,
    transport_down: Cell<bool>,
    handles: RefCell<Vec<(InvocationId, InvocationStatus)>>,
    result: RefCell<Option<RoleResult>>,
    cancel_log: RefCell<Option<CancelLog>>,
    received: RefCell<Vec<RoleInvocationSpec>>,
}

impl ScriptedRunner {
    pub(crate) fn new(tag: &str) -> Self {
        Self {
            session: SessionRef::new(&format!("session-{tag}")).expect("valid session ref"),
            invoke_calls: Cell::new(0),
            poll_calls: Cell::new(0),
            collect_calls: Cell::new(0),
            cancel_calls: Cell::new(0),
            failures_left: Cell::new(0),
            omit_session: Cell::new(false),
            fail_cancels: Cell::new(false),
            transport_down: Cell::new(false),
            handles: RefCell::new(Vec::new()),
            result: RefCell::new(None),
            cancel_log: RefCell::new(None),
            received: RefCell::new(Vec::new()),
        }
    }

    pub(crate) fn fail_next_invokes(&self, count: u32) {
        self.failures_left.set(count);
    }

    pub(crate) fn omit_session_identity(&self) {
        self.omit_session.set(true);
    }

    pub(crate) fn fail_cancels(&self) {
        self.fail_cancels.set(true);
    }

    /// While down, `poll` cannot reach the runner and reports a transport error.
    pub(crate) fn set_transport_down(&self, down: bool) {
        self.transport_down.set(down);
    }

    pub(crate) fn share_cancel_log(&self, log: &CancelLog) {
        *self.cancel_log.borrow_mut() = Some(Rc::clone(log));
    }

    /// Sets (or remembers) the status this runner reports for one handle.
    pub(crate) fn set_status(&self, id: &InvocationId, status: InvocationStatus) {
        let mut handles = self.handles.borrow_mut();
        match handles.iter().position(|(known, _)| known == id) {
            Some(index) => handles[index].1 = status,
            None => handles.push((id.clone(), status)),
        }
    }

    pub(crate) fn set_result(&self, result: RoleResult) {
        *self.result.borrow_mut() = Some(result);
    }

    pub(crate) fn invokes(&self) -> u32 {
        self.invoke_calls.get()
    }

    pub(crate) fn polls(&self) -> u32 {
        self.poll_calls.get()
    }

    pub(crate) fn collects(&self) -> u32 {
        self.collect_calls.get()
    }

    pub(crate) fn cancels(&self) -> u32 {
        self.cancel_calls.get()
    }

    pub(crate) fn last_spec(&self) -> RoleInvocationSpec {
        self.received
            .borrow()
            .last()
            .cloned()
            .expect("the runner received a spec")
    }
}

impl AgentRunner for ScriptedRunner {
    fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
        self.invoke_calls.set(self.invoke_calls.get() + 1);
        let invocation_id = spec.invocation_id.clone();
        self.received.borrow_mut().push(spec);
        if self.failures_left.get() > 0 {
            self.failures_left.set(self.failures_left.get() - 1);
            return Err(AgentError::Transport {
                detail: "scripted transport failure".to_owned(),
            });
        }
        self.set_status(&invocation_id, InvocationStatus::Admitted);
        let session = (!self.omit_session.get()).then(|| self.session.clone());
        Ok(InvocationHandle::new(invocation_id, session))
    }

    fn poll(&self, handle: &InvocationHandle) -> Result<InvocationStatus, AgentError> {
        self.poll_calls.set(self.poll_calls.get() + 1);
        let id = handle.invocation_id();
        if self.transport_down.get() {
            return Err(AgentError::Transport {
                detail: "scripted transport outage".to_owned(),
            });
        }
        self.handles
            .borrow()
            .iter()
            .find(|(known, _)| known == id)
            .map(|(_, status)| status.clone())
            .ok_or_else(|| AgentError::UnknownHandle {
                invocation_id: id.clone(),
            })
    }

    fn collect(
        &self,
        handle: &InvocationHandle,
        _deadline: Deadline,
    ) -> Result<RoleResult, AgentError> {
        self.collect_calls.set(self.collect_calls.get() + 1);
        self.result
            .borrow()
            .clone()
            .ok_or_else(|| AgentError::UnknownHandle {
                invocation_id: handle.invocation_id().clone(),
            })
    }

    fn cancel(
        &self,
        handle: &InvocationHandle,
        _reason: CancelReason,
        _deadline: Deadline,
    ) -> Result<(), AgentError> {
        self.cancel_calls.set(self.cancel_calls.get() + 1);
        let id = handle.invocation_id();
        if let Some(log) = self.cancel_log.borrow().as_ref() {
            log.borrow_mut().push(id.clone());
        }
        if self.fail_cancels.get() {
            return Err(AgentError::CancelFailed {
                invocation_id: id.clone(),
                detail: "scripted cancel failure".to_owned(),
            });
        }
        Ok(())
    }

    fn capabilities(&self) -> RunnerCapabilities {
        RunnerCapabilities {
            supports_native_subagents: false,
            supports_streaming_events: false,
            supports_model_selection: false,
            isolation: Isolation::Process,
            result_delivery: ResultDelivery::File,
            max_depth: 0,
        }
    }
}

/// A decorator over the reference graph. It fails `mark_running` a scripted
/// number of times and counts `complete` calls; every other call delegates.
#[derive(Default)]
pub(crate) struct ProbeGraph {
    pub(crate) inner: InMemoryInvocationGraph,
    pub(crate) mark_running_failures: Cell<u32>,
    pub(crate) completes: Cell<u32>,
}

impl InvocationGraph for ProbeGraph {
    fn admit(
        &self,
        request: AdmissionRequest,
        deadline: Deadline,
    ) -> Result<AdmissionOutcome, GraphError> {
        self.inner.admit(request, deadline)
    }

    fn attach_native_session(
        &self,
        id: &InvocationId,
        session: SessionRef,
        deadline: Deadline,
    ) -> Result<NativeSessionBinding, GraphError> {
        self.inner.attach_native_session(id, session, deadline)
    }

    fn mark_running(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<InvocationNode, GraphError> {
        if self.mark_running_failures.get() > 0 {
            self.mark_running_failures
                .set(self.mark_running_failures.get() - 1);
            return Err(GraphError::WriteFailed {
                detail: "scripted write failure".to_owned(),
            });
        }
        self.inner.mark_running(id, deadline)
    }

    fn complete(
        &self,
        id: &InvocationId,
        binding: ResultBinding,
        at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<InvocationNode, GraphError> {
        self.completes.set(self.completes.get() + 1);
        self.inner.complete(id, binding, at, deadline)
    }

    fn fail(
        &self,
        id: &InvocationId,
        class: InvocationFailure,
        at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<InvocationNode, GraphError> {
        self.inner.fail(id, class, at, deadline)
    }

    fn time_out(
        &self,
        id: &InvocationId,
        at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<InvocationNode, GraphError> {
        self.inner.time_out(id, at, deadline)
    }

    fn cancel_subtree(
        &self,
        id: &InvocationId,
        at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<Vec<InvocationId>, GraphError> {
        self.inner.cancel_subtree(id, at, deadline)
    }

    fn node(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<Option<InvocationNode>, GraphError> {
        self.inner.node(id, deadline)
    }

    fn children(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<Vec<InvocationNode>, GraphError> {
        self.inner.children(id, deadline)
    }

    fn descendants(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<Vec<InvocationNode>, GraphError> {
        self.inner.descendants(id, deadline)
    }

    fn ancestors(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<Vec<InvocationNode>, GraphError> {
        self.inner.ancestors(id, deadline)
    }

    fn active_nodes(
        &self,
        run_id: &RunId,
        deadline: Deadline,
    ) -> Result<Vec<InvocationNode>, GraphError> {
        self.inner.active_nodes(run_id, deadline)
    }

    fn record_observation(
        &self,
        observation: HarnessObservation,
        received_at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<ObservationOutcome, GraphError> {
        self.inner
            .record_observation(observation, received_at, deadline)
    }

    fn lifecycle(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<LifecycleView, GraphError> {
        self.inner.lifecycle(id, deadline)
    }

    fn observation_history(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<Vec<ObservationRecord>, GraphError> {
        self.inner.observation_history(id, deadline)
    }
}

pub(crate) fn runner_set(
    registry: &RunnerRegistry,
    doubles: &[(&str, Rc<ScriptedRunner>)],
) -> RunnerSet {
    let entries: Vec<(RunnerProfileId, Rc<dyn AgentRunner>)> = doubles
        .iter()
        .map(|(profile, double)| {
            (
                profile_id(profile),
                Rc::clone(double) as Rc<dyn AgentRunner>,
            )
        })
        .collect();
    RunnerSet::for_registry(registry, entries).expect("runner set matches the registry")
}

/// A graph, registry, and runner set wired together for one test.
pub(crate) struct Lab {
    pub(crate) graph: InMemoryInvocationGraph,
    pub(crate) registry: RunnerRegistry,
    pub(crate) runners: RunnerSet,
}

impl Lab {
    pub(crate) fn new(
        instances: Vec<RunnerInstance>,
        doubles: &[(&str, Rc<ScriptedRunner>)],
    ) -> Self {
        let registry = RunnerRegistry::new(instances).expect("registry profiles are unique");
        let runners = runner_set(&registry, doubles);
        Self {
            graph: InMemoryInvocationGraph::default(),
            registry,
            runners,
        }
    }

    pub(crate) fn broker(&self) -> DelegationBroker<'_> {
        DelegationBroker::new(&self.graph, &self.registry, &self.runners)
    }

    pub(crate) fn node(&self, tag: &str) -> Option<InvocationNode> {
        self.graph
            .node(&inv(tag), deadline())
            .expect("the graph reads")
    }

    pub(crate) fn active_count(&self) -> usize {
        self.graph
            .active_nodes(&run_id(), deadline())
            .expect("the graph reads")
            .len()
    }

    pub(crate) fn descendant_count(&self, tag: &str) -> usize {
        self.graph
            .descendants(&inv(tag), deadline())
            .expect("the graph reads")
            .len()
    }
}

/// Three harnesses, one profile each, one double each.
pub(crate) fn standard_lab() -> (
    Lab,
    Rc<ScriptedRunner>,
    Rc<ScriptedRunner>,
    Rc<ScriptedRunner>,
) {
    let a = Rc::new(ScriptedRunner::new("a"));
    let b = Rc::new(ScriptedRunner::new("b"));
    let c = Rc::new(ScriptedRunner::new("c"));
    let lab = Lab::new(
        vec![
            instance("profile-a", "harness-a"),
            instance("profile-b", "harness-b"),
            instance("profile-c", "harness-c"),
        ],
        &[
            ("profile-a", Rc::clone(&a)),
            ("profile-b", Rc::clone(&b)),
            ("profile-c", Rc::clone(&c)),
        ],
    );
    (lab, a, b, c)
}

/// Decodes a canonical body into (kind, label, content) triples, reading each
/// item by its declared lengths only.
pub(crate) fn decode(body: &str) -> Vec<(String, String, String)> {
    let mut rest = body
        .strip_prefix(BODY_HEADER)
        .expect("the body starts with the format tag");
    let mut items = Vec::new();
    while !rest.is_empty() {
        let (header, after) = rest.split_once('\n').expect("a header line");
        let mut parts = header.split(' ');
        let kind = parts.next().expect("a kind token").to_owned();
        let label_len: usize = parts
            .next()
            .expect("a label length")
            .parse()
            .expect("numeric");
        let content_len: usize = parts
            .next()
            .expect("a content length")
            .parse()
            .expect("numeric");
        let label = &after[..label_len];
        let after = &after[label_len..];
        assert_eq!(after.as_bytes()[0], b'\n', "label terminator");
        let after = &after[1..];
        let content = &after[..content_len];
        let after = &after[content_len..];
        assert_eq!(after.as_bytes()[0], b'\n', "content terminator");
        rest = &after[1..];
        items.push((kind, label.to_owned(), content.to_owned()));
    }
    items
}

pub(crate) fn triple(kind: &str, label: &str, content: &str) -> (String, String, String) {
    (kind.to_owned(), label.to_owned(), content.to_owned())
}

pub(crate) const KEY_SECOND: &str = "key-second";
pub(crate) const KEY_GRAND: &str = "key-grand";
pub(crate) const CONTRACT_PATH: &str = "delegate-result-document";
pub(crate) const OPERATOR_REASON: &str = "operator stop";

pub(crate) fn at() -> EpochSeconds {
    EpochSeconds::new(ADMIT_AT)
}

pub(crate) fn contract_path() -> ResultContractPath {
    ResultContractPath::new(CONTRACT_PATH).expect("valid contract path")
}

pub(crate) fn running_status() -> InvocationStatus {
    InvocationStatus::Running {
        last_event_at: at(),
        payment_so_far: Payment::new(0, 0, 0, MicroUsd::new(0)),
    }
}

pub(crate) fn completed_status() -> InvocationStatus {
    InvocationStatus::Completed {
        result_ref: contract_path(),
    }
}

pub(crate) fn cancel_reason(text: &str) -> CancelReason {
    CancelReason::new(text).expect("a non-empty reason")
}

/// A `role_result@v1` document for a builder, decoded through the real codec.
/// The invocation id and envelope digest are whatever the caller binds.
pub(crate) fn builder_result(
    invocation: &InvocationId,
    digest: &Digest,
    unavailable: bool,
) -> RoleResult {
    let evidence = if unavailable {
        "{\"kind\":\"file\",\"reason\":\"no artifact was produced\"}"
    } else {
        ""
    };
    let text = format!(
        "{{\"schema\":\"role_result@v1\",\"invocation_id\":\"{invocation}\",\"run_id\":\"{run}\",\
         \"iteration_no\":{ITERATION},\"role\":\"builder\",\"context_envelope_digest\":\"{digest}\",\
         \"state\":\"completed\",\"payload_schema\":\"builder_result@v1\",\
         \"payload\":{{\"attempted\":[],\"changed_paths\":[],\"commands_run\":[],\"artifacts\":[],\
         \"claims\":[],\"self_checks\":[],\"blocked_on\":[]}},\
         \"payment\":{{\"input_tokens\":1,\"output_tokens\":1,\"tool_calls\":0,\"wall_seconds\":0,\
         \"cost_usd\":0.0}},\"unavailable_evidence\":[{evidence}],\"errors\":[]}}",
        invocation = invocation.as_str(),
        run = ulid(RUN_TAG),
    );
    RoleResult::decode(&text).expect("the fixture is a valid role_result")
}

pub(crate) fn node_status(lab: &Lab, tag: &str) -> NodeStatus {
    lab.node(tag).expect("the node exists").status
}
