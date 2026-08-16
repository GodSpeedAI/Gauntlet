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
    /// The remainder is the recovery reserve.
    fn forward_ceiling(&self, limit: u64) -> u64 {
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
}
