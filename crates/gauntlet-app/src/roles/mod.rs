//! Roles as repository artifacts — spec §10.1, plan Task 17, AK-063.
//!
//! The pipeline: a role FILE (`.agents/roles/<role>.md`, §10.1 front
//! matter + prose contract) → the typed [`RoleDefinition`] (validated
//! construction over the EXISTING config grammar and $VAR machinery —
//! one parse site, no forked rule class) → [`install`], which
//! materializes the files VERBATIM under the configured state root for
//! a harness's role surface and writes `.gauntlet/roles.lock` →
//! [`verify`], which re-reads the installed state and the sources and
//! compares CONTENT digests against the lock, reporting drift as a
//! typed configuration failure.
//!
//! The invariants this module owns:
//!
//! - **Intersection, not union** (the plan's exact words): a role file
//!   may NARROW what the envelope builder permits, never GRANT beyond
//!   it. An allow naming a §13.2 envelope-denied category is refused at
//!   construction — fail closed, never a silent narrowing. The typed
//!   deny-set ([`RoleDefinition::context_deny_set`]) and the effective
//!   envelope grant ([`RoleDefinition::envelope_grant`]) are the
//!   consume-side hook the envelope builder consumes when composed
//!   dispatch lands (plan Task 20); dispatch itself does not exist in
//!   this task.
//! - **Fail closed at the seams**: unknown front-matter keys, unknown
//!   context categories, deny shadowing allow, an unregistered or unset
//!   harness id, a write path escaping the roots, a hand-edited or
//!   re-forged lock — every one is a typed error at the boundary where
//!   it would otherwise silently weaken a constraint.
//! - **The installer writes only verbatim bytes, only under the
//!   state root**; resolved $VAR values are validated for presence and
//!   never serialized, printed, or logged (spec §8.1).
//! - **The lock is runtime-owned state** (the frozen preregistration's
//!   D2), not a frozen reference: it gates nothing but the install it
//!   describes, and verify treats it as CLAIMS to be checked against
//!   reality, never as reality itself.

pub mod category;
pub mod definition;
pub mod harness;
pub mod install;

pub use category::{CategoryClass, ContextCategory, ContextDenySet};
pub use definition::{FilesystemMode, NetworkMode, RoleDefinition, RoleError, RolePermissions};
pub use harness::{HarnessRegistry, RegistryError, RoleSurface};
pub use install::{
    DriftKind, DriftRecord, Enforcement, InstallInput, InstallReport, LockEntry, RolesError,
    RolesLock, VerifyInput, VerifyReport, install, verify,
};
