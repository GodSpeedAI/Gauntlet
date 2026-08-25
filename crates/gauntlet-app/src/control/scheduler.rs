//! `UnitScheduler` — spec §11.2 eligibility and ordering, §11.3 slicing,
//! and the depends_on graph validation (Task-7 F7, adjudicated into Task 12).
//!
//! Spec §11.2, quoted in full where it decides behavior:
//!
//! "A Unit is dispatch-eligible only if **all** hold:
//!
//! - `status ∈ {open}` and not already claimed or in flight;
//! - every `depends_on` Unit is `settled` or `out_of_scope`;
//! - `blast_radius != irreversible` (scheduler-level, unconditional — §17.1);
//! - global and per-tier concurrency slots available;
//! - a runner is configured for the roles the slice needs;
//! - the budget meter will not breach the recovery reserve.
//!
//! Ordering: ascending blast-radius risk (`contained` first), then
//! dependency depth, then `UnitId` lexicographic. Deterministic ordering
//! matters: a scheduler whose behavior varies run to run cannot be compared
//! against itself."
//!
//! §11.3: "A slice inherits the **highest** blast radius and the **most
//! restrictive** authority of its members. Slices never mix `irreversible`
//! members with anything." The authority lattice is the domain's own
//! (`authority_rank`: critic < lead_agent < human), pinned by the §5.3.2
//! coupling rules — never a local copy.
//!
//! ## Purity contract
//!
//! The scheduler holds no state, reads no store, and consults no clock.
//! Every fact it decides on arrives in [`PlanInput`], assembled FRESH by
//! the composition (slice B3) each tick — §11.4 step 3's "re-evaluate
//! eligibility from scratch" is the caller's wiring duty, and a stale input
//! here is a caller defect. The one live consultation is the budget
//! conjunct: the scheduler itself asks the injected [`BudgetMeter`]
//! `would_breach_reserve` at call time (§17.4: consulted before every
//! dispatch, not after) — a precomputed boolean from the caller has no
//! parameter to travel through.

use std::collections::BTreeMap;

use gauntlet_domain::{Authority, BlastRadius, RoleId};
use gauntlet_domain::{Unit, UnitId, UnitStatus, authority_rank, blast_rank};
use gauntlet_ports::budget_meter::{BudgetMeter, BudgetSlice};

use gauntlet_domain::InvocationId;

// ---------------------------------------------------------------------------
// Inputs — the fresh facts one scheduling decision runs on
// ---------------------------------------------------------------------------

/// Current concurrency occupancy (§11.2 conjunct 4): the global slot count
/// and the per-tier slot counts, with what is in flight right now. The
/// plan's own output does NOT consume these — planning answers "which
/// Units are eligible NOW, in what order"; how many dispatches may start
/// concurrently is the controller's dispatch loop, bounded by the same
/// numbers. The tier lists are keyed by the tier itself (at most four
/// entries); a tier WITHOUT a capacity entry is a tier no policy admits
/// (§17.3) — its Units are not dispatch-eligible, reported as exactly
/// that, never silently treated as unlimited.
#[derive(Debug, Clone, Default)]
pub struct Concurrency {
    pub global_capacity: u32,
    pub global_in_flight: u32,
    pub tier_capacity: Vec<(BlastRadius, u32)>,
    pub tier_in_flight: Vec<(BlastRadius, u32)>,
}

impl Concurrency {
    /// The configured capacity for one tier, when a policy admits it.
    pub fn tier_capacity(&self, tier: BlastRadius) -> Option<u32> {
        self.tier_capacity
            .iter()
            .find(|(key, _)| *key == tier)
            .map(|(_, capacity)| *capacity)
    }

    /// The current in-flight count for one tier.
    pub fn tier_in_flight(&self, tier: BlastRadius) -> u32 {
        self.tier_in_flight
            .iter()
            .find(|(key, _)| *key == tier)
            .map(|(_, count)| *count)
            .unwrap_or(0)
    }
}

/// The plan input: the answer-key snapshot, current occupancy, configured
/// roles, slice cohorting, and per-Unit dispatch estimates. All fresh, all
/// caller-assembled (see the module's purity contract).
#[derive(Debug, Clone, Default)]
pub struct PlanInput {
    pub units: Vec<Unit>,
    /// Units currently claimed or in flight, with the invocation holding
    /// them (§11.2 conjunct 1's second clause).
    pub claimed: Vec<(UnitId, InvocationId)>,
    pub concurrency: Concurrency,
    /// Roles for which a runner is configured (§11.2 conjunct 5).
    pub configured_roles: Vec<RoleId>,
    /// Slice cohorting (§11.3): Units keyed to the same [`CohortId`] are
    /// not meaningfully separable and are planned as one slice. Unkeyed
    /// Units dispatch alone.
    pub cohorts: BTreeMap<UnitId, CohortId>,
    /// The estimated forward slice one dispatch of each Unit needs — the
    /// number the meter conjunct is decided on. Required for every Unit
    /// that reaches the budget conjunct; a missing estimate is a typed
    /// input error, never a zero.
    pub estimates: BTreeMap<UnitId, BudgetSlice>,
    /// The caller's DURABLE status projection (Task-12 correction C6.2).
    ///
    /// Membership and eligibility are different questions. §11.2 conjunct
    /// 2 asks "is this Unit's dependency settled or out_of_scope" — a
    /// question that requires the dependency to be IN the graph — while
    /// conjunct 1 refuses a Unit whose own status is not open. A caller
    /// that answered the first question by DELETING settled members from
    /// `units` makes multi-hop graphs uncompletable (`UnknownDependency`
    /// the first plan after a settlement), so `units` carries the whole
    /// membership and this map carries what each member's status actually
    /// IS.
    ///
    /// Why a projection rather than the Unit's own parsed status: §5.3.2
    /// makes `status = settled` a fact about a `VerificationRecord` with
    /// `Outcome = PASS`, and a controller that never writes status back to
    /// the answer key would read a settled dependency's FILE status as
    /// `open` forever. An absent entry falls back to the parsed status —
    /// the honest reading for a caller planning over an unstarted run.
    pub statuses: BTreeMap<UnitId, UnitStatus>,
}

impl PlanInput {
    /// The status this plan decides `unit` on: the caller's durable
    /// projection where it carries one, otherwise the Unit's own parsed
    /// status.
    pub fn status_of(&self, unit: &Unit) -> UnitStatus {
        self.statuses
            .get(unit.id())
            .copied()
            .unwrap_or_else(|| unit.status())
    }
}

/// A slice cohort key (§11.3 "same file, same invariant"): Units sharing a
/// cohort are grouped into one builder dispatch.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct CohortId {
    cohort: String,
}

impl CohortId {
    /// A cohort key; non-empty and whitespace-free, like every reference
    /// token in the domain vocabulary.
    pub fn new(cohort: &str) -> Result<Self, SchedulerError> {
        if cohort.is_empty() || cohort.bytes().any(|b| b.is_ascii_whitespace()) {
            return Err(SchedulerError::CohortInvalid {
                cohort: cohort.to_owned(),
            });
        }
        Ok(Self {
            cohort: cohort.to_owned(),
        })
    }

    pub fn as_str(&self) -> &str {
        &self.cohort
    }
}

// ---------------------------------------------------------------------------
// Eligibility — §11.2, all conjuncts
// ---------------------------------------------------------------------------

/// Why a Unit is not dispatch-eligible — one typed reason per failed
/// §11.2 conjunct, checked in the spec's own bullet order, each carrying
/// the blocking fact.
#[derive(Debug, Clone, PartialEq)]
pub enum Ineligible {
    /// Conjunct 1a: the status is not `open`.
    StatusNotOpen { unit: UnitId, status: UnitStatus },
    /// Conjunct 1b: the Unit is claimed or in flight.
    Claimed {
        unit: UnitId,
        invocation: InvocationId,
    },
    /// Conjunct 2: a `depends_on` Unit is not `settled`/`out_of_scope`
    /// (`status: None` — the target is absent from the input entirely).
    DependencyUnresolved {
        unit: UnitId,
        dependency: UnitId,
        status: Option<UnitStatus>,
    },
    /// Conjunct 3: irreversible work is never an automatic dispatch
    /// candidate — "scheduler-level, unconditional, independent of what
    /// any prompt says" (§17.1).
    Irreversible { unit: UnitId },
    /// Conjunct 4a: no global slot is free.
    ConcurrencyGlobal {
        unit: UnitId,
        capacity: u32,
        in_flight: u32,
    },
    /// Conjunct 4b: no slot free at the Unit's tier.
    ConcurrencyTier {
        unit: UnitId,
        tier: BlastRadius,
        capacity: u32,
        in_flight: u32,
    },
    /// Conjunct 4c: the Unit's tier has no concurrency entry — no policy
    /// admits it (§17.3).
    TierConcurrencyUnconfigured { unit: UnitId, tier: BlastRadius },
    /// Conjunct 5: no runner is configured for a role the dispatch needs.
    RunnerUnconfigured { unit: UnitId, role: RoleId },
    /// Conjunct 6, undecided: the input carries no dispatch estimate for
    /// this Unit, so the budget conjunct cannot be decided — an unestimated
    /// dispatch is an undecided fact, never an assumed zero. `plan()`
    /// pre-validates estimates and reports the typed
    /// [`SchedulerError::MissingEstimate`]; this arm is the direct-call
    /// surface of the same broken-input condition.
    EstimateMissing { unit: UnitId },
    /// Conjunct 6: the meter refused the estimated slice (§5.3.7 reserve).
    BudgetReserveBreach { unit: UnitId, slice: BudgetSlice },
}

impl Ineligible {
    /// The Unit this refusal names — the controller's §11.6 terminal-truth
    /// key (slice B3 correction C1): which units the loop still owes work
    /// on is decided per-unit, from the refusal's own subject.
    pub fn unit(&self) -> &UnitId {
        match self {
            Self::StatusNotOpen { unit, .. }
            | Self::Claimed { unit, .. }
            | Self::DependencyUnresolved { unit, .. }
            | Self::Irreversible { unit }
            | Self::ConcurrencyGlobal { unit, .. }
            | Self::ConcurrencyTier { unit, .. }
            | Self::TierConcurrencyUnconfigured { unit, .. }
            | Self::RunnerUnconfigured { unit, .. }
            | Self::EstimateMissing { unit }
            | Self::BudgetReserveBreach { unit, .. } => unit,
        }
    }

    /// Whether this refusal is a BUDGET fact (§11.6: `budget_exhausted`
    /// requires "a budget dimension breached" — nothing else may wear that
    /// terminal).
    pub fn is_budget_block(&self) -> bool {
        matches!(self, Self::BudgetReserveBreach { .. })
    }

    /// Whether this refusal is a GOVERNANCE fact (§11.6 `blocked`: "≥1 Unit
    /// requiring an authority the run cannot exercise; escalation ticket
    /// created"). Irreversibility, an unadmitted tier, a missing runner,
    /// and an operator-frozen status all leave the run unable to proceed
    /// on any budget.
    pub fn is_governance_block(&self) -> bool {
        matches!(
            self,
            Self::Irreversible { .. }
                | Self::TierConcurrencyUnconfigured { .. }
                | Self::RunnerUnconfigured { .. }
                | Self::StatusNotOpen { .. }
        )
    }
}

impl core::fmt::Display for Ineligible {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::StatusNotOpen { unit, status } => {
                write!(f, "Unit {unit} is {status}, not open (spec 11.2)")
            }
            Self::Claimed { unit, invocation } => {
                write!(
                    f,
                    "Unit {unit} is already claimed by {invocation} (spec 11.2)"
                )
            }
            Self::DependencyUnresolved {
                unit,
                dependency,
                status,
            } => match status {
                Some(status) => write!(
                    f,
                    "Unit {unit} depends on {dependency}, which is {status}, not settled or \
                     out_of_scope (spec 11.2)"
                ),
                None => write!(
                    f,
                    "Unit {unit} depends on {dependency}, which the plan input does not carry \
                     (spec 11.2)"
                ),
            },
            Self::Irreversible { unit } => write!(
                f,
                "Unit {unit} is irreversible: never an automatic dispatch candidate — \
                 scheduler-level, unconditional (spec 11.2, 17.1)"
            ),
            Self::ConcurrencyGlobal {
                unit,
                capacity,
                in_flight,
            } => write!(
                f,
                "no global concurrency slot for Unit {unit}: {in_flight} of {capacity} in \
                 flight (spec 11.2)"
            ),
            Self::ConcurrencyTier {
                unit,
                tier,
                capacity,
                in_flight,
            } => write!(
                f,
                "no {tier} concurrency slot for Unit {unit}: {in_flight} of {capacity} in \
                 flight (spec 11.2)"
            ),
            Self::TierConcurrencyUnconfigured { unit, tier } => write!(
                f,
                "Unit {unit}'s tier {tier} has no concurrency entry — no policy admits it \
                 (spec 11.2, 17.3)"
            ),
            Self::RunnerUnconfigured { unit, role } => write!(
                f,
                "no runner is configured for the {role} role Unit {unit} needs (spec 11.2)"
            ),
            Self::EstimateMissing { unit } => write!(
                f,
                "Unit {unit} has no dispatch estimate — the budget conjunct is decided on the \
                 meter against the estimate, and an unestimated dispatch is an undecided fact"
            ),
            Self::BudgetReserveBreach { unit, slice } => write!(
                f,
                "the meter refuses Unit {unit}'s estimated slice ({} tokens): it would breach \
                 the recovery reserve (spec 11.2, 5.3.7)",
                slice.tokens
            ),
        }
    }
}

// ---------------------------------------------------------------------------
// Errors — input-shape failures, not eligibility answers
// ---------------------------------------------------------------------------

/// Plan-input failures — typed, naming the offending Unit(s). These are
/// NOT ineligibility: the input itself is wrong, and deciding anything on
/// it would be deciding on invented facts.
#[derive(Debug, Clone, PartialEq)]
pub enum SchedulerError {
    /// A `depends_on` names a Unit the plan input does not carry (Task-7
    /// F7). A dangling dependency would simply never become eligible —
    /// the load-time check names it instead of leaving a silent deadlock.
    UnknownDependency { unit: UnitId, dependency: UnitId },
    /// The depends_on graph has a cycle (Task-7 F7), named as the path.
    DependencyCycle { cycle: Vec<UnitId> },
    /// The same Unit appears twice in the input.
    DuplicateUnit { unit: UnitId },
    /// A Unit reached the budget conjunct with no estimate. Never a zero:
    /// an unestimated dispatch is an undecided fact.
    MissingEstimate { unit: UnitId },
    /// A cohort key was not usable.
    CohortInvalid { cohort: String },
    /// §11.3's never-mix rule fired during slice construction. Unreachable
    /// through `plan` (irreversible Units are ineligible long before
    /// slicing) and kept as a typed guard so no future path can build the
    /// forbidden slice.
    IrreversibleSliceMember { unit: UnitId },
}

impl core::fmt::Display for SchedulerError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::UnknownDependency { unit, dependency } => write!(
                f,
                "Unit {unit} depends on {dependency}, which the plan input does not carry — a \
                 dangling depends_on never becomes eligible; fix the answer key (spec 5.3.2)"
            ),
            Self::DependencyCycle { cycle } => {
                let path: Vec<String> = cycle.iter().map(|u| u.as_str().to_owned()).collect();
                write!(
                    f,
                    "the depends_on graph has a cycle: {} (ordering only — a cycle can never \
                     order; spec 5.3.2)",
                    path.join(" -> ")
                )
            }
            Self::DuplicateUnit { unit } => {
                write!(f, "Unit {unit} appears more than once in the plan input")
            }
            Self::MissingEstimate { unit } => write!(
                f,
                "Unit {unit} has no dispatch estimate — the budget conjunct is decided on the \
                 meter, and an unestimated dispatch is an undecided fact"
            ),
            Self::CohortInvalid { cohort } => {
                write!(
                    f,
                    "cohort key {cohort:?} must be non-empty and whitespace-free"
                )
            }
            Self::IrreversibleSliceMember { unit } => write!(
                f,
                "Unit {unit} is irreversible and can never share a slice (spec 11.3, 17.1)"
            ),
        }
    }
}

impl std::error::Error for SchedulerError {}

// ---------------------------------------------------------------------------
// The plan output
// ---------------------------------------------------------------------------

/// One planned dispatch: a single Unit, or one §11.3 slice. The inherited
/// tier facts (highest blast radius, most restrictive authority) are
/// computed here, once, from the members — never re-derived downstream.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedDispatch {
    pub units: Vec<UnitId>,
    pub blast_radius: BlastRadius,
    pub authority: Authority,
    /// The summed member estimates: one builder performs every member's
    /// work, so one dispatch pays for all of it.
    pub estimated_slice: BudgetSlice,
    /// The deepest member's dependency depth (see [`UnitScheduler::plan`]
    /// for the depth rule).
    pub depth: u32,
}

impl PlannedDispatch {
    /// A single-Unit dispatch.
    pub fn single(
        unit: UnitId,
        blast_radius: BlastRadius,
        authority: Authority,
        estimated_slice: BudgetSlice,
        depth: u32,
    ) -> Self {
        Self {
            units: vec![unit],
            blast_radius,
            authority,
            estimated_slice,
            depth,
        }
    }

    /// Whether this dispatch is a multi-Unit slice (§11.3).
    pub fn is_slice(&self) -> bool {
        self.units.len() > 1
    }
}

// ---------------------------------------------------------------------------
// The scheduler
// ---------------------------------------------------------------------------

/// The §11.2 scheduler: pure eligibility + ordering + slicing over fresh
/// facts. Stateless by design — see the module's purity contract.
#[derive(Debug, Clone)]
pub struct UnitScheduler;

impl UnitScheduler {
    /// No `Default` impl on purpose (the domain `DraftUnit` precedent): a
    /// default scheduler would imply a default scheduling policy, and
    /// defaults are §8.6 configuration, not domain state.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self
    }

    /// Evaluates one Unit against every §11.2 conjunct, in the spec's
    /// bullet order. `Ok(())` is dispatch-eligible; every refusal carries
    /// the blocking fact. The budget conjunct consults the meter HERE, at
    /// call time — never a caller-supplied answer.
    pub fn eligible(
        &self,
        unit: &Unit,
        input: &PlanInput,
        meter: &dyn BudgetMeter,
    ) -> Result<(), Ineligible> {
        // Conjunct 1 — status open, and not claimed or in flight. The
        // status is the input's DURABLE projection (correction C6.2), so a
        // settled member re-included for graph membership is refused here
        // rather than dispatched again.
        let status = input.status_of(unit);
        if status != UnitStatus::Open {
            return Err(Ineligible::StatusNotOpen {
                unit: unit.id().clone(),
                status,
            });
        }
        if let Some((_, invocation)) = input
            .claimed
            .iter()
            .find(|(claimed, _)| claimed == unit.id())
        {
            return Err(Ineligible::Claimed {
                unit: unit.id().clone(),
                invocation: invocation.clone(),
            });
        }
        // Conjunct 2 — every depends_on Unit settled or out_of_scope. The
        // status is looked up from the input's own snapshot, through the
        // same durable projection conjunct 1 used (correction C6.2): a
        // dependency the run settled reads settled even though the answer
        // key it was parsed from still says open.
        let by_id = |id: &UnitId| input.units.iter().find(|u| u.id() == id);
        for dependency in unit.depends_on() {
            let status = by_id(dependency).map(|dep| input.status_of(dep));
            if status != Some(UnitStatus::Settled) && status != Some(UnitStatus::OutOfScope) {
                return Err(Ineligible::DependencyUnresolved {
                    unit: unit.id().clone(),
                    dependency: dependency.clone(),
                    status,
                });
            }
        }
        // Conjunct 3 — irreversible is never an automatic candidate.
        // §17.1: "Scheduler-level, unconditional, independent of what any
        // prompt says."
        if unit.blast_radius() == BlastRadius::Irreversible {
            return Err(Ineligible::Irreversible {
                unit: unit.id().clone(),
            });
        }
        // Conjunct 4 — global and per-tier concurrency. A tier with no
        // capacity entry is a tier no policy admits (§17.3), not a tier
        // with infinite slots.
        let in_flight = input.concurrency.global_in_flight;
        if in_flight >= input.concurrency.global_capacity {
            return Err(Ineligible::ConcurrencyGlobal {
                unit: unit.id().clone(),
                capacity: input.concurrency.global_capacity,
                in_flight,
            });
        }
        let tier = unit.blast_radius();
        let tier_capacity = input.concurrency.tier_capacity(tier).ok_or(
            Ineligible::TierConcurrencyUnconfigured {
                unit: unit.id().clone(),
                tier,
            },
        )?;
        let tier_in_flight = input.concurrency.tier_in_flight(tier);
        if tier_in_flight >= tier_capacity {
            return Err(Ineligible::ConcurrencyTier {
                unit: unit.id().clone(),
                tier,
                capacity: tier_capacity,
                in_flight: tier_in_flight,
            });
        }
        // Conjunct 5 — a runner is configured for the roles the dispatch
        // needs. A Unit slice dispatches to one builder (§11.3).
        if !input.configured_roles.contains(&RoleId::Builder) {
            return Err(Ineligible::RunnerUnconfigured {
                unit: unit.id().clone(),
                role: RoleId::Builder,
            });
        }
        // Conjunct 6 — the meter, consulted now: would this Unit's
        // estimated slice breach the recovery reserve?
        let slice = input
            .estimates
            .get(unit.id())
            .ok_or(Ineligible::EstimateMissing {
                unit: unit.id().clone(),
            })?;
        if meter.would_breach_reserve(*slice) {
            return Err(Ineligible::BudgetReserveBreach {
                unit: unit.id().clone(),
                slice: *slice,
            });
        }
        Ok(())
    }

    /// Builds the ordered plan over `input`:
    ///
    /// 1. validates the input shape: duplicate Units, missing estimates,
    ///    and the depends_on graph (unknown targets and cycles are typed
    ///    errors naming the offender — Task-7 F7);
    /// 2. evaluates every Unit's eligibility (§11.2);
    /// 3. groups eligible Units into §11.3 slices by cohort, inheriting
    ///    the highest blast radius and most restrictive authority;
    /// 4. orders the dispatches: ascending blast-radius risk, then
    ///    dependency depth ASCENDING (dependencies first — a Unit's depth
    ///    is strictly greater than each of its dependencies', so ascending
    ///    depth is the ordering that respects `depends_on`'s "ordering
    ///    only" role while the spec's leading key stays "ascending"),
    ///    then `UnitId` lexicographic. Determinism is the invariant:
    ///    shuffled input plans identically.
    pub fn plan(
        &self,
        input: &PlanInput,
        meter: &dyn BudgetMeter,
    ) -> Result<Vec<PlannedDispatch>, SchedulerError> {
        let by_id = validate_input_shape(input)?;
        let depths = dependency_depths(&by_id);

        let mut dispatches: Vec<PlannedDispatch> = Vec::new();
        let mut eligible_units: Vec<&Unit> = Vec::new();
        for unit in &input.units {
            if self.eligible(unit, input, meter).is_ok() {
                eligible_units.push(unit);
            }
        }
        // Slice cohorting (§11.3): group ELIGIBLE members sharing a cohort.
        let mut grouped: BTreeMap<&CohortId, Vec<&Unit>> = BTreeMap::new();
        let mut singles: Vec<&Unit> = Vec::new();
        for unit in eligible_units {
            match input.cohorts.get(unit.id()) {
                Some(cohort) => grouped.entry(cohort).or_default().push(unit),
                None => singles.push(unit),
            }
        }
        let build = |members: &[&Unit]| -> Result<PlannedDispatch, SchedulerError> {
            let mut blast = BlastRadius::Contained;
            let mut authority = Authority::Critic;
            let mut slice = BudgetSlice {
                rounds: 0,
                wall_seconds: 0,
                tokens: 0,
                cost_usd: gauntlet_domain::MicroUsd::new(0),
            };
            let mut depth = 0u32;
            let mut ids = Vec::with_capacity(members.len());
            for unit in members {
                // §11.3: never mix irreversible members — unreachable via
                // plan (eligibility already refused them), guarded anyway.
                if unit.blast_radius() == BlastRadius::Irreversible {
                    return Err(SchedulerError::IrreversibleSliceMember {
                        unit: unit.id().clone(),
                    });
                }
                if blast_rank(unit.blast_radius()) > blast_rank(blast) {
                    blast = unit.blast_radius();
                }
                if authority_rank(unit.authority()) > authority_rank(authority) {
                    authority = unit.authority();
                }
                let estimate =
                    input
                        .estimates
                        .get(unit.id())
                        .ok_or(SchedulerError::MissingEstimate {
                            unit: unit.id().clone(),
                        })?;
                slice.rounds = slice.rounds.saturating_add(estimate.rounds);
                slice.wall_seconds = slice.wall_seconds.saturating_add(estimate.wall_seconds);
                slice.tokens = slice.tokens.saturating_add(estimate.tokens);
                slice.cost_usd = gauntlet_domain::MicroUsd::new(
                    slice.cost_usd.get().saturating_add(estimate.cost_usd.get()),
                );
                depth = depth.max(*depths.get(unit.id()).unwrap_or(&0));
                ids.push(unit.id().clone());
            }
            Ok(PlannedDispatch {
                units: ids,
                blast_radius: blast,
                authority,
                estimated_slice: slice,
                depth,
            })
        };
        for members in grouped.values() {
            dispatches.push(build(members)?);
        }
        for unit in singles {
            dispatches.push(build(&[unit])?);
        }
        dispatches.sort_by(|a, b| {
            (blast_rank(a.blast_radius), a.depth, &a.units).cmp(&(
                blast_rank(b.blast_radius),
                b.depth,
                &b.units,
            ))
        });
        Ok(dispatches)
    }

    /// The typed §11.2 refusal for every NON-eligible Unit of the input,
    /// after the same shape validation `plan` performs (an invalid input is
    /// an error, never a batch of reasons). This is the controller's §11.6
    /// terminal-truth input (slice B3 correction C1): when a plan admits
    /// nothing, WHETHER the run is budget-exhausted or governance-blocked
    /// is decided from these typed reasons — `plan`'s boolean filter
    /// discards exactly the facts the terminal needs.
    pub fn ineligibility_report(
        &self,
        input: &PlanInput,
        meter: &dyn BudgetMeter,
    ) -> Result<Vec<Ineligible>, SchedulerError> {
        let by_id = validate_input_shape(input)?;
        let _ = by_id;
        let mut reasons = Vec::new();
        for unit in &input.units {
            if let Err(reason) = self.eligible(unit, input, meter) {
                reasons.push(reason);
            }
        }
        Ok(reasons)
    }
}

/// The input-shape validation every plan-side entry point shares: one
/// entry per Unit (duplicates are typed), an estimate for every Unit
/// (eligible and ineligible alike — the budget conjunct is decided against
/// it), and the Task-7 F7 depends_on graph (unknown targets and cycles
/// are typed errors naming the offender).
fn validate_input_shape(input: &PlanInput) -> Result<BTreeMap<&UnitId, &Unit>, SchedulerError> {
    let mut by_id: BTreeMap<&UnitId, &Unit> = BTreeMap::new();
    for unit in &input.units {
        if by_id.insert(unit.id(), unit).is_some() {
            return Err(SchedulerError::DuplicateUnit {
                unit: unit.id().clone(),
            });
        }
    }
    for unit in &input.units {
        if !input.estimates.contains_key(unit.id()) {
            return Err(SchedulerError::MissingEstimate {
                unit: unit.id().clone(),
            });
        }
    }
    validate_graph(&by_id)?;
    Ok(by_id)
}

/// Task-7 F7 validation: every `depends_on` target must exist in the input
/// (unknown ⇒ typed error naming it), and the graph must be acyclic (a
/// cycle ⇒ typed error naming the path, start-to-start). Units are visited
/// in `UnitId` order so the reported offender is a pure function of the
/// input.
fn validate_graph(by_id: &BTreeMap<&UnitId, &Unit>) -> Result<(), SchedulerError> {
    for (id, unit) in by_id {
        for dependency in unit.depends_on() {
            if !by_id.contains_key(dependency) {
                return Err(SchedulerError::UnknownDependency {
                    unit: (*id).clone(),
                    dependency: dependency.clone(),
                });
            }
        }
    }
    // Three-color DFS: WHITE=0 (unvisited), GRAY=1 (on the current path),
    // BLACK=2 (fully explored). An edge into a GRAY node closes a cycle.
    // The map is keyed by the input's own &UnitId references, so edges are
    // resolved back to those keys by id text.
    let mut color: BTreeMap<&UnitId, u8> = by_id.keys().map(|key| (*key, 0u8)).collect();
    let key_of = |id: &UnitId| {
        by_id
            .keys()
            .find(|key| key.as_str() == id.as_str())
            .expect("every depends_on target was validated present")
    };
    for start in by_id.keys().copied().collect::<Vec<_>>() {
        if color.get(start) != Some(&0) {
            continue;
        }
        let mut stack: Vec<(&UnitId, usize)> = vec![(start, 0)];
        color.insert(start, 1);
        while let Some((node, next)) = stack.pop() {
            let dependencies = by_id.get(node).expect("keyed by this map").depends_on();
            if next >= dependencies.len() {
                color.insert(node, 2);
                continue;
            }
            stack.push((node, next + 1));
            let dependency = &dependencies[next];
            match color.get(key_of(dependency)).copied() {
                Some(1) => {
                    // Gray: the stack holds the path from `dependency` down
                    // to `node`; append `dependency` to close it.
                    let from = stack
                        .iter()
                        .position(|(member, _)| member.as_str() == dependency.as_str())
                        .expect("a gray dependency is on the current path");
                    let mut cycle: Vec<UnitId> = stack[from..]
                        .iter()
                        .map(|(member, _)| (*member).clone())
                        .collect();
                    cycle.push(dependency.clone());
                    return Err(SchedulerError::DependencyCycle { cycle });
                }
                Some(0) => {
                    let key = key_of(dependency);
                    color.insert(key, 1);
                    stack.push((key, 0));
                }
                _ => {}
            }
        }
    }
    Ok(())
}

/// Dependency depth per Unit: 0 for a Unit with no `depends_on`, else
/// `1 + max(depth of its dependencies)`. Because a dependency's depth is
/// ALWAYS strictly smaller than its dependent's, ascending depth orders
/// dependencies before dependents — the §11.2 ordering key. Called only
/// after [`validate_graph`] (cycles would not terminate).
fn dependency_depths(by_id: &BTreeMap<&UnitId, &Unit>) -> BTreeMap<UnitId, u32> {
    fn depth_of(
        id: &UnitId,
        by_id: &BTreeMap<&UnitId, &Unit>,
        memo: &mut BTreeMap<UnitId, u32>,
    ) -> u32 {
        if let Some(depth) = memo.get(id) {
            return *depth;
        }
        let unit = by_id
            .get(
                &by_id
                    .keys()
                    .find(|key| key.as_str() == id.as_str())
                    .copied()
                    .expect("depths are computed over validated inputs"),
            )
            .copied()
            .expect("the key came from this map");
        let depth = unit
            .depends_on()
            .iter()
            .map(|dependency| depth_of(dependency, by_id, memo))
            .max()
            .map_or(0, |deepest| deepest + 1);
        memo.insert(id.clone(), depth);
        depth
    }
    let mut memo = BTreeMap::new();
    for id in by_id.keys() {
        depth_of(id, by_id, &mut memo);
    }
    memo
}

#[cfg(test)]
mod tests {
    use super::*;
    use gauntlet_domain::{
        Budget, DiagnosticAllowance, EvidenceRequirement, Limits, MicroUsd, Producer, UnitError,
    };
    use gauntlet_domain::{EvidenceKind, EvidenceLocator, VerifyBy};
    use gauntlet_ports::budget_meter::BudgetMeterError;
    use gauntlet_ports::budget_meter::Remaining;

    fn unit_id(n: u16) -> UnitId {
        UnitId::new(&format!("AK-{n:03}")).expect("valid unit id")
    }

    fn requirement() -> EvidenceRequirement {
        EvidenceRequirement::new(
            EvidenceKind::TestReport,
            EvidenceLocator::new("fixture-report-locator").expect("valid locator"),
            Producer::Instrument,
        )
    }

    fn unit(n: u16, blast: BlastRadius, authority: Authority, depends_on: Vec<UnitId>) -> Unit {
        try_unit(n, blast, authority, depends_on).expect("valid unit")
    }

    fn try_unit(
        n: u16,
        blast: BlastRadius,
        authority: Authority,
        depends_on: Vec<UnitId>,
    ) -> Result<Unit, UnitError> {
        Unit::new(
            unit_id(n),
            format!("claim {n}"),
            VerifyBy::StaticCheck,
            blast,
            authority,
            requirement(),
            depends_on,
        )
    }

    /// A settled dependency with id 901.
    fn settled_dependency() -> Unit {
        let mut dep = unit(901, BlastRadius::Contained, Authority::Critic, Vec::new());
        dep.transition(UnitStatus::InProgress).expect("moves");
        dep.transition(UnitStatus::OutOfScope).expect("moves");
        dep
    }

    // Named test facts — §8.1 literal discipline: duration- and
    // cost-shaped values are never inline numbers, in tests as in source.
    const SLICE_ROUNDS: u32 = 1;
    const SLICE_SPAN: u64 = 60;
    const SLICE_COST: u64 = 5;
    const RUN_ROUNDS: u32 = 8;
    const RUN_PARALLEL: u32 = 2;
    const RUN_DEPTH: u32 = 2;
    const RUN_SPAN: u64 = 3600;
    const RUN_TOKENS: u64 = 1000;
    const RUN_COST: u64 = 500;
    const RESERVE_PCT: u8 = 20;
    const DIAG_TOKENS: u64 = 100;
    const DIAG_COST: u64 = 50;
    const SLOTS: u32 = 2;

    fn slice(tokens: u64) -> BudgetSlice {
        BudgetSlice {
            rounds: SLICE_ROUNDS,
            wall_seconds: SLICE_SPAN,
            tokens,
            cost_usd: MicroUsd::new(SLICE_COST),
        }
    }

    /// The real domain budget behind the meter — refusal arithmetic is the
    /// domain's, so the test builds a genuine one.
    fn meter_budget() -> Budget {
        Budget::new(
            Limits::new(
                RUN_ROUNDS,
                RUN_PARALLEL,
                RUN_DEPTH,
                RUN_SPAN,
                RUN_TOKENS,
                MicroUsd::new(RUN_COST),
            )
            .expect("usable"),
            RESERVE_PCT,
            DiagnosticAllowance::new(DIAG_TOKENS, MicroUsd::new(DIAG_COST)),
        )
        .expect("usable")
    }

    /// A meter over a real domain budget: refuses exactly what the domain's
    /// charge would refuse (1000 tokens, 20 percent reserve ⇒ 800 forward).
    struct RealMeter {
        budget: Budget,
    }

    impl BudgetMeter for RealMeter {
        fn reserve(
            &self,
            _: BudgetSlice,
        ) -> Result<gauntlet_ports::budget_meter::Reservation, BudgetMeterError> {
            unreachable!("the scheduler never reserves; it only consults")
        }
        fn charge(
            &self,
            _: &gauntlet_ports::budget_meter::Reservation,
            _: gauntlet_domain::Payment,
        ) -> Result<(), BudgetMeterError> {
            unreachable!("the scheduler never charges")
        }
        fn remaining(&self) -> Remaining {
            gauntlet_ports::budget_meter::remaining_of(&self.budget)
        }
        fn would_breach_reserve(&self, slice: BudgetSlice) -> bool {
            self.budget.would_breach_reserve(
                slice.rounds,
                slice.wall_seconds,
                slice.tokens,
                slice.cost_usd,
            )
        }
    }

    fn plan_input(units: Vec<Unit>) -> PlanInput {
        let concurrency = Concurrency {
            global_capacity: SLOTS,
            global_in_flight: 0,
            tier_capacity: vec![
                (BlastRadius::Contained, SLOTS),
                (BlastRadius::Local, SLOTS),
                (BlastRadius::CrossCutting, SLOTS),
                (BlastRadius::Irreversible, SLOTS),
            ],
            tier_in_flight: Vec::new(),
        };
        let estimates = units.iter().map(|u| (u.id().clone(), slice(50))).collect();
        PlanInput {
            units,
            claimed: Vec::new(),
            concurrency,
            configured_roles: vec![RoleId::Builder],
            cohorts: BTreeMap::new(),
            estimates,
            // No durable projection: these plans are over an unstarted
            // run, where the parsed status IS the status.
            statuses: BTreeMap::new(),
        }
    }

    fn scheduler() -> UnitScheduler {
        UnitScheduler::new()
    }

    #[test]
    fn an_open_irreversible_unit_is_never_eligible_and_never_sliced() {
        // §11.2 conjunct 3 / §17.1: "never an automatic dispatch candidate
        // at the scheduler level" — regardless of any other fact. Every
        // OTHER conjunct is satisfied here on purpose.
        let irreversible = unit(1, BlastRadius::Irreversible, Authority::Human, Vec::new());
        let input = plan_input(vec![irreversible.clone()]);
        let meter = RealMeter {
            budget: meter_budget(),
        };
        let refused = scheduler()
            .eligible(&irreversible, &input, &meter)
            .expect_err("irreversible is never eligible");
        assert!(
            matches!(refused, Ineligible::Irreversible { .. }),
            "{refused}"
        );
        // Even with a cohort partner: the partner plans ALONE; the
        // irreversible member never enters any slice.
        let partner = unit(2, BlastRadius::Contained, Authority::Critic, Vec::new());
        let irreversible = unit(1, BlastRadius::Irreversible, Authority::Human, Vec::new());
        let mut input = plan_input(vec![irreversible, partner]);
        let cohort = CohortId::new("same-invariant").expect("valid cohort");
        input.cohorts.insert(unit_id(1), cohort.clone());
        input.cohorts.insert(unit_id(2), cohort);
        let plan = scheduler().plan(&input, &meter).expect("plans");
        assert_eq!(plan.len(), 1, "only the eligible partner plans");
        assert_eq!(plan[0].units, vec![unit_id(2)], "alone — never mixed");
        assert!(!plan[0].is_slice());
    }

    #[test]
    fn the_durable_projection_decides_status_for_membership_and_dependencies() {
        // Correction C6.2: the plan carries the WHOLE graph, so a settled
        // member is present for conjunct 2 to answer about. Its parsed
        // file status is still `open` (§5.3.2 makes settlement a fact about
        // a PASS VerificationRecord, and no writer puts it back in the
        // answer key), so both conjuncts must decide on the caller's
        // durable projection instead.
        let dependency = unit(901, BlastRadius::Contained, Authority::Critic, Vec::new());
        let dependent = unit(
            1,
            BlastRadius::Contained,
            Authority::Critic,
            vec![unit_id(901)],
        );
        assert_eq!(
            dependency.status(),
            UnitStatus::Open,
            "the PARSED status stays open — the projection is the only \
             place the settlement is visible"
        );
        let mut input = plan_input(vec![dependency.clone(), dependent.clone()]);
        input.statuses.insert(unit_id(901), UnitStatus::Settled);
        let meter = RealMeter {
            budget: meter_budget(),
        };
        // Conjunct 2: the dependency reads settled through the projection,
        // so the dependent is eligible.
        scheduler()
            .eligible(&dependent, &input, &meter)
            .expect("a durably settled dependency releases its dependent");
        // Conjunct 1: the settled member is still in the graph, and is
        // refused with the DURABLE status, not the parsed one.
        let refused = scheduler()
            .eligible(&dependency, &input, &meter)
            .expect_err("a durably settled member is not re-dispatched");
        assert!(
            matches!(
                &refused,
                Ineligible::StatusNotOpen {
                    status: UnitStatus::Settled,
                    unit,
                } if unit == &unit_id(901)
            ),
            "the refusal reports the durable status: {refused}"
        );
        // The plan admits the dependent and only the dependent.
        let planned = scheduler()
            .plan(&input, &meter)
            .expect("the graph is whole");
        assert_eq!(
            planned
                .iter()
                .flat_map(|dispatch| dispatch.units.clone())
                .collect::<Vec<_>>(),
            vec![unit_id(1)],
            "membership is whole; eligibility filtered separately"
        );
    }

    #[test]
    fn unsettled_dependencies_block_and_settled_or_out_of_scope_release() {
        let dep_open = unit(901, BlastRadius::Contained, Authority::Critic, Vec::new());
        let mut dep_settled = unit(902, BlastRadius::Contained, Authority::Critic, Vec::new());
        // Settled requires a live settlement; out_of_scope is reachable by
        // transition and is the §11.2-recognized alternative used here.
        dep_settled
            .transition(UnitStatus::InProgress)
            .expect("moves");
        dep_settled
            .transition(UnitStatus::OutOfScope)
            .expect("moves");
        let dependent = unit(
            1,
            BlastRadius::Contained,
            Authority::Critic,
            vec![unit_id(901)],
        );
        let input = plan_input(vec![dep_open, dependent.clone()]);
        let meter = RealMeter {
            budget: meter_budget(),
        };
        let refused = scheduler()
            .eligible(&dependent, &input, &meter)
            .expect_err("an open dependency blocks");
        assert!(
            matches!(
                &refused,
                Ineligible::DependencyUnresolved {
                    dependency,
                    status: Some(UnitStatus::Open),
                    ..
                } if dependency == &unit_id(901)
            ),
            "{refused}"
        );
        // With the dependency out_of_scope, the dependent is eligible.
        let dependent = unit(
            1,
            BlastRadius::Contained,
            Authority::Critic,
            vec![unit_id(902)],
        );
        let input = plan_input(vec![dep_settled, dependent.clone()]);
        scheduler()
            .eligible(&dependent, &input, &meter)
            .expect("out_of_scope releases the dependency");
    }

    #[test]
    fn status_and_claim_conjuncts_hold() {
        let mut in_progress = unit(1, BlastRadius::Contained, Authority::Critic, Vec::new());
        in_progress
            .transition(UnitStatus::InProgress)
            .expect("moves");
        let input = plan_input(vec![in_progress.clone()]);
        let meter = RealMeter {
            budget: meter_budget(),
        };
        assert!(matches!(
            scheduler().eligible(&in_progress, &input, &meter),
            Err(Ineligible::StatusNotOpen { .. })
        ));
        // Claimed/in-flight: an open unit held by an invocation.
        let claimed = unit(2, BlastRadius::Contained, Authority::Critic, Vec::new());
        let mut input = plan_input(vec![claimed.clone()]);
        input.claimed.push((
            unit_id(2),
            InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAW").expect("valid id"),
        ));
        assert!(matches!(
            scheduler().eligible(&claimed, &input, &meter),
            Err(Ineligible::Claimed { .. })
        ));
    }

    #[test]
    fn global_and_tier_concurrency_are_both_accounted() {
        let meter = RealMeter {
            budget: meter_budget(),
        };
        // Global exhausted.
        let candidate = unit(1, BlastRadius::Contained, Authority::Critic, Vec::new());
        let mut input = plan_input(vec![candidate.clone()]);
        input.concurrency.global_in_flight = 2;
        assert!(matches!(
            scheduler().eligible(&candidate, &input, &meter),
            Err(Ineligible::ConcurrencyGlobal {
                capacity: 2,
                in_flight: 2,
                ..
            })
        ));
        // Tier exhausted (global free).
        let mut input = plan_input(vec![candidate.clone()]);
        input.concurrency.global_in_flight = 0;
        input
            .concurrency
            .tier_in_flight
            .push((BlastRadius::Contained, 2));
        assert!(matches!(
            scheduler().eligible(&candidate, &input, &meter),
            Err(Ineligible::ConcurrencyTier {
                capacity: 2,
                in_flight: 2,
                ..
            })
        ));
        // Tier unconfigured: no entry is no admission (§17.3), not ∞.
        let mut input = plan_input(vec![candidate.clone()]);
        input
            .concurrency
            .tier_capacity
            .retain(|(tier, _)| *tier != BlastRadius::Contained);
        assert!(matches!(
            scheduler().eligible(&candidate, &input, &meter),
            Err(Ineligible::TierConcurrencyUnconfigured { .. })
        ));
        // Both free: eligible.
        let input = plan_input(vec![candidate.clone()]);
        scheduler()
            .eligible(&candidate, &input, &meter)
            .expect("free slots admit");
    }

    #[test]
    fn a_missing_runner_excludes_eligibility() {
        let candidate = unit(1, BlastRadius::Contained, Authority::Critic, Vec::new());
        let mut input = plan_input(vec![candidate.clone()]);
        input.configured_roles.clear();
        let meter = RealMeter {
            budget: meter_budget(),
        };
        assert!(matches!(
            scheduler().eligible(&candidate, &input, &meter),
            Err(Ineligible::RunnerUnconfigured {
                role: RoleId::Builder,
                ..
            })
        ));
    }

    #[test]
    fn meter_refusal_excludes_eligibility() {
        // §17.4: consulted before every dispatch. The domain arithmetic:
        // 1000 tokens at a 20 percent reserve — 800 forward; an 801-token
        // estimate breaches.
        let candidate = unit(1, BlastRadius::Contained, Authority::Critic, Vec::new());
        let mut input = plan_input(vec![candidate.clone()]);
        input.estimates.insert(unit_id(1), slice(801));
        let meter = RealMeter {
            budget: meter_budget(),
        };
        let refused = scheduler()
            .eligible(&candidate, &input, &meter)
            .expect_err("the meter's refusal excludes");
        assert!(
            matches!(&refused, Ineligible::BudgetReserveBreach { slice, .. } if slice.tokens == 801),
            "{refused}"
        );
        // At exactly the ceiling the answer is eligible (landing ON the
        // ceiling leaves the reserve intact — the domain's boundary).
        input.estimates.insert(unit_id(1), slice(800));
        scheduler()
            .eligible(&candidate, &input, &meter)
            .expect("on the ceiling is not a breach");
    }

    #[test]
    fn a_missing_estimate_is_a_typed_input_error_never_a_zero() {
        let candidate = unit(1, BlastRadius::Contained, Authority::Critic, Vec::new());
        let mut input = plan_input(vec![candidate.clone()]);
        input.estimates.remove(&unit_id(1));
        let meter = RealMeter {
            budget: meter_budget(),
        };
        assert!(matches!(
            scheduler().plan(&input, &meter),
            Err(SchedulerError::MissingEstimate { unit }) if unit == unit_id(1)
        ));
    }

    #[test]
    fn ordering_follows_blast_then_depth_then_id_and_is_deterministic() {
        // §11.2: "ascending blast-radius risk (contained first), then
        // dependency depth, then UnitId lexicographic."
        let meter = RealMeter {
            budget: meter_budget(),
        };
        let shallow_local = unit(10, BlastRadius::Local, Authority::Critic, Vec::new());
        let dep = settled_dependency();
        let deep_contained = unit(
            11,
            BlastRadius::Contained,
            Authority::Critic,
            vec![dep.id().clone()],
        );
        let sibling_contained = unit(12, BlastRadius::Contained, Authority::Critic, Vec::new());
        let units = vec![
            sibling_contained.clone(),
            deep_contained.clone(),
            shallow_local.clone(),
            dep.clone(),
        ];
        let input = plan_input(units);
        let plan = scheduler().plan(&input, &meter).expect("plans");
        let ids: Vec<&Vec<UnitId>> = plan.iter().map(|d| &d.units).collect();
        // contained (both) before local; among contained: shallow (depth 0,
        // AK-900 dep out_of_scope... wait the dependency is out_of_scope so
        // deep_contained has depth 1) — depth ascending puts the sibling
        // (depth 0) before the dependent (depth 1).
        assert_eq!(
            ids,
            vec![&vec![unit_id(12)], &vec![unit_id(11)], &vec![unit_id(10)]],
            "contained first, then shallower, then id"
        );
        // Depth ascending = dependencies first: depth(dependent) is
        // strictly greater than depth(dependency).
        assert_eq!(plan[0].depth, 0);
        assert_eq!(plan[1].depth, 1);
        // Determinism: three permutations of the SAME input plan
        // identically (§11.2: "a scheduler whose behavior varies run to run
        // cannot be compared against itself").
        let base = scheduler().plan(&input, &meter).expect("plans");
        for permutation in [
            vec![
                deep_contained.clone(),
                shallow_local.clone(),
                sibling_contained.clone(),
                dep.clone(),
            ],
            vec![
                dep.clone(),
                shallow_local.clone(),
                deep_contained.clone(),
                sibling_contained.clone(),
            ],
        ] {
            let shuffled = plan_input(permutation);
            assert_eq!(
                scheduler().plan(&shuffled, &meter).expect("plans"),
                base,
                "input order must not move the plan"
            );
        }
    }

    #[test]
    fn slices_inherit_the_highest_blast_and_most_restrictive_authority() {
        // §11.3 + the §5.3.2-pinned lattice: authority Human > LeadAgent >
        // Critic; blast irreversible > cross_cutting > local > contained.
        let a = unit(1, BlastRadius::Contained, Authority::Critic, Vec::new());
        let b = unit(2, BlastRadius::Local, Authority::LeadAgent, Vec::new());
        let c = unit(3, BlastRadius::CrossCutting, Authority::Human, Vec::new());
        let mut input = plan_input(vec![a, b.clone(), c]);
        let cohort = CohortId::new("same-invariant").expect("valid cohort");
        for n in [1u16, 2, 3] {
            input.cohorts.insert(unit_id(n), cohort.clone());
        }
        let meter = RealMeter {
            budget: meter_budget(),
        };
        let plan = scheduler().plan(&input, &meter).expect("plans");
        assert_eq!(plan.len(), 1, "one cohort, one slice");
        let slice = &plan[0];
        assert!(slice.is_slice());
        assert_eq!(slice.units, vec![unit_id(1), unit_id(2), unit_id(3)]);
        assert_eq!(
            slice.blast_radius,
            BlastRadius::CrossCutting,
            "highest blast"
        );
        assert_eq!(
            slice.authority,
            Authority::Human,
            "most restrictive authority"
        );
        // The slice's estimate is the SUM of member estimates: one builder
        // does all three Units' work.
        assert_eq!(slice.estimated_slice.tokens, 3 * 50);
        assert_eq!(slice.estimated_slice.rounds, 3);
        // A cohort of one still plans (degenerate slice = single dispatch).
        let mut input = plan_input(vec![b]);
        input.cohorts.insert(unit_id(2), cohort);
        let plan = scheduler().plan(&input, &meter).expect("plans");
        assert_eq!(plan.len(), 1);
        assert!(!plan[0].is_slice());
    }

    #[test]
    fn the_authority_lattice_is_pinned_by_the_coupling_rules() {
        // §5.3.2: "irreversible ⟹ human" and "cross_cutting|irreversible ⟹
        // authority ≠ critic". Pin the PUBLIC lattice the scheduler
        // inherits with: exactly those couplings the domain admits.
        for blast in BlastRadius::ALL {
            for authority in Authority::ALL {
                let built = try_unit(1, *blast, *authority, Vec::new());
                let admits = !matches!(
                    &built,
                    Err(UnitError::IrreversibleRequiresHuman { .. })
                        | Err(UnitError::CriticForbiddenForBlastRadius { .. })
                );
                let lattice_agrees = if *blast == BlastRadius::Irreversible {
                    // Only human — and human is the most restrictive.
                    *authority == Authority::Human && authority_rank(*authority) == 2
                } else if *blast == BlastRadius::CrossCutting {
                    // Not critic; lead or human — both above critic.
                    *authority != Authority::Critic
                        && authority_rank(*authority) > authority_rank(Authority::Critic)
                } else {
                    // Any authority at the contained/local tiers.
                    authority_rank(*authority) <= 2
                };
                assert_eq!(
                    admits, lattice_agrees,
                    "the lattice and the domain coupling disagree at {blast}/{authority}"
                );
            }
        }
        // And the blast ordering is the tier table's order.
        assert!(blast_rank(BlastRadius::Contained) < blast_rank(BlastRadius::Local));
        assert!(blast_rank(BlastRadius::Local) < blast_rank(BlastRadius::CrossCutting));
        assert!(blast_rank(BlastRadius::CrossCutting) < blast_rank(BlastRadius::Irreversible));
    }

    #[test]
    fn a_dangling_depends_on_is_a_typed_load_error() {
        // Task-7 F7: unknown target at plan-input load — a typed error
        // naming it, never a silently never-eligible Unit.
        let orphan = unit(
            1,
            BlastRadius::Contained,
            Authority::Critic,
            vec![unit_id(777)],
        );
        let input = plan_input(vec![orphan]);
        let meter = RealMeter {
            budget: meter_budget(),
        };
        let error = scheduler()
            .plan(&input, &meter)
            .expect_err("dangling depends_on");
        assert!(
            matches!(&error, SchedulerError::UnknownDependency { unit, dependency }
                if unit == &unit_id(1) && dependency == &unit_id(777)),
            "{error}"
        );
        assert!(
            error.to_string().contains("AK-777"),
            "names the target: {error}"
        );
    }

    #[test]
    fn a_depends_on_cycle_is_a_typed_error_naming_the_cycle() {
        // Task-7 F7: A -> B -> C -> A.
        let a = unit(
            1,
            BlastRadius::Contained,
            Authority::Critic,
            vec![unit_id(2)],
        );
        let b = unit(
            2,
            BlastRadius::Contained,
            Authority::Critic,
            vec![unit_id(3)],
        );
        let c = unit(
            3,
            BlastRadius::Contained,
            Authority::Critic,
            vec![unit_id(1)],
        );
        let input = plan_input(vec![a, b, c]);
        let meter = RealMeter {
            budget: meter_budget(),
        };
        let error = scheduler()
            .plan(&input, &meter)
            .expect_err("cyclic depends_on");
        assert!(
            matches!(&error, SchedulerError::DependencyCycle { cycle }
                if cycle.first() == cycle.last() && cycle.len() == 4),
            "the cycle path is named start-to-start: {error}"
        );
        assert!(
            error.to_string().contains("->"),
            "the path is readable: {error}"
        );
    }

    #[test]
    fn duplicate_units_are_refused_and_invalid_cohorts_are_refused() {
        let meter = RealMeter {
            budget: meter_budget(),
        };
        let a = unit(1, BlastRadius::Contained, Authority::Critic, Vec::new());
        let again = unit(1, BlastRadius::Contained, Authority::Critic, Vec::new());
        let input = plan_input(vec![a, again]);
        assert!(matches!(
            scheduler().plan(&input, &meter),
            Err(SchedulerError::DuplicateUnit { .. })
        ));
        assert!(matches!(
            CohortId::new(""),
            Err(SchedulerError::CohortInvalid { .. })
        ));
        assert!(matches!(
            CohortId::new("has space"),
            Err(SchedulerError::CohortInvalid { .. })
        ));
    }
}
