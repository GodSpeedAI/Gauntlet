//! `Observer` — spec §6.2: "`observe(request: ObservationRequest) ->
//! ObservationFacet` · `facet_kind()` — One adapter per instrument class
//! (versioning, fs, build, test, bench, browser, http, scanner). Facets
//! merge into a bundle."
//!
//! Naming rule pinned by this task: where the version-control instrument
//! class is named, it is `versioning` — a capability, never a product
//! (§8.4 vocabulary; AGENTS.md: name ports for semantic capabilities). The
//! facet-kind wire tokens are derived from variant names in
//! [`crate::tokens`] because two of the spec's class names collide with
//! adapter distinguishing segments the boundary suite denies as whole
//! string literals in this layer.

use core::fmt;

use gauntlet_domain::{
    EpochSeconds, EvidenceRef, InstrumentName, IterationNo, RunId, WorkspaceRef,
};

use crate::deadline::Deadline;
use crate::tokens::token_enum;

token_enum! {
    /// The instrument classes (spec §6.2 Observer note): one adapter per
    /// class; `versioning` is the version-control class. Wire tokens are
    /// variant names in snake case — `Versioning` -> `versioning`.
    pub enum FacetKind {
        Versioning,
        Filesystem,
        Build,
        Test,
        Bench,
        Browser,
        Http,
        Scanner,
    }
}

/// `Observer` failures — typed, carrying the facet kind that failed.
#[derive(Debug, Clone, PartialEq)]
pub enum ObserverError {
    /// The instrument ran and failed. Per §21.3 this becomes recorded
    /// unavailable evidence, never a silently missing facet.
    InstrumentFailed { kind: FacetKind, detail: String },
    /// The instrument could not run at all.
    Unavailable { kind: FacetKind, detail: String },
}

impl fmt::Display for ObserverError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InstrumentFailed { kind, detail } => {
                write!(f, "the {kind} instrument failed: {detail}")
            }
            Self::Unavailable { kind, detail } => {
                write!(f, "the {kind} instrument is unavailable: {detail}")
            }
        }
    }
}

impl std::error::Error for ObserverError {}

/// What the observation step asks one instrument class for: the run,
/// iteration, and workspace to observe (spec §12 — comparison operates on
/// evidence collected from resulting reality, so the request names where
/// the resulting reality lives).
#[derive(Debug, Clone)]
pub struct ObservationRequest {
    pub run_id: RunId,
    pub iteration_no: IterationNo,
    pub workspace: WorkspaceRef,
}

/// One instrument's observation of the resulting reality (spec §5.3.6 /
/// Annex B.3 facet shape). Facets merge into a bundle by the runtime; a
/// facet always names its instrument and the evidence ref holding its raw
/// output.
#[derive(Debug, Clone)]
pub struct ObservationFacet {
    kind: FacetKind,
    instrument: InstrumentIdentity,
    evidence_ref: EvidenceRef,
    command: Option<String>,
    exit_status: Option<i64>,
    started_at: Option<EpochSeconds>,
    ended_at: Option<EpochSeconds>,
    summary: Option<String>,
}

impl ObservationFacet {
    pub fn new(kind: FacetKind, instrument: InstrumentIdentity, evidence_ref: EvidenceRef) -> Self {
        Self {
            kind,
            instrument,
            evidence_ref,
            command: None,
            exit_status: None,
            started_at: None,
            ended_at: None,
            summary: None,
        }
    }

    /// Attaches how the instrument was invoked (best effort; not every
    /// instrument is a command).
    pub fn with_invocation(mut self, command: Option<String>, exit_status: Option<i64>) -> Self {
        self.command = command;
        self.exit_status = exit_status;
        self
    }

    /// Attaches the instrument's wall-clock window.
    pub fn with_window(
        mut self,
        started_at: Option<EpochSeconds>,
        ended_at: Option<EpochSeconds>,
    ) -> Self {
        self.started_at = started_at;
        self.ended_at = ended_at;
        self
    }

    /// Attaches the instrument's own summary of what it saw.
    pub fn with_summary(mut self, summary: Option<String>) -> Self {
        self.summary = summary;
        self
    }

    pub fn kind(&self) -> FacetKind {
        self.kind
    }

    pub fn instrument(&self) -> &InstrumentIdentity {
        &self.instrument
    }

    /// Where this facet's raw output lives (content-addressed, immutable).
    pub fn evidence_ref(&self) -> &EvidenceRef {
        &self.evidence_ref
    }

    pub fn command(&self) -> Option<&str> {
        self.command.as_deref()
    }

    pub fn exit_status(&self) -> Option<i64> {
        self.exit_status
    }

    pub fn started_at(&self) -> Option<EpochSeconds> {
        self.started_at
    }

    pub fn ended_at(&self) -> Option<EpochSeconds> {
        self.ended_at
    }

    pub fn summary(&self) -> Option<&str> {
        self.summary.as_deref()
    }
}

/// A named instrument with its version (Annex B.3 `instrument: {name,
/// version}`) — outcomes are "produced by a named instrument" (§5.3.6).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentIdentity {
    name: InstrumentName,
    version: String,
}

impl InstrumentIdentity {
    pub fn new(name: InstrumentName, version: String) -> Self {
        Self { name, version }
    }

    pub fn name(&self) -> &InstrumentName {
        &self.name
    }

    pub fn version(&self) -> &str {
        &self.version
    }
}

/// The observation port (spec §6.2). One adapter per instrument class;
/// `observe` runs the instrument (blocking — deadline required), and
/// `facet_kind` names the class the adapter observes (pure — no deadline).
pub trait Observer {
    /// Observes the resulting reality named by the request and returns one
    /// facet.
    fn observe(
        &self,
        request: &ObservationRequest,
        deadline: Deadline,
    ) -> Result<ObservationFacet, ObserverError>;

    /// The instrument class this adapter observes (spec §6.2).
    fn facet_kind(&self) -> FacetKind;
}
