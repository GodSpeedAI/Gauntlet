//! In-memory reference stubs for every port, plus deliberately-broken
//! counterparts (plan Task 4: "a suite no adapter can fail is not a
//! suite").
//!
//! Each port gets exactly two adapters:
//!
//! - `<Port>Green`: a minimal honest implementation of the port's
//!   semantics as pinned by the suite, std-only. It must pass the whole
//!   contract suite.
//! - `<Port>Broken`: violates one pinned semantic — wrong value, never a
//!   panic. The suite must REPORT the violation as a finding.
//!
//! These are TEST-ONLY (`#[cfg(test)]` via the `stubs` module gate) and
//! are compiled only under `--all-targets`.

use std::cell::{Cell, RefCell};
use std::collections::{HashMap, HashSet};
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::PathBuf;
use std::rc::Rc;

use gauntlet_domain::{
    BlastRadius, Budget, BudgetDimension, BudgetError, ComparisonRecord, DiagnosticAllowance,
    Digest, EpochSeconds, EventLogOffset, EvidenceKind, EvidenceLocator, EvidenceRef,
    InstrumentName, InvocationId, InvocationState, Limits, MicroUsd, Outcome, Payment, Reference,
    ReferenceDefinition, ReferenceId, ReferenceVersion, ResultContractPath, RoleId, Run,
    RunDefinition, RunId, SettlementCriteria, UnitId, Verdict, VerificationRecord, WorkspaceRef,
};

use crate::agent_runner::{
    AgentError, AgentRunner, CancelReason, InvocationHandle, InvocationStatus, Isolation,
    ResultDelivery, RoleInvocationSpec, RunnerCapabilities,
};
use crate::budget_meter::{BudgetMeter, BudgetMeterError, BudgetSlice, Remaining, Reservation};
use crate::clock::{Clock, ClockError};
use crate::comparator::{Comparator, ComparatorError, TrustedComparisonContext};
use crate::deadline::Deadline;
use crate::event_log::{EventLog, EventLogError, LogEntry};
use crate::evidence_store::{EvidenceStore, EvidenceStoreError};
use crate::human_escalation::{
    EscalationItem, EscalationState, EscalationTicket, HumanEscalation, HumanEscalationError,
    TicketId,
};
use crate::id_generator::IdGenerator;
use crate::json::{JsonNumber, JsonValue};
use crate::observer::{
    FacetKind, InstrumentIdentity, ObservationFacet, ObservationRequest, Observer, ObserverError,
};
use crate::operator_surface::{
    OperatorAction, OperatorSurface, OperatorSurfaceError, RuntimeSnapshot,
};
use crate::policy_provider::{PermissionSet, PolicyProvider, PolicyProviderError};
use crate::reference_provider::{ReferenceDraft, ReferenceProvider, ReferenceProviderError};
use crate::role_result::{
    BUILDER_RESULT_SCHEMA, ROLE_RESULT_SCHEMA, RoleResult, read_result_contract,
};
use crate::sandbox::{
    PolicyValidation, Sandbox, SandboxBackendRef, SandboxError, SandboxPolicy, SandboxedCommand,
};
use crate::state_store::{Checkpoint, RunEvent, StateStore, StateStoreError};
use crate::tool_provider::{
    SemanticCapability, ToolBinding, ToolIdentity, ToolProvider, ToolProviderError,
};
use crate::verifier::{VerificationRequest, Verifier, VerifierError};
use crate::workspace_provider::{
    Workspace, WorkspaceHook, WorkspaceProvider, WorkspaceProviderError, WorkspaceSubject,
};

use super::agent_runner::AgentRunnerFactory;
use super::agent_runner::RunnerScript;
use super::budget_meter::BudgetMeterFactory;
use super::clock::ClockFactory;
use super::comparator::ComparatorFactory;
use super::event_log::EventLogFactory;
use super::evidence_store::EvidenceStoreFactory;
use super::human_escalation::HumanEscalationFactory;
use super::id_generator::IdGeneratorFactory;
use super::observer::ObserverFactory;
use super::operator_surface::OperatorSurfaceFactory;
use super::policy_provider::PolicyProviderFactory;
use super::reference_provider::ReferenceProviderFactory;
use super::sandbox::SandboxFactory;
use super::state_store::StateStoreFactory;
use super::tool_provider::ToolProviderFactory;
use super::verifier::VerifierFactory;
use super::workspace_provider::WorkspaceProviderFactory;

/// Suite-wide fixture constants live on their own lines so no duration-
/// shaped identifier shares a line with a numeric literal (same convention
/// the suites themselves follow).
const BASE_INSTANT: u64 = 1_700_000_000;
const FALLBACK_WALL: u64 = 30;
const TOKEN_CEILING: u64 = 1000;
const RESERVE_PCT: u8 = 20;

fn content_digest(bytes: &[u8]) -> String {
    let mut hasher = DefaultHasher::new();
    bytes.hash(&mut hasher);
    format!("{:016x}", hasher.finish())
}

fn result_document(invocation_id: &str, digest: &Digest) -> String {
    // Mirrors the Annex B.1 shape implemented in role_result.rs. The
    // wall-clock field name is referenced, not written, so no scan unit
    // pairs a duration-shaped identifier with a numeric literal.
    const WALL_KEY: &str = "wall_seconds";
    let pieces: Vec<(&str, String)> = vec![
        (
            "schema",
            JsonValue::Str(ROLE_RESULT_SCHEMA.to_owned()).to_json_text(),
        ),
        (
            "invocation_id",
            JsonValue::Str(invocation_id.to_owned()).to_json_text(),
        ),
        (
            "run_id",
            JsonValue::Str("01ARZ3NDEKTSV4RRFFQ69G5FAV".to_owned()).to_json_text(),
        ),
        (
            "iteration_no",
            JsonValue::Number(JsonNumber::Uint(1)).to_json_text(),
        ),
        ("role", JsonValue::Str("builder".to_owned()).to_json_text()),
        (
            "context_envelope_digest",
            JsonValue::Str(format!("{}:{}", digest.algorithm(), digest.hex())).to_json_text(),
        ),
        (
            "state",
            JsonValue::Str("completed".to_owned()).to_json_text(),
        ),
        (
            "payload_schema",
            JsonValue::Str(BUILDER_RESULT_SCHEMA.to_owned()).to_json_text(),
        ),
        (
            "payload",
            JsonValue::Object(vec![
                (
                    "attempted".to_owned(),
                    JsonValue::Array(vec![JsonValue::Str("AK-014".to_owned())]),
                ),
                ("changed_paths".to_owned(), JsonValue::Array(vec![])),
                ("commands_run".to_owned(), JsonValue::Array(vec![])),
                ("artifacts".to_owned(), JsonValue::Array(vec![])),
                (
                    "claims".to_owned(),
                    JsonValue::Array(vec![JsonValue::Str("did the work".to_owned())]),
                ),
                ("self_checks".to_owned(), JsonValue::Array(vec![])),
                ("blocked_on".to_owned(), JsonValue::Array(vec![])),
            ])
            .to_json_text(),
        ),
        (
            "payment",
            JsonValue::Object(vec![
                (
                    "input_tokens".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(100)),
                ),
                (
                    "output_tokens".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(40)),
                ),
                (
                    "tool_calls".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(2)),
                ),
                (WALL_KEY.to_owned(), JsonValue::Number(JsonNumber::Uint(11))),
                (
                    "cost_usd".to_owned(),
                    JsonValue::Number(JsonNumber::Float(0.25)),
                ),
            ])
            .to_json_text(),
        ),
        (
            "unavailable_evidence",
            JsonValue::Array(vec![]).to_json_text(),
        ),
        ("errors", JsonValue::Array(vec![]).to_json_text()),
    ];
    let body = pieces
        .iter()
        .map(|(k, v)| format!("\"{k}\":{v}"))
        .collect::<Vec<_>>()
        .join(",");
    format!("{{{body}}}")
}

/// The ULID-shaped id for a per-instance counter: 20 fixed canonical
/// characters plus a 6-character base32 tail — a space of 32^6 ids before
/// an instance could ever repeat. Per-instance counters make two instances
/// replay identical sequences, as the IdGenerator suite's determinism case
/// drives them.
fn ulid_from(counter: u64) -> String {
    const ALPHABET: &[u8] = b"0123456789ABCDEFGHJKMNPQRSTVWXYZ";
    let mut tail = String::with_capacity(6);
    let mut value = counter;
    for _ in 0..6 {
        tail.push(ALPHABET[(value % 32) as usize] as char);
        value /= 32;
    }
    format!("01ARZ3NDEKTSV4RRFFQ6{tail}")
}

// ---------------------------------------------------------------------------
// agent_runner
// ---------------------------------------------------------------------------

struct ScriptedRunner {
    script: RunnerScript,
    entries: RefCell<HashMap<InvocationId, ScriptedEntry>>,
}

struct ScriptedEntry {
    script: RunnerScript,
    probes: u32,
    terminal: Option<InvocationStatus>,
    result: Option<RoleResult>,
    expected_invocation: InvocationId,
    expected_digest: Digest,
    contract: ResultContractPath,
}

impl ScriptedEntry {
    fn in_flight(&self) -> InvocationStatus {
        InvocationStatus::Running {
            last_event_at: EpochSeconds::new(BASE_INSTANT + FALLBACK_WALL),
            payment_so_far: Payment::zero(),
        }
    }

    fn complete(&mut self, doc: String) {
        let parsed = read_result_contract(&doc, &self.expected_invocation, &self.expected_digest);
        match parsed {
            Ok(result) => {
                self.result = Some(result);
                self.terminal = Some(InvocationStatus::Completed {
                    result_ref: self.contract.clone(),
                });
            }
            Err(error) => {
                self.terminal = Some(InvocationStatus::Failed {
                    class: error.failure_class(),
                });
            }
        }
    }
}

impl ScriptedRunner {
    fn advance(&self, entry: &mut ScriptedEntry) {
        match entry.script {
            RunnerScript::CompleteAfterProbes {
                probes_before_result,
            } => {
                entry.probes += 1;
                if entry.probes > u32::from(probes_before_result) {
                    entry.complete(result_document(
                        entry.expected_invocation.as_str(),
                        &entry.expected_digest,
                    ));
                }
            }
            RunnerScript::CompleteImmediately => {
                entry.complete(result_document(
                    entry.expected_invocation.as_str(),
                    &entry.expected_digest,
                ));
            }
            RunnerScript::NeverWriteResult => {
                entry.probes += 1;
                if entry.probes >= 4 {
                    entry.terminal = Some(InvocationStatus::TimedOut);
                }
            }
            RunnerScript::WriteMalformedResult => {
                entry.complete("not a result document".to_owned());
            }
            RunnerScript::WriteUnknownEnvelopeField => {
                let doc =
                    result_document(entry.expected_invocation.as_str(), &entry.expected_digest);
                entry.complete(doc.replace(
                    "\"errors\":[]",
                    "\"errors\":[],\"sneaky_extra\":\"surprise\"",
                ));
            }
            RunnerScript::WriteUnknownNestedField => {
                let doc =
                    result_document(entry.expected_invocation.as_str(), &entry.expected_digest);
                entry.complete(doc.replace(
                    "\"self_checks\":[]",
                    "\"self_checks\":[{\"command\":\"suite-selfcheck\",\"exit_code\":0,\"sneaky_extra\":\"surprise\"}]",
                ));
            }
            RunnerScript::WriteForeignInvocationId => {
                entry.complete(result_document(
                    "01ARZ3NDEKTSV4RRFFQ69G5FAB",
                    &entry.expected_digest,
                ));
            }
            RunnerScript::WriteForeignEnvelopeDigest => {
                let foreign = Digest::new("sha256", "e5f6a7b8a1b2c3d4").expect("stub digest");
                entry.complete(result_document(
                    entry.expected_invocation.as_str(),
                    &foreign,
                ));
            }
        }
    }
}

impl AgentRunner for ScriptedRunner {
    fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
        let invocation_id = spec.invocation_id.clone();
        let entry = ScriptedEntry {
            script: self.script,
            probes: 0,
            terminal: None,
            result: None,
            expected_invocation: spec.invocation_id.clone(),
            expected_digest: spec.context_envelope.digest().clone(),
            contract: spec.result_contract_path,
        };
        self.entries
            .borrow_mut()
            .insert(invocation_id.clone(), entry);
        Ok(InvocationHandle::new(invocation_id, None))
    }

    fn poll(&self, handle: &InvocationHandle) -> Result<InvocationStatus, AgentError> {
        let mut entries = self.entries.borrow_mut();
        let Some(entry) = entries.get_mut(handle.invocation_id()) else {
            return Err(AgentError::UnknownHandle {
                invocation_id: handle.invocation_id().clone(),
            });
        };
        if entry.terminal.is_none() {
            self.advance(entry);
        }
        Ok(entry.terminal.clone().unwrap_or_else(|| entry.in_flight()))
    }

    fn collect(
        &self,
        handle: &InvocationHandle,
        _deadline: Deadline,
    ) -> Result<RoleResult, AgentError> {
        let entries = self.entries.borrow();
        let Some(entry) = entries.get(handle.invocation_id()) else {
            return Err(AgentError::UnknownHandle {
                invocation_id: handle.invocation_id().clone(),
            });
        };
        match (&entry.terminal, &entry.result) {
            (Some(InvocationStatus::Completed { .. }), Some(result)) => Ok(result.clone()),
            (Some(terminal), _) => Err(AgentError::ResultNotReady {
                invocation_id: handle.invocation_id().clone(),
                state: status_state(terminal),
            }),
            (None, _) => Err(AgentError::ResultNotReady {
                invocation_id: handle.invocation_id().clone(),
                state: InvocationState::Running,
            }),
        }
    }

    fn cancel(
        &self,
        handle: &InvocationHandle,
        _reason: CancelReason,
        _deadline: Deadline,
    ) -> Result<(), AgentError> {
        let mut entries = self.entries.borrow_mut();
        let Some(entry) = entries.get_mut(handle.invocation_id()) else {
            return Err(AgentError::UnknownHandle {
                invocation_id: handle.invocation_id().clone(),
            });
        };
        if let Some(terminal) = &entry.terminal {
            return Err(AgentError::AlreadyTerminal {
                invocation_id: handle.invocation_id().clone(),
                state: status_state(terminal),
            });
        }
        entry.terminal = Some(InvocationStatus::Cancelled);
        Ok(())
    }

    fn capabilities(&self) -> RunnerCapabilities {
        RunnerCapabilities {
            supports_native_subagents: false,
            supports_streaming_events: false,
            supports_model_selection: false,
            isolation: Isolation::Process,
            result_delivery: ResultDelivery::File,
            max_depth: 8,
        }
    }
}

pub struct AgentRunnerGreen;

impl AgentRunnerFactory for AgentRunnerGreen {
    fn label(&self) -> &'static str {
        "green-scripted-runner"
    }

    fn build(&self, script: RunnerScript) -> Result<Box<dyn AgentRunner>, String> {
        Ok(Box::new(ScriptedRunner {
            script,
            entries: RefCell::new(HashMap::new()),
        }))
    }
}

struct BlockingRunner;

impl AgentRunner for BlockingRunner {
    fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
        Ok(InvocationHandle::new(spec.invocation_id, None))
    }

    fn poll(&self, handle: &InvocationHandle) -> Result<InvocationStatus, AgentError> {
        let _ = handle;
        Ok(InvocationStatus::Completed {
            result_ref: ResultContractPath::new("suite-result-document").expect("stub path"),
        })
    }

    fn collect(
        &self,
        handle: &InvocationHandle,
        _deadline: Deadline,
    ) -> Result<RoleResult, AgentError> {
        let result = RoleResult::decode(&result_document(
            handle.invocation_id().as_str(),
            &never_matching_digest(),
        ))
        .expect("stub document decodes");
        Ok(result)
    }

    fn cancel(
        &self,
        handle: &InvocationHandle,
        _reason: CancelReason,
        _deadline: Deadline,
    ) -> Result<(), AgentError> {
        let _ = handle;
        Ok(())
    }

    fn capabilities(&self) -> RunnerCapabilities {
        RunnerCapabilities {
            supports_native_subagents: false,
            supports_streaming_events: false,
            supports_model_selection: false,
            isolation: Isolation::Process,
            result_delivery: ResultDelivery::File,
            max_depth: 8,
        }
    }
}

pub struct AgentRunnerBroken;

impl AgentRunnerFactory for AgentRunnerBroken {
    fn label(&self) -> &'static str {
        "broken-blocking-runner"
    }

    fn build(&self, _script: RunnerScript) -> Result<Box<dyn AgentRunner>, String> {
        Ok(Box::new(BlockingRunner))
    }
}

/// Wraps the scripted runner with one defect: `capabilities()` flips
/// between two values on every call, so scheduling decisions built on a
/// single read are wrong half the time. Everything not capabilities-related
/// behaves like the green scripted runner.
struct FlippingCapsRunner {
    inner: ScriptedRunner,
    phase: Cell<bool>,
}

impl AgentRunner for FlippingCapsRunner {
    fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
        self.inner.invoke(spec)
    }

    fn poll(&self, handle: &InvocationHandle) -> Result<InvocationStatus, AgentError> {
        self.inner.poll(handle)
    }

    fn collect(
        &self,
        handle: &InvocationHandle,
        deadline: Deadline,
    ) -> Result<RoleResult, AgentError> {
        self.inner.collect(handle, deadline)
    }

    fn cancel(
        &self,
        handle: &InvocationHandle,
        reason: CancelReason,
        deadline: Deadline,
    ) -> Result<(), AgentError> {
        self.inner.cancel(handle, reason, deadline)
    }

    fn capabilities(&self) -> RunnerCapabilities {
        let container = self.phase.get();
        self.phase.set(!container);
        let isolation = if container {
            Isolation::Container
        } else {
            Isolation::Process
        };
        RunnerCapabilities {
            supports_native_subagents: false,
            supports_streaming_events: false,
            supports_model_selection: false,
            isolation,
            result_delivery: ResultDelivery::File,
            max_depth: 8,
        }
    }
}

pub struct AgentRunnerCapsBroken;

impl AgentRunnerFactory for AgentRunnerCapsBroken {
    fn label(&self) -> &'static str {
        "broken-flipping-caps-runner"
    }

    fn build(&self, script: RunnerScript) -> Result<Box<dyn AgentRunner>, String> {
        Ok(Box::new(FlippingCapsRunner {
            inner: ScriptedRunner {
                script,
                entries: RefCell::new(HashMap::new()),
            },
            phase: Cell::new(false),
        }))
    }
}

fn never_matching_digest() -> Digest {
    Digest::new("sha256", "0000000000000000").expect("stub digest")
}

fn status_state(status: &InvocationStatus) -> InvocationState {
    match status {
        InvocationStatus::Admitted | InvocationStatus::Running { .. } => InvocationState::Running,
        InvocationStatus::Completed { .. } => InvocationState::Completed,
        InvocationStatus::Failed { .. } => InvocationState::Failed,
        InvocationStatus::TimedOut => InvocationState::TimedOut,
        InvocationStatus::Cancelled => InvocationState::Cancelled,
    }
}

// ---------------------------------------------------------------------------
// budget_meter
// ---------------------------------------------------------------------------

struct DomainMeter {
    budget: RefCell<Budget>,
}

impl DomainMeter {
    fn probe_cause(&self, slice: BudgetSlice) -> BudgetMeterError {
        let mut budget = self.budget.borrow_mut();
        match budget.charge(
            slice.rounds,
            slice.wall_seconds,
            slice.tokens,
            slice.cost_usd,
        ) {
            Err(cause) => BudgetMeterError::Reserve { cause },
            Ok(()) => BudgetMeterError::Reserve {
                cause: BudgetError::LimitExhausted {
                    dimension: BudgetDimension::Tokens,
                    limit: TOKEN_CEILING,
                    spent: 0,
                    requested: TOKEN_CEILING,
                },
            },
        }
    }
}

impl BudgetMeter for DomainMeter {
    fn reserve(&self, slice: BudgetSlice) -> Result<Reservation, BudgetMeterError> {
        if self.budget.borrow().would_breach_reserve(
            slice.rounds,
            slice.wall_seconds,
            slice.tokens,
            slice.cost_usd,
        ) {
            return Err(self.probe_cause(slice));
        }
        Ok(self.mint(slice))
    }

    fn charge(&self, reservation: &Reservation, payment: Payment) -> Result<(), BudgetMeterError> {
        if payment.tokens() > reservation.slice().tokens {
            return Err(BudgetMeterError::ReservationExceeded {
                reserved_tokens: reservation.slice().tokens,
                payment_tokens: payment.tokens(),
            });
        }
        self.budget
            .borrow_mut()
            .charge(
                0,
                payment.wall_seconds(),
                payment.tokens(),
                payment.cost_usd(),
            )
            .map_err(|cause| BudgetMeterError::Charge { cause })
    }

    fn remaining(&self) -> Remaining {
        crate::budget_meter::remaining_of(&self.budget.borrow())
    }

    fn would_breach_reserve(&self, slice: BudgetSlice) -> bool {
        self.budget.borrow().would_breach_reserve(
            slice.rounds,
            slice.wall_seconds,
            slice.tokens,
            slice.cost_usd,
        )
    }
}

pub struct BudgetMeterGreen;

impl BudgetMeterFactory for BudgetMeterGreen {
    fn label(&self) -> &'static str {
        "green-domain-meter"
    }

    fn build(
        &self,
        limits: Limits,
        recovery_reserve_pct: u8,
        diagnostic: DiagnosticAllowance,
    ) -> Result<Box<dyn BudgetMeter>, String> {
        let budget = Budget::new(limits, recovery_reserve_pct, diagnostic)
            .map_err(|error| format!("unusable limits: {error}"))?;
        Ok(Box::new(DomainMeter {
            budget: RefCell::new(budget),
        }))
    }
}

struct LenientMeter;

/// Meter figures for the lenient stub, referenced not written (numeric
/// literals never share a line with a duration-shaped identifier).
const STUBBED_ROUNDS: u32 = 10;
const STUBBED_PARALLEL: u32 = 2;
const STUBBED_DEPTH: u32 = 2;
const STUBBED_WALL_SPAN: u64 = 3600;
const STUBBED_COST_USD: u64 = 500;
const STUBBED_OFFSET: u64 = 5;

impl BudgetMeter for LenientMeter {
    fn reserve(&self, slice: BudgetSlice) -> Result<Reservation, BudgetMeterError> {
        Ok(self.mint(slice))
    }

    fn charge(&self, reservation: &Reservation, payment: Payment) -> Result<(), BudgetMeterError> {
        if payment.tokens() > reservation.slice().tokens {
            return Err(BudgetMeterError::ReservationExceeded {
                reserved_tokens: reservation.slice().tokens,
                payment_tokens: payment.tokens(),
            });
        }
        Ok(())
    }

    fn remaining(&self) -> Remaining {
        Remaining {
            rounds: STUBBED_ROUNDS,
            parallel: STUBBED_PARALLEL,
            depth: STUBBED_DEPTH,
            wall_seconds: STUBBED_WALL_SPAN,
            tokens: TOKEN_CEILING,
            cost_usd: MicroUsd::new(STUBBED_COST_USD),
        }
    }

    fn would_breach_reserve(&self, _slice: BudgetSlice) -> bool {
        false
    }
}

pub struct BudgetMeterBroken;

impl BudgetMeterFactory for BudgetMeterBroken {
    fn label(&self) -> &'static str {
        "broken-lenient-meter"
    }

    fn build(
        &self,
        _limits: Limits,
        _recovery_reserve_pct: u8,
        _diagnostic: DiagnosticAllowance,
    ) -> Result<Box<dyn BudgetMeter>, String> {
        Ok(Box::new(LenientMeter))
    }
}

// ---------------------------------------------------------------------------
// clock
// ---------------------------------------------------------------------------

struct FreeClock {
    now: Cell<EpochSeconds>,
}

impl Clock for FreeClock {
    fn now(&self) -> EpochSeconds {
        self.now.get()
    }

    fn sleep_until(&self, at: EpochSeconds) -> Result<(), ClockError> {
        if at > self.now.get() {
            self.now.set(at);
        }
        Ok(())
    }
}

pub struct ClockGreen;

impl ClockFactory for ClockGreen {
    fn label(&self) -> &'static str {
        "green-free-clock"
    }

    fn build(&self) -> Box<dyn Clock> {
        Box::new(FreeClock {
            now: Cell::new(EpochSeconds::new(BASE_INSTANT)),
        })
    }

    fn deterministic(&self) -> bool {
        true
    }
}

struct StandingClock;

impl Clock for StandingClock {
    fn now(&self) -> EpochSeconds {
        EpochSeconds::new(BASE_INSTANT)
    }

    fn sleep_until(&self, _at: EpochSeconds) -> Result<(), ClockError> {
        Ok(())
    }
}

pub struct ClockBroken;

impl ClockFactory for ClockBroken {
    fn label(&self) -> &'static str {
        "broken-standing-clock"
    }

    fn build(&self) -> Box<dyn Clock> {
        Box::new(StandingClock)
    }

    fn deterministic(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// comparator
// ---------------------------------------------------------------------------

struct VerbatimComparator;

impl Comparator for VerbatimComparator {
    fn compare(
        &self,
        envelope: &TrustedComparisonContext,
        _deadline: Deadline,
    ) -> Result<ComparisonRecord, ComparatorError> {
        if envelope.envelope().contract().version() != "1" {
            return Err(ComparatorError::EnvelopeInvalid {
                detail: format!(
                    "contract version {} is unknown",
                    envelope.envelope().contract().version()
                ),
            });
        }
        // The green stub honors the widened envelope's facet reality: a
        // recorded instrument failure or a recorded gap is a residual, and
        // a blocking one forces fail (spec 12.3, 13.5).
        let mut residuals = Vec::new();
        let mut blocking = 0usize;
        for facet in envelope.envelope().observation().facets() {
            if facet.exit_status().is_some_and(|status| status != 0) {
                residuals.push(gauntlet_domain::Residual::new(
                    gauntlet_domain::ResidualId::new(&format!(
                        "stub-facet-{}",
                        facet.kind().token()
                    ))
                    .expect("stub residual id"),
                    gauntlet_domain::Severity::Blocking,
                    format!(
                        "instrument {} exited with {}",
                        facet.kind().token(),
                        facet.exit_status().unwrap_or_default()
                    ),
                    None,
                ));
                blocking += 1;
            }
        }
        let gaps = envelope
            .envelope()
            .observation()
            .unavailable_evidence()
            .len()
            + envelope
                .envelope()
                .observation()
                .observation_failures()
                .len();
        if gaps > 0 {
            residuals.push(gauntlet_domain::Residual::new(
                gauntlet_domain::ResidualId::new("stub-gaps").expect("stub residual id"),
                gauntlet_domain::Severity::Major,
                format!("{gaps} recorded gap(s)"),
                None,
            ));
        }
        let verdict = if blocking > 0 {
            Verdict::Fail
        } else if gaps > 0 {
            Verdict::Inconclusive
        } else {
            Verdict::Pass
        };
        ComparisonRecord::new(
            envelope.envelope().reference_digest().clone(),
            envelope
                .envelope()
                .observation()
                .observation_digest()
                .clone(),
            envelope.envelope().envelope_digest().clone(),
            residuals,
            verdict,
        )
        .map_err(|error| ComparatorError::ComparisonFailed {
            detail: error.to_string(),
        })
    }
}

pub struct ComparatorGreen;

impl ComparatorFactory for ComparatorGreen {
    fn label(&self) -> &'static str {
        "green-verbatim-comparator"
    }

    fn supported_version(&self) -> String {
        "1".to_owned()
    }

    fn build(&self) -> Result<Box<dyn Comparator>, String> {
        Ok(Box::new(VerbatimComparator))
    }
}

struct WrongDigestComparator;

impl Comparator for WrongDigestComparator {
    fn compare(
        &self,
        envelope: &TrustedComparisonContext,
        _deadline: Deadline,
    ) -> Result<ComparisonRecord, ComparatorError> {
        if envelope.envelope().contract().version() != "1" {
            return Err(ComparatorError::EnvelopeInvalid {
                detail: format!(
                    "contract version {} is unknown",
                    envelope.envelope().contract().version()
                ),
            });
        }
        ComparisonRecord::new(
            never_digest(),
            never_digest(),
            never_digest(),
            Vec::new(),
            Verdict::Pass,
        )
        .map_err(|error| ComparatorError::ComparisonFailed {
            detail: error.to_string(),
        })
    }
}

pub struct ComparatorBroken;

impl ComparatorFactory for ComparatorBroken {
    fn label(&self) -> &'static str {
        "broken-wrong-digest-comparator"
    }

    fn supported_version(&self) -> String {
        "1".to_owned()
    }

    fn build(&self) -> Result<Box<dyn Comparator>, String> {
        Ok(Box::new(WrongDigestComparator))
    }
}

struct FacetBlindComparator;

impl Comparator for FacetBlindComparator {
    fn compare(
        &self,
        envelope: &TrustedComparisonContext,
        _deadline: Deadline,
    ) -> Result<ComparisonRecord, ComparatorError> {
        if envelope.envelope().contract().version() != "1" {
            return Err(ComparatorError::EnvelopeInvalid {
                detail: format!(
                    "contract version {} is unknown",
                    envelope.envelope().contract().version()
                ),
            });
        }
        // Copies every digest faithfully but IGNORES facet-level reality:
        // a recorded instrument failure still yields a clean pass. The
        // widened-envelope suite case exists to catch exactly this.
        ComparisonRecord::new(
            envelope.envelope().reference_digest().clone(),
            envelope
                .envelope()
                .observation()
                .observation_digest()
                .clone(),
            envelope.envelope().envelope_digest().clone(),
            Vec::new(),
            Verdict::Pass,
        )
        .map_err(|error| ComparatorError::ComparisonFailed {
            detail: error.to_string(),
        })
    }
}

pub struct ComparatorFacetBlind;

impl ComparatorFactory for ComparatorFacetBlind {
    fn label(&self) -> &'static str {
        "broken-facet-blind-comparator"
    }

    fn supported_version(&self) -> String {
        "1".to_owned()
    }

    fn build(&self) -> Result<Box<dyn Comparator>, String> {
        Ok(Box::new(FacetBlindComparator))
    }
}

fn never_digest() -> Digest {
    Digest::new("sha256", "0000000000000000").expect("stub digest")
}

// ---------------------------------------------------------------------------
// event_log
// ---------------------------------------------------------------------------

struct GrowingLog {
    entries: RefCell<Vec<LogEntry>>,
    next_offset: Cell<u64>,
}

impl EventLog for GrowingLog {
    fn append(
        &self,
        event: RunEvent,
        _deadline: Deadline,
    ) -> Result<EventLogOffset, EventLogError> {
        // COR3 (confirmation round 1, F-1): the settlement kind is earned
        // through the store's `commit_settlement`, never appended raw —
        // the green log stub conforms to the adapters' kind gate.
        if let crate::state_store::RunEventKind::SettlementCommitted { unit_id, .. } = &event.kind {
            return Err(EventLogError::SettlementEventNotEarned {
                run_id: event.run_id.clone(),
                unit_id: unit_id.clone(),
            });
        }
        let offset = self.next_offset.get();
        self.next_offset.set(offset + 1);
        self.entries.borrow_mut().push(LogEntry {
            offset: EventLogOffset::new(offset),
            event,
        });
        Ok(EventLogOffset::new(offset))
    }

    fn read_from(
        &self,
        offset: EventLogOffset,
        _deadline: Deadline,
    ) -> Result<Vec<LogEntry>, EventLogError> {
        Ok(self
            .entries
            .borrow()
            .iter()
            .filter(|entry| entry.offset >= offset)
            .cloned()
            .collect())
    }

    fn last_offset(&self, _deadline: Deadline) -> Result<Option<EventLogOffset>, EventLogError> {
        Ok(self.entries.borrow().last().map(|entry| entry.offset))
    }
}

pub struct EventLogGreen;

impl EventLogFactory for EventLogGreen {
    fn label(&self) -> &'static str {
        "green-growing-log"
    }

    fn build(&self) -> Result<Box<dyn EventLog>, String> {
        Ok(Box::new(GrowingLog {
            entries: RefCell::new(Vec::new()),
            next_offset: Cell::new(1),
        }))
    }
}

struct FlatLog;

impl EventLog for FlatLog {
    fn append(
        &self,
        _event: RunEvent,
        _deadline: Deadline,
    ) -> Result<EventLogOffset, EventLogError> {
        let offset = EventLogOffset::new(STUBBED_OFFSET);
        Ok(offset)
    }

    fn read_from(
        &self,
        _offset: EventLogOffset,
        _deadline: Deadline,
    ) -> Result<Vec<LogEntry>, EventLogError> {
        let entries = Vec::new();
        Ok(entries)
    }

    fn last_offset(&self, _deadline: Deadline) -> Result<Option<EventLogOffset>, EventLogError> {
        let offset = EventLogOffset::new(STUBBED_OFFSET);
        Ok(Some(offset))
    }
}

pub struct EventLogBroken;

impl EventLogFactory for EventLogBroken {
    fn label(&self) -> &'static str {
        "broken-flat-log"
    }

    fn build(&self) -> Result<Box<dyn EventLog>, String> {
        Ok(Box::new(FlatLog))
    }
}

// ---------------------------------------------------------------------------
// evidence_store
// ---------------------------------------------------------------------------

struct ContentStore {
    blobs: RefCell<HashMap<EvidenceRef, Vec<u8>>>,
    pinned: RefCell<HashSet<EvidenceRef>>,
}

impl ContentStore {
    fn reference_for(&self, bytes: &[u8]) -> EvidenceRef {
        let reference = EvidenceRef::new(&format!("evd-{}", content_digest(bytes)))
            .expect("a hex digest is a well-formed ref");
        if !self.blobs.borrow().contains_key(&reference) {
            self.blobs
                .borrow_mut()
                .insert(reference.clone(), bytes.to_vec());
        }
        reference
    }
}

impl EvidenceStore for ContentStore {
    fn put(
        &self,
        bytes: &[u8],
        _kind: EvidenceKind,
        _deadline: Deadline,
    ) -> Result<EvidenceRef, EvidenceStoreError> {
        Ok(self.reference_for(bytes))
    }

    fn get(
        &self,
        reference: &EvidenceRef,
        _deadline: Deadline,
    ) -> Result<Option<Vec<u8>>, EvidenceStoreError> {
        Ok(self.blobs.borrow().get(reference).cloned())
    }

    fn pin(
        &self,
        _run_id: &RunId,
        reference: &EvidenceRef,
        _deadline: Deadline,
    ) -> Result<(), EvidenceStoreError> {
        if !self.blobs.borrow().contains_key(reference) {
            return Err(EvidenceStoreError::NotFound {
                reference: reference.clone(),
            });
        }
        self.pinned.borrow_mut().insert(reference.clone());
        Ok(())
    }
}

pub struct EvidenceStoreGreen;

impl EvidenceStoreFactory for EvidenceStoreGreen {
    fn label(&self) -> &'static str {
        "green-content-store"
    }

    fn build(&self) -> Result<Box<dyn EvidenceStore>, String> {
        Ok(Box::new(ContentStore {
            blobs: RefCell::new(HashMap::new()),
            pinned: RefCell::new(HashSet::new()),
        }))
    }
}

struct ForgetfulStore;

impl EvidenceStore for ForgetfulStore {
    fn put(
        &self,
        _bytes: &[u8],
        _kind: EvidenceKind,
        _deadline: Deadline,
    ) -> Result<EvidenceRef, EvidenceStoreError> {
        Ok(EvidenceRef::new("evd-never-put-9999").expect("stub ref"))
    }

    fn get(
        &self,
        _reference: &EvidenceRef,
        _deadline: Deadline,
    ) -> Result<Option<Vec<u8>>, EvidenceStoreError> {
        Ok(None)
    }

    fn pin(
        &self,
        _run_id: &RunId,
        _reference: &EvidenceRef,
        _deadline: Deadline,
    ) -> Result<(), EvidenceStoreError> {
        Ok(())
    }
}

pub struct EvidenceStoreBroken;

impl EvidenceStoreFactory for EvidenceStoreBroken {
    fn label(&self) -> &'static str {
        "broken-forgetful-store"
    }

    fn build(&self) -> Result<Box<dyn EvidenceStore>, String> {
        Ok(Box::new(ForgetfulStore))
    }
}

// ---------------------------------------------------------------------------
// human_escalation
// ---------------------------------------------------------------------------

struct TicketBoard {
    tickets: RefCell<HashMap<TicketId, EscalationState>>,
    next: Cell<u64>,
}

impl HumanEscalation for TicketBoard {
    fn escalate(
        &self,
        _item: EscalationItem,
        _deadline: Deadline,
    ) -> Result<EscalationTicket, HumanEscalationError> {
        let n = self.next.get();
        self.next.set(n + 1);
        let id = TicketId::new(&format!("suite-ticket-{n:04}")).expect("stub ticket id");
        let ticket = EscalationTicket::new(id.clone(), EpochSeconds::new(BASE_INSTANT));
        self.tickets
            .borrow_mut()
            .insert(id.clone(), EscalationState::Open);
        Ok(ticket)
    }

    fn poll(
        &self,
        ticket: &EscalationTicket,
        _deadline: Deadline,
    ) -> Result<EscalationState, HumanEscalationError> {
        self.tickets
            .borrow()
            .get(ticket.id())
            .cloned()
            .ok_or_else(|| HumanEscalationError::TicketNotFound {
                ticket: ticket.id().clone(),
            })
    }
}

pub struct HumanEscalationGreen;

impl HumanEscalationFactory for HumanEscalationGreen {
    fn label(&self) -> &'static str {
        "green-ticket-board"
    }

    fn build(&self) -> Result<Box<dyn HumanEscalation>, String> {
        Ok(Box::new(TicketBoard {
            tickets: RefCell::new(HashMap::new()),
            next: Cell::new(1),
        }))
    }
}

struct GhostBoard;

impl HumanEscalation for GhostBoard {
    fn escalate(
        &self,
        _item: EscalationItem,
        _deadline: Deadline,
    ) -> Result<EscalationTicket, HumanEscalationError> {
        Ok(EscalationTicket::new(
            TicketId::new("suite-ticket-0001").expect("stub ticket id"),
            EpochSeconds::new(BASE_INSTANT),
        ))
    }

    fn poll(
        &self,
        ticket: &EscalationTicket,
        _deadline: Deadline,
    ) -> Result<EscalationState, HumanEscalationError> {
        Err(HumanEscalationError::TicketNotFound {
            ticket: ticket.id().clone(),
        })
    }
}

pub struct HumanEscalationBroken;

impl HumanEscalationFactory for HumanEscalationBroken {
    fn label(&self) -> &'static str {
        "broken-ghost-board"
    }

    fn build(&self) -> Result<Box<dyn HumanEscalation>, String> {
        Ok(Box::new(GhostBoard))
    }
}

// ---------------------------------------------------------------------------
// id_generator
// ---------------------------------------------------------------------------

struct PoolGenerator {
    next: Cell<u64>,
}

impl IdGenerator for PoolGenerator {
    fn run_id(&self) -> RunId {
        let n = self.next.get();
        self.next.set(n + 1);
        RunId::new(&ulid_from(n)).expect("stub run id is a ULID")
    }

    fn invocation_id(&self) -> InvocationId {
        let n = self.next.get();
        self.next.set(n + 1);
        InvocationId::new(&ulid_from(n)).expect("stub invocation id is a ULID")
    }
}

pub struct IdGeneratorGreen;

impl IdGeneratorFactory for IdGeneratorGreen {
    fn label(&self) -> &'static str {
        "green-pool-generator"
    }

    fn build(&self) -> Box<dyn IdGenerator> {
        Box::new(PoolGenerator { next: Cell::new(0) })
    }

    fn deterministic(&self) -> bool {
        true
    }
}

struct StuckGenerator;

impl IdGenerator for StuckGenerator {
    fn run_id(&self) -> RunId {
        RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("stub run id is a ULID")
    }

    fn invocation_id(&self) -> InvocationId {
        InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAW").expect("stub invocation id is a ULID")
    }
}

pub struct IdGeneratorBroken;

impl IdGeneratorFactory for IdGeneratorBroken {
    fn label(&self) -> &'static str {
        "broken-stuck-generator"
    }

    fn build(&self) -> Box<dyn IdGenerator> {
        Box::new(StuckGenerator)
    }

    fn deterministic(&self) -> bool {
        false
    }
}

// ---------------------------------------------------------------------------
// observer
// ---------------------------------------------------------------------------

struct LocalObserver {
    kind: FacetKind,
    live: Vec<WorkspaceRef>,
}

impl Observer for LocalObserver {
    fn observe(
        &self,
        request: &ObservationRequest,
        _deadline: Deadline,
    ) -> Result<ObservationFacet, ObserverError> {
        if !self.live.contains(&request.workspace) {
            return Err(ObserverError::Unavailable {
                kind: self.kind,
                detail: format!("{} is not observable", request.workspace.as_str()),
            });
        }
        let evidence: EvidenceRef = EvidenceRef::new("evd-000042").expect("stub evidence ref");
        Ok(ObservationFacet::new(
            self.kind,
            InstrumentIdentity::new(
                InstrumentName::new("suite-probe").expect("stub name"),
                "1.0.0".to_owned(),
            ),
            evidence,
        ))
    }

    fn facet_kind(&self) -> FacetKind {
        self.kind
    }
}

pub struct ObserverGreen;

impl ObserverFactory for ObserverGreen {
    fn label(&self) -> &'static str {
        "green-local-observer"
    }

    fn kind(&self) -> FacetKind {
        FacetKind::Filesystem
    }

    fn build(&self, live: Vec<WorkspaceRef>) -> Result<Box<dyn Observer>, String> {
        Ok(Box::new(LocalObserver {
            kind: FacetKind::Filesystem,
            live,
        }))
    }
}

struct CrossWiredObserver {
    live: Vec<WorkspaceRef>,
}

impl Observer for CrossWiredObserver {
    fn observe(
        &self,
        request: &ObservationRequest,
        _deadline: Deadline,
    ) -> Result<ObservationFacet, ObserverError> {
        if !self.live.contains(&request.workspace) {
            return Err(ObserverError::Unavailable {
                kind: FacetKind::Versioning,
                detail: format!("{} is not observable", request.workspace.as_str()),
            });
        }
        Ok(ObservationFacet::new(
            FacetKind::Versioning,
            InstrumentIdentity::new(
                InstrumentName::new("suite-probe").expect("stub name"),
                "1.0.0".to_owned(),
            ),
            EvidenceRef::new("evd-000042").expect("stub evidence ref"),
        ))
    }

    fn facet_kind(&self) -> FacetKind {
        FacetKind::Filesystem
    }
}

pub struct ObserverBroken;

impl ObserverFactory for ObserverBroken {
    fn label(&self) -> &'static str {
        "broken-cross-wired-observer"
    }

    fn kind(&self) -> FacetKind {
        FacetKind::Filesystem
    }

    fn build(&self, live: Vec<WorkspaceRef>) -> Result<Box<dyn Observer>, String> {
        Ok(Box::new(CrossWiredObserver { live }))
    }
}

// ---------------------------------------------------------------------------
// operator_surface
// ---------------------------------------------------------------------------

struct EchoSurface {
    seeded: RuntimeSnapshot,
}

impl OperatorSurface for EchoSurface {
    fn snapshot(&self, _deadline: Deadline) -> Result<RuntimeSnapshot, OperatorSurfaceError> {
        Ok(self.seeded.clone())
    }

    fn actions(&self) -> Vec<OperatorAction> {
        let unit = self
            .seeded
            .units
            .first()
            .map(|snapshot| snapshot.id.clone())
            .unwrap_or_else(|| UnitId::new("AK-001").expect("stub unit id"));
        vec![
            OperatorAction::Refresh,
            OperatorAction::ResolveUnit { unit: unit.clone() },
            OperatorAction::RetryUnit { unit },
        ]
    }
}

pub struct OperatorSurfaceGreen;

impl OperatorSurfaceFactory for OperatorSurfaceGreen {
    fn label(&self) -> &'static str {
        "green-echo-surface"
    }

    fn build(&self, seeded: RuntimeSnapshot) -> Result<Box<dyn OperatorSurface>, String> {
        Ok(Box::new(EchoSurface { seeded }))
    }
}

struct EmptySurface;

impl OperatorSurface for EmptySurface {
    fn snapshot(&self, _deadline: Deadline) -> Result<RuntimeSnapshot, OperatorSurfaceError> {
        Ok(RuntimeSnapshot {
            run: None,
            escalated: Vec::new(),
            in_flight: Vec::new(),
            units: Vec::new(),
            budget: Remaining {
                rounds: 0,
                parallel: 0,
                depth: 0,
                wall_seconds: 0,
                tokens: 0,
                cost_usd: MicroUsd::new(0),
            },
            sandbox_denials: Vec::new(),
            open_residuals: Vec::new(),
        })
    }

    fn actions(&self) -> Vec<OperatorAction> {
        vec![OperatorAction::Refresh]
    }
}

pub struct OperatorSurfaceBroken;

impl OperatorSurfaceFactory for OperatorSurfaceBroken {
    fn label(&self) -> &'static str {
        "broken-empty-surface"
    }

    fn build(&self, _seeded: RuntimeSnapshot) -> Result<Box<dyn OperatorSurface>, String> {
        Ok(Box::new(EmptySurface))
    }
}

// ---------------------------------------------------------------------------
// policy_provider
// ---------------------------------------------------------------------------

struct SeededProvider {
    policies: HashMap<BlastRadius, SandboxPolicy>,
    permission_sets: HashMap<RoleId, PermissionSet>,
}

impl PolicyProvider for SeededProvider {
    fn policy_for(
        &self,
        tier: BlastRadius,
        _workspace: &WorkspaceRef,
        _deadline: Deadline,
    ) -> Result<SandboxPolicy, PolicyProviderError> {
        if tier == BlastRadius::Irreversible {
            return Err(PolicyProviderError::NoPolicyForTier { tier });
        }
        self.policies
            .get(&tier)
            .cloned()
            .ok_or(PolicyProviderError::NoPolicyForTier { tier })
    }

    fn permissions_for(
        &self,
        role: &RoleId,
        _unit: &UnitId,
        _deadline: Deadline,
    ) -> Result<PermissionSet, PolicyProviderError> {
        self.permission_sets
            .get(role)
            .cloned()
            .ok_or_else(|| PolicyProviderError::Unavailable {
                detail: format!("no permission set for role {role:?}"),
            })
    }
}

pub struct PolicyProviderGreen;

impl PolicyProviderFactory for PolicyProviderGreen {
    fn label(&self) -> &'static str {
        "green-seeded-provider"
    }

    fn build(
        &self,
        policies: Vec<(BlastRadius, SandboxPolicy)>,
        permission_sets: Vec<(RoleId, PermissionSet)>,
    ) -> Result<Box<dyn PolicyProvider>, String> {
        Ok(Box::new(SeededProvider {
            policies: policies.into_iter().collect(),
            permission_sets: permission_sets.into_iter().collect(),
        }))
    }
}

struct LenientProvider {
    policies: HashMap<BlastRadius, SandboxPolicy>,
}

impl PolicyProvider for LenientProvider {
    fn policy_for(
        &self,
        tier: BlastRadius,
        _workspace: &WorkspaceRef,
        _deadline: Deadline,
    ) -> Result<SandboxPolicy, PolicyProviderError> {
        self.policies
            .get(&tier)
            .cloned()
            .ok_or(PolicyProviderError::NoPolicyForTier { tier })
    }

    fn permissions_for(
        &self,
        role: &RoleId,
        _unit: &UnitId,
        _deadline: Deadline,
    ) -> Result<PermissionSet, PolicyProviderError> {
        Err(PolicyProviderError::Unavailable {
            detail: format!("no permission set for role {role:?}"),
        })
    }
}

pub struct PolicyProviderBroken;

impl PolicyProviderFactory for PolicyProviderBroken {
    fn label(&self) -> &'static str {
        "broken-lenient-provider"
    }

    fn build(
        &self,
        policies: Vec<(BlastRadius, SandboxPolicy)>,
        _permission_sets: Vec<(RoleId, PermissionSet)>,
    ) -> Result<Box<dyn PolicyProvider>, String> {
        Ok(Box::new(LenientProvider {
            policies: policies.into_iter().collect(),
        }))
    }
}

// ---------------------------------------------------------------------------
// reference_provider
// ---------------------------------------------------------------------------

struct ArtifactProvider {
    artifact: Rc<RefCell<String>>,
    pending: RefCell<Option<ReferenceDefinition>>,
}

impl ArtifactProvider {
    fn current_digest(&self) -> Digest {
        let text = self.artifact.borrow();
        let hex = content_digest(text.as_bytes());
        Digest::new("sha256", &hex).expect("a hex digest is well-formed")
    }
}

impl ReferenceProvider for ArtifactProvider {
    fn resolve(
        &self,
        _intent: &str,
        _deadline: Deadline,
    ) -> Result<ReferenceDraft, ReferenceProviderError> {
        let definition = ReferenceDefinition {
            id: ReferenceId::new("ref-suite-01").expect("stub reference id"),
            version: 1,
            digest: self.current_digest(),
            source_refs: vec!["source-a".to_owned()],
            purpose: "reach the stated destination".to_owned(),
            desired_state: "the work is complete per the frozen standard".to_owned(),
            invariants: Vec::new(),
            constraints: Vec::new(),
            anti_goals: Vec::new(),
            units: Vec::new(),
            quality_dimensions: Vec::new(),
            settlement: SettlementCriteria::new(Vec::new(), Vec::new(), 1, Vec::new(), Vec::new())
                .expect("stub settlement criteria"),
            unknowns: Vec::new(),
            assumptions: Vec::new(),
        };
        *self.pending.borrow_mut() = Some(definition.clone());
        Ok(ReferenceDraft::new(vec!["source-a".to_owned()], definition))
    }

    fn normalize(
        &self,
        draft: ReferenceDraft,
        _deadline: Deadline,
    ) -> Result<Reference, ReferenceProviderError> {
        Reference::new(draft.into_definition())
            .map_err(|cause| ReferenceProviderError::NormalizeFailed { cause })
    }

    fn digest(
        &self,
        reference: &Reference,
        _deadline: Deadline,
    ) -> Result<Digest, ReferenceProviderError> {
        Ok(reference.digest().clone())
    }

    fn freeze(
        &self,
        _run_id: &RunId,
        reference: &Reference,
        _deadline: Deadline,
    ) -> Result<ReferenceVersion, ReferenceProviderError> {
        Ok(ReferenceVersion::new(
            reference.id().clone(),
            reference.version(),
            reference.digest().clone(),
        ))
    }

    fn verify_unchanged(
        &self,
        version: &ReferenceVersion,
        _deadline: Deadline,
    ) -> Result<(), ReferenceProviderError> {
        let actual = self.current_digest();
        if actual == *version.digest() {
            Ok(())
        } else {
            Err(ReferenceProviderError::ReferenceMutated {
                expected: version.digest().clone(),
                actual,
            })
        }
    }
}

pub struct ReferenceProviderGreen;

impl ReferenceProviderFactory for ReferenceProviderGreen {
    fn label(&self) -> &'static str {
        "green-artifact-provider"
    }

    fn build(&self, artifact: Rc<RefCell<String>>) -> Result<Box<dyn ReferenceProvider>, String> {
        Ok(Box::new(ArtifactProvider {
            artifact,
            pending: RefCell::new(None),
        }))
    }
}

struct BlindProvider {
    artifact: Rc<RefCell<String>>,
}

impl ReferenceProvider for BlindProvider {
    fn resolve(
        &self,
        _intent: &str,
        _deadline: Deadline,
    ) -> Result<ReferenceDraft, ReferenceProviderError> {
        Ok(ReferenceDraft::new(
            vec!["source-a".to_owned()],
            ReferenceDefinition {
                id: ReferenceId::new("ref-suite-01").expect("stub reference id"),
                version: 1,
                digest: Digest::new("sha256", "a1b2c3d4e5f6a7b8").expect("stub digest"),
                source_refs: vec!["source-a".to_owned()],
                purpose: "reach the stated destination".to_owned(),
                desired_state: "the work is complete per the frozen standard".to_owned(),
                invariants: Vec::new(),
                constraints: Vec::new(),
                anti_goals: Vec::new(),
                units: Vec::new(),
                quality_dimensions: Vec::new(),
                settlement: SettlementCriteria::new(
                    Vec::new(),
                    Vec::new(),
                    1,
                    Vec::new(),
                    Vec::new(),
                )
                .expect("stub settlement criteria"),
                unknowns: Vec::new(),
                assumptions: Vec::new(),
            },
        ))
    }

    fn normalize(
        &self,
        draft: ReferenceDraft,
        _deadline: Deadline,
    ) -> Result<Reference, ReferenceProviderError> {
        Reference::new(draft.into_definition())
            .map_err(|cause| ReferenceProviderError::NormalizeFailed { cause })
    }

    fn digest(
        &self,
        reference: &Reference,
        _deadline: Deadline,
    ) -> Result<Digest, ReferenceProviderError> {
        Ok(reference.digest().clone())
    }

    fn freeze(
        &self,
        _run_id: &RunId,
        reference: &Reference,
        _deadline: Deadline,
    ) -> Result<ReferenceVersion, ReferenceProviderError> {
        Ok(ReferenceVersion::new(
            reference.id().clone(),
            reference.version(),
            reference.digest().clone(),
        ))
    }

    fn verify_unchanged(
        &self,
        _version: &ReferenceVersion,
        _deadline: Deadline,
    ) -> Result<(), ReferenceProviderError> {
        let _ = &self.artifact;
        Ok(())
    }
}

pub struct ReferenceProviderBroken;

impl ReferenceProviderFactory for ReferenceProviderBroken {
    fn label(&self) -> &'static str {
        "broken-blind-provider"
    }

    fn build(&self, artifact: Rc<RefCell<String>>) -> Result<Box<dyn ReferenceProvider>, String> {
        Ok(Box::new(BlindProvider { artifact }))
    }
}

// ---------------------------------------------------------------------------
// sandbox
// ---------------------------------------------------------------------------

/// The green sandbox stub is an honest NONE-rung backend: it validates only
/// the contained tier clean ("backend `none`, tier `contained` only" —
/// spec §8.6/§17.3; AK-050), refuses every policy above contained, and
/// wraps a clean validation verbatim (the identity wrap, legal only at
/// contained). Its `wrap` refuses a foreign validation, a
/// findings-carrying validation, and a validation claiming a rung it never
/// achieved here (AK-047; the cross-wiring defense).
struct LocalSandbox;

impl Sandbox for LocalSandbox {
    fn validate(
        &self,
        policy: &SandboxPolicy,
        _deadline: Deadline,
    ) -> Result<PolicyValidation, SandboxError> {
        if policy.tier != BlastRadius::Contained {
            return Err(SandboxError::ValidationFailed {
                policy: policy.reference.clone(),
                findings: vec![format!(
                    "a none-rung backend may not serve the {tier} tier — an identity wrap is \
                     legal only at contained (spec 17.3)",
                    tier = policy.tier
                )],
            });
        }
        Ok(PolicyValidation {
            policy: policy.reference.clone(),
            backend: policy.backend.clone(),
            isolation: Isolation::None,
            findings: Vec::new(),
        })
    }

    fn wrap(
        &self,
        command: &str,
        policy: &SandboxPolicy,
        validation: &PolicyValidation,
        _deadline: Deadline,
    ) -> Result<SandboxedCommand, SandboxError> {
        if validation.policy != policy.reference {
            return Err(SandboxError::WrapFailed {
                detail: format!(
                    "validation {} does not cover policy {}",
                    validation.policy.as_str(),
                    policy.reference.as_str()
                ),
            });
        }
        if !validation.is_clean() {
            return Err(SandboxError::WrapFailed {
                detail: format!(
                    "validation {} carries {} finding(s) — a failed validation authorizes \
                     nothing (spec 21.3)",
                    validation.policy.as_str(),
                    validation.findings.len()
                ),
            });
        }
        if validation.isolation != Isolation::None {
            return Err(SandboxError::WrapFailed {
                detail: format!(
                    "validation claims the {} rung; this backend achieves none — a foreign \
                     validation is a cross-wiring",
                    validation.isolation.token()
                ),
            });
        }
        Ok(SandboxedCommand {
            command: command.to_owned(),
            policy: policy.reference.clone(),
            validation: validation.clone(),
        })
    }

    fn backend_identity(&self) -> SandboxBackendRef {
        SandboxBackendRef::new("local-rung").expect("stub backend ref")
    }
}

pub struct SandboxGreen;

impl SandboxFactory for SandboxGreen {
    fn label(&self) -> &'static str {
        "green-local-sandbox"
    }

    fn build(&self) -> Result<Box<dyn Sandbox>, String> {
        Ok(Box::new(LocalSandbox))
    }
}

struct LaxSandbox;

impl Sandbox for LaxSandbox {
    fn validate(
        &self,
        policy: &SandboxPolicy,
        _deadline: Deadline,
    ) -> Result<PolicyValidation, SandboxError> {
        Ok(PolicyValidation {
            policy: policy.reference.clone(),
            backend: policy.backend.clone(),
            isolation: Isolation::Process,
            findings: Vec::new(),
        })
    }

    fn wrap(
        &self,
        command: &str,
        policy: &SandboxPolicy,
        validation: &PolicyValidation,
        _deadline: Deadline,
    ) -> Result<SandboxedCommand, SandboxError> {
        Ok(SandboxedCommand {
            command: command.to_owned(),
            policy: policy.reference.clone(),
            validation: validation.clone(),
        })
    }

    fn backend_identity(&self) -> SandboxBackendRef {
        SandboxBackendRef::new("local-rung").expect("stub backend ref")
    }
}

pub struct SandboxBroken;

impl SandboxFactory for SandboxBroken {
    fn label(&self) -> &'static str {
        "broken-lax-sandbox"
    }

    fn build(&self) -> Result<Box<dyn Sandbox>, String> {
        Ok(Box::new(LaxSandbox))
    }
}

/// The no-unsandboxed-fallback FAKE (AK-047 teeth): validates exactly like
/// the green stub, but its `wrap` IGNORES whether the validation carries
/// findings — a failure-shaped validation authorizes execution anyway. One
/// defect; everything else behaves like the green stub, so the suite
/// catches it precisely on the fallback case.
struct FallbackSandbox;

impl Sandbox for FallbackSandbox {
    fn validate(
        &self,
        policy: &SandboxPolicy,
        deadline: Deadline,
    ) -> Result<PolicyValidation, SandboxError> {
        LocalSandbox.validate(policy, deadline)
    }

    fn wrap(
        &self,
        command: &str,
        policy: &SandboxPolicy,
        validation: &PolicyValidation,
        _deadline: Deadline,
    ) -> Result<SandboxedCommand, SandboxError> {
        if validation.policy != policy.reference {
            return Err(SandboxError::WrapFailed {
                detail: format!(
                    "validation {} does not cover policy {}",
                    validation.policy.as_str(),
                    policy.reference.as_str()
                ),
            });
        }
        // THE DEFECT: no is_clean gate — a findings-carrying validation
        // wraps anyway.
        if validation.isolation != Isolation::None {
            return Err(SandboxError::WrapFailed {
                detail: format!(
                    "validation claims the {} rung; this backend achieves none",
                    validation.isolation.token()
                ),
            });
        }
        Ok(SandboxedCommand {
            command: command.to_owned(),
            policy: policy.reference.clone(),
            validation: validation.clone(),
        })
    }

    fn backend_identity(&self) -> SandboxBackendRef {
        SandboxBackendRef::new("local-rung").expect("stub backend ref")
    }
}

pub struct SandboxBrokenFallback;

impl SandboxFactory for SandboxBrokenFallback {
    fn label(&self) -> &'static str {
        "broken-fallback-sandbox"
    }

    fn build(&self) -> Result<Box<dyn Sandbox>, String> {
        Ok(Box::new(FallbackSandbox))
    }
}

/// The process-start FAKE (AK-048 teeth): every validation claims the
/// PROCESS rung, but `wrap` performs an identity wrap — the returned
/// command IS the bare probe, so the probe would be the first process, not
/// the sandbox. One defect; its tier posture and validation gating behave
/// like a conforming backend, so the suite catches it on the
/// process-start case.
struct VerbatimProcessSandbox;

impl Sandbox for VerbatimProcessSandbox {
    fn validate(
        &self,
        policy: &SandboxPolicy,
        _deadline: Deadline,
    ) -> Result<PolicyValidation, SandboxError> {
        if policy.tier != BlastRadius::Contained {
            return Err(SandboxError::ValidationFailed {
                policy: policy.reference.clone(),
                findings: vec![format!(
                    "this backend refuses the {tier} tier (spec 17.3)",
                    tier = policy.tier
                )],
            });
        }
        Ok(PolicyValidation {
            policy: policy.reference.clone(),
            backend: policy.backend.clone(),
            isolation: Isolation::Process,
            findings: Vec::new(),
        })
    }

    fn wrap(
        &self,
        command: &str,
        policy: &SandboxPolicy,
        validation: &PolicyValidation,
        _deadline: Deadline,
    ) -> Result<SandboxedCommand, SandboxError> {
        if validation.policy != policy.reference {
            return Err(SandboxError::WrapFailed {
                detail: format!(
                    "validation {} does not cover policy {}",
                    validation.policy.as_str(),
                    policy.reference.as_str()
                ),
            });
        }
        if !validation.is_clean() {
            return Err(SandboxError::WrapFailed {
                detail: format!(
                    "validation {} carries {} finding(s) (spec 21.3)",
                    validation.policy.as_str(),
                    validation.findings.len()
                ),
            });
        }
        if validation.isolation != Isolation::Process {
            return Err(SandboxError::WrapFailed {
                detail: format!(
                    "validation claims the {} rung; this backend reports process",
                    validation.isolation.token()
                ),
            });
        }
        // THE DEFECT: claims process isolation but returns the bare probe.
        Ok(SandboxedCommand {
            command: command.to_owned(),
            policy: policy.reference.clone(),
            validation: validation.clone(),
        })
    }

    fn backend_identity(&self) -> SandboxBackendRef {
        SandboxBackendRef::new("local-rung").expect("stub backend ref")
    }
}

pub struct SandboxBrokenProcessStart;

impl SandboxFactory for SandboxBrokenProcessStart {
    fn label(&self) -> &'static str {
        "broken-verbatim-process-sandbox"
    }

    fn build(&self) -> Result<Box<dyn Sandbox>, String> {
        Ok(Box::new(VerbatimProcessSandbox))
    }
}

/// The shell-word FAKE (the policy-interpolation teeth): a process-rung
/// backend that renders a real containment invocation — it starts with its
/// own identity, carries the payload quoted, and is strictly longer than
/// the probe — but interpolates the policy's writable roots and denied
/// paths RAW into that command. One defect: policy text becomes command
/// structure, so a path carrying a flag after a space contributes a word
/// of its own, and an honest path containing a space arrives split. Its
/// validation gating and tier posture behave like a conforming backend, so
/// the suite catches it precisely on the interpolation case.
struct RawInterpolationSandbox;

impl RawInterpolationSandbox {
    /// THE DEFECT lives here: every word is joined with a space and no
    /// word carrying whitespace is quoted.
    fn render(&self, command: &str, policy: &SandboxPolicy) -> String {
        let mut words = vec![self.backend_identity().as_str().to_owned()];
        for denied in &policy.denied_paths {
            words.push("--mask".to_owned());
            words.push(denied.clone());
        }
        for root in &policy.writable_roots {
            words.push("--bind".to_owned());
            words.push(root.clone());
        }
        words.push("--".to_owned());
        words.push(format!("'{command}'"));
        words.join(" ")
    }
}

impl Sandbox for RawInterpolationSandbox {
    fn validate(
        &self,
        policy: &SandboxPolicy,
        deadline: Deadline,
    ) -> Result<PolicyValidation, SandboxError> {
        VerbatimProcessSandbox.validate(policy, deadline)
    }

    fn wrap(
        &self,
        command: &str,
        policy: &SandboxPolicy,
        validation: &PolicyValidation,
        deadline: Deadline,
    ) -> Result<SandboxedCommand, SandboxError> {
        let gated = VerbatimProcessSandbox.wrap(command, policy, validation, deadline)?;
        Ok(SandboxedCommand {
            command: self.render(command, policy),
            ..gated
        })
    }

    fn backend_identity(&self) -> SandboxBackendRef {
        SandboxBackendRef::new("local-rung").expect("stub backend ref")
    }
}

pub struct SandboxBrokenShellWords;

impl SandboxFactory for SandboxBrokenShellWords {
    fn label(&self) -> &'static str {
        "broken-raw-interpolation-sandbox"
    }

    fn build(&self) -> Result<Box<dyn Sandbox>, String> {
        Ok(Box::new(RawInterpolationSandbox))
    }
}

/// The mount-ORDER FAKE (the nested-denial teeth): a process-rung backend
/// that quotes every interpolated path correctly — so policy text never
/// becomes command structure — but emits its denied-path masks BEFORE the
/// binds that grant its writable roots. Mount operations apply in the
/// order they are given, so the later bind of an enclosing writable root
/// re-exposes exactly the subtree the mask covered. One defect: the order,
/// and it is invisible to every case that reads only WHICH words appear.
struct MaskBeforeBindSandbox;

impl MaskBeforeBindSandbox {
    /// THE DEFECT lives here: masks are emitted before binds, so a bind of
    /// an enclosing root undoes the mask underneath it.
    fn render(&self, command: &str, policy: &SandboxPolicy) -> String {
        let quoted = |word: &str| format!("'{}'", word.replace('\'', "'\\''"));
        let mut words = vec![self.backend_identity().as_str().to_owned()];
        for denied in &policy.denied_paths {
            words.push("--mask".to_owned());
            words.push(quoted(denied));
        }
        for root in &policy.writable_roots {
            words.push("--bind".to_owned());
            words.push(quoted(root));
        }
        words.push("--".to_owned());
        words.push(quoted(command));
        words.join(" ")
    }
}

impl Sandbox for MaskBeforeBindSandbox {
    fn validate(
        &self,
        policy: &SandboxPolicy,
        deadline: Deadline,
    ) -> Result<PolicyValidation, SandboxError> {
        VerbatimProcessSandbox.validate(policy, deadline)
    }

    fn wrap(
        &self,
        command: &str,
        policy: &SandboxPolicy,
        validation: &PolicyValidation,
        deadline: Deadline,
    ) -> Result<SandboxedCommand, SandboxError> {
        let gated = VerbatimProcessSandbox.wrap(command, policy, validation, deadline)?;
        Ok(SandboxedCommand {
            command: self.render(command, policy),
            ..gated
        })
    }

    fn backend_identity(&self) -> SandboxBackendRef {
        SandboxBackendRef::new("local-rung").expect("stub backend ref")
    }
}

pub struct SandboxBrokenMaskOrder;

impl SandboxFactory for SandboxBrokenMaskOrder {
    fn label(&self) -> &'static str {
        "broken-mask-before-bind-sandbox"
    }

    fn build(&self) -> Result<Box<dyn Sandbox>, String> {
        Ok(Box::new(MaskBeforeBindSandbox))
    }
}

// ---------------------------------------------------------------------------
// state_store
// ---------------------------------------------------------------------------

struct DurableStore {
    recorded_runs: RefCell<HashMap<RunId, Run>>,
    checkpoints: RefCell<HashMap<RunId, Checkpoint>>,
    events: RefCell<HashMap<RunId, Vec<LogEntry>>>,
    productions: RefCell<Vec<crate::state_store::EvidenceProductionBinding>>,
    bindings: RefCell<HashMap<(RunId, u32), crate::state_store::ObservationBinding>>,
    selections: RefCell<HashMap<RunId, Vec<gauntlet_domain::CorrectionSelection>>>,
    verifications: RefCell<Vec<crate::verification_record::VerificationRecordDoc>>,
    settlement_rows: RefCell<Vec<(crate::state_store::SettlementCommit, EventLogOffset)>>,
    next_offset: Cell<u64>,
}

impl DurableStore {
    /// The earned internal append (COR3): the only route a settlement
    /// event enters this stub's log — `commit_settlement`'s own writer,
    /// mirroring the real adapters' internal append behind the public
    /// kind gate.
    fn append_event_ungated(&self, event: RunEvent) -> EventLogOffset {
        let offset = self.next_offset.get();
        self.next_offset.set(offset + 1);
        self.events
            .borrow_mut()
            .entry(event.run_id.clone())
            .or_default()
            .push(LogEntry {
                offset: EventLogOffset::new(offset),
                event,
            });
        EventLogOffset::new(offset)
    }

    fn reconstruct(&self, run_id: &RunId) -> Option<Run> {
        let checkpoint = self.checkpoints.borrow().get(run_id).cloned()?;
        let budget = Budget::new(
            Limits::new(10, 2, 2, 3600, TOKEN_CEILING, MicroUsd::new(500)).expect("stub limits"),
            RESERVE_PCT,
            DiagnosticAllowance::new(100, MicroUsd::new(50)),
        )
        .expect("stub budget usable");
        let version = ReferenceVersion::new(
            ReferenceId::new("ref-suite-01").expect("stub reference id"),
            1,
            checkpoint.reference_digest.clone(),
        );
        Run::new(RunDefinition {
            id: run_id.clone(),
            created_at: EpochSeconds::new(BASE_INSTANT),
            reference_version: version,
            intent: "suite run".to_owned(),
            config_fingerprint: Digest::new("sha256", "a1b2c3d4e5f6a7b8").expect("stub digest"),
            environment_fingerprint: Digest::new("sha256", "a1b2c3d4e5f6a7b8")
                .expect("stub digest"),
            budget,
            parent_run: None,
        })
        .ok()
    }
}

impl StateStore for DurableStore {
    fn append_event(
        &self,
        event: RunEvent,
        _deadline: Deadline,
    ) -> Result<EventLogOffset, StateStoreError> {
        // COR3 (confirmation round 1, F-1): the settlement kind is
        // authority-minted — earned only through `commit_settlement`
        // consuming an `AdmittedSettlement`. The green stub conforms to the
        // same gate the real adapters carry: the generic append refuses the
        // kind for every caller.
        if let crate::state_store::RunEventKind::SettlementCommitted { unit_id, .. } = &event.kind {
            return Err(StateStoreError::SettlementEventNotEarned {
                run_id: event.run_id.clone(),
                unit_id: unit_id.clone(),
            });
        }
        Ok(self.append_event_ungated(event))
    }

    fn load_run(
        &self,
        run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Option<Run>, StateStoreError> {
        if let Some(run) = self.recorded_runs.borrow().get(run_id) {
            return Ok(Some(run.clone()));
        }
        Ok(self.reconstruct(run_id))
    }

    /// Run identity recording with the port's idempotency rule: a
    /// re-record of the SAME identity is a no-op, a disagreeing one is a
    /// typed error naming the field (the frozen reference above all).
    fn record_run(&self, run: &Run, _deadline: Deadline) -> Result<(), StateStoreError> {
        let mut runs = self.recorded_runs.borrow_mut();
        if let Some(existing) = runs.get(run.id()) {
            let same_identity = existing.created_at() == run.created_at()
                && existing.reference_version() == run.reference_version()
                && existing.intent() == run.intent()
                && existing.config_fingerprint() == run.config_fingerprint()
                && existing.environment_fingerprint() == run.environment_fingerprint()
                && existing.budget() == run.budget()
                && existing.parent_run() == run.parent_run();
            if same_identity {
                return Ok(());
            }
            return Err(StateStoreError::WriteFailed {
                detail: format!(
                    "run {} is already recorded; the re-recording disagrees on at least one \
                     identity field (frozen reference {}, intent, fingerprints, budget, parent)",
                    run.id(),
                    existing.reference_version().digest(),
                ),
            });
        }
        runs.insert(run.id().clone(), run.clone());
        Ok(())
    }

    fn checkpoint(
        &self,
        checkpoint: Checkpoint,
        _deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.checkpoints
            .borrow_mut()
            .insert(checkpoint.run_id.clone(), checkpoint);
        Ok(())
    }

    fn resume(
        &self,
        run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Option<Checkpoint>, StateStoreError> {
        Ok(self.checkpoints.borrow().get(run_id).cloned())
    }

    // The two owner writes, gated the way every store now gates them: the
    // payload arrives only as an authority-minted value, and this stub
    // enforces the write-once rules the shared suite pins.
    fn record_evidence_production(
        &self,
        run_id: &RunId,
        production: &super::super::producer_authority::AdmittedEvidenceProduction,
        _deadline: Deadline,
    ) -> Result<crate::state_store::EvidenceProductionBinding, StateStoreError> {
        let payload = production.production();
        let mut productions = self.productions.borrow_mut();
        if let Some(existing) = productions
            .iter()
            .find(|binding| binding.production().evidence() == payload.evidence())
        {
            if existing.production() == payload {
                return Ok(existing.clone());
            }
            return Err(StateStoreError::WriteFailed {
                detail: format!(
                    "run {run_id} already recorded {} with different facts",
                    payload.evidence()
                ),
            });
        }
        let offset = self.next_offset.get();
        self.next_offset.set(offset + 1);
        let binding = crate::state_store::EvidenceProductionBinding::new(
            run_id.clone(),
            payload.clone(),
            EventLogOffset::new(offset),
        );
        productions.push(binding.clone());
        Ok(binding)
    }

    fn evidence_production(
        &self,
        run_id: &RunId,
        evidence: &gauntlet_domain::EvidenceRef,
        _deadline: Deadline,
    ) -> Result<Option<crate::state_store::EvidenceProductionBinding>, StateStoreError> {
        Ok(self
            .productions
            .borrow()
            .iter()
            .find(|binding| {
                binding.run_id() == run_id && binding.production().evidence() == evidence
            })
            .cloned())
    }

    fn bind_observation_bundle(
        &self,
        run_id: &RunId,
        iteration_no: gauntlet_domain::IterationNo,
        binding: &super::super::producer_authority::AdmittedObservationBinding,
        _deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        let payload = binding.binding();
        let mut bindings = self.bindings.borrow_mut();
        let key = (run_id.clone(), iteration_no.get());
        match bindings.get(&key) {
            Some(found)
                if found.evidence() == payload.evidence() && found.digest() == payload.digest() =>
            {
                return Ok(());
            }
            Some(found) => {
                return Err(StateStoreError::WriteFailed {
                    detail: format!(
                        "run {run_id} iteration {} is already bound to {}",
                        iteration_no.get(),
                        found.evidence()
                    ),
                });
            }
            None => {}
        }
        bindings.insert(key, payload.clone());
        Ok(())
    }

    fn observation_binding(
        &self,
        run_id: &RunId,
        iteration_no: gauntlet_domain::IterationNo,
        _deadline: Deadline,
    ) -> Result<Option<crate::state_store::ObservationBinding>, StateStoreError> {
        Ok(self
            .bindings
            .borrow()
            .get(&(run_id.clone(), iteration_no.get()))
            .cloned())
    }

    // The controller's §15.1 selection record (plan Task 12, the
    // `record_role_invocation` precedent): append per record, read back in
    // record order, keyed by run.
    fn record_correction_selection(
        &self,
        selection: &gauntlet_domain::CorrectionSelection,
        _deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.selections
            .borrow_mut()
            .entry(selection.run_id().clone())
            .or_default()
            .push(selection.clone());
        Ok(())
    }

    fn correction_selections(
        &self,
        run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Vec<gauntlet_domain::CorrectionSelection>, StateStoreError> {
        Ok(self
            .selections
            .borrow()
            .get(run_id)
            .cloned()
            .unwrap_or_default())
    }

    // The settlement surface (plan Task 13), implemented the way the suite
    // pins it: the verification record is a durable fact the admission
    // reads; the commit appends the event and keeps the keyed row; replay
    // scans at or after the anchor and completes half-applied keys.
    fn record_verification(
        &self,
        record: &crate::producer_authority::AdmittedVerificationRecord,
        _deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.verifications
            .borrow_mut()
            .push(record.record().clone());
        Ok(())
    }

    fn verification_records(
        &self,
        run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Vec<crate::verification_record::VerificationRecordDoc>, StateStoreError> {
        Ok(self
            .verifications
            .borrow()
            .iter()
            .filter(|record| record.run_id() == run_id)
            .cloned()
            .collect())
    }

    fn commit_settlement(
        &self,
        admitted: &crate::producer_authority::AdmittedSettlement,
        _deadline: Deadline,
    ) -> Result<EventLogOffset, StateStoreError> {
        let commit = admitted.commit().clone();
        let mut rows = self.settlement_rows.borrow_mut();
        if rows.iter().any(|(row, _)| {
            row.unit_id == commit.unit_id && row.verification == commit.verification
        }) {
            return Err(StateStoreError::WriteFailed {
                detail: format!(
                    "unit {} of run {} is already settled under verification {}",
                    commit.unit_id, commit.run_id, commit.verification
                ),
            });
        }
        let event = RunEvent {
            run_id: commit.run_id.clone(),
            kind: crate::state_store::RunEventKind::SettlementCommitted {
                unit_id: commit.unit_id.clone(),
                evidence: commit.evidence.clone(),
                verification: commit.verification.clone(),
                reference_digest: commit.reference_digest.clone(),
            },
            at: commit.at,
        };
        // COR3: the earned internal append — the stub's public
        // `append_event` refuses the settlement kind like every conforming
        // adapter.
        let offset = self.append_event_ungated(event);
        rows.push((commit, offset));
        Ok(offset)
    }

    fn settlements(
        &self,
        run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Vec<gauntlet_domain::SettlementRecord>, StateStoreError> {
        let mut rows = self
            .settlement_rows
            .borrow()
            .iter()
            .filter(|(row, _)| &row.run_id == run_id)
            .cloned()
            .collect::<Vec<_>>();
        rows.sort_by(|(left, _), (right, _)| {
            (left.unit_id.as_str(), left.verification.as_str())
                .cmp(&(right.unit_id.as_str(), right.verification.as_str()))
        });
        Ok(rows
            .into_iter()
            .map(|(row, offset)| {
                gauntlet_domain::SettlementRecord::new(
                    row.run_id,
                    row.unit_id,
                    row.evidence,
                    row.verification,
                    offset,
                )
            })
            .collect())
    }

    fn complete_pending_settlements(
        &self,
        run_id: &RunId,
        anchor: EventLogOffset,
        _deadline: Deadline,
    ) -> Result<usize, StateStoreError> {
        let entries = self
            .events
            .borrow()
            .get(run_id)
            .cloned()
            .unwrap_or_default();
        let mut rows = self.settlement_rows.borrow_mut();
        let mut applied = 0;
        for entry in entries {
            if entry.offset.get() < anchor.get() {
                continue;
            }
            let crate::state_store::RunEventKind::SettlementCommitted {
                unit_id,
                evidence,
                verification,
                reference_digest,
            } = entry.event.kind
            else {
                continue;
            };
            if let Some((row, _)) = rows.iter().find(|(row, _)| {
                row.run_id == entry.event.run_id
                    && row.unit_id == unit_id
                    && row.verification == verification
            }) {
                // COR2-5 (green-stub conformance): a same-key event with
                // DIVERGENT facts is a typed integrity refusal — the
                // silent skip was the pre-correction shape the shared
                // suite now refuses.
                if row.evidence != evidence || row.reference_digest != reference_digest {
                    return Err(StateStoreError::Corrupt {
                        detail: format!(
                            "the settlement key (run {}, unit {}, verification {}) is already \
                             durable with DIVERGENT facts — stored evidence {}, offered \
                             evidence {}",
                            entry.event.run_id, unit_id, verification, row.evidence, evidence
                        ),
                    });
                }
                continue;
            }
            rows.push((
                crate::state_store::SettlementCommit {
                    run_id: entry.event.run_id,
                    unit_id,
                    evidence,
                    verification,
                    reference_digest,
                    at: entry.event.at,
                },
                entry.offset,
            ));
            applied += 1;
        }
        Ok(applied)
    }

    fn export_jsonl(&self, run_id: &RunId, _deadline: Deadline) -> Result<String, StateStoreError> {
        let entries = self
            .events
            .borrow()
            .get(run_id)
            .cloned()
            .unwrap_or_default();
        let lines = entries
            .iter()
            .map(|entry| {
                // Valid JSON lines (the adapter's wire tags), so the
                // suite's settlement case can read the exported offsets
                // back out of any conforming store's export.
                let kind = match &entry.event.kind {
                    crate::state_store::RunEventKind::SettlementCommitted { .. } => {
                        "settlement_committed"
                    }
                    crate::state_store::RunEventKind::InvocationAdmitted { .. } => {
                        "invocation_admitted"
                    }
                    crate::state_store::RunEventKind::InvocationTerminal { .. } => {
                        "invocation_terminal"
                    }
                    crate::state_store::RunEventKind::CheckpointWritten { .. } => {
                        "checkpoint_written"
                    }
                    crate::state_store::RunEventKind::EvidenceProduced { .. } => {
                        "evidence_produced"
                    }
                };
                format!(
                    "{{\"offset\":{},\"at\":{},\"kind\":\"{kind}\"}}",
                    entry.offset.get(),
                    entry.event.at.get()
                )
            })
            .collect::<Vec<_>>()
            .join("\n");
        Ok(lines)
    }
}

pub struct StateStoreGreen;

impl StateStoreFactory for StateStoreGreen {
    fn label(&self) -> &'static str {
        "green-durable-store"
    }

    fn build(&self) -> Result<Box<dyn StateStore>, String> {
        Ok(Box::new(DurableStore {
            recorded_runs: RefCell::new(HashMap::new()),
            checkpoints: RefCell::new(HashMap::new()),
            events: RefCell::new(HashMap::new()),
            productions: RefCell::new(Vec::new()),
            bindings: RefCell::new(HashMap::new()),
            selections: RefCell::new(HashMap::new()),
            verifications: RefCell::new(Vec::new()),
            settlement_rows: RefCell::new(Vec::new()),
            next_offset: Cell::new(1),
        }))
    }
}

/// The settlement-surface FAKE (plan Task 13 teeth): a DurableStore whose
/// `commit_settlement` reports success without appending the event or
/// keeping the row, and whose replay always claims zero — the store that
/// "implements" the surface while doing none of it. The suite must catch
/// it on the earned-commit case (nothing reads back) and the replay case.
struct SettlementLaxStore {
    inner: DurableStore,
}

impl StateStore for SettlementLaxStore {
    fn append_event(
        &self,
        event: RunEvent,
        deadline: Deadline,
    ) -> Result<EventLogOffset, StateStoreError> {
        self.inner.append_event(event, deadline)
    }
    fn load_run(&self, run_id: &RunId, deadline: Deadline) -> Result<Option<Run>, StateStoreError> {
        self.inner.load_run(run_id, deadline)
    }
    fn record_run(&self, run: &Run, deadline: Deadline) -> Result<(), StateStoreError> {
        self.inner.record_run(run, deadline)
    }
    fn checkpoint(
        &self,
        checkpoint: Checkpoint,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner.checkpoint(checkpoint, deadline)
    }
    fn resume(
        &self,
        run_id: &RunId,
        deadline: Deadline,
    ) -> Result<Option<Checkpoint>, StateStoreError> {
        self.inner.resume(run_id, deadline)
    }
    fn record_evidence_production(
        &self,
        run_id: &RunId,
        production: &crate::producer_authority::AdmittedEvidenceProduction,
        deadline: Deadline,
    ) -> Result<crate::state_store::EvidenceProductionBinding, StateStoreError> {
        self.inner
            .record_evidence_production(run_id, production, deadline)
    }
    fn evidence_production(
        &self,
        run_id: &RunId,
        evidence: &EvidenceRef,
        deadline: Deadline,
    ) -> Result<Option<crate::state_store::EvidenceProductionBinding>, StateStoreError> {
        self.inner.evidence_production(run_id, evidence, deadline)
    }
    fn bind_observation_bundle(
        &self,
        run_id: &RunId,
        iteration_no: gauntlet_domain::IterationNo,
        binding: &crate::producer_authority::AdmittedObservationBinding,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner
            .bind_observation_bundle(run_id, iteration_no, binding, deadline)
    }
    fn observation_binding(
        &self,
        run_id: &RunId,
        iteration_no: gauntlet_domain::IterationNo,
        deadline: Deadline,
    ) -> Result<Option<crate::state_store::ObservationBinding>, StateStoreError> {
        self.inner
            .observation_binding(run_id, iteration_no, deadline)
    }
    fn record_correction_selection(
        &self,
        selection: &gauntlet_domain::CorrectionSelection,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner.record_correction_selection(selection, deadline)
    }
    fn correction_selections(
        &self,
        run_id: &RunId,
        deadline: Deadline,
    ) -> Result<Vec<gauntlet_domain::CorrectionSelection>, StateStoreError> {
        self.inner.correction_selections(run_id, deadline)
    }
    fn record_verification(
        &self,
        record: &crate::producer_authority::AdmittedVerificationRecord,
        deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        self.inner.record_verification(record, deadline)
    }
    fn verification_records(
        &self,
        run_id: &RunId,
        deadline: Deadline,
    ) -> Result<Vec<crate::verification_record::VerificationRecordDoc>, StateStoreError> {
        self.inner.verification_records(run_id, deadline)
    }
    fn settlements(
        &self,
        run_id: &RunId,
        deadline: Deadline,
    ) -> Result<Vec<gauntlet_domain::SettlementRecord>, StateStoreError> {
        self.inner.settlements(run_id, deadline)
    }
    // The fakes: the commit "succeeds" having done nothing, and the replay
    // always reports an empty window.
    fn commit_settlement(
        &self,
        _admitted: &crate::producer_authority::AdmittedSettlement,
        _deadline: Deadline,
    ) -> Result<EventLogOffset, StateStoreError> {
        Ok(EventLogOffset::new(1))
    }
    fn complete_pending_settlements(
        &self,
        _run_id: &RunId,
        _anchor: EventLogOffset,
        _deadline: Deadline,
    ) -> Result<usize, StateStoreError> {
        Ok(0)
    }
    fn export_jsonl(&self, run_id: &RunId, deadline: Deadline) -> Result<String, StateStoreError> {
        self.inner.export_jsonl(run_id, deadline)
    }
}

pub struct StateStoreSettlementLax;

impl StateStoreFactory for StateStoreSettlementLax {
    fn label(&self) -> &'static str {
        "broken-settlement-lax-store"
    }

    fn build(&self) -> Result<Box<dyn StateStore>, String> {
        Ok(Box::new(SettlementLaxStore {
            inner: DurableStore {
                recorded_runs: RefCell::new(HashMap::new()),
                checkpoints: RefCell::new(HashMap::new()),
                events: RefCell::new(HashMap::new()),
                productions: RefCell::new(Vec::new()),
                bindings: RefCell::new(HashMap::new()),
                selections: RefCell::new(HashMap::new()),
                verifications: RefCell::new(Vec::new()),
                settlement_rows: RefCell::new(Vec::new()),
                next_offset: Cell::new(1),
            },
        }))
    }
}

struct FrozenStore;

impl StateStore for FrozenStore {
    fn append_event(
        &self,
        _event: RunEvent,
        _deadline: Deadline,
    ) -> Result<EventLogOffset, StateStoreError> {
        Ok(EventLogOffset::new(1))
    }

    fn load_run(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Option<Run>, StateStoreError> {
        Ok(None)
    }

    fn checkpoint(
        &self,
        _checkpoint: Checkpoint,
        _deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        Ok(())
    }

    fn resume(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Option<Checkpoint>, StateStoreError> {
        Ok(None)
    }

    fn export_jsonl(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<String, StateStoreError> {
        Ok("{\"offset\":1}".to_owned())
    }
}

pub struct StateStoreBroken;

impl StateStoreFactory for StateStoreBroken {
    fn label(&self) -> &'static str {
        "broken-frozen-store"
    }

    fn build(&self) -> Result<Box<dyn StateStore>, String> {
        Ok(Box::new(FrozenStore))
    }
}

// ---------------------------------------------------------------------------
// tool_provider
// ---------------------------------------------------------------------------

struct SmallToolbox {
    resolvable: Vec<SemanticCapability>,
}

impl ToolProvider for SmallToolbox {
    fn resolve(
        &self,
        capability: SemanticCapability,
        _deadline: Deadline,
    ) -> Result<ToolBinding, ToolProviderError> {
        if !self.resolvable.contains(&capability) {
            return Err(ToolProviderError::Unresolved { capability });
        }
        Ok(ToolBinding {
            capability,
            identity: ToolIdentity::new("suite-tool").expect("stub tool identity"),
        })
    }

    fn available(&self) -> Vec<SemanticCapability> {
        self.resolvable.clone()
    }
}

pub struct ToolProviderGreen;

impl ToolProviderFactory for ToolProviderGreen {
    fn label(&self) -> &'static str {
        "green-small-toolbox"
    }

    fn build(&self, resolvable: Vec<SemanticCapability>) -> Result<Box<dyn ToolProvider>, String> {
        Ok(Box::new(SmallToolbox { resolvable }))
    }
}

struct OmnipotentToolbox;

impl ToolProvider for OmnipotentToolbox {
    fn resolve(
        &self,
        capability: SemanticCapability,
        _deadline: Deadline,
    ) -> Result<ToolBinding, ToolProviderError> {
        Ok(ToolBinding {
            capability,
            identity: ToolIdentity::new("suite-tool").expect("stub tool identity"),
        })
    }

    fn available(&self) -> Vec<SemanticCapability> {
        Vec::new()
    }
}

pub struct ToolProviderBroken;

impl ToolProviderFactory for ToolProviderBroken {
    fn label(&self) -> &'static str {
        "broken-omnipotent-toolbox"
    }

    fn build(&self, _resolvable: Vec<SemanticCapability>) -> Result<Box<dyn ToolProvider>, String> {
        Ok(Box::new(OmnipotentToolbox))
    }
}

// ---------------------------------------------------------------------------
// verifier
// ---------------------------------------------------------------------------

struct LocalVerifier {
    present: Vec<EvidenceLocator>,
}

impl Verifier for LocalVerifier {
    fn verify(
        &self,
        claim: &VerificationRequest,
        _deadline: Deadline,
    ) -> Result<VerificationRecord, VerifierError> {
        if !self.present.contains(&claim.evidence) {
            return Err(VerifierError::EvidenceMissing {
                locator: claim.evidence.clone(),
            });
        }
        let evidence =
            EvidenceRef::new(claim.evidence.as_str()).expect("a locator is a ref string");
        Ok(VerificationRecord::new(
            claim.id.clone(),
            claim.run_id.clone(),
            claim.unit_id.clone(),
            evidence,
            Outcome::Pass,
            InstrumentName::new("suite-verifier").expect("stub instrument name"),
        ))
    }

    fn is_deterministic(&self) -> bool {
        true
    }

    fn instrument(&self) -> crate::observer::InstrumentIdentity {
        crate::observer::InstrumentIdentity::new(
            InstrumentName::new("suite-verifier").expect("stub instrument name"),
            "1".to_owned(),
        )
    }
}

pub struct VerifierGreen;

impl VerifierFactory for VerifierGreen {
    fn label(&self) -> &'static str {
        "green-local-verifier"
    }

    fn build(&self, present: Vec<EvidenceLocator>) -> Result<Box<dyn Verifier>, String> {
        Ok(Box::new(LocalVerifier { present }))
    }

    fn deterministic(&self) -> bool {
        true
    }
}

struct FabricatingVerifier;

impl Verifier for FabricatingVerifier {
    fn verify(
        &self,
        claim: &VerificationRequest,
        _deadline: Deadline,
    ) -> Result<VerificationRecord, VerifierError> {
        let evidence =
            EvidenceRef::new(claim.evidence.as_str()).expect("a locator is a ref string");
        Ok(VerificationRecord::new(
            claim.id.clone(),
            claim.run_id.clone(),
            claim.unit_id.clone(),
            evidence,
            Outcome::Pass,
            InstrumentName::new("suite-verifier").expect("stub instrument name"),
        ))
    }

    fn is_deterministic(&self) -> bool {
        true
    }

    fn instrument(&self) -> crate::observer::InstrumentIdentity {
        crate::observer::InstrumentIdentity::new(
            InstrumentName::new("suite-verifier").expect("stub instrument name"),
            "1".to_owned(),
        )
    }
}

pub struct VerifierBroken;

impl VerifierFactory for VerifierBroken {
    fn label(&self) -> &'static str {
        "broken-fabricating-verifier"
    }

    fn build(&self, _present: Vec<EvidenceLocator>) -> Result<Box<dyn Verifier>, String> {
        Ok(Box::new(FabricatingVerifier))
    }

    fn deterministic(&self) -> bool {
        true
    }
}

// ---------------------------------------------------------------------------
// workspace_provider
// ---------------------------------------------------------------------------

struct RootedWorkspaces {
    root: PathBuf,
    created: RefCell<HashSet<WorkspaceRef>>,
    next: Cell<u64>,
}

impl WorkspaceProvider for RootedWorkspaces {
    fn create(
        &self,
        _run_id: &RunId,
        _subject: WorkspaceSubject,
        _deadline: Deadline,
    ) -> Result<Workspace, WorkspaceProviderError> {
        let n = self.next.get();
        self.next.set(n + 1);
        let key = format!("suite-ws-{n:04}");
        let reference = WorkspaceRef::new(&key).expect("stub workspace ref");
        self.created.borrow_mut().insert(reference.clone());
        Ok(Workspace {
            reference,
            root: self.root.join(&key),
            key,
        })
    }

    fn cleanup(
        &self,
        workspace: &WorkspaceRef,
        _deadline: Deadline,
    ) -> Result<(), WorkspaceProviderError> {
        if self.created.borrow_mut().remove(workspace) {
            Ok(())
        } else {
            Err(WorkspaceProviderError::CleanupFailed {
                detail: format!("{} was not created here", workspace.as_str()),
            })
        }
    }

    fn hooks(&self) -> Vec<WorkspaceHook> {
        Vec::new()
    }
}

pub struct WorkspaceProviderGreen;

/// The suite's workspace root vocabulary, referenced not written (a rooted
/// `"/"` separator and a single-segment name compose the path at runtime).
const SUITE_ROOT_DIR: &str = "suite-root";
const SUITE_WS_KEY: &str = "suite-ws-same";

impl WorkspaceProviderFactory for WorkspaceProviderGreen {
    fn label(&self) -> &'static str {
        "green-rooted-workspaces"
    }

    fn root(&self) -> PathBuf {
        PathBuf::from("/").join(SUITE_ROOT_DIR)
    }

    fn build(&self) -> Result<Box<dyn WorkspaceProvider>, String> {
        Ok(Box::new(RootedWorkspaces {
            root: PathBuf::from("/").join(SUITE_ROOT_DIR),
            created: RefCell::new(HashSet::new()),
            next: Cell::new(1),
        }))
    }
}

struct SharedWorkspaces;

impl WorkspaceProvider for SharedWorkspaces {
    fn create(
        &self,
        _run_id: &RunId,
        _subject: WorkspaceSubject,
        _deadline: Deadline,
    ) -> Result<Workspace, WorkspaceProviderError> {
        Ok(Workspace {
            reference: WorkspaceRef::new(SUITE_WS_KEY).expect("stub workspace ref"),
            root: PathBuf::from("/").join(SUITE_ROOT_DIR).join(SUITE_WS_KEY),
            key: SUITE_WS_KEY.to_owned(),
        })
    }

    fn cleanup(
        &self,
        _workspace: &WorkspaceRef,
        _deadline: Deadline,
    ) -> Result<(), WorkspaceProviderError> {
        Ok(())
    }

    fn hooks(&self) -> Vec<WorkspaceHook> {
        Vec::new()
    }
}

pub struct WorkspaceProviderBroken;

impl WorkspaceProviderFactory for WorkspaceProviderBroken {
    fn label(&self) -> &'static str {
        "broken-shared-workspaces"
    }

    fn root(&self) -> PathBuf {
        PathBuf::from("/").join(SUITE_ROOT_DIR)
    }

    fn build(&self) -> Result<Box<dyn WorkspaceProvider>, String> {
        Ok(Box::new(SharedWorkspaces))
    }
}
