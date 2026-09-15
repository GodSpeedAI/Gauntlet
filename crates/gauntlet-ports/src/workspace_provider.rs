//! `WorkspaceProvider` — spec §6.2: "`create(run_id, unit_or_slice) ->
//! Workspace` · `cleanup` · `hooks` — Optional worktree backing.
//! Path-containment invariants in §17.5."
//!
//! §17.5 invariants this port's types carry: workspaces live under the
//! configured root (the provider enforces containment after
//! canonicalization — symlink escape is checked, not assumed); workspace
//! keys are sanitized to `[A-Za-z0-9._-]`; and units dispatched in parallel
//! never share a workspace (§10.4 — `create` is per unit-or-slice, so a
//! shared workspace is not reachable through this API).

use core::fmt;
use std::path::PathBuf;

use gauntlet_domain::{RunId, UnitId, WorkspaceRef};

use crate::deadline::Deadline;

/// `WorkspaceProvider` failures — typed (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceProviderError {
    /// Creating the workspace failed.
    CreateFailed { detail: String },
    /// A path escaped the workspace root after canonicalization (§17.5.2:
    /// "symlink escape is checked, not assumed absent").
    ContainmentViolated { path: String },
    /// Cleanup failed.
    CleanupFailed { detail: String },
    /// Promotion failed — including promoting an unknown or already-cleaned
    /// workspace, which is a typed refusal rather than a silent no-op.
    PromotionFailed { detail: String },
}

impl fmt::Display for WorkspaceProviderError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::CreateFailed { detail } => {
                write!(f, "creating the workspace failed: {detail}")
            }
            Self::ContainmentViolated { path } => write!(
                f,
                "workspace path {path} escapes the workspace root (spec 17.5.2)"
            ),
            Self::CleanupFailed { detail } => {
                write!(f, "cleaning the workspace failed: {detail}")
            }
            Self::PromotionFailed { detail } => {
                write!(f, "promoting the workspace failed: {detail}")
            }
        }
    }
}

impl std::error::Error for WorkspaceProviderError {}

/// One file changed by `promote` (spec: W5 work-product promotion).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PromotedFile {
    /// Path relative to the workspace/integration tree root.
    pub path: String,
    /// The sha256 hex digest of the promoted content.
    pub sha256: String,
}

/// The result of [`WorkspaceProvider::promote`]. When `supported` is
/// `false` (the default implementation), every list is empty and no
/// promotion happened — callers must treat that as "nothing to report",
/// never as "nothing changed".
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct WorkspacePromotion {
    /// Whether this provider implements promotion at all.
    pub supported: bool,
    /// Paths written into the integration tree (added or changed).
    pub changed: Vec<PromotedFile>,
    /// Paths removed from the integration tree.
    pub deleted: Vec<String>,
    /// Paths whose integration-tree content diverged from the workspace's
    /// seed (someone else changed it since this workspace was seeded) —
    /// left untouched in the integration tree, not applied.
    pub conflicts: Vec<String>,
}

/// What a workspace is created for: one unit, or one slice of units
/// (spec §11.3 slicing: a slice inherits the highest blast radius and most
/// restrictive authority of its members, and never mixes irreversible
/// members with anything).
#[derive(Debug, Clone, PartialEq)]
pub enum WorkspaceSubject {
    Unit(UnitId),
    Slice(Vec<UnitId>),
}

/// An allocated workspace: its ref, its on-disk root (under the configured
/// workspace root — adapters derive it from configuration, never literals),
/// and its sanitized key (§17.5.3).
#[derive(Debug, Clone, PartialEq)]
pub struct Workspace {
    pub reference: WorkspaceRef,
    pub root: PathBuf,
    pub key: String,
}

impl Workspace {
    /// Whether the key conforms to the §17.5.3 sanitization class.
    pub fn key_is_sanitized(&self) -> bool {
        !self.key.is_empty()
            && self
                .key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'.' || b == b'_' || b == b'-')
    }
}

/// A workspace hook: trusted configuration run inside the workspace with a
/// REQUIRED span bound (spec §22.7: "Hooks are trusted configuration, run
/// inside the workspace with a REQUIRED timeout and truncated output in
/// logs") — the bound is a required field, not an option.
#[derive(Debug, Clone, PartialEq)]
pub struct WorkspaceHook {
    pub name: String,
    pub command: String,
    /// The maximum wall-clock seconds the hook may run for. Required.
    pub max_wall_seconds: u64,
}

/// The workspace port (spec §6.2). Creation and cleanup perform filesystem
/// IO, so they carry deadlines; `hooks` is a pure accessor over resolved
/// configuration.
pub trait WorkspaceProvider {
    /// Creates an isolated workspace for one run and one unit or slice
    /// (§6.2 `create(run_id, unit_or_slice)`).
    fn create(
        &self,
        run_id: &RunId,
        subject: WorkspaceSubject,
        deadline: Deadline,
    ) -> Result<Workspace, WorkspaceProviderError>;

    /// Removes a workspace (§6.2 `cleanup`).
    fn cleanup(
        &self,
        workspace: &WorkspaceRef,
        deadline: Deadline,
    ) -> Result<(), WorkspaceProviderError>;

    /// The repo-configured workspace hooks (§6.2 `hooks`).
    fn hooks(&self) -> Vec<WorkspaceHook>;

    /// Promotes a workspace's settled changes into the run's integration
    /// tree (W5 work-product carry-forward). The default implementation
    /// reports `supported: false` so every existing double compiles
    /// unchanged and callers can skip promotion cleanly when a provider
    /// does not implement it.
    fn promote(
        &self,
        #[allow(unused_variables)] workspace: &WorkspaceRef,
        #[allow(unused_variables)] deadline: Deadline,
    ) -> Result<WorkspacePromotion, WorkspaceProviderError> {
        Ok(WorkspacePromotion::default())
    }
}
