//! Ports layer (spec §4.1, §6): the stable semantic capabilities core logic
//! depends on — `AgentRunner`, `Observer`, `Comparator`, `Verifier`,
//! `ReferenceProvider`, `PolicyProvider`, `StateStore`, `EvidenceStore`,
//! `EventLog`, `ToolProvider`, `Sandbox`, `WorkspaceProvider`, `BudgetMeter`,
//! `Clock`, `IdGenerator`, `HumanEscalation`, `OperatorSurface`.
//!
//! Ports name capabilities, never products. Adapters implement them; the
//! application layer consumes them; neither direction may invert (spec
//! §24.3).
//!
//! Spec §6, quoted where it shapes this crate:
//!
//! - "Signatures are pseudo-typed and language-agnostic. Every port returns
//!   a typed error enum (§21.2), never a string. Every port that can block
//!   has a deadline parameter. **No port method takes or returns a
//!   provider-specific type.**"
//! - §7.3: "Each port has a **shared contract test suite** that every
//!   adapter for that port MUST pass. Adding an adapter means implementing
//!   the trait and running the existing suite, not writing a bespoke one."
//!   The suites live in [`contract`], parameterized over adapter factories.
//!   Suite code is test-support by design: adapter TEST targets consume it;
//!   the run/assert machinery (`stubs`, `suite_runs`) is `#[cfg(test)]`
//!   gated so no stub ships in the library binary.
//! - §10.3: every role invocation declares a result contract; the
//!   `role_result@v1` envelope in [`role_result`] is its typed, strictly
//!   parsed form.
//!
//! This crate depends on `gauntlet-domain` only. No external dependencies:
//! the JSON reader behind the result contract is hand-rolled in [`json`].

pub mod agent_runner;
pub mod budget_meter;
pub mod clock;
pub mod comparator;
pub mod comparison_record;
pub mod contract;
pub mod deadline;
pub mod event_log;
pub mod evidence_store;
pub mod human_escalation;
pub mod id_generator;
pub mod json;
pub mod observation_digest;
pub mod observer;
pub mod observer_evidence;
pub mod operator_surface;
pub mod policy_provider;
pub mod reference_provider;
pub mod role_result;
pub mod sandbox;
pub mod state_store;
pub mod tokens;
pub mod tool_provider;
pub mod verification_record;
pub mod verifier;
pub mod workspace_provider;

pub use agent_runner::{
    AgentError, AgentRunner, CancelReason, CancelReasonInvalid, ContextEnvelope, InvocationFailure,
    InvocationHandle, InvocationStatus, Isolation, ResultDelivery, RoleInvocationSpec,
    RunnerCapabilities,
};
pub use budget_meter::{BudgetMeter, BudgetMeterError, BudgetSlice, Remaining, Reservation};
pub use clock::{Clock, ClockError};
pub use comparator::{
    AdversarialComparator, AdversarialComparatorSource, ApplicableConstraint, Comparator,
    ComparatorError, ComparatorSource, ComparisonEnvelope, ContractView, FacetSummary, FailureGap,
    FrozenReferenceProjection, MandatoryComparisonAcceptance, ObservationView, RubricEntry,
    RunComparisonAuthority, TrustedComparisonContext, UnavailableGap,
};
pub use comparison_record::{
    COMPARISON_RECORD_SCHEMA, ComparatorIdentity, ComparisonDocError, ComparisonRecordDoc,
    HardGateResult, RecordContent, RecordVerdict, ResidualDoc,
};
pub use contract::{ContractFinding, SuiteSetupError};
pub use deadline::Deadline;
pub use event_log::{EventLog, EventLogError, LogEntry};
pub use evidence_store::{EvidenceStore, EvidenceStoreError};
pub use human_escalation::{
    EscalationItem, EscalationState, EscalationTicket, HumanEscalation, HumanEscalationError,
    TicketId,
};
pub use id_generator::IdGenerator;
pub use json::{JsonError, JsonNumber, JsonValue};
pub use observer::{
    FacetKind, InstrumentIdentity, ObservationFacet, ObservationRequest, Observer, ObserverError,
};
pub use observer_evidence::{
    FACET_EVIDENCE_SCHEMA, FacetEvidenceDoc, FacetEvidenceError, ObservedRun, command_line,
};
pub use operator_surface::{
    OperatorAction, OperatorSurface, OperatorSurfaceError, RuntimeSnapshot, UnitSnapshot,
};
pub use policy_provider::{
    CredentialScope, Enforcement, PermissionSet, PolicyProvider, PolicyProviderError,
};
pub use reference_provider::{ReferenceDraft, ReferenceProvider, ReferenceProviderError};
pub use role_result::{
    ArtifactClaim, BUILDER_RESULT_SCHEMA, BuilderResultPayload, ROLE_RESULT_SCHEMA, ResultPayment,
    RolePayload, RoleResult, RoleResultError, RoleResultState, SelfCheck, UnavailableEvidenceEntry,
};
pub use sandbox::{
    NetworkPosture, PolicyValidation, Sandbox, SandboxBackendRef, SandboxError, SandboxPolicy,
    SandboxedCommand,
};
pub use state_store::{
    Checkpoint, EvidenceProduction, EvidenceProductionBinding, RunEvent, RunEventKind, StateStore,
    StateStoreError,
};
pub use tokens::UnknownToken;
pub use tool_provider::{
    SemanticCapability, ToolBinding, ToolIdentity, ToolProvider, ToolProviderError,
};
pub use verification_record::{
    BindingClass, RequirementDoc, VERIFICATION_RECORD_SCHEMA, VerificationContent,
    VerificationDocError, VerificationRecordDoc, VerifyingInstrument,
};
pub use verifier::{VerificationRequest, Verifier, VerifierError};
pub use workspace_provider::{
    Workspace, WorkspaceHook, WorkspaceProvider, WorkspaceProviderError, WorkspaceSubject,
};
