//! PostgreSQL `InvocationGraph` tests (E2b). The shared contract suite runs
//! unmodified; the rest pin what only a real database can prove: concurrent
//! admissions on separate connections, transaction atomicity, crash
//! recovery, reconstruction after reopening, and immutable-column guards.
//! Every test gets a fresh PostgreSQL namespace.

use std::path::PathBuf;
use std::sync::{Arc, Barrier};
use std::thread;
use std::time::{SystemTime, UNIX_EPOCH};

use gauntlet_adapter_state_postgres::test_support::ScratchNamespaces;
use gauntlet_adapter_state_postgres::{PostgresConfig, PostgresInvocationGraph};
use gauntlet_domain::{
    AdmissionKey, BudgetDimension, Digest, EpochSeconds, HarnessId, InvocationId, MicroUsd,
    ResultContractPath, RoleId, RunId, RunnerProfileId, SessionRef, WorkspaceRef,
};
use gauntlet_ports::budget_meter::BudgetSlice;
use gauntlet_ports::contract::invocation_graph::{InvocationGraphFactory, invocation_graph_suite};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::invocation_graph::{
    AdmissionOutcome, AdmissionRefusal, AdmissionRequest, GraphError, GraphLimits, InvocationGraph,
    InvocationNode, NativeSessionBinding, NodeStatus, ResultBinding,
};
use gauntlet_ports::lifecycle::{
    HarnessObservation, LifecycleState, ObservationOutcome, ObservationSequence, ObservationSource,
};
use postgres::{Client, NoTls};

const FAR_AT: u64 = 1_900_000_000;
const ADMIT_AT: u64 = 1_800_000_000;
const COMPLETE_AT: u64 = 1_800_000_100;
const CANCEL_AT: u64 = 1_800_000_200;
const RUN_PREFIX: &str = "01J9R";
const NODE_PREFIX: &str = "01J9N";
const TAIL_WIDTH: usize = 21;
const CONTRACT: &str = "contract-a";
const DIGEST: &str = "sha256:a1b2c3d4";
const DEFAULT_MAX_DEPTH: u32 = 8;
const DEFAULT_MAX_CHILDREN: u32 = 8;

/// A fresh namespace per call, dropped when `scratch` (owned by the test) goes out of scope.
fn fresh_config(scratch: &ScratchNamespaces) -> PostgresConfig {
    scratch.fresh("ig")
}

/// A raw connection into the namespace, for fault injection and out-of-band reads.
fn raw(config: &PostgresConfig) -> Client {
    let mut client = Client::connect(&config.url, NoTls).expect("raw connection");
    client
        .batch_execute(&format!("SET search_path TO \"{}\"", config.namespace))
        .expect("raw search_path");
    client
}

fn dl() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(FAR_AT))
}

fn inv(tail: &str) -> InvocationId {
    InvocationId::new(&format!("{NODE_PREFIX}{tail:0>TAIL_WIDTH$}")).expect("valid invocation id")
}

fn run(tail: &str) -> RunId {
    RunId::new(&format!("{RUN_PREFIX}{tail:0>TAIL_WIDTH$}")).expect("valid run id")
}

fn root_budget() -> BudgetSlice {
    BudgetSlice {
        rounds: 4,
        wall_seconds: 600,
        tokens: 1000,
        cost_usd: MicroUsd::new(100),
    }
}

fn child_budget() -> BudgetSlice {
    BudgetSlice {
        rounds: 1,
        wall_seconds: 60,
        tokens: 200,
        cost_usd: MicroUsd::new(10),
    }
}

fn request_with(
    id: &InvocationId,
    run_id: &RunId,
    parent: Option<&InvocationId>,
    key: &str,
    harness: &str,
    budget: BudgetSlice,
    max_children: u32,
) -> AdmissionRequest {
    AdmissionRequest {
        invocation_id: id.clone(),
        run_id: run_id.clone(),
        parent: parent.cloned(),
        role: RoleId::parse("builder").expect("named role"),
        runner_profile: RunnerProfileId::new("profile-1").expect("profile id"),
        harness: HarnessId::new(harness).expect("harness id"),
        workspace: WorkspaceRef::new("workspace-1").expect("workspace"),
        context_digest: Digest::parse(DIGEST).expect("digest"),
        result_contract: ResultContractPath::new(CONTRACT).expect("contract"),
        admission_key: AdmissionKey::new(key).expect("admission key"),
        budget,
        limits: GraphLimits {
            max_depth: DEFAULT_MAX_DEPTH,
            max_children_per_parent: max_children,
        },
        at: EpochSeconds::new(ADMIT_AT),
    }
}

fn child_request(
    id: &InvocationId,
    run_id: &RunId,
    parent: &InvocationId,
    key: &str,
) -> AdmissionRequest {
    request_with(
        id,
        run_id,
        Some(parent),
        key,
        "harness-a",
        child_budget(),
        DEFAULT_MAX_CHILDREN,
    )
}

fn root_request(id: &InvocationId, run_id: &RunId) -> AdmissionRequest {
    let mut root = request_with(
        id,
        run_id,
        None,
        "key-root",
        "harness-a",
        root_budget(),
        DEFAULT_MAX_CHILDREN,
    );
    root.role = RoleId::parse("orchestrator").expect("named role");
    root
}

fn binding() -> ResultBinding {
    ResultBinding {
        contract_path: ResultContractPath::new(CONTRACT).expect("contract"),
        context_digest: Digest::parse(DIGEST).expect("digest"),
        result_digest: Digest::parse("sha256:e5f6a7b8").expect("digest"),
    }
}

fn admitted(outcome: AdmissionOutcome) -> InvocationNode {
    match outcome {
        AdmissionOutcome::Admitted(node) => node,
        other => panic!("expected Admitted, got {other:?}"),
    }
}

/// Runs `job` once per thread, each thread on its own connection to the same
/// namespace, all released together by a barrier.
/// The per-thread work: a graph on the thread's own connection, and the thread index.
type Job<T> = Arc<dyn Fn(&PostgresInvocationGraph, usize) -> T + Send + Sync>;

fn on_connections<T: Send + 'static>(
    config: &PostgresConfig,
    threads: usize,
    job: Job<T>,
) -> Vec<T> {
    let barrier = Arc::new(Barrier::new(threads));
    let handles: Vec<_> = (0..threads)
        .map(|index| {
            let config = config.clone();
            let barrier = Arc::clone(&barrier);
            let job = Arc::clone(&job);
            thread::spawn(move || {
                let graph = PostgresInvocationGraph::open(&config).expect("per-thread graph");
                barrier.wait();
                job(&graph, index)
            })
        })
        .collect();
    handles
        .into_iter()
        .map(|handle| handle.join().expect("worker thread"))
        .collect()
}

struct PostgresGraphFactory<'a> {
    scratch: &'a ScratchNamespaces,
}

impl InvocationGraphFactory for PostgresGraphFactory<'_> {
    fn label(&self) -> &'static str {
        "postgres-invocation-graph"
    }

    fn build(&self) -> Result<Box<dyn InvocationGraph>, String> {
        PostgresInvocationGraph::open(&fresh_config(self.scratch))
            .map(|graph| Box::new(graph) as Box<dyn InvocationGraph>)
            .map_err(|err| err.to_string())
    }
}

#[test]
fn the_shared_contract_suite_passes_on_postgres() {
    let scratch = ScratchNamespaces::default();
    let findings = invocation_graph_suite(&PostgresGraphFactory { scratch: &scratch });
    assert!(
        findings.is_empty(),
        "the PostgreSQL graph must run the suite clean, but reported:\n{}",
        findings
            .iter()
            .map(|f| f.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn identical_concurrent_admissions_create_exactly_one_node() {
    let scratch = ScratchNamespaces::default();
    let config = fresh_config(&scratch);
    let graph = PostgresInvocationGraph::open(&config).expect("graph");
    let (root, run_id) = (inv("1"), run("1"));
    admitted(
        graph
            .admit(root_request(&root, &run_id), dl())
            .expect("root"),
    );
    drop(graph);

    let shared = Arc::new(child_request(&inv("2"), &run_id, &root, "key-same"));
    let outcomes = on_connections(
        &config,
        8,
        Arc::new(move |graph: &PostgresInvocationGraph, _| graph.admit((*shared).clone(), dl())),
    );

    let (mut created, mut replayed) = (0, 0);
    for outcome in outcomes {
        match outcome.expect("admission") {
            AdmissionOutcome::Admitted(_) => created += 1,
            AdmissionOutcome::AlreadyAdmitted(_) => replayed += 1,
            other => panic!("unexpected outcome {other:?}"),
        }
    }
    assert_eq!((created, replayed), (1, 7));
    let graph = PostgresInvocationGraph::open(&config).expect("graph");
    assert_eq!(graph.children(&root, dl()).expect("children").len(), 1);
}

#[test]
fn concurrent_children_respect_the_child_limit() {
    let scratch = ScratchNamespaces::default();
    let config = fresh_config(&scratch);
    let graph = PostgresInvocationGraph::open(&config).expect("graph");
    let (root, run_id) = (inv("1"), run("1"));
    admitted(
        graph
            .admit(root_request(&root, &run_id), dl())
            .expect("root"),
    );
    drop(graph);

    let root_for_threads = root.clone();
    let outcomes = on_connections(
        &config,
        6,
        Arc::new(move |graph: &PostgresInvocationGraph, index: usize| {
            let child = inv(&format!("{}", index + 2));
            let request = request_with(
                &child,
                &run("1"),
                Some(&root_for_threads),
                &format!("key-c{index}"),
                "harness-a",
                child_budget(),
                3,
            );
            graph.admit(request, dl())
        }),
    );

    let (mut created, mut limited) = (0, 0);
    for outcome in outcomes {
        match outcome.expect("admission") {
            AdmissionOutcome::Admitted(_) => created += 1,
            AdmissionOutcome::Refused(AdmissionRefusal::ChildLimit { max: 3 }) => limited += 1,
            other => panic!("unexpected outcome {other:?}"),
        }
    }
    assert_eq!((created, limited), (3, 3));
    let graph = PostgresInvocationGraph::open(&config).expect("graph");
    assert_eq!(graph.children(&root, dl()).expect("children").len(), 3);
}

#[test]
fn concurrent_children_never_exceed_the_parent_budget() {
    let scratch = ScratchNamespaces::default();
    let config = fresh_config(&scratch);
    let graph = PostgresInvocationGraph::open(&config).expect("graph");
    let (root, run_id) = (inv("1"), run("1"));
    admitted(
        graph
            .admit(root_request(&root, &run_id), dl())
            .expect("root"),
    );
    drop(graph);

    // Each child reserves 340 of the parent's 1000 tokens: at most two fit.
    let root_for_threads = root.clone();
    let outcomes = on_connections(
        &config,
        6,
        Arc::new(move |graph: &PostgresInvocationGraph, index: usize| {
            let child = inv(&format!("{}", index + 2));
            let budget = BudgetSlice {
                rounds: 1,
                wall_seconds: 10,
                tokens: 340,
                cost_usd: MicroUsd::new(10),
            };
            let request = request_with(
                &child,
                &run("1"),
                Some(&root_for_threads),
                &format!("key-b{index}"),
                "harness-a",
                budget,
                DEFAULT_MAX_CHILDREN,
            );
            graph.admit(request, dl())
        }),
    );

    let (mut created, mut refused) = (0, 0);
    for outcome in outcomes {
        match outcome.expect("admission") {
            AdmissionOutcome::Admitted(_) => created += 1,
            AdmissionOutcome::Refused(AdmissionRefusal::BudgetExceeded {
                dimension: BudgetDimension::Tokens,
            }) => refused += 1,
            other => panic!("unexpected outcome {other:?}"),
        }
    }
    assert_eq!((created, refused), (2, 4));

    let mut client = raw(&config);
    let live_tokens: i64 = client
        .query_one(
            "SELECT COALESCE(SUM(budget_tokens), 0)::BIGINT FROM invocation_nodes \
             WHERE parent_id = $1 AND status IN ('admitted', 'running')",
            &[&root.as_str()],
        )
        .expect("sum")
        .get(0);
    assert!(
        live_tokens <= 1000,
        "live children reserve {live_tokens} tokens, above the parent budget of 1000"
    );
    assert_eq!(live_tokens, 680);
}

#[test]
fn an_injected_insert_failure_leaves_no_row_and_no_partial_state() {
    let scratch = ScratchNamespaces::default();
    let config = fresh_config(&scratch);
    let graph = PostgresInvocationGraph::open(&config).expect("graph");
    let (root, run_id) = (inv("1"), run("1"));
    admitted(
        graph
            .admit(root_request(&root, &run_id), dl())
            .expect("root"),
    );
    let (poisoned, healthy) = (inv("2"), inv("3"));

    let mut client = raw(&config);
    client
        .batch_execute(&format!(
            "CREATE FUNCTION refuse_poisoned_node() RETURNS trigger LANGUAGE plpgsql AS $$ \
             BEGIN \
               IF NEW.invocation_id = '{poisoned}' THEN \
                 RAISE EXCEPTION 'injected insert failure'; \
               END IF; \
               RETURN NEW; \
             END; $$; \
             CREATE TRIGGER refuse_poisoned_node BEFORE INSERT ON invocation_nodes \
             FOR EACH ROW EXECUTE FUNCTION refuse_poisoned_node();"
        ))
        .expect("install trigger");

    let root_before = graph.node(&root, dl()).expect("root read");
    let children_before = graph.children(&root, dl()).expect("children read");

    let result = graph.admit(child_request(&poisoned, &run_id, &root, "key-poison"), dl());
    assert!(
        matches!(result, Err(GraphError::WriteFailed { .. })),
        "a failing insert must be a typed write failure, got {result:?}"
    );
    assert_eq!(graph.node(&poisoned, dl()).expect("read"), None);
    assert_eq!(graph.node(&root, dl()).expect("root read"), root_before);
    assert_eq!(
        graph.children(&root, dl()).expect("children read"),
        children_before
    );

    // The same key is free again: nothing was bound by the failed attempt.
    admitted(
        graph
            .admit(child_request(&healthy, &run_id, &root, "key-poison"), dl())
            .expect("healthy admission after the failure"),
    );
}

#[test]
fn a_crashed_uncommitted_admission_is_absent_after_reopen() {
    let scratch = ScratchNamespaces::default();
    let config = fresh_config(&scratch);
    let graph = PostgresInvocationGraph::open(&config).expect("graph");
    let (root, run_id) = (inv("1"), run("1"));
    let root_node = admitted(
        graph
            .admit(root_request(&root, &run_id), dl())
            .expect("root"),
    );
    drop(graph);

    let ghost = inv("9");
    {
        // BEGIN and INSERT, then the connection dies without COMMIT.
        let mut client = raw(&config);
        client
            .batch_execute(&format!(
                "BEGIN; INSERT INTO invocation_nodes (invocation_id, run_id, parent_id, root_id, \
                 depth, role, runner_profile, harness, workspace, context_digest, result_contract, \
                 admission_key, budget_rounds, budget_wall_seconds, budget_tokens, \
                 budget_cost_micro_usd, status, admitted_at) VALUES ('{ghost}', '{run_id}', \
                 '{root}', '{root}', 1, 'builder', 'profile-1', 'harness-a', 'workspace-1', \
                 '{DIGEST}', '{CONTRACT}', 'key-ghost', 1, 60, 200, 10, 'admitted', {ADMIT_AT});"
            ))
            .expect("uncommitted insert");
        drop(client);
    }

    let graph = PostgresInvocationGraph::open(&config).expect("reopened graph");
    assert_eq!(graph.node(&ghost, dl()).expect("read"), None);
    assert!(graph.children(&root, dl()).expect("children").is_empty());
    assert_eq!(
        graph.active_nodes(&run_id, dl()).expect("active"),
        vec![root_node]
    );
}

#[test]
fn the_graph_reconstructs_identically_after_reopen() {
    let path = PathBuf::from(format!(
        "/nonexistent-gauntlet-e2/{}-{:x}/gauntlet-state.db",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let (root, run_id) = (inv("1"), run("1"));
    let (c1, c2, c3, s1) = (inv("2"), inv("3"), inv("4"), inv("5"));
    let all = [&root, &c1, &c2, &c3, &s1];

    let graph = PostgresInvocationGraph::open_at(&path).expect("graph");
    admitted(
        graph
            .admit(root_request(&root, &run_id), dl())
            .expect("root"),
    );
    // Each parent reserves at least the sum of its children, so the nested
    // budgets widen toward the root: c1 fits the root, c2 and s1 fit c1.
    let slice = |rounds: u32, wall: u64, tokens: u64, cost: u64| BudgetSlice {
        rounds,
        wall_seconds: wall,
        tokens,
        cost_usd: MicroUsd::new(cost),
    };
    let admit = |id: &InvocationId,
                 parent: &InvocationId,
                 harness: &str,
                 budget: BudgetSlice,
                 key: &str| {
        let request = request_with(
            id,
            &run_id,
            Some(parent),
            key,
            harness,
            budget,
            DEFAULT_MAX_CHILDREN,
        );
        admitted(graph.admit(request, dl()).expect("admission"))
    };
    admit(&c1, &root, "h-b", slice(4, 300, 500, 50), "key-c1");
    admit(&c2, &c1, "h-c", slice(3, 100, 200, 30), "key-c2");
    admit(&c3, &c2, "h-d", slice(1, 60, 100, 10), "key-c3");
    admit(&s1, &c1, "h-a", slice(1, 60, 100, 10), "key-s1");
    graph.mark_running(&c2, dl()).expect("running");
    graph
        .attach_native_session(&c2, SessionRef::new("sess-c2").expect("session"), dl())
        .expect("attach");
    graph
        .complete(&s1, binding(), EpochSeconds::new(COMPLETE_AT), dl())
        .expect("complete");

    let snapshot = |graph: &PostgresInvocationGraph| -> Vec<Vec<InvocationNode>> {
        let mut out = Vec::new();
        for id in all {
            out.push(graph.node(id, dl()).expect("node").into_iter().collect());
            out.push(graph.children(id, dl()).expect("children"));
            out.push(graph.descendants(id, dl()).expect("descendants"));
            out.push(graph.ancestors(id, dl()).expect("ancestors"));
        }
        out.push(graph.active_nodes(&run_id, dl()).expect("active"));
        out
    };
    let before = snapshot(&graph);
    drop(graph);

    let reopened = PostgresInvocationGraph::open_at(&path).expect("reopened graph");
    assert_eq!(
        snapshot(&reopened),
        before,
        "reconstruction differs from the built graph"
    );
    let deepest = reopened.node(&c3, dl()).expect("read").expect("c3 exists");
    assert_eq!(deepest.depth, 3);
    assert_eq!(deepest.harness.as_str(), "h-d");
    assert_eq!(deepest.root, root);
    let c2_node = reopened.node(&c2, dl()).expect("read").expect("c2 exists");
    assert_eq!(c2_node.status, NodeStatus::Running);
    assert_eq!(c2_node.native_session.map(|b| b.epoch), Some(1));
    let s1_node = reopened.node(&s1, dl()).expect("read").expect("s1 exists");
    assert_eq!(s1_node.result_digest, Some(binding().result_digest));
}

#[test]
fn immutable_columns_reject_raw_updates() {
    let scratch = ScratchNamespaces::default();
    let config = fresh_config(&scratch);
    let graph = PostgresInvocationGraph::open(&config).expect("graph");
    let (root, run_id) = (inv("1"), run("1"));
    admitted(
        graph
            .admit(root_request(&root, &run_id), dl())
            .expect("root"),
    );
    let (c1, c2) = (inv("2"), inv("3"));
    admitted(
        graph
            .admit(child_request(&c1, &run_id, &root, "key-1"), dl())
            .expect("c1"),
    );
    admitted(
        graph
            .admit(child_request(&c2, &run_id, &c1, "key-2"), dl())
            .expect("c2"),
    );
    drop(graph);

    let mut client = raw(&config);
    let attempts = [
        format!(
            "UPDATE invocation_nodes SET context_digest = 'sha256:ffff' WHERE invocation_id = '{c2}'"
        ),
        format!("UPDATE invocation_nodes SET parent_id = '{root}' WHERE invocation_id = '{c2}'"),
        format!(
            "UPDATE invocation_nodes SET result_contract = 'contract-b' WHERE invocation_id = '{c2}'"
        ),
    ];
    for sql in &attempts {
        let error = client
            .execute(sql.as_str(), &[])
            .expect_err("an immutable column must not change");
        assert!(
            error
                .as_db_error()
                .is_some_and(|db| db.message().contains("immutable")),
            "unexpected error for {sql}: {error}"
        );
    }
    // Mutable columns still move under the guard.
    client
        .execute(
            "UPDATE invocation_nodes SET status = 'running' WHERE invocation_id = $1",
            &[&c2.as_str()],
        )
        .expect("status is mutable");
}

#[test]
fn cancel_subtree_cancels_only_active_descendants() {
    let scratch = ScratchNamespaces::default();
    let config = fresh_config(&scratch);
    let graph = PostgresInvocationGraph::open(&config).expect("graph");
    let (root, run_id) = (inv("1"), run("1"));
    let (a, b, c, d) = (inv("2"), inv("3"), inv("4"), inv("5"));
    admitted(
        graph
            .admit(root_request(&root, &run_id), dl())
            .expect("root"),
    );
    let wide_a = request_with(
        &a,
        &run_id,
        Some(&root),
        "key-a",
        "harness-a",
        root_budget(),
        DEFAULT_MAX_CHILDREN,
    );
    admitted(graph.admit(wide_a, dl()).expect("a"));
    admitted(
        graph
            .admit(child_request(&b, &run_id, &a, "key-b"), dl())
            .expect("b"),
    );
    admitted(
        graph
            .admit(child_request(&c, &run_id, &a, "key-c"), dl())
            .expect("c"),
    );
    admitted(
        graph
            .admit(child_request(&d, &run_id, &c, "key-d"), dl())
            .expect("d"),
    );
    graph.mark_running(&b, dl()).expect("running");
    let completed_b = graph
        .complete(&b, binding(), EpochSeconds::new(COMPLETE_AT), dl())
        .expect("complete b");

    let cancelled = graph
        .cancel_subtree(&a, EpochSeconds::new(CANCEL_AT), dl())
        .expect("cancel");
    assert_eq!(cancelled, vec![a.clone(), c.clone(), d.clone()]);

    assert_eq!(graph.node(&b, dl()).expect("read"), Some(completed_b));
    let cancelled_c = graph.node(&c, dl()).expect("read").expect("c exists");
    assert_eq!(cancelled_c.status, NodeStatus::Cancelled);
    assert_eq!(cancelled_c.terminal_at, Some(EpochSeconds::new(CANCEL_AT)));
    assert_eq!(
        graph.node(&root, dl()).expect("read").expect("root").status,
        NodeStatus::Admitted,
        "the cancelled subtree does not reach its ancestors"
    );
    assert!(
        graph
            .cancel_subtree(&a, EpochSeconds::new(CANCEL_AT), dl())
            .expect("second cancel")
            .is_empty(),
        "a second cancel is a no-op"
    );
}

// E3a: harness lifecycle observations on PostgreSQL. Every test has its own
// namespace; nodes are fresh roots (one run each) so rounds never interfere.

const OBSERVE_AT: u64 = 1_800_000_050;
const LIFECYCLE_ROUNDS: u32 = 50;

fn observation(
    id: &InvocationId,
    native_session: Option<NativeSessionBinding>,
    state: LifecycleState,
    source: ObservationSource,
    sequence: ObservationSequence,
) -> HarnessObservation {
    HarnessObservation {
        invocation_id: id.clone(),
        native_session,
        state,
        source,
        observed_at: EpochSeconds::new(OBSERVE_AT),
        sequence,
    }
}

/// Admits a fresh root under its own run. The tail names both the node and the run.
fn fresh_root(graph: &PostgresInvocationGraph, tail: u32) -> InvocationId {
    let id = inv(&tail.to_string());
    let run_id = run(&tail.to_string());
    admitted(graph.admit(root_request(&id, &run_id), dl()).expect("root"));
    id
}

fn attach(graph: &PostgresInvocationGraph, id: &InvocationId, name: &str) -> NativeSessionBinding {
    graph
        .attach_native_session(id, SessionRef::new(name).expect("session"), dl())
        .expect("attach")
}

fn applied_sequences(history: &[gauntlet_ports::lifecycle::ObservationRecord]) -> Vec<u64> {
    history
        .iter()
        .filter_map(|record| match record.outcome {
            ObservationOutcome::Applied { sequence, .. } => Some(sequence),
            _ => None,
        })
        .collect()
}

#[test]
fn racing_late_and_new_source_sequences_keep_the_higher_number() {
    let scratch = ScratchNamespaces::default();
    let config = fresh_config(&scratch);
    let graph = PostgresInvocationGraph::open(&config).expect("graph");
    let ids: Vec<InvocationId> = (0..LIFECYCLE_ROUNDS)
        .map(|round| fresh_root(&graph, 100 + round))
        .collect();
    drop(graph);

    for (round, id) in ids.iter().enumerate() {
        // A deterministic mix of which thread submits the late number first.
        let late_first = (round * 7 + 3) % 5 < 3;
        let target = id.clone();
        let outcomes = on_connections(
            &config,
            2,
            Arc::new(move |graph: &PostgresInvocationGraph, index: usize| {
                let late = (index == 0) == late_first;
                let (sequence, state) = if late {
                    (42, LifecycleState::Working)
                } else {
                    (43, LifecycleState::Idle)
                };
                graph.record_observation(
                    observation(
                        &target,
                        None,
                        state,
                        ObservationSource::Structured,
                        ObservationSequence::SourceProvided(sequence),
                    ),
                    EpochSeconds::new(OBSERVE_AT),
                    dl(),
                )
            }),
        );
        for outcome in outcomes {
            outcome.expect("observation");
        }

        let graph = PostgresInvocationGraph::open(&config).expect("graph");
        let effective = graph
            .lifecycle(id, dl())
            .expect("view")
            .effective
            .expect("an effective entry");
        assert_eq!(
            (effective.state, effective.sequence),
            (LifecycleState::Idle, 43),
            "round {round}: the higher sequence must win"
        );
        let history = graph.observation_history(id, dl()).expect("history");
        let ignored: Vec<u64> = history
            .iter()
            .filter_map(|record| match record.outcome {
                ObservationOutcome::IgnoredStale { last_applied } => Some(last_applied),
                _ => None,
            })
            .collect();
        match (applied_sequences(&history).as_slice(), ignored.as_slice()) {
            ([42, 43], []) | ([43], [43]) => {}
            other => panic!("round {round}: unexpected outcomes {other:?}"),
        }
    }
}

#[test]
fn gauntlet_assigned_sequences_are_exact_and_unique_under_contention() {
    let scratch = ScratchNamespaces::default();
    let config = fresh_config(&scratch);
    let graph = PostgresInvocationGraph::open(&config).expect("graph");
    let id = fresh_root(&graph, 1);
    drop(graph);

    let target = id.clone();
    let batches = on_connections(
        &config,
        8,
        Arc::new(move |graph: &PostgresInvocationGraph, _index: usize| {
            (0..5)
                .map(|_| {
                    graph.record_observation(
                        observation(
                            &target,
                            None,
                            LifecycleState::Working,
                            ObservationSource::Screen,
                            ObservationSequence::GauntletAssigned,
                        ),
                        EpochSeconds::new(OBSERVE_AT),
                        dl(),
                    )
                })
                .collect::<Vec<_>>()
        }),
    );
    let mut sequences = Vec::new();
    for outcome in batches.into_iter().flatten() {
        match outcome.expect("observation") {
            ObservationOutcome::Applied { sequence, .. } => sequences.push(sequence),
            other => panic!("a Gauntlet-assigned observation must apply, got {other:?}"),
        }
    }
    sequences.sort_unstable();
    assert_eq!(sequences, (1..=40).collect::<Vec<u64>>());

    let graph = PostgresInvocationGraph::open(&config).expect("graph");
    assert_eq!(
        graph.observation_history(&id, dl()).expect("history").len(),
        40
    );
    let effective = graph
        .lifecycle(&id, dl())
        .expect("view")
        .effective
        .expect("an effective entry");
    assert_eq!(effective.sequence, 40);
}

enum Step {
    Attached(NativeSessionBinding),
    Observed(ObservationOutcome),
}

#[test]
fn a_session_replacement_racing_an_old_epoch_observation_never_shows_that_epoch() {
    let scratch = ScratchNamespaces::default();
    let config = fresh_config(&scratch);
    let graph = PostgresInvocationGraph::open(&config).expect("graph");
    for round in 0..20 {
        let id = fresh_root(&graph, 200 + round);
        let first = attach(&graph, &id, "sess-1");
        assert_eq!(first.epoch, 1);
        let (target, old) = (id.clone(), first.clone());
        let steps = on_connections(
            &config,
            2,
            Arc::new(move |graph: &PostgresInvocationGraph, index: usize| {
                if index == 0 {
                    graph
                        .attach_native_session(
                            &target,
                            SessionRef::new("sess-2").expect("session"),
                            dl(),
                        )
                        .map(Step::Attached)
                } else {
                    graph
                        .record_observation(
                            observation(
                                &target,
                                Some(old.clone()),
                                LifecycleState::Working,
                                ObservationSource::Structured,
                                ObservationSequence::SourceProvided(999),
                            ),
                            EpochSeconds::new(OBSERVE_AT),
                            dl(),
                        )
                        .map(Step::Observed)
                }
            }),
        );
        for step in steps {
            match step.expect("step") {
                Step::Attached(binding) => assert_eq!(binding.epoch, 2, "round {round}"),
                Step::Observed(outcome) => assert!(
                    matches!(
                        outcome,
                        ObservationOutcome::Applied { .. }
                            | ObservationOutcome::RejectedStaleSession { .. }
                    ),
                    "round {round}: the old-epoch observation was {outcome:?}"
                ),
            }
        }

        let view = graph.lifecycle(&id, dl()).expect("view");
        assert!(
            view.per_source.iter().all(|entry| entry.epoch == 2),
            "round {round}: an epoch-1 entry leaked into the current view"
        );
        assert!(
            view.effective.is_none(),
            "round {round}: epoch 2 has no observations"
        );
        let history = graph.observation_history(&id, dl()).expect("history");
        assert_eq!(history.len(), 1, "round {round}");
        match &history[0].outcome {
            ObservationOutcome::Applied {
                sequence: 999,
                previous: None,
            } => {}
            ObservationOutcome::RejectedStaleSession {
                current: Some(bound),
            } if bound.epoch == 2 => {}
            other => panic!("round {round}: unexpected outcome {other:?}"),
        }
    }
}

enum Racer {
    Completion(Result<NodeStatus, GraphError>),
    Observations(Vec<Result<ObservationOutcome, GraphError>>),
}

#[test]
fn completion_racing_observations_settles_once_and_later_observations_are_terminal() {
    let scratch = ScratchNamespaces::default();
    let config = fresh_config(&scratch);
    let graph = PostgresInvocationGraph::open(&config).expect("graph");
    for round in 0..10 {
        let id = fresh_root(&graph, 300 + round);
        graph.mark_running(&id, dl()).expect("running");
        let bound = attach(&graph, &id, "sess-1");
        let (target, old) = (id.clone(), bound.clone());
        let racers = on_connections(
            &config,
            5,
            Arc::new(move |graph: &PostgresInvocationGraph, index: usize| {
                if index == 0 {
                    Racer::Completion(
                        graph
                            .complete(&target, binding(), EpochSeconds::new(COMPLETE_AT), dl())
                            .map(|node| node.status),
                    )
                } else {
                    Racer::Observations(
                        (0..5)
                            .map(|_| {
                                graph.record_observation(
                                    observation(
                                        &target,
                                        Some(old.clone()),
                                        LifecycleState::Working,
                                        ObservationSource::Process,
                                        ObservationSequence::GauntletAssigned,
                                    ),
                                    EpochSeconds::new(OBSERVE_AT),
                                    dl(),
                                )
                            })
                            .collect(),
                    )
                }
            }),
        );
        let mut completions = 0;
        for racer in racers {
            match racer {
                Racer::Completion(result) => {
                    let status = result.expect("the completion is accepted");
                    assert_eq!(status, NodeStatus::Completed, "round {round}");
                    completions += 1;
                }
                Racer::Observations(outcomes) => {
                    for outcome in outcomes {
                        outcome.expect("observation");
                    }
                }
            }
        }
        assert_eq!(completions, 1, "round {round}");

        let graph = PostgresInvocationGraph::open(&config).expect("graph");
        let node = graph.node(&id, dl()).expect("read").expect("node");
        assert_eq!(node.status, NodeStatus::Completed, "round {round}");
        assert!(
            matches!(
                graph.complete(&id, binding(), EpochSeconds::new(COMPLETE_AT), dl()),
                Err(GraphError::IllegalTransition { .. })
            ),
            "round {round}: a second completion must be refused"
        );
        let history = graph.observation_history(&id, dl()).expect("history");
        assert_eq!(
            history.len(),
            20,
            "round {round}: every observation is recorded"
        );
        if let Some(cut) = history
            .iter()
            .position(|record| record.outcome == ObservationOutcome::IgnoredTerminal)
        {
            assert!(
                history[cut..]
                    .iter()
                    .all(|record| record.outcome == ObservationOutcome::IgnoredTerminal),
                "round {round}: an applied observation followed the terminal state"
            );
        }
        assert!(
            history.iter().all(|record| matches!(
                record.outcome,
                ObservationOutcome::Applied { .. } | ObservationOutcome::IgnoredTerminal
            )),
            "round {round}: unexpected outcome in a binding-correct race"
        );

        let view = graph.lifecycle(&id, dl()).expect("view");
        graph
            .record_observation(
                observation(
                    &id,
                    Some(bound.clone()),
                    LifecycleState::Done,
                    ObservationSource::Structured,
                    ObservationSequence::GauntletAssigned,
                ),
                EpochSeconds::new(OBSERVE_AT),
                dl(),
            )
            .expect("observation");
        assert_eq!(
            graph.lifecycle(&id, dl()).expect("view"),
            view,
            "round {round}: the view must not change after completion"
        );
    }
}

#[test]
fn observations_never_change_node_status_budget_or_binding() {
    let scratch = ScratchNamespaces::default();
    let config = fresh_config(&scratch);
    let graph = PostgresInvocationGraph::open(&config).expect("graph");
    let id = fresh_root(&graph, 400);
    let bound = attach(&graph, &id, "sess-1");
    let before = graph.node(&id, dl()).expect("read").expect("node");

    for (state, source, sequence) in [
        (LifecycleState::Done, ObservationSource::Structured, 1),
        (LifecycleState::Idle, ObservationSource::Process, 1),
    ] {
        let outcome = graph
            .record_observation(
                observation(
                    &id,
                    Some(bound.clone()),
                    state,
                    source,
                    ObservationSequence::SourceProvided(sequence),
                ),
                EpochSeconds::new(OBSERVE_AT),
                dl(),
            )
            .expect("observation");
        assert!(matches!(outcome, ObservationOutcome::Applied { .. }));
    }
    assert_eq!(
        graph.node(&id, dl()).expect("read").expect("node"),
        before,
        "observations must not touch the node row"
    );
    let effective = graph
        .lifecycle(&id, dl())
        .expect("view")
        .effective
        .expect("effective");
    assert_eq!(
        (effective.state, effective.source),
        (LifecycleState::Done, ObservationSource::Structured)
    );

    graph.mark_running(&id, dl()).expect("running");
    graph
        .record_observation(
            observation(
                &id,
                Some(bound.clone()),
                LifecycleState::Done,
                ObservationSource::Structured,
                ObservationSequence::SourceProvided(2),
            ),
            EpochSeconds::new(OBSERVE_AT),
            dl(),
        )
        .expect("observation");
    let wrong = ResultBinding {
        contract_path: ResultContractPath::new("contract-b").expect("contract"),
        context_digest: Digest::parse(DIGEST).expect("digest"),
        result_digest: Digest::parse("sha256:e5f6a7b8").expect("digest"),
    };
    assert!(
        matches!(
            graph.complete(&id, wrong, EpochSeconds::new(COMPLETE_AT), dl()),
            Err(GraphError::ContractMismatch { .. })
        ),
        "a Done observation must not stand in for the result binding"
    );
    let node = graph
        .complete(&id, binding(), EpochSeconds::new(COMPLETE_AT), dl())
        .expect("the right binding completes");
    assert_eq!(node.status, NodeStatus::Completed);
}

#[test]
fn history_and_lifecycle_refuse_raw_update_and_delete() {
    let scratch = ScratchNamespaces::default();
    let config = fresh_config(&scratch);
    let graph = PostgresInvocationGraph::open(&config).expect("graph");
    let id = fresh_root(&graph, 500);
    let bound = attach(&graph, &id, "sess-1");
    graph
        .record_observation(
            observation(
                &id,
                Some(bound),
                LifecycleState::Working,
                ObservationSource::Structured,
                ObservationSequence::SourceProvided(1),
            ),
            EpochSeconds::new(OBSERVE_AT),
            dl(),
        )
        .expect("observation");
    drop(graph);

    let mut client = raw(&config);
    let refused = [
        (
            "UPDATE invocation_observations SET state = 'idle'",
            "append-only",
        ),
        ("DELETE FROM invocation_observations", "append-only"),
        ("DELETE FROM invocation_lifecycle", "never deleted"),
    ];
    for (sql, expected) in refused {
        let error = client
            .execute(sql, &[])
            .expect_err("the append-only guard must refuse this");
        assert!(
            error
                .as_db_error()
                .is_some_and(|db| db.message().contains(expected)),
            "unexpected error for {sql}: {error}"
        );
    }
    client
        .execute(
            "UPDATE invocation_lifecycle SET observed_at = observed_at",
            &[],
        )
        .expect("lifecycle rows may be advanced in place");
}

#[test]
fn lifecycle_and_history_reconstruct_identically_after_reopen() {
    let path = PathBuf::from(format!(
        "/nonexistent-gauntlet-e3a/{}-{:x}/gauntlet-state.db",
        std::process::id(),
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0)
    ));
    let graph = PostgresInvocationGraph::open_at(&path).expect("graph");
    let id = fresh_root(&graph, 600);
    let first = attach(&graph, &id, "sess-1");
    let stale_binding = NativeSessionBinding {
        session: SessionRef::new("sess-0").expect("session"),
        epoch: 1,
    };
    let obs = |native: Option<NativeSessionBinding>, state, source, sequence| {
        observation(&id, native, state, source, sequence)
    };
    let at = |o: HarnessObservation| {
        graph
            .record_observation(o, EpochSeconds::new(OBSERVE_AT), dl())
            .expect("observation")
    };
    at(obs(
        Some(first.clone()),
        LifecycleState::Working,
        ObservationSource::Structured,
        ObservationSequence::SourceProvided(1),
    ));
    at(obs(
        Some(first.clone()),
        LifecycleState::Idle,
        ObservationSource::Structured,
        ObservationSequence::SourceProvided(2),
    ));
    at(obs(
        Some(first.clone()),
        LifecycleState::Blocked,
        ObservationSource::Structured,
        ObservationSequence::SourceProvided(1),
    ));
    at(obs(
        Some(first.clone()),
        LifecycleState::Blocked,
        ObservationSource::Screen,
        ObservationSequence::GauntletAssigned,
    ));
    at(obs(
        Some(first.clone()),
        LifecycleState::Done,
        ObservationSource::Screen,
        ObservationSequence::GauntletAssigned,
    ));
    at(obs(
        None,
        LifecycleState::Working,
        ObservationSource::Process,
        ObservationSequence::GauntletAssigned,
    ));
    at(obs(
        Some(stale_binding),
        LifecycleState::Working,
        ObservationSource::Process,
        ObservationSequence::SourceProvided(7),
    ));
    let second = attach(&graph, &id, "sess-2");
    assert_eq!(second.epoch, 2);
    at(obs(
        Some(first.clone()),
        LifecycleState::Working,
        ObservationSource::Structured,
        ObservationSequence::SourceProvided(9),
    ));
    at(obs(
        Some(second.clone()),
        LifecycleState::Working,
        ObservationSource::Structured,
        ObservationSequence::SourceProvided(5),
    ));
    at(obs(
        Some(second.clone()),
        LifecycleState::Unknown,
        ObservationSource::Process,
        ObservationSequence::GauntletAssigned,
    ));
    graph
        .complete(&id, binding(), EpochSeconds::new(COMPLETE_AT), dl())
        .expect("complete");
    at(obs(
        Some(second.clone()),
        LifecycleState::Done,
        ObservationSource::Structured,
        ObservationSequence::SourceProvided(6),
    ));

    let snapshot = |graph: &PostgresInvocationGraph| {
        (
            graph.lifecycle(&id, dl()).expect("view"),
            graph.observation_history(&id, dl()).expect("history"),
            graph.node(&id, dl()).expect("node"),
        )
    };
    let before = snapshot(&graph);
    let outcomes: Vec<&ObservationOutcome> = before.1.iter().map(|r| &r.outcome).collect();
    assert!(
        outcomes
            .iter()
            .any(|o| matches!(o, ObservationOutcome::IgnoredStale { .. }))
    );
    assert!(
        outcomes
            .iter()
            .any(|o| matches!(o, ObservationOutcome::RejectedStaleSession { .. }))
    );
    assert!(
        outcomes
            .iter()
            .any(|o| matches!(o, ObservationOutcome::IgnoredTerminal))
    );
    drop(graph);

    let reopened = PostgresInvocationGraph::open_at(&path).expect("reopened graph");
    assert_eq!(
        snapshot(&reopened),
        before,
        "reconstruction differs after reopen"
    );
}
