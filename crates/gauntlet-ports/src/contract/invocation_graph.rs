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

use crate::agent_runner::InvocationFailure;
use crate::budget_meter::BudgetSlice;
use crate::contract::ContractFinding;
use crate::deadline::Deadline;
use crate::invocation_graph::{
    AdmissionOutcome, AdmissionRefusal, AdmissionRequest, GraphError, GraphLimits, InvocationGraph,
    InvocationNode, NativeSessionBinding, NodeStatus, ResultBinding,
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
    findings.extend(child_without_binding_stays_non_terminal(factory));
    findings.extend(active_nodes_excludes_terminal(factory));
    findings.extend(descendants_order_is_deterministic(factory));
    findings.extend(unknown_invocations_are_reported(factory));
    findings
}
