//! Deterministic tests for the agents wire codec and tool surface. The codec is
//! checked at the decode boundary (exact detail paths), and the surface is
//! checked end to end over the same broker, graph, and scripted runner doubles
//! the affordance tests use.

use std::cell::{Cell, RefCell};
use std::collections::{BTreeSet, HashMap};
use std::path::PathBuf;
use std::sync::Arc;

use gauntlet_domain::{
    AdmissionKey, Authority, BlastRadius, EpochSeconds, HarnessId, InvocationId, PermissionSetRef,
    RoleId, RunId, UnitId, WorkspaceRef,
};
use gauntlet_ports::Deadline;
use gauntlet_ports::agent_runner::Isolation;
use gauntlet_ports::invocation_graph::{GraphLimits, InvocationGraph, InvocationNode, NodeStatus};
use gauntlet_ports::json::{JsonNumber, JsonValue, parse};
use gauntlet_ports::role_result::encode_role_result;
use gauntlet_ports::tool_surface::{ToolOutcome, ToolSurface};
use gauntlet_ports::workspace_provider::{
    Workspace, WorkspaceHook, WorkspaceProvider, WorkspaceProviderError, WorkspaceSubject,
};

use super::DelegationBroker;
use super::agents::{
    AgentsCaller, AgentsPolicy, AgentsService, RoleGrant, WorkspacePolicy, derive_invocation_id,
};
use super::agents_wire::{
    AgentsSurface, CallClock, MAX_CONTENT_BYTES, agents_runtime, decode_spawn,
};
use super::broker::DelegationOutcome;
use super::context::{ContextItemKind, ContextPolicy};
use super::test_support::*;
use crate::fabric::SupportLevel;
use crate::fabric::affordance::AGENTS_AFFORDANCES;
use crate::fabric::selection::Needs;
use crate::govern::{AuthorityGate, GovernDeps};
use gauntlet_ports::tool_catalog::{ToolInvocation, ToolInvoker, ToolName};

const UNIT_TAG: &str = "UN-001";
const ROOT_WORKSPACE: &str = "workspace-a";
const PIN_PROFILE: &str = "profile-a";
const HARNESS_A: &str = "harness-a";
const VARIANT_TAG: &str = "variant-one";
const TASK_LABEL: &str = "task";
const REF_LABEL: &str = "reference";
const TASK_TEXT: &str = "review the change";
const REF_TEXT: &str = "the reference excerpt";
const KEY_FULL: &str = "key-full";
const KEY_OTHER: &str = "key-other";
const KEY_SETTLED: &str = "key-settled";
const KEY_CANCELLED: &str = "key-cancelled";
const CHILD_WINDOW: u64 = 60;
const FULL_SUPPORT: u64 = 2;
const FULL_DEPTH: u64 = 1;

const SPAWN: &str = "agents.spawn";
const STATUS: &str = "agents.status";
const COLLECT: &str = "agents.collect";
const CANCEL: &str = "agents.cancel";
const LIST: &str = "agents.list";

/// A counting workspace provider. It hands out a fresh ref per create and
/// records every cleanup.
#[derive(Default)]
struct CountingWorkspaces {
    created: Cell<u32>,
    cleaned: RefCell<Vec<WorkspaceRef>>,
}

impl WorkspaceProvider for CountingWorkspaces {
    fn create(
        &self,
        _run_id: &RunId,
        _subject: WorkspaceSubject,
        _deadline: Deadline,
    ) -> Result<Workspace, WorkspaceProviderError> {
        let index = self.created.get() + 1;
        self.created.set(index);
        let reference = WorkspaceRef::new(&format!("workspace-child-{index}"))
            .expect("the workspace ref is non-empty");
        Ok(Workspace {
            reference,
            root: PathBuf::new(),
            key: format!("key-{index}"),
        })
    }

    fn cleanup(
        &self,
        workspace: &WorkspaceRef,
        _deadline: Deadline,
    ) -> Result<(), WorkspaceProviderError> {
        self.cleaned.borrow_mut().push(workspace.clone());
        Ok(())
    }

    fn hooks(&self) -> Vec<WorkspaceHook> {
        Vec::new()
    }
}

/// The observable effects a refusal must leave unchanged.
#[derive(Debug, PartialEq, Eq)]
struct Effects {
    live_workspaces: usize,
    invokes: u32,
    active: usize,
}

struct Env {
    lab: Lab,
    a: Arc<ScriptedRunner>,
    policy: AgentsPolicy,
    gate: AuthorityGate,
    govern: GovernDeps,
    workspaces: CountingWorkspaces,
}

impl Env {
    /// A root invocation launched through the broker, owned by the orchestrator.
    fn rooted() -> Self {
        let (lab, a, _b, _c) = standard_lab();
        let env = Self {
            lab,
            a,
            policy: policy(),
            gate: AuthorityGate::new(),
            govern: GovernDeps::none_backend(),
            workspaces: CountingWorkspaces::default(),
        };
        let root = env
            .lab
            .broker()
            .delegate(pinned(
                root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)),
                PIN_PROFILE,
            ))
            .expect("the root launches");
        assert_eq!(root.outcome, DelegationOutcome::Launched);
        env
    }

    fn node(&self, id: &InvocationId) -> InvocationNode {
        self.lab
            .graph
            .node(id, deadline())
            .expect("the graph reads")
            .expect("the node exists")
    }

    fn effects(&self) -> Effects {
        Effects {
            live_workspaces: self.workspaces.created.get() as usize
                - self.workspaces.cleaned.borrow().len(),
            invokes: self.a.invokes(),
            active: self.lab.active_count(),
        }
    }
}

fn policy() -> AgentsPolicy {
    let mut roles = HashMap::new();
    roles.insert(RoleId::Orchestrator, grant(true));
    roles.insert(RoleId::Builder, grant(true));
    roles.insert(RoleId::Critic, grant(false));
    AgentsPolicy {
        roles,
        limits: GraphLimits {
            max_depth: MAX_DEPTH,
            max_children_per_parent: MAX_CHILDREN,
        },
        max_budget: root_slice(),
        isolation: Isolation::Process,
    }
}

fn grant(may_spawn: bool) -> RoleGrant {
    RoleGrant {
        context: ContextPolicy {
            allow: [
                ContextItemKind::TaskStatement,
                ContextItemKind::ReferenceExcerpt,
                ContextItemKind::ConstraintStatement,
                ContextItemKind::ResultSchema,
            ]
            .into_iter()
            .collect::<BTreeSet<_>>(),
            deny: BTreeSet::new(),
        },
        permissions: PermissionSetRef::new("perm-builder").expect("valid permissions"),
        workspace: WorkspacePolicy::Fresh,
        may_spawn,
    }
}

fn unit() -> UnitId {
    UnitId::new(UNIT_TAG).expect("valid unit id")
}

fn root_caller() -> AgentsCaller {
    AgentsCaller {
        run_id: run_id(),
        invocation_id: inv(ROOT_TAG),
        role: RoleId::Orchestrator,
        unit: unit(),
        workspace: WorkspaceRef::new(ROOT_WORKSPACE).expect("valid workspace"),
        recorded: (BlastRadius::Contained, Authority::Critic),
    }
}

/// The caller for an existing node, built from that node's own facts.
fn caller_for(env: &Env, id: &InvocationId, role: RoleId) -> AgentsCaller {
    AgentsCaller {
        run_id: run_id(),
        invocation_id: id.clone(),
        role,
        unit: unit(),
        workspace: env.node(id).workspace,
        recorded: (BlastRadius::Contained, Authority::Critic),
    }
}

fn clock() -> CallClock {
    CallClock {
        at: at(),
        deadline: deadline(),
    }
}

fn surface<'a>(
    env: &'a Env,
    broker: &'a DelegationBroker<'a>,
    caller: AgentsCaller,
) -> AgentsSurface<'a> {
    AgentsSurface::new(
        AgentsService::new(
            caller,
            &env.policy,
            broker,
            &env.lab.graph,
            &env.gate,
            &env.govern,
            &env.workspaces,
        ),
        clock(),
    )
}

// ----- JSON builders ---------------------------------------------------------

fn s(text: &str) -> JsonValue {
    JsonValue::Str(text.to_string())
}

fn n(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber::Uint(value))
}

fn neg(value: i64) -> JsonValue {
    JsonValue::Number(JsonNumber::NegInt(value))
}

fn obj(entries: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(
        entries
            .into_iter()
            .map(|(key, value)| (key.to_string(), value))
            .collect(),
    )
}

fn arr(values: Vec<JsonValue>) -> JsonValue {
    JsonValue::Array(values)
}

/// Sets `key` on an object, replacing any earlier value.
fn set(mut value: JsonValue, key: &str, field: JsonValue) -> JsonValue {
    if let JsonValue::Object(entries) = &mut value {
        entries.retain(|(name, _)| name != key);
        entries.push((key.to_string(), field));
    }
    value
}

fn without(mut value: JsonValue, key: &str) -> JsonValue {
    if let JsonValue::Object(entries) = &mut value {
        entries.retain(|(name, _)| name != key);
    }
    value
}

fn item_json(kind: &str, label: &str, content: &str) -> JsonValue {
    obj(vec![
        ("kind", s(kind)),
        ("label", s(label)),
        ("content", s(content)),
    ])
}

fn budget(rounds: u64, wall: u64, tokens: u64, cost: u64) -> JsonValue {
    obj(vec![
        ("rounds", n(rounds)),
        ("wall_seconds", n(wall)),
        ("tokens", n(tokens)),
        ("cost_usd_micros", n(cost)),
    ])
}

fn child_budget() -> JsonValue {
    budget(
        u64::from(CHILD_ROUNDS),
        CHILD_WALL,
        CHILD_TOKENS,
        CHILD_COST,
    )
}

/// The minimal valid spawn: a role, one task item, a budget, a key, a window.
fn spawn_base(key: &str) -> JsonValue {
    obj(vec![
        ("role", s("builder")),
        (
            "items",
            arr(vec![item_json("task-statement", TASK_LABEL, TASK_TEXT)]),
        ),
        ("budget", child_budget()),
        ("idempotency_key", s(key)),
        ("deadline_seconds", n(CHILD_WINDOW)),
    ])
}

/// The minimal spawn pinned to the profile the scripted runner `a` serves.
fn pinned_spawn(key: &str) -> JsonValue {
    set(
        spawn_base(key),
        "preference",
        obj(vec![("pin", s(PIN_PROFILE))]),
    )
}

fn full_spawn() -> JsonValue {
    obj(vec![
        ("role", s("builder")),
        (
            "items",
            arr(vec![
                item_json("task-statement", TASK_LABEL, TASK_TEXT),
                item_json("reference-excerpt", REF_LABEL, REF_TEXT),
            ]),
        ),
        (
            "requirements",
            obj(vec![
                ("workspace_mutation", s("forbidden")),
                ("min_support_level", n(FULL_SUPPORT)),
                ("needs_resume", JsonValue::Bool(true)),
                ("needs_model_selection", JsonValue::Bool(false)),
                ("needs_native_subagents", JsonValue::Bool(false)),
                ("needs_streaming", JsonValue::Bool(true)),
                ("needs_structured_lifecycle", JsonValue::Bool(true)),
                ("min_isolation", s("container")),
                ("min_depth_support", n(FULL_DEPTH)),
            ]),
        ),
        (
            "preference",
            obj(vec![
                ("pin", s(PIN_PROFILE)),
                ("preferred_harness", s(HARNESS_A)),
                ("preferred_model", s(VARIANT_TAG)),
            ]),
        ),
        ("budget", child_budget()),
        ("idempotency_key", s(KEY_FULL)),
        ("deadline_seconds", n(CHILD_WINDOW)),
    ])
}

fn target(id: &str) -> JsonValue {
    obj(vec![("invocation_id", s(id))])
}

fn text_of(value: &JsonValue, key: &str) -> String {
    match value.get(key) {
        Some(JsonValue::Str(text)) => text.clone(),
        other => panic!("{key} is a string, found {other:?}"),
    }
}

fn expect_ok(outcome: ToolOutcome) -> JsonValue {
    match outcome {
        ToolOutcome::Ok(value) => value,
        other => panic!("expected an ok result, got {other:?}"),
    }
}

/// Decodes a spawn and returns its detail, which must be a rejection.
fn rejected(arguments: &JsonValue) -> String {
    match decode_spawn(arguments, clock()) {
        Err(detail) => detail,
        Ok(_) => panic!("the spawn must be rejected"),
    }
}

// ----- the spawn decode ------------------------------------------------------

#[test]
fn agents_wire_decodes_a_full_valid_spawn() {
    let args = decode_spawn(&full_spawn(), clock()).expect("the full spawn decodes");

    assert_eq!(args.role, RoleId::Builder);
    assert_eq!(args.items.len(), 2);
    assert_eq!(args.items[1].kind, ContextItemKind::ReferenceExcerpt);
    assert_eq!(args.items[1].label, REF_LABEL);
    assert_eq!(args.requirements.workspace_mutation, Needs::Forbidden);
    assert_eq!(args.requirements.min_support_level, SupportLevel::Session);
    assert!(args.requirements.needs_resume);
    assert!(args.requirements.needs_streaming);
    assert!(args.requirements.needs_structured_lifecycle);
    assert_eq!(args.requirements.min_isolation, Isolation::Container);
    assert_eq!(args.requirements.min_depth_support, 1);
    assert_eq!(args.preference.pin, Some(profile_id(PIN_PROFILE)));
    assert_eq!(
        args.preference.preferred_harness,
        Some(HarnessId::new(HARNESS_A).expect("valid harness"))
    );
    assert_eq!(
        args.preference.preferred_model.as_deref(),
        Some(VARIANT_TAG)
    );
    assert_eq!(args.budget.rounds, CHILD_ROUNDS);
    assert_eq!(args.budget.wall_seconds, CHILD_WALL);
    assert_eq!(args.budget.tokens, CHILD_TOKENS);
    assert_eq!(args.budget.cost_usd.get(), CHILD_COST);
    assert_eq!(
        args.idempotency_key,
        AdmissionKey::new(KEY_FULL).expect("key")
    );
    assert_eq!(args.at, at());
    assert_eq!(
        args.deadline.instant(),
        EpochSeconds::new(ADMIT_AT + CHILD_WINDOW)
    );
}

#[test]
fn agents_wire_decodes_the_minimal_valid_spawn() {
    let args = decode_spawn(&spawn_base(KEY_OTHER), clock()).expect("the minimal spawn decodes");

    assert_eq!(args.role, RoleId::Builder);
    assert_eq!(args.items.len(), 1);
    assert_eq!(args.requirements.workspace_mutation, Needs::DontCare);
    assert_eq!(args.requirements.min_support_level, SupportLevel::Process);
    assert_eq!(args.requirements.min_isolation, Isolation::None);
    assert_eq!(args.preference.pin, None);
    assert_eq!(args.preference.preferred_harness, None);
}

#[test]
fn agents_wire_rejects_an_unknown_top_level_key() {
    let value = set(spawn_base(KEY_OTHER), "bogus", n(1));
    assert_eq!(rejected(&value), "arguments.bogus: unknown field");
}

#[test]
fn agents_wire_rejects_an_unknown_nested_key() {
    let extra = set(
        item_json("task-statement", TASK_LABEL, TASK_TEXT),
        "extra",
        s("x"),
    );
    let value = set(spawn_base(KEY_OTHER), "items", arr(vec![extra]));
    assert_eq!(rejected(&value), "arguments.items[0].extra: unknown field");
}

#[test]
fn agents_wire_rejects_a_missing_role() {
    let value = without(spawn_base(KEY_OTHER), "role");
    assert_eq!(rejected(&value), "arguments: missing field role");
}

#[test]
fn agents_wire_rejects_a_wrong_type() {
    let value = set(spawn_base(KEY_OTHER), "role", n(1));
    assert_eq!(rejected(&value), "arguments.role: expected a string");
}

#[test]
fn agents_wire_rejects_a_negative_budget() {
    let value = set(
        spawn_base(KEY_OTHER),
        "budget",
        obj(vec![
            ("rounds", neg(-1)),
            ("wall_seconds", n(CHILD_WALL)),
            ("tokens", n(CHILD_TOKENS)),
            ("cost_usd_micros", n(CHILD_COST)),
        ]),
    );
    assert_eq!(
        rejected(&value),
        "arguments.budget.rounds: must not be negative"
    );
}

#[test]
fn agents_wire_rejects_a_duplicate_json_key() {
    let env = Env::rooted();
    let broker = env.lab.broker();
    let root = surface(&env, &broker, root_caller());
    let before = env.effects();

    match root.call_text(SPAWN, r#"{"role":"builder","role":"critic"}"#) {
        ToolOutcome::Invalid { detail } => {
            assert!(detail.starts_with("arguments: "), "path first: {detail}");
            assert!(detail.contains("duplicate object key"), "{detail}");
        }
        other => panic!("a duplicate key is Invalid, got {other:?}"),
    }
    assert_eq!(env.effects(), before, "nothing ran");
}

#[test]
fn agents_wire_rejects_an_unknown_context_kind() {
    let value = set(
        spawn_base(KEY_OTHER),
        "items",
        arr(vec![
            item_json("task-statement", TASK_LABEL, TASK_TEXT),
            item_json("mystery-kind", REF_LABEL, REF_TEXT),
        ]),
    );
    assert_eq!(
        rejected(&value),
        "arguments.items[1].kind: unknown context kind"
    );
}

#[test]
fn agents_wire_rejects_fresh_context_supplied() {
    let value = set(
        spawn_base(KEY_OTHER),
        "fresh_context",
        JsonValue::Bool(true),
    );
    assert_eq!(rejected(&value), "arguments.fresh_context: unknown field");
}

#[test]
fn agents_wire_rejects_oversized_content() {
    let oversized = "x".repeat(MAX_CONTENT_BYTES + 1);
    let value = set(
        spawn_base(KEY_OTHER),
        "items",
        arr(vec![
            item_json("task-statement", TASK_LABEL, TASK_TEXT),
            item_json("reference-excerpt", REF_LABEL, &oversized),
        ]),
    );
    assert_eq!(
        rejected(&value),
        format!("arguments.items[1].content: exceeds {MAX_CONTENT_BYTES} bytes")
    );
}

#[test]
fn agents_wire_rejects_an_invalid_isolation_token() {
    let value = set(
        spawn_base(KEY_OTHER),
        "requirements",
        obj(vec![("min_isolation", s("bogus-isolation"))]),
    );
    assert_eq!(
        rejected(&value),
        "arguments.requirements.min_isolation: unknown isolation token"
    );
}

// ----- the surface -----------------------------------------------------------

#[test]
fn agents_wire_spawn_launches_once_and_encodes_the_outcome() {
    let env = Env::rooted();
    let broker = env.lab.broker();
    let root = surface(&env, &broker, root_caller());
    let before = env.a.invokes();

    let value = expect_ok(root.call(SPAWN, &pinned_spawn(KEY_OTHER)));

    assert_eq!(env.a.invokes(), before + 1, "exactly one runner launch");
    assert_eq!(value.get("launched"), Some(&JsonValue::Bool(true)));
    assert_eq!(value.get("depth"), Some(&n(1)));
    assert_eq!(value.get("runner_profile"), Some(&s(PIN_PROFILE)));
    assert_eq!(value.get("harness"), Some(&s(HARNESS_A)));
    let expected = derived_id(KEY_OTHER);
    assert_eq!(text_of(&value, "invocation_id"), expected.as_str());
    assert_eq!(value.keys().len(), 5, "the spawn result has five keys");
}

fn derived_id(key: &str) -> InvocationId {
    derive_invocation_id(&run_id(), &AdmissionKey::new(key).expect("key"))
}

#[test]
fn agents_wire_refused_spawn_of_an_ungranted_role_is_a_zero_effect_refusal() {
    let env = Env::rooted();
    let broker = env.lab.broker();
    let root = surface(&env, &broker, root_caller());
    let before = env.effects();
    let request = set(pinned_spawn(KEY_OTHER), "role", s("verifier"));

    match root.call(SPAWN, &request) {
        ToolOutcome::Refused { code, .. } => assert_eq!(code, "role_not_granted"),
        other => panic!("an ungranted role is a refusal, got {other:?}"),
    }

    assert_eq!(env.effects(), before, "the refusal changed nothing");
}

#[test]
fn agents_wire_status_of_a_non_descendant_is_refused_without_leaking_ids() {
    let env = Env::rooted();
    let broker = env.lab.broker();
    let root = surface(&env, &broker, root_caller());
    let child = expect_ok(root.call(SPAWN, &pinned_spawn(KEY_OTHER)));
    let child_id = text_of(&child, "invocation_id");
    let child_caller = caller_for(
        &env,
        &InvocationId::new(&child_id).expect("the child id is well formed"),
        RoleId::Builder,
    );
    let child_surface = surface(&env, &broker, child_caller);

    match child_surface.call(STATUS, &target(&inv(ROOT_TAG).to_string())) {
        ToolOutcome::Refused { code, detail } => {
            assert_eq!(code, "not_your_subtree");
            assert!(
                !detail.contains(&inv(ROOT_TAG).to_string()),
                "the ancestor id is not echoed: {detail}"
            );
            assert!(!detail.contains(&child_id), "no id is echoed: {detail}");
        }
        other => panic!("an ancestor is not in the subtree, got {other:?}"),
    }
}

/// OD-14a: an unanswered collect is a typed, additive `runner_unavailable: true`
/// on a still-running result. The node is left live, and a normal collect keeps
/// its exact keys, with no `runner_unavailable` key at all.
#[test]
fn agents_wire_collect_reports_runner_unavailable_additively_and_only_then() {
    let env = Env::rooted();
    let broker = env.lab.broker();
    let root = surface(&env, &broker, root_caller());
    let child = expect_ok(root.call(SPAWN, &pinned_spawn(KEY_SETTLED)));
    let child_id = InvocationId::new(&text_of(&child, "invocation_id")).expect("well formed");

    env.a.set_transport_down(true);
    let value = expect_ok(root.call(COLLECT, &target(child_id.as_str())));
    assert_eq!(value.get("state"), Some(&s("still_running")));
    assert_eq!(
        value.get("runner_unavailable"),
        Some(&JsonValue::Bool(true))
    );
    assert_eq!(value.get("note"), Some(&s("completion is not settlement")));
    assert_valid(COLLECT, true, &value);
    assert_eq!(env.node(&child_id).status, NodeStatus::Running);

    env.a.set_transport_down(false);
    env.a.set_status(&child_id, running_status());
    let value = expect_ok(root.call(COLLECT, &target(child_id.as_str())));
    assert_eq!(value.get("state"), Some(&s("still_running")));
    assert_eq!(
        value.get("runner_unavailable"),
        None,
        "a normal collect carries no runner_unavailable key"
    );
    assert_eq!(value.keys().len(), 2, "state and note only");
    assert_valid(COLLECT, true, &value);
}

#[test]
fn agents_wire_collect_carries_the_claim_text_and_the_settlement_note() {
    let env = Env::rooted();
    let broker = env.lab.broker();
    let root = surface(&env, &broker, root_caller());
    let child = expect_ok(root.call(SPAWN, &pinned_spawn(KEY_SETTLED)));
    let child_id = InvocationId::new(&text_of(&child, "invocation_id")).expect("well formed");
    let digest = env.node(&child_id).context_digest;
    env.a.set_status(&child_id, completed_status());
    let result = builder_result(&child_id, &digest, false);
    let expected_claim = encode_role_result(&result);
    env.a.set_result(result);

    let value = expect_ok(root.call(COLLECT, &target(child_id.as_str())));

    assert_eq!(value.get("state"), Some(&s("completed")));
    assert_eq!(value.get("claim"), Some(&s(&expected_claim)));
    assert_eq!(
        value.get("note"),
        Some(&s("completion is not settlement")),
        "a claim is never settlement"
    );
}

#[test]
fn agents_wire_lists_the_five_affordances_with_catalog_schemas() {
    let env = Env::rooted();
    let broker = env.lab.broker();
    let root = surface(&env, &broker, root_caller());

    let tools = root.tools();

    assert_eq!(tools.len(), AGENTS_AFFORDANCES.len());
    for (listing, descriptor) in tools.iter().zip(AGENTS_AFFORDANCES) {
        assert_eq!(listing.name, descriptor.name);
        assert_eq!(listing.description, descriptor.summary);
        assert_eq!(listing.input_schema, descriptor.input_schema);
    }
}

#[test]
fn agents_wire_unknown_tool_name_is_invalid() {
    let env = Env::rooted();
    let broker = env.lab.broker();
    let root = surface(&env, &broker, root_caller());

    match root.call("agents.teleport", &obj(vec![])) {
        ToolOutcome::Invalid { detail } => assert_eq!(detail, "unknown tool: agents.teleport"),
        other => panic!("an unknown tool is Invalid, got {other:?}"),
    }
}

#[test]
fn agents_wire_non_object_arguments_are_invalid() {
    let env = Env::rooted();
    let broker = env.lab.broker();
    let root = surface(&env, &broker, root_caller());

    for name in [SPAWN, STATUS, COLLECT, CANCEL, LIST] {
        match root.call(name, &JsonValue::Bool(true)) {
            ToolOutcome::Invalid { detail } => {
                assert_eq!(detail, "arguments: expected an object", "tool {name}");
            }
            other => panic!("non-object arguments are Invalid for {name}, got {other:?}"),
        }
    }
}

// ----- structural validation of every example against its schema -------------

fn schema_of(name: &str, output: bool) -> JsonValue {
    let descriptor = AGENTS_AFFORDANCES
        .iter()
        .find(|descriptor| descriptor.name == name)
        .expect("the catalog names the tool");
    let text = if output {
        descriptor.output_schema
    } else {
        descriptor.input_schema
    };
    parse(text).expect("the schema is JSON")
}

/// A structural check of the keywords the catalog uses: type, enum, required,
/// closed properties, items, and minimum or maximum.
fn validates(schema: &JsonValue, value: &JsonValue, path: &str) -> Result<(), String> {
    let enumeration = match schema.get("enum") {
        Some(JsonValue::Array(options)) => Some(options),
        _ => None,
    };
    if enumeration.is_some_and(|options| !options.contains(value)) {
        return Err(format!("{path}: not in the enumeration"));
    }
    if let Some(kinds) = schema.get("type") {
        let allowed: Vec<&str> = match kinds {
            JsonValue::Str(kind) => vec![kind.as_str()],
            JsonValue::Array(list) => list
                .iter()
                .filter_map(|kind| match kind {
                    JsonValue::Str(kind) => Some(kind.as_str()),
                    _ => None,
                })
                .collect(),
            _ => Vec::new(),
        };
        if !allowed.iter().any(|kind| type_matches(kind, value)) {
            return Err(format!("{path}: expected one of {allowed:?}"));
        }
    }
    if let JsonValue::Number(number) = value {
        let magnitude = number.as_f64();
        let bound = |key: &str| match schema.get(key) {
            Some(JsonValue::Number(bound)) => Some(bound.as_f64()),
            _ => None,
        };
        if bound("minimum").is_some_and(|min| magnitude < min) {
            return Err(format!("{path}: below the minimum"));
        }
        if bound("maximum").is_some_and(|max| magnitude > max) {
            return Err(format!("{path}: above the maximum"));
        }
    }
    if let JsonValue::Object(entries) = value {
        if let Some(JsonValue::Array(required)) = schema.get("required") {
            for name in required {
                let missing = match name {
                    JsonValue::Str(name) => value.get(name).is_none().then_some(name),
                    _ => None,
                };
                if let Some(name) = missing {
                    return Err(format!("{path}: missing required field {name}"));
                }
            }
        }
        let declared: &[(String, JsonValue)] = match schema.get("properties") {
            Some(JsonValue::Object(declared)) => declared,
            _ => &[],
        };
        if schema.get("additionalProperties") == Some(&JsonValue::Bool(false)) {
            for (key, _) in entries {
                if !declared.iter().any(|(name, _)| name == key) {
                    return Err(format!("{path}.{key}: unknown field"));
                }
            }
        }
        for (key, child) in entries {
            if let Some((_, child_schema)) = declared.iter().find(|(name, _)| name == key) {
                validates(child_schema, child, &format!("{path}.{key}"))?;
            }
        }
    }
    if let (JsonValue::Array(elements), Some(element_schema)) = (value, schema.get("items")) {
        for (index, element) in elements.iter().enumerate() {
            validates(element_schema, element, &format!("{path}[{index}]"))?;
        }
    }
    Ok(())
}

fn type_matches(kind: &str, value: &JsonValue) -> bool {
    match (kind, value) {
        ("null", JsonValue::Null)
        | ("boolean", JsonValue::Bool(_))
        | ("string", JsonValue::Str(_))
        | ("array", JsonValue::Array(_))
        | ("object", JsonValue::Object(_)) => true,
        ("integer", JsonValue::Number(number)) => !matches!(number, JsonNumber::Float(_)),
        _ => false,
    }
}

fn assert_valid(name: &str, output: bool, value: &JsonValue) {
    let side = if output { "output" } else { "input" };
    if let Err(problem) = validates(&schema_of(name, output), value, "value") {
        panic!("{name} {side} does not match its schema: {problem}");
    }
}

#[test]
fn agents_wire_encoded_examples_validate_against_their_schemas() {
    let env = Env::rooted();
    let broker = env.lab.broker();
    let root = surface(&env, &broker, root_caller());

    // Inputs: every example a caller may send.
    assert_valid(SPAWN, false, &full_spawn());
    assert_valid(SPAWN, false, &pinned_spawn(KEY_OTHER));
    assert_valid(STATUS, false, &target(&inv(ROOT_TAG).to_string()));
    assert_valid(LIST, false, &obj(vec![]));

    // Outputs: spawn, then a running view, a still-running collect, a claim,
    // a cancel, and the children listing.
    let spawned = expect_ok(root.call(SPAWN, &pinned_spawn(KEY_OTHER)));
    assert_valid(SPAWN, true, &spawned);
    let running_id = text_of(&spawned, "invocation_id");
    let status = expect_ok(root.call(STATUS, &target(&running_id)));
    assert_valid(STATUS, true, &status);
    let collected = expect_ok(root.call(COLLECT, &target(&running_id)));
    assert_eq!(collected.get("state"), Some(&s("still_running")));
    assert_valid(COLLECT, true, &collected);

    let claimed = expect_ok(root.call(SPAWN, &pinned_spawn(KEY_SETTLED)));
    let claimed_id = InvocationId::new(&text_of(&claimed, "invocation_id")).expect("well formed");
    let digest = env.node(&claimed_id).context_digest;
    env.a.set_status(&claimed_id, completed_status());
    env.a
        .set_result(builder_result(&claimed_id, &digest, false));
    let claim_out = expect_ok(root.call(COLLECT, &target(claimed_id.as_str())));
    assert_eq!(claim_out.get("state"), Some(&s("completed")));
    assert_valid(COLLECT, true, &claim_out);

    let cancelling = expect_ok(root.call(SPAWN, &pinned_spawn(KEY_CANCELLED)));
    let cancel_out = expect_ok(root.call(CANCEL, &target(&text_of(&cancelling, "invocation_id"))));
    assert_valid(CANCEL, true, &cancel_out);

    let listed = expect_ok(root.call(LIST, &obj(vec![])));
    assert_valid(LIST, true, &listed);
}

const OTHER_CALLER_TEXT: &str = "01BX5ZZKBKACTAV9WEVGEMMVRZ";

#[test]
fn agents_runtime_matches_the_surface_and_refuses_another_caller() {
    let env = Env::rooted();
    let broker = env.lab.broker();
    let root_id = root_caller().invocation_id.clone();
    let root = surface(&env, &broker, root_caller());
    let runtime = agents_runtime(&root, root_id.clone()).expect("the agents catalog builds");

    let list_args = parse("{}").expect("a well-formed document");
    let via_runtime = runtime.invoke(&ToolInvocation {
        caller: root_id.clone(),
        name: ToolName::new("agents.list").expect("a valid name"),
        args: list_args.clone(),
    });
    assert_eq!(via_runtime, root.call("agents.list", &list_args));

    let other = InvocationId::new(OTHER_CALLER_TEXT).expect("a valid invocation id");
    assert_ne!(other, root_id, "the fixture caller differs");
    let refused = runtime.invoke(&ToolInvocation {
        caller: other,
        name: ToolName::new("agents.list").expect("a valid name"),
        args: list_args,
    });
    assert!(matches!(
        refused,
        ToolOutcome::Refused { ref code, .. } if code == "caller_mismatch"
    ));
}
