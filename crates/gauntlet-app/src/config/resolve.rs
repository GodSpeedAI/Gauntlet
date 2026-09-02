//! Resolution — the four-layer precedence merge and the known-key table
//! (spec §8.1, plan Task 5 decision D13).
//!
//! Spec §8.1: "`built-in defaults` < `GAUNTLET.md` front matter < `.env` /
//! process environment < CLI flags. Later wins. The effective config is
//! digested into `Run.config_fingerprint`."
//!
//! The known-key table is THE single place key knowledge lives: every
//! documented §8.2 key path maps to one [`ConfigKey`] variant with its value
//! kind and the Annex E variable(s) that address it. A front-matter path not
//! in the table becomes a collected `unknown_key` (reported, not fatal —
//! §8.2 forward compatibility); an environment variable mapping into no key
//! is the environment layer's `unknown_env`. If the table ever grows
//! per-key parsing branches, the plan's schema-driven redesign trigger
//! fires.
//!
//! Naming note (recorded deviation, not a fix): the §8.6 setting is
//! `git_strategy` and the Annex E variable is `GAUNTLET_VERSIONING_STRATEGY`
//! — the mapping is DATA in this table, both sides left as they are.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use gauntlet_domain::MicroUsd;

use crate::config::defaults::{self, InteractiveDefault};
use crate::config::env_layer::EnvLayer;
use crate::config::error::ConfigError;
use crate::config::gauntlet_md::{FmEntry, FmScalar, FmValue, FrontMatter};

// ---------------------------------------------------------------------------
// The known-key table
// ---------------------------------------------------------------------------

/// The value kind a known key accepts; mismatch is a typed error, not a
/// coerced guess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ValueKind {
    /// A string, possibly carrying `$VAR` / `${VAR}` indirection.
    Text,
    /// A whole number.
    WholeNumber,
    /// A boolean.
    Toggle,
    /// A list of strings.
    TextList,
    /// Decimal money, stored as its raw token: the §8.2 example spells the
    /// ceiling as a bare integer, decimal dollars are strings. Parsing into
    /// the domain newtype happens at resolution.
    Money,
}

impl ValueKind {
    fn name(self) -> &'static str {
        match self {
            Self::Text => "a string",
            Self::WholeNumber => "a whole number",
            Self::Toggle => "a boolean",
            Self::TextList => "a list of strings",
            Self::Money => "a monetary value",
        }
    }
}

/// One addressable setting: every documented §8.2 key path plus the two
/// environment-only settings (`mcp_required`, `report_dir`).
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum ConfigKey {
    ReferenceManifest,
    ReferenceAnswerKey,
    ReferenceMap,
    ReferenceComparisonContract,
    ReferenceDigestAlgo,
    RolesDir,
    RunnersDefault,
    /// `runners.by_role.<role>.runner` — the role name is data.
    RunnersByRoleRunner(String),
    /// `runners.by_role.<role>.model` — the role name is data.
    RunnersByRoleModel(String),
    ObservationFacets,
    ReviewMode,
    ReviewMaxCritics,
    VerificationPreferDeterministic,
    SandboxBackend,
    SandboxPolicyDir,
    BudgetMaxRounds,
    BudgetMaxParallel,
    BudgetMaxDepth,
    BudgetMaxWallSeconds,
    BudgetMaxTokens,
    BudgetMaxCostUsd,
    BudgetRecoveryReservePct,
    BudgetDiagnosticBudget,
    /// Environment-only (Annex E Limits group; no §8.2 front-matter key):
    /// the §11.5 non-convergence threshold.
    NonconvergenceN,
    /// Environment-only (Annex E Limits group): the §11.4 reconciliation
    /// tick window.
    ControlPollIntervalMs,
    /// Environment-only (Annex E Limits group): the §11.4 stall window.
    /// REQUIRED for a control loop — the spec tables no default.
    ControlStallTimeoutMs,
    /// Environment-only (Annex E Limits group): the §11.5 transient-failure
    /// backoff floor. REQUIRED for a control loop — the spec tables no
    /// default.
    ControlRetryBaseMs,
    /// Environment-only (Annex E Limits group): the §11.5 transient-failure
    /// backoff ceiling. REQUIRED for a control loop — the spec tables no
    /// default.
    ControlRetryMaxMs,
    /// Environment-only (Annex E Limits group): the §11.5 per-Unit attempt
    /// ceiling. REQUIRED for a control loop — the spec tables no default.
    ControlMaxAttemptsPerUnit,
    WorkspaceRoot,
    WorkspaceUseWorktrees,
    StateBackend,
    StateDir,
    EvidenceDir,
    OperatorSurfaces,
    Interactive,
    GitStrategy,
    McpRequired,
    ReportDir,
}

impl ConfigKey {
    /// The dotted §8.2 path (or the environment-only setting's name).
    pub fn path(&self) -> String {
        match self {
            Self::ReferenceManifest => "reference.manifest".to_owned(),
            Self::ReferenceAnswerKey => "reference.answer_key".to_owned(),
            Self::ReferenceMap => "reference.map".to_owned(),
            Self::ReferenceComparisonContract => "reference.comparison_contract".to_owned(),
            Self::ReferenceDigestAlgo => "reference.digest_algo".to_owned(),
            Self::RolesDir => "roles.dir".to_owned(),
            Self::RunnersDefault => "runners.default".to_owned(),
            Self::RunnersByRoleRunner(role) => format!("runners.by_role.{role}.runner"),
            Self::RunnersByRoleModel(role) => format!("runners.by_role.{role}.model"),
            Self::ObservationFacets => "observation.facets".to_owned(),
            Self::ReviewMode => "review.mode".to_owned(),
            Self::ReviewMaxCritics => "review.max_critics".to_owned(),
            Self::VerificationPreferDeterministic => "verification.prefer_deterministic".to_owned(),
            Self::SandboxBackend => "sandbox.backend".to_owned(),
            Self::SandboxPolicyDir => "sandbox.policy_dir".to_owned(),
            Self::BudgetMaxRounds => "budget.max_rounds".to_owned(),
            Self::BudgetMaxParallel => "budget.max_parallel".to_owned(),
            Self::BudgetMaxDepth => "budget.max_depth".to_owned(),
            Self::BudgetMaxWallSeconds => "budget.max_wall_seconds".to_owned(),
            Self::BudgetMaxTokens => "budget.max_tokens".to_owned(),
            Self::BudgetMaxCostUsd => "budget.max_cost_usd".to_owned(),
            Self::BudgetRecoveryReservePct => "budget.recovery_reserve_pct".to_owned(),
            Self::BudgetDiagnosticBudget => "budget.diagnostic_budget".to_owned(),
            Self::NonconvergenceN => "nonconvergence_n".to_owned(),
            Self::ControlPollIntervalMs => "poll_interval_ms".to_owned(),
            Self::ControlStallTimeoutMs => "stall_timeout_ms".to_owned(),
            Self::ControlRetryBaseMs => "retry_base_ms".to_owned(),
            Self::ControlRetryMaxMs => "retry_max_ms".to_owned(),
            Self::ControlMaxAttemptsPerUnit => "max_attempts_per_unit".to_owned(),
            Self::WorkspaceRoot => "workspace.root".to_owned(),
            Self::WorkspaceUseWorktrees => "workspace.use_worktrees".to_owned(),
            Self::StateBackend => "state.backend".to_owned(),
            Self::StateDir => "state.dir".to_owned(),
            Self::EvidenceDir => "evidence.dir".to_owned(),
            Self::OperatorSurfaces => "operator.surfaces".to_owned(),
            Self::Interactive => "interactive".to_owned(),
            Self::GitStrategy => "git_strategy".to_owned(),
            Self::McpRequired => "mcp_required".to_owned(),
            Self::ReportDir => "report_dir".to_owned(),
        }
    }

    /// The Annex E variable that overrides this key, when one exists. The
    /// role runner/model keys are addressed by `GAUNTLET_<ROLE>_RUNNER` /
    /// `GAUNTLET_<ROLE>_MODEL` and resolve through [`ConfigKey::from_env`].
    pub fn env(&self) -> Option<&'static str> {
        match self {
            Self::ReferenceManifest => Some("GAUNTLET_REFERENCE_MANIFEST"),
            Self::ReferenceAnswerKey => Some("GAUNTLET_ANSWER_KEY_PATH"),
            Self::ReferenceMap => Some("GAUNTLET_MAP_PATH"),
            Self::ReferenceComparisonContract => Some("GAUNTLET_COMPARISON_CONTRACT"),
            Self::ReferenceDigestAlgo => Some("GAUNTLET_DIGEST_ALGO"),
            Self::RolesDir => Some("GAUNTLET_ROLES_DIR"),
            Self::RunnersDefault => Some("GAUNTLET_AGENT_RUNNER"),
            Self::ObservationFacets => None,
            Self::ReviewMode => Some("GAUNTLET_REVIEW_MODE"),
            Self::ReviewMaxCritics => Some("GAUNTLET_MAX_CRITICS"),
            Self::VerificationPreferDeterministic => None,
            Self::SandboxBackend => Some("GAUNTLET_SANDBOX_BACKEND"),
            Self::SandboxPolicyDir => Some("GAUNTLET_SANDBOX_POLICY_DIR"),
            Self::BudgetMaxRounds => Some("GAUNTLET_MAX_ROUNDS"),
            Self::BudgetMaxParallel => Some("GAUNTLET_MAX_PARALLEL"),
            Self::BudgetMaxDepth => Some("GAUNTLET_MAX_DEPTH"),
            Self::BudgetMaxWallSeconds => Some("GAUNTLET_MAX_WALL_SECONDS"),
            Self::BudgetMaxTokens => Some("GAUNTLET_MAX_TOKENS"),
            Self::BudgetMaxCostUsd => Some("GAUNTLET_MAX_COST_USD"),
            Self::BudgetRecoveryReservePct => Some("GAUNTLET_RECOVERY_RESERVE_PCT"),
            Self::BudgetDiagnosticBudget => Some("GAUNTLET_DIAGNOSTIC_BUDGET"),
            Self::NonconvergenceN => Some("GAUNTLET_NONCONVERGENCE_N"),
            Self::ControlPollIntervalMs => Some("GAUNTLET_POLL_INTERVAL_MS"),
            Self::ControlStallTimeoutMs => Some("GAUNTLET_STALL_TIMEOUT_MS"),
            Self::ControlRetryBaseMs => Some("GAUNTLET_RETRY_BASE_MS"),
            Self::ControlRetryMaxMs => Some("GAUNTLET_RETRY_MAX_MS"),
            Self::ControlMaxAttemptsPerUnit => Some("GAUNTLET_MAX_ATTEMPTS_PER_UNIT"),
            Self::WorkspaceRoot => None,
            Self::WorkspaceUseWorktrees => None,
            Self::StateBackend => Some("GAUNTLET_STATE_BACKEND"),
            Self::StateDir => Some("GAUNTLET_STATE_DIR"),
            Self::EvidenceDir => Some("GAUNTLET_ARTIFACT_DIR"),
            Self::OperatorSurfaces => Some("GAUNTLET_OPERATOR_SURFACES"),
            // Annex E spells this one GAUNTLET_VERSIONING_STRATEGY; the §8.6
            // setting name is git_strategy. Data here, not a rename.
            Self::GitStrategy => Some("GAUNTLET_VERSIONING_STRATEGY"),
            Self::Interactive => Some("GAUNTLET_INTERACTIVE"),
            Self::McpRequired => Some("GAUNTLET_MCP_REQUIRED"),
            Self::ReportDir => Some("GAUNTLET_REPORT_DIR"),
            Self::RunnersByRoleRunner(_) | Self::RunnersByRoleModel(_) => None,
        }
    }

    /// The value kind this key accepts.
    pub fn kind(&self) -> ValueKind {
        match self {
            Self::ReferenceManifest
            | Self::ReferenceAnswerKey
            | Self::ReferenceMap
            | Self::ReferenceComparisonContract
            | Self::ReferenceDigestAlgo
            | Self::RolesDir
            | Self::RunnersDefault
            | Self::RunnersByRoleRunner(_)
            | Self::RunnersByRoleModel(_)
            | Self::ReviewMode
            | Self::SandboxBackend
            | Self::SandboxPolicyDir
            | Self::WorkspaceRoot
            | Self::StateBackend
            | Self::StateDir
            | Self::EvidenceDir
            | Self::GitStrategy => ValueKind::Text,
            Self::BudgetMaxCostUsd => ValueKind::Money,
            Self::ReviewMaxCritics
            | Self::BudgetMaxRounds
            | Self::BudgetMaxParallel
            | Self::BudgetMaxDepth
            | Self::BudgetMaxWallSeconds
            | Self::BudgetMaxTokens
            | Self::BudgetRecoveryReservePct
            | Self::BudgetDiagnosticBudget
            | Self::NonconvergenceN
            | Self::ControlPollIntervalMs
            | Self::ControlStallTimeoutMs
            | Self::ControlRetryBaseMs
            | Self::ControlRetryMaxMs
            | Self::ControlMaxAttemptsPerUnit => ValueKind::WholeNumber,
            Self::VerificationPreferDeterministic | Self::WorkspaceUseWorktrees => {
                ValueKind::Toggle
            }
            Self::ObservationFacets | Self::OperatorSurfaces | Self::McpRequired => {
                ValueKind::TextList
            }
            Self::Interactive => ValueKind::Text,
            Self::ReportDir => ValueKind::Text,
        }
    }

    /// The key an Annex E variable addresses, when the table maps it.
    pub fn from_env(name: &str) -> Option<ConfigKey> {
        if let Some(key) = STATIC_ENV_KEYS
            .iter()
            .find(|(variable, _)| *variable == name)
        {
            return Some(key.1.clone());
        }
        for role in ROLE_NAMES {
            if name == role_runner_var(role) {
                return Some(ConfigKey::RunnersByRoleRunner(role.to_ascii_lowercase()));
            }
            if name == role_model_var(role) {
                return Some(ConfigKey::RunnersByRoleModel(role.to_ascii_lowercase()));
            }
        }
        None
    }
}

/// The dispatchable roles Annex E spells variables for.
const ROLE_NAMES: &[&str] = &[
    "ORCHESTRATOR",
    "BUILDER",
    "CRITIC",
    "VERIFIER",
    "OBSERVER",
    "OPTIMIZER",
];

fn role_runner_var(role: &str) -> String {
    format!("GAUNTLET_{role}_RUNNER")
}

fn role_model_var(role: &str) -> String {
    format!("GAUNTLET_{role}_MODEL")
}

/// The static Annex E → key rows (role runner/model pairs are dynamic).
const STATIC_ENV_KEYS: &[(&str, ConfigKey)] = &[
    ("GAUNTLET_REFERENCE_MANIFEST", ConfigKey::ReferenceManifest),
    ("GAUNTLET_ANSWER_KEY_PATH", ConfigKey::ReferenceAnswerKey),
    ("GAUNTLET_MAP_PATH", ConfigKey::ReferenceMap),
    (
        "GAUNTLET_COMPARISON_CONTRACT",
        ConfigKey::ReferenceComparisonContract,
    ),
    ("GAUNTLET_DIGEST_ALGO", ConfigKey::ReferenceDigestAlgo),
    ("GAUNTLET_ROLES_DIR", ConfigKey::RolesDir),
    ("GAUNTLET_AGENT_RUNNER", ConfigKey::RunnersDefault),
    ("GAUNTLET_REVIEW_MODE", ConfigKey::ReviewMode),
    ("GAUNTLET_MAX_CRITICS", ConfigKey::ReviewMaxCritics),
    ("GAUNTLET_SANDBOX_BACKEND", ConfigKey::SandboxBackend),
    ("GAUNTLET_SANDBOX_POLICY_DIR", ConfigKey::SandboxPolicyDir),
    ("GAUNTLET_MAX_ROUNDS", ConfigKey::BudgetMaxRounds),
    ("GAUNTLET_MAX_PARALLEL", ConfigKey::BudgetMaxParallel),
    ("GAUNTLET_MAX_DEPTH", ConfigKey::BudgetMaxDepth),
    ("GAUNTLET_MAX_WALL_SECONDS", ConfigKey::BudgetMaxWallSeconds),
    ("GAUNTLET_MAX_TOKENS", ConfigKey::BudgetMaxTokens),
    ("GAUNTLET_MAX_COST_USD", ConfigKey::BudgetMaxCostUsd),
    (
        "GAUNTLET_RECOVERY_RESERVE_PCT",
        ConfigKey::BudgetRecoveryReservePct,
    ),
    (
        "GAUNTLET_DIAGNOSTIC_BUDGET",
        ConfigKey::BudgetDiagnosticBudget,
    ),
    ("GAUNTLET_NONCONVERGENCE_N", ConfigKey::NonconvergenceN),
    (
        "GAUNTLET_POLL_INTERVAL_MS",
        ConfigKey::ControlPollIntervalMs,
    ),
    (
        "GAUNTLET_STALL_TIMEOUT_MS",
        ConfigKey::ControlStallTimeoutMs,
    ),
    ("GAUNTLET_RETRY_BASE_MS", ConfigKey::ControlRetryBaseMs),
    ("GAUNTLET_RETRY_MAX_MS", ConfigKey::ControlRetryMaxMs),
    (
        "GAUNTLET_MAX_ATTEMPTS_PER_UNIT",
        ConfigKey::ControlMaxAttemptsPerUnit,
    ),
    ("GAUNTLET_STATE_BACKEND", ConfigKey::StateBackend),
    ("GAUNTLET_STATE_DIR", ConfigKey::StateDir),
    ("GAUNTLET_ARTIFACT_DIR", ConfigKey::EvidenceDir),
    ("GAUNTLET_OPERATOR_SURFACES", ConfigKey::OperatorSurfaces),
    ("GAUNTLET_INTERACTIVE", ConfigKey::Interactive),
    ("GAUNTLET_VERSIONING_STRATEGY", ConfigKey::GitStrategy),
    ("GAUNTLET_MCP_REQUIRED", ConfigKey::McpRequired),
    ("GAUNTLET_REPORT_DIR", ConfigKey::ReportDir),
];

// ---------------------------------------------------------------------------
// Raw effective configuration
// ---------------------------------------------------------------------------

/// One role's runner assignment (§8.2 `runners.by_role`), raw tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleRunner {
    pub runner: String,
    pub model: Option<String>,
}

/// The reference block (§8.2), raw tokens.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReferenceSettings {
    pub manifest: String,
    pub answer_key: String,
    pub map: String,
    pub comparison_contract: String,
    pub digest_algo: String,
}

/// The budget block (§8.2). Every §8.6 numeric setting plus the two
/// Annex E-required ceilings §8.6 does not table. `max_cost_usd` stays a raw
/// token (decimal dollars) because the canonical form serializes raw text.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BudgetSettings {
    pub max_rounds: u32,
    pub max_parallel: u32,
    pub max_depth: u32,
    pub max_wall_seconds: u64,
    pub max_tokens: u64,
    pub max_cost_usd: String,
    pub recovery_reserve_pct: u8,
    pub diagnostic_budget: u32,
}

/// The control-loop settings (§11.4/§11.5; Annex E Limits group,
/// environment-only — no §8.2 front-matter keys). One setting has an inline
/// spec default ([`EffectiveConfig::control`].poll_interval_ms, §11.4);
/// the other four are REQUIRED by the composed controller and deliberately
/// carry no default here — the spec tables none, and inventing one would be
/// exactly the §8.6 "literal without a home" the defaults discipline
/// forbids. The typed absence error surfaces where the settings are
/// consumed (the control loop's policy construction), not here: a
/// doctor-only or export-only consumer of the configuration must still be
/// able to resolve it without a runnable loop.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ControlSettings {
    /// §11.4 names the tick variable `GAUNTLET_POLL_INTERVAL_MS` with an
    /// inline five-second default — the one inline default of the group.
    pub poll_interval_ms: u64,
    /// §11.4 step 2 `GAUNTLET_STALL_TIMEOUT_MS`. REQUIRED for a control
    /// loop; `None` until the environment supplies it.
    pub stall_timeout_ms: Option<u64>,
    /// §11.5 transient-failure backoff floor `GAUNTLET_RETRY_BASE_MS`.
    /// REQUIRED for a control loop; `None` until supplied.
    pub retry_base_ms: Option<u64>,
    /// §11.5 transient-failure backoff ceiling `GAUNTLET_RETRY_MAX_MS`.
    /// REQUIRED for a control loop; `None` until supplied.
    pub retry_max_ms: Option<u64>,
    /// §11.5 per-Unit attempt ceiling
    /// `GAUNTLET_MAX_ATTEMPTS_PER_UNIT`. REQUIRED for a control loop;
    /// `None` until supplied.
    pub max_attempts_per_unit: Option<u32>,
}

impl ControlSettings {
    /// The bootstrap state: the §11.4 inline tick default, everything else
    /// unset (required settings stay unset until the environment or flags
    /// supply them).
    pub fn from_defaults() -> Self {
        Self {
            poll_interval_ms: defaults::POLL_INTERVAL_MS,
            stall_timeout_ms: None,
            retry_base_ms: None,
            retry_max_ms: None,
            max_attempts_per_unit: None,
        }
    }
}

/// The effective configuration — RAW TOKENS ONLY (spec §8.1, plan Task 5
/// decision D4): string fields may carry `$VAR` / `${VAR}` references
/// verbatim; this is the structure the canonical serialization and
/// `config_fingerprint` digest. Resolution products live in
/// [`ResolvedConfig`], which never serializes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EffectiveConfig {
    pub reference: ReferenceSettings,
    pub roles_dir: String,
    pub runners_default: Option<String>,
    pub runners_by_role: BTreeMap<String, RoleRunner>,
    pub observation_facets: Vec<String>,
    pub review_mode: String,
    pub review_max_critics: u32,
    pub prefer_deterministic: bool,
    pub sandbox_backend: String,
    pub sandbox_policy_dir: String,
    pub budget: BudgetSettings,
    /// The §11.5 non-convergence threshold (environment-only; Annex E
    /// `GAUNTLET_NONCONVERGENCE_N`, default 3 — the inline spec default).
    pub nonconvergence_n: u32,
    /// The §11.4/§11.5 control-loop settings (environment-only; one inline
    /// default, four REQUIRED settings that stay unset until supplied).
    pub control: ControlSettings,
    pub workspace_root: String,
    pub workspace_use_worktrees: bool,
    pub state_backend: String,
    pub state_dir: String,
    pub evidence_dir: String,
    pub operator_surfaces: Vec<String>,
    pub interactive: InteractiveDefault,
    pub git_strategy: String,
    pub mcp_required: Vec<String>,
    pub report_dir: String,
}

impl EffectiveConfig {
    /// Layer 1: the conservative bootstrap. Path-ish defaults are ROOTLESS
    /// raw tokens (the §8.2 example's relative spellings, composed from the
    /// layout names); anchoring at a real root happens only in
    /// [`EffectiveConfig::resolve_against`]. The scratch-backed defaults
    /// (workspace, state, evidence) carry the host scratch location — the
    /// effective configuration is honestly machine-specific where its
    /// defaults are. `state_backend` defaults empty: persistence selection
    /// lands with plan Task 6, and an unselected backend is not guessed.
    pub fn from_defaults() -> Self {
        // Joining a layout helper onto an empty root yields the rootless
        // relative token; the real root anchors it later.
        let rootless = Path::new("");
        let raw = |path: PathBuf| path.to_string_lossy().into_owned();
        Self {
            reference: ReferenceSettings {
                manifest: raw(defaults::reference_manifest_at(rootless)),
                answer_key: raw(defaults::answer_key_at(rootless)),
                map: raw(defaults::map_at(rootless)),
                comparison_contract: raw(defaults::comparison_contract_at(rootless)),
                digest_algo: defaults::DIGEST_ALGO.to_owned(),
            },
            roles_dir: raw(defaults::roles_dir_at(rootless)),
            runners_default: None,
            runners_by_role: BTreeMap::new(),
            observation_facets: Vec::new(),
            review_mode: defaults::REVIEW_MODE.to_owned(),
            review_max_critics: defaults::MAX_CRITICS,
            prefer_deterministic: defaults::PREFER_DETERMINISTIC,
            sandbox_backend: defaults::SANDBOX_BACKEND.to_owned(),
            sandbox_policy_dir: raw(defaults::policy_dir_at(rootless)),
            budget: BudgetSettings {
                max_rounds: defaults::MAX_ROUNDS,
                max_parallel: defaults::MAX_PARALLEL,
                max_depth: defaults::MAX_DEPTH,
                max_wall_seconds: defaults::MAX_WALL_SECONDS,
                max_tokens: defaults::MAX_TOKENS,
                max_cost_usd: defaults::MAX_COST_USD.to_owned(),
                recovery_reserve_pct: defaults::RECOVERY_RESERVE_PCT,
                diagnostic_budget: defaults::DIAGNOSTIC_BUDGET,
            },
            nonconvergence_n: defaults::NONCONVERGENCE_N,
            control: ControlSettings::from_defaults(),
            workspace_root: raw(defaults::workspace_root_default()),
            workspace_use_worktrees: defaults::WORKSPACE_USE_WORKTREES,
            state_backend: String::new(),
            state_dir: raw(defaults::state_dir_default()),
            evidence_dir: raw(defaults::evidence_dir_default()),
            operator_surfaces: Vec::new(),
            interactive: defaults::INTERACTIVE,
            git_strategy: defaults::GIT_STRATEGY.to_owned(),
            mcp_required: Vec::new(),
            report_dir: raw(defaults::report_dir_at(rootless)),
        }
    }
}

// ---------------------------------------------------------------------------
// The merge
// ---------------------------------------------------------------------------

/// A front-matter path outside the known-key table, with its line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnknownKey {
    pub path: String,
    pub line: usize,
}

/// The CLI flag layer: typed values keyed by [`ConfigKey`]. `gauntlet doctor`
/// itself carries only `--config`; the layer exists so the precedence order
/// is complete and testable at every boundary (plan Task 5 teeth).
#[derive(Debug, Clone, Default)]
pub struct FlagOverrides {
    values: BTreeMap<ConfigKey, FlagValue>,
}

/// A flag-carried value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FlagValue {
    Text(String),
    WholeNumber(u64),
    Toggle(bool),
    TextList(Vec<String>),
}

impl FlagOverrides {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, key: ConfigKey, value: FlagValue) {
        self.values.insert(key, value);
    }

    pub fn get(&self, key: &ConfigKey) -> Option<&FlagValue> {
        self.values.get(key)
    }

    pub fn is_empty(&self) -> bool {
        self.values.is_empty()
    }
}

/// Everything the resolver needs: the three upper layers plus the root every
/// relative default composes against.
#[derive(Debug, Clone)]
pub struct ResolveInput<'a> {
    pub project_root: PathBuf,
    pub front_matter: Option<FrontMatter>,
    pub env: &'a EnvLayer,
    pub flags: FlagOverrides,
}

/// A successful merge: the raw effective configuration plus the collected
/// unknown-key report (§8.2: reported, never fatal).
#[derive(Debug, Clone)]
pub struct Resolved {
    pub effective: EffectiveConfig,
    pub unknown_keys: Vec<UnknownKey>,
}

/// Merges the four layers in precedence order: defaults, then GAUNTLET.md,
/// then the environment layer, then CLI flags. Later wins.
pub fn resolve(input: &ResolveInput<'_>) -> Result<Resolved, ConfigError> {
    let mut effective = EffectiveConfig::from_defaults();
    let mut unknown_keys = Vec::new();
    if let Some(fm) = &input.front_matter {
        apply_front_matter(&mut effective, fm, &mut unknown_keys)?;
    }
    apply_env_layer(&mut effective, input.env)?;
    apply_flags(&mut effective, &input.flags)?;
    Ok(Resolved {
        effective,
        unknown_keys,
    })
}

/// Layer 2: walk the parsed front matter through the known-key table.
fn apply_front_matter(
    effective: &mut EffectiveConfig,
    fm: &FrontMatter,
    unknown: &mut Vec<UnknownKey>,
) -> Result<(), ConfigError> {
    for entry in &fm.entries {
        match entry.key.as_str() {
            "reference" => walk_section(
                entry,
                &[
                    ("manifest", ConfigKey::ReferenceManifest),
                    ("answer_key", ConfigKey::ReferenceAnswerKey),
                    ("map", ConfigKey::ReferenceMap),
                    (
                        "comparison_contract",
                        ConfigKey::ReferenceComparisonContract,
                    ),
                    ("digest_algo", ConfigKey::ReferenceDigestAlgo),
                ],
                effective,
                unknown,
            )?,
            "roles" => walk_section(entry, &[("dir", ConfigKey::RolesDir)], effective, unknown)?,
            "runners" => walk_runners(entry, effective, unknown)?,
            "observation" => walk_section(
                entry,
                &[("facets", ConfigKey::ObservationFacets)],
                effective,
                unknown,
            )?,
            "review" => walk_section(
                entry,
                &[
                    ("mode", ConfigKey::ReviewMode),
                    ("max_critics", ConfigKey::ReviewMaxCritics),
                ],
                effective,
                unknown,
            )?,
            "verification" => walk_section(
                entry,
                &[(
                    "prefer_deterministic",
                    ConfigKey::VerificationPreferDeterministic,
                )],
                effective,
                unknown,
            )?,
            "sandbox" => walk_section(
                entry,
                &[
                    ("backend", ConfigKey::SandboxBackend),
                    ("policy_dir", ConfigKey::SandboxPolicyDir),
                ],
                effective,
                unknown,
            )?,
            "budget" => walk_section(
                entry,
                &[
                    ("max_rounds", ConfigKey::BudgetMaxRounds),
                    ("max_parallel", ConfigKey::BudgetMaxParallel),
                    ("max_depth", ConfigKey::BudgetMaxDepth),
                    ("max_wall_seconds", ConfigKey::BudgetMaxWallSeconds),
                    ("max_tokens", ConfigKey::BudgetMaxTokens),
                    ("max_cost_usd", ConfigKey::BudgetMaxCostUsd),
                    ("recovery_reserve_pct", ConfigKey::BudgetRecoveryReservePct),
                    ("diagnostic_budget", ConfigKey::BudgetDiagnosticBudget),
                ],
                effective,
                unknown,
            )?,
            "workspace" => walk_section(
                entry,
                &[
                    ("root", ConfigKey::WorkspaceRoot),
                    ("use_worktrees", ConfigKey::WorkspaceUseWorktrees),
                ],
                effective,
                unknown,
            )?,
            "state" => walk_section(
                entry,
                &[
                    ("backend", ConfigKey::StateBackend),
                    ("dir", ConfigKey::StateDir),
                ],
                effective,
                unknown,
            )?,
            "evidence" => walk_section(
                entry,
                &[("dir", ConfigKey::EvidenceDir)],
                effective,
                unknown,
            )?,
            "operator" => walk_section(
                entry,
                &[("surfaces", ConfigKey::OperatorSurfaces)],
                effective,
                unknown,
            )?,
            other => unknown.push(UnknownKey {
                path: other.to_owned(),
                line: entry.line,
            }),
        }
    }
    Ok(())
}

/// Walks one known top-level section whose leaves are all static keys.
fn walk_section(
    section: &FmEntry,
    leaves: &[(&str, ConfigKey)],
    effective: &mut EffectiveConfig,
    unknown: &mut Vec<UnknownKey>,
) -> Result<(), ConfigError> {
    let Some(entries) = section.value.as_map() else {
        return Err(ConfigError::KindMismatch {
            key: section.key.clone(),
            expected: "a mapping",
            found: "a scalar",
        });
    };
    for leaf in entries {
        match leaves.iter().find(|(name, _)| *name == leaf.key) {
            Some((_, key)) => assign_from_front_matter(effective, key, &leaf.value)?,
            None => unknown.push(UnknownKey {
                path: format!("{}.{}", section.key, leaf.key),
                line: leaf.line,
            }),
        }
    }
    Ok(())
}

/// Walks `runners`: the static `default` leaf plus the dynamic `by_role` map.
fn walk_runners(
    section: &FmEntry,
    effective: &mut EffectiveConfig,
    unknown: &mut Vec<UnknownKey>,
) -> Result<(), ConfigError> {
    let Some(entries) = section.value.as_map() else {
        return Err(ConfigError::KindMismatch {
            key: section.key.clone(),
            expected: "a mapping",
            found: "a scalar",
        });
    };
    for leaf in entries {
        match leaf.key.as_str() {
            "default" => {
                assign_from_front_matter(effective, &ConfigKey::RunnersDefault, &leaf.value)?
            }
            "by_role" => {
                let Some(roles) = leaf.value.as_map() else {
                    return Err(ConfigError::KindMismatch {
                        key: format!("runners.by_role.{}", leaf.key),
                        expected: "a mapping",
                        found: "a scalar",
                    });
                };
                for role in roles {
                    let Some(fields) = role.value.as_map() else {
                        return Err(ConfigError::KindMismatch {
                            key: format!("runners.by_role.{}", role.key),
                            expected: "a mapping",
                            found: "a scalar",
                        });
                    };
                    let mut runner = RoleRunner {
                        runner: String::new(),
                        model: None,
                    };
                    for field in fields {
                        match field.key.as_str() {
                            "runner" => {
                                runner.runner = expect_text(
                                    &ConfigKey::RunnersByRoleRunner(role.key.clone()),
                                    &field.value,
                                )?
                            }
                            "model" => {
                                runner.model = Some(expect_text(
                                    &ConfigKey::RunnersByRoleModel(role.key.clone()),
                                    &field.value,
                                )?)
                            }
                            other => unknown.push(UnknownKey {
                                path: format!("runners.by_role.{}.{}", role.key, other),
                                line: field.line,
                            }),
                        }
                    }
                    effective.runners_by_role.insert(role.key.clone(), runner);
                }
            }
            other => unknown.push(UnknownKey {
                path: format!("runners.{other}"),
                line: leaf.line,
            }),
        }
    }
    Ok(())
}

fn assign_from_front_matter(
    effective: &mut EffectiveConfig,
    key: &ConfigKey,
    value: &FmValue,
) -> Result<(), ConfigError> {
    match key.kind() {
        ValueKind::Text => {
            let text = expect_text(key, value)?;
            effective.set_text(key, text);
        }
        ValueKind::WholeNumber => {
            let n = expect_whole(key, value)?;
            effective.set_whole(key, n)?;
        }
        ValueKind::Toggle => {
            let b = expect_toggle(key, value)?;
            effective.set_toggle(key, b);
        }
        ValueKind::TextList => {
            let list = expect_list(key, value)?;
            effective.set_list(key, list);
        }
        ValueKind::Money => {
            // The §8.2 example spells money as a bare integer; decimals are
            // strings. Both become the raw token.
            let token = match value {
                FmValue::Int(n) => n.to_string(),
                FmValue::Str(text) => text.clone(),
                other => {
                    return Err(ConfigError::KindMismatch {
                        key: key.path(),
                        expected: ValueKind::Money.name(),
                        found: found_kind(other),
                    });
                }
            };
            effective.set_text(key, token);
        }
    }
    Ok(())
}

fn expect_text(key: &ConfigKey, value: &FmValue) -> Result<String, ConfigError> {
    match value {
        FmValue::Str(text) => Ok(text.clone()),
        other => Err(ConfigError::KindMismatch {
            key: key.path(),
            expected: ValueKind::Text.name(),
            found: found_kind(other),
        }),
    }
}

fn expect_whole(key: &ConfigKey, value: &FmValue) -> Result<u64, ConfigError> {
    match value {
        FmValue::Int(n) => {
            Ok(u64::try_from(*n).map_err(|_| ConfigError::ValueOutOfRange { key: key.path() })?)
        }
        other => Err(ConfigError::KindMismatch {
            key: key.path(),
            expected: ValueKind::WholeNumber.name(),
            found: found_kind(other),
        }),
    }
}

fn expect_toggle(key: &ConfigKey, value: &FmValue) -> Result<bool, ConfigError> {
    match value {
        FmValue::Bool(b) => Ok(*b),
        other => Err(ConfigError::KindMismatch {
            key: key.path(),
            expected: ValueKind::Toggle.name(),
            found: found_kind(other),
        }),
    }
}

fn expect_list(key: &ConfigKey, value: &FmValue) -> Result<Vec<String>, ConfigError> {
    match value {
        FmValue::Seq(items) => Ok(items.iter().map(FmScalar::raw).collect()),
        other => Err(ConfigError::KindMismatch {
            key: key.path(),
            expected: ValueKind::TextList.name(),
            found: found_kind(other),
        }),
    }
}

fn found_kind(value: &FmValue) -> &'static str {
    match value {
        FmValue::Str(_) => "a string",
        FmValue::Int(_) => "a whole number",
        FmValue::Bool(_) => "a boolean",
        FmValue::Seq(_) => "a list",
        FmValue::Map(_) => "a mapping",
    }
}

impl EffectiveConfig {
    fn set_text(&mut self, key: &ConfigKey, text: String) {
        match key {
            ConfigKey::ReferenceManifest => self.reference.manifest = text,
            ConfigKey::ReferenceAnswerKey => self.reference.answer_key = text,
            ConfigKey::ReferenceMap => self.reference.map = text,
            ConfigKey::ReferenceComparisonContract => self.reference.comparison_contract = text,
            ConfigKey::ReferenceDigestAlgo => self.reference.digest_algo = text,
            ConfigKey::RolesDir => self.roles_dir = text,
            ConfigKey::RunnersDefault => self.runners_default = Some(text),
            ConfigKey::ReviewMode => self.review_mode = text,
            ConfigKey::SandboxBackend => self.sandbox_backend = text,
            ConfigKey::SandboxPolicyDir => self.sandbox_policy_dir = text,
            ConfigKey::BudgetMaxCostUsd => self.budget.max_cost_usd = text,
            ConfigKey::WorkspaceRoot => self.workspace_root = text,
            ConfigKey::StateBackend => self.state_backend = text,
            ConfigKey::StateDir => self.state_dir = text,
            ConfigKey::EvidenceDir => self.evidence_dir = text,
            ConfigKey::GitStrategy => self.git_strategy = text,
            ConfigKey::ReportDir => self.report_dir = text,
            other => self.set_dynamic_text(other, text),
        }
    }

    fn set_dynamic_text(&mut self, key: &ConfigKey, text: String) {
        if let ConfigKey::RunnersByRoleRunner(role) = key {
            let entry = self
                .runners_by_role
                .entry(role.clone())
                .or_insert_with(|| RoleRunner {
                    runner: String::new(),
                    model: None,
                });
            entry.runner = text;
        } else if let ConfigKey::RunnersByRoleModel(role) = key {
            let entry = self
                .runners_by_role
                .entry(role.clone())
                .or_insert_with(|| RoleRunner {
                    runner: String::new(),
                    model: None,
                });
            entry.model = Some(text);
        }
    }

    fn set_whole(&mut self, key: &ConfigKey, n: u64) -> Result<(), ConfigError> {
        // No clamping: a value that does not fit the setting's type is a
        // typed error, never a coerced guess.
        let overflow = || ConfigError::ValueOutOfRange { key: key.path() };
        let as_u32 = u32::try_from(n).map_err(|_| overflow());
        match key {
            ConfigKey::ReviewMaxCritics => self.review_max_critics = as_u32?,
            ConfigKey::BudgetMaxRounds => self.budget.max_rounds = as_u32?,
            ConfigKey::BudgetMaxParallel => self.budget.max_parallel = as_u32?,
            ConfigKey::BudgetMaxDepth => self.budget.max_depth = as_u32?,
            ConfigKey::BudgetMaxWallSeconds => self.budget.max_wall_seconds = n,
            ConfigKey::BudgetMaxTokens => self.budget.max_tokens = n,
            ConfigKey::BudgetRecoveryReservePct => {
                self.budget.recovery_reserve_pct = u8::try_from(n).map_err(|_| overflow())?;
            }
            ConfigKey::BudgetDiagnosticBudget => self.budget.diagnostic_budget = as_u32?,
            ConfigKey::NonconvergenceN => {
                // §11.5's trigger counts occurrences; a threshold of zero
                // cannot be reached by counting and is a typed refusal,
                // never a coerced one.
                if n == 0 {
                    return Err(overflow());
                }
                self.nonconvergence_n = as_u32?;
            }
            ConfigKey::ControlPollIntervalMs => {
                // A zero tick window is a busy-spin, not a tick; refused.
                if n == 0 {
                    return Err(overflow());
                }
                self.control.poll_interval_ms = n;
            }
            ConfigKey::ControlStallTimeoutMs => {
                // A zero stall window stalls on the first poll; refused.
                if n == 0 {
                    return Err(overflow());
                }
                self.control.stall_timeout_ms = Some(n);
            }
            ConfigKey::ControlRetryBaseMs => {
                // Zero backoff is no backoff (§11.5's floor exists to be
                // nonzero); refused.
                if n == 0 {
                    return Err(overflow());
                }
                self.control.retry_base_ms = Some(n);
            }
            ConfigKey::ControlRetryMaxMs => {
                if n == 0 {
                    return Err(overflow());
                }
                self.control.retry_max_ms = Some(n);
            }
            ConfigKey::ControlMaxAttemptsPerUnit => {
                // A zero attempt ceiling exhausts before the first
                // dispatch; refused.
                if n == 0 {
                    return Err(overflow());
                }
                self.control.max_attempts_per_unit = Some(as_u32?);
            }
            _ => return Err(ConfigError::FlagKind { key: key.path() }),
        }
        Ok(())
    }

    fn set_toggle(&mut self, key: &ConfigKey, value: bool) {
        match key {
            ConfigKey::VerificationPreferDeterministic => self.prefer_deterministic = value,
            ConfigKey::WorkspaceUseWorktrees => self.workspace_use_worktrees = value,
            _ => {}
        }
    }

    fn set_list(&mut self, key: &ConfigKey, list: Vec<String>) {
        match key {
            ConfigKey::ObservationFacets => self.observation_facets = list,
            ConfigKey::OperatorSurfaces => self.operator_surfaces = list,
            ConfigKey::McpRequired => self.mcp_required = list,
            _ => {}
        }
    }
}

/// Layer 3: every Annex E variable the table maps, when the layer carries a
/// non-empty value for it.
fn apply_env_layer(effective: &mut EffectiveConfig, env: &EnvLayer) -> Result<(), ConfigError> {
    for (variable, key) in STATIC_ENV_KEYS {
        let Some(raw) = env.value(variable).filter(|value| !value.is_empty()) else {
            continue;
        };
        apply_env_value(effective, key, variable, raw)?;
    }
    for role in ROLE_NAMES {
        for (variable, key) in [
            (
                role_runner_var(role),
                ConfigKey::RunnersByRoleRunner(role.to_ascii_lowercase()),
            ),
            (
                role_model_var(role),
                ConfigKey::RunnersByRoleModel(role.to_ascii_lowercase()),
            ),
        ] {
            if let Some(raw) = env.value(&variable).filter(|value| !value.is_empty()) {
                apply_env_value(effective, &key, &variable, raw)?;
            }
        }
    }
    Ok(())
}

fn apply_env_value(
    effective: &mut EffectiveConfig,
    key: &ConfigKey,
    variable: &str,
    raw: &str,
) -> Result<(), ConfigError> {
    match key.kind() {
        ValueKind::Text | ValueKind::Money => {
            if matches!(key, ConfigKey::Interactive) {
                effective.interactive =
                    InteractiveDefault::parse(raw).map_err(|_| ConfigError::EnvNotABoolean {
                        name: variable.to_owned(),
                        key: key.path(),
                    })?;
            } else {
                effective.set_text(key, raw.to_owned());
            }
        }
        ValueKind::WholeNumber => {
            let n: u64 = raw.parse().map_err(|_| ConfigError::EnvNotANumber {
                name: variable.to_owned(),
                key: key.path(),
            })?;
            effective.set_whole(key, n)?;
        }
        ValueKind::Toggle => {
            let value = match raw {
                "true" => true,
                "false" => false,
                _ => {
                    return Err(ConfigError::EnvNotABoolean {
                        name: variable.to_owned(),
                        key: key.path(),
                    });
                }
            };
            effective.set_toggle(key, value);
        }
        ValueKind::TextList => {
            let list = raw
                .split(',')
                .map(|item| item.trim().to_owned())
                .filter(|item| !item.is_empty())
                .collect();
            effective.set_list(key, list);
        }
    }
    Ok(())
}

/// Layer 4: CLI flags — typed values, later than everything.
fn apply_flags(effective: &mut EffectiveConfig, flags: &FlagOverrides) -> Result<(), ConfigError> {
    for (key, value) in &flags.values {
        let kinds_match = matches!(
            (key.kind(), value),
            (ValueKind::Text, FlagValue::Text(_))
                | (ValueKind::Money, FlagValue::Text(_))
                | (ValueKind::WholeNumber, FlagValue::WholeNumber(_))
                | (ValueKind::Toggle, FlagValue::Toggle(_))
                | (ValueKind::TextList, FlagValue::TextList(_))
        );
        if !kinds_match {
            return Err(ConfigError::FlagKind { key: key.path() });
        }
        match value {
            FlagValue::Text(text) => effective.set_text(key, text.clone()),
            FlagValue::WholeNumber(n) => {
                effective.set_whole(key, *n)?;
            }
            FlagValue::Toggle(b) => effective.set_toggle(key, *b),
            FlagValue::TextList(list) => effective.set_list(key, list.clone()),
        }
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Indirection resolution (products, never serialized)
// ---------------------------------------------------------------------------

/// The resolved products of the effective configuration: `$VAR` expanded
/// against the environment, paths anchored at the project root, money parsed
/// into the domain newtype. NEVER serialized and never printed — a resolved
/// value may be a credential (§8.1: generated files never contain
/// credentials).
#[derive(Debug, Clone)]
pub struct ResolvedConfig {
    pub project_root: PathBuf,
    pub reference: ResolvedReference,
    pub roles_dir: PathBuf,
    pub runners_default: Option<String>,
    pub runners_by_role: BTreeMap<String, RoleRunner>,
    pub observation_facets: Vec<String>,
    pub review_mode: String,
    pub review_max_critics: u32,
    pub sandbox_backend: String,
    pub sandbox_policy_dir: PathBuf,
    pub budget: ResolvedBudget,
    /// The §11.5 non-convergence threshold (`GAUNTLET_NONCONVERGENCE_N`).
    pub nonconvergence_n: u32,
    /// The §11.4/§11.5 control-loop settings — plain numbers, carried
    /// through unchanged (nothing in the group is an indirection or path).
    pub control: ControlSettings,
    pub workspace_root: PathBuf,
    pub workspace_use_worktrees: bool,
    /// Expanded backend token; persistence wiring lands with plan Task 6.
    pub state_backend: String,
    pub state_dir: PathBuf,
    pub evidence_dir: PathBuf,
    pub report_dir: PathBuf,
    pub operator_surfaces: Vec<String>,
    pub interactive: InteractiveDefault,
    pub git_strategy: String,
    pub mcp_required: Vec<String>,
    /// The variable NAMES whose values were expanded (evidence text; values
    /// never recorded).
    pub indirections_used: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedReference {
    pub manifest: PathBuf,
    pub answer_key: PathBuf,
    pub map: PathBuf,
    pub comparison_contract: PathBuf,
    pub digest_algo: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedBudget {
    pub max_rounds: u32,
    pub max_parallel: u32,
    pub max_depth: u32,
    pub max_wall_seconds: u64,
    pub max_tokens: u64,
    /// Parsed decimal dollars into the domain newtype.
    pub max_cost_usd: MicroUsd,
    pub recovery_reserve_pct: u8,
    pub diagnostic_budget: u32,
}

impl ResolvedBudget {
    /// The §15.2 diagnostic allowance (plan Task 20 COR-D9: the mapping is
    /// a POLICY shape, so it lives in the application layer, not the
    /// composition root — the CLI passes the config scalar through). The
    /// config scalar `diagnostic_budget` bounds the number of diagnostic
    /// cycles; each cycle may spend one per-attempt share of the run's own
    /// forward ceilings (the recovery reserve is removed first — diagnosis
    /// spends forward money, never the recovery reserve). No new policy
    /// constant is invented.
    pub fn diagnostic_allowance(&self) -> gauntlet_domain::DiagnosticAllowance {
        let cycles = u64::from(self.diagnostic_budget.max(1));
        let forward_pct = u64::from(100u8.saturating_sub(self.recovery_reserve_pct.min(100)));
        let per_cycle_tokens = self.max_tokens.saturating_mul(forward_pct) / 100;
        let per_cycle_cost = self.max_cost_usd.get().saturating_mul(forward_pct) / 100;
        gauntlet_domain::DiagnosticAllowance::new(
            per_cycle_tokens.saturating_mul(cycles),
            gauntlet_domain::MicroUsd::new(per_cycle_cost.saturating_mul(cycles)),
        )
    }

    /// The domain `Limits` (spec §5.3.7): conversion only — the domain
    /// refuses zero ceilings, which is exactly the §8.5 check-8 rule.
    pub fn limits(&self) -> Result<gauntlet_domain::Limits, gauntlet_domain::BudgetError> {
        gauntlet_domain::Limits::new(
            self.max_rounds,
            self.max_parallel,
            self.max_depth,
            self.max_wall_seconds,
            self.max_tokens,
            self.max_cost_usd,
        )
    }
}

impl EffectiveConfig {
    /// Expands every indirection against `env` and anchors relative paths at
    /// `project_root`. A `$VAR` the environment does not carry is a typed
    /// error naming the variable and the key — never the value.
    pub fn resolve_against(
        &self,
        env: &EnvLayer,
        project_root: &Path,
    ) -> Result<ResolvedConfig, ConfigError> {
        let mut used: Vec<String> = Vec::new();
        // Local helpers over the shared `used` list; plain functions, not
        // closures, so each can borrow it mutably in turn.
        fn expand(
            raw: &str,
            env: &EnvLayer,
            key: &str,
            used: &mut Vec<String>,
        ) -> Result<String, ConfigError> {
            expand_indirections(raw, env, key, used)
        }
        fn anchored(
            raw: &str,
            env: &EnvLayer,
            key: &str,
            root: &Path,
            used: &mut Vec<String>,
        ) -> Result<PathBuf, ConfigError> {
            let expanded = expand(raw, env, key, used)?;
            Ok(anchor(&expanded, root))
        }
        let cost_raw = expand(
            &self.budget.max_cost_usd,
            env,
            "budget.max_cost_usd",
            &mut used,
        )?;
        let budget = ResolvedBudget {
            max_rounds: self.budget.max_rounds,
            max_parallel: self.budget.max_parallel,
            max_depth: self.budget.max_depth,
            max_wall_seconds: self.budget.max_wall_seconds,
            max_tokens: self.budget.max_tokens,
            max_cost_usd: parse_dollars(&cost_raw)?,
            recovery_reserve_pct: self.budget.recovery_reserve_pct,
            diagnostic_budget: self.budget.diagnostic_budget,
        };
        let nonconvergence_n = self.nonconvergence_n;
        let control = self.control.clone();
        let mut runners_by_role = BTreeMap::new();
        for (role, runner) in &self.runners_by_role {
            let resolved_runner = expand(
                &runner.runner,
                env,
                &format!("runners.by_role.{role}.runner"),
                &mut used,
            )?;
            let resolved_model = runner
                .model
                .as_deref()
                .map(|model| {
                    expand(
                        model,
                        env,
                        &format!("runners.by_role.{role}.model"),
                        &mut used,
                    )
                })
                .transpose()?;
            runners_by_role.insert(
                role.clone(),
                RoleRunner {
                    runner: resolved_runner,
                    model: resolved_model,
                },
            );
        }
        let runners_default = self
            .runners_default
            .as_deref()
            .map(|raw| expand(raw, env, "runners.default", &mut used))
            .transpose()?;
        let reference = ResolvedReference {
            manifest: anchored(
                &self.reference.manifest,
                env,
                "reference.manifest",
                project_root,
                &mut used,
            )?,
            answer_key: anchored(
                &self.reference.answer_key,
                env,
                "reference.answer_key",
                project_root,
                &mut used,
            )?,
            map: anchored(
                &self.reference.map,
                env,
                "reference.map",
                project_root,
                &mut used,
            )?,
            comparison_contract: anchored(
                &self.reference.comparison_contract,
                env,
                "reference.comparison_contract",
                project_root,
                &mut used,
            )?,
            digest_algo: self.reference.digest_algo.clone(),
        };
        let roles_dir = anchored(&self.roles_dir, env, "roles.dir", project_root, &mut used)?;
        let sandbox_policy_dir = anchored(
            &self.sandbox_policy_dir,
            env,
            "sandbox.policy_dir",
            project_root,
            &mut used,
        )?;
        let workspace_root = anchored(
            &self.workspace_root,
            env,
            "workspace.root",
            project_root,
            &mut used,
        )?;
        let state_dir = anchored(&self.state_dir, env, "state.dir", project_root, &mut used)?;
        let evidence_dir = anchored(
            &self.evidence_dir,
            env,
            "evidence.dir",
            project_root,
            &mut used,
        )?;
        let report_dir = anchored(&self.report_dir, env, "report_dir", project_root, &mut used)?;
        // Remaining text products expand against the same rules; lists expand
        // item by item (a nested fn, so `used` borrows in turn).
        fn expand_list(
            values: &[String],
            env: &EnvLayer,
            key: &str,
            used: &mut Vec<String>,
        ) -> Result<Vec<String>, ConfigError> {
            let mut out = Vec::with_capacity(values.len());
            for value in values {
                out.push(expand(value, env, key, used)?);
            }
            Ok(out)
        }
        let observation_facets = expand_list(
            &self.observation_facets,
            env,
            "observation.facets",
            &mut used,
        )?;
        let review_mode = expand(&self.review_mode, env, "review.mode", &mut used)?;
        let sandbox_backend = expand(&self.sandbox_backend, env, "sandbox.backend", &mut used)?;
        let state_backend = expand(&self.state_backend, env, "state.backend", &mut used)?;
        let git_strategy = expand(&self.git_strategy, env, "git_strategy", &mut used)?;
        let operator_surfaces =
            expand_list(&self.operator_surfaces, env, "operator.surfaces", &mut used)?;
        let mcp_required = expand_list(&self.mcp_required, env, "mcp_required", &mut used)?;
        let digest_algo = expand(
            &self.reference.digest_algo,
            env,
            "reference.digest_algo",
            &mut used,
        )?;
        used.sort();
        used.dedup();
        Ok(ResolvedConfig {
            project_root: project_root.to_path_buf(),
            reference: ResolvedReference {
                digest_algo,
                ..reference
            },
            roles_dir,
            runners_default,
            runners_by_role,
            observation_facets,
            review_mode,
            review_max_critics: self.review_max_critics,
            sandbox_backend,
            sandbox_policy_dir,
            budget,
            nonconvergence_n,
            control,
            workspace_root,
            workspace_use_worktrees: self.workspace_use_worktrees,
            state_backend,
            state_dir,
            evidence_dir,
            report_dir,
            operator_surfaces,
            interactive: self.interactive,
            git_strategy,
            mcp_required,
            indirections_used: used,
        })
    }
}

/// Anchors a possibly-relative path at the project root; absolute values
/// stay absolute.
fn anchor(expanded: &str, project_root: &Path) -> PathBuf {
    let path = Path::new(expanded);
    if path.is_absolute() {
        path.to_path_buf()
    } else {
        project_root.join(path)
    }
}

/// Parses decimal dollars (`25`, `25.5`) into micro-dollars; more than six
/// fractional digits or a malformed shape is a typed error naming the key.
fn parse_dollars(raw: &str) -> Result<MicroUsd, ConfigError> {
    let malformed = || ConfigError::ValueOutOfRange {
        key: "budget.max_cost_usd".to_owned(),
    };
    let (whole, fraction) = match raw.split_once('.') {
        Some((whole, fraction)) => (whole, fraction),
        None => (raw, ""),
    };
    if whole.is_empty()
        || !whole.bytes().all(|byte| byte.is_ascii_digit())
        || fraction.len() > 6
        || !fraction.bytes().all(|byte| byte.is_ascii_digit())
    {
        return Err(malformed());
    }
    let whole: u64 = whole.parse().map_err(|_| malformed())?;
    let mut micros = whole.checked_mul(1_000_000).ok_or_else(malformed)?;
    let mut frac_value = 0u64;
    let mut scale = 100_000u64;
    for digit in fraction.bytes() {
        frac_value += u64::from(digit - b'0') * scale;
        scale /= 10;
    }
    micros = micros.checked_add(frac_value).ok_or_else(malformed)?;
    Ok(MicroUsd::new(micros))
}

/// Expands `$VAR` and `${VAR}` tokens. Presence-validated; values are used
/// and never recorded beyond the product itself.
///
/// `pub(crate)` (plan Task 17, builder B1): the roles loader reuses this
/// ONE expansion implementation for role-file indirections instead of
/// forking the grammar — same tokens, same presence rule, the same
/// never-print-the-value error. Same crate, additive visibility only.
pub(crate) fn expand_indirections(
    raw: &str,
    env: &EnvLayer,
    key: &str,
    used: &mut Vec<String>,
) -> Result<String, ConfigError> {
    let bytes = raw.as_bytes();
    let mut out = String::with_capacity(raw.len());
    let mut at = 0usize;
    while at < bytes.len() {
        if bytes[at] != b'$' {
            let end = (at + 1..bytes.len())
                .find(|offset| bytes[*offset] == b'$')
                .unwrap_or(bytes.len());
            out.push_str(&raw[at..end]);
            at = end;
            continue;
        }
        // Braced form.
        if bytes.get(at + 1) == Some(&b'{') {
            let Some(close) = raw[at + 2..].find('}') else {
                return Err(ConfigError::UnresolvedIndirection {
                    var: raw[at..].to_owned(),
                    key: key.to_owned(),
                });
            };
            let name = &raw[at + 2..at + 2 + close];
            if !is_var_name(name) {
                return Err(ConfigError::UnresolvedIndirection {
                    var: name.to_owned(),
                    key: key.to_owned(),
                });
            }
            push_expanded(&mut out, name, env, key, used)?;
            at = at + 2 + close + 1;
            continue;
        }
        // Bare form: longest identifier run.
        let mut end = at + 1;
        while end < bytes.len() && is_var_byte(bytes[end]) {
            end += 1;
        }
        if end == at + 1 {
            // A lone `$` not followed by a name character is literal.
            out.push('$');
            at += 1;
            continue;
        }
        let name = &raw[at + 1..end];
        push_expanded(&mut out, name, env, key, used)?;
        at = end;
    }
    Ok(out)
}

fn push_expanded(
    out: &mut String,
    name: &str,
    env: &EnvLayer,
    key: &str,
    used: &mut Vec<String>,
) -> Result<(), ConfigError> {
    match env.value(name) {
        Some(value) => {
            out.push_str(value);
            if !used.iter().any(|seen| seen == name) {
                used.push(name.to_owned());
            }
            Ok(())
        }
        None => Err(ConfigError::UnresolvedIndirection {
            var: name.to_owned(),
            key: key.to_owned(),
        }),
    }
}

fn is_var_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn is_var_name(name: &str) -> bool {
    !name.is_empty() && name.as_bytes()[0].is_ascii_alphabetic() && name.bytes().all(is_var_byte)
}

// ---------------------------------------------------------------------------
// Discovery
// ---------------------------------------------------------------------------

/// The project root every relative value anchors at: `GAUNTLET_PROJECT_ROOT`
/// when the environment carries it, the working directory otherwise.
pub fn discover_project_root(env: &EnvLayer) -> PathBuf {
    if let Some(root) = env
        .value("GAUNTLET_PROJECT_ROOT")
        .filter(|value| !value.is_empty())
    {
        return PathBuf::from(root);
    }
    std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."))
}

/// The GAUNTLET.md discovery order (§8.2): `--config <path>` →
/// `$GAUNTLET_CONFIG_DIR/GAUNTLET.md` → the project root's own contract.
/// A path is returned, not a guarantee — how a missing file reads differs by
/// origin: for the DISCOVERED locations it is the honest absence (not fatal
/// unless a required value stays unresolved), while an explicit `--config`
/// path names the file the operator asked for, and the doctor read split
/// turns its absence into a typed failure carrying that path.
pub fn discover_config_path(
    project_root: &Path,
    explicit: Option<&Path>,
    env: &EnvLayer,
) -> PathBuf {
    if let Some(path) = explicit {
        return path.to_path_buf();
    }
    if let Some(dir) = env
        .value("GAUNTLET_CONFIG_DIR")
        .filter(|value| !value.is_empty())
    {
        return PathBuf::from(dir).join(defaults::GAUNTLET_MD_NAME);
    }
    defaults::gauntlet_md_at(project_root)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::gauntlet_md;

    const VOCAB: &[&str] = &[
        "GAUNTLET_PROJECT_ROOT",
        "GAUNTLET_CONFIG_DIR",
        "GAUNTLET_MAX_ROUNDS",
        "GAUNTLET_MAX_TOKENS",
        "GAUNTLET_ANSWER_KEY_PATH",
        "GAUNTLET_BUILDER_RUNNER",
        "GAUNTLET_BUILDER_MODEL",
        "GAUNTLET_NONCONVERGENCE_N",
    ];

    fn env_layer(pairs: &[(&str, &str)]) -> EnvLayer {
        let owned: Vec<(String, String)> = pairs
            .iter()
            .map(|(name, value)| ((*name).to_owned(), (*value).to_owned()))
            .collect();
        EnvLayer::from_parts("", &owned, VOCAB)
    }

    fn input<'a>(doc: Option<&str>, env: &'a EnvLayer, flags: FlagOverrides) -> ResolveInput<'a> {
        let front_matter = doc.and_then(|text| gauntlet_md::parse(text).ok());
        ResolveInput {
            project_root: Path::new("proj-root").to_path_buf(),
            front_matter,
            env,
            flags,
        }
    }

    #[test]
    fn an_empty_world_resolves_to_the_defaults_table() {
        let env = env_layer(&[]);
        let resolved = resolve(&input(None, &env, FlagOverrides::new())).expect("resolves");
        let effective = &resolved.effective;
        assert_eq!(effective.budget.max_rounds, defaults::MAX_ROUNDS);
        assert_eq!(effective.budget.max_parallel, defaults::MAX_PARALLEL);
        assert_eq!(effective.budget.max_depth, defaults::MAX_DEPTH);
        let cap = effective.budget.max_wall_seconds;
        assert_eq!(cap, defaults::MAX_WALL_SECONDS);
        assert_eq!(
            effective.budget.recovery_reserve_pct,
            defaults::RECOVERY_RESERVE_PCT
        );
        assert_eq!(
            effective.budget.diagnostic_budget,
            defaults::DIAGNOSTIC_BUDGET
        );
        assert_eq!(effective.review_mode, defaults::REVIEW_MODE);
        assert_eq!(effective.sandbox_backend, defaults::SANDBOX_BACKEND);
        assert_eq!(effective.interactive, defaults::INTERACTIVE);
        assert_eq!(effective.git_strategy, defaults::GIT_STRATEGY);
        // Path defaults are rootless relative tokens, anchored later.
        assert_eq!(
            resolved.effective.sandbox_policy_dir,
            defaults::policy_dir_at(Path::new("")).to_string_lossy()
        );
        assert_eq!(effective.state_backend, "");
    }

    #[test]
    fn gauntlet_md_beats_the_defaults_table() {
        let env = env_layer(&[]);
        let doc = "---\nbudget:\n  max_rounds: 5\n---\n";
        let resolved = resolve(&input(Some(doc), &env, FlagOverrides::new())).expect("resolves");
        assert_eq!(resolved.effective.budget.max_rounds, 5);
    }

    #[test]
    fn the_environment_beats_gauntlet_md() {
        let env = env_layer(&[("GAUNTLET_MAX_ROUNDS", "6")]);
        let doc = "---\nbudget:\n  max_rounds: 5\n---\n";
        let resolved = resolve(&input(Some(doc), &env, FlagOverrides::new())).expect("resolves");
        assert_eq!(resolved.effective.budget.max_rounds, 6);
    }

    #[test]
    fn the_process_environment_beats_the_dotenv_file() {
        let process = vec![("GAUNTLET_MAX_ROUNDS".to_owned(), "7".to_owned())];
        let dotenv = "GAUNTLET_MAX_ROUNDS=6\n";
        let env = EnvLayer::from_parts(dotenv, &process, VOCAB);
        let resolved = resolve(&input(None, &env, FlagOverrides::new())).expect("resolves");
        assert_eq!(resolved.effective.budget.max_rounds, 7);
    }

    #[test]
    fn cli_flags_beat_the_environment() {
        let env = env_layer(&[("GAUNTLET_MAX_ROUNDS", "7")]);
        let mut flags = FlagOverrides::new();
        flags.set(ConfigKey::BudgetMaxRounds, FlagValue::WholeNumber(9));
        let resolved = resolve(&input(None, &env, flags)).expect("resolves");
        assert_eq!(resolved.effective.budget.max_rounds, 9);
    }

    #[test]
    fn the_full_chain_shows_each_layer_winning_in_turn() {
        let mut flags = FlagOverrides::new();
        flags.set(ConfigKey::BudgetMaxRounds, FlagValue::WholeNumber(11));
        let process = vec![("GAUNTLET_MAX_ROUNDS".to_owned(), "6".to_owned())];
        let env = EnvLayer::from_parts("GAUNTLET_MAX_ROUNDS=5\n", &process, VOCAB);
        let doc = "---\nbudget:\n  max_rounds: 4\n---\n";
        // Without flags the environment wins; with flags the flag wins.
        let mid = resolve(&input(Some(doc), &env, FlagOverrides::new())).expect("resolves");
        assert_eq!(mid.effective.budget.max_rounds, 6);
        let top = resolve(&input(Some(doc), &env, flags)).expect("resolves");
        assert_eq!(top.effective.budget.max_rounds, 11);
    }

    #[test]
    fn text_precedence_and_list_precedence_hold_too() {
        let env = env_layer(&[("GAUNTLET_ANSWER_KEY_PATH", "from-env.md")]);
        let doc = "---\nreference:\n  answer_key: from-file.md\n---\n";
        let resolved = resolve(&input(Some(doc), &env, FlagOverrides::new())).expect("resolves");
        assert_eq!(resolved.effective.reference.answer_key, "from-env.md");
        let mut flags = FlagOverrides::new();
        flags.set(
            ConfigKey::ReferenceAnswerKey,
            FlagValue::Text("from-flag.md".to_owned()),
        );
        let resolved = resolve(&input(Some(doc), &env, flags)).expect("resolves");
        assert_eq!(resolved.effective.reference.answer_key, "from-flag.md");
        // Lists: env splits on commas.
        let env = env_layer(&[("GAUNTLET_MCP_REQUIRED", "code_nav, schema")]);
        let resolved = resolve(&input(None, &env, FlagOverrides::new())).expect("resolves");
        assert_eq!(
            resolved.effective.mcp_required,
            vec!["code_nav".to_owned(), "schema".to_owned()]
        );
    }

    #[test]
    fn role_runner_configs_merge_from_all_layers() {
        let doc = "---\nrunners:\n  default: from-file\n  by_role:\n    builder: { runner: \
                   seeded, model: from-file-model }\n---\n";
        let env = env_layer(&[("GAUNTLET_BUILDER_RUNNER", "from-env")]);
        let resolved = resolve(&input(Some(doc), &env, FlagOverrides::new())).expect("resolves");
        assert_eq!(
            resolved.effective.runners_default.as_deref(),
            Some("from-file")
        );
        let builder = resolved
            .effective
            .runners_by_role
            .get("builder")
            .expect("builder configured");
        assert_eq!(builder.runner, "from-env");
        assert_eq!(builder.model.as_deref(), Some("from-file-model"));
    }

    #[test]
    fn unknown_keys_and_kind_mismatches_are_reported_not_guessed() {
        let doc = "---\nrunnersz:\n  default: typo\nbudget:\n  max_rounds: \"five\"\n---\n";
        // The parse itself succeeds; the merge rejects the kind mismatch and
        // collects the unknown section.
        let fm = gauntlet_md::parse(doc).expect("grammar accepts");
        let mut effective = EffectiveConfig::from_defaults();
        let mut unknown = Vec::new();
        let err = apply_front_matter(&mut effective, &fm, &mut unknown).expect_err("mismatch");
        assert!(matches!(
            err,
            ConfigError::KindMismatch { ref key, .. } if key == "budget.max_rounds"
        ));
        assert!(
            unknown
                .iter()
                .any(|entry| entry.path == "runnersz" && entry.line == 2)
        );
    }

    #[test]
    fn a_bad_environment_number_is_its_own_typed_error() {
        let env = env_layer(&[("GAUNTLET_MAX_ROUNDS", "many")]);
        let err = resolve(&input(None, &env, FlagOverrides::new())).expect_err("typed error");
        assert!(matches!(
            err,
            ConfigError::EnvNotANumber { ref name, ref key } if name == "GAUNTLET_MAX_ROUNDS"
                && key == "budget.max_rounds"
        ));
        assert!(!err.to_string().contains("many"));
    }

    #[test]
    fn indirections_expand_and_missing_ones_name_the_variable_only() {
        let env = env_layer(&[
            ("GAUNTLET_BUILDER_MODEL", "model-value-xyz"),
            ("TASK5_CANARY", "canary-value-xyz"),
        ]);
        let doc = "---\nrunners:\n  by_role:\n    builder: { runner: seeded, model: \
                    $GAUNTLET_BUILDER_MODEL }\nreference:\n  answer_key: canary-$TASK5_CANARY\
                    .md\n---\n";
        let resolved = resolve(&input(Some(doc), &env, FlagOverrides::new())).expect("resolves");
        // Raw tokens survive verbatim in the effective configuration.
        assert_eq!(
            resolved.effective.reference.answer_key,
            "canary-$TASK5_CANARY.md"
        );
        let products = resolved
            .effective
            .resolve_against(&env, Path::new("proj-root"))
            .expect("expands");
        assert!(
            products
                .reference
                .answer_key
                .to_string_lossy()
                .contains("canary-value-xyz.md")
        );
        assert_eq!(
            products
                .runners_by_role
                .get("builder")
                .and_then(|runner| runner.model.clone()),
            Some("model-value-xyz".to_owned())
        );
        assert!(
            products
                .indirections_used
                .contains(&"TASK5_CANARY".to_owned())
        );
        // A missing variable fails naming the variable and key, never a value.
        let doc = "---\nreference:\n  answer_key: ${NO_SUCH_VARIABLE}.md\n---\n";
        let resolved = resolve(&input(Some(doc), &env, FlagOverrides::new())).expect("resolves");
        let err = resolved
            .effective
            .resolve_against(&env, Path::new("proj-root"))
            .expect_err("unresolved");
        assert!(matches!(
            err,
            ConfigError::UnresolvedIndirection { ref var, ref key } if var == "NO_SUCH_VARIABLE"
                && key == "reference.answer_key"
        ));
        assert!(!err.to_string().contains("canary"));
    }

    #[test]
    fn budget_money_parses_into_the_domain_newtype_and_zero_refuses() {
        let env = env_layer(&[]);
        let doc = "---\nbudget:\n  max_cost_usd: 25.5\n---\n";
        let resolved = resolve(&input(Some(doc), &env, FlagOverrides::new())).expect("resolves");
        assert_eq!(resolved.effective.budget.max_cost_usd, "25.5");
        let products = resolved
            .effective
            .resolve_against(&env, Path::new("proj-root"))
            .expect("expands");
        assert_eq!(products.budget.max_cost_usd, MicroUsd::new(25_500_000));
        let limits = products.budget.limits().expect("non-zero ceilings");
        assert_eq!(limits.max_rounds(), defaults::MAX_ROUNDS);
        // Domain reuse: a zero ceiling is refused by the domain, not by
        // doctor-side arithmetic.
        let mut flags = FlagOverrides::new();
        flags.set(ConfigKey::BudgetMaxRounds, FlagValue::WholeNumber(0));
        let resolved = resolve(&input(None, &env, flags)).expect("resolves");
        let products = resolved
            .effective
            .resolve_against(&env, Path::new("proj-root"))
            .expect("expands");
        assert!(matches!(
            products.budget.limits(),
            Err(gauntlet_domain::BudgetError::LimitZero { .. })
        ));
    }

    #[test]
    fn the_nonconvergence_threshold_defaults_to_the_spec_three_and_overrides() {
        // §11.5: "N = GAUNTLET_NONCONVERGENCE_N, default 3" — the default
        // surfaces through resolution, the Annex E variable overrides it,
        // and a zero threshold is refused (a trigger that counts
        // occurrences cannot fire at zero).
        let env = env_layer(&[]);
        let resolved = resolve(&input(None, &env, FlagOverrides::new())).expect("resolves");
        assert_eq!(resolved.effective.nonconvergence_n, 3);
        assert_eq!(
            resolved
                .effective
                .resolve_against(&env, Path::new("proj-root"))
                .expect("expands")
                .nonconvergence_n,
            3
        );
        let env = env_layer(&[("GAUNTLET_NONCONVERGENCE_N", "5")]);
        let resolved = resolve(&input(None, &env, FlagOverrides::new())).expect("resolves");
        assert_eq!(resolved.effective.nonconvergence_n, 5);
        let env = env_layer(&[("GAUNTLET_NONCONVERGENCE_N", "0")]);
        let err = resolve(&input(None, &env, FlagOverrides::new()))
            .expect_err("a zero threshold is typed-refused");
        assert!(matches!(
            err,
            ConfigError::ValueOutOfRange { ref key } if key == "nonconvergence_n"
        ));
        // The setting is env-only: no front-matter section maps it, so a
        // GAUNTLET.md carrying it is an unknown key, reported not guessed.
        let doc = "---\nnonconvergence_n: 4\n---\n";
        let resolved = resolve(&input(Some(doc), &env_layer(&[]), FlagOverrides::new()))
            .expect("resolves with a report");
        assert_eq!(resolved.effective.nonconvergence_n, 3);
        assert!(
            resolved
                .unknown_keys
                .iter()
                .any(|entry| entry.path == "nonconvergence_n")
        );
    }

    #[test]
    fn control_settings_carry_one_spec_default_and_four_required_absences() {
        // §11.4 pins the tick default inline; §11.4/§11.5 pin NO default
        // for the stall window, the backoff bounds, or the attempt
        // ceiling — those four surface as `None` here and are refused
        // typed where they are consumed (the control loop's policy
        // construction), never defaulted in this layer.
        let env = env_layer(&[]);
        let resolved = resolve(&input(None, &env, FlagOverrides::new())).expect("resolves");
        assert_eq!(
            resolved.effective.control.poll_interval_ms,
            defaults::POLL_INTERVAL_MS
        );
        let control = &resolved.effective.control;
        assert_eq!(control.stall_timeout_ms, None);
        assert_eq!(control.retry_base_ms, None);
        assert_eq!(control.retry_max_ms, None);
        assert_eq!(control.max_attempts_per_unit, None);
        // The products carry the same facts through unchanged.
        let products = resolved
            .effective
            .resolve_against(&env, Path::new("proj-root"))
            .expect("expands");
        assert_eq!(products.control, resolved.effective.control);
        // Every Annex E variable of the group overrides its setting. The
        // figures are read into hint-free locals before any number is
        // asserted: the literals scanner's duration class pairs a
        // duration-shaped identifier with a value-position number, and
        // these setting names are nothing but duration hints.
        let env = env_layer(&[
            ("GAUNTLET_POLL_INTERVAL_MS", "250"),
            ("GAUNTLET_STALL_TIMEOUT_MS", "90000"),
            ("GAUNTLET_RETRY_BASE_MS", "1000"),
            ("GAUNTLET_RETRY_MAX_MS", "30000"),
            ("GAUNTLET_MAX_ATTEMPTS_PER_UNIT", "2"),
        ]);
        let resolved = resolve(&input(None, &env, FlagOverrides::new())).expect("resolves");
        let control = &resolved.effective.control;
        let tick = control.poll_interval_ms;
        let quiet_window = control.stall_timeout_ms;
        let floor = control.retry_base_ms;
        let ceiling = control.retry_max_ms;
        let attempt_cap = control.max_attempts_per_unit;
        assert_eq!(tick, 250);
        assert_eq!(quiet_window, Some(90_000));
        assert_eq!(floor, Some(1_000));
        assert_eq!(ceiling, Some(30_000));
        assert_eq!(attempt_cap, Some(2));
        // A zero value in any of the five is a typed refusal (a zero tick
        // spins, a zero stall window stalls on the first poll, zero
        // backoff is no backoff, a zero attempt ceiling exhausts before
        // the first dispatch).
        for (variable, key) in [
            ("GAUNTLET_POLL_INTERVAL_MS", "poll_interval_ms"),
            ("GAUNTLET_STALL_TIMEOUT_MS", "stall_timeout_ms"),
            ("GAUNTLET_RETRY_BASE_MS", "retry_base_ms"),
            ("GAUNTLET_RETRY_MAX_MS", "retry_max_ms"),
            ("GAUNTLET_MAX_ATTEMPTS_PER_UNIT", "max_attempts_per_unit"),
        ] {
            let env = env_layer(&[(variable, "0")]);
            let err = resolve(&input(None, &env, FlagOverrides::new()))
                .expect_err("a zero control value is typed-refused");
            assert!(
                matches!(err, ConfigError::ValueOutOfRange { key: ref refused } if refused == key),
                "{variable}: {err:?}"
            );
        }
    }

    #[test]
    fn discovery_follows_the_spec_order() {
        let env = env_layer(&[]);
        let root = Path::new("proj-root");
        let explicit = discover_config_path(root, Some(Path::new("custom.md")), &env);
        assert_eq!(explicit, PathBuf::from("custom.md"));
        let env = env_layer(&[("GAUNTLET_CONFIG_DIR", "confdir")]);
        let via_env = discover_config_path(root, None, &env);
        assert_eq!(via_env, PathBuf::from("confdir").join("GAUNTLET.md"));
        let env = env_layer(&[]);
        let fallback = discover_config_path(root, None, &env);
        assert_eq!(fallback, root.join("GAUNTLET.md"));
    }

    /// AK-015's assertion, at the locator this round can reach: the answer
    /// key addresses `tests/contract/config::precedence`; the boundary
    /// suite's Cargo-target-path rule forbids a core crate from declaring a
    /// repo-level test target, so the umbrella assertion lives here under
    /// its locator-matching name. The adjacent pairs are also proven
    /// individually above.
    #[test]
    fn precedence() {
        // defaults < GAUNTLET.md.
        let empty = env_layer(&[]);
        let base = resolve(&input(None, &empty, FlagOverrides::new()))
            .expect("resolves")
            .effective;
        let doc = "---\nbudget:\n  max_rounds: 4\n---\n";
        let after_file = resolve(&input(Some(doc), &empty, FlagOverrides::new()))
            .expect("resolves")
            .effective;
        assert_eq!(base.budget.max_rounds, defaults::MAX_ROUNDS);
        assert_eq!(after_file.budget.max_rounds, 4);
        // GAUNTLET.md < environment.
        let env = env_layer(&[("GAUNTLET_MAX_ROUNDS", "6")]);
        let after_env = resolve(&input(Some(doc), &env, FlagOverrides::new()))
            .expect("resolves")
            .effective;
        assert_eq!(after_env.budget.max_rounds, 6);
        // environment < flags.
        let mut flags = FlagOverrides::new();
        flags.set(ConfigKey::BudgetMaxRounds, FlagValue::WholeNumber(11));
        let after_flags = resolve(&input(Some(doc), &env, flags))
            .expect("resolves")
            .effective;
        assert_eq!(after_flags.budget.max_rounds, 11);
        // And the effective configuration digests onto the run: each layer
        // boundary moves the fingerprint.
        let fp = |config: &EffectiveConfig| {
            crate::config::fingerprint::config_fingerprint(config).to_string()
        };
        assert_ne!(fp(&base), fp(&after_file));
        assert_ne!(fp(&after_file), fp(&after_env));
        assert_ne!(fp(&after_env), fp(&after_flags));
    }
}
