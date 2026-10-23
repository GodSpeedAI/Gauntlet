//! The lifecycle-sequencing proof (E3): acceptance tests for the supervisor in
//! `lifecycle`, run against the in-memory reference graph and scripted runner
//! doubles. Each test's doc comment names the invariant it pins.

use super::test_support::*;
use std::sync::Arc;

use gauntlet_domain::{EpochSeconds, SessionRef};
use gauntlet_ports::invocation_graph::{InvocationGraph, NativeSessionBinding, NodeStatus};
use gauntlet_ports::lifecycle::{
    HarnessLifecycleSample, HarnessObservation, LifecycleState, ObservationOutcome,
    ObservationSequence, ObservationSource,
};

use super::lifecycle::LifecycleSupervisor;
use super::reconcile::CollectOutcome;

/// Observation instants, named so the literal scan reads them as data.
const SEEN_A: u64 = 10;
const SEEN_B: u64 = 11;
const SEEN_C: u64 = 12;
const SEEN_D: u64 = 20;
const SEEN_E: u64 = 21;
const SEEN_F: u64 = 30;
/// The second Gauntlet-assigned number of a key.
const SECOND: u64 = 2;

/// A runner-side reading of `state` from `source`, observed at `observed`, on
/// `session`.
fn reading(
    state: LifecycleState,
    source: ObservationSource,
    observed: u64,
    session: &SessionRef,
) -> HarnessLifecycleSample {
    HarnessLifecycleSample {
        state,
        source,
        observed_at: EpochSeconds::new(observed),
        native_session: Some(session.clone()),
    }
}

/// One root node, admitted and running on a single bound session.
fn root_lab() -> (Lab, Arc<ScriptedRunner>) {
    let a = Arc::new(ScriptedRunner::new("a"));
    let lab = Lab::new(
        vec![instance("profile-a", "harness-a")],
        &[("profile-a", Arc::clone(&a))],
    );
    lab.broker()
        .delegate(root_request(ROOT_TAG, KEY_ROOT, items_for(ROOT_MARKER)))
        .expect("the root launches");
    a.set_status(&inv(ROOT_TAG), running_status());
    (lab, a)
}

fn bound(lab: &Lab) -> NativeSessionBinding {
    lab.node(ROOT_TAG)
        .expect("the node exists")
        .native_session
        .expect("the root is bound")
}

fn bound_session(lab: &Lab) -> SessionRef {
    bound(lab).session
}

/// The Gauntlet-assigned numbers of the recorded history for one source.
fn recorded_numbers(lab: &Lab, source: ObservationSource) -> Vec<u64> {
    lab.graph
        .observation_history(&inv(ROOT_TAG), deadline())
        .expect("history reads")
        .iter()
        .filter(|record| record.observation.source == source)
        .filter_map(|record| match record.observation.sequence {
            ObservationSequence::SourceProvided(number) => Some(number),
            ObservationSequence::GauntletAssigned => None,
        })
        .collect()
}

/// Pins: each (invocation, epoch, source) key numbers its applied readings
/// 1, 2, 3, … with no gaps and no repeats, and the view takes the entry of the
/// highest-trust source.
#[test]
fn lifecycle_sequencing_numbers_each_source_strictly_increasing() {
    let (lab, _a) = root_lab();
    let session = bound_session(&lab);
    let supervisor = LifecycleSupervisor::new(&lab.graph);
    for (state, at) in [
        (LifecycleState::Working, SEEN_A),
        (LifecycleState::Idle, SEEN_B),
        (LifecycleState::Blocked, SEEN_C),
    ] {
        let outcome = supervisor
            .receive(
                &inv(ROOT_TAG),
                reading(state, ObservationSource::Screen, at, &session),
                deadline(),
            )
            .expect("the reading is recorded");
        assert!(matches!(outcome, ObservationOutcome::Applied { .. }));
    }
    supervisor
        .receive(
            &inv(ROOT_TAG),
            reading(
                LifecycleState::Done,
                ObservationSource::Structured,
                SEEN_C,
                &session,
            ),
            deadline(),
        )
        .expect("the reading is recorded");

    assert_eq!(
        recorded_numbers(&lab, ObservationSource::Screen),
        vec![1, 2, 3]
    );
    assert_eq!(
        recorded_numbers(&lab, ObservationSource::Structured),
        vec![1]
    );
    let view = lab
        .graph
        .lifecycle(&inv(ROOT_TAG), deadline())
        .expect("view");
    let effective = view.effective.expect("readings were applied");
    assert_eq!(effective.source, ObservationSource::Structured);
    assert_eq!(effective.state, LifecycleState::Done);
}

/// Pins: a reading observed before the applied view of its key cannot roll
/// that view back, whatever order it arrives in. It is kept as history only.
#[test]
fn lifecycle_sequencing_a_late_reading_cannot_roll_the_view_back() {
    let (lab, _a) = root_lab();
    let session = bound_session(&lab);
    let supervisor = LifecycleSupervisor::new(&lab.graph);
    supervisor
        .receive(
            &inv(ROOT_TAG),
            reading(
                LifecycleState::Working,
                ObservationSource::Screen,
                SEEN_D,
                &session,
            ),
            deadline(),
        )
        .expect("recorded");

    let late = supervisor
        .receive(
            &inv(ROOT_TAG),
            reading(
                LifecycleState::Idle,
                ObservationSource::Screen,
                SEEN_A,
                &session,
            ),
            deadline(),
        )
        .expect("recorded");

    assert_eq!(late, ObservationOutcome::IgnoredStale { last_applied: 1 });
    let view = lab
        .graph
        .lifecycle(&inv(ROOT_TAG), deadline())
        .expect("view");
    assert_eq!(
        view.effective.expect("the first reading stands").state,
        LifecycleState::Working
    );
    assert_eq!(
        lab.graph
            .observation_history(&inv(ROOT_TAG), deadline())
            .expect("history")
            .len(),
        SECOND as usize,
        "the late reading is kept as history"
    );
}

/// Pins: an older stamp that races a newer applied reading cannot overwrite it.
/// The graph compares the number atomically, so the older stamp is refused as
/// stale. This simulates the race at the graph, because a real interleaving
/// cannot be scripted from the supervisor.
#[test]
fn lifecycle_sequencing_a_racing_older_stamp_cannot_overwrite_a_newer_reading() {
    let (lab, _a) = root_lab();
    let binding = bound(&lab);
    let observe = |state, number, at| HarnessObservation {
        invocation_id: inv(ROOT_TAG),
        native_session: Some(binding.clone()),
        state,
        source: ObservationSource::Screen,
        observed_at: EpochSeconds::new(at),
        sequence: ObservationSequence::SourceProvided(number),
    };
    lab.graph
        .record_observation(
            observe(LifecycleState::Working, 1, SEEN_A),
            EpochSeconds::new(SEEN_A),
            deadline(),
        )
        .expect("recorded");
    // The newer stamp is applied first.
    assert!(matches!(
        lab.graph
            .record_observation(
                observe(LifecycleState::Blocked, SECOND, SEEN_B),
                EpochSeconds::new(SEEN_B),
                deadline()
            )
            .expect("recorded"),
        ObservationOutcome::Applied {
            sequence: SECOND,
            ..
        }
    ));
    // The older stamp arrives second and must not overwrite it.
    let racing = lab
        .graph
        .record_observation(
            observe(LifecycleState::Idle, 1, SEEN_C),
            EpochSeconds::new(SEEN_C),
            deadline(),
        )
        .expect("recorded");

    assert_eq!(
        racing,
        ObservationOutcome::IgnoredStale {
            last_applied: SECOND
        }
    );
    let view = lab
        .graph
        .lifecycle(&inv(ROOT_TAG), deadline())
        .expect("view");
    assert_eq!(
        view.effective.expect("applied").state,
        LifecycleState::Blocked
    );
}

/// Pins: after a restart (a fresh supervisor with no carried state) numbering
/// resumes from the persisted history, so no sequence number is reused for a
/// key.
#[test]
fn lifecycle_sequencing_resumes_from_the_persisted_high_water_after_restart() {
    let (lab, _a) = root_lab();
    let session = bound_session(&lab);
    {
        let before_restart = LifecycleSupervisor::new(&lab.graph);
        before_restart
            .receive(
                &inv(ROOT_TAG),
                reading(
                    LifecycleState::Working,
                    ObservationSource::Screen,
                    SEEN_A,
                    &session,
                ),
                deadline(),
            )
            .expect("recorded");
        before_restart
            .receive(
                &inv(ROOT_TAG),
                reading(
                    LifecycleState::Idle,
                    ObservationSource::Screen,
                    SEEN_B,
                    &session,
                ),
                deadline(),
            )
            .expect("recorded");
    }

    let after_restart = LifecycleSupervisor::new(&lab.graph);
    let outcome = after_restart
        .receive(
            &inv(ROOT_TAG),
            reading(
                LifecycleState::Blocked,
                ObservationSource::Screen,
                SEEN_C,
                &session,
            ),
            deadline(),
        )
        .expect("recorded");

    assert_eq!(
        outcome,
        ObservationOutcome::Applied {
            sequence: 3,
            previous: Some(LifecycleState::Idle),
        }
    );
    assert_eq!(
        recorded_numbers(&lab, ObservationSource::Screen),
        vec![1, 2, 3]
    );
}

/// Pins: after the native session is replaced (epoch bump), a reading from the
/// old session never mutates the current view. It is kept as history. The new
/// epoch's view starts empty and takes only its own session's readings.
#[test]
fn lifecycle_sequencing_a_wrong_session_reading_after_epoch_bump_never_mutates_state() {
    let (lab, _a) = root_lab();
    let old = bound_session(&lab);
    let supervisor = LifecycleSupervisor::new(&lab.graph);
    supervisor
        .receive(
            &inv(ROOT_TAG),
            reading(
                LifecycleState::Working,
                ObservationSource::Screen,
                SEEN_A,
                &old,
            ),
            deadline(),
        )
        .expect("recorded");

    let replacement = SessionRef::new("session-replacement").expect("valid session ref");
    lab.graph
        .attach_native_session(&inv(ROOT_TAG), replacement.clone(), deadline())
        .expect("the live node rebinds");
    assert_eq!(bound(&lab).epoch, 2);

    let stale = supervisor
        .receive(
            &inv(ROOT_TAG),
            reading(
                LifecycleState::Idle,
                ObservationSource::Screen,
                SEEN_D,
                &old,
            ),
            deadline(),
        )
        .expect("recorded");
    assert!(matches!(
        stale,
        ObservationOutcome::RejectedStaleSession { .. }
    ));
    let view = lab
        .graph
        .lifecycle(&inv(ROOT_TAG), deadline())
        .expect("view");
    assert_eq!(
        view.effective, None,
        "the old session cannot touch the new epoch"
    );

    let fresh = supervisor
        .receive(
            &inv(ROOT_TAG),
            reading(
                LifecycleState::Blocked,
                ObservationSource::Screen,
                SEEN_E,
                &replacement,
            ),
            deadline(),
        )
        .expect("recorded");
    assert_eq!(
        fresh,
        ObservationOutcome::Applied {
            sequence: SECOND,
            previous: None,
        },
        "the rejected reading consumed number 1, so the new epoch resumes at 2"
    );
    let view = lab
        .graph
        .lifecycle(&inv(ROOT_TAG), deadline())
        .expect("view");
    assert_eq!(
        view.effective.expect("applied").state,
        LifecycleState::Blocked
    );
}

/// Pins: a reading after the invocation is terminal changes neither its status
/// nor its lifecycle view. The reading is kept as history with the terminal
/// outcome.
#[test]
fn lifecycle_sequencing_a_reading_after_terminal_changes_neither_status_nor_view() {
    let (lab, _a) = root_lab();
    let session = bound_session(&lab);
    lab.graph
        .time_out(&inv(ROOT_TAG), at(), deadline())
        .expect("the node times out");

    let outcome = LifecycleSupervisor::new(&lab.graph)
        .receive(
            &inv(ROOT_TAG),
            reading(
                LifecycleState::Working,
                ObservationSource::Screen,
                SEEN_F,
                &session,
            ),
            deadline(),
        )
        .expect("recorded");

    assert_eq!(outcome, ObservationOutcome::IgnoredTerminal);
    assert_eq!(node_status(&lab, ROOT_TAG), NodeStatus::TimedOut);
    let view = lab
        .graph
        .lifecycle(&inv(ROOT_TAG), deadline())
        .expect("view");
    assert_eq!(view.effective, None);
}

/// Pins: a harness `done` reading, even a structured one, never completes a
/// node without a bound, valid result contract. The node stays Running and
/// collect keeps reporting it open.
#[test]
fn harness_lifecycle_done_without_a_valid_contract_never_completes() {
    let (lab, a) = root_lab();
    let session = bound_session(&lab);
    LifecycleSupervisor::new(&lab.graph)
        .receive(
            &inv(ROOT_TAG),
            reading(
                LifecycleState::Done,
                ObservationSource::Structured,
                SEEN_A,
                &session,
            ),
            deadline(),
        )
        .expect("recorded");

    for _ in 0..2 {
        assert!(matches!(
            lab.broker()
                .collect(&inv(ROOT_TAG), at(), deadline())
                .expect("the runner is polled"),
            CollectOutcome::StillRunning(_)
        ));
    }
    let node = lab.node(ROOT_TAG).expect("the node exists");
    assert_eq!(node.status, NodeStatus::Running);
    assert!(node.result_digest.is_none());
    assert_eq!(a.collects(), 0);
    let view = lab
        .graph
        .lifecycle(&inv(ROOT_TAG), deadline())
        .expect("view");
    assert_eq!(
        view.effective.expect("the done reading is recorded").state,
        LifecycleState::Done,
        "the reading is recorded yet still never completes the node"
    );
}

/// Pins: blocked and idle readings from any source neither fail nor complete a
/// node. Status and collect keep it Running.
#[test]
fn harness_lifecycle_blocked_and_idle_neither_fail_nor_complete() {
    let (lab, _a) = root_lab();
    let session = bound_session(&lab);
    let supervisor = LifecycleSupervisor::new(&lab.graph);
    for (state, source) in [
        (LifecycleState::Blocked, ObservationSource::Screen),
        (LifecycleState::Idle, ObservationSource::Process),
        (LifecycleState::Idle, ObservationSource::Structured),
    ] {
        supervisor
            .receive(
                &inv(ROOT_TAG),
                reading(state, source, SEEN_A, &session),
                deadline(),
            )
            .expect("recorded");
    }

    let status = lab
        .broker()
        .status(&inv(ROOT_TAG), deadline())
        .expect("status reads");
    assert_eq!(status.node.status, NodeStatus::Running);
    assert!(matches!(
        lab.broker()
            .collect(&inv(ROOT_TAG), at(), deadline())
            .expect("the runner is polled"),
        CollectOutcome::StillRunning(_)
    ));
    assert_eq!(node_status(&lab, ROOT_TAG), NodeStatus::Running);
}

/// Pins: collect decides identically whatever lifecycle history the node holds,
/// including `done`. Every history, and the empty history, yields the same
/// still-running outcome, and the same contract-backed completion.
#[test]
fn harness_lifecycle_history_never_changes_the_collect_decision() {
    let histories: [&[(LifecycleState, ObservationSource)]; 3] = [
        &[],
        &[
            (LifecycleState::Done, ObservationSource::Structured),
            (LifecycleState::Blocked, ObservationSource::Screen),
        ],
        &[
            (LifecycleState::Idle, ObservationSource::Process),
            (LifecycleState::Done, ObservationSource::Process),
        ],
    ];
    let mut running = Vec::new();
    let mut completed = Vec::new();
    for history in histories {
        let (lab, a) = root_lab();
        let session = bound_session(&lab);
        let supervisor = LifecycleSupervisor::new(&lab.graph);
        for (at_secs, (state, source)) in history.iter().enumerate() {
            supervisor
                .receive(
                    &inv(ROOT_TAG),
                    reading(*state, *source, SEEN_A + at_secs as u64, &session),
                    deadline(),
                )
                .expect("recorded");
        }
        running.push(
            lab.broker()
                .collect(&inv(ROOT_TAG), at(), deadline())
                .expect("the runner is polled"),
        );

        let root = lab.node(ROOT_TAG).expect("the node exists");
        a.set_status(&inv(ROOT_TAG), completed_status());
        a.set_result(builder_result(&inv(ROOT_TAG), &root.context_digest, false));
        completed.push(
            lab.broker()
                .collect(&inv(ROOT_TAG), at(), deadline())
                .expect("the runner is polled"),
        );
    }
    assert!(running.windows(2).all(|pair| pair[0] == pair[1]));
    assert!(completed.windows(2).all(|pair| pair[0] == pair[1]));
    assert!(matches!(completed[0], CollectOutcome::Completed { .. }));
}

/// Pins: the broker's status feeds the runner's readings through the
/// supervisor, reports the view as forensics only, and leaves node status to
/// the graph.
#[test]
fn status_records_runner_readings_and_reports_them_as_forensics_only() {
    let (lab, a) = root_lab();
    let session = bound_session(&lab);
    a.push_sample(
        &inv(ROOT_TAG),
        reading(
            LifecycleState::Done,
            ObservationSource::Structured,
            SEEN_A,
            &session,
        ),
    );

    let status = lab
        .broker()
        .status(&inv(ROOT_TAG), deadline())
        .expect("status reads");

    assert_eq!(status.node.status, NodeStatus::Running);
    assert_eq!(
        status
            .forensic_lifecycle
            .effective
            .expect("the reading is recorded")
            .state,
        LifecycleState::Done
    );
    assert_eq!(node_status(&lab, ROOT_TAG), NodeStatus::Running);
    assert_eq!(a.collects(), 0);
}

/// Pins: each runner reading is recorded once. A second status poll does not
/// replay the same reading.
#[test]
fn status_drains_each_runner_reading_exactly_once() {
    let (lab, a) = root_lab();
    let session = bound_session(&lab);
    a.push_sample(
        &inv(ROOT_TAG),
        reading(
            LifecycleState::Idle,
            ObservationSource::Screen,
            SEEN_A,
            &session,
        ),
    );

    lab.broker()
        .status(&inv(ROOT_TAG), deadline())
        .expect("status reads");
    lab.broker()
        .status(&inv(ROOT_TAG), deadline())
        .expect("status reads");

    assert_eq!(
        lab.graph
            .observation_history(&inv(ROOT_TAG), deadline())
            .expect("history")
            .len(),
        1
    );
    assert!(matches!(
        lab.graph
            .lifecycle(&inv(ROOT_TAG), deadline())
            .expect("view")
            .effective,
        Some(ref entry) if entry.sequence == 1
    ));
}
