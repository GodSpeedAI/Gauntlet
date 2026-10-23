//! The `InvocationGraph` shared contract suite (spec §6, E2 design).
//!
//! Pinned rules: every refusal and every rejected transition leaves the
//! graph exactly as it was (re-read after the refusal); terminal states are
//! final; admission is idempotent on (run, admission key) with identical
//! immutable facts; the budget rule is componentwise over live children; a
//! completion must present the node's own admitted contract path and
//! context digest; `cancel_subtree` is one atomic step over the active part
//! of a subtree.

use gauntlet_domain::{
    AdmissionKey, BudgetDimension, Digest, EpochSeconds, HarnessId, InvocationId, MicroUsd,
    ResultContractPath, RoleId, RunId, RunnerProfileId, SessionRef, WorkspaceRef,
};

use std::fmt::Debug;

use crate::agent_runner::InvocationFailure;
use crate::budget_meter::BudgetSlice;
use crate::contract::ContractFinding;
use crate::deadline::Deadline;
use crate::invocation_graph::{
    AdmissionOutcome, AdmissionRefusal, AdmissionRequest, GraphError, GraphLimits, InvocationGraph,
    InvocationNode, NativeSessionBinding, NodeStatus, ResultBinding,
};
use crate::lifecycle::{
    HarnessObservation, LifecycleState, LifecycleView, ObservationOutcome, ObservationRecord,
    ObservationSequence, ObservationSource, SequenceOrigin,
};

/// The suite's name in findings.
pub const SUITE: &str = "invocation_graph";

/// Constructs a fresh, empty graph for each case.
pub trait InvocationGraphFactory {
    fn label(&self) -> &'static str;

    fn build(&self) -> Result<Box<dyn InvocationGraph>, String>;
}

const RUN_PREFIX: &str = "01J9R";
const NODE_PREFIX: &str = "01J9N";
const FAR_AT: u64 = 1_900_000_000;
const ADMIT_AT: u64 = 1_800_000_000;
const REPLAY_AT: u64 = 1_800_000_001;
const COMPLETE_AT: u64 = 1_800_000_100;
const CANCEL_AT: u64 = 1_800_000_200;

const HARNESS_A: &str = "harness-a";
const HARNESS_B: &str = "harness-b";
const HARNESS_C: &str = "harness-c";
const PROFILE: &str = "profile-1";
const WORKSPACE: &str = "workspace-1";
const CONTRACT_A: &str = "contract-a";
const CONTRACT_B: &str = "contract-b";
const DIGEST_A: &str = "sha256:a1b2c3d4";
const DIGEST_B: &str = "sha256:e5f6a7b8";
const SESSION_ONE: &str = "session-1";
const SESSION_TWO: &str = "session-2";

const ROLE_ORCHESTRATOR: &str = "orchestrator";
const ROLE_BUILDER: &str = "builder";
const ROLE_CRITIC: &str = "critic";

const ROOT_ROUNDS: u32 = 4;
const ROOT_WALL_BUDGET: u64 = 600;
const ROOT_TOKENS: u64 = 1000;
const ROOT_COST: u64 = 100;
const CHILD_ROUNDS: u32 = 1;
const CHILD_WALL_BUDGET: u64 = 60;
const CHILD_TOKENS: u64 = 200;
const CHILD_COST: u64 = 10;
const HEAVY_TOKENS: u64 = 600;

const TAIL_WIDTH: usize = 21;
const DEFAULT_MAX_DEPTH: u32 = 8;
const DEFAULT_MAX_CHILDREN: u32 = 8;
const SHALLOW_MAX_DEPTH: u32 = 1;
const ONE_CHILD: u32 = 1;

fn finding(case: &'static str, rule: &'static str, detail: String) -> ContractFinding {
    ContractFinding::new(SUITE, case, rule, detail)
}

/// Runs one case body; an `Err` becomes a finding for the case.
fn check(
    case: &'static str,
    rule: &'static str,
    body: impl FnOnce() -> Result<(), String>,
) -> Vec<ContractFinding> {
    match body() {
        Ok(()) => Vec::new(),
        Err(detail) => vec![finding(case, rule, detail)],
    }
}

fn ensure(condition: bool, detail: impl FnOnce() -> String) -> Result<(), String> {
    if condition { Ok(()) } else { Err(detail()) }
}

fn g<T>(result: Result<T, GraphError>) -> Result<T, String> {
    result.map_err(|error| error.to_string())
}

fn build(factory: &dyn InvocationGraphFactory) -> Result<Box<dyn InvocationGraph>, String> {
    factory.build()
}

fn dl() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(FAR_AT))
}

fn ulid(prefix: &str, tail: &str) -> String {
    format!("{prefix}{tail:0>width$}", width = TAIL_WIDTH)
}

fn inv(tail: &str) -> InvocationId {
    InvocationId::new(&ulid(NODE_PREFIX, tail)).expect("suite invocation id is a valid ULID")
}

fn run(tail: &str) -> RunId {
    RunId::new(&ulid(RUN_PREFIX, tail)).expect("suite run id is a valid ULID")
}

fn role(token: &str) -> RoleId {
    RoleId::parse(token).expect("suite role is a named role")
}

fn root_slice() -> BudgetSlice {
    BudgetSlice {
        rounds: ROOT_ROUNDS,
        wall_seconds: ROOT_WALL_BUDGET,
        tokens: ROOT_TOKENS,
        cost_usd: MicroUsd::new(ROOT_COST),
    }
}

fn child_slice() -> BudgetSlice {
    BudgetSlice {
        rounds: CHILD_ROUNDS,
        wall_seconds: CHILD_WALL_BUDGET,
        tokens: CHILD_TOKENS,
        cost_usd: MicroUsd::new(CHILD_COST),
    }
}

fn limits() -> GraphLimits {
    GraphLimits {
        max_depth: DEFAULT_MAX_DEPTH,
        max_children_per_parent: DEFAULT_MAX_CHILDREN,
    }
}

/// A child request with suite defaults: builder role, harness A, contract A,
/// digest A, the child slice, default limits.
fn request(
    id: &InvocationId,
    run_id: &RunId,
    parent: Option<&InvocationId>,
    key: &str,
) -> AdmissionRequest {
    AdmissionRequest {
        invocation_id: id.clone(),
        run_id: run_id.clone(),
        parent: parent.cloned(),
        role: role(ROLE_BUILDER),
        runner_profile: RunnerProfileId::new(PROFILE).expect("suite profile id is valid"),
        harness: HarnessId::new(HARNESS_A).expect("suite harness id is valid"),
        workspace: WorkspaceRef::new(WORKSPACE).expect("suite workspace is valid"),
        context_digest: Digest::parse(DIGEST_A).expect("suite digest is valid"),
        result_contract: ResultContractPath::new(CONTRACT_A).expect("suite contract is valid"),
        admission_key: AdmissionKey::new(key).expect("suite admission key is valid"),
        budget: child_slice(),
        limits: limits(),
        at: EpochSeconds::new(ADMIT_AT),
    }
}

/// A root request: orchestrator role, the run's budget slice, no parent.
fn root_request(id: &InvocationId, run_id: &RunId, key: &str) -> AdmissionRequest {
    let mut root = request(id, run_id, None, key);
    root.role = role(ROLE_ORCHESTRATOR);
    root.budget = root_slice();
    root
}

fn binding(contract: &str, digest: &str) -> ResultBinding {
    ResultBinding {
        contract_path: ResultContractPath::new(contract).expect("suite contract is valid"),
        context_digest: Digest::parse(digest).expect("suite digest is valid"),
        result_digest: Digest::parse(DIGEST_B).expect("suite digest is valid"),
    }
}

fn session(name: &str) -> SessionRef {
    SessionRef::new(name).expect("suite session ref is valid")
}

fn ids(nodes: &[InvocationNode]) -> Vec<InvocationId> {
    nodes.iter().map(|n| n.invocation_id.clone()).collect()
}

fn admit_ok(
    graph: &dyn InvocationGraph,
    request: AdmissionRequest,
) -> Result<InvocationNode, String> {
    match g(graph.admit(request, dl()))? {
        AdmissionOutcome::Admitted(node) => Ok(node),
        other => Err(format!("expected Admitted, got {other:?}")),
    }
}

fn admit_refused(
    graph: &dyn InvocationGraph,
    request: AdmissionRequest,
) -> Result<AdmissionRefusal, String> {
    match g(graph.admit(request, dl()))? {
        AdmissionOutcome::Refused(refusal) => Ok(refusal),
        other => Err(format!("expected Refused, got {other:?}")),
    }
}

fn snapshot(
    graph: &dyn InvocationGraph,
    subjects: &[&InvocationId],
) -> Result<Vec<Option<InvocationNode>>, String> {
    subjects.iter().map(|id| g(graph.node(id, dl()))).collect()
}

/// Re-reads the subjects and requires them to equal the earlier snapshot.
fn ensure_unchanged(
    graph: &dyn InvocationGraph,
    subjects: &[&InvocationId],
    before: &[Option<InvocationNode>],
    what: &str,
) -> Result<(), String> {
    let after = snapshot(graph, subjects)?;
    ensure(after == before, || {
        format!("{what}: the graph changed across a refused operation")
    })
}

fn is_illegal<T>(result: Result<T, GraphError>) -> bool {
    matches!(result, Err(GraphError::IllegalTransition { .. }))
}

/// Case: a root admission is Admitted, roots its own lineage at depth 0,
/// takes the run's budget slice, and reads back identically.
fn root_admission_records_a_root(factory: &dyn InvocationGraphFactory) -> Vec<ContractFinding> {
    const CASE: &str = "root_admission_records_a_root";
    const RULE: &str = "E2 design: admit(parent=None) roots the lineage at itself, depth 0, with the run's budget slice";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let root = inv("1");
        let run_id = run("1");
        let node = admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        ensure(
            node.parent.is_none() && node.root == root && node.depth == 0,
            || {
                format!(
                    "root has parent {:?}, root {}, depth {}",
                    node.parent, node.root, node.depth
                )
            },
        )?;
        ensure(node.status == NodeStatus::Admitted, || {
            format!("a fresh root is {}, not admitted", node.status)
        })?;
        ensure(node.budget == root_slice(), || {
            "the root did not keep its own budget slice".to_owned()
        })?;
        ensure(g(graph.node(&root, dl()))? == Some(node), || {
            "node() does not return the admitted root".to_owned()
        })
    })
}

/// Case: a child records its parent, the lineage root, and depth one.
fn child_admission_records_parent_root_and_depth(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "child_admission_records_parent_root_and_depth";
    const RULE: &str =
        "E2 design: a child's node carries its parent, the root's id, and depth parent+1";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let root = inv("1");
        let child = inv("2");
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        let node = admit_ok(
            graph.as_ref(),
            request(&child, &run_id, Some(&root), "key-child"),
        )?;
        ensure(node.parent.as_ref() == Some(&root), || {
            format!("child parent is {:?}, not the root", node.parent)
        })?;
        ensure(node.root == root && node.depth == 1, || {
            format!(
                "child root {} depth {}, want root and depth 1",
                node.root, node.depth
            )
        })?;
        let kids = g(graph.children(&root, dl()))?;
        ensure(ids(&kids) == vec![child.clone()], || {
            format!("children(root) is {:?}, want the one child", ids(&kids))
        })
    })
}

/// Case: a child past the depth limit is refused with DepthLimit and no node
/// is created.
fn depth_limit_refusal_creates_no_node(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "depth_limit_refusal_creates_no_node";
    const RULE: &str = "E2 design: admit refuses a child deeper than max_depth and creates nothing";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, child, grandchild) = (inv("1"), inv("2"), inv("3"));
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        admit_ok(
            graph.as_ref(),
            request(&child, &run_id, Some(&root), "key-child"),
        )?;
        let before = snapshot(graph.as_ref(), &[&root, &child, &grandchild])?;
        let mut deep = request(&grandchild, &run_id, Some(&child), "key-grandchild");
        deep.limits = GraphLimits {
            max_depth: SHALLOW_MAX_DEPTH,
            max_children_per_parent: DEFAULT_MAX_CHILDREN,
        };
        let refusal = admit_refused(graph.as_ref(), deep)?;
        ensure(
            refusal
                == AdmissionRefusal::DepthLimit {
                    max: SHALLOW_MAX_DEPTH,
                    would_be: SHALLOW_MAX_DEPTH + 1,
                },
            || format!("refusal is {refusal:?}, want DepthLimit at the shallow maximum"),
        )?;
        ensure_unchanged(
            graph.as_ref(),
            &[&root, &child, &grandchild],
            &before,
            "depth limit",
        )?;
        ensure(g(graph.children(&child, dl()))?.is_empty(), || {
            "a depth-refused child appeared under its parent".to_owned()
        })
    })
}

/// Case: a child beyond the parent's live-child limit is refused with
/// ChildLimit and the graph is unchanged.
fn child_limit_refusal_leaves_graph_unchanged(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "child_limit_refusal_leaves_graph_unchanged";
    const RULE: &str = "E2 design: admit refuses when the parent already holds max_children_per_parent live children";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, first, second) = (inv("1"), inv("2"), inv("3"));
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        let mut only_one = request(&first, &run_id, Some(&root), "key-first");
        only_one.limits = GraphLimits {
            max_depth: DEFAULT_MAX_DEPTH,
            max_children_per_parent: ONE_CHILD,
        };
        admit_ok(graph.as_ref(), only_one)?;
        let before = snapshot(graph.as_ref(), &[&root, &first, &second])?;
        let mut crowded = request(&second, &run_id, Some(&root), "key-second");
        crowded.limits = GraphLimits {
            max_depth: DEFAULT_MAX_DEPTH,
            max_children_per_parent: ONE_CHILD,
        };
        let refusal = admit_refused(graph.as_ref(), crowded)?;
        ensure(
            refusal == AdmissionRefusal::ChildLimit { max: ONE_CHILD },
            || format!("refusal is {refusal:?}, want ChildLimit at one"),
        )?;
        ensure_unchanged(
            graph.as_ref(),
            &[&root, &first, &second],
            &before,
            "child limit",
        )
    })
}

/// Case: a slice exceeding the parent's budget in one dimension is refused
/// with that dimension, for every dimension; the exact-fit slice is admitted.
fn budget_exceeded_each_dimension(factory: &dyn InvocationGraphFactory) -> Vec<ContractFinding> {
    const CASE: &str = "budget_exceeded_each_dimension";
    const RULE: &str = "E2 design: the live children's reserved slices plus the request must fit the parent budget componentwise";
    check(CASE, RULE, || {
        let over = [
            (
                BudgetDimension::Rounds,
                BudgetSlice {
                    rounds: ROOT_ROUNDS + 1,
                    ..child_slice()
                },
            ),
            (
                BudgetDimension::WallSeconds,
                BudgetSlice {
                    wall_seconds: ROOT_WALL_BUDGET + 1,
                    ..child_slice()
                },
            ),
            (
                BudgetDimension::Tokens,
                BudgetSlice {
                    tokens: ROOT_TOKENS + 1,
                    ..child_slice()
                },
            ),
            (
                BudgetDimension::CostUsd,
                BudgetSlice {
                    cost_usd: MicroUsd::new(ROOT_COST + 1),
                    ..child_slice()
                },
            ),
        ];
        let run_id = run("1");
        let root = inv("1");
        for (index, (dimension, slice)) in over.into_iter().enumerate() {
            let graph = build(factory)?;
            admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
            let child = inv(&format!("{}", index + 2));
            let mut greedy = request(&child, &run_id, Some(&root), "key-greedy");
            greedy.budget = slice;
            let refusal = admit_refused(graph.as_ref(), greedy)?;
            ensure(
                refusal == AdmissionRefusal::BudgetExceeded { dimension },
                || format!("over-budget in {dimension} was refused as {refusal:?}"),
            )?;
            ensure(g(graph.children(&root, dl()))?.is_empty(), || {
                format!("a {dimension}-refused child was created")
            })?;
        }
        let graph = build(factory)?;
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        let mut exact = request(&inv("9"), &run_id, Some(&root), "key-exact");
        exact.budget = root_slice();
        admit_ok(graph.as_ref(), exact)
            .map(|_| ())
            .map_err(|detail| {
                format!("a slice equal to the whole parent budget was refused: {detail}")
            })
    })
}

/// Case: a terminal child releases its reservation, so a new sibling that
/// previously did not fit is admitted.
fn budget_released_when_child_turns_terminal(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "budget_released_when_child_turns_terminal";
    const RULE: &str = "E2 design: only NON-terminal children reserve parent budget";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, first, second) = (inv("1"), inv("2"), inv("3"));
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        let mut heavy = request(&first, &run_id, Some(&root), "key-first");
        heavy.budget = BudgetSlice {
            tokens: HEAVY_TOKENS,
            ..child_slice()
        };
        admit_ok(graph.as_ref(), heavy.clone())?;
        let mut sibling = request(&second, &run_id, Some(&root), "key-second");
        sibling.budget = BudgetSlice {
            tokens: HEAVY_TOKENS,
            ..child_slice()
        };
        let refusal = admit_refused(graph.as_ref(), sibling.clone())?;
        ensure(
            refusal
                == AdmissionRefusal::BudgetExceeded {
                    dimension: BudgetDimension::Tokens,
                },
            || format!("a sibling over the live reservation was refused as {refusal:?}"),
        )?;
        g(graph.fail(
            &first,
            InvocationFailure::Execution,
            EpochSeconds::new(COMPLETE_AT),
            dl(),
        ))?;
        admit_ok(graph.as_ref(), sibling)
            .map(|_| ())
            .map_err(|detail| {
                format!("the sibling still did not fit after the first child ended: {detail}")
            })
    })
}

/// Case: replaying the same key and facts returns AlreadyAdmitted with the
/// original node and creates no second node. The replay's `at` is not a fact.
fn idempotent_replay_returns_already_admitted(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "idempotent_replay_returns_already_admitted";
    const RULE: &str = "E2 design: same (run, admission key) with identical facts is exactly-once logical admission";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, child) = (inv("1"), inv("2"));
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        let original = request(&child, &run_id, Some(&root), "key-child");
        let first = admit_ok(graph.as_ref(), original.clone())?;
        let mut replay = original;
        replay.at = EpochSeconds::new(REPLAY_AT);
        match g(graph.admit(replay, dl()))? {
            AdmissionOutcome::AlreadyAdmitted(node) => ensure(node == first, || {
                "the replay returned a node that differs from the original".to_owned()
            })?,
            other => return Err(format!("replay returned {other:?}, want AlreadyAdmitted")),
        }
        ensure(g(graph.children(&root, dl()))?.len() == 1, || {
            "the replay created a second child node".to_owned()
        })
    })
}

/// Case: reusing a key for a different harness is refused and the original
/// node keeps its facts.
fn key_reuse_with_different_facts_refused(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "key_reuse_with_different_facts_refused";
    const RULE: &str = "E2 design: same admission key with different immutable facts is refused";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, child) = (inv("1"), inv("2"));
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        admit_ok(
            graph.as_ref(),
            request(&child, &run_id, Some(&root), "key-child"),
        )?;
        let before = snapshot(graph.as_ref(), &[&child])?;
        let mut other = request(&child, &run_id, Some(&root), "key-child");
        other.harness = HarnessId::new(HARNESS_B).expect("suite harness id is valid");
        let refusal = admit_refused(graph.as_ref(), other)?;
        ensure(
            refusal == AdmissionRefusal::KeyReusedWithDifferentFacts,
            || format!("key reuse was refused as {refusal:?}"),
        )?;
        ensure_unchanged(graph.as_ref(), &[&child], &before, "key reuse")
    })
}

/// Case: a second root in the same run is refused; a root in another run is
/// its own admission.
fn second_root_refused(factory: &dyn InvocationGraphFactory) -> Vec<ContractFinding> {
    const CASE: &str = "second_root_refused";
    const RULE: &str = "E2 design: exactly one root per run";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let (run_one, run_two) = (run("1"), run("2"));
        let (root, second_root, other_root) = (inv("1"), inv("2"), inv("3"));
        admit_ok(graph.as_ref(), root_request(&root, &run_one, "key-root"))?;
        let before = snapshot(graph.as_ref(), &[&second_root])?;
        let refusal = admit_refused(
            graph.as_ref(),
            root_request(&second_root, &run_one, "key-second-root"),
        )?;
        ensure(refusal == AdmissionRefusal::RootAlreadyExists, || {
            format!("second root was refused as {refusal:?}")
        })?;
        ensure_unchanged(graph.as_ref(), &[&second_root], &before, "second root")?;
        admit_ok(
            graph.as_ref(),
            root_request(&other_root, &run_two, "key-other-root"),
        )
        .map(|_| ())
        .map_err(|detail| format!("a root in another run was refused: {detail}"))
    })
}

/// Case: admission under a terminal parent is refused.
fn admission_under_terminal_parent_refused(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "admission_under_terminal_parent_refused";
    const RULE: &str = "E2 design: no new work under a terminal parent (ParentTerminal)";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, child) = (inv("1"), inv("2"));
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        g(graph.fail(
            &root,
            InvocationFailure::Execution,
            EpochSeconds::new(COMPLETE_AT),
            dl(),
        ))?;
        let before = snapshot(graph.as_ref(), &[&root, &child])?;
        let refusal = admit_refused(
            graph.as_ref(),
            request(&child, &run_id, Some(&root), "key-child"),
        )?;
        ensure(refusal == AdmissionRefusal::ParentTerminal, || {
            format!("admission under a failed parent was refused as {refusal:?}")
        })?;
        ensure_unchanged(graph.as_ref(), &[&root, &child], &before, "terminal parent")
    })
}

/// Case: a parent that the graph does not hold is refused as UnknownParent,
/// and a parent from another run as WrongRun.
fn unknown_and_foreign_parents_refused(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "unknown_and_foreign_parents_refused";
    const RULE: &str = "E2 design: the parent must exist and belong to the request's run";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let (run_one, run_two) = (run("1"), run("2"));
        let (root_one, root_two, stray) = (inv("1"), inv("2"), inv("3"));
        admit_ok(
            graph.as_ref(),
            root_request(&root_one, &run_one, "key-root-one"),
        )?;
        admit_ok(
            graph.as_ref(),
            root_request(&root_two, &run_two, "key-root-two"),
        )?;
        let unknown = admit_refused(
            graph.as_ref(),
            request(&stray, &run_one, Some(&inv("7")), "key-stray"),
        )?;
        ensure(unknown == AdmissionRefusal::UnknownParent, || {
            format!("unknown parent was refused as {unknown:?}")
        })?;
        let foreign = admit_refused(
            graph.as_ref(),
            request(&stray, &run_two, Some(&root_one), "key-foreign"),
        )?;
        ensure(foreign == AdmissionRefusal::WrongRun, || {
            format!("a foreign parent was refused as {foreign:?}")
        })?;
        ensure(g(graph.node(&stray, dl()))?.is_none(), || {
            "a refused child appeared in the graph".to_owned()
        })
    })
}

/// Case: three nodes with three harnesses at depths 0, 1, 2 keep their
/// lineage: ancestors parent-first, descendants by depth.
fn heterogeneous_lineage_is_preserved(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "heterogeneous_lineage_is_preserved";
    const RULE: &str = "E2 design: identities stay distinct across harnesses; ancestors are parent-first, root last";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, middle, leaf) = (inv("1"), inv("2"), inv("3"));
        let root_node = admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        let mut second = request(&middle, &run_id, Some(&root), "key-middle");
        second.harness = HarnessId::new(HARNESS_B).expect("suite harness id is valid");
        second.role = role(ROLE_BUILDER);
        let middle_node = admit_ok(graph.as_ref(), second)?;
        let mut third = request(&leaf, &run_id, Some(&middle), "key-leaf");
        third.harness = HarnessId::new(HARNESS_C).expect("suite harness id is valid");
        third.role = role(ROLE_CRITIC);
        let leaf_node = admit_ok(graph.as_ref(), third)?;
        let harnesses = [&root_node.harness, &middle_node.harness, &leaf_node.harness];
        ensure(
            harnesses[0] != harnesses[1]
                && harnesses[1] != harnesses[2]
                && harnesses[0] != harnesses[2],
            || "three nodes did not keep three distinct harness ids".to_owned(),
        )?;
        ensure(
            [root_node.depth, middle_node.depth, leaf_node.depth] == [0, 1, 2],
            || "depths are not 0, 1, 2".to_owned(),
        )?;
        ensure(
            ids(&g(graph.ancestors(&leaf, dl()))?) == vec![middle.clone(), root.clone()],
            || "ancestors of the leaf are not parent-first, root last".to_owned(),
        )?;
        ensure(g(graph.ancestors(&root, dl()))?.is_empty(), || {
            "the root has ancestors".to_owned()
        })?;
        ensure(
            ids(&g(graph.descendants(&root, dl()))?) == vec![middle.clone(), leaf.clone()],
            || "descendants of the root are not in depth order".to_owned(),
        )?;
        ensure(
            g(graph.descendants(&root, dl()))?
                .iter()
                .all(|n| n.root == root),
            || "a descendant does not name the root as its lineage root".to_owned(),
        )
    })
}

/// Case: native sessions bind with epochs 1, then 2.
fn attach_native_session_epochs_increment(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "attach_native_session_epochs_increment";
    const RULE: &str = "E2 design: attach_native_session bumps the epoch 1, 2, 3, ... per node";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, child) = (inv("1"), inv("2"));
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        admit_ok(
            graph.as_ref(),
            request(&child, &run_id, Some(&root), "key-child"),
        )?;
        let first = g(graph.attach_native_session(&child, session(SESSION_ONE), dl()))?;
        ensure(first.epoch == 1, || {
            format!("first epoch is {}, want 1", first.epoch)
        })?;
        let second = g(graph.attach_native_session(&child, session(SESSION_TWO), dl()))?;
        ensure(second.epoch == 2, || {
            format!("second epoch is {}, want 2", second.epoch)
        })?;
        let stored = g(graph.node(&child, dl()))?;
        ensure(
            stored.and_then(|n| n.native_session)
                == Some(NativeSessionBinding {
                    session: session(SESSION_TWO),
                    epoch: 2,
                }),
            || "the node does not carry the latest session and epoch".to_owned(),
        )
    })
}

/// Case: attaching a native session to a terminal node is refused and the
/// node is unchanged.
fn attach_native_session_refused_on_terminal(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "attach_native_session_refused_on_terminal";
    const RULE: &str = "E2 design: a native session cannot bind to a terminal node";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, child) = (inv("1"), inv("2"));
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        admit_ok(
            graph.as_ref(),
            request(&child, &run_id, Some(&root), "key-child"),
        )?;
        g(graph.time_out(&child, EpochSeconds::new(COMPLETE_AT), dl()))?;
        let before = snapshot(graph.as_ref(), &[&child])?;
        ensure(
            is_illegal(graph.attach_native_session(&child, session(SESSION_ONE), dl())),
            || "attaching a session to a timed-out node was not an illegal transition".to_owned(),
        )?;
        ensure_unchanged(graph.as_ref(), &[&child], &before, "terminal attach")
    })
}

/// Case: a matching binding completes a running node and stamps its time.
fn complete_with_matching_binding_succeeds(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "complete_with_matching_binding_succeeds";
    const RULE: &str = "E2 design: complete with the node's own contract path and context digest transitions to Completed";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, child) = (inv("1"), inv("2"));
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        admit_ok(
            graph.as_ref(),
            request(&child, &run_id, Some(&root), "key-child"),
        )?;
        g(graph.mark_running(&child, dl()))?;
        let node = g(graph.complete(
            &child,
            binding(CONTRACT_A, DIGEST_A),
            EpochSeconds::new(COMPLETE_AT),
            dl(),
        ))?;
        ensure(node.status == NodeStatus::Completed, || {
            format!("status after a matching completion is {}", node.status)
        })?;
        ensure(
            node.terminal_at == Some(EpochSeconds::new(COMPLETE_AT)),
            || "terminal_at does not carry the completion instant".to_owned(),
        )?;
        let expected = Some(Digest::parse(DIGEST_B).expect("suite digest is valid"));
        ensure(node.result_digest == expected, || {
            format!(
                "the returned completion carries result digest {:?}, want the binding's",
                node.result_digest
            )
        })?;
        let reread = g(graph.node(&child, dl()))?;
        ensure(
            reread.as_ref().map(|n| &n.result_digest) == Some(&expected),
            || {
                format!(
                    "the re-read completed node carries {reread:?}, want the binding's result digest"
                )
            },
        )?;
        let pending = inv("3");
        admit_ok(
            graph.as_ref(),
            request(&pending, &run_id, Some(&root), "key-pending"),
        )?;
        g(graph.mark_running(&pending, dl()))?;
        for id in [&root, &pending] {
            let other = g(graph.node(id, dl()))?;
            ensure(
                other.as_ref().is_some_and(|n| n.result_digest.is_none()),
                || format!("non-completed node {id} carries a result digest: {other:?}"),
            )?;
        }
        Ok(())
    })
}

/// Case: a binding with the wrong contract path is ContractMismatch and the
/// node is unchanged.
fn complete_with_wrong_contract_path_mismatches(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "complete_with_wrong_contract_path_mismatches";
    const RULE: &str = "E2 design: a completion naming another contract path is ContractMismatch; the node is unchanged";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, child) = (inv("1"), inv("2"));
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        admit_ok(
            graph.as_ref(),
            request(&child, &run_id, Some(&root), "key-child"),
        )?;
        g(graph.mark_running(&child, dl()))?;
        let before = snapshot(graph.as_ref(), &[&child])?;
        let result = graph.complete(
            &child,
            binding(CONTRACT_B, DIGEST_A),
            EpochSeconds::new(COMPLETE_AT),
            dl(),
        );
        ensure(
            matches!(result, Err(GraphError::ContractMismatch { .. })),
            || format!("wrong contract path returned {result:?}, want ContractMismatch"),
        )?;
        ensure_unchanged(graph.as_ref(), &[&child], &before, "wrong contract path")
    })
}

/// Case: a binding with the wrong context digest is ContractMismatch and the
/// node is unchanged.
fn complete_with_wrong_context_digest_mismatches(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "complete_with_wrong_context_digest_mismatches";
    const RULE: &str = "E2 design: a completion naming another context digest is ContractMismatch; the node is unchanged";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, child) = (inv("1"), inv("2"));
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        admit_ok(
            graph.as_ref(),
            request(&child, &run_id, Some(&root), "key-child"),
        )?;
        let before = snapshot(graph.as_ref(), &[&child])?;
        let result = graph.complete(
            &child,
            binding(CONTRACT_A, DIGEST_B),
            EpochSeconds::new(COMPLETE_AT),
            dl(),
        );
        ensure(
            matches!(result, Err(GraphError::ContractMismatch { .. })),
            || format!("wrong context digest returned {result:?}, want ContractMismatch"),
        )?;
        ensure_unchanged(graph.as_ref(), &[&child], &before, "wrong context digest")
    })
}

/// Case: a sibling's binding cannot complete another child, while its own
/// binding still completes it.
fn child_binding_cannot_complete_sibling(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "child_binding_cannot_complete_sibling";
    const RULE: &str = "E2 design: a child can never complete another child's contract";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, first, second) = (inv("1"), inv("2"), inv("3"));
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        admit_ok(
            graph.as_ref(),
            request(&first, &run_id, Some(&root), "key-first"),
        )?;
        let mut other = request(&second, &run_id, Some(&root), "key-second");
        other.result_contract =
            ResultContractPath::new(CONTRACT_B).expect("suite contract is valid");
        other.context_digest = Digest::parse(DIGEST_B).expect("suite digest is valid");
        admit_ok(graph.as_ref(), other)?;
        let before = snapshot(graph.as_ref(), &[&second])?;
        let stolen = graph.complete(
            &second,
            binding(CONTRACT_A, DIGEST_A),
            EpochSeconds::new(COMPLETE_AT),
            dl(),
        );
        ensure(
            matches!(stolen, Err(GraphError::ContractMismatch { .. })),
            || format!("a sibling's binding returned {stolen:?}, want ContractMismatch"),
        )?;
        ensure_unchanged(graph.as_ref(), &[&second], &before, "sibling binding")?;
        g(graph.complete(
            &first,
            binding(CONTRACT_A, DIGEST_A),
            EpochSeconds::new(COMPLETE_AT),
            dl(),
        ))
        .map(|_| ())
        .map_err(|detail| format!("the child's own binding did not complete it: {detail}"))
    })
}

/// Case: completing a completed node is IllegalTransition.
fn complete_twice_is_illegal(factory: &dyn InvocationGraphFactory) -> Vec<ContractFinding> {
    const CASE: &str = "complete_twice_is_illegal";
    const RULE: &str =
        "E2 design: terminal states are final; a second completion is IllegalTransition";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, child) = (inv("1"), inv("2"));
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        admit_ok(
            graph.as_ref(),
            request(&child, &run_id, Some(&root), "key-child"),
        )?;
        g(graph.complete(
            &child,
            binding(CONTRACT_A, DIGEST_A),
            EpochSeconds::new(COMPLETE_AT),
            dl(),
        ))?;
        let before = snapshot(graph.as_ref(), &[&child])?;
        ensure(
            is_illegal(graph.complete(
                &child,
                binding(CONTRACT_A, DIGEST_A),
                EpochSeconds::new(CANCEL_AT),
                dl(),
            )),
            || "a second completion was not an illegal transition".to_owned(),
        )?;
        ensure_unchanged(graph.as_ref(), &[&child], &before, "second completion")
    })
}

/// Case: failed and timed-out nodes accept no further transition.
fn failed_and_timed_out_are_final(factory: &dyn InvocationGraphFactory) -> Vec<ContractFinding> {
    const CASE: &str = "failed_and_timed_out_are_final";
    const RULE: &str =
        "E2 design: Failed and TimedOut are final; every later transition is IllegalTransition";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, failed, timed) = (inv("1"), inv("2"), inv("3"));
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        admit_ok(
            graph.as_ref(),
            request(&failed, &run_id, Some(&root), "key-failed"),
        )?;
        admit_ok(
            graph.as_ref(),
            request(&timed, &run_id, Some(&root), "key-timed"),
        )?;
        let at = EpochSeconds::new(COMPLETE_AT);
        let failed_node = g(graph.fail(&failed, InvocationFailure::Execution, at, dl()))?;
        ensure(
            failed_node.status == NodeStatus::Failed(InvocationFailure::Execution),
            || format!("fail left the node {}", failed_node.status),
        )?;
        let timed_node = g(graph.time_out(&timed, at, dl()))?;
        ensure(timed_node.status == NodeStatus::TimedOut, || {
            format!("time_out left the node {}", timed_node.status)
        })?;
        let subjects = [&failed, &timed];
        let before = snapshot(graph.as_ref(), &subjects)?;
        let attempts = [
            is_illegal(graph.mark_running(&failed, dl())),
            is_illegal(graph.complete(&failed, binding(CONTRACT_A, DIGEST_A), at, dl())),
            is_illegal(graph.fail(&failed, InvocationFailure::MissingResult, at, dl())),
            is_illegal(graph.time_out(&failed, at, dl())),
            is_illegal(graph.mark_running(&timed, dl())),
            is_illegal(graph.complete(&timed, binding(CONTRACT_A, DIGEST_A), at, dl())),
            is_illegal(graph.fail(&timed, InvocationFailure::Execution, at, dl())),
            is_illegal(graph.time_out(&timed, at, dl())),
        ];
        ensure(attempts.iter().all(|&illegal| illegal), || {
            format!("a transition from a final state was accepted: {attempts:?}")
        })?;
        ensure_unchanged(graph.as_ref(), &subjects, &before, "final states")
    })
}

/// Case: cancel_subtree cancels the active part of the subtree in one step,
/// leaves completed descendants alone, returns the newly cancelled ids root
/// first, and is idempotent.
fn cancel_subtree_cancels_active_descendants_only(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "cancel_subtree_cancels_active_descendants_only";
    const RULE: &str = "E2 design: cancel_subtree marks the node and every non-terminal descendant Cancelled atomically; settled descendants are historical facts";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, first, first_leaf, second, second_leaf) =
            (inv("1"), inv("2"), inv("3"), inv("4"), inv("5"));
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        admit_ok(
            graph.as_ref(),
            request(&first, &run_id, Some(&root), "key-first"),
        )?;
        admit_ok(
            graph.as_ref(),
            request(&first_leaf, &run_id, Some(&first), "key-first-leaf"),
        )?;
        g(graph.mark_running(&first_leaf, dl()))?;
        admit_ok(
            graph.as_ref(),
            request(&second, &run_id, Some(&root), "key-second"),
        )?;
        admit_ok(
            graph.as_ref(),
            request(&second_leaf, &run_id, Some(&second), "key-second-leaf"),
        )?;
        g(graph.complete(
            &second,
            binding(CONTRACT_A, DIGEST_A),
            EpochSeconds::new(COMPLETE_AT),
            dl(),
        ))?;
        let cancelled = g(graph.cancel_subtree(&root, EpochSeconds::new(CANCEL_AT), dl()))?;
        ensure(
            cancelled
                == vec![
                    root.clone(),
                    first.clone(),
                    first_leaf.clone(),
                    second_leaf.clone(),
                ],
            || format!("cancelled ids are {cancelled:?}, want root, then the active descendants"),
        )?;
        let settled = g(graph.node(&second, dl()))?.ok_or("the completed child vanished")?;
        ensure(
            settled.status == NodeStatus::Completed
                && settled.terminal_at == Some(EpochSeconds::new(COMPLETE_AT)),
            || "a completed descendant was altered by cancel_subtree".to_owned(),
        )?;
        let leaf = g(graph.node(&second_leaf, dl()))?.ok_or("the cancelled leaf vanished")?;
        ensure(leaf.status == NodeStatus::Cancelled, || {
            format!("an active descendant is {}, not cancelled", leaf.status)
        })?;
        let again = g(graph.cancel_subtree(&root, EpochSeconds::new(CANCEL_AT), dl()))?;
        ensure(again.is_empty(), || {
            format!("a second cancel_subtree returned {again:?}, want none")
        })
    })
}

/// Case: cancel_subtree on an already-terminal root leaves the root and its
/// settled descendants untouched, cancels only the active descendants, and
/// returns exactly those ids in depth-then-admission order. The broker relies
/// on this when it cascades after failing or orphaning a node.
fn cancel_subtree_on_a_terminal_root_cancels_only_active_descendants(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "cancel_subtree_on_a_terminal_root_cancels_only_active_descendants";
    const RULE: &str = "E2 design: cancel_subtree on a terminal node leaves the node unchanged; only non-terminal descendants become Cancelled, and the returned ids are exactly the newly cancelled ones";
    const ROOT_FAILED_AT: u64 = 1_800_000_150;
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, c1, c2, c3, g1) = (inv("1"), inv("2"), inv("3"), inv("4"), inv("5"));
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        admit_ok(graph.as_ref(), request(&c1, &run_id, Some(&root), "key-c1"))?;
        admit_ok(graph.as_ref(), request(&c2, &run_id, Some(&root), "key-c2"))?;
        admit_ok(graph.as_ref(), request(&c3, &run_id, Some(&root), "key-c3"))?;
        admit_ok(graph.as_ref(), request(&g1, &run_id, Some(&c1), "key-g1"))?;
        g(graph.mark_running(&c1, dl()))?;
        g(graph.mark_running(&g1, dl()))?;
        g(graph.complete(
            &c2,
            binding(CONTRACT_A, DIGEST_A),
            EpochSeconds::new(COMPLETE_AT),
            dl(),
        ))?;
        g(graph.fail(
            &root,
            InvocationFailure::Execution,
            EpochSeconds::new(ROOT_FAILED_AT),
            dl(),
        ))?;
        let before = snapshot(graph.as_ref(), &[&root, &c2])?;
        ensure(
            before[0].as_ref().map(|n| n.status)
                == Some(NodeStatus::Failed(InvocationFailure::Execution)),
            || "the root did not settle as Failed before the cascade".to_owned(),
        )?;

        let cancelled = g(graph.cancel_subtree(&root, EpochSeconds::new(CANCEL_AT), dl()))?;
        ensure(
            cancelled == vec![c1.clone(), c3.clone(), g1.clone()],
            || format!("cancelled ids are {cancelled:?}, want [C1, C3, G] (depth, then admission)"),
        )?;
        ensure_unchanged(
            graph.as_ref(),
            &[&root, &c2],
            &before,
            "terminal root and completed child",
        )?;
        for id in [&c1, &c3, &g1] {
            let node = g(graph.node(id, dl()))?.ok_or("an active descendant vanished")?;
            ensure(
                node.status == NodeStatus::Cancelled
                    && node.terminal_at == Some(EpochSeconds::new(CANCEL_AT)),
                || {
                    format!(
                        "active descendant {id} is {} after the cascade",
                        node.status
                    )
                },
            )?;
        }
        let again = g(graph.cancel_subtree(&root, EpochSeconds::new(CANCEL_AT), dl()))?;
        ensure(again.is_empty(), || {
            format!("a second cancel_subtree on the terminal root returned {again:?}, want none")
        })?;
        ensure_unchanged(
            graph.as_ref(),
            &[&root, &c2],
            &before,
            "second cascade on the terminal root",
        )
    })
}

/// Case: cancel_subtree on a terminal leaf is a no-op: it returns no ids and
/// leaves the leaf and its parent exactly as they were.
fn cancel_subtree_on_a_terminal_leaf_is_a_noop_returning_empty(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "cancel_subtree_on_a_terminal_leaf_is_a_noop_returning_empty";
    const RULE: &str =
        "E2 design: cancel_subtree on a terminal leaf returns no ids and changes nothing";
    const LEAF_FAILED_AT: u64 = 1_800_000_120;
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, leaf) = (inv("1"), inv("2"));
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        admit_ok(
            graph.as_ref(),
            request(&leaf, &run_id, Some(&root), "key-leaf"),
        )?;
        g(graph.mark_running(&leaf, dl()))?;
        g(graph.fail(
            &leaf,
            InvocationFailure::Execution,
            EpochSeconds::new(LEAF_FAILED_AT),
            dl(),
        ))?;
        let before = snapshot(graph.as_ref(), &[&root, &leaf])?;
        let cancelled = g(graph.cancel_subtree(&leaf, EpochSeconds::new(CANCEL_AT), dl()))?;
        ensure(cancelled.is_empty(), || {
            format!("cancel_subtree on a terminal leaf returned {cancelled:?}, want none")
        })?;
        ensure_unchanged(
            graph.as_ref(),
            &[&root, &leaf],
            &before,
            "terminal leaf cascade",
        )?;
        let node = g(graph.node(&leaf, dl()))?.ok_or("the failed leaf vanished")?;
        ensure(
            node.status == NodeStatus::Failed(InvocationFailure::Execution)
                && node.terminal_at == Some(EpochSeconds::new(LEAF_FAILED_AT)),
            || "the failed leaf was altered by cancel_subtree".to_owned(),
        )
    })
}

/// Case: a child that claims nothing and presents no binding stays
/// non-terminal; the graph never completes it implicitly.
fn child_without_binding_stays_non_terminal(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "child_without_binding_stays_non_terminal";
    const RULE: &str =
        "E2 design: no implicit completion; without a binding an admitted child stays non-terminal";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, child) = (inv("1"), inv("2"));
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        admit_ok(
            graph.as_ref(),
            request(&child, &run_id, Some(&root), "key-child"),
        )?;
        let node = g(graph.node(&child, dl()))?.ok_or("the admitted child is missing")?;
        ensure(
            node.status == NodeStatus::Admitted && node.terminal_at.is_none(),
            || {
                format!(
                    "an unbound child is {} with terminal_at {:?}",
                    node.status, node.terminal_at
                )
            },
        )?;
        ensure(
            ids(&g(graph.active_nodes(&run_id, dl()))?).contains(&child),
            || "an unbound child is missing from the active nodes".to_owned(),
        )
    })
}

/// Case: active_nodes excludes terminal nodes, keeps admission order, and
/// is scoped to its run.
fn active_nodes_excludes_terminal(factory: &dyn InvocationGraphFactory) -> Vec<ContractFinding> {
    const CASE: &str = "active_nodes_excludes_terminal";
    const RULE: &str =
        "E2 design: active nodes of a run are the non-terminal ones, in admission order";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let (run_one, run_two) = (run("1"), run("2"));
        let (root, first, second, other_root) = (inv("1"), inv("2"), inv("3"), inv("4"));
        admit_ok(graph.as_ref(), root_request(&root, &run_one, "key-root"))?;
        admit_ok(
            graph.as_ref(),
            request(&first, &run_one, Some(&root), "key-first"),
        )?;
        admit_ok(
            graph.as_ref(),
            request(&second, &run_one, Some(&root), "key-second"),
        )?;
        admit_ok(
            graph.as_ref(),
            root_request(&other_root, &run_two, "key-other"),
        )?;
        g(graph.fail(
            &first,
            InvocationFailure::Execution,
            EpochSeconds::new(COMPLETE_AT),
            dl(),
        ))?;
        let active = ids(&g(graph.active_nodes(&run_one, dl()))?);
        ensure(active == vec![root.clone(), second.clone()], || {
            format!("active nodes of the run are {active:?}, want root and the second child")
        })?;
        let other = ids(&g(graph.active_nodes(&run_two, dl()))?);
        ensure(other == vec![other_root.clone()], || {
            format!("active nodes of the other run are {other:?}")
        })
    })
}

/// Case: descendants come back by depth, then admission order, and the same
/// order on every call.
fn descendants_order_is_deterministic(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "descendants_order_is_deterministic";
    const RULE: &str = "E2 design: descendants are ordered by depth, then admission order, identically across calls";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, first, second, first_leaf, second_leaf) =
            (inv("1"), inv("2"), inv("3"), inv("4"), inv("5"));
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        admit_ok(
            graph.as_ref(),
            request(&first, &run_id, Some(&root), "key-first"),
        )?;
        admit_ok(
            graph.as_ref(),
            request(&second, &run_id, Some(&root), "key-second"),
        )?;
        admit_ok(
            graph.as_ref(),
            request(&first_leaf, &run_id, Some(&first), "key-first-leaf"),
        )?;
        admit_ok(
            graph.as_ref(),
            request(&second_leaf, &run_id, Some(&second), "key-second-leaf"),
        )?;
        let want = vec![
            first.clone(),
            second.clone(),
            first_leaf.clone(),
            second_leaf.clone(),
        ];
        let once = ids(&g(graph.descendants(&root, dl()))?);
        ensure(once == want, || {
            format!("descendants are {once:?}, want depth-then-admission order")
        })?;
        let twice = ids(&g(graph.descendants(&root, dl()))?);
        ensure(twice == once, || {
            "descendants changed between two identical calls".to_owned()
        })
    })
}

/// Case: queries about an id the graph never admitted report it as
/// UnknownInvocation; node() reports absence as None.
fn unknown_invocations_are_reported(factory: &dyn InvocationGraphFactory) -> Vec<ContractFinding> {
    const CASE: &str = "unknown_invocations_are_reported";
    const RULE: &str = "E2 design: structural queries on an unknown invocation are UnknownInvocation; node() is None";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let stranger = inv("9");
        ensure(g(graph.node(&stranger, dl()))?.is_none(), || {
            "node() of an unknown id was not None".to_owned()
        })?;
        ensure(
            matches!(
                graph.children(&stranger, dl()),
                Err(GraphError::UnknownInvocation { .. })
            ),
            || "children() of an unknown id was not UnknownInvocation".to_owned(),
        )?;
        ensure(
            matches!(
                graph.cancel_subtree(&stranger, EpochSeconds::new(CANCEL_AT), dl()),
                Err(GraphError::UnknownInvocation { .. })
            ),
            || "cancel_subtree() of an unknown id was not UnknownInvocation".to_owned(),
        )
    })
}

// E3a: lifecycle observation sequencing. Every case re-reads the view or the
// history after each refused, ignored, or terminal observation to prove it had
// no effect.

const LIFECYCLE_OBSERVED_AT: u64 = 1_800_000_050;
const LIFECYCLE_RECEIVED_AT: u64 = 1_800_000_051;
const LIFECYCLE_LATE_RECEIVED_AT: u64 = 1_800_000_090;
const LIFECYCLE_FINAL_RECEIVED_AT: u64 = 1_800_000_095;
const LIFECYCLE_LAST_RECEIVED_AT: u64 = 1_800_000_096;
const SEQ_FIRST: u64 = 1;
const SEQ_SECOND: u64 = 2;
const SEQ_THIRD: u64 = 3;
const SEQ_EARLY: u64 = 42;
const SEQ_LATE: u64 = 43;
const SEQ_HIGHEST: u64 = 100;
const SEQ_FAR_FUTURE: u64 = 999;
const EPOCH_NONE: u32 = 0;
const EPOCH_FIRST: u32 = 1;
const EPOCH_SECOND: u32 = 2;
const HISTORY_LEN_LATE: usize = 2;
const HISTORY_LEN_REPLAY: usize = 2;
const HISTORY_LEN_WRONG_BINDING: usize = 3;
const HISTORY_LEN_PRE_SESSION: usize = 3;
const HISTORY_LEN_TERMINAL: usize = 3;
const HISTORY_LEN_RECEIPT: usize = 4;
const TWO_SOURCES: usize = 2;

/// Compares two values; the detail names what was wanted and what was seen.
fn want<T: PartialEq + Debug>(actual: T, expected: T, what: &str) -> Result<(), String> {
    if actual == expected {
        Ok(())
    } else {
        Err(format!("{what}: got {actual:?}, want {expected:?}"))
    }
}

fn bound(name: &str, epoch: u32) -> NativeSessionBinding {
    NativeSessionBinding {
        session: session(name),
        epoch,
    }
}

fn observation(
    id: &InvocationId,
    native: Option<NativeSessionBinding>,
    state: LifecycleState,
    source: ObservationSource,
    sequence: ObservationSequence,
) -> HarnessObservation {
    HarnessObservation {
        invocation_id: id.clone(),
        native_session: native,
        state,
        source,
        observed_at: EpochSeconds::new(LIFECYCLE_OBSERVED_AT),
        sequence,
    }
}

fn observe(
    graph: &dyn InvocationGraph,
    observation: HarnessObservation,
    received_at: u64,
) -> Result<ObservationOutcome, String> {
    g(graph.record_observation(observation, EpochSeconds::new(received_at), dl()))
}

fn view_of(graph: &dyn InvocationGraph, id: &InvocationId) -> Result<LifecycleView, String> {
    g(graph.lifecycle(id, dl()))
}

fn history_of(
    graph: &dyn InvocationGraph,
    id: &InvocationId,
) -> Result<Vec<ObservationRecord>, String> {
    g(graph.observation_history(id, dl()))
}

fn effective_state(view: &LifecycleView) -> Option<LifecycleState> {
    view.effective.as_ref().map(|entry| entry.state)
}

fn attach(
    graph: &dyn InvocationGraph,
    id: &InvocationId,
    name: &str,
) -> Result<NativeSessionBinding, String> {
    g(graph.attach_native_session(id, session(name), dl()))
}

/// Admits a root and one child of it under the run.
fn admit_pair(
    graph: &dyn InvocationGraph,
    run_id: &RunId,
    root: &InvocationId,
    child: &InvocationId,
) -> Result<(), String> {
    admit_ok(graph, root_request(root, run_id, "key-root"))?;
    admit_ok(graph, request(child, run_id, Some(root), "key-child"))?;
    Ok(())
}

/// Case: a late, lower sequence from the same source is IgnoredStale and the
/// view stays on the higher sequence (seq 43 idle, then late seq 42 working).
fn lifecycle_late_lower_sequence_is_ignored_stale(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "lifecycle_late_lower_sequence_is_ignored_stale";
    const RULE: &str = "E3a: a source-provided sequence not above the last applied one of its key is IgnoredStale, and the view never rolls back";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let (run_id, root, child) = (run("1"), inv("1"), inv("2"));
        admit_pair(graph.as_ref(), &run_id, &root, &child)?;
        let binding = attach(graph.as_ref(), &child, SESSION_ONE)?;
        let idle = observation(
            &child,
            Some(binding.clone()),
            LifecycleState::Idle,
            ObservationSource::Structured,
            ObservationSequence::SourceProvided(SEQ_LATE),
        );
        want(
            observe(graph.as_ref(), idle, LIFECYCLE_RECEIVED_AT)?,
            ObservationOutcome::Applied {
                sequence: SEQ_LATE,
                previous: None,
            },
            "seq 43 idle",
        )?;
        let before = view_of(graph.as_ref(), &child)?;
        let late = observation(
            &child,
            Some(binding),
            LifecycleState::Working,
            ObservationSource::Structured,
            ObservationSequence::SourceProvided(SEQ_EARLY),
        );
        want(
            observe(graph.as_ref(), late, LIFECYCLE_LATE_RECEIVED_AT)?,
            ObservationOutcome::IgnoredStale {
                last_applied: SEQ_LATE,
            },
            "late seq 42 working",
        )?;
        let after = view_of(graph.as_ref(), &child)?;
        want(
            after.clone(),
            before,
            "the late observation changed the view",
        )?;
        want(
            effective_state(&after),
            Some(LifecycleState::Idle),
            "effective state after the late observation",
        )?;
        let history = history_of(graph.as_ref(), &child)?;
        want(history.len(), HISTORY_LEN_LATE, "history length")?;
        want(
            history.last().map(|record| record.outcome.clone()),
            Some(ObservationOutcome::IgnoredStale {
                last_applied: SEQ_LATE,
            }),
            "outcome of the late observation in history",
        )
    })
}

/// Case: replaying an applied sequence number is ignored and changes nothing.
fn lifecycle_equal_sequence_replay_is_ignored(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "lifecycle_equal_sequence_replay_is_ignored";
    const RULE: &str =
        "E3a: a source-provided sequence equal to the last applied one is IgnoredStale";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let (run_id, root, child) = (run("1"), inv("1"), inv("2"));
        admit_pair(graph.as_ref(), &run_id, &root, &child)?;
        let binding = attach(graph.as_ref(), &child, SESSION_ONE)?;
        let first = observation(
            &child,
            Some(binding.clone()),
            LifecycleState::Idle,
            ObservationSource::Structured,
            ObservationSequence::SourceProvided(SEQ_LATE),
        );
        want(
            observe(graph.as_ref(), first, LIFECYCLE_RECEIVED_AT)?,
            ObservationOutcome::Applied {
                sequence: SEQ_LATE,
                previous: None,
            },
            "first seq 43",
        )?;
        let before = view_of(graph.as_ref(), &child)?;
        let replay = observation(
            &child,
            Some(binding),
            LifecycleState::Working,
            ObservationSource::Structured,
            ObservationSequence::SourceProvided(SEQ_LATE),
        );
        want(
            observe(graph.as_ref(), replay, LIFECYCLE_LATE_RECEIVED_AT)?,
            ObservationOutcome::IgnoredStale {
                last_applied: SEQ_LATE,
            },
            "replayed seq 43",
        )?;
        want(
            view_of(graph.as_ref(), &child)?,
            before,
            "the replayed observation changed the view",
        )?;
        want(
            history_of(graph.as_ref(), &child)?.len(),
            HISTORY_LEN_REPLAY,
            "replay is recorded in history",
        )
    })
}

/// Case: a higher sequence applies, and reports the state it replaced.
fn lifecycle_higher_sequence_applies(factory: &dyn InvocationGraphFactory) -> Vec<ContractFinding> {
    const CASE: &str = "lifecycle_higher_sequence_applies";
    const RULE: &str = "E3a: a source-provided sequence above the last applied one applies and becomes the entry of its key";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let (run_id, root, child) = (run("1"), inv("1"), inv("2"));
        admit_pair(graph.as_ref(), &run_id, &root, &child)?;
        let binding = attach(graph.as_ref(), &child, SESSION_ONE)?;
        let idle = observation(
            &child,
            Some(binding.clone()),
            LifecycleState::Idle,
            ObservationSource::Structured,
            ObservationSequence::SourceProvided(SEQ_LATE),
        );
        observe(graph.as_ref(), idle, LIFECYCLE_RECEIVED_AT)?;
        let working = observation(
            &child,
            Some(binding),
            LifecycleState::Working,
            ObservationSource::Structured,
            ObservationSequence::SourceProvided(SEQ_HIGHEST),
        );
        want(
            observe(graph.as_ref(), working, LIFECYCLE_LATE_RECEIVED_AT)?,
            ObservationOutcome::Applied {
                sequence: SEQ_HIGHEST,
                previous: Some(LifecycleState::Idle),
            },
            "seq 100 working",
        )?;
        let view = view_of(graph.as_ref(), &child)?;
        want(
            effective_state(&view),
            Some(LifecycleState::Working),
            "effective state after seq 100",
        )?;
        want(
            view.effective.map(|entry| entry.sequence),
            Some(SEQ_HIGHEST),
            "effective sequence",
        )?;
        want(view.per_source.len(), 1, "one entry for the one source")
    })
}

/// Case: sources sequence independently, and the effective entry is the
/// highest-trust source (Screen working + Structured idle is Idle).
fn lifecycle_sources_are_independent_and_trust_ranked(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "lifecycle_sources_are_independent_and_trust_ranked";
    const RULE: &str = "E3a: each source sequences independently; the effective entry is the highest-trust source with an applied observation";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let (run_id, root, child) = (run("1"), inv("1"), inv("2"));
        admit_pair(graph.as_ref(), &run_id, &root, &child)?;
        let binding = attach(graph.as_ref(), &child, SESSION_ONE)?;
        let screen = observation(
            &child,
            Some(binding.clone()),
            LifecycleState::Working,
            ObservationSource::Screen,
            ObservationSequence::SourceProvided(SEQ_HIGHEST),
        );
        want(
            observe(graph.as_ref(), screen, LIFECYCLE_RECEIVED_AT)?,
            ObservationOutcome::Applied {
                sequence: SEQ_HIGHEST,
                previous: None,
            },
            "screen seq 100",
        )?;
        let structured = observation(
            &child,
            Some(binding),
            LifecycleState::Idle,
            ObservationSource::Structured,
            ObservationSequence::SourceProvided(SEQ_FIRST),
        );
        want(
            observe(graph.as_ref(), structured, LIFECYCLE_LATE_RECEIVED_AT)?,
            ObservationOutcome::Applied {
                sequence: SEQ_FIRST,
                previous: None,
            },
            "structured seq 1 is not stale against the screen sequence",
        )?;
        let view = view_of(graph.as_ref(), &child)?;
        want(
            view.effective.as_ref().map(|entry| entry.source),
            Some(ObservationSource::Structured),
            "effective source",
        )?;
        want(
            effective_state(&view),
            Some(LifecycleState::Idle),
            "structured idle outranks screen working",
        )?;
        want(
            view.per_source
                .iter()
                .map(|entry| entry.source)
                .collect::<Vec<_>>(),
            vec![ObservationSource::Structured, ObservationSource::Screen],
            "per-source entries, highest trust first",
        )
    })
}

/// Case: with only a process observation the effective entry is that process
/// entry, and a later screen observation does not displace it.
fn lifecycle_process_only_entry_is_effective(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "lifecycle_process_only_entry_is_effective";
    const RULE: &str = "E3a: a process entry is effective when no structured entry exists, and a lower-trust screen entry does not displace it";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let (run_id, root, child) = (run("1"), inv("1"), inv("2"));
        admit_pair(graph.as_ref(), &run_id, &root, &child)?;
        let binding = attach(graph.as_ref(), &child, SESSION_ONE)?;
        let process = observation(
            &child,
            Some(binding.clone()),
            LifecycleState::Blocked,
            ObservationSource::Process,
            ObservationSequence::SourceProvided(SEQ_FIRST),
        );
        observe(graph.as_ref(), process, LIFECYCLE_RECEIVED_AT)?;
        let view = view_of(graph.as_ref(), &child)?;
        want(
            view.effective.as_ref().map(|entry| entry.source),
            Some(ObservationSource::Process),
            "process-only effective source",
        )?;
        want(
            effective_state(&view),
            Some(LifecycleState::Blocked),
            "process-only effective state",
        )?;
        let screen = observation(
            &child,
            Some(binding),
            LifecycleState::Working,
            ObservationSource::Screen,
            ObservationSequence::SourceProvided(SEQ_FIRST),
        );
        observe(graph.as_ref(), screen, LIFECYCLE_LATE_RECEIVED_AT)?;
        let after = view_of(graph.as_ref(), &child)?;
        want(
            effective_state(&after),
            Some(LifecycleState::Blocked),
            "a screen entry displaced the process entry",
        )?;
        want(
            after.per_source.len(),
            TWO_SOURCES,
            "screen and process each keep one entry",
        )
    })
}

/// Case: Gauntlet-assigned sequences are 1, 2, 3 per (invocation, epoch,
/// source), independent across sources, and recorded with their origin.
fn lifecycle_gauntlet_sequences_count_per_key(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "lifecycle_gauntlet_sequences_count_per_key";
    const RULE: &str = "E3a: a GauntletAssigned sequence is last applied + 1 per (invocation, epoch, source), always applied";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let (run_id, root, child) = (run("1"), inv("1"), inv("2"));
        admit_pair(graph.as_ref(), &run_id, &root, &child)?;
        let binding = attach(graph.as_ref(), &child, SESSION_ONE)?;
        let gauntlet = |state: LifecycleState, source: ObservationSource| {
            observation(
                &child,
                Some(binding.clone()),
                state,
                source,
                ObservationSequence::GauntletAssigned,
            )
        };
        want(
            observe(
                graph.as_ref(),
                gauntlet(LifecycleState::Working, ObservationSource::Screen),
                LIFECYCLE_RECEIVED_AT,
            )?,
            ObservationOutcome::Applied {
                sequence: SEQ_FIRST,
                previous: None,
            },
            "screen assigned 1",
        )?;
        want(
            observe(
                graph.as_ref(),
                gauntlet(LifecycleState::Blocked, ObservationSource::Screen),
                LIFECYCLE_LATE_RECEIVED_AT,
            )?,
            ObservationOutcome::Applied {
                sequence: SEQ_SECOND,
                previous: Some(LifecycleState::Working),
            },
            "screen assigned 2",
        )?;
        want(
            observe(
                graph.as_ref(),
                gauntlet(LifecycleState::Idle, ObservationSource::Screen),
                LIFECYCLE_FINAL_RECEIVED_AT,
            )?,
            ObservationOutcome::Applied {
                sequence: SEQ_THIRD,
                previous: Some(LifecycleState::Blocked),
            },
            "screen assigned 3",
        )?;
        want(
            observe(
                graph.as_ref(),
                gauntlet(LifecycleState::Done, ObservationSource::Structured),
                LIFECYCLE_LAST_RECEIVED_AT,
            )?,
            ObservationOutcome::Applied {
                sequence: SEQ_FIRST,
                previous: None,
            },
            "structured assigned 1 (independent of screen)",
        )?;
        let view = view_of(graph.as_ref(), &child)?;
        let structured = view
            .effective
            .ok_or_else(|| "no effective entry after four applied observations".to_owned())?;
        want(structured.sequence, SEQ_FIRST, "structured entry sequence")?;
        want(
            structured.origin,
            SequenceOrigin::GauntletAssigned,
            "structured entry origin",
        )?;
        let screen = view
            .per_source
            .iter()
            .find(|entry| entry.source == ObservationSource::Screen)
            .ok_or_else(|| "no screen entry in the view".to_owned())?;
        want(screen.sequence, SEQ_THIRD, "screen entry sequence")?;
        let history = history_of(graph.as_ref(), &child)?;
        let third = history
            .get(2)
            .ok_or_else(|| "history lacks the third observation".to_owned())?;
        want(
            third.observation.sequence,
            ObservationSequence::SourceProvided(SEQ_THIRD),
            "history records the sequence actually used",
        )?;
        want(
            third.origin,
            SequenceOrigin::GauntletAssigned,
            "history records who assigned the sequence",
        )
    })
}

/// Case: a new epoch starts its sequences again; the superseded epoch's
/// entries remain history and leave the current view.
fn lifecycle_gauntlet_sequences_restart_per_epoch(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "lifecycle_gauntlet_sequences_restart_per_epoch";
    const RULE: &str =
        "E3a: sequences restart at each epoch; a superseded epoch's entries are history only";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let (run_id, root, child) = (run("1"), inv("1"), inv("2"));
        admit_pair(graph.as_ref(), &run_id, &root, &child)?;
        let first = attach(graph.as_ref(), &child, SESSION_ONE)?;
        let gauntlet = |binding: &NativeSessionBinding, state: LifecycleState| {
            observation(
                &child,
                Some(binding.clone()),
                state,
                ObservationSource::Screen,
                ObservationSequence::GauntletAssigned,
            )
        };
        observe(
            graph.as_ref(),
            gauntlet(&first, LifecycleState::Working),
            LIFECYCLE_RECEIVED_AT,
        )?;
        want(
            observe(
                graph.as_ref(),
                gauntlet(&first, LifecycleState::Idle),
                LIFECYCLE_LATE_RECEIVED_AT,
            )?,
            ObservationOutcome::Applied {
                sequence: SEQ_SECOND,
                previous: Some(LifecycleState::Working),
            },
            "epoch 1 second assignment",
        )?;
        let second = attach(graph.as_ref(), &child, SESSION_TWO)?;
        want(second.epoch, EPOCH_SECOND, "second attach epoch")?;
        want(
            observe(
                graph.as_ref(),
                gauntlet(&second, LifecycleState::Working),
                LIFECYCLE_FINAL_RECEIVED_AT,
            )?,
            ObservationOutcome::Applied {
                sequence: SEQ_FIRST,
                previous: None,
            },
            "epoch 2 restarts at 1",
        )?;
        let view = view_of(graph.as_ref(), &child)?;
        want(
            view.per_source.len(),
            1,
            "only the current epoch's entries are in the view",
        )?;
        want(
            view.effective.map(|entry| (entry.epoch, entry.sequence)),
            Some((EPOCH_SECOND, SEQ_FIRST)),
            "effective entry is the epoch 2 entry",
        )
    })
}

/// Case: an observation naming a superseded binding is rejected and leaves the
/// view unchanged; the new binding's sequence restarts at 1 and applies.
fn lifecycle_stale_session_observation_is_rejected(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "lifecycle_stale_session_observation_is_rejected";
    const RULE: &str = "E3a: an observation naming a superseded session binding is RejectedStaleSession; the new session's sequence restarts";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let (run_id, root, child) = (run("1"), inv("1"), inv("2"));
        admit_pair(graph.as_ref(), &run_id, &root, &child)?;
        let s1 = attach(graph.as_ref(), &child, SESSION_ONE)?;
        want(
            observe(
                graph.as_ref(),
                observation(
                    &child,
                    Some(s1.clone()),
                    LifecycleState::Idle,
                    ObservationSource::Structured,
                    ObservationSequence::SourceProvided(SEQ_HIGHEST),
                ),
                LIFECYCLE_RECEIVED_AT,
            )?,
            ObservationOutcome::Applied {
                sequence: SEQ_HIGHEST,
                previous: None,
            },
            "S1 seq 100 under epoch 1",
        )?;
        let s2 = attach(graph.as_ref(), &child, SESSION_TWO)?;
        want(s2.epoch, EPOCH_SECOND, "replacement epoch")?;
        let before = view_of(graph.as_ref(), &child)?;
        want(
            effective_state(&before),
            None,
            "a fresh epoch has no effective entry",
        )?;
        want(
            observe(
                graph.as_ref(),
                observation(
                    &child,
                    Some(s1),
                    LifecycleState::Working,
                    ObservationSource::Structured,
                    ObservationSequence::SourceProvided(SEQ_FAR_FUTURE),
                ),
                LIFECYCLE_LATE_RECEIVED_AT,
            )?,
            ObservationOutcome::RejectedStaleSession {
                current: Some(s2.clone()),
            },
            "S1 seq 999 after replacement",
        )?;
        want(
            view_of(graph.as_ref(), &child)?,
            before,
            "the rejected S1 observation changed the view",
        )?;
        want(
            observe(
                graph.as_ref(),
                observation(
                    &child,
                    Some(s2),
                    LifecycleState::Working,
                    ObservationSource::Structured,
                    ObservationSequence::SourceProvided(SEQ_FIRST),
                ),
                LIFECYCLE_FINAL_RECEIVED_AT,
            )?,
            ObservationOutcome::Applied {
                sequence: SEQ_FIRST,
                previous: None,
            },
            "S2 seq 1 under epoch 2",
        )?;
        let view = view_of(graph.as_ref(), &child)?;
        want(
            effective_state(&view),
            Some(LifecycleState::Working),
            "S2 working is effective",
        )?;
        want(
            history_of(graph.as_ref(), &child)?
                .iter()
                .filter(|record| {
                    matches!(
                        record.outcome,
                        ObservationOutcome::RejectedStaleSession { .. }
                    )
                })
                .count(),
            1,
            "one rejection in history",
        )
    })
}

/// Case: a binding that differs in epoch, or in session ref, or an absent
/// binding while a session is attached, is RejectedStaleSession.
fn lifecycle_wrong_binding_is_rejected(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "lifecycle_wrong_binding_is_rejected";
    const RULE: &str =
        "E3a: an observation must name the node's current binding exactly (session ref and epoch)";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let (run_id, root, child) = (run("1"), inv("1"), inv("2"));
        admit_pair(graph.as_ref(), &run_id, &root, &child)?;
        attach(graph.as_ref(), &child, SESSION_ONE)?;
        let current = attach(graph.as_ref(), &child, SESSION_TWO)?;
        let before = view_of(graph.as_ref(), &child)?;
        let named = [
            Some(bound(SESSION_TWO, EPOCH_FIRST)),
            Some(bound(SESSION_ONE, EPOCH_SECOND)),
            None,
        ];
        for (index, native) in named.into_iter().enumerate() {
            let outcome = observe(
                graph.as_ref(),
                observation(
                    &child,
                    native,
                    LifecycleState::Working,
                    ObservationSource::Structured,
                    ObservationSequence::SourceProvided(SEQ_FIRST),
                ),
                LIFECYCLE_RECEIVED_AT,
            )?;
            want(
                outcome,
                ObservationOutcome::RejectedStaleSession {
                    current: Some(current.clone()),
                },
                &format!("wrong binding #{index}"),
            )?;
        }
        want(
            view_of(graph.as_ref(), &child)?,
            before,
            "a wrong-binding observation changed the view",
        )?;
        want(
            history_of(graph.as_ref(), &child)?.len(),
            HISTORY_LEN_WRONG_BINDING,
            "rejections are recorded in history",
        )
    })
}

/// Case: before any session an observation with no binding applies at epoch 0;
/// one naming a binding is rejected; once a session is attached the epoch-0
/// entries leave the view and a binding-less observation is rejected.
fn lifecycle_pre_session_observation_needs_no_binding(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "lifecycle_pre_session_observation_needs_no_binding";
    const RULE: &str = "E3a: with no session attached only observations naming no binding apply (epoch 0); once attached, binding-less observations are rejected";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let (run_id, root, child) = (run("1"), inv("1"), inv("2"));
        admit_pair(graph.as_ref(), &run_id, &root, &child)?;
        want(
            observe(
                graph.as_ref(),
                observation(
                    &child,
                    None,
                    LifecycleState::Working,
                    ObservationSource::Screen,
                    ObservationSequence::SourceProvided(SEQ_FIRST),
                ),
                LIFECYCLE_RECEIVED_AT,
            )?,
            ObservationOutcome::Applied {
                sequence: SEQ_FIRST,
                previous: None,
            },
            "pre-session observation without a binding",
        )?;
        let pre = view_of(graph.as_ref(), &child)?;
        want(
            pre.effective.map(|entry| entry.epoch),
            Some(EPOCH_NONE),
            "pre-session entries are epoch 0",
        )?;
        want(
            observe(
                graph.as_ref(),
                observation(
                    &child,
                    Some(bound(SESSION_ONE, EPOCH_FIRST)),
                    LifecycleState::Working,
                    ObservationSource::Structured,
                    ObservationSequence::SourceProvided(SEQ_FIRST),
                ),
                LIFECYCLE_LATE_RECEIVED_AT,
            )?,
            ObservationOutcome::RejectedStaleSession { current: None },
            "pre-session observation naming a binding",
        )?;
        let session = attach(graph.as_ref(), &child, SESSION_ONE)?;
        want(
            view_of(graph.as_ref(), &child)?.effective,
            None,
            "attaching a session starts a new epoch with no entries",
        )?;
        want(
            observe(
                graph.as_ref(),
                observation(
                    &child,
                    None,
                    LifecycleState::Idle,
                    ObservationSource::Screen,
                    ObservationSequence::SourceProvided(SEQ_SECOND),
                ),
                LIFECYCLE_FINAL_RECEIVED_AT,
            )?,
            ObservationOutcome::RejectedStaleSession {
                current: Some(session),
            },
            "binding-less observation after attach",
        )?;
        want(
            history_of(graph.as_ref(), &child)?.len(),
            HISTORY_LEN_PRE_SESSION,
            "every pre-session observation is recorded",
        )
    })
}

/// Case: a terminal node ignores every observation (IgnoredTerminal), the view
/// is unchanged, and the node's status does not move.
fn lifecycle_terminal_node_ignores_observations(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "lifecycle_terminal_node_ignores_observations";
    const RULE: &str = "E3a: observations on a terminal node are IgnoredTerminal, appended to history, and leave the view unchanged";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let (run_id, root, child) = (run("1"), inv("1"), inv("2"));
        admit_pair(graph.as_ref(), &run_id, &root, &child)?;
        let binding = attach(graph.as_ref(), &child, SESSION_ONE)?;
        observe(
            graph.as_ref(),
            observation(
                &child,
                Some(binding.clone()),
                LifecycleState::Working,
                ObservationSource::Structured,
                ObservationSequence::SourceProvided(SEQ_FIRST),
            ),
            LIFECYCLE_RECEIVED_AT,
        )?;
        let before = view_of(graph.as_ref(), &child)?;
        g(graph.time_out(&child, EpochSeconds::new(COMPLETE_AT), dl()))?;
        want(
            observe(
                graph.as_ref(),
                observation(
                    &child,
                    Some(binding),
                    LifecycleState::Done,
                    ObservationSource::Structured,
                    ObservationSequence::SourceProvided(SEQ_SECOND),
                ),
                LIFECYCLE_LATE_RECEIVED_AT,
            )?,
            ObservationOutcome::IgnoredTerminal,
            "done after timeout",
        )?;
        want(
            observe(
                graph.as_ref(),
                observation(
                    &child,
                    None,
                    LifecycleState::Idle,
                    ObservationSource::Screen,
                    ObservationSequence::GauntletAssigned,
                ),
                LIFECYCLE_FINAL_RECEIVED_AT,
            )?,
            ObservationOutcome::IgnoredTerminal,
            "gauntlet-assigned after timeout",
        )?;
        want(
            view_of(graph.as_ref(), &child)?,
            before,
            "observations after timeout changed the view",
        )?;
        let node = g(graph.node(&child, dl()))?.ok_or_else(|| "child vanished".to_owned())?;
        want(
            node.status,
            NodeStatus::TimedOut,
            "status after observations",
        )?;
        let history = history_of(graph.as_ref(), &child)?;
        want(
            history.len(),
            HISTORY_LEN_TERMINAL,
            "terminal observations are appended to history",
        )?;
        want(
            history
                .iter()
                .skip(1)
                .all(|record| record.outcome == ObservationOutcome::IgnoredTerminal),
            true,
            "every post-terminal record is IgnoredTerminal",
        )
    })
}

/// Case: Done and Idle observations leave the node's status, binding, and
/// budget untouched, and the node stays non-terminal.
fn lifecycle_done_and_idle_leave_node_status_unchanged(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "lifecycle_done_and_idle_leave_node_status_unchanged";
    const RULE: &str =
        "E3a hierarchy: harness Done and Idle never change node status, binding, or budget";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let (run_id, root, child) = (run("1"), inv("1"), inv("2"));
        admit_pair(graph.as_ref(), &run_id, &root, &child)?;
        let binding = attach(graph.as_ref(), &child, SESSION_ONE)?;
        g(graph.mark_running(&child, dl()))?;
        let subjects = [&child];
        let before = snapshot(graph.as_ref(), &subjects)?;
        observe(
            graph.as_ref(),
            observation(
                &child,
                Some(binding.clone()),
                LifecycleState::Done,
                ObservationSource::Structured,
                ObservationSequence::SourceProvided(SEQ_FIRST),
            ),
            LIFECYCLE_RECEIVED_AT,
        )?;
        want(
            observe(
                graph.as_ref(),
                observation(
                    &child,
                    Some(binding),
                    LifecycleState::Idle,
                    ObservationSource::Structured,
                    ObservationSequence::GauntletAssigned,
                ),
                LIFECYCLE_LATE_RECEIVED_AT,
            )?,
            ObservationOutcome::Applied {
                sequence: SEQ_SECOND,
                previous: Some(LifecycleState::Done),
            },
            "idle after done",
        )?;
        ensure_unchanged(
            graph.as_ref(),
            &subjects,
            &before,
            "done and idle observations",
        )?;
        let node = g(graph.node(&child, dl()))?.ok_or_else(|| "child vanished".to_owned())?;
        ensure(!node.status.is_terminal(), || {
            format!("observations made the node terminal: {}", node.status)
        })?;
        want(
            effective_state(&view_of(graph.as_ref(), &child)?),
            Some(LifecycleState::Idle),
            "the view still reports the observed idle",
        )
    })
}

/// Case: a Done observation cannot complete a node (a mismatched binding is
/// still refused), and its node fails and cancels exactly as a node without
/// the observation does.
fn lifecycle_done_observation_does_not_complete_or_fail(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "lifecycle_done_observation_does_not_complete_or_fail";
    const RULE: &str = "E3a hierarchy: a Done observation neither completes nor fails a node, and cancel/fail behave exactly as without it";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let run_id = run("1");
        let (root, with_done, without_done, fail_done, fail_plain) =
            (inv("1"), inv("2"), inv("3"), inv("4"), inv("5"));
        admit_ok(graph.as_ref(), root_request(&root, &run_id, "key-root"))?;
        for (id, key) in [
            (&with_done, "key-done"),
            (&without_done, "key-plain"),
            (&fail_done, "key-fail-done"),
            (&fail_plain, "key-fail-plain"),
        ] {
            admit_ok(graph.as_ref(), request(id, &run_id, Some(&root), key))?;
        }
        let done_binding = attach(graph.as_ref(), &with_done, SESSION_ONE)?;
        let fail_binding = attach(graph.as_ref(), &fail_done, SESSION_TWO)?;
        for id in [&with_done, &without_done, &fail_done, &fail_plain] {
            g(graph.mark_running(id, dl()))?;
        }
        observe(
            graph.as_ref(),
            observation(
                &with_done,
                Some(done_binding.clone()),
                LifecycleState::Done,
                ObservationSource::Structured,
                ObservationSequence::SourceProvided(SEQ_FIRST),
            ),
            LIFECYCLE_RECEIVED_AT,
        )?;
        observe(
            graph.as_ref(),
            observation(
                &fail_done,
                Some(fail_binding),
                LifecycleState::Done,
                ObservationSource::Structured,
                ObservationSequence::SourceProvided(SEQ_FIRST),
            ),
            LIFECYCLE_RECEIVED_AT,
        )?;
        ensure(
            matches!(
                graph.complete(
                    &with_done,
                    binding(CONTRACT_B, DIGEST_A),
                    EpochSeconds::new(COMPLETE_AT),
                    dl()
                ),
                Err(GraphError::ContractMismatch { .. })
            ),
            || "a Done observation let a mismatched binding complete the node".to_owned(),
        )?;
        let still = g(graph.node(&with_done, dl()))?
            .ok_or_else(|| "the admitted child is missing".to_owned())?;
        want(
            still.status,
            NodeStatus::Running,
            "status after a refused completion",
        )?;

        let failure = InvocationFailure::Execution;
        let failed_with = g(graph.fail(&fail_done, failure, EpochSeconds::new(COMPLETE_AT), dl()))?;
        let failed_plain =
            g(graph.fail(&fail_plain, failure, EpochSeconds::new(COMPLETE_AT), dl()))?;
        want(failed_with.status, failed_plain.status, "fail status")?;
        want(
            failed_with.terminal_at,
            failed_plain.terminal_at,
            "fail terminal time",
        )?;

        let cancelled = g(graph.cancel_subtree(&root, EpochSeconds::new(CANCEL_AT), dl()))?;
        want(
            cancelled,
            vec![root.clone(), with_done.clone(), without_done.clone()],
            "cancel_subtree returns the same ids with or without the Done observation",
        )
    })
}

/// Case: every received observation is in history in receipt order with its
/// outcome, origin, and receipt time; the sequence used is recorded.
fn lifecycle_history_records_every_received_observation(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "lifecycle_history_records_every_received_observation";
    const RULE: &str = "E3a: observation history is append-only and holds every received observation, ignored and rejected ones included, in receipt order";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let (run_id, root, child) = (run("1"), inv("1"), inv("2"));
        admit_pair(graph.as_ref(), &run_id, &root, &child)?;
        let binding = attach(graph.as_ref(), &child, SESSION_ONE)?;
        let working = |sequence: ObservationSequence| {
            observation(
                &child,
                Some(binding.clone()),
                LifecycleState::Working,
                ObservationSource::Structured,
                sequence,
            )
        };
        let received = [
            (
                working(ObservationSequence::SourceProvided(SEQ_FIRST)),
                LIFECYCLE_RECEIVED_AT,
            ),
            (
                working(ObservationSequence::SourceProvided(SEQ_FIRST)),
                LIFECYCLE_LATE_RECEIVED_AT,
            ),
            (
                observation(
                    &child,
                    None,
                    LifecycleState::Working,
                    ObservationSource::Structured,
                    ObservationSequence::SourceProvided(SEQ_LATE),
                ),
                LIFECYCLE_FINAL_RECEIVED_AT,
            ),
            (
                observation(
                    &child,
                    Some(binding.clone()),
                    LifecycleState::Idle,
                    ObservationSource::Structured,
                    ObservationSequence::GauntletAssigned,
                ),
                LIFECYCLE_LAST_RECEIVED_AT,
            ),
        ];
        let mut outcomes = Vec::new();
        for (observation, at) in received {
            outcomes.push(observe(graph.as_ref(), observation, at)?);
        }
        let expected_outcomes = vec![
            ObservationOutcome::Applied {
                sequence: SEQ_FIRST,
                previous: None,
            },
            ObservationOutcome::IgnoredStale {
                last_applied: SEQ_FIRST,
            },
            ObservationOutcome::RejectedStaleSession {
                current: Some(binding),
            },
            ObservationOutcome::Applied {
                sequence: SEQ_SECOND,
                previous: Some(LifecycleState::Working),
            },
        ];
        want(
            outcomes,
            expected_outcomes,
            "outcomes returned by the writes",
        )?;

        let history = history_of(graph.as_ref(), &child)?;
        want(history.len(), HISTORY_LEN_RECEIPT, "history length")?;
        want(
            history
                .iter()
                .map(|record| record.received_at)
                .collect::<Vec<_>>(),
            [
                LIFECYCLE_RECEIVED_AT,
                LIFECYCLE_LATE_RECEIVED_AT,
                LIFECYCLE_FINAL_RECEIVED_AT,
                LIFECYCLE_LAST_RECEIVED_AT,
            ]
            .map(EpochSeconds::new)
            .to_vec(),
            "receipt times in receipt order",
        )?;
        want(
            history
                .iter()
                .map(|record| record.origin)
                .collect::<Vec<_>>(),
            vec![
                SequenceOrigin::SourceProvided,
                SequenceOrigin::SourceProvided,
                SequenceOrigin::SourceProvided,
                SequenceOrigin::GauntletAssigned,
            ],
            "origins in receipt order",
        )?;
        want(
            history.last().map(|record| record.observation.sequence),
            Some(ObservationSequence::SourceProvided(SEQ_SECOND)),
            "the assigned sequence is recorded as used",
        )?;
        want(
            history
                .iter()
                .map(|record| record.outcome.clone())
                .collect::<Vec<_>>(),
            vec![
                ObservationOutcome::Applied {
                    sequence: SEQ_FIRST,
                    previous: None,
                },
                ObservationOutcome::IgnoredStale {
                    last_applied: SEQ_FIRST,
                },
                ObservationOutcome::RejectedStaleSession {
                    current: Some(bound(SESSION_ONE, EPOCH_FIRST)),
                },
                ObservationOutcome::Applied {
                    sequence: SEQ_SECOND,
                    previous: Some(LifecycleState::Working),
                },
            ],
            "history outcomes match the writes",
        )
    })
}

/// Case: the three lifecycle queries report an unknown invocation as
/// UnknownInvocation.
fn lifecycle_unknown_invocation_is_reported(
    factory: &dyn InvocationGraphFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "lifecycle_unknown_invocation_is_reported";
    const RULE: &str = "E3a: record_observation, lifecycle and observation_history on an unknown invocation are UnknownInvocation";
    check(CASE, RULE, || {
        let graph = build(factory)?;
        let stranger = inv("9");
        let probe = observation(
            &stranger,
            None,
            LifecycleState::Working,
            ObservationSource::Screen,
            ObservationSequence::GauntletAssigned,
        );
        ensure(
            matches!(
                graph.record_observation(probe, EpochSeconds::new(LIFECYCLE_RECEIVED_AT), dl()),
                Err(GraphError::UnknownInvocation { .. })
            ),
            || "record_observation of an unknown id was not UnknownInvocation".to_owned(),
        )?;
        ensure(
            matches!(
                graph.lifecycle(&stranger, dl()),
                Err(GraphError::UnknownInvocation { .. })
            ),
            || "lifecycle() of an unknown id was not UnknownInvocation".to_owned(),
        )?;
        ensure(
            matches!(
                graph.observation_history(&stranger, dl()),
                Err(GraphError::UnknownInvocation { .. })
            ),
            || "observation_history() of an unknown id was not UnknownInvocation".to_owned(),
        )
    })
}

/// Runs the full `InvocationGraph` contract suite against one factory.
pub fn invocation_graph_suite(factory: &dyn InvocationGraphFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(root_admission_records_a_root(factory));
    findings.extend(child_admission_records_parent_root_and_depth(factory));
    findings.extend(depth_limit_refusal_creates_no_node(factory));
    findings.extend(child_limit_refusal_leaves_graph_unchanged(factory));
    findings.extend(budget_exceeded_each_dimension(factory));
    findings.extend(budget_released_when_child_turns_terminal(factory));
    findings.extend(idempotent_replay_returns_already_admitted(factory));
    findings.extend(key_reuse_with_different_facts_refused(factory));
    findings.extend(second_root_refused(factory));
    findings.extend(admission_under_terminal_parent_refused(factory));
    findings.extend(unknown_and_foreign_parents_refused(factory));
    findings.extend(heterogeneous_lineage_is_preserved(factory));
    findings.extend(attach_native_session_epochs_increment(factory));
    findings.extend(attach_native_session_refused_on_terminal(factory));
    findings.extend(complete_with_matching_binding_succeeds(factory));
    findings.extend(complete_with_wrong_contract_path_mismatches(factory));
    findings.extend(complete_with_wrong_context_digest_mismatches(factory));
    findings.extend(child_binding_cannot_complete_sibling(factory));
    findings.extend(complete_twice_is_illegal(factory));
    findings.extend(failed_and_timed_out_are_final(factory));
    findings.extend(cancel_subtree_cancels_active_descendants_only(factory));
    findings.extend(cancel_subtree_on_a_terminal_root_cancels_only_active_descendants(factory));
    findings.extend(cancel_subtree_on_a_terminal_leaf_is_a_noop_returning_empty(
        factory,
    ));
    findings.extend(child_without_binding_stays_non_terminal(factory));
    findings.extend(active_nodes_excludes_terminal(factory));
    findings.extend(descendants_order_is_deterministic(factory));
    findings.extend(unknown_invocations_are_reported(factory));
    findings.extend(lifecycle_late_lower_sequence_is_ignored_stale(factory));
    findings.extend(lifecycle_equal_sequence_replay_is_ignored(factory));
    findings.extend(lifecycle_higher_sequence_applies(factory));
    findings.extend(lifecycle_sources_are_independent_and_trust_ranked(factory));
    findings.extend(lifecycle_process_only_entry_is_effective(factory));
    findings.extend(lifecycle_gauntlet_sequences_count_per_key(factory));
    findings.extend(lifecycle_gauntlet_sequences_restart_per_epoch(factory));
    findings.extend(lifecycle_stale_session_observation_is_rejected(factory));
    findings.extend(lifecycle_wrong_binding_is_rejected(factory));
    findings.extend(lifecycle_pre_session_observation_needs_no_binding(factory));
    findings.extend(lifecycle_terminal_node_ignores_observations(factory));
    findings.extend(lifecycle_done_and_idle_leave_node_status_unchanged(factory));
    findings.extend(lifecycle_done_observation_does_not_complete_or_fail(
        factory,
    ));
    findings.extend(lifecycle_history_records_every_received_observation(
        factory,
    ));
    findings.extend(lifecycle_unknown_invocation_is_reported(factory));
    findings
}
