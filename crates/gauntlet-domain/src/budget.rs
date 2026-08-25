//! `Budget` — spec §5.3.7.
//!
//! Spec §5.3.7 invariant, quoted: "the controller MUST refuse to dispatch any
//! invocation that would consume the recovery reserve. The reserve exists so
//! that a run can always afford to checkpoint, roll back, and report."
//!
//! Consequence for this type: a charge that would push a dimension past its
//! forward ceiling — `limit * (100 - reserve_pct) / 100` — is refused with a
//! typed error. A charge that lands exactly ON the ceiling is allowed: the
//! reserve itself stays intact. Budget values are never defaulted here;
//! the §8.6 defaults table is configuration (plan Task 5).

use core::fmt;

/// Money in millionths of a dollar — unsigned, so negative cost is
/// unrepresentable. No `Default`: a zero budget is a configuration
/// statement, not an implicit value.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct MicroUsd {
    micros: u64,
}

impl MicroUsd {
    pub fn new(micros: u64) -> Self {
        Self { micros }
    }

    pub fn get(self) -> u64 {
        self.micros
    }
}

impl fmt::Display for MicroUsd {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} micro-usd", self.micros)
    }
}

/// Which budget dimension an error or query is about.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetDimension {
    Rounds,
    Parallel,
    Depth,
    WallSeconds,
    Tokens,
    CostUsd,
    DiagnosticTokens,
    DiagnosticCostUsd,
}

impl fmt::Display for BudgetDimension {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Rounds => "rounds",
            Self::Parallel => "parallel",
            Self::Depth => "depth",
            Self::WallSeconds => "wall seconds",
            Self::Tokens => "tokens",
            Self::CostUsd => "cost",
            Self::DiagnosticTokens => "diagnostic tokens",
            Self::DiagnosticCostUsd => "diagnostic cost",
        };
        f.write_str(name)
    }
}

/// Budget failures — typed, with the numbers that decided them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BudgetError {
    /// The raw limit is exceeded, independent of any reserve.
    LimitExhausted {
        dimension: BudgetDimension,
        limit: u64,
        spent: u64,
        requested: u64,
    },
    /// The charge would consume the recovery reserve (spec §5.3.7): the
    /// controller refuses it so the run can always afford to checkpoint,
    /// roll back, and report.
    RecoveryReserveBreached {
        dimension: BudgetDimension,
        forward_ceiling: u64,
        spent: u64,
        requested: u64,
    },
    /// The configured reserve percentage is not a usable portion.
    ReserveOutOfRange { pct: u8 },
    /// A restored `Spent` water-mark sits where no legal charge sequence
    /// can leave it — past a raw limit, inside the recovery reserve, or
    /// past the diagnostic allowance (spec §18.2 resume fidelity: durable
    /// state that the charge path cannot produce is corrupt, not resumed).
    RestoreUnreachable {
        dimension: BudgetDimension,
        /// The bound the water-mark crossed: the raw limit, the forward
        /// ceiling, or the allowance, whichever the dimension enforces.
        ceiling: u64,
        spent: u64,
    },
    /// A ceiling of zero was configured — indistinguishable from
    /// unconfigured, and never permission for unlimited recursion.
    LimitZero { dimension: BudgetDimension },
    /// A concurrency or depth limit is exceeded at dispatch time.
    ScopeExceeded { parallel: u32, depth: u32 },
}

impl fmt::Display for BudgetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::LimitExhausted {
                dimension,
                limit,
                spent,
                requested,
            } => write!(
                f,
                "budget {dimension} exhausted: limit {limit}, spent {spent}, requested {requested}"
            ),
            Self::RecoveryReserveBreached {
                dimension,
                forward_ceiling,
                spent,
                requested,
            } => write!(
                f,
                "budget {dimension}: charge of {requested} over spent {spent} would pass the \
                 forward ceiling {forward_ceiling} and consume the recovery reserve — refused so \
                 the run can always checkpoint, roll back, and report (spec 5.3.7)"
            ),
            Self::ReserveOutOfRange { pct } => write!(
                f,
                "recovery reserve of {pct} percent is outside the legal one through one \
                 hundred range: the reserve must be a real portion that is never spendable on \
                 forward work (spec 5.3.7)"
            ),
            Self::RestoreUnreachable {
                dimension,
                ceiling,
                spent,
            } => write!(
                f,
                "restored budget {dimension} spend of {spent} sits past the {ceiling} bound the \
                 charge path enforces — no legal charge sequence can leave this water-mark, so \
                 the durable spend is corrupt and refused rather than resumed (spec 18.2)"
            ),
            Self::LimitZero { dimension } => write!(
                f,
                "budget {dimension} ceiling of zero is not a usable limit — absence of a \
                 configured budget is never permission for unlimited recursion"
            ),
            Self::ScopeExceeded { parallel, depth } => write!(
                f,
                "dispatch at parallel {parallel} and depth {depth} exceeds the configured bounds"
            ),
        }
    }
}

impl std::error::Error for BudgetError {}

/// The ceilings of a run (spec §5.3.7: `max_rounds`, `max_parallel`,
/// `max_depth`, `max_wall_seconds`, `max_tokens`, `max_cost_usd`). "Absence
/// of a configured budget is never permission for unlimited recursion" — a
/// Budget without limits is not constructible; the values come from
/// configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    max_rounds: u32,
    max_parallel: u32,
    max_depth: u32,
    max_wall_seconds: u64,
    max_tokens: u64,
    max_cost_usd: MicroUsd,
}

impl Limits {
    /// Ceiling set for a run. Zero-valued ceilings are refused: a zero limit
    /// is indistinguishable from "unconfigured", and absence of a configured
    /// budget is never permission for unlimited recursion.
    pub fn new(
        max_rounds: u32,
        max_parallel: u32,
        max_depth: u32,
        max_wall_seconds: u64,
        max_tokens: u64,
        max_cost_usd: MicroUsd,
    ) -> Result<Self, BudgetError> {
        for (dimension, zeroed) in [
            (BudgetDimension::Rounds, max_rounds == 0),
            (BudgetDimension::Parallel, max_parallel == 0),
            (BudgetDimension::Depth, max_depth == 0),
            (BudgetDimension::WallSeconds, max_wall_seconds == 0),
            (BudgetDimension::Tokens, max_tokens == 0),
            (BudgetDimension::CostUsd, max_cost_usd.get() == 0),
        ] {
            if zeroed {
                return Err(BudgetError::LimitZero { dimension });
            }
        }
        Ok(Self {
            max_rounds,
            max_parallel,
            max_depth,
            max_wall_seconds,
            max_tokens,
            max_cost_usd,
        })
    }

    pub fn max_rounds(&self) -> u32 {
        self.max_rounds
    }

    pub fn max_parallel(&self) -> u32 {
        self.max_parallel
    }

    pub fn max_depth(&self) -> u32 {
        self.max_depth
    }

    pub fn max_wall_seconds(&self) -> u64 {
        self.max_wall_seconds
    }

    pub fn max_tokens(&self) -> u64 {
        self.max_tokens
    }

    pub fn max_cost_usd(&self) -> MicroUsd {
        self.max_cost_usd
    }
}

/// The separate §15.2 diagnosis allowance (spec §5.3.7
/// `diagnostic_budget`). Spending it does not touch the forward budget or
/// the recovery reserve — a run can always afford to diagnose.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticAllowance {
    tokens: u64,
    cost_usd: MicroUsd,
}

impl DiagnosticAllowance {
    pub fn new(tokens: u64, cost_usd: MicroUsd) -> Self {
        Self { tokens, cost_usd }
    }

    pub fn tokens(&self) -> u64 {
        self.tokens
    }

    pub fn cost_usd(&self) -> MicroUsd {
        self.cost_usd
    }
}

/// Mirrored spend counters (spec §5.3.7 `spent: {...}`), diagnostic spend
/// tracked beside forward spend.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spent {
    rounds: u32,
    wall_seconds: u64,
    tokens: u64,
    cost_usd: MicroUsd,
    diagnostic_tokens: u64,
    diagnostic_cost_usd: MicroUsd,
}

impl Spent {
    fn fresh() -> Self {
        Self {
            rounds: 0,
            wall_seconds: 0,
            tokens: 0,
            cost_usd: MicroUsd::new(0),
            diagnostic_tokens: 0,
            diagnostic_cost_usd: MicroUsd::new(0),
        }
    }

    /// The value-carrying constructor — the §18.2 resume-fidelity path
    /// (Task 12, closing Task-6 critic-A MAJOR-2: without it, non-zero
    /// durable `budget_spent` could not resume, because spend accrued only
    /// through [`Budget::charge`]). It performs NO invariant checking by
    /// design: a `Spent` is inert counters, and admission under ceilings is
    /// [`Budget::restore`]'s refusal to make — the same way `charge` is the
    /// only path that ACCRUES spend, `restore` is the only path that
    /// re-admits it, and both enforce the same watermark.
    pub fn from_counters(
        rounds: u32,
        wall_seconds: u64,
        tokens: u64,
        cost_usd: MicroUsd,
        diagnostic_tokens: u64,
        diagnostic_cost_usd: MicroUsd,
    ) -> Self {
        Self {
            rounds,
            wall_seconds,
            tokens,
            cost_usd,
            diagnostic_tokens,
            diagnostic_cost_usd,
        }
    }

    pub fn rounds(&self) -> u32 {
        self.rounds
    }

    pub fn wall_seconds(&self) -> u64 {
        self.wall_seconds
    }

    pub fn tokens(&self) -> u64 {
        self.tokens
    }

    pub fn cost_usd(&self) -> MicroUsd {
        self.cost_usd
    }

    pub fn diagnostic_tokens(&self) -> u64 {
        self.diagnostic_tokens
    }

    pub fn diagnostic_cost_usd(&self) -> MicroUsd {
        self.diagnostic_cost_usd
    }
}

/// The run's budget (spec §5.3.7): ceilings, the recovery reserve, the
/// diagnostic allowance, and the mirrored spend counters. The controller
/// consults [`Budget::would_breach_reserve`] before every dispatch and
/// [`Budget::charge`] refuses any charge that would consume the reserve.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Budget {
    limits: Limits,
    recovery_reserve_pct: u8,
    diagnostic: DiagnosticAllowance,
    spent: Spent,
}

impl Budget {
    /// A budget with unspent counters. The reserve must be a real portion
    /// (1..=100 percent, never spendable on forward work).
    pub fn new(
        limits: Limits,
        recovery_reserve_pct: u8,
        diagnostic: DiagnosticAllowance,
    ) -> Result<Self, BudgetError> {
        if recovery_reserve_pct == 0 || recovery_reserve_pct > 100 {
            return Err(BudgetError::ReserveOutOfRange {
                pct: recovery_reserve_pct,
            });
        }
        Ok(Self {
            limits,
            recovery_reserve_pct,
            diagnostic,
            spent: Spent::fresh(),
        })
    }

    /// Rebuilds a budget from durable spend counters (spec §18.2 resume
    /// fidelity: "Every iteration boundary writes a durable checkpoint
    /// containing … budget spent", and a checkpoint is only worth writing
    /// if it is worth resuming). The reserve-portion rule is the
    /// constructor's own; on top of it, restore refuses any `spent`
    /// water-mark the charge path could not have left:
    ///
    /// - every forward component at or under its raw limit AND at or under
    ///   the forward ceiling `charge` enforces (a water-mark inside the
    ///   recovery reserve means the reserve was already consumed —
    ///   unreachable via `charge`, corrupt to resume);
    /// - diagnostic spend at or under the diagnostic allowance.
    ///
    /// Landing exactly ON the forward ceiling restores: the boundary charge
    /// books and leaves the reserve intact.
    pub fn restore(
        limits: Limits,
        recovery_reserve_pct: u8,
        diagnostic: DiagnosticAllowance,
        spent: Spent,
    ) -> Result<Self, BudgetError> {
        if recovery_reserve_pct == 0 || recovery_reserve_pct > 100 {
            return Err(BudgetError::ReserveOutOfRange {
                pct: recovery_reserve_pct,
            });
        }
        let forward = [
            (
                BudgetDimension::Rounds,
                u64::from(limits.max_rounds),
                u64::from(spent.rounds),
            ),
            (
                BudgetDimension::WallSeconds,
                limits.max_wall_seconds,
                spent.wall_seconds,
            ),
            (BudgetDimension::Tokens, limits.max_tokens, spent.tokens),
            (
                BudgetDimension::CostUsd,
                limits.max_cost_usd.get(),
                spent.cost_usd.get(),
            ),
        ];
        for (dimension, limit, booked) in forward {
            let ceiling = limit * u64::from(100 - recovery_reserve_pct) / 100;
            if booked > ceiling {
                return Err(BudgetError::RestoreUnreachable {
                    dimension,
                    // Name the binding bound: the raw limit when the
                    // water-mark is past even that, the forward ceiling
                    // when only the reserve rule refuses it.
                    ceiling: if booked > limit { limit } else { ceiling },
                    spent: booked,
                });
            }
        }
        let diagnostic_bounds = [
            (
                BudgetDimension::DiagnosticTokens,
                diagnostic.tokens(),
                spent.diagnostic_tokens,
            ),
            (
                BudgetDimension::DiagnosticCostUsd,
                diagnostic.cost_usd().get(),
                spent.diagnostic_cost_usd.get(),
            ),
        ];
        for (dimension, allowance, booked) in diagnostic_bounds {
            if booked > allowance {
                return Err(BudgetError::RestoreUnreachable {
                    dimension,
                    ceiling: allowance,
                    spent: booked,
                });
            }
        }
        Ok(Self {
            limits,
            recovery_reserve_pct,
            diagnostic,
            spent,
        })
    }

    pub fn limits(&self) -> &Limits {
        &self.limits
    }

    pub fn recovery_reserve_pct(&self) -> u8 {
        self.recovery_reserve_pct
    }

    pub fn diagnostic_allowance(&self) -> &DiagnosticAllowance {
        &self.diagnostic
    }

    pub fn spent(&self) -> &Spent {
        &self.spent
    }

    /// The forward ceiling for one dimension: `limit * (100 - pct) / 100`.
    /// The remainder is the recovery reserve. Public (N5, Task-12
    /// correction): the budget-meter port's `remaining()` projection reads
    /// the DOMAIN's own ceiling arithmetic rather than duplicating the
    /// formula — `remaining()` must report what forward work may still
    /// spend (§17.4: the reserve is never spendable on forward work), and
    /// the single source of that number is here.
    pub fn forward_ceiling(&self, limit: u64) -> u64 {
        limit * u64::from(100 - self.recovery_reserve_pct) / 100
    }

    /// Whether a forward charge would consume the recovery reserve (spec
    /// §5.3.7). A charge landing exactly ON the ceiling does not: the
    /// reserve itself stays intact.
    pub fn would_breach_reserve(
        &self,
        rounds: u32,
        wall_seconds: u64,
        tokens: u64,
        cost_usd: MicroUsd,
    ) -> bool {
        [
            (
                u64::from(self.limits.max_rounds),
                u64::from(self.spent.rounds),
                u64::from(rounds),
            ),
            (
                self.limits.max_wall_seconds,
                self.spent.wall_seconds,
                wall_seconds,
            ),
            (self.limits.max_tokens, self.spent.tokens, tokens),
            (
                self.limits.max_cost_usd.get(),
                self.spent.cost_usd.get(),
                cost_usd.get(),
            ),
        ]
        .into_iter()
        .any(|(limit, spent, requested)| {
            let projected = spent.saturating_add(requested);
            projected > limit || projected > self.forward_ceiling(limit)
        })
    }

    /// Charges forward work. Refuses — [`BudgetError::RecoveryReserveBreached`]
    /// — any charge that would consume the recovery reserve, and —
    /// [`BudgetError::LimitExhausted`] — any charge past a raw limit. This is
    /// the §5.3.7 invariant at the type level: the refusal is the only
    /// outcome, there is no override flag.
    pub fn charge(
        &mut self,
        rounds: u32,
        wall_seconds: u64,
        tokens: u64,
        cost_usd: MicroUsd,
    ) -> Result<(), BudgetError> {
        let checks = [
            (
                BudgetDimension::Rounds,
                u64::from(self.limits.max_rounds),
                u64::from(self.spent.rounds),
                u64::from(rounds),
            ),
            (
                BudgetDimension::WallSeconds,
                self.limits.max_wall_seconds,
                self.spent.wall_seconds,
                wall_seconds,
            ),
            (
                BudgetDimension::Tokens,
                self.limits.max_tokens,
                self.spent.tokens,
                tokens,
            ),
            (
                BudgetDimension::CostUsd,
                self.limits.max_cost_usd.get(),
                self.spent.cost_usd.get(),
                cost_usd.get(),
            ),
        ];
        for (dimension, limit, spent, requested) in checks {
            let projected = spent.saturating_add(requested);
            if projected > limit {
                return Err(BudgetError::LimitExhausted {
                    dimension,
                    limit,
                    spent,
                    requested,
                });
            }
            let ceiling = self.forward_ceiling(limit);
            if projected > ceiling {
                return Err(BudgetError::RecoveryReserveBreached {
                    dimension,
                    forward_ceiling: ceiling,
                    spent,
                    requested,
                });
            }
        }
        self.spent.rounds += rounds;
        self.spent.wall_seconds += wall_seconds;
        self.spent.tokens += tokens;
        self.spent.cost_usd = MicroUsd::new(self.spent.cost_usd.get() + cost_usd.get());
        Ok(())
    }

    /// Charges the separate §15.2 diagnostic allowance. Capped by the
    /// allowance; never touches forward spend or the reserve.
    pub fn charge_diagnostic(
        &mut self,
        tokens: u64,
        cost_usd: MicroUsd,
    ) -> Result<(), BudgetError> {
        let projected_tokens = self.spent.diagnostic_tokens.saturating_add(tokens);
        if projected_tokens > self.diagnostic.tokens() {
            return Err(BudgetError::LimitExhausted {
                dimension: BudgetDimension::DiagnosticTokens,
                limit: self.diagnostic.tokens(),
                spent: self.spent.diagnostic_tokens,
                requested: tokens,
            });
        }
        let projected_cost = self
            .spent
            .diagnostic_cost_usd
            .get()
            .saturating_add(cost_usd.get());
        if projected_cost > self.diagnostic.cost_usd().get() {
            return Err(BudgetError::LimitExhausted {
                dimension: BudgetDimension::DiagnosticCostUsd,
                limit: self.diagnostic.cost_usd().get(),
                spent: self.spent.diagnostic_cost_usd.get(),
                requested: cost_usd.get(),
            });
        }
        self.spent.diagnostic_tokens = projected_tokens;
        self.spent.diagnostic_cost_usd = MicroUsd::new(projected_cost);
        Ok(())
    }

    /// Instantaneous dispatch-scope check (parallelism and depth are not
    /// cumulative counters).
    pub fn check_dispatch_scope(&self, parallel: u32, depth: u32) -> Result<(), BudgetError> {
        if parallel > self.limits.max_parallel || depth > self.limits.max_depth {
            return Err(BudgetError::ScopeExceeded { parallel, depth });
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn limits() -> Limits {
        // Ten rounds, two wide, two deep; generous walls and tokens so the
        // reserve arithmetic under study is the tokens/cost dimensions.
        Limits::new(10, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("usable ceilings")
    }

    fn budget(pct: u8) -> Budget {
        Budget::new(
            limits(),
            pct,
            DiagnosticAllowance::new(100, MicroUsd::new(50)),
        )
        .expect("usable budget")
    }

    #[test]
    fn the_recovery_reserve_boundary_is_exact() {
        // Spec §5.3.7: "the controller MUST refuse to dispatch any
        // invocation that would consume the recovery reserve." 20 percent of
        // 1000 tokens is a 200-token reserve; 800 is the forward ceiling.
        let budget = budget(20);
        assert!(!budget.would_breach_reserve(0, 0, 800, MicroUsd::new(0)));
        // One token past the boundary consumes the reserve.
        assert!(budget.would_breach_reserve(0, 0, 801, MicroUsd::new(0)));
        // Landing exactly ON the ceiling is allowed and books cleanly.
        let mut budget = budget;
        budget
            .charge(0, 0, 800, MicroUsd::new(0))
            .expect("charge to exactly the ceiling");
        assert_eq!(budget.spent().tokens(), 800);
    }

    #[test]
    fn a_charge_that_consumes_the_reserve_is_refused() {
        let mut budget = budget(20);
        let refused = budget.charge(0, 0, 801, MicroUsd::new(0));
        assert!(matches!(
            refused,
            Err(BudgetError::RecoveryReserveBreached {
                forward_ceiling: 800,
                ..
            })
        ));
        // The refused charge booked nothing.
        assert_eq!(budget.spent().tokens(), 0);
        // After filling forward work to the ceiling, even one more token is
        // a reserve consumption and is refused.
        budget
            .charge(0, 0, 800, MicroUsd::new(0))
            .expect("charge to the ceiling");
        assert!(matches!(
            budget.charge(0, 0, 1, MicroUsd::new(0)),
            Err(BudgetError::RecoveryReserveBreached { .. })
        ));
        assert_eq!(budget.spent().tokens(), 800, "the reserve is still intact");
    }

    #[test]
    fn exhaustion_past_the_raw_limit_is_its_own_error() {
        let mut budget = budget(20);
        assert!(matches!(
            budget.charge(0, 0, 1500, MicroUsd::new(0)),
            Err(BudgetError::LimitExhausted {
                dimension: BudgetDimension::Tokens,
                limit: 1000,
                ..
            })
        ));
        // The rounds dimension has its own ceiling: 20 percent of 10 is a
        // two-round reserve.
        assert!(matches!(
            budget.charge(9, 0, 0, MicroUsd::new(0)),
            Err(BudgetError::RecoveryReserveBreached { .. })
        ));
        budget
            .charge(8, 0, 0, MicroUsd::new(0))
            .expect("rounds to the ceiling");
    }

    #[test]
    fn the_reserve_must_be_a_real_portion() {
        assert!(matches!(
            Budget::new(
                limits(),
                0,
                DiagnosticAllowance::new(100, MicroUsd::new(50))
            ),
            Err(BudgetError::ReserveOutOfRange { pct: 0 })
        ));
        assert!(matches!(
            Budget::new(
                limits(),
                101,
                DiagnosticAllowance::new(100, MicroUsd::new(50))
            ),
            Err(BudgetError::ReserveOutOfRange { pct: 101 })
        ));
        // Both extremes of the legal range construct.
        let _ = budget(1);
        let _ = budget(100);
    }

    #[test]
    fn zero_ceilings_are_not_limits() {
        // "Absence of a configured budget is never permission for unlimited
        // recursion": a zero ceiling is refused, not treated as infinite.
        for zeroed in [
            Limits::new(0, 2, 2, 3600, 1000, MicroUsd::new(500)),
            Limits::new(10, 0, 2, 3600, 1000, MicroUsd::new(500)),
            Limits::new(10, 2, 0, 3600, 1000, MicroUsd::new(500)),
            Limits::new(10, 2, 2, 0, 1000, MicroUsd::new(500)),
            Limits::new(10, 2, 2, 3600, 0, MicroUsd::new(500)),
            Limits::new(10, 2, 2, 3600, 1000, MicroUsd::new(0)),
        ] {
            assert!(matches!(zeroed, Err(BudgetError::LimitZero { .. })));
        }
    }

    #[test]
    fn diagnostic_spend_never_touches_forward_work_or_the_reserve() {
        let mut budget = budget(20);
        budget
            .charge_diagnostic(100, MicroUsd::new(50))
            .expect("allowance covers the diagnosis");
        assert_eq!(budget.spent().diagnostic_tokens(), 100);
        assert_eq!(budget.spent().tokens(), 0, "forward spend untouched");
        // The diagnostic allowance caps separately.
        assert!(matches!(
            budget.charge_diagnostic(1, MicroUsd::new(0)),
            Err(BudgetError::LimitExhausted {
                dimension: BudgetDimension::DiagnosticTokens,
                ..
            })
        ));
        // A reserve already consumed forward is still available for
        // diagnosis: the run can always afford to diagnose.
        assert!(matches!(
            budget.charge(0, 0, 801, MicroUsd::new(0)),
            Err(BudgetError::RecoveryReserveBreached { .. })
        ));
    }

    #[test]
    fn dispatch_scope_checks_parallelism_and_depth_instantaneously() {
        let budget = budget(20);
        budget.check_dispatch_scope(2, 2).expect("at the bounds");
        assert!(matches!(
            budget.check_dispatch_scope(3, 1),
            Err(BudgetError::ScopeExceeded { .. })
        ));
        assert!(matches!(
            budget.check_dispatch_scope(1, 3),
            Err(BudgetError::ScopeExceeded { .. })
        ));
    }

    #[test]
    fn charges_book_every_dimension() {
        let mut budget = budget(20);
        budget
            .charge(1, 60, 100, MicroUsd::new(10))
            .expect("within every ceiling");
        assert_eq!(budget.spent().rounds(), 1);
        let elapsed = 60;
        assert_eq!(budget.spent().wall_seconds(), elapsed);
        assert_eq!(budget.spent().tokens(), 100);
        assert_eq!(budget.spent().cost_usd(), MicroUsd::new(10));
        // Ceilings are cumulative across charges.
        assert!(matches!(
            budget.charge(8, 0, 0, MicroUsd::new(0)),
            Err(BudgetError::RecoveryReserveBreached { .. })
        ));
    }

    // -----------------------------------------------------------------------
    // restore — the §18.2 resume-fidelity reconstruction path (Task 12,
    // closing Task-6 critic-A MAJOR-2: non-zero `budget_spent` could not
    // resume because `Spent` had no value-carrying constructor)
    // -----------------------------------------------------------------------

    #[test]
    fn a_zero_spent_restores_to_a_fresh_budget() {
        let restored = Budget::restore(
            limits(),
            20,
            DiagnosticAllowance::new(100, MicroUsd::new(50)),
            Spent::from_counters(0, 0, 0, MicroUsd::new(0), 0, MicroUsd::new(0)),
        )
        .expect("zero spend restores");
        let fresh = budget(20);
        assert_eq!(restored.spent(), fresh.spent());
        assert_eq!(restored.limits(), fresh.limits());
        assert_eq!(
            restored.recovery_reserve_pct(),
            fresh.recovery_reserve_pct()
        );
    }

    #[test]
    fn restore_refuses_spend_no_charge_sequence_can_produce() {
        // Past the RAW limit (1000 tokens): unreachable via charge, which
        // refuses past the limit outright.
        assert!(matches!(
            Budget::restore(
                limits(),
                20,
                DiagnosticAllowance::new(100, MicroUsd::new(50)),
                Spent::from_counters(0, 0, 1001, MicroUsd::new(0), 0, MicroUsd::new(0)),
            ),
            Err(BudgetError::RestoreUnreachable {
                dimension: BudgetDimension::Tokens,
                ..
            })
        ));
        // Inside the raw limit but INSIDE the reserve (forward ceiling 800
        // at a 20 percent reserve): a spent water-mark no legal charge can
        // leave, because charge refuses to cross the ceiling. Restoring it
        // would resume a run whose reserve is already spent.
        assert!(matches!(
            Budget::restore(
                limits(),
                20,
                DiagnosticAllowance::new(100, MicroUsd::new(50)),
                Spent::from_counters(0, 0, 801, MicroUsd::new(0), 0, MicroUsd::new(0)),
            ),
            Err(BudgetError::RestoreUnreachable {
                dimension: BudgetDimension::Tokens,
                ..
            })
        ));
        // Every forward dimension enforces the same refusal, each with its
        // own dimension tag: rounds (ceiling 8 of 10), wall seconds (2880
        // of 3600), cost (400 of 500 micro-usd).
        for (dimension, spent) in [
            (
                BudgetDimension::Rounds,
                Spent::from_counters(9, 0, 0, MicroUsd::new(0), 0, MicroUsd::new(0)),
            ),
            (
                BudgetDimension::WallSeconds,
                Spent::from_counters(0, 2881, 0, MicroUsd::new(0), 0, MicroUsd::new(0)),
            ),
            (
                BudgetDimension::CostUsd,
                Spent::from_counters(0, 0, 0, MicroUsd::new(401), 0, MicroUsd::new(0)),
            ),
        ] {
            assert!(
                matches!(
                    Budget::restore(
                        limits(),
                        20,
                        DiagnosticAllowance::new(100, MicroUsd::new(50)),
                        spent,
                    ),
                    Err(BudgetError::RestoreUnreachable { dimension: found, .. })
                        if found == dimension
                ),
                "restore accepted a spent water-mark inside the {dimension} reserve"
            );
        }
        // Diagnostic spend past its own allowance is equally unreachable.
        assert!(matches!(
            Budget::restore(
                limits(),
                20,
                DiagnosticAllowance::new(100, MicroUsd::new(50)),
                Spent::from_counters(0, 0, 0, MicroUsd::new(0), 101, MicroUsd::new(0)),
            ),
            Err(BudgetError::RestoreUnreachable {
                dimension: BudgetDimension::DiagnosticTokens,
                ..
            })
        ));
        assert!(matches!(
            Budget::restore(
                limits(),
                20,
                DiagnosticAllowance::new(100, MicroUsd::new(50)),
                Spent::from_counters(0, 0, 0, MicroUsd::new(0), 0, MicroUsd::new(51)),
            ),
            Err(BudgetError::RestoreUnreachable {
                dimension: BudgetDimension::DiagnosticCostUsd,
                ..
            })
        ));
        // Landing exactly ON every forward ceiling is reachable (the
        // boundary charge books) and restores.
        assert!(
            Budget::restore(
                limits(),
                20,
                DiagnosticAllowance::new(100, MicroUsd::new(50)),
                Spent::from_counters(8, 2880, 800, MicroUsd::new(400), 100, MicroUsd::new(50)),
            )
            .is_ok()
        );
    }

    #[test]
    fn restore_reuses_the_new_reserve_range_rule() {
        // The reserve portion rule is the constructor's own; restore does
        // not get a second, laxer one.
        assert!(matches!(
            Budget::restore(
                limits(),
                0,
                DiagnosticAllowance::new(100, MicroUsd::new(50)),
                Spent::from_counters(0, 0, 0, MicroUsd::new(0), 0, MicroUsd::new(0)),
            ),
            Err(BudgetError::ReserveOutOfRange { pct: 0 })
        ));
    }

    #[test]
    fn restore_agrees_with_replaying_the_charges() {
        // Behavioral equivalence: for a sequence of legal charges, the
        // budget restored from the spent water-mark those charges left
        // agrees with the budget built by replaying them — same counters,
        // same reserve arithmetic, same admission for later charges.
        let charges = [
            (1u32, 60u64, 100u64, MicroUsd::new(10)),
            (0, 120, 250, MicroUsd::new(15)),
            (2, 30, 40, MicroUsd::new(5)),
        ];
        let mut replayed = budget(20);
        for (rounds, wall, tokens, cost) in charges {
            replayed
                .charge(rounds, wall, tokens, cost)
                .expect("the sequence is legal");
        }
        let restored = Budget::restore(
            limits(),
            20,
            DiagnosticAllowance::new(100, MicroUsd::new(50)),
            *replayed.spent(),
        )
        .expect("a water-mark left by legal charges restores");
        assert_eq!(restored.spent(), replayed.spent());

        // Reserve arithmetic agrees across the follow-up probes, including
        // the boundary values: the last legal charge, one past it.
        for (rounds, wall, tokens, cost) in [
            (5u32, 2670u64, 410u64, MicroUsd::new(380)),
            (6, 2671, 411, MicroUsd::new(381)),
            (0, 0, 1, MicroUsd::new(0)),
        ] {
            assert_eq!(
                restored.would_breach_reserve(rounds, wall, tokens, cost),
                replayed.would_breach_reserve(rounds, wall, tokens, cost),
                "the restored and replayed budgets disagree on a follow-up slice"
            );
        }
        // Subsequent charge admission agrees — including refusals.
        let mut restored_admission = restored;
        let mut replayed_admission = replayed;
        assert_eq!(
            restored_admission
                .charge(5, 2670, 410, MicroUsd::new(380))
                .is_ok(),
            replayed_admission
                .charge(5, 2670, 410, MicroUsd::new(380))
                .is_ok()
        );
        assert_eq!(
            restored_admission.charge(0, 0, 1, MicroUsd::new(0)),
            replayed_admission.charge(0, 0, 1, MicroUsd::new(0))
        );
        assert_eq!(
            restored_admission.spent(),
            replayed_admission.spent(),
            "agreed admissions must book identically"
        );
        // Diagnostic accounting rides along unchanged.
        assert!(
            restored_admission
                .charge_diagnostic(100, MicroUsd::new(50))
                .is_ok()
        );
        assert!(
            restored_admission
                .charge_diagnostic(1, MicroUsd::new(0))
                .is_err()
        );
    }
}
