//! Domain layer (spec §4.1): `Reference` · `AnswerKey`/`Unit` · `Run` ·
//! `Iteration` · `Observation` · `Comparison`/`Residual` · `Verification` ·
//! `Settlement` · `Budget` · `Authority` · `CapabilityLedger` ·
//! `CandidateRevision`.
//!
//! Typed value objects, aggregates, and constructor-level invariants only.
//! Depends on no other crate: framework- and infrastructure-independent by
//! construction (spec §24.3), enforced by `just boundaries`.

mod budget;
mod capability;
mod comparison;
mod correction;
mod iteration;
mod observation;
mod reference;
mod role_invocation;
mod run;
mod settlement;
mod unit;
mod values;
mod verification;
mod wire;

#[cfg(test)]
mod test_sources;

pub use budget::{
    Budget, BudgetDimension, BudgetError, DiagnosticAllowance, Limits, MicroUsd, Spent,
};
pub use capability::{
    CandidateRevision, CandidateRevisionError, Capability, CapabilityError, CapabilityStatus,
    RevisionDecision,
};
pub use comparison::{ComparisonError, ComparisonRecord, Residual};
pub use correction::{
    BudgetFacts, CorrectionDescriptor, CorrectionError, CorrectionSelection, CorrectionShape,
    DecisionStep, RejectedAlternative, RejectionClass, RejectionReason, SelectionDimension,
};
pub use iteration::{Iteration, IterationError};
pub use observation::{ObservationBundle, ObservationFailure, UnavailableEvidence};
pub use reference::{
    Assumption, NarrativeField, QualityDimension, QualityDimensionDef, Reference,
    ReferenceDefinition, ReferenceError, ReferenceVersion, SettlementCriteria, Unknown,
    hard_gate_aggregate,
};
pub use role_invocation::{Payment, RoleInvocation, RoleInvocationDefinition, RoleInvocationError};
pub use run::{Run, RunDefinition, RunError};
pub use settlement::{EventLogOffset, SettlementRecord};
pub use unit::{
    Actor, DraftField, DraftUnit, EvidenceRequirement, TierAxis, Unit, UnitError, authority_rank,
    blast_rank,
};
pub use values::{
    AdmissionKey, Authority, BlastRadius, CapabilityId, CustomRoleName, Digest, DimensionId,
    EpochSeconds, EvidenceKind, EvidenceLocator, EvidenceRef, HarnessId, InstrumentName,
    InvocationId, InvocationState, IterationNo, ModelIdentity, Outcome, PermissionSetRef,
    ReferenceId, ResidualId, ResultContractPath, ReviewMode, RevisionId, RoleId, RunId,
    RunnerIdentity, RunnerProfileId, SandboxPolicyRef, SessionRef, Severity, TerminalState, UnitId,
    UnitStatus, ValueError, ValueKind, Verdict, VerificationRef, VerifyBy, WorkspaceRef,
};
pub use verification::VerificationRecord;
// Producer belongs to the Unit aggregate's evidence contract (spec §5.3.2),
// declared next to it.
pub use unit::Producer;
