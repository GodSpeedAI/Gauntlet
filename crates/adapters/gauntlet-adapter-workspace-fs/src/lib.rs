//! Workspace adapter (spec §4.1, §6.2; plan Task 20 / R-1b part a): the
//! production filesystem-backed [`WorkspaceProvider`] — per-unit workspaces
//! under a configured root, path-traversal-refusing (§17.5), with per-run
//! isolation and §17.5.3-sanitized keys.
//!
//! Layout under the configured root (the composition passes e.g.
//! `<state_dir>/workspaces`; paths are DERIVED from the run id and subject,
//! never chosen by callers):
//!
//! ```text
//! <root>/<run>.<unit>.<seq>      one workspace directory per create
//! ```
//!
//! The key is derived and sanitized to `[A-Za-z0-9._-]` in that order, so a
//! key is always a single path component: no caller- or domain-supplied
//! string can carry a separator or a `..` segment into a join (the roles
//! installer's `contained_join` discipline, applied at the derive step).
//! Containment is then CHECKED, not assumed (§17.5.2): after
//! canonicalization the workspace must still sit under the configured root,
//! so a symlink planted over a workspace path is a typed refusal rather than
//! an escape.

mod containment;
mod json;
mod provider;
mod seed;

pub use json::parse_manifest_json;
pub use provider::FsWorkspaceProvider;
pub use seed::{
    CopyOutcome, FileDigest, Manifest, WorkspaceSeed, compute_manifest, digest_file,
    manifest_to_json, write_file_atomic,
};
