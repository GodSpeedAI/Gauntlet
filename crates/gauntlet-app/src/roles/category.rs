//! Context categories — the closed vocabulary a role file's
//! `context.allow` / `context.deny` may name (spec §10.1, §13.2, §10.2).
//!
//! Spec §13.2 fixes what the comparison envelope may carry ("The runtime
//! constructs the envelope from an allowlist") and what it may never
//! carry ("It does **not** receive builder conversation history, builder
//! reasoning, implementation rationalizations, attempt counts, builder
//! identity, prior verdicts on this unit, or operator chat"). Spec §10.1
//! spells the same split in role-file grammar: `context.allow` and
//! `context.deny`. This module is that vocabulary as data, in three
//! classes:
//!
//! 1. **Envelope-permitted** — the five §13.2 allowlist lines, one per
//!    `EnvelopeInputs` member the application envelope builder accepts.
//!    A role file may allow these.
//! 2. **Runtime-provided** — the §10.2 role tables' own reading words
//!    (`claim + evidence + instrument output`, `target system state`,
//!    `run history, metrics`). Roles whose context flows outside the
//!    comparison envelope name these. They cannot over-grant the
//!    envelope: the envelope type has no member they could ride in (the
//!    ports envelope module documents that property as structural).
//! 3. **Envelope-denied** — the §13.2 not-received list in §10.1's
//!    spellings. A role file naming one in `context.allow` is an
//!    over-grant attempt and is a typed construction error, never a
//!    silent narrowing.
//!
//! The intersection invariant (plan Task 17): a role file may NARROW
//! what the envelope builder permits, never GRANT beyond it. A deny
//! naming a category the role's own allow names is a contradictory
//! contract — the frozen task preregistration's structural-transfer
//! test lists "a deny that shadows allow" among the shapes the typed
//! construction rejects — so it is refused, not silently resolved.
//!
//! One canonical spelling per category (`AGENTS.md` naming): the
//! [`ContextCategory::parse`] token set is closed, and an unknown token
//! is a typed error — a typo in a deny entry must never silently stop
//! denying.

use core::fmt;

/// One context category a role file's `context` block may name.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ContextCategory {
    // --- envelope-permitted (§13.2 allowlist; `EnvelopeInputs` members) ---
    Reference,
    ObservationBundle,
    ComparisonContract,
    ApplicableConstraints,
    OpenResiduals,
    // --- runtime-provided (§10.2 role tables; outside the envelope) ---
    Claim,
    Evidence,
    InstrumentOutput,
    TargetState,
    RunHistory,
    Metrics,
    // --- envelope-denied (§13.2 not-received list; §10.1 spellings) ---
    BuilderTrajectory,
    BuilderReasoning,
    ImplementationRationale,
    AttemptCount,
    BuilderIdentity,
    PriorVerdicts,
    OperatorChat,
}

/// Which of the three classes a category belongs to. The class decides
/// where the category may appear: an `allow` may name permitted and
/// runtime-provided categories; a `deny` may name any of them.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CategoryClass {
    /// May ride in the §13.2 comparison envelope.
    EnvelopePermitted,
    /// Flows to the role outside the envelope; structurally absent from
    /// the envelope type.
    RuntimeProvided,
    /// The §13.2 not-received list; refused in any `allow`.
    EnvelopeDenied,
}

impl ContextCategory {
    /// Exactly the §13.2 allowlist categories, the ones the envelope
    /// builder can construct an envelope from. This is the permit set
    /// every role file's `context.allow` intersects against.
    pub const ENVELOPE_PERMITTED: &'static [ContextCategory] = &[
        ContextCategory::Reference,
        ContextCategory::ObservationBundle,
        ContextCategory::ComparisonContract,
        ContextCategory::ApplicableConstraints,
        ContextCategory::OpenResiduals,
    ];

    /// The category's canonical role-file token (one spelling per
    /// concept).
    pub fn token(self) -> &'static str {
        match self {
            Self::Reference => "reference",
            Self::ObservationBundle => "observation_bundle",
            Self::ComparisonContract => "comparison_contract",
            Self::ApplicableConstraints => "applicable_constraints",
            Self::OpenResiduals => "open_residuals",
            Self::Claim => "claim",
            Self::Evidence => "evidence",
            Self::InstrumentOutput => "instrument_output",
            Self::TargetState => "target_state",
            Self::RunHistory => "run_history",
            Self::Metrics => "metrics",
            Self::BuilderTrajectory => "builder_trajectory",
            Self::BuilderReasoning => "builder_reasoning",
            Self::ImplementationRationale => "implementation_rationale",
            Self::AttemptCount => "attempt_count",
            Self::BuilderIdentity => "builder_identity",
            Self::PriorVerdicts => "prior_verdicts",
            Self::OperatorChat => "operator_chat",
        }
    }

    /// Parses one role-file token. The vocabulary is closed: an unknown
    /// token is a typed error, so a typo can never silently weaken a
    /// deny (or inflate an allow).
    pub fn parse(token: &str) -> Option<Self> {
        const TOKENS: &[(&str, ContextCategory)] = &[
            ("reference", ContextCategory::Reference),
            ("observation_bundle", ContextCategory::ObservationBundle),
            ("comparison_contract", ContextCategory::ComparisonContract),
            (
                "applicable_constraints",
                ContextCategory::ApplicableConstraints,
            ),
            ("open_residuals", ContextCategory::OpenResiduals),
            ("claim", ContextCategory::Claim),
            ("evidence", ContextCategory::Evidence),
            ("instrument_output", ContextCategory::InstrumentOutput),
            ("target_state", ContextCategory::TargetState),
            ("run_history", ContextCategory::RunHistory),
            ("metrics", ContextCategory::Metrics),
            ("builder_trajectory", ContextCategory::BuilderTrajectory),
            ("builder_reasoning", ContextCategory::BuilderReasoning),
            (
                "implementation_rationale",
                ContextCategory::ImplementationRationale,
            ),
            ("attempt_count", ContextCategory::AttemptCount),
            ("builder_identity", ContextCategory::BuilderIdentity),
            ("prior_verdicts", ContextCategory::PriorVerdicts),
            ("operator_chat", ContextCategory::OperatorChat),
        ];
        TOKENS
            .iter()
            .find(|(known, _)| *known == token)
            .map(|(_, category)| *category)
    }

    /// Which of the three classes the category belongs to.
    pub fn class(self) -> CategoryClass {
        if Self::ENVELOPE_PERMITTED.contains(&self) {
            return CategoryClass::EnvelopePermitted;
        }
        match self {
            Self::Claim
            | Self::Evidence
            | Self::InstrumentOutput
            | Self::TargetState
            | Self::RunHistory
            | Self::Metrics => CategoryClass::RuntimeProvided,
            _ => CategoryClass::EnvelopeDenied,
        }
    }

    /// Whether the §13.2 envelope may carry this category at all — the
    /// question behind the over-grant refusal.
    pub fn envelope_permitted(self) -> bool {
        self.class() == CategoryClass::EnvelopePermitted
    }
}

impl fmt::Display for ContextCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.token())
    }
}

/// The typed deny-set a role file's `context.deny` compiles into: the
/// data the envelope builder consumes when it assembles a §13.2 envelope
/// for this role's invocation (plan Task 17's consume-side hook; the
/// composed dispatch wiring is plan Task 20).
///
/// The set is sorted and de-duplicated at construction, so equality is
/// content equality and iteration is deterministic. It is data, not
/// instructions to a model (spec §10.1: "a structural allowlist, not an
/// instruction to the model").
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContextDenySet {
    categories: Vec<ContextCategory>,
}

impl ContextDenySet {
    /// Builds the set from parsed categories: sorted, de-duplicated.
    pub fn new(mut categories: Vec<ContextCategory>) -> Self {
        categories.sort();
        categories.dedup();
        Self { categories }
    }

    /// The denied categories, deterministic order.
    pub fn categories(&self) -> &[ContextCategory] {
        &self.categories
    }

    /// Whether the set denies a category — the consumption question the
    /// envelope builder asks before including context.
    pub fn denies(&self, category: ContextCategory) -> bool {
        self.categories.contains(&category)
    }
}
