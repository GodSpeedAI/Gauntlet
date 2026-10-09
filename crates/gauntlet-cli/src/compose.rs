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
//! - **Wrap caller (charter D3, R-3; plan Task 25's routing):** for the
//!   process-backed `cli-agent` token the composition calls
//!   `AuthorityGate::admit` + `AuthorityGate::wrap_builder_command`
//!   before any run starts, over the FIRST planned dispatch's real
//!   facts. Under the shipped `none` backend posture the verdict is
//!   `BuilderCommand::Unwrapped` and the run proceeds. Under a LIVE
//!   backend the `BuilderCommand::Sandboxed` verdict is ROUTED, not
//!   refused (the Task-25 leg): the composition installs the SAME
//!   posture's sandbox + policy resolution as the runner's containment
//!   seam, so every policy-carrying dispatch spawns the gate's wrapped
//!   command with the sandbox as its first process (AK-048), and the
//!   pre-composition verdict proves the posture CAN wrap before any run
//!   starts. A wrap/admission REFUSAL still fails the composition
//!   fail-closed. Never a silent unwrap (§21.3, AK-047).
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
use gauntlet_adapter_state_postgres::EmbeddedStateStore;
use gauntlet_adapter_verify_command_exit_code::CommandExitVerifier;
use gauntlet_adapter_verify_file_digest::FileDigestVerifier;
use gauntlet_adapter_verify_test_report::TestReportVerifier;
use gauntlet_adapter_workspace_fs::{FsWorkspaceProvider, WorkspaceSeed};
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
pub(crate) const STATE_DB_NAME: &str = "gauntlet-state.db";

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
    /// The composition-time wrap was refused by the gate.
    WrapRefused { detail: String },
    /// A fabric configuration could not be composed into runners (see
    /// [`crate::fabric_compose`]): a scenario did not load, or the registry
    /// and runner set disagree.
    Fabric { detail: String },
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
            Self::WrapRefused { detail } => {
                write!(f, "the composition-time wrap was refused: {detail}")
            }
            Self::Fabric { detail } => write!(f, "the fabric could not be composed: {detail}"),
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
pub(crate) fn runner_for(
    token: &str,
    context: &RuntimeContext,
    clock: &Arc<dyn Clock>,
    containment: Option<Arc<dyn generic_cli_runner::CommandContainment>>,
) -> Result<Arc<dyn AgentRunner>, CompositionError> {
    match token {
        recursive_runner::RUNNER_TOKEN => {
            let config = recursive_runner::PrimeAdapterConfig::default();
            let state_dir = context.state_dir().to_path_buf();
            let transport = recursive_runner::ProcessKernelTransport::new(
                config.binary.clone(),
                config.admission_timeout(),
            );
            let control = recursive_runner::ProcessHarnessControl::new(config.binary.clone());
            // The same canonical workspace root the process-backed runner
            // anchors under (COR-D3, §17.5.1 invariant 1): the prime-agent
            // runner now resolves its parent's cwd per invocation, at
            // `<workspace_root>/<spec.workspace>/`, never a fixed project
            // root — R-1b is closed (`.agents/CURRENT_STATUS.yml`), and the
            // frozen answer key at the project root must never sit inside a
            // builder-writable workspace (spec §8.5 check 7 / §22 item 5,
            // invariant I1).
            let workspace_root = context.state_dir().join(defaults::WORKSPACES_DIR_NAME);
            Ok(Arc::new(recursive_runner::PrimeAgentRunner::new(
                config,
                state_dir,
                workspace_root,
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
            // The task-25 wrap routing: under a live posture the SAME
            // sandbox + policy instances the gate holds back the runner's
            // containment seam, so the gate's Sandboxed verdict reaches
            // the actually-spawned child (AK-048). Under the conservative
            // `none` posture there is no seam and no policy ever rides a
            // dispatch — the historical template-only world, unchanged.
            let runner = match containment {
                Some(seam) => runner.with_containment(seam),
                None => runner,
            };
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
/// Builds the [`WorkspaceSeed`] for W5 work-product carry-forward: the
/// project root every run's integration tree snapshots from, the
/// `<state_dir>/integration` root that snapshot and its per-run copies
/// live under, and the excludes — VCS/build noise, this runtime's own
/// state/evidence/workspace/reference surfaces (when they resolve under
/// the project root), and every evidence locator the answer key's units
/// name (instrument-produced evidence must never ride in a builder's
/// carried work product — spec I2 / §14.3).
fn workspace_seed(
    context: &RuntimeContext,
    workspace_root: &std::path::Path,
    evidence_root: &std::path::Path,
) -> Result<Option<WorkspaceSeed>, CompositionError> {
    let Ok(canonical_project_root) = std::fs::canonicalize(&context.project_root) else {
        // Project-root discovery already requires an anchorable root in
        // practice; a canonicalization failure here means nothing to seed
        // over rather than a composition failure.
        return Ok(None);
    };
    let integration_root = context.state_dir().join(defaults::INTEGRATION_DIR_NAME);
    let mut excludes: Vec<String> = vec![
        ".git".to_owned(),
        "target".to_owned(),
        "node_modules".to_owned(),
    ];
    let relative_under_root = |path: &std::path::Path| -> Option<String> {
        let canonical = std::fs::canonicalize(path).ok()?;
        let relative = canonical.strip_prefix(&canonical_project_root).ok()?;
        if relative.as_os_str().is_empty() {
            return None;
        }
        Some(relative.to_string_lossy().into_owned())
    };
    for path in [
        context.state_dir(),
        evidence_root,
        workspace_root,
        context.resolved.reference.manifest.as_path(),
        context.resolved.reference.answer_key.as_path(),
        context.resolved.reference.map.as_path(),
        context.resolved.reference.comparison_contract.as_path(),
        context.roles_dir(),
        context.resolved.sandbox_policy_dir.as_path(),
    ] {
        if let Some(relative) = relative_under_root(path) {
            excludes.push(relative);
        }
    }
    // W6 detective containment watches the same surfaces — but WITHOUT
    // the answer-key evidence locators below: a locator path under the
    // project root is instrument-owned, and a builder touching it is
    // itself a breach worth catching, never seed noise to hide.
    let containment_excludes = excludes.clone();
    // Best-effort: an unreadable/unparseable answer key at composition
    // time skips this addition rather than failing the whole composition
    // (the answer key's own readability is enforced where it is
    // consumed, not here).
    if let Ok(key_text) = std::fs::read_to_string(&context.resolved.reference.answer_key) {
        let parsed = gauntlet_app::reference::parse::parse_answer_key(&key_text);
        for unit in &parsed.units {
            excludes.push(unit.evidence_required().locator().as_str().to_owned());
        }
    }
    Ok(Some(WorkspaceSeed {
        project_root: canonical_project_root,
        integration_root,
        excludes,
        containment_excludes,
    }))
}

pub(crate) fn ensure_configured_dir(path: &std::path::Path) -> Result<(), CompositionError> {
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
fn govern_posture(
    context: &RuntimeContext,
) -> Result<(GovernDeps, Option<LivePosture>), CompositionError> {
    use gauntlet_ports::sandbox::Sandbox;
    let token = context.resolved.sandbox_backend.clone();
    if token == defaults::SANDBOX_BACKEND {
        return Ok((GovernDeps::none_backend(), None));
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
    // The live posture exercises LEAD-AGENT authority (the §17.1
    // ceiling): the §5.3.2 tier/authority coupling forbids critic
    // authority on cross_cutting units outright, so a live posture that
    // could not exercise lead-agent authority could never dispatch the
    // tier it fronts a backend for. The conservative `none` posture keeps
    // its critic ceiling (task-20's world, unchanged). Recorded as the
    // task-25 operator delegation for the real-runner E2E.
    let policies_arc: Arc<ContractDirPolicy> = Arc::new(policies);
    let sandbox_arc: Arc<ProcessIsolationBackend> = Arc::new(backend);
    let govern = GovernDeps::with_backend(
        &token,
        Some(sandbox_arc.clone()),
        Some(policies_arc.clone()),
        gauntlet_domain::Authority::LeadAgent,
    )
    .map_err(|error| CompositionError::Store {
        detail: error.to_string(),
    })?;
    Ok((
        govern,
        Some(LivePosture {
            token,
            sandbox: sandbox_arc,
            policies: policies_arc,
        }),
    ))
}

/// The live-backend parts the govern posture is built from, kept so the
/// SAME sandbox and policy instances can back the runner's containment
/// seam — one posture, one set of facts, no bypass copy.
#[derive(Clone)]
struct LivePosture {
    token: String,
    sandbox: Arc<ProcessIsolationBackend>,
    policies: Arc<ContractDirPolicy>,
}

/// The task-25 wrap routing (deliverable 2): the containment seam the
/// composition installs into the process-backed runner over the live
/// posture. Every dispatch whose spec carries the gate-admitted policy
/// ref is wrapped HERE — through the SAME policy resolution and sandbox
/// the gate admits against, with validation re-earned at the spawn
/// boundary (validation precedes use, spec §6.2) — so the sandbox is the
/// FIRST process the runner starts (AK-048). A refusal from any leg is
/// the typed `ContainmentRefusal`: the dispatch does not run, and
/// nothing ever falls back to the unwrapped command (§21.3, AK-047).
#[derive(Debug)]
struct LiveContainment {
    token: String,
    sandbox: Arc<ProcessIsolationBackend>,
    policies: Arc<ContractDirPolicy>,
}

impl generic_cli_runner::CommandContainment for LiveContainment {
    fn wrap(
        &self,
        workspace: &gauntlet_domain::WorkspaceRef,
        policy_ref: &gauntlet_domain::SandboxPolicyRef,
        command: &str,
    ) -> Result<String, generic_cli_runner::ContainmentRefusal> {
        use gauntlet_ports::policy_provider::PolicyProvider;
        use gauntlet_ports::sandbox::Sandbox;
        let refused = |detail: String| {
            generic_cli_runner::ContainmentRefusal::new(format!(
                "the '{token}' posture refused to wrap the dispatch under policy {policy_ref}: \
                 {detail} — not executed unwrapped (spec 21.3, AK-047)",
                token = self.token,
                policy_ref = policy_ref.as_str(),
            ))
        };
        // Resolve the admitted policy BY REF over the dispatchable tiers
        // (the ref the durable admission record names resolves to exactly
        // one tier policy; the §5.3.5 top tier resolves none by design).
        for tier in [
            gauntlet_domain::BlastRadius::Contained,
            gauntlet_domain::BlastRadius::Local,
            gauntlet_domain::BlastRadius::CrossCutting,
        ] {
            let Ok(policy) =
                self.policies
                    .policy_for(tier, workspace, crate::context::doctor_bound())
            else {
                continue;
            };
            if policy.reference.as_str() != policy_ref.as_str() {
                continue;
            }
            // Validation precedes use — AGAIN, at the spawn boundary: a
            // policy that stopped validating between the gate's admission
            // and this spawn must refuse the dispatch, never wrap stale.
            let validation = self
                .sandbox
                .validate(&policy, crate::context::doctor_bound())
                .map_err(|error| refused(error.to_string()))?;
            let wrapped = self
                .sandbox
                .wrap(
                    command,
                    &policy,
                    &validation,
                    crate::context::doctor_bound(),
                )
                .map_err(|error| refused(error.to_string()))?;
            return Ok(wrapped.command);
        }
        Err(refused(
            "the admitted policy resolves to no dispatchable-tier policy under this posture"
                .to_owned(),
        ))
    }
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
#[derive(Debug)]
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
    /// Why a read command refuses: the state instance does not exist. The
    /// instance is identified by its locator path under `state_dir`; an
    /// unreachable database is a typed store error, never `false`.
    pub fn state_exists(state_dir: &std::path::Path) -> Result<bool, CompositionError> {
        gauntlet_adapter_state_postgres::state_exists(&state_dir.join(STATE_DB_NAME)).map_err(
            |error| CompositionError::Store {
                detail: error.to_string(),
            },
        )
    }
}

/// Opens the read-side operator surface; refuses (typed) when the run
/// state was never created.
pub fn open_operator_surface(
    context: &RuntimeContext,
) -> Result<OperatorSurface, CompositionError> {
    let state_dir = context.state_dir().to_path_buf();
    if !OperatorSurface::state_exists(&state_dir)? {
        return Err(CompositionError::Store {
            detail: format!(
                "no run state at {} — no run has been composed here yet",
                state_dir.join(STATE_DB_NAME).display()
            ),
        });
    }
    let state = EmbeddedStateStore::open_at(&state_dir.join(STATE_DB_NAME)).map_err(|error| {
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
    ensure_configured_dir(&workspace_root)?;
    let state = Arc::new(
        EmbeddedStateStore::open_at(&state_dir.join(STATE_DB_NAME)).map_err(|error| {
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
    let mut workspaces =
        FsWorkspaceProvider::open(&workspace_root).map_err(|error| CompositionError::Store {
            detail: error.to_string(),
        })?;
    // W5 work-product carry-forward: seed every workspace over a
    // run-scoped integration tree (a filtered copy of the project root),
    // so a unit's builder starts from what the run has already settled
    // instead of an empty directory, and `promote` can carry a unit's
    // settled changes forward. Excludes cover VCS/build noise, this
    // runtime's own state/evidence surfaces, the reference/answer-key
    // inputs, and every evidence locator the answer key names — none of
    // that is a builder's work product, and an instrument-produced
    // locator riding into the next unit's seed would be stale or
    // forgeable (spec I2 / §14.3).
    if let Some(seed) = workspace_seed(context, &workspace_root, &evidence_root)? {
        workspaces = workspaces.with_seed(seed);
    }

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
    // falling back to the same default. The govern posture is composed
    // FIRST: a live backend backs the process-backed runner's containment
    // seam (the task-25 wrap routing) from the SAME sandbox + policy
    // instances the gate holds.
    let (govern, live_posture) = govern_posture(context)?;
    let containment: Option<Arc<dyn generic_cli_runner::CommandContainment>> =
        live_posture.as_ref().map(|posture| {
            Arc::new(LiveContainment {
                token: posture.token.clone(),
                sandbox: posture.sandbox.clone(),
                policies: posture.policies.clone(),
            }) as Arc<dyn generic_cli_runner::CommandContainment>
        });
    let critic_token = context
        .resolved
        .runners_by_role
        .get("critic")
        .map(|role| role.runner.clone())
        .unwrap_or_else(|| token.clone());
    let builder_runner = runner_for(&token, context, &clock, containment.clone())?;
    let critic_runner = runner_for(&critic_token, context, &clock, containment)?;
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
    let critic_identity = gauntlet_domain::RunnerIdentity::new(&critic_token).map_err(|error| {
        CompositionError::RunnerConstruction {
            detail: error.to_string(),
        }
    })?;
    let recording_critic_runner = Arc::new(gauntlet_app::compare::RecordingAgentRunner::new(
        critic_runner,
        state.clone(),
        evidence.clone(),
        critic_identity,
        clock.clone(),
    ));
    let model = ModelComparatorSource::new(ModelComparator::with_clock(
        recording_critic_runner,
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
        // R-3 / D3 / plan Task 25: the wrap caller runs BEFORE any
        // dispatch when the process-backed token is selected — after this
        // composition point, before the controller's first invocation. A
        // LIVE `Sandboxed` verdict is ROUTED (the containment seam is
        // already installed on the runner); only a refusal stops the run.
        govern: wrap_checked_posture(context, &token, runner_isolation, govern)?,
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
/// `BuilderCommand::Unwrapped` leg. Under a live backend the
/// `BuilderCommand::Sandboxed` verdict is the task-25 ROUTING proof — the
/// posture CAN wrap, and the seam that wraps the first dispatch wraps
/// every dispatch — so the run proceeds; only a typed refusal (an
/// unadmittable dispatch shape, a failed validation, an unusable backend)
/// fails the composition, fail-closed, never unwrapped (§21.3, AK-047).
fn wrap_checked_posture(
    context: &RuntimeContext,
    token: &str,
    runner_isolation: gauntlet_ports::agent_runner::Isolation,
    govern: GovernDeps,
) -> Result<GovernDeps, CompositionError> {
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
        // The task-25 routing: the verdict is `Sandboxed` — the posture
        // wraps, the runner's installed containment seam performs the
        // same wrap per dispatch, and the run proceeds. Refusing here was
        // the recorded task-20 deferral; retiring that deferral IS this
        // task's production correction.
        Ok(gauntlet_app::govern::BuilderCommand::Sandboxed(_)) => Ok(govern),
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
