//! Observation (spec §5.3.6, §12; Annex B.3; invariants I2/I8; plan
//! Task 9): the observation bundle assembled from instrument-produced
//! facets, with explicit gaps and a claims facet that is excluded from
//! verification inputs BY CONSTRUCTION.
//!
//! - **§12.1 (I2):** comparison operates on evidence collected from
//!   resulting reality, never on the builder's completion claim. Here
//!   that is a TYPE property: [`bundle::ClaimsFacet`] is a distinct type
//!   with no conversion into [`bundle::FacetRecord`], and
//!   [`input::VerificationInputs::of_bundle`] reads the instrument-facet
//!   array and the gap arrays ONLY — there is no code path from claims
//!   content into verification inputs and no runtime `if kind == claims`
//!   anywhere (the exclusion is the absence of a path).
//! - **§12.3 (I8):** `unavailable_evidence` and `observation_failures`
//!   are REQUIRED arrays — populated `Vec` types on
//!   [`bundle::ObservationBundleDoc`], never `Option`; an empty array is
//!   a positive statement that nothing failed. Observer failures become
//!   BOTH entries at exactly ONE site ([`assembly`]).
//! - **§12.4:** [`digest::observation_digest`] covers the normalized
//!   bundle minus timestamps (`observed_at`, per-facet `started_at` /
//!   `ended_at`), so the same observation re-taken at a different time
//!   digests identically. Claims ARE covered by the digest (forensic
//!   tamper-evidence) and are NOT verification input (I2) — two different
//!   concerns, both true.
//! - **Annex B.3:** [`bundle::ObservationBundleDoc`] is the typed mirror
//!   of `observation_bundle@v1`, decoded strictly (unknown keys
//!   rejected), mirroring the `role_result` conventions.
//!
//! The bundle-level `environment_fingerprint` (B.3) is supplied by the
//! caller at assembly ([`assembly::ObservationPlan`]); each facet's own
//! environment fingerprint lives inside its content-addressed
//! `facet_evidence@v1` evidence document (see
//! `gauntlet_ports::observer_evidence` for the §12.2-vs-B.3
//! reconciliation). `project_revision` is likewise caller-supplied — the
//! runtime knows the revision it dispatched against; the versioning facet
//! records repository state, not the revision contract (a plan Task 12
//! wiring point).

pub mod assembly;
pub mod bundle;
pub mod digest;
pub mod input;

pub use assembly::{AssembledBundle, AssemblyError, ObservationAssembler, ObservationPlan};
pub use bundle::{
    BundleContent, ClaimsFacet, FacetRecord, OBSERVATION_BUNDLE_SCHEMA, ObservationBundleDoc,
    ObservationDocError, ObservationFailureRecord, UnavailableEvidenceRecord,
};
pub use digest::{normalized_bytes, observation_digest};
pub use input::{InstrumentFacets, VerificationInputs};
