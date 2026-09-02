//! The run composition (plan Task 20, brief Item 2): builds the ONE
//! `ControllerDeps` literal over REAL adapters from a resolved
//! [RuntimeContext](crate::context::RuntimeContext) and hands the
//! composed [`RunController`] to `gauntlet run` / `gauntlet resume`.
//! The CLI wires; it does not decide (the plan's redesign trigger) —
//! every decision recorded here is a composition FACT (which adapter
//! fronts which capability), never a policy.
//!
//! Composition decisions this module OWNS (each recorded in the task
//! report):
//!
//! - **Runner selection (charter D2, fail-closed):** `runners.default`
//!   (`GAUNTLET.md` `runners.default` / env `GAUNTLET_AGENT_RUNNER`)
//!   selects among exactly THREE fronted tokens — `prime-agent`,
//!   `cli-agent`, `mock-agent`. Unset or unknown is a typed
//!   configuration failure naming the mechanism; nothing dispatches.
//! - **Mock scenario seam:** the mock token requires the
//!   `GAUNTLET_MOCK_SCENARIO` hook (a scenario fixture stem). Fail-closed:
//!   a mock run without a scenario is a typed configuration error. The
//!   hook stays OUT of `.env.example`/Annex E lockstep on purpose (the
//!   AK-019 instrument refuses undocumented variables; this is a
//!   test-division hook, not operator configuration).
//! - **Wrap caller (charter D3, R-3):** for the process-backed
//!   `cli-agent` token the composition calls
//!   `AuthorityGate::admit` + `AuthorityGate::wrap_builder_command`
//!   before any run starts, over the FIRST planned dispatch's real
//!   facts. Under the shipped `none` backend posture the verdict is
//!   `BuilderCommand::Unwrapped` and the run proceeds; a LIVE backend
//!   verdict (`BuilderCommand::Sandboxed`) refuses the run HERE —
//!   fail-closed — because routing a wrapped command through the
//!   controller's runner seam is plan Task 25's composed E2E leg.
//!   Never a silent unwrap (§21.3, AK-047).
//! - **Deny-set wiring (R2-N1):** the critic role's typed
//!   `context.deny` set is loaded from the installed roles and threaded
//!   through `ComparisonDeps.role_deny` into the governed comparison
//!   chain, which enforces it before the envelope is built.
//! - **Diagnostic allowance mapping (COR-D9):** the config scalar
//!   `diagnostic_budget` (default 2, unitless in the §8.6 table) bounds
//!   the number of §15.2 diagnostic cycles; the domain allowance is that
//!   many per-attempt shares of the run's own forward ceilings. The
//!   mapping itself is a POLICY shape and lives in the application layer
//!   (`ResolvedBudget::diagnostic_allowance`) — the CLI passes the
//!   config scalar through, it does not decide.
//! - **Escalation + ids + workspaces:** the run composition fronts the
//!   fs escalation queue (under the state dir), the production ULID id
//!   generator (or the Annex E `GAUNTLET_ID_SEED` hook), and B1's fs
//!   workspace provider rooted under the state dir.
//! - **Annex E clock hook (live-E2E correction):** the composition
//!   consults `GAUNTLET_CLOCK` through the mock crate's documented
//!   selector — unset keeps the wall clock (behavior-identical);
//!   `deterministic` selects the virtual clock; any other token is a
//!   typed refusal, never a silent fallback.
//! - **Produce policy (live-E2E correction):** this composition fronts
//!   the produced-artifact §14.2 rungs (digest manifest, command
//!   transcript, test report), so it composes the controller with
//!   `compose_binding_produced_artifacts` — the evidence owner binds the
//!   locator's ACTUAL content (ingest a produced artifact; materialize
//!   the observation facet only at absence; never overwrite). The
//!   default controller compose keeps the facet-materialization world
//!   the frozen facet-document proof rungs decide through.

use std::sync::Arc;

use gauntlet_adapter_agent_cli as generic_cli_runner;
use gauntlet_adapter_agent_mock as mock_runner;
use gauntlet_adapter_agent_prime as recursive_runner;
use gauntlet_adapter_compare_diff::DiffComparator;
use gauntlet_adapter_compare_model::{ModelComparator, ModelComparatorSource};
use gauntlet_adapter_escalation_fs::{ESCALATIONS_DIR_NAME, FsEscalationQueue};
use gauntlet_adapter_evidence_fs::FsEvidenceStore;
use gauntlet_adapter_id_entropy::UlidIdGenerator;
use gauntlet_adapter_meter_memory::MeteredBudget;
use gauntlet_adapter_observe_build::BuildObserver;
use gauntlet_adapter_observe_filesystem::FilesystemObserver;
use gauntlet_adapter_observe_git::GitObserver;
use gauntlet_adapter_observe_test::{TestCommand, TestObserver};
use gauntlet_adapter_policy_fs::PolicyFs;
use gauntlet_adapter_sandbox_bwrap::ProcessIsolationBackend;
use gauntlet_adapter_state_sqlite::EmbeddedStateStore;
use gauntlet_adapter_verify_command_exit_code::CommandExitVerifier;
use gauntlet_adapter_verify_file_digest::FileDigestVerifier;
use gauntlet_adapter_verify_test_report::TestReportVerifier;
use gauntlet_adapter_workspace_fs::FsWorkspaceProvider;
use gauntlet_app::config::defaults;
use gauntlet_app::control::controller::{
    BudgetShape, ComparisonDeps, ControllerDeps, MeterFactory, RunController,
};
use gauntlet_app::control::disposition::ControlPolicy;
use gauntlet_app::govern::GovernDeps;
use gauntlet_app::roles::ContextDenySet;
use gauntlet_ports::agent_runner::AgentRunner;
use gauntlet_ports::clock::Clock;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::id_generator::IdGenerator;
use gauntlet_ports::observer::Observer;

use crate::context::RuntimeContext;

/// The state store file name under the state dir (the composition's one
/// choice; single-segment by intent).
const STATE_DB_NAME: &str = "gauntlet-state.db";

/// The mock token's scenario hook (see the module docs).
pub const MOCK_SCENARIO_VAR: &str = "GAUNTLET_MOCK_SCENARIO";

/// Why a run could not be composed — typed, naming the mechanism
/// (spec §21.2; the failure classes configuration/reference/environment).
#[derive(Debug)]
pub enum CompositionError {
    /// A required configuration is absent or unknown. Carries the config
    /// key AND the environment variable that could set it — the
    /// mechanism, not just the fact (charter D2).
    Configuration {
        key: &'static str,
        env: &'static str,
        detail: String,
    },
    /// An Annex E test hook (spec §8.3) carried a value outside its
    /// vocabulary. Never a silent fallback — a hook that changes nothing
    /// is the fail-open shape the hook selectors refuse (the live-E2E
    /// correction: the composition consulted no hook at all, so a typo'd
    /// `GAUNTLET_CLOCK` looked like a wall-clock run).
    EnvHook { hook: &'static str, detail: String },
    /// The resolved sandbox backend token names no backend this
    /// composition fronts.
    UnknownBackend { token: String },
    /// The runner could not be constructed from its configuration.
    RunnerConstruction { detail: String },
    /// The comparison contract artifact could not be read or parsed.
    ComparisonContract { detail: String },
    /// The roles could not be loaded (the deny set is composition-
    /// required when the roles tree is served).
    Roles { detail: String },
    /// The state/evidence/escalation roots could not be opened.
    Store { detail: String },
    /// The plan preview refused (the ruler's unit graph is invalid).
    Plan { detail: String },
    /// A LIVE backend wrap verdict refuses the run (charter D3): the
    /// fully-composed live-containment leg is plan Task 25's E2E. The
    /// command is never run unwrapped instead.
    LiveContainmentDeferred { unit: String },
    /// The composition-time wrap was refused by the gate.
    WrapRefused { detail: String },
}

impl core::fmt::Display for CompositionError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Configuration { key, env, detail } => {
                write!(f, "configuration failure: {key} ({env}): {detail}")
            }
            Self::EnvHook { hook, detail } => {
                write!(f, "test hook failure: {hook}: {detail}")
            }
            Self::UnknownBackend { token } => write!(
                f,
                "configuration failure: sandbox.backend token '{token}' names no backend this \
                 composition fronts"
            ),
            Self::RunnerConstruction { detail } => {
                write!(f, "the runner could not be constructed: {detail}")
            }
            Self::ComparisonContract { detail } => {
                write!(f, "the comparison contract is unusable: {detail}")
            }
            Self::Roles { detail } => write!(f, "the roles could not be loaded: {detail}"),
            Self::Store { detail } => write!(f, "a durable store could not be opened: {detail}"),
            Self::Plan { detail } => write!(f, "the plan preview refused: {detail}"),
            Self::LiveContainmentDeferred { unit } => write!(
                f,
                "unit {unit} needs a live-sandbox wrap verdict; routing a wrapped builder \
                 command through the composed loop is plan Task 25's leg — refusing here \
                 rather than running unwrapped (spec 21.3, AK-047)"
            ),
            Self::WrapRefused { detail } => {
                write!(f, "the composition-time wrap was refused: {detail}")
            }
        }
    }
}

impl std::error::Error for CompositionError {}

/// The runner token resolved from `runners.default` (charter D2):
/// unset is a typed configuration failure naming both the config key and
/// the environment variable — the repository's own GAUNTLET.md stays
/// unset on purpose (UNK-003 is the operator's decision).
pub fn select_runner_token(context: &RuntimeContext) -> Result<String, CompositionError> {
    let env_token = std::env::var("GAUNTLET_AGENT_RUNNER")
        .ok()
        .filter(|v| !v.is_empty());
    let token = context
        .resolved
        .runners_default
        .clone()
        .or(env_token)
        .ok_or_else(|| CompositionError::Configuration {
            key: "runners.default",
            env: "GAUNTLET_AGENT_RUNNER",
            detail: "no default runner is configured — set runners.default in GAUNTLET.md or \
                     GAUNTLET_AGENT_RUNNER in the environment; dispatch refuses without one"
                .to_owned(),
        })?;
    let fronted = [
        recursive_runner::RUNNER_TOKEN,
        generic_cli_runner::RUNNER_TOKEN,
        mock_runner::RUNNER_TOKEN,
    ];
    if fronted.contains(&token.as_str()) {
        Ok(token)
    } else {
        Err(CompositionError::Configuration {
            key: "runners.default",
            env: "GAUNTLET_AGENT_RUNNER",
            detail: format!(
                "token '{token}' names no runner this composition fronts (fronted tokens: {})",
                fronted.join(", ")
            ),
        })
    }
}

/// Which runner token a runner was built from (for the identity record).
#[allow(clippy::arc_with_non_send_sync)] // the loop is single-threaded; the adapters carry interior mutability (the p6 fixture precedent, now in production)
fn runner_for(
    token: &str,
    context: &RuntimeContext,
    clock: &Arc<dyn Clock>,
) -> Result<Arc<dyn AgentRunner>, CompositionError> {
    match token {
        recursive_runner::RUNNER_TOKEN => {
            let config = recursive_runner::PrimeAdapterConfig::default();
            let state_dir = context.state_dir().to_path_buf();
            let transport = recursive_runner::ProcessKernelTransport::new(
                config.binary.clone(),
                config.admission_timeout(),
                config.admission_linger(),
            );
            let control = recursive_runner::ProcessHarnessControl::new(config.binary.clone());
            Ok(Arc::new(recursive_runner::PrimeAgentRunner::new(
                config,
                state_dir,
                context.project_root.clone(),
                transport,
                control,
                clock.clone(),
            )))
        }
        generic_cli_runner::RUNNER_TOKEN => {
            let config = generic_cli_runner::CliAgentConfig::from_env().map_err(|error| {
                CompositionError::Configuration {
                    key: "runners.default",
                    env: generic_cli_runner::COMMAND_ENV_VAR,
                    detail: error.to_string(),
                }
            })?;
            let runner = generic_cli_runner::CliAgentRunner::new(
                config,
                context.state_dir().to_path_buf(),
                context.state_dir().join(defaults::WORKSPACES_DIR_NAME),
                generic_cli_runner::ProcessChildTransport::new(),
                clock.clone(),
            )
            .map_err(|error| CompositionError::RunnerConstruction {
                detail: error.to_string(),
            })?;
            Ok(Arc::new(runner))
        }
        mock_runner::RUNNER_TOKEN => {
            let stem = std::env::var(MOCK_SCENARIO_VAR)
                .ok()
                .filter(|v| !v.is_empty());
            let Some(stem) = stem else {
                return Err(CompositionError::Configuration {
                    key: "runners.default",
                    env: MOCK_SCENARIO_VAR,
                    detail: format!(
                        "the mock runner replays a named scenario fixture; set \
                         {MOCK_SCENARIO_VAR} to a scenario stem (e.g. happy) or select a real \
                         runner token"
                    ),
                });
            };
            let scenario = mock_runner::load_scenario(&stem).map_err(|error| {
                CompositionError::Configuration {
                    key: "runners.default",
                    env: MOCK_SCENARIO_VAR,
                    detail: error.to_string(),
                }
            })?;
            Ok(Arc::new(mock_runner::MockAgentRunner::new(
                scenario,
                context.state_dir().join(defaults::WORKSPACES_DIR_NAME),
            )))
        }
        other => Err(CompositionError::Configuration {
            key: "runners.default",
            env: "GAUNTLET_AGENT_RUNNER",
            detail: format!("token '{other}' names no runner this composition fronts"),
        }),
    }
}

/// The observation plan: maps the configured `observation.facets` tokens
/// to the real observer adapters this composition fronts. `test` and
/// `build` require their tool bindings (`GAUNTLET_TOOL_TEST` /
/// `GAUNTLET_TOOL_BUILD`); an unknown token is a typed configuration
/// failure — the composition fronts no instrument it cannot name.
#[allow(clippy::arc_with_non_send_sync)] // single-threaded loop; observers share the store/clock Arcs
fn observers(
    context: &RuntimeContext,
    clock: &Arc<dyn Clock>,
    evidence: &Arc<FsEvidenceStore>,
) -> Result<Vec<Arc<dyn Observer>>, CompositionError> {
    let workspace_root = context.state_dir().join(defaults::WORKSPACES_DIR_NAME);
    let mut observers: Vec<Arc<dyn Observer>> = Vec::new();
    for facet in &context.resolved.observation_facets {
        match facet.as_str() {
            "test" => {
                let binding = std::env::var("GAUNTLET_TOOL_TEST")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| CompositionError::Configuration {
                        key: "observation.facets",
                        env: "GAUNTLET_TOOL_TEST",
                        detail: "the test facet observes a declared test command; set the \
                                 binding or drop the facet"
                            .to_owned(),
                    })?;
                let mut tokens = binding.split_whitespace();
                let Some(program) = tokens.next() else {
                    return Err(CompositionError::Configuration {
                        key: "observation.facets",
                        env: "GAUNTLET_TOOL_TEST",
                        detail: "the binding is empty".to_owned(),
                    });
                };
                observers.push(Arc::new(TestObserver::from_parts(
                    TestCommand::new(program, tokens.map(str::to_owned).collect()),
                    workspace_root.clone(),
                    clock.clone(),
                    evidence.clone(),
                    &host_pairs(),
                )));
            }
            "build" => {
                let binding = std::env::var("GAUNTLET_TOOL_BUILD")
                    .ok()
                    .filter(|value| !value.trim().is_empty())
                    .ok_or_else(|| CompositionError::Configuration {
                        key: "observation.facets",
                        env: "GAUNTLET_TOOL_BUILD",
                        detail: "the build facet observes a declared build command; set the \
                                 binding or drop the facet"
                            .to_owned(),
                    })?;
                let mut tokens = binding.split_whitespace();
                let Some(program) = tokens.next() else {
                    return Err(CompositionError::Configuration {
                        key: "observation.facets",
                        env: "GAUNTLET_TOOL_BUILD",
                        detail: "the binding is empty".to_owned(),
                    });
                };
                observers.push(Arc::new(BuildObserver::from_parts(
                    gauntlet_adapter_observe_build::BuildCommand::new(
                        program,
                        tokens.map(str::to_owned).collect(),
                    ),
                    workspace_root.clone(),
                    clock.clone(),
                    evidence.clone(),
                    &host_pairs(),
                )));
            }
            "filesystem" => observers.push(Arc::new(FilesystemObserver::from_parts(
                workspace_root.clone(),
                clock.clone(),
                evidence.clone(),
                &host_pairs(),
            ))),
            "versioning" => observers.push(Arc::new(GitObserver::from_parts(
                workspace_root.clone(),
                clock.clone(),
                evidence.clone(),
                &host_pairs(),
            ))),
            other => {
                return Err(CompositionError::Configuration {
                    key: "observation.facets",
                    env: "GAUNTLET_OBSERVER_RUNNER",
                    detail: format!(
                        "facet '{other}' names no observer this composition fronts (fronted: \
                         test, build, filesystem, versioning)"
                    ),
                });
            }
        }
    }
    if observers.is_empty() {
        return Err(CompositionError::Configuration {
            key: "observation.facets",
            env: "GAUNTLET_OBSERVER_RUNNER",
            detail: "no observation facet is configured — an observation-less run assembles an \
                     all-healthy attestation over zero probes and is refused; name at least \
                     one facet (test, build, filesystem, versioning)"
                .to_owned(),
        });
    }
    Ok(observers)
}

/// The write-inventory's guard needle (crates/gauntlet-arch-tests
/// WRITE_API_INVENTORY): every directory the composition ensures is a
/// RESOLVED CONFIGURATION path (state dir / evidence dir from the
/// §8.2 merge) passed through here — the composition never invents a
/// write path.
fn ensure_configured_dir(path: &std::path::Path) -> Result<(), CompositionError> {
    std::fs::create_dir_all(path).map_err(|error| CompositionError::Store {
        detail: format!("{}: {error}", path.display()),
    })
}

fn host_pairs() -> Vec<(String, String)> {
    std::env::vars_os()
        .map(|(key, value)| {
            (
                key.to_string_lossy().into_owned(),
                value.to_string_lossy().into_owned(),
            )
        })
        .collect()
}

/// The §17 governance posture (charter D3/R17-F4 context): the resolved
/// `sandbox.backend` token selects the conservative none posture or the
/// live process-isolation posture (whose policy provider resolves tier
/// policies from the configured policy dir). Any other token is a typed
/// configuration failure.
fn govern_posture(context: &RuntimeContext) -> Result<GovernDeps, CompositionError> {
    use gauntlet_ports::sandbox::Sandbox;
    let token = context.resolved.sandbox_backend.clone();
    if token == defaults::SANDBOX_BACKEND {
        return Ok(GovernDeps::none_backend());
    }
    let binary =
        gauntlet_adapter_sandbox_bwrap::locate().ok_or_else(|| CompositionError::Store {
            detail: format!(
                "the '{token}' backend's process-isolation binary could not be located"
            ),
        })?;
    let backend = ProcessIsolationBackend::new(binary);
    if backend.backend_identity().as_str() != token {
        return Err(CompositionError::UnknownBackend { token });
    }
    // R17-F4: the runtime-provided result-contract directory must be
    // writable when a LIVE backend wraps — the policy provider is
    // composed BEHIND the contract-dir wrapper so every tier policy it
    // resolves carries the state dir's invocation tree as a writable
    // root (the runner anchors its result contracts there).
    let policies = ContractDirPolicy::from_parts(
        PolicyFs::new(
            context.resolved.sandbox_policy_dir.clone(),
            gauntlet_adapter_policy_fs::AnchorRoots {
                workspace_root: context.state_dir().join(defaults::WORKSPACES_DIR_NAME),
                project_root: context.project_root.clone(),
                home: home_dir(),
            },
        ),
        context.state_dir().join(RUN_IO_DIR_NAME),
    );
    GovernDeps::with_backend(
        &token,
        Some(Arc::new(backend)),
        Some(Arc::new(policies)),
        gauntlet_domain::Authority::Critic,
    )
    .map_err(|error| CompositionError::Store {
        detail: error.to_string(),
    })
}

/// The state dir's per-run invocation-IO directory name — where the
/// process-backed runner anchors its result contracts (the adapter's own
/// `runs/<run_id>/inv/<invocation_id>/` layout).
pub const RUN_IO_DIR_NAME: &str = "runs";

/// The R17-F4 policy wrapper: resolves through the REAL policy provider,
/// then adds the runtime-provided result-contract directory to the
/// policy's writable roots (idempotently). Composition-level glue — the
/// runtime owns the contract dir, so the runtime's composition owns the
/// extension.
struct ContractDirPolicy {
    inner: PolicyFs,
    contract_dir: std::path::PathBuf,
}

impl ContractDirPolicy {
    fn from_parts(inner: PolicyFs, contract_dir: std::path::PathBuf) -> Self {
        Self {
            inner,
            contract_dir,
        }
    }
}

impl gauntlet_ports::policy_provider::PolicyProvider for ContractDirPolicy {
    fn policy_for(
        &self,
        tier: gauntlet_domain::BlastRadius,
        workspace: &gauntlet_domain::WorkspaceRef,
        deadline: Deadline,
    ) -> Result<
        gauntlet_ports::sandbox::SandboxPolicy,
        gauntlet_ports::policy_provider::PolicyProviderError,
    > {
        let mut policy = self.inner.policy_for(tier, workspace, deadline)?;
        // The runtime PROVIDES the result-contract directory (R17-F4):
        // materialize it before claiming it as a writable root — a bind
        // source the runtime never created is a sandbox startup refusal,
        // and a promise the runtime does not keep is not containment.
        // Failure escalates through the provider's typed error (never a
        // silent widening or a silent shrink of the policy).
        if let Err(error) = std::fs::create_dir_all(&self.contract_dir) {
            return Err(
                gauntlet_ports::policy_provider::PolicyProviderError::Unavailable {
                    detail: format!(
                        "the runtime could not materialize the result-contract directory \
                         {}: {error}",
                        self.contract_dir.display()
                    ),
                },
            );
        }
        let entry = self.contract_dir.to_string_lossy().into_owned();
        if !policy.writable_roots.contains(&entry) {
            policy.writable_roots.push(entry);
        }
        Ok(policy)
    }

    /// The permissions half delegates verbatim: the contract-dir
    /// extension is a CONTAINMENT fact, not a permission grant (the
    /// inner provider's own recorded out-of-scope refusal stands).
    fn permissions_for(
        &self,
        role: &gauntlet_domain::RoleId,
        unit: &gauntlet_domain::UnitId,
        deadline: Deadline,
    ) -> Result<
        gauntlet_ports::policy_provider::PermissionSet,
        gauntlet_ports::policy_provider::PolicyProviderError,
    > {
        self.inner.permissions_for(role, unit, deadline)
    }
}

/// The operator's home directory (the `$HOME` anchor root), read from
/// the process environment; unset is an empty path — the policy's own
/// credential denials then anchor at nothing, which the policy provider's
/// grammar refuses rather than widening.
fn home_dir() -> std::path::PathBuf {
    std::env::var_os("HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_default()
}

/// The configured review mode, parsed typed (an unknown token is a
/// configuration failure, never a silent adaptive fallback).
fn review_mode(context: &RuntimeContext) -> Result<gauntlet_domain::ReviewMode, CompositionError> {
    gauntlet_domain::ReviewMode::parse(&context.resolved.review_mode).map_err(|_| {
        CompositionError::Configuration {
            key: "review.mode",
            env: "GAUNTLET_REVIEW_MODE",
            detail: format!(
                "token '{}' is outside the §5.2 vocabulary (single, fanout, adaptive, \
                 high_assurance)",
                context.resolved.review_mode
            ),
        }
    })
}

/// Loads the critic role's typed `context.deny` set from the roles dir
/// (R2-N1). The comparison chain enforces it before any envelope is
/// built; a roles tree that does not serve the critic role is a typed
/// configuration failure.
pub fn critic_deny_set(context: &RuntimeContext) -> Result<ContextDenySet, CompositionError> {
    let path = context.roles_dir().join("critic.md");
    let text = std::fs::read_to_string(&path).map_err(|error| CompositionError::Roles {
        detail: format!("reading {}: {error}", path.display()),
    })?;
    let definition = gauntlet_app::roles::RoleDefinition::load(
        "critic",
        &text,
        &context.env,
        crate::spec_vocab::CAPABILITY_VOCABULARY,
    )
    .map_err(|error| CompositionError::Roles {
        detail: error.to_string(),
    })?;
    Ok(definition.context_deny_set().clone())
}

/// The §14.2 rung set this composition fronts (module docs): the three
/// deterministic rungs that need NO project-specific contract, each
/// bound to the canonical workspace root. Shared by the run composition
/// and `gauntlet verify`.
pub fn composed_ladder(
    workspace_root: &std::path::Path,
) -> Result<gauntlet_app::verify::ladder::Ladder, CompositionError> {
    let ladder = gauntlet_app::verify::ladder::Ladder::new(vec![
        gauntlet_app::verify::ladder::LadderRung::new(
            gauntlet_app::verify::ladder::RungKind::FileDigest,
            Arc::new(
                FileDigestVerifier::new(workspace_root.to_path_buf()).map_err(|error| {
                    CompositionError::Store {
                        detail: error.to_string(),
                    }
                })?,
            ),
        ),
        gauntlet_app::verify::ladder::LadderRung::new(
            gauntlet_app::verify::ladder::RungKind::CommandExitCode,
            Arc::new(
                CommandExitVerifier::new(workspace_root.to_path_buf()).map_err(|error| {
                    CompositionError::Store {
                        detail: error.to_string(),
                    }
                })?,
            ),
        ),
        gauntlet_app::verify::ladder::LadderRung::new(
            gauntlet_app::verify::ladder::RungKind::TestReport,
            Arc::new(
                TestReportVerifier::new(workspace_root.to_path_buf()).map_err(|error| {
                    CompositionError::Store {
                        detail: error.to_string(),
                    }
                })?,
            ),
        ),
    ])
    .map_err(|error| CompositionError::Store {
        detail: error.to_string(),
    })?;
    Ok(ladder)
}

/// The read-side surface the operator commands share (`status`,
/// `inspect`, `verify`, `escalations`, `export`): the SAME adapters the
/// run composition wires, opened for reads. The state database is NOT
/// created here — a missing state file is a typed refusal, never a
/// surprise database.
pub struct OperatorSurface {
    pub state: Arc<dyn gauntlet_ports::state_store::StateStore>,
    pub workspace_root: std::path::PathBuf,
    pub escalation_root: std::path::PathBuf,
    pub answer_key: std::path::PathBuf,
    pub manifest: std::path::PathBuf,
}

impl Clone for OperatorSurface {
    fn clone(&self) -> Self {
        Self {
            state: self.state.clone(),
            workspace_root: self.workspace_root.clone(),
            escalation_root: self.escalation_root.clone(),
            answer_key: self.answer_key.clone(),
            manifest: self.manifest.clone(),
        }
    }
}

impl OperatorSurface {
    /// Why a read command refuses: the state database does not exist.
    pub fn state_exists(state_dir: &std::path::Path) -> bool {
        state_dir.join(STATE_DB_NAME).is_file()
    }
}

/// Opens the read-side operator surface; refuses (typed) when the run
/// state was never created.
pub fn open_operator_surface(
    context: &RuntimeContext,
) -> Result<OperatorSurface, CompositionError> {
    let state_dir = context.state_dir().to_path_buf();
    if !OperatorSurface::state_exists(&state_dir) {
        return Err(CompositionError::Store {
            detail: format!(
                "no run state at {} — no run has been composed here yet",
                state_dir.join(STATE_DB_NAME).display()
            ),
        });
    }
    let state = EmbeddedStateStore::open(&state_dir.join(STATE_DB_NAME)).map_err(|error| {
        CompositionError::Store {
            detail: error.to_string(),
        }
    })?;
    Ok(OperatorSurface {
        state: Arc::new(state),
        workspace_root: state_dir.join(defaults::WORKSPACES_DIR_NAME),
        escalation_root: state_dir.join(ESCALATIONS_DIR_NAME),
        answer_key: context.resolved.reference.answer_key.clone(),
        manifest: context.resolved.reference.manifest.clone(),
    })
}

/// Everything the composed run needs beyond the controller itself.
pub struct ComposedRun {
    pub controller: RunController,
    /// The run's OWN state store handle — the report and the operator
    /// surfaces read through it (the same store the loop wrote).
    pub state: Arc<dyn gauntlet_ports::state_store::StateStore>,
    /// The escalation queue root (what `gauntlet escalations list`
    /// reads).
    pub escalation_root: std::path::PathBuf,
}

/// Composes the full run controller over REAL adapters (non-dry-run).
#[allow(clippy::arc_with_non_send_sync)] // single-threaded loop; comparator/model sources carry the same interior mutability
pub fn compose_run(
    context: &RuntimeContext,
    intent: &str,
) -> Result<ComposedRun, CompositionError> {
    let token = select_runner_token(context)?;
    // The Annex E clock hook (spec §8.3; the mock crate's hook selector):
    // absent or empty keeps the production wall clock — behavior-identical
    // to the pre-hook composition — the exact `deterministic` token
    // selects the virtual clock, and ANY other value is the selector's
    // typed error surfaced as a composition failure. A hook that silently
    // changes nothing is the fail-open shape the live-E2E correction
    // removed (the composition never consulted the hook at all).
    let clock: Arc<dyn Clock> =
        match mock_runner::clock_from_env().map_err(|error| CompositionError::EnvHook {
            hook: "GAUNTLET_CLOCK",
            detail: error.to_string(),
        })? {
            Some(deterministic) => Arc::new(deterministic),
            None => Arc::new(recursive_runner::SystemClock::new()),
        };
    let state_dir = context.state_dir().to_path_buf();
    let workspace_root = state_dir.join(defaults::WORKSPACES_DIR_NAME);

    // Durable roots (state, evidence, escalations, workspaces) — both
    // ensured through the guard-named helper the write inventory pins.
    let evidence_root = context.resolved.evidence_dir.clone();
    ensure_configured_dir(&state_dir)?;
    ensure_configured_dir(&evidence_root)?;
    let state = Arc::new(
        EmbeddedStateStore::open(&state_dir.join(STATE_DB_NAME)).map_err(|error| {
            CompositionError::Store {
                detail: error.to_string(),
            }
        })?,
    );
    let evidence = Arc::new(FsEvidenceStore::open(&evidence_root).map_err(|error| {
        CompositionError::Store {
            detail: error.to_string(),
        }
    })?);
    let escalation_root = state_dir.join(ESCALATIONS_DIR_NAME);
    let escalation =
        FsEscalationQueue::open(&escalation_root).map_err(|error| CompositionError::Store {
            detail: error.to_string(),
        })?;
    let workspaces =
        FsWorkspaceProvider::open(&workspace_root).map_err(|error| CompositionError::Store {
            detail: error.to_string(),
        })?;

    // Ids: the Annex E seed hook keeps its documented meaning (deterministic
    // ids for tests); production runs draw the id seed from the operating
    let ids: Arc<dyn IdGenerator> = match mock_runner::id_generator_from_env() {
        Some(seeded) => Arc::new(seeded),
        None => Arc::new(
            UlidIdGenerator::new().map_err(|error| CompositionError::Store {
                detail: error.to_string(),
            })?,
        ),
    };

    // Runners (D2): the builder dispatches through the default token; the
    // critic dispatches through `runners.by_role.critic` when configured,
    // falling back to the same default.
    let critic_token = context
        .resolved
        .runners_by_role
        .get("critic")
        .map(|role| role.runner.clone())
        .unwrap_or_else(|| token.clone());
    let builder_runner = runner_for(&token, context, &clock)?;
    let critic_runner = runner_for(&critic_token, context, &clock)?;
    let runner_isolation = builder_runner.capabilities().isolation;

    // Observation.
    let observers = observers(context, &clock, &evidence)?;

    // Comparison: the frozen contract artifact, the deterministic diff
    // source, and the model source over the critic runner.
    let contract_text = std::fs::read_to_string(&context.resolved.reference.comparison_contract)
        .map_err(|error| CompositionError::ComparisonContract {
            detail: format!(
                "reading {}: {error}",
                context.resolved.reference.comparison_contract.display()
            ),
        })?;
    let contract = gauntlet_app::compare::contract::ComparisonContract::parse(&contract_text)
        .map_err(|error| CompositionError::ComparisonContract {
            detail: error.to_string(),
        })?;
    let budget_for_slices = gauntlet_domain::Budget::new(
        context
            .resolved
            .budget
            .limits()
            .map_err(|error| CompositionError::Configuration {
                key: "budget.max_rounds",
                env: "GAUNTLET_MAX_ROUNDS",
                detail: error.to_string(),
            })?,
        context.resolved.budget.recovery_reserve_pct,
        context.resolved.budget.diagnostic_allowance(),
    )
    .map_err(|error| CompositionError::Configuration {
        key: "budget.max_tokens",
        env: "GAUNTLET_MAX_TOKENS",
        detail: error.to_string(),
    })?;
    let model = ModelComparatorSource::new(ModelComparator::with_clock(
        critic_runner,
        ids.clone(),
        None,
        budget_for_slices,
        clock.clone(),
    ));
    let comparison = ComparisonDeps {
        contract,
        deterministic: Arc::new(DiffComparatorSource),
        model: Arc::new(model),
        adversarial: None,
        review_mode: review_mode(context)?,
        model_dispatches: context.resolved.review_max_critics,
        role_deny: critic_deny_set(context)?,
    };

    // Verification ladder (module docs): the deterministic rungs this
    // composition fronts, as a PER-COMPLETION FACTORY (COR-D3) — the
    // controller anchors the same rung set at each completion's evidence
    // workspace, so the instruments examine the workspace the dispatch
    // actually ran in. (`gauntlet verify` re-anchors the same set at the
    // canonical root — see `composed_ladder`.)
    let ladder_for: gauntlet_app::control::controller::LadderFactory =
        Arc::new(|root: &std::path::Path| composed_ladder(root).map_err(|error| error.to_string()));

    // Reference.
    let reference = gauntlet_app::reference::provider::FileReferenceProvider::from_paths(
        context.resolved.reference.answer_key.clone(),
        context.resolved.reference.manifest.clone(),
        std::rc::Rc::new(FsEvidenceStore::open(&evidence_root).map_err(|error| {
            CompositionError::Store {
                detail: error.to_string(),
            }
        })?),
    );

    // The composition-root meter factory (C2): the bare REAL adapter.
    let meter_factory: MeterFactory = Arc::new(
        |budget: gauntlet_domain::Budget| -> Arc<dyn gauntlet_ports::budget_meter::BudgetMeter> {
            Arc::new(MeteredBudget::new(budget))
        },
    );

    let deps = ControllerDeps {
        state: state.clone(),
        evidence,
        ids,
        clock: clock.clone(),
        runner: builder_runner,
        runner_identity: gauntlet_domain::RunnerIdentity::new(&token).map_err(|error| {
            CompositionError::RunnerConstruction {
                detail: error.to_string(),
            }
        })?,
        observers,
        comparison,
        ladder_for,
        reference: Arc::new(reference),
        answer_key: gauntlet_app::reference::provider::AnswerKeySource::Path(
            context.resolved.reference.answer_key.clone(),
        ),
        escalation: Arc::new(escalation),
        meter_factory,
        budget: BudgetShape {
            limits: context.resolved.budget.limits().map_err(|error| {
                CompositionError::Configuration {
                    key: "budget.max_rounds",
                    env: "GAUNTLET_MAX_ROUNDS",
                    detail: error.to_string(),
                }
            })?,
            recovery_reserve_pct: context.resolved.budget.recovery_reserve_pct,
            diagnostic: context.resolved.budget.diagnostic_allowance(),
        },
        policy: ControlPolicy::from_settings(
            &context.resolved.control,
            context.resolved.nonconvergence_n,
        )
        .map_err(|error| CompositionError::Configuration {
            key: "control.stall_timeout_ms",
            env: "GAUNTLET_STALL_TIMEOUT_MS",
            detail: error.to_string(),
        })?,
        workspace_root,
        config_fingerprint: gauntlet_app::config::fingerprint::config_fingerprint(
            &context.effective,
        ),
        // R-3 / D3: the wrap caller runs BEFORE any dispatch when the
        // process-backed token is selected — after this composition
        // point, before the controller's first invocation.
        govern: wrap_checked_posture(context, &token, runner_isolation)?,
        workspaces: Some(Arc::new(workspaces)),
        environment_fingerprint: environment_fingerprint(),
        intent: intent.to_owned(),
    };
    let controller = RunController::compose_binding_produced_artifacts(deps).map_err(|error| {
        CompositionError::Plan {
            detail: error.to_string(),
        }
    })?;
    Ok(ComposedRun {
        controller,
        state,
        escalation_root,
    })
}

fn environment_fingerprint() -> gauntlet_domain::Digest {
    // The composed environment: a digest over the HOST environment's
    // NAMES (never values — §8.1), so the record distinguishes
    // environments without storing secrets.
    let mut names = host_pairs();
    names.sort();
    let joined = names
        .into_iter()
        .map(|(key, _)| key)
        .collect::<Vec<_>>()
        .join("\n");
    gauntlet_app::config::fingerprint::digest_canonical(&joined)
}

/// The D3 wrap caller (module docs): admits the run's FIRST builder
/// dispatch shape and routes through the wrap verdict BEFORE any run
/// starts. Under the shipped none posture this is the composition-level
/// `BuilderCommand::Unwrapped` leg; a live-backend `Sandboxed` verdict
/// refuses the run (recorded for plan Task 25), never unwraps.
fn wrap_checked_posture(
    context: &RuntimeContext,
    token: &str,
    runner_isolation: gauntlet_ports::agent_runner::Isolation,
) -> Result<GovernDeps, CompositionError> {
    let govern = govern_posture(context)?;
    if token != generic_cli_runner::RUNNER_TOKEN {
        // The wrap caller fronts the process-backed token only (D3).
        return Ok(govern);
    }
    // The dispatch shape the wrap protects: the FIRST planned unit's real
    // facts, parsed from the frozen ruler.
    let key_text =
        std::fs::read_to_string(&context.resolved.reference.answer_key).map_err(|error| {
            CompositionError::Plan {
                detail: format!(
                    "reading {}: {error}",
                    context.resolved.reference.answer_key.display()
                ),
            }
        })?;
    let parsed = gauntlet_app::reference::parse::parse_answer_key(&key_text);
    let unit = parsed.units.first().ok_or_else(|| CompositionError::Plan {
        detail: "the ruler parses to no units — nothing to admit".to_owned(),
    })?;
    let facts = gauntlet_app::govern::GateFacts {
        unit: unit.id().clone(),
        tier: unit.blast_radius(),
        authority: unit.authority(),
        runner_isolation,
        workspace: gauntlet_domain::WorkspaceRef::new(defaults::WORKSPACES_DIR_NAME).map_err(
            |error| CompositionError::Plan {
                detail: error.to_string(),
            },
        )?,
    };
    let gate = gauntlet_app::govern::AuthorityGate::new();
    let deadline = crate::context::doctor_bound();
    let admission = gate
        .admit(&govern, &facts, None, deadline)
        .map_err(|refusal| CompositionError::WrapRefused {
            detail: refusal.summary(),
        })?;
    let command = std::env::var(generic_cli_runner::COMMAND_ENV_VAR).unwrap_or_default();
    match gate.wrap_builder_command(&govern, &admission, &command, deadline) {
        Ok(gauntlet_app::govern::BuilderCommand::Unwrapped { .. }) => Ok(govern),
        Ok(gauntlet_app::govern::BuilderCommand::Sandboxed(_)) => {
            Err(CompositionError::LiveContainmentDeferred {
                unit: unit.id().to_string(),
            })
        }
        Err(refusal) => Err(CompositionError::WrapRefused {
            detail: refusal.to_string(),
        }),
    }
}

/// The deterministic comparator source: composition-level glue over the
/// diff adapter (the p6 `DiffSource` shape, moved to the composition
/// root it always belonged to).
struct DiffComparatorSource;

impl gauntlet_ports::comparator::ComparatorSource for DiffComparatorSource {
    fn across_dimensions(&self) -> Result<Box<dyn gauntlet_ports::comparator::Comparator>, String> {
        Ok(Box::new(DiffComparator::across_dimensions()))
    }

    fn for_dimension(
        &self,
        dimension: &str,
    ) -> Result<Box<dyn gauntlet_ports::comparator::Comparator>, String> {
        Ok(Box::new(DiffComparator::for_dimension(dimension)))
    }
}
