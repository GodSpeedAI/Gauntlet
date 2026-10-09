//! Deterministic tests for the DELEGATE path, run against the in-memory
//! reference graph and scripted runner doubles.

use super::test_support::*;
use std::collections::BTreeSet;
use std::sync::Arc;

use gauntlet_domain::{BudgetDimension, EpochSeconds, HarnessId, InvocationId};
use gauntlet_ports::InvocationStatus;
use gauntlet_ports::agent_runner::{AgentError, InvocationFailure};
use gauntlet_ports::invocation_graph::{
    AdmissionRefusal, GraphError, InvocationGraph, NodeStatus, ResultBinding,
};
use gauntlet_ports::lifecycle::{
    HarnessObservation, LifecycleState, ObservationSequence, ObservationSource,
};
use gauntlet_ports::role_result::encode_role_result;

use super::DelegationBroker;
use super::broker::{DelegationOutcome, DelegationRefusal};
use super::context::{
    ContextItemKind, ContextPolicy, ContextRefusal, MAX_ENVELOPE_BYTES, build_envelope,
};
use super::reconcile::{BrokerError, CollectOutcome, RecoveryAction};
use crate::fabric::SupportLevel;
use crate::fabric::selection::SelectionRefusal;

#[test]
fn fresh_context_body_carries_exactly_the_allowed_items() {
    let (lab, a, _, _) = standard_lab();
    // The parent's material exists only in this local; no item carries it.
    let parent_transcript = format!("{} {}", "secret", "transcript marker");
    let items = vec![
        task(TASK_TEXT),
        reference("spec-excerpt", REF_TEXT),
        item(
            ContextItemKind::ConstraintStatement,
            "no-network",
            CONSTRAINT_TEXT,
        ),
    ];
    let delegated = lab
        .broker()
        .delegate(root_request(ROOT_TAG, KEY_ROOT, items))
        .expect("the root launches");

    assert_eq!(delegated.outcome, DelegationOutcome::Launched);
    let spec = a.last_spec();
    let body = spec.context_envelope.body();
    assert!(!body.contains(&parent_transcript));
    assert_eq!(
        spec.context_envelope.digest(),
        &delegated.node.context_digest
    );
    assert_eq!(
        decode(body),
        vec![
            triple("task-statement", "task", TASK_TEXT),
            triple("reference-excerpt", "spec-excerpt", REF_TEXT),
            triple("constraint-statement", "no-network", CONSTRAINT_TEXT),
        ]
    );
}

#[test]
fn identical_items_yield_identical_digests_and_different_items_differ() {
    let policy = full_policy();
    let first = build_envelope(&policy, &[task(TASK_TEXT), reference("spec", REF_TEXT)])
        .expect("envelope builds");
    let reordered = build_envelope(&policy, &[reference("spec", REF_TEXT), task(TASK_TEXT)])
        .expect("envelope builds");
    let changed = build_envelope(
        &policy,
        &[task(TASK_TEXT), reference("spec", OTHER_REF_TEXT)],
    )
    .expect("envelope builds");

    assert_eq!(first.body(), reordered.body());
    assert_eq!(first.digest(), reordered.digest());
    assert_ne!(first.digest(), changed.digest());
}

#[test]
fn denied_kind_is_refused_with_no_runner_call_and_no_node() {
    let (lab, a, b, c) = standard_lab();
    let mut denied = root_request(
        ROOT_TAG,
        KEY_ROOT,
        vec![
            task(TASK_TEXT),
            item(ContextItemKind::EvidenceReference, "builder-trace", "trace"),
        ],
    );
    denied.policy = ContextPolicy {
        allow: all_kinds(),
        deny: [ContextItemKind::EvidenceReference].into_iter().collect(),
    };

    let refusal = lab
        .broker()
        .delegate(denied)
        .expect_err("the kind is denied");

    assert_eq!(
        refusal,
        DelegationRefusal::Context(ContextRefusal::KindNotAllowed {
            kind: ContextItemKind::EvidenceReference,
        })
    );
    assert_eq!((a.invokes(), b.invokes(), c.invokes()), (0, 0, 0));
    assert!(lab.node(ROOT_TAG).is_none());
    assert_eq!(lab.active_count(), 0);
}

#[test]
fn content_forging_a_second_item_cannot_change_the_item_list() {
    let policy = full_policy();
    let honest = build_envelope(
        &policy,
        &[
            task(TASK_TEXT),
            reference("alpha", "one"),
            reference("beta", "xyz"),
        ],
    )
    .expect("honest envelope builds");
    // Imitates the delimiter and a second item inside one item's content.
    let forged_content = "one\nreference-excerpt 4 3\nbeta\nxyz";
    let forged = build_envelope(
        &policy,
        &[task(TASK_TEXT), reference("alpha", forged_content)],
    )
    .expect("forged envelope builds");

    assert_ne!(honest.digest(), forged.digest());
    assert_eq!(decode(honest.body()).len(), 3);
    let decoded = decode(forged.body());
    assert_eq!(decoded.len(), 2);
    assert_eq!(
        decoded[1],
        triple("reference-excerpt", "alpha", forged_content)
    );
}

#[test]
fn policy_deny_wins_and_unlisted_kinds_are_refused() {
    let both: ContextPolicy = ContextPolicy {
        allow: all_kinds(),
        deny: [ContextItemKind::ReferenceExcerpt].into_iter().collect(),
    };
    assert_eq!(
        build_envelope(&both, &[task(TASK_TEXT), reference("spec", REF_TEXT)])
            .expect_err("deny wins over allow"),
        ContextRefusal::KindNotAllowed {
            kind: ContextItemKind::ReferenceExcerpt,
        }
    );

    let narrow = ContextPolicy {
        allow: [ContextItemKind::TaskStatement].into_iter().collect(),
        deny: BTreeSet::new(),
    };
    assert_eq!(
        build_envelope(&narrow, &[task(TASK_TEXT), reference("spec", REF_TEXT)])
            .expect_err("a kind outside allow is refused"),
        ContextRefusal::KindNotAllowed {
            kind: ContextItemKind::ReferenceExcerpt,
        }
    );
}

#[test]
fn blank_task_duplicate_and_oversized_envelopes_are_refused() {
    let policy = full_policy();
    assert_eq!(
        build_envelope(&policy, &[task("   "), reference("spec", REF_TEXT)])
            .expect_err("a blank task statement is refused"),
        ContextRefusal::EmptyTaskStatement
    );
    assert_eq!(
        build_envelope(
            &policy,
            &[
                task(TASK_TEXT),
                reference("spec", REF_TEXT),
                reference("spec", OTHER_REF_TEXT),
            ],
        )
        .expect_err("a duplicate (kind, label) is refused"),
        ContextRefusal::DuplicateItem {
            kind: ContextItemKind::ReferenceExcerpt,
            label: "spec".to_owned(),
        }
    );
    let oversized = "x".repeat(MAX_ENVELOPE_BYTES);
    assert!(matches!(
        build_envelope(&policy, &[task(TASK_TEXT), reference("big", &oversized)]),
        Err(ContextRefusal::BodyTooLarge { limit, size })
            if limit == MAX_ENVELOPE_BYTES && size > MAX_ENVELOPE_BYTES
    ));
}

// ---------------------------------------------------------------------------
// (d)-(f) Order and zero side effects
// ---------------------------------------------------------------------------

#[test]
fn unknown_parent_is_refused_with_zero_runner_calls() {
    let (lab, a, b, c) = standard_lab();
    let refusal = lab
        .broker()
        .delegate(request(
            CHILD_TAG,
            Some(UNKNOWN_TAG),
            KEY_CHILD,
            items_for(CHILD_MARKER),
        ))
        .expect_err("the parent is unknown");
    assert_eq!(
        refusal,
        DelegationRefusal::Admission(AdmissionRefusal::UnknownParent)
    );
    assert_eq!((a.invokes(), b.invokes(), c.invokes()), (0, 0, 0));
    assert!(lab.node(CHILD_TAG).is_none());
}

#[test]
fn depth_limit_refusal_makes_no_runner_call_and_leaves_the_graph_unchanged() {
    let (lab, a, b, c) = standard_lab();
    lab.broker()
        .delegate(root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)))
        .expect("the root launches");
    let before = lab.descendant_count(ROOT_TAG);

    let mut too_deep = request(
        CHILD_TAG,
        Some(ROOT_TAG),
        KEY_CHILD,
        items_for(CHILD_MARKER),
    );
    too_deep.limits.max_depth = 0;
    let refusal = lab
        .broker()
        .delegate(too_deep)
        .expect_err("the child would sit below the depth limit");

    assert_eq!(
        refusal,
        DelegationRefusal::Admission(AdmissionRefusal::DepthLimit {
            max: 0,
            would_be: 1,
        })
    );
    assert_eq!((a.invokes(), b.invokes(), c.invokes()), (1, 0, 0));
    assert_eq!(lab.descendant_count(ROOT_TAG), before);
    assert!(lab.node(CHILD_TAG).is_none());
}

#[test]
fn child_limit_refusal_makes_no_runner_call_and_leaves_the_graph_unchanged() {
    let (lab, a, b, c) = standard_lab();
    lab.broker()
        .delegate(root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)))
        .expect("the root launches");
    for (tag, key) in [(CHILD_TAG, "key-1"), (SECOND_CHILD_TAG, "key-2")] {
        let mut child = request(tag, Some(ROOT_TAG), key, items_for(CHILD_MARKER));
        child.budget = grand_slice();
        lab.broker()
            .delegate(child)
            .expect("a live child is admitted");
    }
    let before = lab.descendant_count(ROOT_TAG);

    let mut third = request(
        THIRD_CHILD_TAG,
        Some(ROOT_TAG),
        "key-3",
        items_for(CHILD_MARKER),
    );
    third.budget = grand_slice();
    let refusal = lab
        .broker()
        .delegate(third)
        .expect_err("the parent already holds its live children");

    assert_eq!(
        refusal,
        DelegationRefusal::Admission(AdmissionRefusal::ChildLimit { max: MAX_CHILDREN })
    );
    assert_eq!((a.invokes(), b.invokes(), c.invokes()), (3, 0, 0));
    assert_eq!(lab.descendant_count(ROOT_TAG), before);
    assert!(lab.node(THIRD_CHILD_TAG).is_none());
}

#[test]
fn budget_refusal_makes_no_runner_call_and_leaves_the_graph_unchanged() {
    let (lab, a, b, c) = standard_lab();
    lab.broker()
        .delegate(root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)))
        .expect("the root launches");
    let before = lab.descendant_count(ROOT_TAG);

    let mut greedy = request(
        CHILD_TAG,
        Some(ROOT_TAG),
        KEY_CHILD,
        items_for(CHILD_MARKER),
    );
    greedy.budget.rounds = ROOT_ROUNDS + 1;
    let refusal = lab
        .broker()
        .delegate(greedy)
        .expect_err("the slice exceeds the parent budget");

    assert_eq!(
        refusal,
        DelegationRefusal::Admission(AdmissionRefusal::BudgetExceeded {
            dimension: BudgetDimension::Rounds,
        })
    );
    assert_eq!((a.invokes(), b.invokes(), c.invokes()), (1, 0, 0));
    assert_eq!(lab.descendant_count(ROOT_TAG), before);
    assert!(lab.node(CHILD_TAG).is_none());
}

#[test]
fn terminal_parent_refuses_new_children_with_zero_runner_calls() {
    let (lab, a, b, c) = standard_lab();
    let root = lab
        .broker()
        .delegate(root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)))
        .expect("the root launches")
        .node;
    lab.graph
        .complete(
            &inv(ROOT_TAG),
            ResultBinding {
                contract_path: root.result_contract.clone(),
                context_digest: root.context_digest.clone(),
                result_digest: root.context_digest.clone(),
            },
            EpochSeconds::new(ADMIT_AT),
            deadline(),
        )
        .expect("the root completes");

    let refusal = lab
        .broker()
        .delegate(request(
            CHILD_TAG,
            Some(ROOT_TAG),
            KEY_CHILD,
            items_for(CHILD_MARKER),
        ))
        .expect_err("a settled parent takes no new work");

    assert_eq!(
        refusal,
        DelegationRefusal::Admission(AdmissionRefusal::ParentTerminal)
    );
    assert_eq!((a.invokes(), b.invokes(), c.invokes()), (1, 0, 0));
    assert!(lab.node(CHILD_TAG).is_none());
}

#[test]
fn selection_refusals_make_no_node_and_no_runner_call() {
    let a = Arc::new(ScriptedRunner::new("a"));
    let mut without_model_selection = instance("profile-a", "harness-a");
    without_model_selection.capabilities.model_selection = false;
    let lab = Lab::new(
        vec![instance("profile-a", "harness-a")],
        &[("profile-a", Arc::clone(&a))],
    );
    let no_model = Lab::new(
        vec![without_model_selection],
        &[("profile-a", Arc::clone(&a))],
    );

    // No instance reaches the required support level.
    let mut too_strong = request(ROOT_TAG, None, KEY_ROOT, items_for(ROOT_MARKER));
    too_strong.requirements.min_support_level = SupportLevel::Native;
    assert!(matches!(
        lab.broker().delegate(too_strong),
        Err(DelegationRefusal::NoEligibleRunner(
            SelectionRefusal::NoEligibleRunner { .. }
        ))
    ));

    // Capability mismatch: the only runner cannot select a model.
    let mut needs_model = request(ROOT_TAG, None, KEY_ROOT, items_for(ROOT_MARKER));
    needs_model.requirements.needs_model_selection = true;
    assert!(matches!(
        no_model.broker().delegate(needs_model),
        Err(DelegationRefusal::NoEligibleRunner(
            SelectionRefusal::NoEligibleRunner { .. }
        ))
    ));

    // A pin that violates the requirements is refused, not replaced.
    let mut pin_violation = pinned(
        request(ROOT_TAG, None, KEY_ROOT, items_for(ROOT_MARKER)),
        "profile-a",
    );
    pin_violation.requirements.min_support_level = SupportLevel::Native;
    assert!(matches!(
        lab.broker().delegate(pin_violation),
        Err(DelegationRefusal::NoEligibleRunner(
            SelectionRefusal::PinViolatesRequirements { .. }
        ))
    ));

    // A pin naming no profile is refused.
    let unknown_pin = pinned(
        request(ROOT_TAG, None, KEY_ROOT, items_for(ROOT_MARKER)),
        "profile-zz",
    );
    assert!(matches!(
        lab.broker().delegate(unknown_pin),
        Err(DelegationRefusal::NoEligibleRunner(
            SelectionRefusal::PinUnknown { .. }
        ))
    ));

    assert_eq!(a.invokes(), 0);
    assert_eq!(lab.active_count(), 0);
    assert_eq!(no_model.active_count(), 0);
}

#[test]
fn explicit_pin_is_honored() {
    let (lab, a, b, c) = standard_lab();
    let delegated = lab
        .broker()
        .delegate(pinned(
            root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)),
            "profile-b",
        ))
        .expect("the pinned runner launches");

    assert_eq!(delegated.node.runner_profile, profile_id("profile-b"));
    assert_eq!(
        delegated.node.harness,
        HarnessId::new("harness-b").expect("valid")
    );
    assert_eq!((a.invokes(), b.invokes(), c.invokes()), (0, 1, 0));
}

// ---------------------------------------------------------------------------
// (g)-(i) Exactly-once launch
// ---------------------------------------------------------------------------

#[test]
fn same_request_twice_launches_once_and_returns_already_launched() {
    let (lab, a, _, _) = standard_lab();
    let req = root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER));
    let first = lab
        .broker()
        .delegate(req.clone())
        .expect("the first call launches");
    let second = lab
        .broker()
        .delegate(req)
        .expect("the replay is not a refusal");

    assert_eq!(first.outcome, DelegationOutcome::Launched);
    assert_eq!(second.outcome, DelegationOutcome::AlreadyLaunched);
    assert_eq!(second.node.invocation_id, first.node.invocation_id);
    assert_eq!(second.handle, first.handle);
    assert_eq!(a.invokes(), 1);
    assert_eq!(lab.active_count(), 1);
}

#[test]
fn crash_after_failed_invoke_is_relaunched_exactly_once_on_retry() {
    let (lab, a, _, _) = standard_lab();
    a.fail_next_invokes(1);
    let req = root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER));

    let refusal = lab
        .broker()
        .delegate(req.clone())
        .expect_err("the first invoke fails");
    assert!(matches!(
        refusal,
        DelegationRefusal::RunnerAdmission(AgentError::Transport { .. })
    ));
    let stranded = lab.node(ROOT_TAG).expect("the node was admitted");
    assert_eq!(stranded.status, NodeStatus::Admitted);
    assert!(stranded.native_session.is_none());

    let retried = lab
        .broker()
        .delegate(req.clone())
        .expect("the retry with the same key launches");
    assert_eq!(retried.outcome, DelegationOutcome::Launched);
    assert_eq!(a.invokes(), 2);
    assert_eq!(retried.node.status, NodeStatus::Running);
    assert_eq!(
        retried
            .node
            .native_session
            .as_ref()
            .map(|binding| binding.epoch),
        Some(1)
    );
    assert_eq!(lab.active_count(), 1);

    let replay = lab
        .broker()
        .delegate(req)
        .expect("the replay is not a refusal");
    assert_eq!(replay.outcome, DelegationOutcome::AlreadyLaunched);
    assert_eq!(a.invokes(), 2);
}

#[test]
fn same_key_with_different_facts_is_refused_without_a_runner_call() {
    let (lab, a, _, _) = standard_lab();
    lab.broker()
        .delegate(root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)))
        .expect("the first call launches");

    let refusal = lab
        .broker()
        .delegate(root_request(
            ROOT_TAG,
            KEY_ROOT,
            vec![task(TASK_TEXT), reference("spec", OTHER_REF_TEXT)],
        ))
        .expect_err("the key is bound to other facts");

    assert_eq!(
        refusal,
        DelegationRefusal::Admission(AdmissionRefusal::KeyReusedWithDifferentFacts)
    );
    assert_eq!(a.invokes(), 1);
}

// ---------------------------------------------------------------------------
// (j) Heterogeneous lineage
// ---------------------------------------------------------------------------

#[test]
fn heterogeneous_lineage_across_three_harnesses_respects_graph_limits() {
    let (lab, a, b, c) = standard_lab();
    lab.broker()
        .delegate(pinned(
            root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)),
            "profile-a",
        ))
        .expect("the root launches on harness-a");
    lab.broker()
        .delegate(pinned(
            request(
                CHILD_TAG,
                Some(ROOT_TAG),
                KEY_CHILD,
                items_for(CHILD_MARKER),
            ),
            "profile-b",
        ))
        .expect("the child launches on harness-b");
    let mut grandchild = pinned(
        request(
            GRANDCHILD_TAG,
            Some(CHILD_TAG),
            "key-grand",
            items_for(GRAND_MARKER),
        ),
        "profile-c",
    );
    grandchild.budget = grand_slice();
    lab.broker()
        .delegate(grandchild)
        .expect("the grandchild launches on harness-c");

    // Each runner was invoked once and received only its own envelope.
    assert_eq!((a.invokes(), b.invokes(), c.invokes()), (1, 1, 1));
    let a_body = a.last_spec().context_envelope.body().to_owned();
    let b_body = b.last_spec().context_envelope.body().to_owned();
    let c_body = c.last_spec().context_envelope.body().to_owned();
    assert!(a_body.contains(ROOT_MARKER) && !a_body.contains(CHILD_MARKER));
    assert!(b_body.contains(CHILD_MARKER) && !b_body.contains(GRAND_MARKER));
    assert!(c_body.contains(GRAND_MARKER) && !c_body.contains(ROOT_MARKER));

    // Lineage: depths, roots, ancestors, and descendants come from the graph.
    let root = lab.node(ROOT_TAG).expect("root node");
    let child = lab.node(CHILD_TAG).expect("child node");
    let grand = lab.node(GRANDCHILD_TAG).expect("grandchild node");
    assert_eq!((root.depth, child.depth, grand.depth), (0, 1, 2));
    assert_eq!(grand.root, inv(ROOT_TAG));
    assert_eq!(grand.harness, HarnessId::new("harness-c").expect("valid"));
    let ancestors: Vec<InvocationId> = lab
        .graph
        .ancestors(&inv(GRANDCHILD_TAG), deadline())
        .expect("reads")
        .into_iter()
        .map(|node| node.invocation_id)
        .collect();
    assert_eq!(ancestors, vec![inv(CHILD_TAG), inv(ROOT_TAG)]);
    let descendants: Vec<InvocationId> = lab
        .graph
        .descendants(&inv(ROOT_TAG), deadline())
        .expect("reads")
        .into_iter()
        .map(|node| node.invocation_id)
        .collect();
    assert_eq!(descendants, vec![inv(CHILD_TAG), inv(GRANDCHILD_TAG)]);

    // A fourth level is refused by the graph's depth limit, with no call.
    let mut great = pinned(
        request(
            GREAT_GRANDCHILD_TAG,
            Some(GRANDCHILD_TAG),
            "key-great",
            items_for(GRAND_MARKER),
        ),
        "profile-a",
    );
    great.budget = grand_slice();
    let refusal = lab
        .broker()
        .delegate(great)
        .expect_err("the fourth level exceeds the depth limit");
    assert_eq!(
        refusal,
        DelegationRefusal::Admission(AdmissionRefusal::DepthLimit {
            max: MAX_DEPTH,
            would_be: 3,
        })
    );
    assert_eq!((a.invokes(), b.invokes(), c.invokes()), (1, 1, 1));
    assert!(lab.node(GREAT_GRANDCHILD_TAG).is_none());
}

// ---------------------------------------------------------------------------

#[test]
fn a_handle_without_session_identity_is_failed_cancelled_and_never_replayed() {
    let (lab, a, _, _) = standard_lab();
    a.omit_session_identity();
    let req = root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER));

    let refusal = lab
        .broker()
        .delegate(req.clone())
        .expect_err("a handle with no session identity is refused");
    assert_eq!(
        refusal,
        DelegationRefusal::RunnerGaveNoSessionIdentity {
            profile: profile_id("profile-a"),
        }
    );
    let failed = lab.node(ROOT_TAG).expect("the node was admitted");
    assert_eq!(
        failed.status,
        NodeStatus::Failed(InvocationFailure::Execution)
    );
    assert!(failed.native_session.is_none());
    assert_eq!((a.invokes(), a.cancels()), (1, 1));

    let replay = lab
        .broker()
        .delegate(req)
        .expect_err("a settled key is never launched again");
    assert_eq!(
        replay,
        DelegationRefusal::AlreadyTerminal {
            node: Box::new(failed),
        }
    );
    assert_eq!((a.invokes(), a.cancels()), (1, 1));
}

#[test]
fn a_replay_heals_a_lost_running_transition_without_a_second_launch() {
    let a = Arc::new(ScriptedRunner::new("a"));
    let lab = Lab::new(
        vec![instance("profile-a", "harness-a")],
        &[("profile-a", Arc::clone(&a))],
    );
    let graph = ProbeGraph::default();
    graph.mark_running_failures.set(1);
    let broker = DelegationBroker::new(&graph, &lab.registry, &lab.runners);
    let req = root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER));

    let lost = broker
        .delegate(req.clone())
        .expect_err("the Running write fails");
    assert!(matches!(
        lost,
        DelegationRefusal::Graph(GraphError::WriteFailed { .. })
    ));
    let stranded = graph
        .inner
        .node(&inv(ROOT_TAG), deadline())
        .expect("the graph reads")
        .expect("the node was admitted");
    assert_eq!(stranded.status, NodeStatus::Admitted);
    assert!(stranded.native_session.is_some());

    let healed = broker
        .delegate(req)
        .expect("the replay heals the transition");
    assert_eq!(healed.outcome, DelegationOutcome::AlreadyLaunched);
    assert_eq!(healed.node.status, NodeStatus::Running);
    assert_eq!(a.invokes(), 1);
}

// ---------------------------------------------------------------------------
// collect: completion is invocation completion, bound by the graph
// ---------------------------------------------------------------------------

#[test]
fn collect_completes_a_bound_result_and_stays_open_while_the_runner_runs() {
    let (lab, a, _, _) = standard_lab();
    let root = lab
        .broker()
        .delegate(root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)))
        .expect("the root launches")
        .node;

    a.set_status(&inv(ROOT_TAG), running_status());
    assert!(matches!(
        lab.broker()
            .collect(&inv(ROOT_TAG), at(), deadline())
            .expect("the runner is polled"),
        CollectOutcome::StillRunning(_)
    ));
    assert_eq!(a.collects(), 0);

    a.set_status(&inv(ROOT_TAG), completed_status());
    a.set_result(builder_result(&inv(ROOT_TAG), &root.context_digest, false));
    let CollectOutcome::Completed { node, result } = lab
        .broker()
        .collect(&inv(ROOT_TAG), at(), deadline())
        .expect("the runner is polled")
    else {
        panic!("a bound result completes the invocation");
    };
    assert_eq!(node.status, NodeStatus::Completed);
    assert!(node.result_digest.is_some());
    assert_eq!(result.invocation_id(), &inv(ROOT_TAG));
    assert_eq!(node_status(&lab, ROOT_TAG), NodeStatus::Completed);
    assert_eq!(a.collects(), 1);
}

#[test]
fn a_result_bound_to_another_childs_envelope_is_a_graph_contract_mismatch() {
    let (lab, _, b, c) = standard_lab();
    lab.broker()
        .delegate(root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)))
        .expect("the root launches");
    lab.broker()
        .delegate(pinned(
            request(
                CHILD_TAG,
                Some(ROOT_TAG),
                KEY_CHILD,
                items_for(CHILD_MARKER),
            ),
            "profile-b",
        ))
        .expect("the child launches");
    lab.broker()
        .delegate(pinned(
            request(
                SECOND_CHILD_TAG,
                Some(ROOT_TAG),
                KEY_SECOND,
                items_for(GRAND_MARKER),
            ),
            "profile-c",
        ))
        .expect("the sibling launches");
    let sibling = lab.node(SECOND_CHILD_TAG).expect("the sibling node");

    // The child's runner hands back a document bound to the sibling's envelope.
    b.set_status(&inv(CHILD_TAG), completed_status());
    b.set_result(builder_result(
        &inv(CHILD_TAG),
        &sibling.context_digest,
        false,
    ));
    let outcome = lab
        .broker()
        .collect(&inv(CHILD_TAG), at(), deadline())
        .expect("the runner is polled");

    let child = lab.node(CHILD_TAG).expect("the child node");
    assert_eq!(outcome, CollectOutcome::Failed(child.clone()));
    assert_eq!(
        child.status,
        NodeStatus::Failed(InvocationFailure::ContractMismatch)
    );
    assert_eq!(
        lab.node(SECOND_CHILD_TAG).expect("the sibling node"),
        sibling
    );
    assert_eq!(c.collects(), 0);
}

#[test]
fn a_same_digest_result_from_another_invocation_is_refused_by_the_broker() {
    // Identical items give identical envelopes, so the digest cannot tell
    // siblings apart. Only the invocation id does, and the broker checks it.
    let (lab, _, b, _) = standard_lab();
    lab.broker()
        .delegate(root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)))
        .expect("the root launches");
    lab.broker()
        .delegate(pinned(
            request(
                CHILD_TAG,
                Some(ROOT_TAG),
                KEY_CHILD,
                items_for(CHILD_MARKER),
            ),
            "profile-b",
        ))
        .expect("the child launches");
    let sibling = lab
        .broker()
        .delegate(pinned(
            request(
                SECOND_CHILD_TAG,
                Some(ROOT_TAG),
                KEY_SECOND,
                items_for(CHILD_MARKER),
            ),
            "profile-c",
        ))
        .expect("the sibling launches")
        .node;
    let child_digest = lab.node(CHILD_TAG).expect("the child node").context_digest;
    assert_eq!(child_digest, sibling.context_digest);

    b.set_status(&inv(CHILD_TAG), completed_status());
    b.set_result(builder_result(
        &inv(SECOND_CHILD_TAG),
        &sibling.context_digest,
        false,
    ));
    let outcome = lab
        .broker()
        .collect(&inv(CHILD_TAG), at(), deadline())
        .expect("the runner is polled");
    assert!(matches!(outcome, CollectOutcome::Failed(ref node)
        if node.status == NodeStatus::Failed(InvocationFailure::ContractMismatch)));
    assert_eq!(node_status(&lab, SECOND_CHILD_TAG), NodeStatus::Running);
}

#[test]
fn a_child_that_says_done_through_its_lifecycle_never_completes_its_node() {
    let a = Arc::new(ScriptedRunner::new("a"));
    let lab = Lab::new(
        vec![instance("profile-a", "harness-a")],
        &[("profile-a", Arc::clone(&a))],
    );
    let graph = ProbeGraph::default();
    let broker = DelegationBroker::new(&graph, &lab.registry, &lab.runners);
    let root = broker
        .delegate(root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)))
        .expect("the root launches")
        .node;
    a.set_status(&inv(ROOT_TAG), running_status());

    // The child claims completion through forensics only.
    graph
        .record_observation(
            HarnessObservation {
                invocation_id: inv(ROOT_TAG),
                native_session: root.native_session.clone(),
                state: LifecycleState::Done,
                source: ObservationSource::Structured,
                observed_at: at(),
                sequence: ObservationSequence::GauntletAssigned,
            },
            at(),
            deadline(),
        )
        .expect("the observation is recorded");

    for _ in 0..2 {
        assert!(matches!(
            broker
                .collect(&inv(ROOT_TAG), at(), deadline())
                .expect("the runner is polled"),
            CollectOutcome::StillRunning(_)
        ));
    }
    let node = graph
        .inner
        .node(&inv(ROOT_TAG), deadline())
        .expect("the graph reads")
        .expect("the node exists");
    assert_eq!(node.status, NodeStatus::Running);
    assert!(node.result_digest.is_none());
    assert_eq!(graph.completes.get(), 0);
    assert_eq!(a.collects(), 0);
}

#[test]
fn a_runner_timeout_times_the_node_out_and_it_is_never_completed() {
    let (lab, a, _, _) = standard_lab();
    lab.broker()
        .delegate(root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)))
        .expect("the root launches");
    a.set_status(&inv(ROOT_TAG), InvocationStatus::TimedOut);

    let outcome = lab
        .broker()
        .collect(&inv(ROOT_TAG), at(), deadline())
        .expect("the runner is polled");
    assert!(matches!(outcome, CollectOutcome::TimedOut(ref node)
        if node.status == NodeStatus::TimedOut));
    assert_eq!(node_status(&lab, ROOT_TAG), NodeStatus::TimedOut);
    assert!(matches!(
        lab.broker()
            .collect(&inv(ROOT_TAG), at(), deadline())
            .expect("the terminal node is read"),
        CollectOutcome::AlreadyTerminal(_)
    ));
}

#[test]
fn a_malformed_result_status_fails_the_node_with_its_class() {
    let (lab, a, _, _) = standard_lab();
    lab.broker()
        .delegate(root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)))
        .expect("the root launches");
    a.set_status(
        &inv(ROOT_TAG),
        InvocationStatus::Failed {
            class: InvocationFailure::MalformedResult,
        },
    );

    let outcome = lab
        .broker()
        .collect(&inv(ROOT_TAG), at(), deadline())
        .expect("the runner is polled");
    assert!(matches!(outcome, CollectOutcome::Failed(ref node)
        if node.status == NodeStatus::Failed(InvocationFailure::MalformedResult)));
    assert_eq!(
        node_status(&lab, ROOT_TAG),
        NodeStatus::Failed(InvocationFailure::MalformedResult)
    );
}

#[test]
fn unavailable_evidence_completes_the_invocation_as_an_untrusted_claim() {
    let (lab, a, _, _) = standard_lab();
    let root = lab
        .broker()
        .delegate(root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)))
        .expect("the root launches")
        .node;
    a.set_status(&inv(ROOT_TAG), completed_status());
    a.set_result(builder_result(&inv(ROOT_TAG), &root.context_digest, true));

    let CollectOutcome::Completed { node, result } = lab
        .broker()
        .collect(&inv(ROOT_TAG), at(), deadline())
        .expect("the runner is polled")
    else {
        panic!("the bound result completes the invocation");
    };
    // The invocation is complete, but the result still declares missing
    // evidence. Nothing here turns that claim into a settled fact.
    assert_eq!(node.status, NodeStatus::Completed);
    assert_eq!(result.unavailable_evidence().len(), 1);
}

#[test]
fn the_reconciliation_source_has_no_settlement_or_verification_path() {
    // Completion != settlement. The broker may take the content digest from
    // the verify module, the hashing rule, and nothing else from it.
    for (name, source) in [
        ("broker.rs", include_str!("broker.rs")),
        ("reconcile.rs", include_str!("reconcile.rs")),
    ] {
        for banned in [
            "crate::settle",
            "crate::compare",
            "crate::observation",
            "crate::promote",
        ] {
            assert!(!source.contains(banned), "{name} reaches {banned}");
        }
        assert_eq!(
            source.matches("crate::verify::").count(),
            source.matches("crate::verify::content_digest").count(),
            "{name} reaches past the content digest in the verify module"
        );
    }
}

#[test]
fn every_reconcile_call_refuses_an_unknown_invocation_with_a_typed_error() {
    let (lab, _, _, _) = standard_lab();
    let missing = inv(UNKNOWN_TAG);
    assert_eq!(
        lab.broker()
            .status(&missing, deadline())
            .expect_err("the invocation is unknown"),
        BrokerError::UnknownInvocation {
            id: missing.clone(),
        }
    );
    assert_eq!(
        lab.broker()
            .collect(&missing, at(), deadline())
            .expect_err("the invocation is unknown"),
        BrokerError::UnknownInvocation {
            id: missing.clone(),
        }
    );
    assert_eq!(
        lab.broker()
            .cancel(&missing, cancel_reason(OPERATOR_REASON), at(), deadline())
            .expect_err("the invocation is unknown"),
        BrokerError::Graph(GraphError::UnknownInvocation {
            id: missing.clone(),
        })
    );
}

// ---------------------------------------------------------------------------
// status: polls only live, bound nodes
// ---------------------------------------------------------------------------

#[test]
fn status_polls_only_live_bound_nodes_and_never_moves_the_graph() {
    let (lab, a, _, _) = standard_lab();
    let root = lab
        .broker()
        .delegate(root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)))
        .expect("the root launches")
        .node;
    a.set_status(&inv(ROOT_TAG), running_status());

    let live = lab
        .broker()
        .status(&inv(ROOT_TAG), deadline())
        .expect("the status reads");
    assert_eq!(live.runner_status, Some(running_status()));
    assert_eq!(live.node.status, NodeStatus::Running);
    assert_eq!(a.polls(), 1);

    a.set_status(&inv(ROOT_TAG), completed_status());
    a.set_result(builder_result(&inv(ROOT_TAG), &root.context_digest, false));
    lab.broker()
        .collect(&inv(ROOT_TAG), at(), deadline())
        .expect("the runner is polled");
    let settled = lab
        .broker()
        .status(&inv(ROOT_TAG), deadline())
        .expect("the status reads");
    assert_eq!(settled.node.status, NodeStatus::Completed);
    assert_eq!(settled.runner_status, None);
    assert_eq!(settled.lifecycle.effective, None);
    assert_eq!(a.polls(), 2);
}

// ---------------------------------------------------------------------------
// cancel: graph first, runners leaf-first, idempotent
// ---------------------------------------------------------------------------

#[test]
fn cancel_reaches_the_leaves_first_and_a_second_cancel_is_a_no_op() {
    let (lab, a, b, c) = standard_lab();
    let log: CancelLog = Arc::default();
    for double in [&a, &b, &c] {
        double.share_cancel_log(&log);
    }
    lab.broker()
        .delegate(pinned(
            root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)),
            "profile-a",
        ))
        .expect("the root launches");
    lab.broker()
        .delegate(pinned(
            request(
                CHILD_TAG,
                Some(ROOT_TAG),
                KEY_CHILD,
                items_for(CHILD_MARKER),
            ),
            "profile-b",
        ))
        .expect("the child launches");
    let mut grandchild = pinned(
        request(
            GRANDCHILD_TAG,
            Some(CHILD_TAG),
            KEY_GRAND,
            items_for(GRAND_MARKER),
        ),
        "profile-c",
    );
    grandchild.budget = grand_slice();
    lab.broker()
        .delegate(grandchild)
        .expect("the grandchild launches");

    let report = lab
        .broker()
        .cancel(
            &inv(ROOT_TAG),
            cancel_reason(OPERATOR_REASON),
            at(),
            deadline(),
        )
        .expect("the subtree cancels");
    assert_eq!(
        report.cancelled,
        vec![inv(ROOT_TAG), inv(CHILD_TAG), inv(GRANDCHILD_TAG)]
    );
    assert!(report.runner_errors.is_empty());
    assert_eq!(
        *log.borrow(),
        vec![inv(GRANDCHILD_TAG), inv(CHILD_TAG), inv(ROOT_TAG)]
    );
    for tag in [ROOT_TAG, CHILD_TAG, GRANDCHILD_TAG] {
        assert_eq!(node_status(&lab, tag), NodeStatus::Cancelled);
    }

    let second = lab
        .broker()
        .cancel(
            &inv(ROOT_TAG),
            cancel_reason(OPERATOR_REASON),
            at(),
            deadline(),
        )
        .expect("the second cancel reads");
    assert!(second.cancelled.is_empty());
    assert!(second.runner_errors.is_empty());
    assert_eq!(log.borrow().len(), 3);
}

#[test]
fn a_completed_child_is_not_cancelled_and_its_runner_is_not_called() {
    let (lab, a, b, c) = standard_lab();
    lab.broker()
        .delegate(pinned(
            root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)),
            "profile-a",
        ))
        .expect("the root launches");
    let child = lab
        .broker()
        .delegate(pinned(
            request(
                CHILD_TAG,
                Some(ROOT_TAG),
                KEY_CHILD,
                items_for(CHILD_MARKER),
            ),
            "profile-b",
        ))
        .expect("the child launches")
        .node;
    lab.broker()
        .delegate(pinned(
            request(
                SECOND_CHILD_TAG,
                Some(ROOT_TAG),
                KEY_SECOND,
                items_for(GRAND_MARKER),
            ),
            "profile-c",
        ))
        .expect("the sibling launches");

    b.set_status(&inv(CHILD_TAG), completed_status());
    b.set_result(builder_result(
        &inv(CHILD_TAG),
        &child.context_digest,
        false,
    ));
    assert!(matches!(
        lab.broker()
            .collect(&inv(CHILD_TAG), at(), deadline())
            .expect("the runner is polled"),
        CollectOutcome::Completed { .. }
    ));

    let report = lab
        .broker()
        .cancel(
            &inv(ROOT_TAG),
            cancel_reason(OPERATOR_REASON),
            at(),
            deadline(),
        )
        .expect("the subtree cancels");
    assert_eq!(report.cancelled, vec![inv(ROOT_TAG), inv(SECOND_CHILD_TAG)]);
    assert_eq!(b.cancels(), 0);
    assert_eq!(c.cancels(), 1);
    assert_eq!(node_status(&lab, CHILD_TAG), NodeStatus::Completed);
    assert_eq!(node_status(&lab, SECOND_CHILD_TAG), NodeStatus::Cancelled);
    assert_eq!(a.cancels(), 1);
}

#[test]
fn a_runner_cancel_failure_is_reported_and_the_graph_stays_cancelled() {
    let (lab, a, b, _) = standard_lab();
    b.fail_cancels();
    lab.broker()
        .delegate(pinned(
            root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)),
            "profile-a",
        ))
        .expect("the root launches");
    lab.broker()
        .delegate(pinned(
            request(
                CHILD_TAG,
                Some(ROOT_TAG),
                KEY_CHILD,
                items_for(CHILD_MARKER),
            ),
            "profile-b",
        ))
        .expect("the child launches");

    let report = lab
        .broker()
        .cancel(
            &inv(ROOT_TAG),
            cancel_reason(OPERATOR_REASON),
            at(),
            deadline(),
        )
        .expect("the subtree cancels despite the runner failure");
    assert_eq!(report.cancelled, vec![inv(ROOT_TAG), inv(CHILD_TAG)]);
    assert_eq!(
        report.runner_errors,
        vec![(
            inv(CHILD_TAG),
            AgentError::CancelFailed {
                invocation_id: inv(CHILD_TAG),
                detail: "scripted cancel failure".to_owned(),
            },
        )]
    );
    assert_eq!(node_status(&lab, CHILD_TAG), NodeStatus::Cancelled);
    assert_eq!(a.cancels(), 1);
}

// ---------------------------------------------------------------------------
// recover: reconstruct from the graph alone
// ---------------------------------------------------------------------------

/// A three-node tree (root, child, grandchild) and a second child whose launch
/// failed, so it is admitted with no binding.
fn recovery_tree() -> (
    Lab,
    Arc<ScriptedRunner>,
    Arc<ScriptedRunner>,
    Arc<ScriptedRunner>,
) {
    let (lab, a, b, c) = standard_lab();
    lab.broker()
        .delegate(pinned(
            root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)),
            "profile-a",
        ))
        .expect("the root launches");
    lab.broker()
        .delegate(pinned(
            request(
                CHILD_TAG,
                Some(ROOT_TAG),
                KEY_CHILD,
                items_for(CHILD_MARKER),
            ),
            "profile-b",
        ))
        .expect("the child launches");
    let mut grandchild = pinned(
        request(
            GRANDCHILD_TAG,
            Some(CHILD_TAG),
            KEY_GRAND,
            items_for(GRAND_MARKER),
        ),
        "profile-c",
    );
    grandchild.budget = grand_slice();
    lab.broker()
        .delegate(grandchild)
        .expect("the grandchild launches");
    c.fail_next_invokes(1);
    assert!(matches!(
        lab.broker().delegate(pinned(
            request(
                SECOND_CHILD_TAG,
                Some(ROOT_TAG),
                KEY_SECOND,
                items_for(GRAND_MARKER),
            ),
            "profile-c",
        )),
        Err(DelegationRefusal::RunnerAdmission(_))
    ));
    (lab, a, b, c)
}

#[test]
fn recovery_reattaches_orphans_cancels_running_descendants_and_reports_never_launched() {
    let (lab, _, _, _) = recovery_tree();

    // A restart: fresh doubles over the same graph. The first still holds the
    // root. The second forgot the child. The third still holds the grandchild.
    let restart_a = Arc::new(ScriptedRunner::new("a4"));
    let restart_b = Arc::new(ScriptedRunner::new("b4"));
    let restart_c = Arc::new(ScriptedRunner::new("c4"));
    restart_a.set_status(&inv(ROOT_TAG), running_status());
    restart_c.set_status(&inv(GRANDCHILD_TAG), running_status());
    let restarted = Lab::new(
        vec![
            instance("profile-a", "harness-a"),
            instance("profile-b", "harness-b"),
            instance("profile-c", "harness-c"),
        ],
        &[
            ("profile-a", Arc::clone(&restart_a)),
            ("profile-b", Arc::clone(&restart_b)),
            ("profile-c", Arc::clone(&restart_c)),
        ],
    );
    let broker = DelegationBroker::new(&lab.graph, &restarted.registry, &restarted.runners);

    let report = broker
        .recover(&run_id(), at(), deadline())
        .expect("the run recovers");
    assert_eq!(
        report.actions,
        vec![
            (inv(ROOT_TAG), RecoveryAction::Reattached),
            (inv(CHILD_TAG), RecoveryAction::Orphaned),
            (inv(SECOND_CHILD_TAG), RecoveryAction::NeverLaunched),
        ]
    );
    assert!(report.unreconciled.is_empty());
    assert_eq!(report.cancelled_descendants, vec![inv(GRANDCHILD_TAG)]);
    assert!(report.cascade_errors.is_empty());
    assert_eq!(
        node_status(&lab, CHILD_TAG),
        NodeStatus::Failed(InvocationFailure::OrphanedByRestart)
    );
    assert_eq!(node_status(&lab, GRANDCHILD_TAG), NodeStatus::Cancelled);
    assert_eq!(node_status(&lab, ROOT_TAG), NodeStatus::Running);
    assert_eq!(restart_c.cancels(), 1);

    // The second recovery sees only the live root.
    let again = broker
        .recover(&run_id(), at(), deadline())
        .expect("the run recovers again");
    assert_eq!(
        again.actions,
        vec![(inv(ROOT_TAG), RecoveryAction::Reattached)]
    );
    assert!(again.cancelled_descendants.is_empty());
    assert_eq!(restart_c.cancels(), 1);
}

#[test]
fn recovery_applies_a_terminal_runner_status_once() {
    let (lab, _, _, _) = standard_lab();
    let root = lab
        .broker()
        .delegate(pinned(
            root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)),
            "profile-a",
        ))
        .expect("the root launches")
        .node;
    let restart_a = Arc::new(ScriptedRunner::new("a5"));
    restart_a.set_status(&inv(ROOT_TAG), completed_status());
    restart_a.set_result(builder_result(&inv(ROOT_TAG), &root.context_digest, false));
    let restarted = Lab::new(
        vec![instance("profile-a", "harness-a")],
        &[("profile-a", Arc::clone(&restart_a))],
    );
    let broker = DelegationBroker::new(&lab.graph, &restarted.registry, &restarted.runners);

    let first = broker
        .recover(&run_id(), at(), deadline())
        .expect("the run recovers");
    assert_eq!(
        first.actions,
        vec![(
            inv(ROOT_TAG),
            RecoveryAction::Settled(NodeStatus::Completed)
        )]
    );
    assert!(
        lab.node(ROOT_TAG)
            .expect("the root node")
            .result_digest
            .is_some()
    );

    let second = broker
        .recover(&run_id(), at(), deadline())
        .expect("the run recovers again");
    assert!(second.actions.is_empty());
    assert_eq!(restart_a.collects(), 1);
}

#[test]
fn a_runner_that_cannot_answer_leaves_its_node_unchanged_and_the_next_recovery_retries_it() {
    let (lab, a, b, c) = standard_lab();
    lab.broker()
        .delegate(pinned(
            root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)),
            "profile-a",
        ))
        .expect("the root launches");
    lab.broker()
        .delegate(pinned(
            request(
                CHILD_TAG,
                Some(ROOT_TAG),
                KEY_CHILD,
                items_for(CHILD_MARKER),
            ),
            "profile-b",
        ))
        .expect("the child launches");
    lab.broker()
        .delegate(pinned(
            request(
                SECOND_CHILD_TAG,
                Some(ROOT_TAG),
                KEY_SECOND,
                items_for(GRAND_MARKER),
            ),
            "profile-c",
        ))
        .expect("the sibling launches");
    a.set_status(&inv(ROOT_TAG), running_status());
    b.set_status(&inv(CHILD_TAG), running_status());
    c.set_status(&inv(SECOND_CHILD_TAG), running_status());

    b.set_transport_down(true);
    let broker = lab.broker();
    let first = broker
        .recover(&run_id(), at(), deadline())
        .expect("the run recovers");
    assert_eq!(
        first.actions,
        vec![
            (inv(ROOT_TAG), RecoveryAction::Reattached),
            (inv(SECOND_CHILD_TAG), RecoveryAction::Reattached),
        ]
    );
    assert_eq!(first.unreconciled.len(), 1);
    assert_eq!(first.unreconciled[0].id, inv(CHILD_TAG));
    assert!(first.unreconciled[0].detail.contains("transport"));
    assert_eq!(node_status(&lab, CHILD_TAG), NodeStatus::Running);

    b.set_transport_down(false);
    let second = broker
        .recover(&run_id(), at(), deadline())
        .expect("the run recovers again");
    assert_eq!(
        second.actions,
        vec![
            (inv(ROOT_TAG), RecoveryAction::Reattached),
            (inv(CHILD_TAG), RecoveryAction::Reattached),
            (inv(SECOND_CHILD_TAG), RecoveryAction::Reattached),
        ]
    );
    assert!(second.unreconciled.is_empty());
}

#[test]
fn a_failed_child_cascades_cancellation_to_its_running_grandchild_through_collect() {
    let (lab, _, b, c) = standard_lab();
    lab.broker()
        .delegate(pinned(
            root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)),
            "profile-a",
        ))
        .expect("the root launches");
    lab.broker()
        .delegate(pinned(
            request(
                CHILD_TAG,
                Some(ROOT_TAG),
                KEY_CHILD,
                items_for(CHILD_MARKER),
            ),
            "profile-b",
        ))
        .expect("the child launches");
    let mut grandchild = pinned(
        request(
            GRANDCHILD_TAG,
            Some(CHILD_TAG),
            KEY_GRAND,
            items_for(GRAND_MARKER),
        ),
        "profile-c",
    );
    grandchild.budget = grand_slice();
    lab.broker()
        .delegate(grandchild)
        .expect("the grandchild launches");

    b.set_status(
        &inv(CHILD_TAG),
        InvocationStatus::Failed {
            class: InvocationFailure::Execution,
        },
    );
    let outcome = lab
        .broker()
        .collect(&inv(CHILD_TAG), at(), deadline())
        .expect("the runner is polled");
    assert!(matches!(outcome, CollectOutcome::Failed(ref node)
        if node.status == NodeStatus::Failed(InvocationFailure::Execution)));
    assert_eq!(node_status(&lab, GRANDCHILD_TAG), NodeStatus::Cancelled);
    assert_eq!(c.cancels(), 1);
    assert_eq!(node_status(&lab, ROOT_TAG), NodeStatus::Running);
}

#[test]
fn a_cascade_leaves_a_completed_grandchild_untouched() {
    let (lab, _, b, c) = standard_lab();
    lab.broker()
        .delegate(pinned(
            root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)),
            "profile-a",
        ))
        .expect("the root launches");
    lab.broker()
        .delegate(pinned(
            request(
                CHILD_TAG,
                Some(ROOT_TAG),
                KEY_CHILD,
                items_for(CHILD_MARKER),
            ),
            "profile-b",
        ))
        .expect("the child launches");
    let mut done_request = pinned(
        request(
            SECOND_CHILD_TAG,
            Some(CHILD_TAG),
            KEY_SECOND,
            items_for(GRAND_MARKER),
        ),
        "profile-c",
    );
    done_request.budget = grand_slice();
    let done = lab
        .broker()
        .delegate(done_request)
        .expect("the completed grandchild launches")
        .node;
    let mut running_request = pinned(
        request(
            GRANDCHILD_TAG,
            Some(CHILD_TAG),
            KEY_GRAND,
            items_for(CHILD_MARKER),
        ),
        "profile-c",
    );
    running_request.budget = grand_slice();
    lab.broker()
        .delegate(running_request)
        .expect("the running grandchild launches");

    c.set_status(&inv(SECOND_CHILD_TAG), completed_status());
    c.set_result(builder_result(
        &inv(SECOND_CHILD_TAG),
        &done.context_digest,
        false,
    ));
    assert!(matches!(
        lab.broker()
            .collect(&inv(SECOND_CHILD_TAG), at(), deadline())
            .expect("the runner is polled"),
        CollectOutcome::Completed { .. }
    ));
    c.set_status(&inv(GRANDCHILD_TAG), running_status());

    b.set_status(
        &inv(CHILD_TAG),
        InvocationStatus::Failed {
            class: InvocationFailure::Execution,
        },
    );
    lab.broker()
        .collect(&inv(CHILD_TAG), at(), deadline())
        .expect("the runner is polled");
    assert_eq!(node_status(&lab, SECOND_CHILD_TAG), NodeStatus::Completed);
    assert_eq!(node_status(&lab, GRANDCHILD_TAG), NodeStatus::Cancelled);
    assert_eq!(c.cancels(), 1);
}

#[test]
fn a_completed_node_records_the_digest_of_the_canonical_encoding_of_its_result() {
    let (lab, a, _, _) = standard_lab();
    let root = lab
        .broker()
        .delegate(pinned(
            root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)),
            "profile-a",
        ))
        .expect("the root launches")
        .node;
    a.set_status(&inv(ROOT_TAG), completed_status());
    let result = builder_result(&inv(ROOT_TAG), &root.context_digest, false);
    a.set_result(result.clone());

    let CollectOutcome::Completed { node, .. } = lab
        .broker()
        .collect(&inv(ROOT_TAG), at(), deadline())
        .expect("the runner is polled")
    else {
        panic!("the bound result completes the invocation");
    };
    let expected = crate::verify::content_digest(encode_role_result(&result).as_bytes());
    assert_eq!(node.result_digest, Some(expected.clone()));

    // Equal results digest equal; a one-field change digests differently.
    let same = builder_result(&inv(ROOT_TAG), &root.context_digest, false);
    let changed = builder_result(&inv(ROOT_TAG), &root.context_digest, true);
    assert_eq!(
        crate::verify::content_digest(encode_role_result(&same).as_bytes()),
        expected
    );
    assert_ne!(
        crate::verify::content_digest(encode_role_result(&changed).as_bytes()),
        expected
    );
}

// ---------------------------------------------------------------------------
// Exactly once under retries
// ---------------------------------------------------------------------------

#[test]
fn exactly_once_under_retries_with_a_failed_invoke_completes_once() {
    let (lab, _, b, _) = standard_lab();
    lab.broker()
        .delegate(pinned(
            root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)),
            "profile-a",
        ))
        .expect("the root launches");
    let child_request = pinned(
        request(
            CHILD_TAG,
            Some(ROOT_TAG),
            KEY_CHILD,
            items_for(CHILD_MARKER),
        ),
        "profile-b",
    );

    b.fail_next_invokes(1);
    assert!(matches!(
        lab.broker().delegate(child_request.clone()),
        Err(DelegationRefusal::RunnerAdmission(
            AgentError::Transport { .. }
        ))
    ));
    let launched = lab
        .broker()
        .delegate(child_request.clone())
        .expect("the retry with the same key launches");
    assert_eq!(launched.outcome, DelegationOutcome::Launched);
    let replay = lab
        .broker()
        .delegate(child_request.clone())
        .expect("the replay is not a refusal");
    assert_eq!(replay.outcome, DelegationOutcome::AlreadyLaunched);
    let again = lab
        .broker()
        .delegate(child_request)
        .expect("the second replay is not a refusal");
    assert_eq!(again.outcome, DelegationOutcome::AlreadyLaunched);

    // One logical child, one successful invoke (the other invoke failed).
    assert_eq!(lab.descendant_count(ROOT_TAG), 1);
    assert_eq!(b.invokes(), 2);

    b.set_status(&inv(CHILD_TAG), completed_status());
    b.set_result(builder_result(
        &inv(CHILD_TAG),
        &launched.node.context_digest,
        false,
    ));
    assert!(matches!(
        lab.broker()
            .collect(&inv(CHILD_TAG), at(), deadline())
            .expect("the runner is polled"),
        CollectOutcome::Completed { .. }
    ));
    assert!(matches!(
        lab.broker()
            .collect(&inv(CHILD_TAG), at(), deadline())
            .expect("the terminal node is read"),
        CollectOutcome::AlreadyTerminal(_)
    ));
    assert_eq!(b.collects(), 1);
    assert_eq!(node_status(&lab, CHILD_TAG), NodeStatus::Completed);
}
