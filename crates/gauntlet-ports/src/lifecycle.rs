//! Harness lifecycle observations (E3a): liveness and forensics only.
//!
//! A harness reports a lifecycle state (working, blocked, idle, done,
//! unknown) about the native session it runs. These reports are evidence
//! about liveness. They are NEVER node status: recording one never changes
//! an invocation's status, never completes or fails it, and never touches
//! its budget. Completion and failure remain owned by the `InvocationGraph`
//! transitions and their result bindings.
//!
//! Each observation is judged against the node's CURRENT native-session
//! binding (session ref and epoch). An observation for an older or newer
//! binding is rejected, and a rejected or ignored observation never alters
//! the lifecycle view.
//!
//! Sequencing is per `(invocation, epoch, source)`:
//!
//! - a [`ObservationSequence::SourceProvided`] number applies only when it is
//!   strictly greater than the last applied number of its key; otherwise it
//!   is ignored as stale. Numbers are compared as given, so a late, lower
//!   number from the same source cannot roll the view backward;
//! - a [`ObservationSequence::GauntletAssigned`] sequence is `last applied + 1`
//!   (the first is 1), assigned atomically and always applied. It establishes
//!   receipt order at the Gauntlet observation boundary for that key only. It
//!   does NOT establish the order in which the remote harness produced the
//!   observations, and it says nothing about ordering across sources.
//!
//! Sequences restart at each epoch; entries of a superseded epoch remain in
//! the forensic history but are not part of the current view.
//!
//! The effective lifecycle of a node is the entry of the highest-trust source
//! that has an applied observation under the current epoch. Trust rank is
//! Structured > Process > Screen, and each source has at most one entry per
//! epoch, so the effective entry is always unique.

use core::fmt;

use gauntlet_domain::{EpochSeconds, InvocationId, SessionRef};

use crate::invocation_graph::NativeSessionBinding;

/// The lifecycle state a harness reports. Advisory: see the module docs.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LifecycleState {
    Working,
    Blocked,
    Idle,
    Done,
    Unknown,
}

impl LifecycleState {
    /// The stable lowercase token for this state.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Working => "working",
            Self::Blocked => "blocked",
            Self::Idle => "idle",
            Self::Done => "done",
            Self::Unknown => "unknown",
        }
    }
}

impl fmt::Display for LifecycleState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Where a lifecycle observation came from. The declaration order IS the
/// trust rank, lowest first: `Screen < Process < Structured`. A higher rank
/// wins the effective lifecycle when sources disagree.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum ObservationSource {
    /// Read from the rendered screen of the harness. Least trusted.
    Screen,
    /// Inferred from the harness process. Middle trust.
    Process,
    /// Reported by the harness through its structured interface. Most trusted.
    Structured,
}

impl ObservationSource {
    /// The stable lowercase token for this source.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Screen => "screen",
            Self::Process => "process",
            Self::Structured => "structured",
        }
    }
}

impl fmt::Display for ObservationSource {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Who established an observation's sequence number.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SequenceOrigin {
    /// The source supplied the number; Gauntlet only compared it.
    SourceProvided,
    /// Gauntlet assigned the number at receipt, per (invocation, epoch, source).
    GauntletAssigned,
}

/// The sequence an observation arrives with.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ObservationSequence {
    /// A number the source supplied. Applies only when greater than the last
    /// applied number of its (invocation, epoch, source) key. Sequences start
    /// at 1, so `SourceProvided(0)` never applies.
    SourceProvided(u64),
    /// Gauntlet assigns the next number of the key when the observation is
    /// applied. This is receipt order at the Gauntlet boundary only.
    GauntletAssigned,
}

/// One lifecycle observation as received.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessObservation {
    pub invocation_id: InvocationId,
    /// The binding the observer saw. `None` is valid only before any session
    /// is attached to the node.
    pub native_session: Option<NativeSessionBinding>,
    pub state: LifecycleState,
    pub source: ObservationSource,
    /// When the observed state held, as reported by the observer.
    pub observed_at: EpochSeconds,
    pub sequence: ObservationSequence,
}

/// A raw lifecycle reading as a runner saw it, before Gauntlet sequences it.
///
/// This type deliberately has NO sequence: a runner or harness cannot supply
/// one. The supervising app layer receives the sample, stamps it with a
/// Gauntlet-assigned sequence at receipt, binds it to the node's current
/// session and epoch, and only then records it as a [`HarnessObservation`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HarnessLifecycleSample {
    /// The state the runner read.
    pub state: LifecycleState,
    /// Where the runner read it.
    pub source: ObservationSource,
    /// When the runner read it, from its injected clock.
    pub observed_at: EpochSeconds,
    /// The native session the runner read the state from, when it has one.
    pub native_session: Option<SessionRef>,
}

/// The result of recording one observation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ObservationOutcome {
    /// The observation became the latest entry of its key.
    /// `sequence` is the number used (assigned, for `GauntletAssigned`).
    /// `previous` is the state of the same key's entry before this one, or
    /// `None` when the key had no applied observation.
    Applied {
        sequence: u64,
        previous: Option<LifecycleState>,
    },
    /// A source-provided number was not greater than `last_applied` of its
    /// key. Nothing changed.
    IgnoredStale { last_applied: u64 },
    /// The observation does not name the node's current binding exactly
    /// (session ref and epoch). `current` is the binding the node holds now,
    /// or `None` when no session is attached. Nothing changed.
    RejectedStaleSession {
        current: Option<NativeSessionBinding>,
    },
    /// The node is terminal. Lifecycle observations no longer apply.
    IgnoredTerminal,
}

/// The latest applied observation of one (epoch, source) key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleEntry {
    pub state: LifecycleState,
    pub source: ObservationSource,
    /// The number used for this entry. For a `GauntletAssigned` entry this is
    /// the assigned number; it is the highest applied number of its key.
    pub sequence: u64,
    pub origin: SequenceOrigin,
    pub observed_at: EpochSeconds,
    /// The epoch the entry belongs to. Epoch 0 means no session was attached.
    pub epoch: u32,
}

/// The current lifecycle view of one node.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleView {
    /// The entry of the highest-trust source under the current epoch.
    pub effective: Option<LifecycleEntry>,
    /// One entry per source with an applied observation under the current
    /// epoch, highest trust first.
    pub per_source: Vec<LifecycleEntry>,
}

/// One received observation, kept as forensics whatever its outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObservationRecord {
    /// The observation as received, except that `sequence` is
    /// `SourceProvided(n)` with the number actually used when a sequence was
    /// applied to it. `origin` remains the authority on who assigned it. A
    /// rejected or ignored `GauntletAssigned` observation keeps
    /// `GauntletAssigned` because no number was consumed.
    pub observation: HarnessObservation,
    pub origin: SequenceOrigin,
    pub outcome: ObservationOutcome,
    pub received_at: EpochSeconds,
}
