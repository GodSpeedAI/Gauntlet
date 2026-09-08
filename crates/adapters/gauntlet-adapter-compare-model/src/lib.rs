//! Model-backed comparator adapter (spec §4.1, §13; plan Task 10): a
//! fresh-context critic dispatched through an `AgentRunner`, whose result
//! is strict-parsed from the `role_result@v1` contract at the declared
//! path (spec §10.3, Annex B.1). The composition root injects the runner
//! (mock for tests, a real runner in production) — this adapter never
//! names one.
//!
//! Flow (`compare`): validate the envelope's contract version → build the
//! critic `RoleInvocationSpec` (role critic; `context_envelope_digest` =
//! the envelope's digest, §5.3.5/§13.2; a result-contract path derived
//! from the invocation id) → `invoke` → `poll` to a terminal status
//! within the deadline → read and strict-parse `role_result@v1` at the
//! path → the payload MUST be the `Comparison` variant → convert to the
//! domain record.
//!
//! Failure taxonomy (uncertainty is NEVER converted into success, spec
//! §21.2):
//!
//! - unknown contract version ⇒ [`ComparatorError::EnvelopeInvalid`]
//!   (checked before any dispatch);
//! - no terminal status within the bounded poll run, or the runner
//!   reporting `TimedOut` ⇒ [`ComparatorError::TimedOut`];
//! - the child completing without a readable result document, a malformed
//!   document, a `Failed`/`Cancelled` terminal status, or a record whose
//!   residuals contradict its verdict ⇒ [`ComparatorError::ComparisonFailed`];
//! - wrong `invocation_id`, wrong `context_envelope_digest`, or a
//!   non-comparison payload ⇒ [`ComparatorError::ContractMismatch`]
//!   (spec §10.3 rule 6, `contract_mismatch` class).
//!
//! The bounded poll loop follows the repository's documented time-free
//! convention (Task 8: "probe counts stand in for §10.3 rule 5 deadlines"
//! — `collect` carries the real deadline); see [`PROBE_CEILING`].

use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};

use gauntlet_domain::{
    Budget, ComparisonRecord, ModelIdentity, PermissionSetRef, Residual, ResultContractPath,
    RoleId, Severity, WorkspaceRef,
};
use gauntlet_ports::agent_runner::{
    AgentRunner, ContextEnvelope, InvocationStatus, RoleInvocationSpec,
};
use gauntlet_ports::clock::Clock;
use gauntlet_ports::comparator::{
    AdversarialComparator, AdversarialComparatorSource, Comparator, ComparatorError,
    ComparatorSource, ComparisonEnvelope, PendingComparison, TrustedComparisonContext,
};
use gauntlet_ports::comparison_record::{ComparisonRecordDoc, ResidualDoc};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::id_generator::IdGenerator;

/// The comparison-contract version this adapter runs under (must equal the
/// frozen artifact's, spec §9.5).
pub const SUPPORTED_CONTRACT_VERSION: &str = "1";

/// This adapter's own identity (AK-028-class provenance).
pub const ADAPTER_IDENTITY: &str = "gauntlet-adapter-compare-model";

/// The bounded poll run (Task-8 convention): polls are non-blocking, so a
/// count stands in for wall-clock patience; `collect` carries the real
/// deadline.
pub const PROBE_CEILING: u32 = 24;

/// The standard critic instruction (spec §13.1: the comparator "answers
/// exactly one question: how does observed reality differ from the
/// intended reference?").
pub const CRITIC_PROMPT: &str = "compare the observed reality in the context envelope \
     against the reference view and record every difference as a residual in the \
     comparison_record@v1 result contract at the declared path; produce residuals only";

/// The adversarial instruction (spec §13.3 `high_assurance`: "an
/// adversarial comparator instructed to find the strongest case the work
/// does NOT meet the reference").
pub const ADVERSARIAL_PROMPT: &str = "find the STRONGEST case that the observed reality in \
     the context envelope does NOT meet the reference view, and record it as residuals in \
     the comparison_record@v1 result contract at the declared path";

/// The model-backed comparator over a runner (plan Task 10 D5.2).
#[derive(Clone)]
pub struct ModelComparator {
    runner: Arc<dyn AgentRunner>,
    ids: Arc<dyn IdGenerator>,
    model_hint: Option<ModelIdentity>,
    adversarial: bool,
    dimension: Option<String>,
    budget_slice: Budget,
    clock: Arc<dyn Clock>,
}

/// Regular model comparator factory for the application review port.
#[derive(Clone)]
pub struct ModelComparatorSource {
    comparator: ModelComparator,
}

impl ModelComparatorSource {
    pub fn new(comparator: ModelComparator) -> Self {
        Self { comparator }
    }
}

impl ComparatorSource for ModelComparatorSource {
    fn across_dimensions(&self) -> Result<Box<dyn Comparator>, String> {
        Ok(Box::new(self.comparator.clone()))
    }

    fn for_dimension(&self, dimension: &str) -> Result<Box<dyn Comparator>, String> {
        Ok(Box::new(self.comparator.clone().for_dimension(dimension)))
    }
}

/// The only model-adapter factory for the semantic high-assurance capability.
/// Its construction calls [`ModelComparator::adversarial`] directly rather than
/// trusting a source label or accepting a normal comparator.
#[derive(Clone)]
pub struct ModelAdversarialSource {
    comparator: ModelComparator,
}

impl ModelAdversarialSource {
    pub fn from_model(comparator: ModelComparator) -> Self {
        Self {
            comparator: comparator.adversarial(),
        }
    }
}

impl AdversarialComparatorSource for ModelAdversarialSource {
    fn adversarial_across_dimensions(&self) -> Result<AdversarialComparator, String> {
        // The wrapped comparator was built by `ModelAdversarialSource::from_model`,
        // which calls `ModelComparator::adversarial()` directly — the instruction
        // is enabled here, not asserted by a label.
        Ok(AdversarialComparator::instructed(Box::new(
            self.comparator.clone(),
        )))
    }
}

impl ModelComparator {
    /// The critic comparator (standard instruction).
    pub fn new(
        runner: Arc<dyn AgentRunner>,
        ids: Arc<dyn IdGenerator>,
        model_hint: Option<ModelIdentity>,
        budget_slice: Budget,
    ) -> Self {
        Self::with_clock(runner, ids, model_hint, budget_slice, Arc::new(SystemClock))
    }

    /// Constructs the adapter with an injected clock. Production composition
    /// uses the system-clock adapter above; deterministic tests and hosts that
    /// already own a clock must use this constructor so deadline checks are
    /// observable rather than inferred from a probe count.
    pub fn with_clock(
        runner: Arc<dyn AgentRunner>,
        ids: Arc<dyn IdGenerator>,
        model_hint: Option<ModelIdentity>,
        budget_slice: Budget,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            runner,
            ids,
            model_hint,
            adversarial: false,
            dimension: None,
            budget_slice,
            clock,
        }
    }

    /// The adversarial comparator (§13.3 `high_assurance`).
    pub fn adversarial(mut self) -> Self {
        self.adversarial = true;
        self
    }

    /// Scopes the comparator to one dimension (§13.3 fanout/adaptive — the
    /// dimension travels in the instruction, not in the §13.2 envelope).
    pub fn for_dimension(mut self, dimension: &str) -> Self {
        self.dimension = Some(dimension.to_owned());
        self
    }

    fn prompt(&self) -> String {
        let base = if self.adversarial {
            ADVERSARIAL_PROMPT
        } else {
            CRITIC_PROMPT
        };
        match &self.dimension {
            Some(dimension) => format!("{base}; judge the {dimension} dimension only"),
            None => base.to_owned(),
        }
    }

    /// The full flow, returning the critic's `comparison_record@v1`
    /// document (the wire form; `compare` converts it to the domain
    /// record).
    fn admit_comparison(
        &self,
        context: &TrustedComparisonContext,
        deadline: Deadline,
    ) -> Result<PendingModelComparison, ComparatorError> {
        let envelope = context.envelope();
        if envelope.contract().version() != SUPPORTED_CONTRACT_VERSION {
            return Err(ComparatorError::EnvelopeInvalid {
                detail: format!(
                    "contract version {} is not {} — never a guessed run",
                    envelope.contract().version(),
                    SUPPORTED_CONTRACT_VERSION
                ),
            });
        }
        if self.runner.capabilities().isolation < gauntlet_ports::agent_runner::Isolation::Process {
            return Err(ComparatorError::EnvelopeInvalid {
                detail: "a model critic requires process-or-stronger runner isolation;                          isolation:none cannot establish fresh-context containment (spec 13.2)"
                    .to_owned(),
            });
        }
        let invocation_id = self.ids.invocation_id();
        let result_contract_path =
            ResultContractPath::new(&format!("critic-comparison-{}", invocation_id.as_str()))
                .expect("the path grammar holds: an invocation id is plain ref material");
        let prompt = self.prompt();
        // The adapter—not its caller—names a fresh per-invocation workspace
        // and an envelope-only permission set. Consequently a builder
        // workspace/permission cannot be accidentally reused by this critic.
        let workspace = WorkspaceRef::new(&format!("critic-comparison-{}", invocation_id.as_str()))
            .expect("an invocation id is valid workspace-ref material");
        let permissions = PermissionSetRef::new("comparison-envelope-only")
            .expect("the fixed envelope-only permission ref is valid");
        let spec = RoleInvocationSpec {
            invocation_id: invocation_id.clone(),
            run_id: envelope.run_id().clone(),
            iteration_no: envelope.iteration_no(),
            role: RoleId::Critic,
            prompt,
            context_envelope: ContextEnvelope::new(
                context.envelope().to_json(),
                envelope.envelope_digest().clone(),
            ),
            workspace,
            permissions,
            result_contract_path,
            deadline,
            model_hint: self.model_hint.clone(),
            sandbox_policy: None,
            budget_slice: self.budget_slice.clone(),
        };
        let handle = self.runner.invoke(spec.clone()).map_err(|error| {
            ComparatorError::ComparisonFailed {
                detail: format!("admitting the critic failed: {error}"),
            }
        })?;
        Ok(PendingModelComparison {
            runner: self.runner.clone(),
            handle,
            spec,
            envelope: envelope.clone(),
            dimension: self.dimension.clone(),
            clock: self.clock.clone(),
            timed_out_cancelled: false,
        })
    }

    /// Compatibility single-comparison API: admit then collect immediately.
    pub fn comparison_document(
        &self,
        context: &TrustedComparisonContext,
        deadline: Deadline,
    ) -> Result<ComparisonRecordDoc, ComparatorError> {
        let mut pending = self.admit_comparison(context, deadline)?;
        match pending.collect(deadline) {
            Ok(document) => Ok(document),
            Err(original) => match pending.cancel(deadline) {
                Ok(()) => Err(original),
                Err(cancellation) => Err(ComparatorError::RecoveryRequired {
                    detail: format!(
                        "comparison failed ({original}) and cancelling its admitted critic could not confirm termination: {cancellation}"
                    ),
                }),
            },
        }
    }
}

/// State retained between admission and fan-in collection. It owns the exact
/// spec and handle created at admission; no later call can substitute ids,
/// paths, or envelope digest.
struct PendingModelComparison {
    runner: Arc<dyn AgentRunner>,
    handle: gauntlet_ports::agent_runner::InvocationHandle,
    spec: RoleInvocationSpec,
    envelope: ComparisonEnvelope,
    dimension: Option<String>,
    clock: Arc<dyn Clock>,
    /// Avoids a second cancellation by the review recovery path after this
    /// pending comparison has already cancelled itself on deadline expiry.
    timed_out_cancelled: bool,
}

impl PendingModelComparison {
    fn cancel_with_reason(
        &mut self,
        reason: &str,
        deadline: Deadline,
    ) -> Result<(), ComparatorError> {
        let reason = gauntlet_ports::agent_runner::CancelReason::new(reason)
            .expect("fixed cancellation reason");
        match self.runner.cancel(&self.handle, reason, deadline) {
            Ok(()) | Err(gauntlet_ports::agent_runner::AgentError::AlreadyTerminal { .. }) => {
                Ok(())
            }
            Err(error) => Err(ComparatorError::ComparisonFailed {
                detail: format!(
                    "cancelling uncollected critic {} failed: {error}",
                    self.spec.invocation_id
                ),
            }),
        }
    }

    fn cancel_for_deadline(
        &mut self,
        deadline: Deadline,
    ) -> Result<ComparisonRecordDoc, ComparatorError> {
        match self.cancel_with_reason("comparison deadline expired before the next poll", deadline)
        {
            Ok(()) => {
                self.timed_out_cancelled = true;
                Err(ComparatorError::TimedOut {
                    detail: format!(
                        "the critic {} exceeded its absolute deadline before polling",
                        self.spec.invocation_id
                    ),
                })
            }
            Err(error) => Err(ComparatorError::RecoveryRequired {
                detail: format!(
                    "the critic {} exceeded its absolute deadline and cancellation could not confirm termination: {error}",
                    self.spec.invocation_id
                ),
            }),
        }
    }
}

impl PendingComparison for PendingModelComparison {
    /// The exact §5.3.5 spec this critic was admitted under, created at
    /// admission and never substitutable afterwards. The governed review binds
    /// the collected document to this tuple at durable acceptance.
    fn invocation_spec(&self) -> Option<&RoleInvocationSpec> {
        Some(&self.spec)
    }

    fn cancel(&mut self, deadline: Deadline) -> Result<(), ComparatorError> {
        if self.timed_out_cancelled {
            return Ok(());
        }
        self.cancel_with_reason(
            "a sibling comparison failed before this admitted critic was collected",
            deadline,
        )
    }

    fn collect(&mut self, deadline: Deadline) -> Result<ComparisonRecordDoc, ComparatorError> {
        if deadline.exceeded_at(self.clock.now()) {
            return self.cancel_for_deadline(deadline);
        }
        let this = self;
        let status = match drive_to_terminal_with(
            this.runner.as_ref(),
            &this.handle,
            deadline,
            this.clock.as_ref(),
        ) {
            Ok(status) => status,
            Err(error @ ComparatorError::TimedOut { .. })
                if deadline.exceeded_at(this.clock.now()) =>
            {
                // The probe loop discovered expiry before a poll. The pending
                // owner, not a later review cleanup path, cancels immediately.
                let _ = error;
                return this.cancel_for_deadline(deadline);
            }
            Err(error) => return Err(error),
        };
        match status {
            InvocationStatus::Completed { result_ref }
                if result_ref == this.spec.result_contract_path => {}
            InvocationStatus::Completed { result_ref } => {
                return Err(ComparatorError::ContractMismatch {
                    detail: format!(
                        "the critic {} completed at {result_ref}, not its declared result contract path {}",
                        this.spec.invocation_id, this.spec.result_contract_path
                    ),
                });
            }
            InvocationStatus::TimedOut => {
                return Err(ComparatorError::TimedOut {
                    detail: format!("the critic {} timed out", this.spec.invocation_id),
                });
            }
            InvocationStatus::Failed { class } => {
                return Err(ComparatorError::ComparisonFailed {
                    detail: format!("the critic {} failed: {class}", this.spec.invocation_id),
                });
            }
            InvocationStatus::Cancelled => {
                return Err(ComparatorError::ComparisonFailed {
                    detail: format!("the critic {} was cancelled", this.spec.invocation_id),
                });
            }
            InvocationStatus::Admitted | InvocationStatus::Running { .. } => {
                return Err(ComparatorError::TimedOut {
                    detail: format!(
                        "the critic {} did not reach terminal state",
                        this.spec.invocation_id
                    ),
                });
            }
        }
        if deadline.exceeded_at(this.clock.now()) {
            return this.cancel_for_deadline(deadline);
        }
        let result = this
            .runner
            .collect(&this.handle, deadline)
            .map_err(|error| ComparatorError::ComparisonFailed {
                detail: format!(
                    "collecting critic {}'s result contract failed: {error}",
                    this.spec.invocation_id
                ),
            })?;
        let binding_ok = result.invocation_id() == &this.spec.invocation_id
            && result.run_id() == &this.spec.run_id
            && result.iteration_no() == this.spec.iteration_no
            && result.role() == &RoleId::Critic
            && result.context_envelope_digest() == this.spec.context_envelope.digest()
            && result.state() == gauntlet_ports::role_result::RoleResultState::Completed;
        if !binding_ok {
            return Err(ComparatorError::ContractMismatch {
                detail: format!(
                    "the collected critic result does not match completed dispatch {}",
                    this.spec.invocation_id
                ),
            });
        }
        let gauntlet_ports::role_result::RolePayload::Comparison(document) = result.payload()
        else {
            return Err(ComparatorError::ContractMismatch {
                detail: "the critic payload is not comparison_record@v1".to_owned(),
            });
        };
        let document = (**document).clone();
        validate_document_binding(&document, &this.envelope, this.dimension.as_deref())?;
        // Durable acceptance is NOT performed here. The adapter surfaces its
        // admitted invocation through `invocation_spec`, and the governed
        // review accepts the collected product against that tuple. An adapter
        // that accepted its own output would be deciding what counts.
        Ok(document)
    }
}

/// The interval between non-terminal polls in [`drive_to_terminal_with`]'s
/// bounded probe run (task26-dcritic-apparatus-repair B4 change 1). Kept
/// local to this adapter rather than imported from `gauntlet-app`'s
/// `config::defaults::POLL_INTERVAL_MS`: an adapter crate does not depend
/// on the application layer (the composition-root-only dependency
/// direction the architecture boundary suite polices), so this is a
/// LOCAL constant, deliberately set to the same 10ms cadence
/// `gauntlet-adapter-agent-cli`'s own transport poll loop already uses
/// (`ProcessChildTransport`'s liveness poll) rather than an unrelated
/// value invented for this adapter alone.
///
/// Effective bounded wall-clock window this gives a real (non-mock)
/// critic child process to reach a terminal state before
/// [`drive_to_terminal_with`] gives up and returns `TimedOut`:
/// `PROBE_CEILING * POLL_INTERVAL_MS` = 24 * 10ms = 240ms, plus whatever
/// the polls themselves cost — a REAL bound, never assumed instantaneous.
/// Before this constant existed the loop polled with NO delay between
/// iterations: 24 back-to-back non-blocking `poll()` calls complete in
/// far under 1ms of wall-clock, which is not enough time for ANY real
/// child process (fork, exec, do its work, write its result file) to
/// reach a terminal state — the loop would return `TimedOut` against a
/// live OS child almost unconditionally, never against a `MockAgentRunner`
/// whose `poll()` can report `Completed` on the very first call. See
/// `the_probe_interval_gives_a_real_child_wall_clock_to_finish` below for
/// the RED (0ms) / GREEN (10ms) discriminating proof.
const POLL_INTERVAL_MS: u64 = 10;

fn drive_to_terminal_with(
    runner: &dyn AgentRunner,
    handle: &gauntlet_ports::agent_runner::InvocationHandle,
    deadline: Deadline,
    clock: &dyn Clock,
) -> Result<InvocationStatus, ComparatorError> {
    for _ in 0..PROBE_CEILING {
        // A deadline is absolute. Never perform even one additional probe
        // after it expires; the owner cancels before returning to recovery.
        if deadline.exceeded_at(clock.now()) {
            return Err(ComparatorError::TimedOut {
                detail: "comparison deadline expired before the next poll".to_owned(),
            });
        }
        let status = runner
            .poll(handle)
            .map_err(|error| ComparatorError::ComparisonFailed {
                detail: format!("polling the critic failed: {error}"),
            })?;
        if status.is_terminal() {
            return Ok(status);
        }
        std::thread::sleep(std::time::Duration::from_millis(POLL_INTERVAL_MS));
    }
    Err(ComparatorError::TimedOut {
        detail: format!("no terminal state within bounded probe run ({PROBE_CEILING})"),
    })
}

/// Minimal production clock kept inside this infrastructure adapter. Runtime
/// composition can inject its semantic `Clock` through `with_clock`.
struct SystemClock;
impl Clock for SystemClock {
    fn now(&self) -> gauntlet_domain::EpochSeconds {
        let seconds = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_secs();
        gauntlet_domain::EpochSeconds::new(seconds)
    }
    fn sleep_until(
        &self,
        _: gauntlet_domain::EpochSeconds,
    ) -> Result<(), gauntlet_ports::clock::ClockError> {
        Ok(())
    }
}

impl Comparator for ModelComparator {
    fn compare(
        &self,
        context: &TrustedComparisonContext,
        deadline: Deadline,
    ) -> Result<ComparisonRecord, ComparatorError> {
        let document = self.comparison_document(context, deadline)?;
        to_domain_record(&document)
    }

    fn comparison_document(
        &self,
        context: &TrustedComparisonContext,
        deadline: Deadline,
    ) -> Result<Option<ComparisonRecordDoc>, ComparatorError> {
        ModelComparator::comparison_document(self, context, deadline).map(Some)
    }

    fn admit(
        &self,
        context: &TrustedComparisonContext,
        deadline: Deadline,
    ) -> Result<Option<Box<dyn PendingComparison>>, ComparatorError> {
        self.admit_comparison(context, deadline)
            .map(|pending| Some(Box::new(pending) as Box<dyn PendingComparison>))
    }
}

/// Refuses a nested `comparison_record@v1` unless it describes exactly the
/// envelope and requested scope that this invocation dispatched. The outer
/// `role_result` binding is necessary but not sufficient: model payload is
/// untrusted external input (§10.3 rule 6, §21.2).
fn validate_document_binding(
    document: &ComparisonRecordDoc,
    envelope: &ComparisonEnvelope,
    requested_dimension: Option<&str>,
) -> Result<(), ComparatorError> {
    let equal = |field: &str, actual: String, expected: String| {
        if actual == expected {
            Ok(())
        } else {
            Err(ComparatorError::ContractMismatch {
                detail: format!(
                    "nested comparison record {field} is {actual:?}, but the dispatched envelope requires {expected:?}"
                ),
            })
        }
    };
    equal(
        "run_id",
        document.run_id().to_string(),
        envelope.run_id().to_string(),
    )?;
    equal(
        "iteration_no",
        document.iteration_no().get().to_string(),
        envelope.iteration_no().get().to_string(),
    )?;
    equal(
        "reference_digest",
        document.reference_digest().to_string(),
        envelope.reference_digest().to_string(),
    )?;
    equal(
        "observation_digest",
        document.observation_digest().to_string(),
        envelope.observation().observation_digest().to_string(),
    )?;
    equal(
        "envelope_digest",
        document.envelope_digest().to_string(),
        envelope.envelope_digest().to_string(),
    )?;
    equal(
        "comparison_contract_version",
        document.contract_version().to_owned(),
        envelope.contract().version().to_owned(),
    )?;
    if let Some(dimension) = requested_dimension {
        equal(
            "comparator_identity.dimension",
            document.identity().dimension().to_owned(),
            dimension.to_owned(),
        )?;
        for residual in document.residuals() {
            equal(
                "residual.dimension",
                residual.dimension().to_owned(),
                dimension.to_owned(),
            )?;
        }
    }
    Ok(())
}

/// Converts the critic's document to the domain record. The domain
/// constructor enforces the §13.5 coupling (a blocking residual forces a
/// fail verdict), so a critic document whose verdict contradicts its own
/// residuals is refused here — the wire form round-trips, but it is never
/// admitted as consequential state (spec §21.2: model output is untrusted
/// external input).
fn to_domain_record(document: &ComparisonRecordDoc) -> Result<ComparisonRecord, ComparatorError> {
    let residuals = document
        .residuals()
        .iter()
        .map(domain_residual)
        .collect::<Vec<_>>();
    ComparisonRecord::new(
        document.reference_digest().clone(),
        document.observation_digest().clone(),
        document.envelope_digest().clone(),
        residuals,
        document.verdict().status(),
    )
    .map_err(|error| ComparatorError::ComparisonFailed {
        detail: format!("the critic's record is internally inconsistent: {error}"),
    })
}

fn domain_residual(residual: &ResidualDoc) -> Residual {
    Residual::new(
        residual.id().clone(),
        residual.severity(),
        format!(
            "expected {}; observed {}; {}",
            residual.expected(),
            residual.observed(),
            residual.diagnosis()
        ),
        Some(residual.dimension().to_owned()).filter(|dimension| !dimension.is_empty()),
    )
}

/// Whether a severity is expressible as a domain residual at all (kept
/// explicit for the doc-to-domain path's readability).
#[allow(dead_code)]
fn severity_is_domain_vocabulary(severity: Severity) -> bool {
    Severity::ALL.contains(&severity)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::rc::Rc;
    use std::sync::Arc;

    use super::{
        ADVERSARIAL_PROMPT, CRITIC_PROMPT, ModelAdversarialSource, ModelComparator,
        ModelComparatorSource, PROBE_CEILING, SUPPORTED_CONTRACT_VERSION,
    };
    use gauntlet_domain::{
        Budget, DiagnosticAllowance, Digest, EpochSeconds, EvidenceKind, EvidenceRef, IterationNo,
        Limits, MicroUsd, ReferenceId, ReferenceVersion, ResidualId, RunId, Severity, Verdict,
    };
    use gauntlet_ports::agent_runner::{
        AgentError, AgentRunner, InvocationHandle, InvocationStatus, RoleInvocationSpec,
        RunnerCapabilities,
    };
    use gauntlet_ports::clock::{Clock, ClockError};
    use gauntlet_ports::comparator::{
        AdversarialComparatorSource, Comparator, ComparatorError, ComparatorSource, ContractView,
        FrozenReferenceProjection, RunComparisonAuthority, TrustedComparisonContext,
    };
    use gauntlet_ports::comparison_record::{
        ComparatorIdentity, ComparisonRecordDoc, RecordContent, RecordVerdict, ResidualDoc,
    };
    use gauntlet_ports::deadline::Deadline;
    use gauntlet_ports::evidence_store::{EvidenceStore, EvidenceStoreError};
    use gauntlet_ports::id_generator::IdGenerator;

    const BASE_INSTANT: u64 = 1_700_000_000;
    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const OTHER_ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAB";

    fn bound() -> Deadline {
        Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + 90))
    }

    fn budget() -> Budget {
        Budget::new(
            Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("limits"),
            20,
            DiagnosticAllowance::new(100, MicroUsd::new(50)),
        )
        .expect("budget")
    }

    /// SHA-256 of the frozen fixture view `FrozenEvidence` serves, pinned (this
    /// adapter carries no hashing dependency).
    /// `FrozenReferenceProjection::load` re-derives and checks it.
    const FROZEN_VIEW_DIGEST: &str =
        "5605fa116f3e94cfb7e1406ce5cf64b0647a1896c00e34690640325fa5163565";

    fn fixture_run() -> gauntlet_domain::Run {
        gauntlet_domain::Run::new(gauntlet_domain::RunDefinition {
            id: RunId::new(ULID).expect("valid run id"),
            created_at: EpochSeconds::new(BASE_INSTANT),
            reference_version: ReferenceVersion::with_evidence(
                ReferenceId::new("fixture-reference").expect("ref"),
                1,
                Digest::new("sha256", FROZEN_VIEW_DIGEST).expect("digest"),
                EvidenceRef::new("evd-frozen-reference").expect("ref"),
            ),
            intent: "model comparator fixture".to_owned(),
            config_fingerprint: Digest::new("sha256", &"ab".repeat(32)).expect("digest"),
            environment_fingerprint: Digest::new("sha256", &"cd".repeat(32)).expect("digest"),
            budget: budget(),
            parent_run: None,
        })
        .expect("the fixture run is valid")
    }

    /// Earns the authority from the durable fixture run and loads its frozen
    /// projection — the production calls, not a shortcut.
    fn governed(failing_facet: bool) -> (RunComparisonAuthority, FrozenReferenceProjection) {
        let run_id = RunId::new(ULID).expect("valid run id");
        let authority = RunComparisonAuthority::open(
            &TestState(failing_facet),
            &run_id,
            IterationNo::new(3).expect("iterations"),
            bound(),
        )
        .expect("the durable run yields authority");
        let projection =
            FrozenReferenceProjection::load(&authority, &FrozenEvidence(failing_facet), bound())
                .expect("the pinned frozen evidence projects");
        (authority, projection)
    }

    fn trusted_of(version: &str, failing_facet: bool) -> TrustedComparisonContext {
        let (authority, projection) = governed(failing_facet);
        TrustedComparisonContext::issue(
            &authority,
            &projection,
            &FrozenEvidence(failing_facet),
            ContractView::new(
                version.to_owned(),
                vec!["functional behavior".to_owned()],
                Vec::new(),
            ),
            Vec::new(),
            Vec::new(),
            bound(),
        )
        .expect("the governed chain mints the fixture capability")
    }

    /// Carries which bundle the fixture evidence serves, so the durable
    /// binding records the digest of THAT content (§12.4).
    struct TestState(bool);
    impl gauntlet_ports::state_store::StateStore for TestState {
        fn append_event(
            &self,
            _: gauntlet_ports::state_store::RunEvent,
            _: Deadline,
        ) -> Result<gauntlet_domain::EventLogOffset, gauntlet_ports::state_store::StateStoreError>
        {
            unreachable!()
        }
        fn load_run(
            &self,
            run_id: &RunId,
            _: Deadline,
        ) -> Result<Option<gauntlet_domain::Run>, gauntlet_ports::state_store::StateStoreError>
        {
            let run = fixture_run();
            Ok((run_id == run.id()).then_some(run))
        }
        fn checkpoint(
            &self,
            _: gauntlet_ports::state_store::Checkpoint,
            _: Deadline,
        ) -> Result<(), gauntlet_ports::state_store::StateStoreError> {
            unreachable!()
        }
        fn resume(
            &self,
            _: &RunId,
            _: Deadline,
        ) -> Result<
            Option<gauntlet_ports::state_store::Checkpoint>,
            gauntlet_ports::state_store::StateStoreError,
        > {
            unreachable!()
        }
        fn export_jsonl(
            &self,
            _: &RunId,
            _: Deadline,
        ) -> Result<String, gauntlet_ports::state_store::StateStoreError> {
            unreachable!()
        }
        /// The fixture run observed exactly once, at iteration 3. Any other
        /// iteration has NO durably bound observation — and therefore no
        /// comparison authority at all.
        fn observation_binding(
            &self,
            run_id: &RunId,
            iteration_no: IterationNo,
            _: Deadline,
        ) -> Result<
            Option<gauntlet_ports::state_store::ObservationBinding>,
            gauntlet_ports::state_store::StateStoreError,
        > {
            let bound = run_id.as_str() == ULID && iteration_no.get() == 3;
            Ok(bound.then(|| {
                let document = gauntlet_ports::json::parse(&bundle_json(self.0))
                    .expect("the fixture bundle is JSON");
                gauntlet_ports::state_store::ObservationBinding::new(
                    EvidenceRef::new(BUNDLE_REF).expect("ref"),
                    gauntlet_ports::observation_digest::observation_digest(&document),
                )
            }))
        }

        fn accept_comparison(
            &self,
            _: &gauntlet_ports::comparator::AdmittedComparison,
            _: Deadline,
        ) -> Result<(), gauntlet_ports::state_store::StateStoreError> {
            Ok(())
        }
    }
    const BUNDLE_REF: &str = "evd-observation-bundle-0001";

    /// The §5.3.6 bundle the fixture run bound at iteration 3. The envelope's
    /// observation view is projected from THESE bytes inside the port.
    fn bundle_json(failing_facet: bool) -> String {
        let facets = if failing_facet {
            "{\"kind\":\"test\",\"instrument\":{\"name\":\"suite-probe\",\
             \"version\":\"1.2.3\"},\"command\":\"re-run the declared self-check\",\
             \"exit_status\":1,\"evidence_ref\":\"evd-failing-0001\"}"
        } else {
            ""
        };
        // §12.4: the port re-derives this digest from these bytes, so the
        // fixture stamps in what they actually hash to (the normalization
        // pins the self-referential member to the empty string).
        let body = |digest: &str| {
            format!(
                "{{\"schema\":\"observation_bundle@v1\",\"run_id\":\"{ULID}\",\
                 \"iteration_no\":3,\"observation_digest\":\"{digest}\",\
                 \"facets\":[{facets}],\"unavailable_evidence\":[],\
                 \"observation_failures\":[]}}"
            )
        };
        let probe = gauntlet_ports::json::parse(&body("")).expect("the fixture bundle is JSON");
        body(&gauntlet_ports::observation_digest::observation_digest(&probe).to_string())
    }

    /// Serves the two pinned objects the governed chain reads.
    struct FrozenEvidence(bool);
    impl EvidenceStore for FrozenEvidence {
        fn put(
            &self,
            _: &[u8],
            _: EvidenceKind,
            _: Deadline,
        ) -> Result<EvidenceRef, EvidenceStoreError> {
            unreachable!("fixture only loads")
        }
        fn get(
            &self,
            reference: &EvidenceRef,
            _: Deadline,
        ) -> Result<Option<Vec<u8>>, EvidenceStoreError> {
            Ok(match reference.as_str() {
                BUNDLE_REF => Some(bundle_json(self.0).into_bytes()),
                _ => Some(b"the rendered reference\n".to_vec()),
            })
        }
        fn pin(&self, _: &RunId, _: &EvidenceRef, _: Deadline) -> Result<(), EvidenceStoreError> {
            unreachable!("fixture only loads")
        }
    }
    /// The in-crate scripted critic runner (plan Task 10 D5.4): implements
    /// `AgentRunner`, writes a `comparison_record@v1` payload inside a
    /// `role_result@v1` envelope at the declared contract path, and
    /// scripts the derailments. Chosen over extending the pinned mock
    /// scenario dialect so AK-059's eight fixtures and the suite pin stay
    /// byte-identical (see the builder report).
    ///
    /// The scripted critic genuinely READS the dispatch envelope: it
    /// parses the canonical JSON from the spec's `context_envelope`,
    /// derives its record from the facet reality it finds there, and
    /// echoes the dispatch identity (invocation id, envelope digest) — so
    /// the composition under test is the real §10.3/§13.2 path.
    enum Derailment {
        None,
        /// Completes but never writes the document.
        MissingDocument,
        /// Completes, writes syntactically invalid JSON.
        Malformed,
        /// Completes, writes a builder payload instead of a comparison.
        WrongPayload,
        /// Completes, writes a result naming a foreign invocation.
        ForeignInvocation,
        /// Never reaches a terminal status.
        Hang,
        /// Terminal TimedOut.
        Timeout,
        /// Outer role-result identity is valid but the nested comparison
        /// record names a different envelope.
        CrossWiredEnvelope,
        /// The runner cannot return the declared result contract.
        CollectionFailure,
        /// Reports `Running` for a fixed number of polls, then terminal
        /// (task26-dcritic-apparatus-repair B4 change 1's discriminating
        /// tooth): drives the REAL `drive_to_terminal_with` probe loop
        /// through several genuinely non-terminal polls, so the interval
        /// between them is on the wall-clock critical path — the exact
        /// shape a real (non-mock) child process forces on the loop.
        DelayedTerminal(u32),
    }

    struct ScriptedCritic {
        root: PathBuf,
        derailment: Derailment,
        specs: Rc<RefCell<Vec<RoleInvocationSpec>>>,
        cancellations: Rc<RefCell<u32>>,
        polls: Rc<RefCell<u32>>,
    }

    // Test-only single-threaded scripting state (Rc<RefCell>) behind the
    // Send+Sync `Arc<dyn AgentRunner>` the adapter holds: the composition
    // under test is single-threaded, and the production composition root
    // injects a real multi-threaded runner instead.
    impl ScriptedCritic {
        #[allow(clippy::arc_with_non_send_sync)]
        fn shared(self) -> std::sync::Arc<Self> {
            std::sync::Arc::new(self)
        }
    }

    impl ScriptedCritic {
        fn write_document(&self, spec: &RoleInvocationSpec, invocation_id: &str) {
            let envelope =
                gauntlet_ports::json::parse(spec.context_envelope.body()).expect("valid JSON");
            let text_of = |value: &gauntlet_ports::json::JsonValue| -> String {
                match value {
                    gauntlet_ports::json::JsonValue::Str(s) => s.clone(),
                    _ => String::new(),
                }
            };
            let reference_digest = envelope
                .get("reference_digest")
                .map(&text_of)
                .unwrap_or_default();
            let observation_digest = envelope
                .get("observation")
                .and_then(|observation| observation.get("observation_digest"))
                .map(&text_of)
                .unwrap_or_default();
            let failing = envelope
                .get("observation")
                .and_then(|observation| observation.get("facets"))
                .is_some_and(|facets| {
                    let gauntlet_ports::json::JsonValue::Array(facets) = facets else {
                        return false;
                    };
                    facets.iter().any(|facet| {
                        facet.get("exit_status").and_then(|value| match value {
                            gauntlet_ports::json::JsonValue::Number(
                                gauntlet_ports::json::JsonNumber::Uint(n),
                            ) => Some(i64::try_from(*n).unwrap_or(0)),
                            gauntlet_ports::json::JsonValue::Number(
                                gauntlet_ports::json::JsonNumber::NegInt(n),
                            ) => Some(*n),
                            _ => None,
                        }) == Some(1)
                    })
                });
            let residuals = if failing {
                vec![ResidualDoc::new(
                    ResidualId::new("critic-000001").expect("valid id"),
                    "functional behavior".to_owned(),
                    Vec::new(),
                    "the reference behavior".to_owned(),
                    "evd-failing-0001 exited 1".to_owned(),
                    vec![EvidenceRef::new("evd-failing-0001").expect("valid ref")],
                    Severity::Blocking,
                    0.9,
                    false,
                    "the scripted critic read the failing facet in the envelope".to_owned(),
                    String::new(),
                    Vec::new(),
                )]
            } else {
                Vec::new()
            };
            let verdict_status = if failing {
                Verdict::Fail
            } else {
                Verdict::Pass
            };
            let comparison = ComparisonRecordDoc::new(RecordContent {
                run_id: spec.run_id.clone(),
                iteration_no: spec.iteration_no,
                reference_digest: Digest::parse(&reference_digest)
                    .unwrap_or_else(|_| Digest::new("sha256", "00").expect("digest")),
                observation_digest: Digest::parse(&observation_digest)
                    .unwrap_or_else(|_| Digest::new("sha256", "01").expect("digest")),
                envelope_digest: spec.context_envelope.digest().clone(),
                contract_version: SUPPORTED_CONTRACT_VERSION.to_owned(),
                identity: ComparatorIdentity::new(
                    "scripted-critic".to_owned(),
                    "scripted-model".to_owned(),
                    String::new(),
                ),
                residuals,
                hard_gate_results: Vec::new(),
                verdict: RecordVerdict::new(
                    verdict_status,
                    "the scripted critic's judgment".to_owned(),
                ),
            });
            let comparison = if let Derailment::CrossWiredEnvelope = self.derailment {
                let mut content = comparison.content();
                content.envelope_digest = Digest::new("sha256", "ff00").expect("test digest");
                ComparisonRecordDoc::new(content)
            } else {
                comparison
            };
            let document = match self.derailment {
                Derailment::WrongPayload => {
                    // A builder payload under the builder schema token.
                    "{\"schema\":\"role_result@v1\",\"invocation_id\":\"{id}\",\"run_id\":\"{id}\"\
                     ,\"iteration_no\":1,\"role\":\"builder\",\"context_envelope_digest\":\"{digest}\"\
                     ,\"state\":\"completed\",\"payload_schema\":\"builder_result@v1\",\"payload\":{\
                     \"attempted\":[],\"changed_paths\":[],\"commands_run\":[],\"artifacts\":[],\
                     \"claims\":[],\"self_checks\":[],\"blocked_on\":[]},\"payment\":{\"input_tokens\":0,\
                     \"output_tokens\":0,\"tool_calls\":0,\"wall_seconds\":0,\"cost_usd\":0.0},\
                     \"unavailable_evidence\":[],\"errors\":[]}"
                        .replace("{id}", invocation_id)
                        .replace(
                            "{digest}",
                            spec.context_envelope.digest().to_string().as_str(),
                        )
                }
                Derailment::Malformed => "{not json".to_owned(),
                Derailment::ForeignInvocation => {
                    // A well-formed comparison record echoing a FOREIGN
                    // invocation id.
                    format!(
                        "{{\"schema\":\"role_result@v1\",\"invocation_id\":\"{OTHER_ULID}\",\
                         \"run_id\":\"{ULID}\",\"iteration_no\":3,\"role\":\"critic\",\
                         \"context_envelope_digest\":\"{}\",\
                         \"state\":\"completed\",\
                         \"payload_schema\":\"comparison_record@v1\",\"payload\":{},\
                         \"payment\":{{\"input_tokens\":0,\"output_tokens\":0,\"tool_calls\":0,\
                         \"wall_seconds\":0,\"cost_usd\":0.0}},\"unavailable_evidence\":[],\
                         \"errors\":[]}}",
                        spec.context_envelope.digest(),
                        comparison.to_payload_json(),
                    )
                }
                _ => format!(
                    "{{\"schema\":\"role_result@v1\",\"invocation_id\":\"{invocation_id}\",\
                     \"run_id\":\"{}\",\"iteration_no\":{},\"role\":\"critic\",\
                     \"context_envelope_digest\":\"{}\",\
                     \"state\":\"completed\",\
                     \"payload_schema\":\"comparison_record@v1\",\"payload\":{},\
                     \"payment\":{{\"input_tokens\":10,\"output_tokens\":20,\"tool_calls\":1,\
                     \"wall_seconds\":2,\"cost_usd\":0.0}},\"unavailable_evidence\":[],\
                     \"errors\":[]}}",
                    spec.run_id.as_str(),
                    spec.iteration_no.get(),
                    spec.context_envelope.digest(),
                    comparison.to_payload_json(),
                ),
            };
            std::fs::write(self.root.join(spec.result_contract_path.as_str()), document)
                .expect("the scripted critic writes its contract");
        }
    }

    impl AgentRunner for ScriptedCritic {
        fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
            self.specs.borrow_mut().push(spec.clone());
            if let Derailment::MissingDocument = self.derailment {
                // Completes without writing anything.
            } else {
                self.write_document(&spec, spec.invocation_id.as_str());
            }
            Ok(InvocationHandle::new(spec.invocation_id, None))
        }

        fn poll(&self, _handle: &InvocationHandle) -> Result<InvocationStatus, AgentError> {
            *self.polls.borrow_mut() += 1;
            match self.derailment {
                Derailment::Hang => Ok(InvocationStatus::Running {
                    last_event_at: EpochSeconds::new(BASE_INSTANT),
                    payment_so_far: gauntlet_domain::Payment::zero(),
                }),
                Derailment::Timeout => Ok(InvocationStatus::TimedOut),
                Derailment::DelayedTerminal(required_polls)
                    if *self.polls.borrow() < required_polls =>
                {
                    Ok(InvocationStatus::Running {
                        last_event_at: EpochSeconds::new(BASE_INSTANT),
                        payment_so_far: gauntlet_domain::Payment::zero(),
                    })
                }
                _ => {
                    let spec = self
                        .specs
                        .borrow()
                        .last()
                        .cloned()
                        .expect("invoke precedes poll");
                    Ok(InvocationStatus::Completed {
                        result_ref: spec.result_contract_path,
                    })
                }
            }
        }

        fn collect(
            &self,
            _handle: &InvocationHandle,
            _deadline: Deadline,
        ) -> Result<gauntlet_ports::role_result::RoleResult, AgentError> {
            if matches!(self.derailment, Derailment::CollectionFailure) {
                return Err(AgentError::CollectionFailed {
                    invocation_id: _handle.invocation_id().clone(),
                    detail: "scripted collection failure".to_owned(),
                });
            }
            let spec = self
                .specs
                .borrow()
                .last()
                .cloned()
                .expect("invoke precedes collect");
            let raw = std::fs::read_to_string(self.root.join(spec.result_contract_path.as_str()))
                .map_err(|error| AgentError::CollectionFailed {
                invocation_id: spec.invocation_id.clone(),
                detail: error.to_string(),
            })?;
            // The runner returns a strictly decoded document; the comparator
            // owns dispatch-specific identity binding below, so a foreign but
            // well-formed result reaches its `ContractMismatch` check.
            gauntlet_ports::role_result::RoleResult::decode(&raw).map_err(|error| {
                AgentError::CollectionFailed {
                    invocation_id: spec.invocation_id,
                    detail: error.to_string(),
                }
            })
        }

        fn cancel(
            &self,
            _handle: &InvocationHandle,
            _reason: gauntlet_ports::agent_runner::CancelReason,
            _deadline: Deadline,
        ) -> Result<(), AgentError> {
            *self.cancellations.borrow_mut() += 1;
            Ok(())
        }

        fn capabilities(&self) -> RunnerCapabilities {
            RunnerCapabilities {
                supports_native_subagents: false,
                supports_streaming_events: false,
                supports_model_selection: false,
                isolation: gauntlet_ports::agent_runner::Isolation::Process,
                result_delivery: gauntlet_ports::agent_runner::ResultDelivery::File,
                max_depth: 0,
            }
        }
    }

    /// A fixed id source (deterministic invocation ids).
    struct FixedIds;

    impl IdGenerator for FixedIds {
        fn run_id(&self) -> RunId {
            RunId::new(ULID).expect("valid run id")
        }

        fn invocation_id(&self) -> gauntlet_domain::InvocationId {
            gauntlet_domain::InvocationId::new(ULID).expect("valid invocation id")
        }
    }

    struct FixedClock(EpochSeconds);
    impl Clock for FixedClock {
        fn now(&self) -> EpochSeconds {
            self.0
        }
        fn sleep_until(&self, _: EpochSeconds) -> Result<(), ClockError> {
            Ok(())
        }
    }

    type ComparatorSetup = (
        ModelComparator,
        Rc<RefCell<Vec<RoleInvocationSpec>>>,
        PathBuf,
        Rc<RefCell<u32>>,
    );

    fn build_comparator(derailment: Derailment) -> ComparatorSetup {
        let root = std::env::temp_dir().join(format!(
            "gauntlet-compare-model-{}-{}/",
            std::process::id(),
            root_tag()
        ));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).expect("the scratch root is creatable");
        let specs = Rc::new(RefCell::new(Vec::new()));
        let cancellations = Rc::new(RefCell::new(0));
        let runner = ScriptedCritic {
            root: root.clone(),
            derailment,
            specs: specs.clone(),
            cancellations: cancellations.clone(),
            polls: Rc::new(RefCell::new(0)),
        };
        let comparator = ModelComparator::with_clock(
            runner.shared(),
            Arc::new(FixedIds),
            None,
            budget(),
            Arc::new(FixedClock(EpochSeconds::new(BASE_INSTANT))),
        );
        (comparator, specs, root, cancellations)
    }

    fn root_tag() -> u64 {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        NEXT.fetch_add(1, std::sync::atomic::Ordering::SeqCst)
    }

    #[test]
    fn the_critic_composition_couples_the_dispatch_and_the_record() {
        let (comparator, specs, _root, _cancellations) = build_comparator(Derailment::None);
        let context = trusted_of(SUPPORTED_CONTRACT_VERSION, true);
        let record = comparator
            .compare(&context, bound())
            .expect("the composed critic comparison succeeds");
        // The critic DERIVED its residual from the envelope's facet
        // reality (it read the context envelope) and the record couples
        // all three digests.
        assert_eq!(record.verdict(), Verdict::Fail);
        assert_eq!(record.residuals().len(), 1);
        assert!(record.residuals()[0].detail().contains("evd-failing-0001"));
        assert_eq!(
            record.reference_digest(),
            context.envelope().reference_digest()
        );
        assert_eq!(
            record.observation_digest(),
            context.envelope().observation().observation_digest()
        );
        assert_eq!(
            record.envelope_digest(),
            context.envelope().envelope_digest()
        );
        // The dispatch was a critic RoleInvocationSpec carrying the
        // envelope's digest and serialized body (§5.3.5/§13.2).
        let specs = specs.borrow();
        assert_eq!(specs.len(), 1);
        let spec = &specs[0];
        assert_eq!(spec.role, gauntlet_domain::RoleId::Critic);
        assert_eq!(
            spec.context_envelope.digest(),
            context.envelope().envelope_digest()
        );
        assert_eq!(
            spec.context_envelope.body(),
            context.envelope().to_json(),
            "the critic received exactly the canonical envelope bytes"
        );
        // The clean envelope yields the critic's clean judgment.
        let (comparator, _specs, _root, _cancellations) = build_comparator(Derailment::None);
        let record = comparator
            .compare(&trusted_of(SUPPORTED_CONTRACT_VERSION, false), bound())
            .expect("the clean comparison succeeds");
        assert_eq!(record.verdict(), Verdict::Pass);
        assert_eq!(record.residuals().len(), 0);
    }

    #[test]
    fn an_iteration_the_run_never_observed_has_no_comparison_authority() {
        let (_comparator, specs, _root, _cancellations) = build_comparator(Derailment::None);
        // The attacker can no longer choose the reference view (assemble reads
        // it from the verified projection) nor the observation (the chain
        // reads the ref the RUN bound). The remaining move — comparing at an
        // iteration of the same durable run that was never observed — has no
        // authority to open at all: there is nothing this comparison could be
        // about, and saying so is not a deny list.
        let run_id = RunId::new(ULID).expect("run");
        let refusal = RunComparisonAuthority::open(
            &TestState(false),
            &run_id,
            IterationNo::new(4).expect("iteration"),
            bound(),
        );
        let detail = match refusal {
            Err(ComparatorError::EnvelopeInvalid { detail }) => detail,
            other => panic!("an unobserved iteration must be refused, got {other:?}"),
        };
        assert!(
            detail.contains("no durably bound observation bundle"),
            "the refusal names the missing durable binding: {detail}"
        );
        // The observed iteration still works, so the refusal is specific.
        assert!(
            RunComparisonAuthority::open(
                &TestState(false),
                &run_id,
                IterationNo::new(3).expect("iteration"),
                bound(),
            )
            .is_ok()
        );
        assert!(
            specs.borrow().is_empty(),
            "no critic was admitted along the way"
        );
    }

    #[test]
    fn model_source_factory_distinguishes_normal_and_adversarial_prompts() {
        let (comparator, normal_specs, _root, _cancellations) = build_comparator(Derailment::None);
        let normal = ModelComparatorSource::new(comparator);
        let normal_comparator = normal
            .across_dimensions()
            .expect("normal factory supplies comparator");
        normal_comparator
            .comparison_document(&trusted_of(SUPPORTED_CONTRACT_VERSION, false), bound())
            .expect("normal comparator completes");
        assert_eq!(normal_specs.borrow()[0].prompt, CRITIC_PROMPT);

        let (comparator, adversarial_specs, _root, _cancellations) =
            build_comparator(Derailment::None);
        let adversarial = ModelAdversarialSource::from_model(comparator);
        let adversarial_comparator = adversarial
            .adversarial_across_dimensions()
            .expect("adversarial factory supplies comparator");
        adversarial_comparator
            .as_comparator()
            .comparison_document(&trusted_of(SUPPORTED_CONTRACT_VERSION, false), bound())
            .expect("adversarial comparator completes");
        assert_eq!(adversarial_specs.borrow()[0].prompt, ADVERSARIAL_PROMPT);
    }

    #[test]
    fn expired_deadline_cancels_before_the_first_poll() {
        let root = std::env::temp_dir().join(format!(
            "gauntlet-compare-model-expired-{}-{}/",
            std::process::id(),
            root_tag()
        ));
        std::fs::remove_dir_all(&root).ok();
        std::fs::create_dir_all(&root).expect("scratch root");
        let specs = Rc::new(RefCell::new(Vec::new()));
        let cancellations = Rc::new(RefCell::new(0));
        let polls = Rc::new(RefCell::new(0));
        let comparator = ModelComparator::with_clock(
            ScriptedCritic {
                root,
                derailment: Derailment::Hang,
                specs: specs.clone(),
                cancellations: cancellations.clone(),
                polls: polls.clone(),
            }
            .shared(),
            Arc::new(FixedIds),
            None,
            budget(),
            Arc::new(FixedClock(EpochSeconds::new(BASE_INSTANT + 2))),
        );
        let expired = Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + 1));
        assert!(matches!(
            comparator.comparison_document(&trusted_of(SUPPORTED_CONTRACT_VERSION, false), expired),
            Err(ComparatorError::TimedOut { .. })
        ));
        assert_eq!(
            specs.borrow().len(),
            1,
            "admission is followed by owned recovery"
        );
        assert_eq!(
            *polls.borrow(),
            0,
            "expired deadline refuses the first poll"
        );
        assert_eq!(
            *cancellations.borrow(),
            1,
            "expired collection cancels immediately"
        );
    }

    #[test]
    fn forever_running_primary_is_actively_cancelled_by_direct_model_collection() {
        let (comparator, _specs, _root, cancellations) = build_comparator(Derailment::Hang);
        assert!(matches!(
            comparator.comparison_document(&trusted_of(SUPPORTED_CONTRACT_VERSION, true), bound()),
            Err(ComparatorError::TimedOut { .. })
        ));
        assert_eq!(
            *cancellations.borrow(),
            1,
            "bounded timeout actively cancels the primary"
        );
    }

    #[test]
    fn every_derailment_is_a_typed_error_and_never_a_pass() {
        // Timeout (hang): typed TimedOut.
        let (comparator, _s, _r, _cancellations) = build_comparator(Derailment::Hang);
        assert!(matches!(
            comparator.compare(&trusted_of(SUPPORTED_CONTRACT_VERSION, true), bound()),
            Err(ComparatorError::TimedOut { .. })
        ));
        // Runner-reported timeout.
        let (comparator, _s, _r, _cancellations) = build_comparator(Derailment::Timeout);
        assert!(matches!(
            comparator.compare(&trusted_of(SUPPORTED_CONTRACT_VERSION, true), bound()),
            Err(ComparatorError::TimedOut { .. })
        ));
        // A failed validated collection is never bypassed by reopening a
        // local path.
        let (comparator, _s, _r, _cancellations) = build_comparator(Derailment::CollectionFailure);
        assert!(matches!(
            comparator.compare(&trusted_of(SUPPORTED_CONTRACT_VERSION, true), bound()),
            Err(ComparatorError::ComparisonFailed { .. })
        ));
        // No document after completion.
        let (comparator, _s, _r, _cancellations) = build_comparator(Derailment::MissingDocument);
        assert!(matches!(
            comparator.compare(&trusted_of(SUPPORTED_CONTRACT_VERSION, true), bound()),
            Err(ComparatorError::ComparisonFailed { .. })
        ));
        // Malformed document.
        let (comparator, _s, _r, _cancellations) = build_comparator(Derailment::Malformed);
        assert!(matches!(
            comparator.compare(&trusted_of(SUPPORTED_CONTRACT_VERSION, true), bound()),
            Err(ComparatorError::ComparisonFailed { .. })
        ));
        // Foreign invocation id: contract_mismatch class.
        let (comparator, _s, _r, _cancellations) = build_comparator(Derailment::ForeignInvocation);
        assert!(matches!(
            comparator.compare(&trusted_of(SUPPORTED_CONTRACT_VERSION, true), bound()),
            Err(ComparatorError::ContractMismatch { .. })
        ));
        // An outer-valid role result may not cross-wire its nested record.
        let (comparator, _s, _r, _cancellations) = build_comparator(Derailment::CrossWiredEnvelope);
        assert!(matches!(
            comparator.compare(&trusted_of(SUPPORTED_CONTRACT_VERSION, true), bound()),
            Err(ComparatorError::ContractMismatch { .. })
        ));
        // A builder payload from a critic: contract_mismatch class.
        let (comparator, _s, _r, _cancellations) = build_comparator(Derailment::WrongPayload);
        assert!(matches!(
            comparator.compare(&trusted_of(SUPPORTED_CONTRACT_VERSION, true), bound()),
            Err(ComparatorError::ContractMismatch { .. })
        ));
        // Unknown contract version: refused BEFORE any dispatch.
        let (comparator, specs, _r, _cancellations) = build_comparator(Derailment::None);
        assert!(matches!(
            comparator.compare(&trusted_of("suite-version-not-known-9", true), bound()),
            Err(ComparatorError::EnvelopeInvalid { .. })
        ));
        assert!(
            specs.borrow().is_empty(),
            "an unknown version never dispatches a critic"
        );
    }

    #[test]
    fn the_bounded_probe_run_is_recorded() {
        assert_eq!(PROBE_CEILING, 24, "the Task-8 convention ceiling");
    }

    /// task26-dcritic-apparatus-repair B4 change 1's discriminating tooth.
    ///
    /// RED (documented, not re-introduced): with NO delay between
    /// non-terminal polls, `drive_to_terminal_with`'s loop over a
    /// genuinely non-terminal (`Derailment::DelayedTerminal`) runner
    /// completes in far under 1ms of wall-clock — 24 back-to-back
    /// non-blocking calls cost nothing measurable. Reverting
    /// `POLL_INTERVAL_MS` to 0 reproduces this: the assertion below then
    /// fails (elapsed time collapses to sub-millisecond).
    ///
    /// GREEN (this test, as committed): the runner requires 3 genuinely
    /// non-terminal polls before reporting `Completed` — a real,
    /// deterministic stand-in for "the child hasn't finished yet" — so
    /// `drive_to_terminal_with` must sleep at least twice between polls 1
    /// and 3. With `POLL_INTERVAL_MS = 10`, that floor is 20ms; the
    /// assertion below requires only half that (10ms) to stay clear of
    /// scheduler jitter while remaining IMPOSSIBLE under the RED (0ms)
    /// shape by two orders of magnitude.
    #[test]
    fn the_probe_interval_gives_a_real_child_wall_clock_to_finish() {
        let (comparator, _specs, root, _cancellations) =
            build_comparator(Derailment::DelayedTerminal(3));
        let started = std::time::Instant::now();
        let result =
            comparator.comparison_document(&trusted_of(SUPPORTED_CONTRACT_VERSION, false), bound());
        let elapsed = started.elapsed();
        std::fs::remove_dir_all(&root).ok();
        result.expect("the delayed-terminal critic still completes within the probe ceiling");
        assert!(
            elapsed >= std::time::Duration::from_millis(10),
            "drive_to_terminal_with must actually wait between non-terminal polls \
             (elapsed {elapsed:?} — a 0ms interval regression would complete this in \
             microseconds against a scripted runner that answers instantly)"
        );
    }

    /// The adapter passes the UNMODIFIED shared comparator suite (spec
    /// §7.3) — the scripted critic is a genuine envelope reader, so the
    /// suite's facet-visibility case is non-vacuous here.
    #[test]
    fn the_model_comparator_passes_the_unmodified_shared_contract_suite() {
        struct Factory;
        impl gauntlet_ports::contract::comparator::ComparatorFactory for Factory {
            fn label(&self) -> &'static str {
                "gauntlet-adapter-compare-model"
            }

            fn supported_version(&self) -> String {
                SUPPORTED_CONTRACT_VERSION.to_owned()
            }

            fn build(&self) -> Result<Box<dyn Comparator>, String> {
                let (comparator, _specs, _root, _cancellations) =
                    build_comparator(Derailment::None);
                Ok(Box::new(comparator))
            }
        }
        let findings = gauntlet_ports::contract::comparator::comparator_suite(&Factory);
        assert!(
            findings.is_empty(),
            "the shared comparator suite must pass: {findings:?}"
        );
    }
}
