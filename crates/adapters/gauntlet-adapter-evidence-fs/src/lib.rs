//! Evidence adapter (spec §4.1, §18.2; plan Task 6): a content-addressed,
//! write-once `EvidenceStore` over the filesystem, with per-run pinning and
//! no overwrite path in the API at all.
//!
//! Layout under the store root (paths are derived from the content digest,
//! never chosen by callers):
//!
//! ```text
//! objects/<b0-2>/<b2-4>/<64-hex>       the bytes, written once, fsynced, renamed
//! objects/<b0-2>/<b2-4>/<64-hex>.kind  the EvidenceKind wire token, write-once
//! pins/<run-id>/<evidence-ref>         empty marker file per (run, ref) pin
//! ```
//!
//! Refs are `sha256:<hex>`: the ref a `put` returns is a pure function of the
//! bytes, so "same bytes ⇒ same ref" is the only reachable outcome and an
//! overwrite is unrepresentable through the port. The write path still
//! verifies a colliding path byte-for-byte — a mismatch is a typed error, not
//! an overwrite — because content addressing is this adapter's policy, not a
//! law of the filesystem.

mod store;

pub use store::FsEvidenceStore;
