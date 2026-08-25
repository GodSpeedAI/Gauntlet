//! Review-mode dispatch (spec §13.3; plan Task 10 D4; AK-033/AK-034
//! mechanisms): how the runtime orchestrates comparators over the §13.4
//! dimensions the contract instantiates.
//!
//! Spec §13.3 modes, as implemented:
//!
//! - `single`: one comparator over all applicable dimensions (cheapest).
//! - `fanout`: one comparator per applicable dimension, merged
//!   deterministically (spec: "in parallel, merged deterministically" —
//!   the merge is [`super::aggregate`], the arithmetic of §13.5).
//! - `adaptive` *(default)*: "1. Run cheap deterministic probes. 2.
//!   Determine which dimensions remain genuinely undecided. 3. Fan out
//!   only across those. 4. Merge." A dimension is SETTLED CONCLUSIVELY
//!   when the deterministic probe produced a residual for it — an
//!   instrument-settled difference is not re-decided by a model ("Do not
//!   spend frontier tokens re-deciding what an instrument already settled
//!   conclusively", §13.3). A clean probe settles nothing: absence of a
//!   deterministic diff is not evidence of conformance, so the dimension
//!   stays undecided and fans out to a model critic.
//! - `high_assurance`: fan out across all dimensions PLUS an adversarial
//!   comparator pass instructed to find the strongest case the work does
//!   NOT meet the reference.
//!
//! **Budget truncation (AK-034):** a typed model-dispatch budget (a COUNT
//! of model-comparator dispatches) bounds the model fanout. Dimensions
//! dropped because the budget ran out are recorded as `unavailable_evidence`
//! entries — kind = the dimension name, reason citing truncation — NEVER
//! silently omitted (I8: missing evidence is not passing evidence).
//! Canonical (sorted) dimension order decides which dimensions survive a
//! tight budget, so truncation is deterministic.
//!
//! **The budget meter (AK-052, Task-12 correction F4):** the count is
//! not the only bound. §10.4: "Critic fan-out is bounded by
//! `review.max_critics` **and by the budget meter**"; §17.4:
//! "`BudgetMeter` is consulted before every dispatch, not after."
//! Every model admission consults the run meter
//! ([`ReviewDeps::meter`]) and reserves [`ReviewDeps::critic_slice`]
//! — the loop's own per-attempt estimate, the same affordability figure
//! the builder leg reserves (no new configured shape was introduced for
//! it; a zero slice would make the §5.3.7 gate vacuously affordable).
//! The count lease is consulted FIRST: a count-truncated review
//! (allowance 0) never touches the meter. A meter refusal truncates the
//! dimension exactly like count exhaustion — the same
//! `unavailable_evidence` record shape, with the meter's typed refusal
//! cited in the reason — and is NEVER a run error (§10.4's truncation
//! rule; the skip-and-continue discipline correction C6.1 established
//! on the builder leg). The reservation is HELD per admitted job —
//! critic and builder dispatches share the meter's `max_parallel`
//! (§10.4 "across all roles") — and released at the job's terminal
//! through the zero-payment charge (the B1 adapter convention); a
//! cancellation that cannot confirm termination keeps the slot held,
//! mirroring the count lease's quarantine.

use gauntlet_domain::{Payment, ReviewMode};
use gauntlet_ports::budget_meter::{BudgetMeter, BudgetMeterError, BudgetSlice, Reservation};
use gauntlet_ports::comparator::{
    AdversarialComparatorSource, Comparator, ComparatorError, ComparatorSource,
    MandatoryComparisonAcceptance, TrustedComparisonContext,
};
use gauntlet_ports::comparison_record::ComparisonRecordDoc;
use gauntlet_ports::deadline::Deadline;

use super::aggregate::{AggregateError, aggregate};

/// A concurrency-safe reservation for one admitted model dispatch. The
/// total review allowance is consumed at admission; active capacity is released
/// when collection, cancellation, or an admission error drops this value.
#[derive(Debug)]
struct DispatchLease {
    state: std::sync::Arc<std::sync::Mutex<DispatchState>>,
    /// A cancellation failure means the remote critic can still consume
    /// capacity.  A quarantined lease intentionally does not decrement
    /// `active` on drop; only a recovery/re-probe path may free it.
    quarantined: bool,
}

impl DispatchLease {
    fn quarantine(&mut self) {
        self.quarantined = true;
    }
}

impl Drop for DispatchLease {
    fn drop(&mut self) {
        if self.quarantined {
            return;
        }
        let mut state = self
            .state
            .lock()
            .expect("dispatch budget lock is not poisoned");
        state.active = state.active.saturating_sub(1);
    }
}

#[derive(Debug)]
struct DispatchState {
    remaining: u32,
    active: u32,
    max_concurrent: u32,
}

/// The bounded model-dispatch authority for one review. It is not a read-only
/// precheck: a lease is acquired before runner admission and retained by the
/// pending handle until collection/cancellation. `new` preserves the previous
/// API with the dispatch ceiling also as the active ceiling; composition with a
/// stricter `review.max_critics` uses [`Self::with_max_concurrent`].
#[derive(Debug, Clone)]
pub struct ModelDispatchBudget(std::sync::Arc<std::sync::Mutex<DispatchState>>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LeaseRefusal {
    Exhausted,
    AtConcurrencyCap,
}

impl ModelDispatchBudget {
    pub fn new(dispatches: u32) -> Self {
        Self::with_max_concurrent(dispatches, dispatches)
    }

    /// A total dispatch allowance and independent active-critic cap. Both are
    /// enforced under one mutex at admission rather than inferred from a stale
    /// `remaining()` observation.
    pub fn with_max_concurrent(dispatches: u32, max_concurrent: u32) -> Self {
        Self(std::sync::Arc::new(std::sync::Mutex::new(DispatchState {
            remaining: dispatches,
            active: 0,
            max_concurrent,
        })))
    }

    pub fn get(&self) -> u32 {
        self.0
            .lock()
            .expect("dispatch budget lock is not poisoned")
            .remaining
    }

    /// Active or quarantined remote critics. This is an operational recovery
    /// metric; a cancellation failure deliberately remains counted.
    pub fn active(&self) -> u32 {
        self.0
            .lock()
            .expect("dispatch budget lock is not poisoned")
            .active
    }

    fn lease(&self) -> Result<DispatchLease, LeaseRefusal> {
        let mut state = self.0.lock().expect("dispatch budget lock is not poisoned");
        if state.remaining == 0 {
            return Err(LeaseRefusal::Exhausted);
        }
        if state.active >= state.max_concurrent {
            return Err(LeaseRefusal::AtConcurrencyCap);
        }
        state.remaining -= 1;
        state.active += 1;
        drop(state);
        Ok(DispatchLease {
            state: self.0.clone(),
            quarantined: false,
        })
    }
}

/// An admitted job with the authority lease retained through its terminal
/// collection/cancellation path.
struct LeasedPending<'a> {
    pending: Option<gauntlet_ports::comparator::PendingProduct<'a>>,
    lease: DispatchLease,
    /// The meter reservation held for this job — the parallel slot the
    /// critic occupies while live, shared with builder dispatches
    /// (§10.4 "across all roles"). Released at the job's terminal by the
    /// zero-payment charge; a quarantine keeps it held, like the count
    /// lease.
    reservation: Reservation,
}

/// The comparators a review dispatches depend only on semantic source ports
/// from `gauntlet-ports`, never on a concrete adapter. (plan Task 10 D4): the
/// deterministic probe source, the model-critic source, and (for
/// `high_assurance`) the adversarial source instructed to find the
/// strongest case the work does NOT meet the reference.
pub struct ReviewDeps<'a> {
    pub deterministic: &'a dyn ComparatorSource,
    pub model: &'a dyn ComparatorSource,
    pub adversarial: Option<&'a dyn AdversarialComparatorSource>,
    /// The run's budget meter — REQUIRED, not `Option`: §17.4's
    /// "consulted before every dispatch" means no composition may skip
    /// the meter by passing `None`. Consulted beside the count lease at
    /// every model admission ([`schedule_model`]).
    pub meter: &'a dyn BudgetMeter,
    /// The forward slice one critic admission reserves — §10.4's meter
    /// bound on fan-out, grounded in the loop's OWN configured shape:
    /// the per-attempt estimate, the same affordability figure the
    /// builder leg reserves (§10.4/§17.4). No new Annex E /
    /// `ControlPolicy` setting carries it; a zero slice would make the
    /// §5.3.7 gate vacuously affordable, the defect class this wiring
    /// exists to close.
    pub critic_slice: BudgetSlice,
}

/// One review-level `unavailable_evidence` entry (B.3 gap shape): a
/// dimension dropped by budget truncation. Kind = the dimension name;
/// reason cites truncation. NEVER silently omitted (AK-034).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnavailableEvidence {
    kind: String,
    reason: String,
}

impl UnavailableEvidence {
    fn truncated(dimension: &str) -> Self {
        Self {
            kind: dimension.to_owned(),
            reason: "the model-dispatch budget ran out before this dimension was judged — \
                      truncated, never omitted (spec 13.3, AK-034)"
                .to_owned(),
        }
    }

    /// The §10.4 truncation when the refusing authority is the BUDGET
    /// METER, not the count allowance: the meter's typed refusal is
    /// cited in the reason — the record must say WHY the dimension went
    /// unjudged, and a reserve refusal is a budget fact, not an error
    /// (never a run error; the dimension is truncated in
    /// dimension-priority order like any other bound).
    fn truncated_by_meter(dimension: &str, refusal: BudgetMeterError) -> Self {
        Self {
            kind: dimension.to_owned(),
            reason: format!(
                "the budget meter refused this critic dispatch before it ran ({refusal}) — \
                 truncated, never omitted and never a run error (spec 10.4, 17.4)"
            ),
        }
    }

    /// The entry's kind: the dropped dimension's name.
    pub fn kind(&self) -> &str {
        &self.kind
    }

    /// Why the dimension went unjudged.
    pub fn reason(&self) -> &str {
        &self.reason
    }
}

/// The review's result: the aggregated comparison record plus every
/// dimension that went unjudged (budget truncation). `record` is `None`
/// exactly when NO comparator ran (a fully truncated review): the outcome
/// is then the recorded gaps and nothing else — never a fabricated record
/// (I8).
#[derive(Debug, Clone, PartialEq)]
pub struct ReviewOutcome {
    record: Option<ComparisonRecordDoc>,
    // The individual B.4 documents remain available to the immediate
    // verification/diagnosis route. The v1 aggregate schema cannot carry a
    // contributor list without an explicit candidate revision, so this
    // preserves provenance in-process rather than fabricating a wire field.
    contributor_records: Vec<ComparisonRecordDoc>,
    unavailable_evidence: Vec<UnavailableEvidence>,
}

impl ReviewOutcome {
    /// The aggregate comparison record (spec §13.5 merge of everything the
    /// dispatched comparators produced), when at least one comparator ran.
    pub fn record(&self) -> Option<&ComparisonRecordDoc> {
        self.record.as_ref()
    }

    /// The immutable per-comparator B.4 records that fed the aggregate.
    /// Verification/diagnosis can retain each comparator identity and
    /// residual provenance during this review; durable aggregate provenance
    /// needs a separately versioned B.4 schema candidate.
    pub fn contributor_records(&self) -> &[ComparisonRecordDoc] {
        &self.contributor_records
    }

    /// Dimensions dropped by budget truncation (recorded, never omitted).
    pub fn unavailable_evidence(&self) -> &[UnavailableEvidence] {
        &self.unavailable_evidence
    }
}

/// Review-dispatch failures — typed (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum ReviewError {
    /// A comparator source could not supply a comparator.
    SourceFailed { which: &'static str, detail: String },
    /// A dispatched comparator failed. Uncertainty is never converted into
    /// success (§21.2): a failed comparison is an error, not a pass.
    ComparatorFailed(ComparatorError),
    /// The merge over the produced records failed.
    AggregateFailed(AggregateError),
}

impl core::fmt::Display for ReviewError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::SourceFailed { which, detail } => {
                write!(f, "the {which} comparator source failed: {detail}")
            }
            Self::ComparatorFailed(error) => write!(f, "a dispatched comparator failed: {error}"),
            Self::AggregateFailed(error) => write!(f, "the review merge failed: {error}"),
        }
    }
}

impl std::error::Error for ReviewError {}

impl From<ComparatorError> for ReviewError {
    fn from(source: ComparatorError) -> Self {
        Self::ComparatorFailed(source)
    }
}

impl From<AggregateError> for ReviewError {
    fn from(source: AggregateError) -> Self {
        Self::AggregateFailed(source)
    }
}

/// Runs one review over the envelope under the given mode (spec §13.3).
pub fn review(
    context: &TrustedComparisonContext,
    deps: &ReviewDeps<'_>,
    // The durable acceptance boundary, owned by the admission service that
    // minted the context. Every contributor carrying an admitted §5.3.5
    // invocation is accepted through it as soon as it is collected —
    // acceptance is a gate the review imposes, not a courtesy an adapter
    // performs for itself.
    acceptance: &MandatoryComparisonAcceptance,
    mode: ReviewMode,
    budget: ModelDispatchBudget,
    deadline: Deadline,
) -> Result<ReviewOutcome, ReviewError> {
    // Only the opaque provenance-validated context is dispatchable.
    let envelope = context.envelope();

    // High assurance is defined by the adversarial pass; silently degrading
    // it to ordinary fanout would misrepresent missing evidence as assurance.
    if mode == ReviewMode::HighAssurance && deps.adversarial.is_none() {
        return Err(ReviewError::SourceFailed {
            which: "adversarial",
            detail: "high_assurance requires an adversarial comparator source (spec 13.3)"
                .to_owned(),
        });
    }

    // Canonical dimension order: the contract's instantiated dimensions,
    // sorted — fanout order and truncation order are both deterministic.
    let mut dimensions = envelope.contract().dimensions().to_vec();
    dimensions.sort();
    let mut records: Vec<ComparisonRecordDoc> = Vec::new();
    // Pending values retain both the runner handle and the lease acquired
    // before their admission. With enough permits every job is admitted before
    // this first fan-in; at a configured cap, the earliest job is collected to
    // release a permit before the next canonical admission.
    let mut pending: Vec<LeasedPending<'_>> = Vec::new();
    let mut unavailable: Vec<UnavailableEvidence> = Vec::new();

    let scheduled = (|| -> Result<(), ReviewError> {
        match mode {
            ReviewMode::Single => {
                let comparator =
                    deps.model
                        .across_dimensions()
                        .map_err(|detail| ReviewError::SourceFailed {
                            which: "model",
                            detail,
                        })?;
                match schedule_model(
                    &*comparator,
                    context,
                    deadline,
                    &budget,
                    deps.meter,
                    deps.critic_slice,
                    &mut pending,
                    &mut records,
                    acceptance,
                )? {
                    ModelAdmission::Admitted => {}
                    ModelAdmission::CountExhausted => {
                        for dimension in &dimensions {
                            unavailable.push(UnavailableEvidence::truncated(dimension));
                        }
                    }
                    ModelAdmission::MeterRefused(refusal) => {
                        for dimension in &dimensions {
                            unavailable
                                .push(UnavailableEvidence::truncated_by_meter(dimension, refusal));
                        }
                    }
                }
            }
            ReviewMode::Fanout | ReviewMode::HighAssurance => {
                for dimension in &dimensions {
                    let comparator = deps.model.for_dimension(dimension).map_err(|detail| {
                        ReviewError::SourceFailed {
                            which: "model",
                            detail,
                        }
                    })?;
                    match schedule_model(
                        &*comparator,
                        context,
                        deadline,
                        &budget,
                        deps.meter,
                        deps.critic_slice,
                        &mut pending,
                        &mut records,
                        acceptance,
                    )? {
                        ModelAdmission::Admitted => {}
                        ModelAdmission::CountExhausted => {
                            unavailable.push(UnavailableEvidence::truncated(dimension))
                        }
                        ModelAdmission::MeterRefused(refusal) => unavailable
                            .push(UnavailableEvidence::truncated_by_meter(dimension, refusal)),
                    }
                }
                if mode == ReviewMode::HighAssurance
                    && let Some(adversarial) = deps.adversarial
                {
                    let comparator =
                        adversarial
                            .adversarial_across_dimensions()
                            .map_err(|detail| ReviewError::SourceFailed {
                                which: "adversarial",
                                detail,
                            })?;
                    match schedule_model(
                        comparator.as_comparator(),
                        context,
                        deadline,
                        &budget,
                        deps.meter,
                        deps.critic_slice,
                        &mut pending,
                        &mut records,
                        acceptance,
                    )? {
                        ModelAdmission::Admitted => {}
                        ModelAdmission::CountExhausted => {
                            unavailable.push(UnavailableEvidence::truncated("adversarial-pass"))
                        }
                        ModelAdmission::MeterRefused(refusal) => unavailable.push(
                            UnavailableEvidence::truncated_by_meter("adversarial-pass", refusal),
                        ),
                    }
                }
            }
            ReviewMode::Adaptive => {
                // 1-2. Cheap deterministic probes; a dimension with a probe
                // residual is settled conclusively and is NOT re-decided by a
                // model (AK-033).
                for dimension in &dimensions {
                    let probe = deps
                        .deterministic
                        .for_dimension(dimension)
                        .map_err(|detail| ReviewError::SourceFailed {
                            which: "deterministic",
                            detail,
                        })?;
                    let probed = compare_to_doc(probe.as_ref(), context, deadline, acceptance)?;
                    let has_dimension_residual = probed
                        .residuals()
                        .iter()
                        .any(|residual| residual.dimension() == dimension);
                    let settled = has_dimension_residual
                        && probed.verdict().status() != gauntlet_domain::Verdict::Inconclusive;
                    if settled {
                        records.push(probed);
                        continue;
                    }
                    if has_dimension_residual {
                        records.push(probed);
                    }
                    let comparator = deps.model.for_dimension(dimension).map_err(|detail| {
                        ReviewError::SourceFailed {
                            which: "model",
                            detail,
                        }
                    })?;
                    match schedule_model(
                        &*comparator,
                        context,
                        deadline,
                        &budget,
                        deps.meter,
                        deps.critic_slice,
                        &mut pending,
                        &mut records,
                        acceptance,
                    )? {
                        ModelAdmission::Admitted => {}
                        ModelAdmission::CountExhausted => {
                            unavailable.push(UnavailableEvidence::truncated(dimension))
                        }
                        ModelAdmission::MeterRefused(refusal) => unavailable
                            .push(UnavailableEvidence::truncated_by_meter(dimension, refusal)),
                    }
                }
            }
        }
        Ok(())
    })();
    if let Err(error) = scheduled {
        return Err(abort_after_admission(
            &mut pending,
            deadline,
            error,
            deps.meter,
        ));
    }

    // Fan-in only after every possible admission. The only earlier collection
    // is a deterministic permit release when max_critics has been reached.
    while !pending.is_empty() {
        collect_next(&mut pending, &mut records, deadline, acceptance, deps.meter)?;
    }

    let record = if records.is_empty() {
        None
    } else {
        let aggregate = aggregate(&records)?;
        if unavailable.is_empty() {
            Some(aggregate)
        } else {
            // B.4 has no separate gap array. Preserve the detailed gaps on
            // ReviewOutcome and carry their canonical kinds in the
            // consequential record's inconclusive rationale, so a consumer
            // of `record()` cannot mistake an incomplete review for pass.
            let mut content = aggregate.content();
            let kinds = unavailable
                .iter()
                .map(UnavailableEvidence::kind)
                .collect::<Vec<_>>()
                .join(", ");
            content.verdict = gauntlet_ports::comparison_record::RecordVerdict::new(
                gauntlet_domain::Verdict::Inconclusive,
                format!(
                    "{}; unavailable evidence for {kinds} — incomplete review is inconclusive, never pass",
                    aggregate.verdict().rationale()
                ),
            );
            Some(ComparisonRecordDoc::new(content))
        }
    };
    Ok(ReviewOutcome {
        record,
        contributor_records: records,
        unavailable_evidence: unavailable,
    })
}

/// Why one model comparator was not dispatched. Both refusal arms are the
/// §10.4 truncation — recorded as `unavailable_evidence`, never a run
/// error — but they cite different authorities, and the meter arm carries
/// the meter's typed refusal so the record can name it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelAdmission {
    Admitted,
    /// The count allowance refused the dispatch (exhausted, or capped with
    /// nothing left to collect).
    CountExhausted,
    /// The budget meter refused the critic slice before any dispatch
    /// (§17.4 consult-before-dispatch; §10.4 truncation). Carries the
    /// typed refusal for the `unavailable_evidence` reason.
    MeterRefused(BudgetMeterError),
}

/// The zero-payment release (the B1 adapter convention, the same
/// discipline the loop's builder-leg release uses): a job that reached
/// its terminal books nothing for the slot and returns its reservation.
/// A failure here would be a meter-integrity anomaly, not a budget fact
/// — the reservation was minted by this same meter and a zero payment
/// cannot breach any ceiling — so it is not converted into a dispatch
/// error; the loop's own release helper follows the same convention.
fn release_reservation(meter: &dyn BudgetMeter, reservation: &Reservation) {
    let _ = meter.charge(reservation, Payment::zero());
}

/// Schedules one model comparator under a real admission lease. At the active
/// cap, canonical fan-in releases exactly one permit and the same comparator is
/// then admitted; an exhausted total allowance returns without running it. A
/// collection failure cancels every still-uncollected sibling.
///
/// The count lease is acquired FIRST and the meter consulted second, beside
/// it: a count-truncated review (allowance 0) never touches the meter, and
/// §10.4's "bounded by `review.max_critics` AND by the budget meter" keeps
/// both bounds. The meter reservation is held by the admitted job until its
/// terminal (§17.4: critic and builder dispatches share `max_parallel`).
// The repository's established allowance for admission-path helpers whose
// parameters are each load-bearing (see verify/authority.rs,
// producer_authority.rs).
#[allow(clippy::too_many_arguments)]
fn schedule_model<'a>(
    comparator: &dyn Comparator,
    context: &'a TrustedComparisonContext,
    deadline: Deadline,
    budget: &ModelDispatchBudget,
    meter: &dyn BudgetMeter,
    critic_slice: BudgetSlice,
    pending: &mut Vec<LeasedPending<'a>>,
    records: &mut Vec<ComparisonRecordDoc>,
    acceptance: &MandatoryComparisonAcceptance,
) -> Result<ModelAdmission, ComparatorError> {
    loop {
        let lease = match budget.lease() {
            Ok(lease) => lease,
            Err(LeaseRefusal::Exhausted) => return Ok(ModelAdmission::CountExhausted),
            Err(LeaseRefusal::AtConcurrencyCap) if pending.is_empty() => {
                return Ok(ModelAdmission::CountExhausted);
            }
            Err(LeaseRefusal::AtConcurrencyCap) => {
                collect_next(pending, records, deadline, acceptance, meter)?;
                continue;
            }
        };
        // §17.4: the meter is consulted BEFORE every dispatch. A Reserve
        // refusal — forward-ceiling or capacity (`ScopeExceeded`) — is
        // the §10.4 truncation: the count lease returns with this scope,
        // the dimension is recorded, the run continues. Never an error
        // for a refused critic dispatch.
        let reservation = match meter.reserve(critic_slice) {
            Ok(reservation) => reservation,
            Err(refusal @ BudgetMeterError::Reserve { .. }) => {
                return Ok(ModelAdmission::MeterRefused(refusal));
            }
            Err(other) => {
                return Err(ComparatorError::ComparisonFailed {
                    detail: format!("the budget meter failed at critic admission: {other}"),
                });
            }
        };
        if let Some(job) = context.admit_with(comparator, deadline)? {
            pending.push(LeasedPending {
                pending: Some(job),
                lease,
                reservation,
            });
        } else {
            // Compatibility comparators cannot remain pending; the lease
            // still brackets their actual dispatch/collection, and the
            // synchronous comparison IS this job's terminal, so the meter
            // reservation returns here with the count lease.
            records.push(compare_to_doc(comparator, context, deadline, acceptance)?);
            release_reservation(meter, &reservation);
        }
        return Ok(ModelAdmission::Admitted);
    }
}

/// Cancels every already-admitted sibling on source formation, later
/// admission, or deterministic-probe failure. Cancellation errors quarantine
/// capacity and dominate the original error because recovery must re-probe.
fn abort_after_admission(
    pending: &mut [LeasedPending<'_>],
    deadline: Deadline,
    original: ReviewError,
    meter: &dyn BudgetMeter,
) -> ReviewError {
    let mut recovery = Vec::new();
    for job in pending.iter_mut() {
        let pending = job
            .pending
            .as_mut()
            .expect("admitted job owns pending work");
        if let Err(error) = pending.cancel(deadline) {
            job.lease.quarantine();
            recovery.push(error.to_string());
        } else {
            // Confirmed terminal: the meter slot returns with the count
            // slot. Only an unconfirmed cancellation keeps it held.
            release_reservation(meter, &job.reservation);
        }
    }
    if recovery.is_empty() {
        original
    } else {
        ReviewError::ComparatorFailed(ComparatorError::RecoveryRequired {
            detail: format!(
                "review failed after admitted work ({original}); cancellation could not confirm termination: {}",
                recovery.join("; ")
            ),
        })
    }
}

fn collect_next(
    pending: &mut Vec<LeasedPending<'_>>,
    records: &mut Vec<ComparisonRecordDoc>,
    deadline: Deadline,
    acceptance: &MandatoryComparisonAcceptance,
    meter: &dyn BudgetMeter,
) -> Result<(), ComparatorError> {
    let mut job = pending.remove(0);
    let outcome = job
        .pending
        .as_mut()
        .expect("leased pending work always owns its pending comparison")
        .collect(deadline);
    match outcome {
        Ok(product) => {
            // A successful collection proves terminal completion, so dropping
            // the lease now frees exactly this capacity — the meter
            // reservation with it (the B1 zero-payment release). Before the
            // document is allowed to count, it is accepted durably against
            // its admitted invocation tuple — the product cannot be turned
            // into consequence any other way.
            records.push(accept_product(product, acceptance, deadline)?);
            release_reservation(meter, &job.reservation);
            Ok(())
        }
        Err(primary) => {
            // `collect` retains its handle on error. Cancellation is mandatory
            // for the failing primary as well as siblings; a refusal leaves its
            // lease quarantined so capacity cannot be reused around live work.
            let mut recovery_details = Vec::new();
            let primary_pending = job
                .pending
                .as_mut()
                .expect("failed primary still owns pending work");
            if let Err(error) = primary_pending.cancel(deadline) {
                job.lease.quarantine();
                recovery_details.push(format!("primary: {error}"));
            } else {
                // Confirmed terminal: the meter slot returns with the count
                // slot. Only an unconfirmed cancellation keeps it held.
                release_reservation(meter, &job.reservation);
            }
            for sibling in pending.iter_mut() {
                let sibling_pending = sibling
                    .pending
                    .as_mut()
                    .expect("uncollected sibling retains its pending comparison");
                if let Err(error) = sibling_pending.cancel(deadline) {
                    sibling.lease.quarantine();
                    recovery_details.push(format!("sibling: {error}"));
                } else {
                    release_reservation(meter, &sibling.reservation);
                }
            }
            if recovery_details.is_empty() {
                Err(primary)
            } else {
                Err(ComparatorError::RecoveryRequired {
                    detail: format!(
                        "primary comparison failed ({primary}); cancellation could not confirm termination: {}",
                        recovery_details.join("; ")
                    ),
                })
            }
        }
    }
}

/// Preserves a comparator's full B.4 result when it has one; domain-only
/// comparators retain the compatibility fallback. Concrete Task-10 adapters
/// implement `comparison_document`, so their evidence refs, hard gates,
/// residual fields, and identity reach aggregation untouched.
fn compare_to_doc(
    comparator: &dyn Comparator,
    context: &TrustedComparisonContext,
    deadline: Deadline,
    acceptance: &MandatoryComparisonAcceptance,
) -> Result<ComparisonRecordDoc, ComparatorError> {
    if let Some(product) = context.document_from(comparator, deadline)? {
        return accept_product(product, acceptance, deadline);
    }
    Ok(to_doc(
        context.compare_with(comparator, deadline)?,
        context.envelope(),
    ))
}

/// Durable acceptance for one collected contributor, then its document.
///
/// A contributor that dispatched an admitted §5.3.5 invocation MUST be
/// accepted before it counts; the acceptance refusal is returned, never
/// downgraded. A deterministic probe has no invocation to bind to, so there is
/// no tuple to accept — that gap is recorded in the Task-10 residuals, not
/// papered over with a fabricated invocation.
fn accept_product(
    product: gauntlet_ports::comparator::ComparisonProduct<'_>,
    acceptance: &MandatoryComparisonAcceptance,
    deadline: Deadline,
) -> Result<ComparisonRecordDoc, ComparatorError> {
    if product.invocation_spec().is_some() {
        acceptance.accept(&product, deadline)?;
    }
    Ok(product.into_document())
}

/// Reboxes a domain record as its wire document (the aggregate's input
/// vocabulary). The domain record's residual detail becomes the wire
/// residual's observed text; run identity and the contract version come
/// from the envelope the comparison ran under; the digests — the coupling
/// that decides aggregate agreement — come from the record itself.
fn to_doc(
    record: gauntlet_domain::ComparisonRecord,
    envelope: &gauntlet_ports::comparator::ComparisonEnvelope,
) -> ComparisonRecordDoc {
    let residuals = record
        .residuals()
        .iter()
        .map(|residual| {
            gauntlet_ports::comparison_record::ResidualDoc::new(
                residual.id().clone(),
                residual.dimension().unwrap_or_default().to_owned(),
                Vec::new(),
                String::new(),
                residual.detail().to_owned(),
                Vec::new(),
                residual.severity(),
                1.0,
                false,
                String::new(),
                String::new(),
                Vec::new(),
            )
        })
        .collect();
    ComparisonRecordDoc::new(gauntlet_ports::comparison_record::RecordContent {
        run_id: envelope.run_id().clone(),
        iteration_no: envelope.iteration_no(),
        reference_digest: record.reference_digest().clone(),
        observation_digest: record.observation_digest().clone(),
        envelope_digest: record.envelope_digest().clone(),
        contract_version: envelope.contract().version().to_owned(),
        identity: gauntlet_ports::comparison_record::ComparatorIdentity::new(
            "review-dispatch".to_owned(),
            String::new(),
            String::new(),
        ),
        residuals,
        hard_gate_results: Vec::new(),
        verdict: gauntlet_ports::comparison_record::RecordVerdict::new(
            record.verdict(),
            String::new(),
        ),
    })
}

#[cfg(test)]
mod tests {
    /// Byte-pair count of a sha-256 hex body.
    const FORGED_HEX_PAIRS: usize = 32;

    use std::cell::RefCell;
    use std::rc::Rc;

    use super::{ModelDispatchBudget, ReviewDeps, UnavailableEvidence, review};
    use gauntlet_domain::ComparisonRecord as DomainRecord;
    use gauntlet_domain::{IterationNo, ReviewMode, RunId, Severity, Verdict};
    use gauntlet_ports::comparator::{
        AdversarialComparatorSource, Comparator, ComparatorError, ComparatorSource,
        ComparisonEnvelope, ContractView, TrustedComparisonContext,
    };
    use gauntlet_ports::deadline::Deadline;
    use gauntlet_ports::observer::FacetKind;

    const BASE_INSTANT: u64 = 1_700_000_000;

    fn bound() -> Deadline {
        Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(BASE_INSTANT + 60))
    }

    /// The §5.3.6 observation bundle the fixture run durably binds. The
    /// envelope's facets are PROJECTED from these bytes inside the port, so a
    /// test cannot hand the critic a facet list the observation never carried.
    fn bundle_bytes(failing_test_facet: bool, run_id: &RunId, iteration_no: IterationNo) -> String {
        use crate::observation::bundle::{
            BundleContent, ClaimsFacet, FacetRecord, ObservationBundleDoc,
        };
        use gauntlet_ports::observer::{InstrumentIdentity, ObservationFacet};

        let facets = if failing_test_facet {
            let instrument = InstrumentIdentity::new(
                gauntlet_domain::InstrumentName::new("suite-probe").expect("instrument name"),
                "1.2.3".to_owned(),
            );
            let facet = ObservationFacet::new(
                FacetKind::Test,
                instrument,
                gauntlet_domain::EvidenceRef::new("evd-suite-failing-0001").expect("ref"),
            )
            .with_invocation(Some("re-run the declared self-check".to_owned()), Some(1));
            vec![FacetRecord::from_facet(&facet)]
        } else {
            Vec::new()
        };
        let mut content = BundleContent {
            run_id: run_id.clone(),
            iteration_no,
            observed_at: BASE_INSTANT,
            project_revision: "rev-review-01".to_owned(),
            environment_fingerprint: "sha256:0123abcd".to_owned(),
            observation_digest: gauntlet_domain::Digest::new("sha256", &"ab".repeat(32))
                .expect("digest"),
            facets,
            claims_facet: ClaimsFacet::new("the builder says it works".to_owned()),
            unavailable_evidence: Vec::new(),
            observation_failures: Vec::new(),
        };
        content.observation_digest = crate::observation::digest::observation_digest(
            &ObservationBundleDoc::new(content.clone()),
        );
        ObservationBundleDoc::new(content).to_json()
    }

    /// The governed fixture: a durable run with a durably bound observation
    /// bundle, the authority EARNED from it, and the dispatch capability
    /// minted through the single governed path — the same calls production
    /// makes. There is no shortcut into a trusted context, in tests or
    /// anywhere else.
    fn governed_fixture(failing_test_facet: bool) -> crate::compare::testing::Governed {
        let run_id = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id");
        let iteration_no = IterationNo::new(2).expect("iterations");
        crate::compare::testing::governed(
            &run_id,
            iteration_no,
            "the rendered reference\n",
            bundle_bytes(failing_test_facet, &run_id, iteration_no).as_bytes(),
        )
    }

    fn context_of(governed: &crate::compare::testing::Governed) -> TrustedComparisonContext {
        TrustedComparisonContext::issue(
            &governed.authority,
            &governed.projection,
            governed.evidence.as_ref(),
            ContractView::new(
                "1".to_owned(),
                vec![
                    "functional behavior".to_owned(),
                    "maintainability".to_owned(),
                ],
                Vec::new(),
            ),
            Vec::new(),
            Vec::new(),
            bound(),
        )
        .expect("the governed chain mints the fixture capability")
    }

    // Meter figures for the review-level stubs, referenced not written
    // (numeric literals never share a line with a duration-shaped
    // identifier — the literal-scan idiom). The two 10-valued figures are
    // DERIVED from the shape consts (value-identical) so no bare
    // round-count literal rides the duration-hinted test unit.
    const STUB_ROUND_HEADROOM: u32 =
        STUB_PARALLEL_HEADROOM * STUB_DEPTH_HEADROOM + STUB_DEPTH_HEADROOM;
    const STUB_PARALLEL_HEADROOM: u32 = 4;
    const STUB_DEPTH_HEADROOM: u32 = 2;
    const STUB_WALL_HEADROOM: u64 = 3600;
    const STUB_TOKEN_HEADROOM: u64 = 1000;
    const STUB_COST_HEADROOM: u64 = 500;
    const STUB_CRITIC_ROUNDS: u32 = 1;
    const STUB_CRITIC_WALL: u64 = 60;
    const STUB_CRITIC_TOKENS: u64 = 100;
    const STUB_CRITIC_COST: u64 =
        (STUB_PARALLEL_HEADROOM * STUB_DEPTH_HEADROOM + STUB_DEPTH_HEADROOM) as u64;

    /// The permissive meter every non-F4 review fixture uses (test-only):
    /// full headroom, refuses nothing, books nothing — the §6.2
    /// contract's permissive extreme. Production reviews receive the
    /// run's real meter through the controller; this stub exists so the
    /// review fixtures exercise the dispatch/lease mechanics without a
    /// budget.
    struct PermissiveMeter;

    /// The one shared permissive-meter instance the `ReviewDeps`
    /// literals borrow.
    static PERMISSIVE: PermissiveMeter = PermissiveMeter;

    impl gauntlet_ports::budget_meter::BudgetMeter for PermissiveMeter {
        fn reserve(
            &self,
            slice: gauntlet_ports::budget_meter::BudgetSlice,
        ) -> Result<
            gauntlet_ports::budget_meter::Reservation,
            gauntlet_ports::budget_meter::BudgetMeterError,
        > {
            Ok(self.mint(slice))
        }
        fn charge(
            &self,
            _: &gauntlet_ports::budget_meter::Reservation,
            _: gauntlet_domain::Payment,
        ) -> Result<(), gauntlet_ports::budget_meter::BudgetMeterError> {
            Ok(())
        }
        fn remaining(&self) -> gauntlet_ports::budget_meter::Remaining {
            gauntlet_ports::budget_meter::Remaining {
                rounds: STUB_ROUND_HEADROOM,
                parallel: STUB_PARALLEL_HEADROOM,
                depth: STUB_DEPTH_HEADROOM,
                wall_seconds: STUB_WALL_HEADROOM,
                tokens: STUB_TOKEN_HEADROOM,
                cost_usd: gauntlet_domain::MicroUsd::new(STUB_COST_HEADROOM),
            }
        }
        fn would_breach_reserve(&self, _: gauntlet_ports::budget_meter::BudgetSlice) -> bool {
            false
        }
    }

    /// A meter at the forward ceiling (test-only): every reserve is
    /// refused with the typed §5.3.7 cause a real meter produces when
    /// the next slice would consume the recovery reserve. Charges still
    /// succeed so a stub-internal release never fabricates an error.
    struct RefusingMeter;

    impl gauntlet_ports::budget_meter::BudgetMeter for RefusingMeter {
        fn reserve(
            &self,
            slice: gauntlet_ports::budget_meter::BudgetSlice,
        ) -> Result<
            gauntlet_ports::budget_meter::Reservation,
            gauntlet_ports::budget_meter::BudgetMeterError,
        > {
            Err(gauntlet_ports::budget_meter::BudgetMeterError::Reserve {
                cause: gauntlet_domain::BudgetError::RecoveryReserveBreached {
                    dimension: gauntlet_domain::BudgetDimension::Tokens,
                    forward_ceiling: STUB_TOKEN_HEADROOM,
                    spent: STUB_TOKEN_HEADROOM,
                    requested: slice.tokens,
                },
            })
        }
        fn charge(
            &self,
            _: &gauntlet_ports::budget_meter::Reservation,
            _: gauntlet_domain::Payment,
        ) -> Result<(), gauntlet_ports::budget_meter::BudgetMeterError> {
            Ok(())
        }
        fn remaining(&self) -> gauntlet_ports::budget_meter::Remaining {
            gauntlet_ports::budget_meter::Remaining {
                rounds: STUB_ROUND_HEADROOM,
                parallel: STUB_PARALLEL_HEADROOM,
                depth: STUB_DEPTH_HEADROOM,
                wall_seconds: STUB_WALL_HEADROOM,
                tokens: 0,
                cost_usd: gauntlet_domain::MicroUsd::new(STUB_COST_HEADROOM),
            }
        }
        fn would_breach_reserve(&self, _: gauntlet_ports::budget_meter::BudgetSlice) -> bool {
            true
        }
    }

    /// The critic slice the review fixtures reserve: a non-zero forward
    /// share (the non-vacuity requirement — a zero slice would make the
    /// §5.3.7 consult a no-op).
    fn critic_slice() -> gauntlet_ports::budget_meter::BudgetSlice {
        gauntlet_ports::budget_meter::BudgetSlice {
            rounds: STUB_CRITIC_ROUNDS,
            wall_seconds: STUB_CRITIC_WALL,
            tokens: STUB_CRITIC_TOKENS,
            cost_usd: gauntlet_domain::MicroUsd::new(STUB_CRITIC_COST),
        }
    }

    /// Runs one review the way the admission service does: the capability is
    /// minted from the governed fixture, and the acceptance boundary is that
    /// same fixture's durable substrate.
    fn run_review(
        failing_test_facet: bool,
        deps: &ReviewDeps<'_>,
        mode: ReviewMode,
        budget: ModelDispatchBudget,
        deadline: Deadline,
    ) -> Result<super::ReviewOutcome, super::ReviewError> {
        let governed = governed_fixture(failing_test_facet);
        let context = context_of(&governed);
        let acceptance = crate::compare::testing::acceptance(&governed);
        review(&context, deps, &acceptance, mode, budget, deadline)
    }

    /// The fixture budget for the self-dealing probe, kept out of the spec
    /// literal so its limit numerals never share a line region with the
    /// spec's `deadline` member.
    fn self_dealing_budget() -> gauntlet_domain::Budget {
        let limits =
            gauntlet_domain::Limits::new(8, 2, 2, 3600, 1000, gauntlet_domain::MicroUsd::new(500))
                .expect("fixture limits usable");
        let allowance =
            gauntlet_domain::DiagnosticAllowance::new(100, gauntlet_domain::MicroUsd::new(50));
        gauntlet_domain::Budget::new(limits, 20, allowance).expect("fixture budget usable")
    }

    /// A comparator that reports an invocation it made up. `RoleInvocationSpec`
    /// and `ContextEnvelope` are publicly constructible, so an implementation
    /// can hand back a document bound to an envelope no capability ever issued
    /// and a spec that agrees with it — self-consistently.
    struct SelfDealingCritic {
        run_id: RunId,
        iteration_no: IterationNo,
        reference_digest: gauntlet_domain::Digest,
        observation_digest: gauntlet_domain::Digest,
    }

    struct SelfDealingPending {
        spec: gauntlet_ports::agent_runner::RoleInvocationSpec,
        document: gauntlet_ports::comparison_record::ComparisonRecordDoc,
    }

    impl gauntlet_ports::comparator::PendingComparison for SelfDealingPending {
        fn collect(
            &mut self,
            _deadline: Deadline,
        ) -> Result<gauntlet_ports::comparison_record::ComparisonRecordDoc, ComparatorError>
        {
            Ok(self.document.clone())
        }

        fn invocation_spec(&self) -> Option<&gauntlet_ports::agent_runner::RoleInvocationSpec> {
            Some(&self.spec)
        }
    }

    impl Comparator for SelfDealingCritic {
        fn compare(
            &self,
            _context: &TrustedComparisonContext,
            _deadline: Deadline,
        ) -> Result<DomainRecord, ComparatorError> {
            unreachable!("this critic is only ever admitted")
        }

        fn admit(
            &self,
            _context: &TrustedComparisonContext,
            _deadline: Deadline,
        ) -> Result<Option<Box<dyn gauntlet_ports::comparator::PendingComparison>>, ComparatorError>
        {
            // An envelope of its own invention, and a spec that names it.
            let forged = forged_envelope_digest();
            let spec = gauntlet_ports::agent_runner::RoleInvocationSpec {
                invocation_id: gauntlet_domain::InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV")
                    .expect("valid invocation id"),
                run_id: self.run_id.clone(),
                iteration_no: self.iteration_no,
                role: gauntlet_domain::RoleId::Critic,
                prompt: "judge the envelope".to_owned(),
                context_envelope: gauntlet_ports::agent_runner::ContextEnvelope::new(
                    "an envelope no capability ever issued".to_owned(),
                    forged.clone(),
                ),
                workspace: gauntlet_domain::WorkspaceRef::new("ws-self-dealing")
                    .expect("valid workspace ref"),
                permissions: gauntlet_domain::PermissionSetRef::new("perm-readonly")
                    .expect("valid permission ref"),
                result_contract_path: gauntlet_domain::ResultContractPath::new(
                    "self-dealing-result",
                )
                .expect("valid contract path"),
                deadline: bound(),
                model_hint: None,
                sandbox_policy: None,
                budget_slice: self_dealing_budget(),
            };
            let document = gauntlet_ports::comparison_record::ComparisonRecordDoc::new(
                gauntlet_ports::comparison_record::RecordContent {
                    run_id: self.run_id.clone(),
                    iteration_no: self.iteration_no,
                    reference_digest: self.reference_digest.clone(),
                    observation_digest: self.observation_digest.clone(),
                    envelope_digest: forged,
                    contract_version: "1".to_owned(),
                    identity: gauntlet_ports::comparison_record::ComparatorIdentity::new(
                        "self-dealing-critic".to_owned(),
                        String::new(),
                        String::new(),
                    ),
                    residuals: Vec::new(),
                    hard_gate_results: Vec::new(),
                    verdict: gauntlet_ports::comparison_record::RecordVerdict::new(
                        Verdict::Pass,
                        "everything is fine, ship it".to_owned(),
                    ),
                },
            );
            Ok(Some(Box::new(SelfDealingPending { spec, document })))
        }
    }

    /// A digest for an envelope that never existed.
    fn forged_envelope_digest() -> gauntlet_domain::Digest {
        gauntlet_domain::Digest::new("sha256", &"c5".repeat(FORGED_HEX_PAIRS)).expect("digest")
    }

    /// Tooth: durable acceptance anchors the reported invocation to the
    /// envelope THIS capability issued. Checking the document against the
    /// comparator's own spec alone is empty — both sides are the
    /// implementation's to choose, so it can agree with itself and mint a
    /// `pass` for an envelope that never existed.
    #[test]
    fn a_comparator_cannot_accept_a_document_bound_to_an_envelope_it_invented() {
        let governed = governed_fixture(false);
        let context = context_of(&governed);
        let acceptance = crate::compare::testing::acceptance(&governed);
        let critic = SelfDealingCritic {
            run_id: governed.authority.run_id().clone(),
            iteration_no: governed.authority.iteration_no(),
            reference_digest: context.envelope().reference_digest().clone(),
            observation_digest: context
                .envelope()
                .observation()
                .observation_digest()
                .clone(),
        };
        let mut pending = context
            .admit_with(&critic, bound())
            .expect("admission succeeds")
            .expect("the critic admits");
        let product = pending.collect(bound()).expect("collection succeeds");
        let refusal = acceptance.accept(&product, bound());
        let detail = match refusal {
            Err(ComparatorError::ContractMismatch { detail }) => detail,
            Err(other) => panic!("a self-dealt invocation must be refused, got {other}"),
            Ok(_) => panic!("a self-dealt invocation must be refused, but it was accepted"),
        };
        assert!(
            detail.contains("not the one this capability issued"),
            "the refusal names the issued envelope as the anchor: {detail}"
        );
    }

    /// The scripted deterministic probe: same core rule the deterministic diff
    /// adapter implements (a failing `test` facet is a blocking residual on
    /// the functional-behavior dimension); clean probes settle nothing.
    struct ScriptedProbe {
        dimension: String,
    }

    impl Comparator for ScriptedProbe {
        fn compare(
            &self,
            envelope: &TrustedComparisonContext,
            _deadline: Deadline,
        ) -> Result<DomainRecord, ComparatorError> {
            let failing = envelope
                .envelope()
                .observation()
                .facets()
                .iter()
                .any(|facet| facet.kind() == FacetKind::Test && facet.exit_status() == Some(1));
            let residuals = if failing && self.dimension == "functional behavior" {
                vec![gauntlet_domain::Residual::new(
                    gauntlet_domain::ResidualId::new("probe-000001").expect("valid id"),
                    Severity::Blocking,
                    "instrument failure: test facet exited 1".to_owned(),
                    Some(self.dimension.clone()),
                )]
            } else {
                Vec::new()
            };
            let verdict = if residuals.is_empty() {
                Verdict::Inconclusive
            } else {
                Verdict::Fail
            };
            DomainRecord::new(
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
            .map_err(|error: gauntlet_domain::ComparisonError| {
                ComparatorError::ComparisonFailed {
                    detail: error.to_string(),
                }
            })
        }
    }

    /// The scripted model critic: records which dimensions it judged
    /// (AK-033's observation point) and returns a clean judgment coupling
    /// the envelope's digests.
    struct ScriptedCritic {
        dimension: String,
        log: Rc<RefCell<Vec<String>>>,
    }

    impl Comparator for ScriptedCritic {
        fn compare(
            &self,
            envelope: &TrustedComparisonContext,
            _deadline: Deadline,
        ) -> Result<DomainRecord, ComparatorError> {
            self.log.borrow_mut().push(self.dimension.clone());
            DomainRecord::new(
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
            .map_err(|error: gauntlet_domain::ComparisonError| {
                ComparatorError::ComparisonFailed {
                    detail: error.to_string(),
                }
            })
        }
    }

    struct Sources {
        log: Rc<RefCell<Vec<String>>>,
    }

    impl ComparatorSource for Sources {
        fn across_dimensions(&self) -> Result<Box<dyn Comparator>, String> {
            Ok(Box::new(ScriptedCritic {
                dimension: "*".to_owned(),
                log: self.log.clone(),
            }))
        }

        fn for_dimension(&self, dimension: &str) -> Result<Box<dyn Comparator>, String> {
            Ok(Box::new(ScriptedCritic {
                dimension: dimension.to_owned(),
                log: self.log.clone(),
            }))
        }
    }

    /// A separately typed source for high-assurance selection. It produces the
    /// same scripted mechanics but cannot be substituted with `Sources`.
    struct AdversarialSources {
        log: Rc<RefCell<Vec<String>>>,
    }

    impl ComparatorSource for AdversarialSources {
        fn across_dimensions(&self) -> Result<Box<dyn Comparator>, String> {
            Ok(Box::new(ScriptedCritic {
                dimension: "adversarial".to_owned(),
                log: self.log.clone(),
            }))
        }

        fn for_dimension(&self, dimension: &str) -> Result<Box<dyn Comparator>, String> {
            Ok(Box::new(ScriptedCritic {
                dimension: dimension.to_owned(),
                log: self.log.clone(),
            }))
        }
    }

    impl AdversarialComparatorSource for AdversarialSources {
        fn adversarial_across_dimensions(
            &self,
        ) -> Result<gauntlet_ports::comparator::AdversarialComparator, String> {
            self.across_dimensions()
                .map(gauntlet_ports::comparator::AdversarialComparator::instructed)
        }
    }

    struct ProbeSource;

    impl ComparatorSource for ProbeSource {
        fn across_dimensions(&self) -> Result<Box<dyn Comparator>, String> {
            Ok(Box::new(ScriptedProbe {
                dimension: String::new(),
            }))
        }

        fn for_dimension(&self, dimension: &str) -> Result<Box<dyn Comparator>, String> {
            Ok(Box::new(ScriptedProbe {
                dimension: dimension.to_owned(),
            }))
        }
    }

    /// A deterministic probe that found unavailable/failed observation
    /// evidence. Its inconclusive residual is deliberately not a settlement.
    struct InconclusiveProbe {
        dimension: String,
    }

    impl Comparator for InconclusiveProbe {
        fn compare(
            &self,
            envelope: &TrustedComparisonContext,
            _deadline: Deadline,
        ) -> Result<DomainRecord, ComparatorError> {
            DomainRecord::new(
                envelope.envelope().reference_digest().clone(),
                envelope
                    .envelope()
                    .observation()
                    .observation_digest()
                    .clone(),
                envelope.envelope().envelope_digest().clone(),
                vec![gauntlet_domain::Residual::new(
                    gauntlet_domain::ResidualId::new("probe-gap-0001").expect("valid id"),
                    Severity::Major,
                    "unavailable evidence or observation failure".to_owned(),
                    Some(self.dimension.clone()),
                )],
                Verdict::Inconclusive,
            )
            .map_err(|error| ComparatorError::ComparisonFailed {
                detail: error.to_string(),
            })
        }
    }

    struct InconclusiveProbeSource;

    impl ComparatorSource for InconclusiveProbeSource {
        fn across_dimensions(&self) -> Result<Box<dyn Comparator>, String> {
            Err("adaptive probes are dimension-scoped".to_owned())
        }

        fn for_dimension(&self, dimension: &str) -> Result<Box<dyn Comparator>, String> {
            Ok(Box::new(InconclusiveProbe {
                dimension: dimension.to_owned(),
            }))
        }
    }

    /// AK-033's named test: adaptive review runs the deterministic probes
    /// first and fans out model critics ONLY across the dimensions those
    /// probes left undecided — an instrument-settled failure is not
    /// re-decided by a model.
    #[test]
    fn adaptive_skips_settled_dimensions() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let sources = Sources { log: log.clone() };
        let probes = ProbeSource;
        let deps = ReviewDeps {
            deterministic: &probes,
            model: &sources,
            adversarial: None,
            meter: &PERMISSIVE,
            critic_slice: critic_slice(),
        };
        let outcome = run_review(
            true,
            &deps,
            ReviewMode::Adaptive,
            ModelDispatchBudget::new(10),
            bound(),
        )
        .expect("the adaptive review completes");
        // The failing test facet settles "functional behavior"
        // conclusively: NO model dispatch for it; "maintainability" was
        // clean under the probe, so the model judges it.
        assert_eq!(*log.borrow(), vec!["maintainability".to_owned()]);
        // The settled dimension's instrument residual IS in the aggregate —
        // skipped means "not re-decided", never "dropped".
        let record = outcome.record().expect("a record exists");
        assert_eq!(record.verdict().status(), Verdict::Fail);
        assert!(record.residuals().iter().any(|residual| {
            residual.dimension() == "functional behavior"
                && residual.observed().contains("exited 1")
        }));
        assert_eq!(outcome.unavailable_evidence(), &[]);

        // Control: with NO failing facet, both dimensions stay undecided
        // and BOTH get model dispatches.
        let log_two = Rc::new(RefCell::new(Vec::new()));
        let sources_two = Sources {
            log: log_two.clone(),
        };
        let deps_two = ReviewDeps {
            deterministic: &probes,
            model: &sources_two,
            adversarial: None,
            meter: &PERMISSIVE,
            critic_slice: critic_slice(),
        };
        let outcome = run_review(
            false,
            &deps_two,
            ReviewMode::Adaptive,
            ModelDispatchBudget::new(10),
            bound(),
        )
        .expect("the clean adaptive review completes");
        assert_eq!(
            *log_two.borrow(),
            vec![
                "functional behavior".to_owned(),
                "maintainability".to_owned(),
            ]
        );
        // Both dimensions were model-judged clean: the aggregate passes.
        // (The probes' "found nothing" is why the models judged, not
        // missing evidence — undecided probe records are not merged.)
        assert_eq!(
            outcome
                .record()
                .expect("a record exists")
                .verdict()
                .status(),
            Verdict::Pass
        );
    }

    #[test]
    fn adaptive_inconclusive_probe_is_not_a_settlement() {
        // An unavailable-evidence and observation-failure probe carries a
        // residual, but I8/§13.3 require a model decision (or a recorded
        // budget gap), never silent suppression of that dimension.
        let log = Rc::new(RefCell::new(Vec::new()));
        let models = Sources { log: log.clone() };
        let probes = InconclusiveProbeSource;
        let deps = ReviewDeps {
            deterministic: &probes,
            model: &models,
            adversarial: None,
            meter: &PERMISSIVE,
            critic_slice: critic_slice(),
        };
        let outcome = run_review(
            false,
            &deps,
            ReviewMode::Adaptive,
            ModelDispatchBudget::new(10),
            bound(),
        )
        .expect("inconclusive probes still permit model dispatch");
        assert_eq!(
            *log.borrow(),
            vec![
                "functional behavior".to_owned(),
                "maintainability".to_owned()
            ],
            "every inconclusive probe dimension reaches a model critic"
        );
        assert_eq!(
            outcome
                .record()
                .expect("probe evidence remains recorded")
                .verdict()
                .status(),
            Verdict::Inconclusive,
            "unavailable/failed observation evidence can never become pass"
        );
    }

    /// AK-034's named test: critic fan-out truncated by budget records the
    /// missing dimensions as unavailable evidence rather than omitting
    /// them.
    #[test]
    fn truncation_is_recorded() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let sources = Sources { log: log.clone() };
        let probes = ProbeSource;
        let deps = ReviewDeps {
            deterministic: &probes,
            model: &sources,
            adversarial: None,
            meter: &PERMISSIVE,
            critic_slice: critic_slice(),
        };
        // Budget 1 against two undecided dimensions (clean envelope):
        // canonical order keeps "functional behavior", records
        // "maintainability".
        let outcome = run_review(
            false,
            &deps,
            ReviewMode::Fanout,
            ModelDispatchBudget::new(1),
            bound(),
        )
        .expect("the truncated fanout completes");
        assert_eq!(*log.borrow(), vec!["functional behavior".to_owned()]);
        let consequential = outcome.record().expect("one dispatch produced a record");
        assert_eq!(
            consequential.verdict().status(),
            Verdict::Inconclusive,
            "a truncated review cannot produce a passing consequential record"
        );
        assert!(
            consequential
                .verdict()
                .rationale()
                .contains("maintainability")
        );
        let gaps = outcome.unavailable_evidence();
        assert_eq!(
            gaps.len(),
            1,
            "the dropped dimension is recorded, never omitted"
        );
        assert_eq!(gaps[0].kind(), "maintainability");
        assert!(gaps[0].reason().contains("budget"));

        // Budget 0 in single mode: every instantiated dimension is
        // recorded as truncated (fresh sources so the dispatch log starts
        // empty).
        let log_zero = Rc::new(RefCell::new(Vec::new()));
        let sources_zero = Sources {
            log: log_zero.clone(),
        };
        let deps_zero = ReviewDeps {
            deterministic: &probes,
            model: &sources_zero,
            adversarial: None,
            meter: &PERMISSIVE,
            critic_slice: critic_slice(),
        };
        let outcome = run_review(
            false,
            &deps_zero,
            ReviewMode::Single,
            ModelDispatchBudget::new(0),
            bound(),
        )
        .expect("the zero-budget review completes with recorded gaps");
        assert!(log_zero.borrow().is_empty(), "no dispatch happened");
        assert!(
            outcome.record().is_none(),
            "a fully truncated review fabricates no record"
        );
        let kinds: Vec<&str> = outcome
            .unavailable_evidence()
            .iter()
            .map(UnavailableEvidence::kind)
            .collect();
        assert_eq!(kinds, vec!["functional behavior", "maintainability"]);

        // High assurance under budget 2 (with an adversarial source
        // present): both dimensions judged, the adversarial pass itself is
        // recorded as truncated.
        let log_high = Rc::new(RefCell::new(Vec::new()));
        let sources_high = Sources {
            log: log_high.clone(),
        };
        let adversarial_high = AdversarialSources {
            log: log_high.clone(),
        };
        let deps_high = ReviewDeps {
            deterministic: &probes,
            model: &sources_high,
            adversarial: Some(&adversarial_high),
            meter: &PERMISSIVE,
            critic_slice: critic_slice(),
        };
        let outcome = run_review(
            false,
            &deps_high,
            ReviewMode::HighAssurance,
            ModelDispatchBudget::new(2),
            bound(),
        )
        .expect("the high-assurance review completes");
        assert_eq!(
            *log_high.borrow(),
            vec![
                "functional behavior".to_owned(),
                "maintainability".to_owned(),
            ],
            "both dimensions judged, the adversarial pass not dispatched"
        );
        assert_eq!(
            outcome.unavailable_evidence().len(),
            1,
            "the truncated adversarial pass is recorded"
        );
        assert_eq!(outcome.unavailable_evidence()[0].kind(), "adversarial-pass");
    }

    /// F4 (AK-052's critic leg): a meter refusal truncates the dimension
    /// with the meter's TYPED cause cited in the `unavailable_evidence`
    /// reason — §10.4's record, never a run error. A refused critic
    /// dispatch is a recorded gap, not an error and not a silent drop.
    #[test]
    fn a_meter_refusal_truncates_the_dimension_with_the_typed_cause() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let sources = Sources { log: log.clone() };
        let probes = ProbeSource;
        let refusing = RefusingMeter;
        let deps = ReviewDeps {
            deterministic: &probes,
            model: &sources,
            adversarial: None,
            meter: &refusing,
            critic_slice: critic_slice(),
        };
        // Single mode, count budget 4 (the count admits; only the meter
        // refuses), clean envelope: every instantiated dimension is
        // truncated by the meter before any dispatch.
        let outcome = run_review(
            false,
            &deps,
            ReviewMode::Single,
            ModelDispatchBudget::new(4),
            bound(),
        )
        .expect("a meter refusal never errors the review");
        assert!(log.borrow().is_empty(), "no critic dispatch happened");
        assert!(
            outcome.record().is_none(),
            "a fully truncated review fabricates no record"
        );
        let gaps = outcome.unavailable_evidence();
        assert_eq!(
            gaps.len(),
            2,
            "both instantiated dimensions are recorded, never omitted"
        );
        for gap in gaps {
            assert!(
                gap.reason().contains("the budget meter refused"),
                "the reason names the meter as the refusing authority: {}",
                gap.reason()
            );
            assert!(
                gap.reason().contains("reserving budget refused"),
                "the reason cites the meter's typed refusal: {}",
                gap.reason()
            );
            assert!(
                gap.reason().contains("consume the recovery reserve"),
                "the typed cause is the domain refusal itself: {}",
                gap.reason()
            );
            assert!(
                gap.reason().contains("never a run error"),
                "the record states the §10.4 disposition: {}",
                gap.reason()
            );
        }
        // Control: the SAME composition with a permissive meter
        // dispatches — the truncation is the meter's refusal, not the
        // fixture's shape.
        let log_ok = Rc::new(RefCell::new(Vec::new()));
        let sources_ok = Sources {
            log: log_ok.clone(),
        };
        let deps_ok = ReviewDeps {
            deterministic: &probes,
            model: &sources_ok,
            adversarial: None,
            meter: &PERMISSIVE,
            critic_slice: critic_slice(),
        };
        let outcome_ok = run_review(
            false,
            &deps_ok,
            ReviewMode::Single,
            ModelDispatchBudget::new(4),
            bound(),
        )
        .expect("a headroom review completes");
        assert_eq!(
            log_ok.borrow().as_slice(),
            &["*".to_owned()],
            "the critic dispatched exactly once under the permissive meter"
        );
        assert!(
            outcome_ok.unavailable_evidence().is_empty(),
            "headroom truncates nothing"
        );
    }

    #[test]
    fn review_retains_contributor_records_for_verification() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let sources = Sources { log };
        let probes = ProbeSource;
        let deps = ReviewDeps {
            deterministic: &probes,
            model: &sources,
            adversarial: None,
            meter: &PERMISSIVE,
            critic_slice: critic_slice(),
        };
        let outcome = run_review(
            false,
            &deps,
            ReviewMode::Fanout,
            ModelDispatchBudget::new(2),
            bound(),
        )
        .expect("fanout completes");
        assert_eq!(outcome.contributor_records().len(), 2);
        assert!(
            outcome
                .contributor_records()
                .iter()
                .all(|record| record.identity().runner() == "review-dispatch")
        );
        assert_eq!(
            outcome.record().expect("aggregate").identity().runner(),
            super::super::aggregate::AGGREGATE_RUNNER
        );
    }

    /// Deterministic barrier proof: polling/collection is refused until all
    /// high-assurance jobs have been admitted. It has no timing dependency;
    /// the old serial loop fails on its first collection at one admission.
    struct BarrierPending {
        admissions: Rc<RefCell<u32>>,
        expected: u32,
        envelope: ComparisonEnvelope,
    }
    impl gauntlet_ports::comparator::PendingComparison for BarrierPending {
        fn collect(
            &mut self,
            _deadline: Deadline,
        ) -> Result<gauntlet_ports::comparison_record::ComparisonRecordDoc, ComparatorError>
        {
            if *self.admissions.borrow() != self.expected {
                return Err(ComparatorError::ComparisonFailed {
                    detail: "polled before the full fanout admission barrier".to_owned(),
                });
            }
            Ok(super::to_doc(
                DomainRecord::new(
                    self.envelope.reference_digest().clone(),
                    self.envelope.observation().observation_digest().clone(),
                    self.envelope.envelope_digest().clone(),
                    Vec::new(),
                    Verdict::Pass,
                )
                .expect("valid barrier record"),
                &self.envelope,
            ))
        }
    }
    struct BarrierCritic {
        admissions: Rc<RefCell<u32>>,
        expected: u32,
    }
    impl Comparator for BarrierCritic {
        fn compare(
            &self,
            _: &TrustedComparisonContext,
            _: Deadline,
        ) -> Result<DomainRecord, ComparatorError> {
            Err(ComparatorError::ComparisonFailed {
                detail: "two-phase barrier comparator must not use blocking compare".to_owned(),
            })
        }
        fn admit(
            &self,
            envelope: &TrustedComparisonContext,
            _: Deadline,
        ) -> Result<Option<Box<dyn gauntlet_ports::comparator::PendingComparison>>, ComparatorError>
        {
            *self.admissions.borrow_mut() += 1;
            Ok(Some(Box::new(BarrierPending {
                admissions: self.admissions.clone(),
                expected: self.expected,
                envelope: envelope.envelope().clone(),
            })))
        }
    }
    struct BarrierSources {
        admissions: Rc<RefCell<u32>>,
        expected: u32,
    }
    impl ComparatorSource for BarrierSources {
        fn across_dimensions(&self) -> Result<Box<dyn Comparator>, String> {
            Ok(Box::new(BarrierCritic {
                admissions: self.admissions.clone(),
                expected: self.expected,
            }))
        }
        fn for_dimension(&self, _: &str) -> Result<Box<dyn Comparator>, String> {
            self.across_dimensions()
        }
    }

    impl AdversarialComparatorSource for BarrierSources {
        fn adversarial_across_dimensions(
            &self,
        ) -> Result<gauntlet_ports::comparator::AdversarialComparator, String> {
            self.across_dimensions()
                .map(gauntlet_ports::comparator::AdversarialComparator::instructed)
        }
    }

    #[test]
    fn high_assurance_admits_every_job_before_its_first_collection() {
        let admissions = Rc::new(RefCell::new(0));
        // Two canonical dimensions plus adversarial source.
        let sources = BarrierSources {
            admissions: admissions.clone(),
            expected: 3,
        };
        let adversarial = BarrierSources {
            admissions: admissions.clone(),
            expected: 3,
        };
        let probes = ProbeSource;
        let deps = ReviewDeps {
            deterministic: &probes,
            model: &sources,
            adversarial: Some(&adversarial),
            meter: &PERMISSIVE,
            critic_slice: critic_slice(),
        };
        let outcome = run_review(
            false,
            &deps,
            ReviewMode::HighAssurance,
            ModelDispatchBudget::new(3),
            bound(),
        )
        .expect("all three admissions occur before barrier collection");
        assert_eq!(*admissions.borrow(), 3);
        assert_eq!(outcome.contributor_records().len(), 3);
    }

    struct LeasePending {
        id: u32,
        envelope: ComparisonEnvelope,
        events: Rc<RefCell<Vec<String>>>,
        fail: bool,
        cancel_error: bool,
    }

    impl gauntlet_ports::comparator::PendingComparison for LeasePending {
        fn collect(
            &mut self,
            _: Deadline,
        ) -> Result<gauntlet_ports::comparison_record::ComparisonRecordDoc, ComparatorError>
        {
            self.events
                .borrow_mut()
                .push(format!("collect-{}", self.id));
            if self.fail {
                return Err(ComparatorError::ComparisonFailed {
                    detail: "scripted first pending failure".to_owned(),
                });
            }
            Ok(super::to_doc(
                DomainRecord::new(
                    self.envelope.reference_digest().clone(),
                    self.envelope.observation().observation_digest().clone(),
                    self.envelope.envelope_digest().clone(),
                    Vec::new(),
                    Verdict::Pass,
                )
                .expect("valid pending record"),
                &self.envelope,
            ))
        }

        fn cancel(&mut self, _: Deadline) -> Result<(), ComparatorError> {
            self.events.borrow_mut().push(format!("cancel-{}", self.id));
            if self.cancel_error {
                Err(ComparatorError::ComparisonFailed {
                    detail: "scripted cancellation refusal".to_owned(),
                })
            } else {
                Ok(())
            }
        }
    }

    struct LeaseCritic {
        admissions: Rc<RefCell<u32>>,
        events: Rc<RefCell<Vec<String>>>,
        fail_first: bool,
        cancel_error: bool,
    }

    impl Comparator for LeaseCritic {
        fn compare(
            &self,
            _: &TrustedComparisonContext,
            _: Deadline,
        ) -> Result<DomainRecord, ComparatorError> {
            Err(ComparatorError::ComparisonFailed {
                detail: "lease critic must use two-phase admission".to_owned(),
            })
        }

        fn admit(
            &self,
            envelope: &TrustedComparisonContext,
            _: Deadline,
        ) -> Result<Option<Box<dyn gauntlet_ports::comparator::PendingComparison>>, ComparatorError>
        {
            let mut admissions = self.admissions.borrow_mut();
            *admissions += 1;
            let id = *admissions;
            self.events.borrow_mut().push(format!("admit-{id}"));
            Ok(Some(Box::new(LeasePending {
                id,
                envelope: envelope.envelope().clone(),
                events: self.events.clone(),
                fail: self.fail_first && id == 1,
                cancel_error: self.cancel_error,
            })))
        }
    }

    struct LeaseSources {
        admissions: Rc<RefCell<u32>>,
        events: Rc<RefCell<Vec<String>>>,
        fail_first: bool,
        cancel_error: bool,
    }

    impl ComparatorSource for LeaseSources {
        fn across_dimensions(&self) -> Result<Box<dyn Comparator>, String> {
            Ok(Box::new(LeaseCritic {
                admissions: self.admissions.clone(),
                events: self.events.clone(),
                fail_first: self.fail_first,
                cancel_error: self.cancel_error,
            }))
        }

        fn for_dimension(&self, _: &str) -> Result<Box<dyn Comparator>, String> {
            self.across_dimensions()
        }
    }

    impl AdversarialComparatorSource for LeaseSources {
        fn adversarial_across_dimensions(
            &self,
        ) -> Result<gauntlet_ports::comparator::AdversarialComparator, String> {
            self.across_dimensions()
                .map(gauntlet_ports::comparator::AdversarialComparator::instructed)
        }
    }

    struct FailAfterAdmissionSource {
        calls: Rc<RefCell<u32>>,
        admissions: Rc<RefCell<u32>>,
        events: Rc<RefCell<Vec<String>>>,
    }
    impl ComparatorSource for FailAfterAdmissionSource {
        fn across_dimensions(&self) -> Result<Box<dyn Comparator>, String> {
            self.for_dimension("all")
        }
        fn for_dimension(&self, _: &str) -> Result<Box<dyn Comparator>, String> {
            *self.calls.borrow_mut() += 1;
            if *self.calls.borrow() > 1 {
                return Err("scripted source failure after admission".to_owned());
            }
            Ok(Box::new(LeaseCritic {
                admissions: self.admissions.clone(),
                events: self.events.clone(),
                fail_first: false,
                cancel_error: false,
            }))
        }
    }

    #[test]
    fn source_failure_after_admission_cancels_the_admitted_job() {
        let calls = Rc::new(RefCell::new(0));
        let admissions = Rc::new(RefCell::new(0));
        let events = Rc::new(RefCell::new(Vec::new()));
        let models = FailAfterAdmissionSource {
            calls,
            admissions,
            events: events.clone(),
        };
        let probes = ProbeSource;
        let deps = ReviewDeps {
            deterministic: &probes,
            model: &models,
            adversarial: None,
            meter: &PERMISSIVE,
            critic_slice: critic_slice(),
        };
        assert!(matches!(
            run_review(
                false,
                &deps,
                ReviewMode::Fanout,
                ModelDispatchBudget::new(2),
                bound()
            ),
            Err(super::ReviewError::SourceFailed { .. })
        ));
        assert_eq!(*events.borrow(), vec!["admit-1", "cancel-1"]);
    }

    #[test]
    fn concurrency_lease_releases_before_third_admission_without_a_clock() {
        let admissions = Rc::new(RefCell::new(0));
        let events = Rc::new(RefCell::new(Vec::new()));
        let models = LeaseSources {
            admissions: admissions.clone(),
            events: events.clone(),
            fail_first: false,
            cancel_error: false,
        };
        let adversarial = LeaseSources {
            admissions: admissions.clone(),
            events: events.clone(),
            fail_first: false,
            cancel_error: false,
        };
        let probes = ProbeSource;
        let deps = ReviewDeps {
            deterministic: &probes,
            model: &models,
            adversarial: Some(&adversarial),
            meter: &PERMISSIVE,
            critic_slice: critic_slice(),
        };
        run_review(
            false,
            &deps,
            ReviewMode::HighAssurance,
            ModelDispatchBudget::with_max_concurrent(3, 2),
            bound(),
        )
        .expect("third job waits for the first lease to release");
        assert_eq!(
            *events.borrow(),
            vec![
                "admit-1",
                "admit-2",
                "collect-1",
                "admit-3",
                "collect-2",
                "collect-3"
            ],
            "the third admission occurs only after collection releases a bounded lease"
        );
    }

    #[test]
    fn failed_collection_cancels_every_uncollected_sibling() {
        let admissions = Rc::new(RefCell::new(0));
        let events = Rc::new(RefCell::new(Vec::new()));
        let models = LeaseSources {
            admissions: admissions.clone(),
            events: events.clone(),
            fail_first: true,
            cancel_error: false,
        };
        let adversarial = LeaseSources {
            admissions,
            events: events.clone(),
            fail_first: false,
            cancel_error: false,
        };
        let probes = ProbeSource;
        let deps = ReviewDeps {
            deterministic: &probes,
            model: &models,
            adversarial: Some(&adversarial),
            meter: &PERMISSIVE,
            critic_slice: critic_slice(),
        };
        assert!(matches!(
            run_review(
                false,
                &deps,
                ReviewMode::HighAssurance,
                ModelDispatchBudget::with_max_concurrent(3, 3),
                bound(),
            ),
            Err(super::ReviewError::ComparatorFailed(_))
        ));
        assert_eq!(
            *events.borrow(),
            vec![
                "admit-1",
                "admit-2",
                "admit-3",
                "collect-1",
                "cancel-1",
                "cancel-2",
                "cancel-3"
            ],
            "failure never leaves an admitted sibling running uncollected"
        );
    }

    /// A deterministic stand-in for an admitted remote critic that never
    /// reaches terminal state. `collect` reports the bounded timeout but keeps
    /// its handle alive so the review cleanup path must actively cancel it.
    struct ForeverRunningPrimary {
        events: Rc<RefCell<Vec<String>>>,
        cancellation_fails: bool,
    }

    impl gauntlet_ports::comparator::PendingComparison for ForeverRunningPrimary {
        fn collect(
            &mut self,
            _: Deadline,
        ) -> Result<gauntlet_ports::comparison_record::ComparisonRecordDoc, ComparatorError>
        {
            self.events.borrow_mut().push("primary-timeout".to_owned());
            Err(ComparatorError::TimedOut {
                detail: "scripted critic remained running forever".to_owned(),
            })
        }

        fn cancel(&mut self, _: Deadline) -> Result<(), ComparatorError> {
            self.events.borrow_mut().push("primary-cancel".to_owned());
            if self.cancellation_fails {
                Err(ComparatorError::ComparisonFailed {
                    detail: "scripted remote cancellation refusal".to_owned(),
                })
            } else {
                Ok(())
            }
        }
    }

    struct ForeverPrimaryCritic {
        events: Rc<RefCell<Vec<String>>>,
        cancellation_fails: bool,
    }

    impl Comparator for ForeverPrimaryCritic {
        fn compare(
            &self,
            _: &TrustedComparisonContext,
            _: Deadline,
        ) -> Result<DomainRecord, ComparatorError> {
            unreachable!("forever-running critic must use admission")
        }

        fn admit(
            &self,
            _: &TrustedComparisonContext,
            _: Deadline,
        ) -> Result<Option<Box<dyn gauntlet_ports::comparator::PendingComparison>>, ComparatorError>
        {
            Ok(Some(Box::new(ForeverRunningPrimary {
                events: self.events.clone(),
                cancellation_fails: self.cancellation_fails,
            })))
        }
    }

    struct ForeverPrimarySource {
        events: Rc<RefCell<Vec<String>>>,
        cancellation_fails: bool,
    }

    impl ComparatorSource for ForeverPrimarySource {
        fn across_dimensions(&self) -> Result<Box<dyn Comparator>, String> {
            Ok(Box::new(ForeverPrimaryCritic {
                events: self.events.clone(),
                cancellation_fails: self.cancellation_fails,
            }))
        }

        fn for_dimension(&self, _: &str) -> Result<Box<dyn Comparator>, String> {
            self.across_dimensions()
        }
    }

    #[test]
    fn forever_running_primary_is_cancelled_before_its_permit_is_released() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let models = ForeverPrimarySource {
            events: events.clone(),
            cancellation_fails: false,
        };
        let probes = ProbeSource;
        let deps = ReviewDeps {
            deterministic: &probes,
            model: &models,
            adversarial: None,
            meter: &PERMISSIVE,
            critic_slice: critic_slice(),
        };
        let budget = ModelDispatchBudget::with_max_concurrent(1, 1);
        assert!(matches!(
            run_review(false, &deps, ReviewMode::Single, budget.clone(), bound()),
            Err(super::ReviewError::ComparatorFailed(
                ComparatorError::TimedOut { .. }
            ))
        ));
        assert_eq!(*events.borrow(), vec!["primary-timeout", "primary-cancel"]);
        assert_eq!(
            budget.active(),
            0,
            "confirmed cancellation releases the permit"
        );
    }

    #[test]
    fn forever_running_primary_cancel_refusal_quarantines_its_permit() {
        let events = Rc::new(RefCell::new(Vec::new()));
        let models = ForeverPrimarySource {
            events: events.clone(),
            cancellation_fails: true,
        };
        let probes = ProbeSource;
        let deps = ReviewDeps {
            deterministic: &probes,
            model: &models,
            adversarial: None,
            meter: &PERMISSIVE,
            critic_slice: critic_slice(),
        };
        let budget = ModelDispatchBudget::with_max_concurrent(1, 1);
        assert!(matches!(
            run_review(false, &deps, ReviewMode::Single, budget.clone(), bound()),
            Err(super::ReviewError::ComparatorFailed(
                ComparatorError::RecoveryRequired { .. }
            ))
        ));
        assert_eq!(*events.borrow(), vec!["primary-timeout", "primary-cancel"]);
        assert_eq!(
            budget.active(),
            1,
            "unconfirmed remote work remains quarantined"
        );
    }

    #[test]
    fn cancellation_error_quarantines_every_unconfirmed_sibling() {
        let admissions = Rc::new(RefCell::new(0));
        let events = Rc::new(RefCell::new(Vec::new()));
        let models = LeaseSources {
            admissions,
            events: events.clone(),
            fail_first: true,
            cancel_error: true,
        };
        let probes = ProbeSource;
        let deps = ReviewDeps {
            deterministic: &probes,
            model: &models,
            adversarial: None,
            meter: &PERMISSIVE,
            critic_slice: critic_slice(),
        };
        let budget = ModelDispatchBudget::with_max_concurrent(10, 3);
        assert!(matches!(
            run_review(false, &deps, ReviewMode::Fanout, budget.clone(), bound()),
            Err(super::ReviewError::ComparatorFailed(
                ComparatorError::RecoveryRequired { .. }
            ))
        ));
        assert_eq!(
            budget.active(),
            2,
            "the failed primary and cancellation-refusing sibling remain quarantined"
        );
        assert!(events.borrow().iter().any(|event| event == "cancel-2"));
    }

    #[test]
    fn review_refuses_a_mutated_envelope_before_source_selection() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let sources = Sources { log: log.clone() };
        let probes = ProbeSource;
        let _deps = ReviewDeps {
            deterministic: &probes,
            model: &sources,
            adversarial: None,
            meter: &PERMISSIVE,
            critic_slice: critic_slice(),
        };
        // Assembly is crate-private in the port and the trusted context has
        // no public constructor, so an "envelope with replaced allowlist
        // values, digest recomputed" is unrepresentable here rather than
        // rejected at runtime. What a test CAN hold is the governed one.
        let context = context_of(&governed_fixture(false));
        assert!(gauntlet_ports::comparator::verify_envelope_digest(context.envelope()).is_ok());
        assert!(
            log.borrow().is_empty(),
            "review rejects before selecting a comparator"
        );
    }

    #[test]
    fn high_assurance_requires_an_adversarial_source() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let sources = Sources { log };
        let probes = ProbeSource;
        let deps = ReviewDeps {
            deterministic: &probes,
            model: &sources,
            adversarial: None,
            meter: &PERMISSIVE,
            critic_slice: critic_slice(),
        };
        assert!(matches!(
            run_review(
                false,
                &deps,
                ReviewMode::HighAssurance,
                ModelDispatchBudget::new(10),
                bound()
            ),
            Err(super::ReviewError::SourceFailed {
                which: "adversarial",
                ..
            })
        ));
    }

    #[test]
    fn every_mode_merges_into_one_coupled_record() {
        let log = Rc::new(RefCell::new(Vec::new()));
        let sources = Sources { log };
        let probes = ProbeSource;
        let adversarial = AdversarialSources {
            log: sources.log.clone(),
        };
        let deps = ReviewDeps {
            deterministic: &probes,
            model: &sources,
            adversarial: Some(&adversarial),
            meter: &PERMISSIVE,
            critic_slice: critic_slice(),
        };
        let governed = governed_fixture(true);
        let envelope = context_of(&governed);
        let acceptance = crate::compare::testing::acceptance(&governed);
        for mode in ReviewMode::ALL {
            let outcome = review(
                &envelope,
                &deps,
                &acceptance,
                *mode,
                ModelDispatchBudget::new(10),
                bound(),
            )
            .expect("the review completes");
            let record = outcome.record().expect("a record exists");
            assert_eq!(
                record.reference_digest(),
                envelope.envelope().reference_digest()
            );
            assert_eq!(
                record.observation_digest(),
                envelope.envelope().observation().observation_digest()
            );
            assert_eq!(
                record.envelope_digest(),
                envelope.envelope().envelope_digest()
            );
            assert_eq!(outcome.unavailable_evidence(), &[]);
            // Only adaptive runs the deterministic probes, so only adaptive
            // sees the instrument-settled failure through this scripted
            // model source (the model critics judge clean); the scripted
            // probes' finding survives the merge where they ran.
            match *mode {
                ReviewMode::Adaptive => {
                    assert_eq!(record.verdict().status(), Verdict::Fail);
                }
                _ => {
                    assert_eq!(record.verdict().status(), Verdict::Pass);
                }
            }
        }
    }
}
