//! Fabric composition (F2b1): a [`RunnerRegistry`] and its [`RunnerSet`] built
//! from a fabric configuration directory, with no server.
//!
//! The directory holds flat TOML files, parsed by the harness-catalog subset
//! parser (unknown keys, duplicate keys, and unsupported syntax all fail):
//!
//! - `profiles/*.toml` one runner profile per file: `schema`, `id`, `harness`,
//!   `token`, optional `provider` and `model`, `command` (a command template,
//!   required for the command-template and interactive tokens and rejected for
//!   the others), `scenario` (required for the scripted token and rejected for
//!   the others), `workspace_mutation` (default false), `isolation` (default
//!   `process` for process runners, `none` for the scripted one), `max_depth`
//!   (default 1).
//! - `roles/*.toml` one role grant per file: `schema`, `role`, `may_spawn`,
//!   `allow` and `deny` (context kind tokens), `permissions`, and `workspace`
//!   (`fresh`, the only implemented policy).
//! - `limits.toml` the graph limits, the per-spawn budget ceiling, and the
//!   isolation fact the authority gate consumes. Every key is required.
//!
//! [`load_fabric`] is pure parsing and validation. It fails closed with a typed
//! [`FabricConfigError`] naming the file and the key, and it reads files in
//! sorted order so the outcome never depends on directory listing order.
//! [`compose_fabric`] builds one runner per profile. Constructing a runner
//! spawns nothing. Availability is decided by an injected
//! [`ExecutableLookup`]: a profile whose canonical executable cannot be
//! discovered is registered as `Unavailable` with its reason, and selection
//! refuses it.
//!
//! Capability facts come from the harness definition and the profile, never
//! from the runner token. A definition at support level 0 stays at level 0
//! whatever runner drives it.

use std::collections::{BTreeSet, HashMap};
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::Arc;

use gauntlet_adapter_agent_cli as generic_cli_runner;
use gauntlet_adapter_agent_mock as mock_runner;
use gauntlet_adapter_agent_prime as recursive_runner;
use gauntlet_adapter_agent_tty as interactive_runner;
use gauntlet_adapter_harness_catalog::{
    HarnessDefinition, LifecycleSource as CatalogLifecycle, Parser, SupportLevel as CatalogSupport,
    Table, Value, builtin, load_custom_dir,
};
use gauntlet_adapter_tool_program_codemode::DEFAULT_PROGRAM_TIMEOUT;
use gauntlet_app::config::defaults;
use gauntlet_app::delegation::{
    AgentsPolicy, ContextItemKind, ContextPolicy, RoleGrant, RunnerSet, WorkspacePolicy,
};
use gauntlet_app::fabric::{
    Availability, FabricCapabilities, LifecycleSource, ModelFacts, RunnerInstance, RunnerProfile,
    RunnerRegistry, SupportLevel,
};
use gauntlet_app::tools::{
    DEFAULT_PROGRAM_MAX_PARALLEL_CALLS, DEFAULT_PROGRAM_MAX_RESULT_BYTES,
    DEFAULT_PROGRAM_MAX_TOOL_CALLS, ProgramCeilings,
};
use gauntlet_domain::{HarnessId, MicroUsd, PermissionSetRef, RoleId, RunnerProfileId};
use gauntlet_ports::agent_runner::{AgentRunner, Isolation};
use gauntlet_ports::budget_meter::BudgetSlice;
use gauntlet_ports::clock::Clock;
use gauntlet_ports::invocation_graph::GraphLimits;

use crate::compose::CompositionError;

/// The profile directory under the fabric configuration directory.
const PROFILES_DIR: &str = "profiles";
/// The role directory under the fabric configuration directory.
const ROLES_DIR: &str = "roles";
/// The limits file under the fabric configuration directory.
const LIMITS_FILE: &str = "limits.toml";
/// The only file extension a configuration directory may hold.
const CONFIG_EXTENSION: &str = "toml";
/// The `schema` value every configuration file must declare.
const SCHEMA_VERSION: i64 = 1;
/// The key every configuration file carries its schema under.
const SCHEMA_KEY: &str = "schema";
/// The largest configuration file accepted, in bytes.
const MAX_CONFIG_BYTES: u64 = 64 * 1024;
/// The environment variable the production lookup scans for executables.
const SEARCH_PATH_VAR: &str = "PATH";
/// The execute permission bits a discovered executable must carry.
const EXECUTE_BITS: u32 = 0o111;
/// The only workspace policy this composition implements.
const WORKSPACE_FRESH: &str = "fresh";
/// The rendering of an unknown provider or model.
const UNKNOWN: &str = "UNKNOWN";

const PROFILE_KEYS: &[&str] = &[
    SCHEMA_KEY,
    "id",
    "harness",
    "token",
    "provider",
    "model",
    "command",
    "scenario",
    "workspace_mutation",
    "isolation",
    "max_depth",
    "mcp_bridge",
];
const ROLE_KEYS: &[&str] = &[
    SCHEMA_KEY,
    "role",
    "may_spawn",
    "allow",
    "deny",
    "permissions",
    "workspace",
];
const LIMITS_KEYS: &[&str] = &[
    SCHEMA_KEY,
    "max_depth",
    "max_children_per_parent",
    "rounds",
    "wall_seconds",
    "tokens",
    "cost_usd_micros",
    "isolation",
    PROGRAM_TIMEOUT_KEY,
    PROGRAM_MAX_TOOL_CALLS_KEY,
    PROGRAM_MAX_PARALLEL_KEY,
    PROGRAM_MAX_RESULT_BYTES_KEY,
];
/// Optional ceilings for `program.run` (see [`FabricLimits::program`]).
const PROGRAM_TIMEOUT_KEY: &str = "program_timeout_ms";
const PROGRAM_MAX_TOOL_CALLS_KEY: &str = "program_max_tool_calls";
const PROGRAM_MAX_PARALLEL_KEY: &str = "program_max_parallel";
const PROGRAM_MAX_RESULT_BYTES_KEY: &str = "program_max_result_bytes";

/// The four runner tokens this composition fronts. The token strings live in
/// the adapters' `RUNNER_TOKEN` constants; this enum only names the choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerToken {
    /// The recursive process runner.
    Recursive,
    /// The command-template process runner.
    CommandTemplate,
    /// The interactive (pseudo-terminal) process runner.
    Interactive,
    /// The scripted runner that replays a named scenario fixture.
    Scripted,
}

impl RunnerToken {
    /// The token a configuration file names, if this composition fronts it.
    pub fn parse(token: &str) -> Option<Self> {
        [
            Self::Recursive,
            Self::CommandTemplate,
            Self::Interactive,
            Self::Scripted,
        ]
        .into_iter()
        .find(|candidate| candidate.token() == token)
    }

    /// The adapter's runner token constant.
    pub fn token(self) -> &'static str {
        match self {
            Self::Recursive => recursive_runner::RUNNER_TOKEN,
            Self::CommandTemplate => generic_cli_runner::RUNNER_TOKEN,
            Self::Interactive => interactive_runner::RUNNER_TOKEN,
            Self::Scripted => mock_runner::RUNNER_TOKEN,
        }
    }

    fn needs_command(self) -> bool {
        matches!(self, Self::CommandTemplate | Self::Interactive)
    }

    fn needs_scenario(self) -> bool {
        matches!(self, Self::Scripted)
    }

    fn default_isolation(self) -> Isolation {
        if self.needs_scenario() {
            Isolation::None
        } else {
            Isolation::Process
        }
    }

    /// The runner's own template probe. Scripted and recursive runners take no
    /// command template, so they have no probe to run.
    fn probe_command(self, template: &str) -> Result<(), String> {
        match self {
            Self::CommandTemplate => {
                generic_cli_runner::probe_template(template).map_err(|error| error.to_string())
            }
            Self::Interactive => {
                interactive_runner::probe_template(template).map_err(|error| error.to_string())
            }
            Self::Recursive | Self::Scripted => Ok(()),
        }
    }
}

/// The graph limits, the per-spawn budget ceiling, and the declared isolation
/// (`limits.toml`). The declared isolation is validated here; composition
/// caps the gate's isolation to what is wired (see `compose_fabric`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FabricLimits {
    pub graph: GraphLimits,
    pub max_budget: BudgetSlice,
    pub isolation: Isolation,
    /// Ceilings for `program.run`. Optional keys in `limits.toml`:
    /// `program_timeout_ms` (default: the codemode adapter's program timeout),
    /// `program_max_tool_calls` (default 16), `program_max_parallel` (default
    /// 4), and `program_max_result_bytes` (default 64 KiB). Each request may
    /// ask for less, never more.
    pub program: ProgramCeilings,
}

/// One runner profile as declared in `profiles/*.toml`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileSpec {
    pub id: RunnerProfileId,
    /// A harness definition id from the built-in or custom catalog.
    pub harness: String,
    pub token: RunnerToken,
    /// `None` is UNKNOWN: a provider is never defaulted.
    pub provider: Option<String>,
    /// `None` is UNKNOWN: a model is never defaulted.
    pub model: Option<String>,
    pub command: Option<String>,
    pub scenario: Option<String>,
    pub workspace_mutation: bool,
    pub isolation: Isolation,
    pub max_depth: u8,
    /// Whether this profile's invocations get a per-invocation MCP bridge to
    /// the fabric server (`mcp_bridge = true`, recursive token only). The
    /// bridge is provisioned only in a process that has a serve launch.
    pub mcp_bridge: bool,
}

/// How a bridge-enabled runner launches the fabric MCP server: the executable
/// and the fixed `fabric serve` argv prefix, with every value a composition
/// input. A runner appends `--caller <its own invocation id>` and its own
/// session binding to it. Supplied only by the process that serves a caller
/// (`fabric serve`); absent elsewhere, where a bridge-enabled profile is
/// registered as unavailable.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct McpServeLaunch {
    pub program: PathBuf,
    pub args: Vec<String>,
}

/// A validated fabric configuration: pure data, nothing constructed.
#[derive(Debug, Clone)]
pub struct FabricConfig {
    /// Built-in and custom harness definitions, sorted by id.
    pub harnesses: Vec<HarnessDefinition>,
    /// Profiles, sorted by id.
    pub profiles: Vec<ProfileSpec>,
    /// Role grants, in sorted file-name order.
    pub roles: Vec<(RoleId, RoleGrant)>,
    pub limits: FabricLimits,
}

/// Why a fabric configuration was refused. Every variant names the file, and
/// where a key is at fault, the key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FabricConfigError {
    /// A harness catalog (built-in or custom) did not load or validate.
    Catalog { source: String, detail: String },
    /// A file or directory could not be read.
    Unreadable { file: String, detail: String },
    /// A symlink, or a directory where a file belongs, is refused.
    NotRegular { file: String },
    /// The file is larger than the accepted bound.
    Oversized { file: String, bytes: u64 },
    /// A configuration directory holds an entry that is not a TOML file.
    UnexpectedEntry { file: String },
    /// The file is not valid text in the accepted TOML subset.
    Syntax { file: String, detail: String },
    /// A key the file kind does not accept.
    UnknownKey { file: String, key: String },
    /// A key the file kind requires is absent.
    MissingKey { file: String, key: String },
    /// A key is present with a value outside its vocabulary or range.
    InvalidValue {
        file: String,
        key: String,
        detail: String,
    },
    /// A profile names a harness no catalog defines (fails closed).
    UnknownHarness { file: String, harness: String },
    /// The command template fails the runner's own template probe.
    ProbeFailed {
        file: String,
        key: String,
        detail: String,
    },
    /// Two files, or two catalog entries, claim the same identity.
    Duplicate {
        file: String,
        key: String,
        value: String,
    },
}

impl std::fmt::Display for FabricConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Catalog { source, detail } => {
                write!(f, "fabric config: harness catalog '{source}': {detail}")
            }
            Self::Unreadable { file, detail } => {
                write!(f, "fabric config '{file}': cannot be read: {detail}")
            }
            Self::NotRegular { file } => write!(
                f,
                "fabric config '{file}': must be a regular file or directory, not a symlink"
            ),
            Self::Oversized { file, bytes } => write!(
                f,
                "fabric config '{file}': {bytes} bytes exceeds the {MAX_CONFIG_BYTES}-byte bound"
            ),
            Self::UnexpectedEntry { file } => write!(
                f,
                "fabric config '{file}': a configuration directory holds only .{CONFIG_EXTENSION} files"
            ),
            Self::Syntax { file, detail } => write!(f, "fabric config '{file}': {detail}"),
            Self::UnknownKey { file, key } => {
                write!(f, "fabric config '{file}': unknown key '{key}'")
            }
            Self::MissingKey { file, key } => {
                write!(f, "fabric config '{file}': key '{key}' is required")
            }
            Self::InvalidValue { file, key, detail } => {
                write!(f, "fabric config '{file}': key '{key}': {detail}")
            }
            Self::UnknownHarness { file, harness } => write!(
                f,
                "fabric config '{file}': key 'harness': '{harness}' is defined by neither the \
                 built-in nor the custom harness catalog"
            ),
            Self::ProbeFailed { file, key, detail } => {
                write!(
                    f,
                    "fabric config '{file}': key '{key}': template probe: {detail}"
                )
            }
            Self::Duplicate { file, key, value } => write!(
                f,
                "fabric config '{file}': key '{key}': '{value}' is already declared"
            ),
        }
    }
}

impl std::error::Error for FabricConfigError {}

/// Resolves an executable name to a discoverable location. The production
/// implementation scans the search path; tests inject a fake.
pub trait ExecutableLookup {
    /// Where a named executable can be run from, if it can.
    fn locate(&self, executable: &str) -> Option<PathBuf>;
}

/// The production lookup: the first executable file on the search path.
#[derive(Debug, Clone, Copy, Default)]
pub struct SearchPathLookup;

impl ExecutableLookup for SearchPathLookup {
    fn locate(&self, executable: &str) -> Option<PathBuf> {
        let search = std::env::var_os(SEARCH_PATH_VAR)?;
        std::env::split_paths(&search)
            .map(|dir| dir.join(executable))
            .find(|candidate| is_executable_file(candidate))
    }
}

fn is_executable_file(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    fs::metadata(path)
        .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & EXECUTE_BITS != 0)
}

/// The injected dependencies of composition.
pub struct FabricDeps<'a> {
    /// The state directory the runners anchor their invocation IO under.
    pub state_dir: PathBuf,
    /// The injected time port every runner takes.
    pub clock: Arc<dyn Clock>,
    /// The containment seam the command-template runner wraps children with
    /// (the composition's live posture); `None` is the historical template-only
    /// world.
    pub containment: Option<Arc<dyn generic_cli_runner::CommandContainment>>,
    /// Executable discovery for the availability decision.
    pub path_lookup: &'a dyn ExecutableLookup,
    /// The fabric server launch a bridge-enabled profile provisions, when this
    /// process serves a caller. `None` makes such profiles unavailable here.
    pub mcp_serve: Option<McpServeLaunch>,
}

/// Why a profile is or is not available, with the facts the report prints.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileReport {
    pub id: RunnerProfileId,
    pub harness: String,
    pub token: &'static str,
    pub availability: Availability,
    pub support_level: SupportLevel,
    pub lifecycle_source: LifecycleSource,
    /// The harness definition's canonical executable, if it declares one.
    pub executable: Option<String>,
    /// `None` is UNKNOWN.
    pub provider: Option<String>,
    /// `None` is UNKNOWN.
    pub model: Option<String>,
    /// Whether the model is known. `false` means UNKNOWN, never a default.
    pub model_known: bool,
    /// The isolation the profile declares (`profiles/*.toml`, or the token's
    /// default).
    pub declared_isolation: Isolation,
    /// The isolation the profile can honestly run under: the declared level
    /// when its runner has a wired containment seam, else `None`.
    pub isolation: Isolation,
}

impl ProfileReport {
    /// One human-readable line. An unknown provider or model prints `UNKNOWN`,
    /// never a blank.
    pub fn render(&self) -> String {
        let availability = match &self.availability {
            Availability::Available => "available".to_owned(),
            Availability::Unavailable { reason } => format!("unavailable ({reason})"),
        };
        format!(
            "profile {} harness {} token {} availability {} support {} lifecycle {} executable {} provider {} model {} isolation {}",
            self.id,
            self.harness,
            self.token,
            availability,
            self.support_level,
            self.lifecycle_source,
            self.executable.as_deref().unwrap_or("none"),
            self.provider.as_deref().unwrap_or(UNKNOWN),
            self.model.as_deref().unwrap_or(UNKNOWN),
            self.isolation,
        )
    }
}

/// A profile whose declared isolation was capped to what its runner wires.
/// Recorded at composition so the cap is visible (serve startup, doctor),
/// never silent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IsolationCap {
    pub profile: RunnerProfileId,
    pub declared: Isolation,
    pub effective: Isolation,
}

impl IsolationCap {
    /// One human-readable line naming the profile and why it was capped.
    pub fn render(&self) -> String {
        format!(
            "profile {} declared isolation {} capped to {}: no containment seam wired",
            self.profile, self.declared, self.effective
        )
    }
}

/// The composed fabric: the registry, the runners bound to it, the affordance
/// policy, the per-profile report, and the isolation caps applied.
pub struct ComposedFabric {
    pub registry: RunnerRegistry,
    pub runners: RunnerSet,
    pub policy: AgentsPolicy,
    pub report: Vec<ProfileReport>,
    /// Every profile whose declared isolation exceeded its effective one.
    pub isolation_caps: Vec<IsolationCap>,
    /// The `program.run` ceilings from `limits.toml`.
    pub program: ProgramCeilings,
}

/// Loads and validates a fabric configuration directory. Pure: reads files,
/// constructs nothing.
pub fn load_fabric(
    dir: &Path,
    custom_harness_dir: Option<&Path>,
) -> Result<FabricConfig, FabricConfigError> {
    let harnesses = load_harnesses(custom_harness_dir)?;
    let limits = parse_limits(&dir.join(LIMITS_FILE))?;

    let mut profiles: Vec<ProfileSpec> = Vec::new();
    let mut profile_ids = BTreeSet::new();
    for path in config_files(&dir.join(PROFILES_DIR))? {
        let table = read_table(&path)?;
        let spec = parse_profile(&path, &table, &harnesses)?;
        let value = spec.id.to_string();
        if !profile_ids.insert(value.clone()) {
            return Err(duplicate(&path, "id", value));
        }
        profiles.push(spec);
    }
    profiles.sort_by(|left, right| left.id.cmp(&right.id));

    let mut roles: Vec<(RoleId, RoleGrant)> = Vec::new();
    let mut role_names = BTreeSet::new();
    for path in config_files(&dir.join(ROLES_DIR))? {
        let table = read_table(&path)?;
        let (role, grant) = parse_role(&path, &table)?;
        let value = role.to_string();
        if !role_names.insert(value.clone()) {
            return Err(duplicate(&path, "role", value));
        }
        roles.push((role, grant));
    }

    Ok(FabricConfig {
        harnesses,
        profiles,
        roles,
        limits,
    })
}

/// Builds the registry, the runner set, the policy, and the report from a
/// validated configuration. Constructing a runner spawns nothing; availability
/// comes from `deps.path_lookup`.
#[allow(clippy::arc_with_non_send_sync)] // single-threaded loop; the adapters carry interior mutability (as in compose::runner_for)
pub fn compose_fabric(
    config: &FabricConfig,
    deps: &FabricDeps<'_>,
) -> Result<ComposedFabric, CompositionError> {
    let workspace_root = deps.state_dir.join(defaults::WORKSPACES_DIR_NAME);
    let mut instances = Vec::with_capacity(config.profiles.len());
    let mut entries: Vec<(RunnerProfileId, Arc<dyn AgentRunner>)> =
        Vec::with_capacity(config.profiles.len());
    let mut report = Vec::with_capacity(config.profiles.len());
    let mut isolation_caps = Vec::new();
    // The gate's ceiling is the strongest isolation any AVAILABLE profile can
    // actually contain. An unwired profile contributes its effective `None`,
    // so the ceiling is `None` when no available profile is wired. The
    // declared `limits.toml` isolation is not the gate's fact: it is honoured
    // only through the profiles whose runners are wired.
    let mut gate_isolation = Isolation::None;

    for spec in &config.profiles {
        let definition = config
            .harnesses
            .iter()
            .find(|definition| definition.id == spec.harness)
            .ok_or_else(|| CompositionError::Fabric {
                detail: format!(
                    "profile '{}' names harness '{}', which the loaded catalog does not hold",
                    spec.id, spec.harness
                ),
            })?;
        let executable = definition.executables.first().cloned();
        // A bridge-enabled profile without a serve launch is registered as
        // unavailable: selection refuses it, and composition still succeeds
        // for processes (such as admit-root) that never invoke runners.
        let availability = if spec.mcp_bridge && deps.mcp_serve.is_none() {
            Availability::Unavailable {
                reason: format!(
                    "profile '{}' enables the MCP bridge, but this process has no fabric serve \
                     launch to provision it with",
                    spec.id
                ),
            }
        } else {
            availability(spec.token, executable.as_deref(), deps.path_lookup)
        };
        let runner = build_runner(spec, &workspace_root, deps)?;
        let isolation = effective_isolation(spec, deps);
        if isolation < spec.isolation {
            isolation_caps.push(IsolationCap {
                profile: spec.id.clone(),
                declared: spec.isolation,
                effective: isolation,
            });
        }
        if matches!(availability, Availability::Available) {
            gate_isolation = gate_isolation.max(isolation);
        }
        let capabilities = capabilities(spec, definition, isolation);
        let harness = HarnessId::new(&spec.harness).map_err(|error| CompositionError::Fabric {
            detail: format!("profile '{}': harness id: {error}", spec.id),
        })?;

        report.push(ProfileReport {
            id: spec.id.clone(),
            harness: spec.harness.clone(),
            token: spec.token.token(),
            availability: availability.clone(),
            support_level: capabilities.support_level,
            lifecycle_source: capabilities.lifecycle_source,
            executable,
            provider: spec.provider.clone(),
            model: spec.model.clone(),
            model_known: spec.model.is_some(),
            declared_isolation: spec.isolation,
            isolation,
        });
        instances.push(RunnerInstance {
            profile: RunnerProfile {
                id: spec.id.clone(),
                harness,
                model: ModelFacts {
                    provider: spec.provider.clone(),
                    model: spec.model.clone(),
                },
                label: definition.label.clone(),
            },
            availability,
            capabilities,
        });
        entries.push((spec.id.clone(), runner));
    }

    let registry = RunnerRegistry::new(instances).map_err(|error| CompositionError::Fabric {
        detail: error.to_string(),
    })?;
    let runners =
        RunnerSet::for_registry(&registry, entries).map_err(|error| CompositionError::Fabric {
            detail: error.to_string(),
        })?;
    let policy = AgentsPolicy {
        roles: config.roles.iter().cloned().collect::<HashMap<_, _>>(),
        limits: config.limits.graph,
        max_budget: config.limits.max_budget,
        isolation: gate_isolation,
    };
    Ok(ComposedFabric {
        registry,
        runners,
        policy,
        report,
        isolation_caps,
        program: config.limits.program,
    })
}

/// Whether a profile's runner has a containment seam wired. Only the
/// command-template runner takes one, and only when composition installs it.
fn containment_wired(token: RunnerToken, deps: &FabricDeps<'_>) -> bool {
    token == RunnerToken::CommandTemplate && deps.containment.is_some()
}

/// The isolation a profile can honestly run under: its declared level when
/// its runner is wired, else `None`.
fn effective_isolation(spec: &ProfileSpec, deps: &FabricDeps<'_>) -> Isolation {
    if containment_wired(spec.token, deps) {
        spec.isolation
    } else {
        Isolation::None
    }
}

fn availability(
    token: RunnerToken,
    executable: Option<&str>,
    lookup: &dyn ExecutableLookup,
) -> Availability {
    match (token, executable) {
        (RunnerToken::Scripted, _) => Availability::Available,
        (_, None) => Availability::Unavailable {
            reason: "the harness definition declares no canonical executable".to_owned(),
        },
        (_, Some(name)) => {
            if lookup.locate(name).is_some() {
                Availability::Available
            } else {
                Availability::Unavailable {
                    reason: format!("executable '{name}' is not discoverable on the search path"),
                }
            }
        }
    }
}

/// The Prime adapter configuration for a recursive profile. The bridge is set
/// only when the profile enables it AND this process has a serve launch; the
/// availability gate already refuses the other case.
fn prime_adapter_config(
    spec: &ProfileSpec,
    deps: &FabricDeps<'_>,
) -> recursive_runner::PrimeAdapterConfig {
    let mut config = recursive_runner::PrimeAdapterConfig::default();
    if spec.mcp_bridge {
        config.mcp_bridge = deps.mcp_serve.as_ref().map(|launch| {
            recursive_runner::McpBridge::new(launch.program.clone(), launch.args.clone())
        });
    }
    config
}

#[allow(clippy::arc_with_non_send_sync)] // single-threaded loop; the adapters carry interior mutability (as in compose::runner_for)
fn build_runner(
    spec: &ProfileSpec,
    workspace_root: &Path,
    deps: &FabricDeps<'_>,
) -> Result<Arc<dyn AgentRunner>, CompositionError> {
    let state_dir = deps.state_dir.clone();
    let workspace = workspace_root.to_path_buf();
    match spec.token {
        RunnerToken::Recursive => {
            let config = prime_adapter_config(spec, deps);
            let transport = recursive_runner::ProcessKernelTransport::new(
                config.binary.clone(),
                config.admission_timeout(),
            );
            let control = recursive_runner::ProcessHarnessControl::new(config.binary.clone());
            Ok(Arc::new(recursive_runner::PrimeAgentRunner::new(
                config,
                state_dir,
                workspace,
                transport,
                control,
                deps.clock.clone(),
            )))
        }
        RunnerToken::CommandTemplate => {
            let config = generic_cli_runner::CliAgentConfig::from_parts(
                command_of(spec)?,
                spec.max_depth,
                generic_cli_runner::config::DEFAULT_CANCEL_SPAN_SECONDS,
            );
            let runner = generic_cli_runner::CliAgentRunner::new(
                config,
                state_dir,
                workspace,
                generic_cli_runner::ProcessChildTransport::new(),
                deps.clock.clone(),
            )
            .map_err(|error| CompositionError::RunnerConstruction {
                detail: format!("profile '{}': {error}", spec.id),
            })?;
            Ok(Arc::new(match &deps.containment {
                Some(seam) => runner.with_containment(seam.clone()),
                None => runner,
            }))
        }
        RunnerToken::Interactive => {
            let config = interactive_runner::InteractiveAgentConfig::from_parts(
                command_of(spec)?,
                spec.max_depth,
                interactive_runner::config::DEFAULT_CANCEL_SPAN_SECONDS,
            );
            let runner = interactive_runner::InteractiveAgentRunner::new(
                config,
                state_dir,
                workspace,
                interactive_runner::InteractiveChildTransport::new(),
                deps.clock.clone(),
            )
            .map_err(|error| CompositionError::RunnerConstruction {
                detail: format!("profile '{}': {error}", spec.id),
            })?;
            Ok(Arc::new(runner))
        }
        RunnerToken::Scripted => {
            let stem = spec
                .scenario
                .as_deref()
                .ok_or_else(|| CompositionError::Fabric {
                    detail: format!("profile '{}' has no scenario", spec.id),
                })?;
            let scenario =
                mock_runner::load_scenario(stem).map_err(|error| CompositionError::Fabric {
                    detail: format!("profile '{}': scenario '{stem}': {error}", spec.id),
                })?;
            Ok(Arc::new(mock_runner::MockAgentRunner::new(
                scenario, workspace,
            )))
        }
    }
}

fn command_of(spec: &ProfileSpec) -> Result<&str, CompositionError> {
    spec.command
        .as_deref()
        .ok_or_else(|| CompositionError::Fabric {
            detail: format!("profile '{}' has no command template", spec.id),
        })
}

fn capabilities(
    spec: &ProfileSpec,
    definition: &HarnessDefinition,
    isolation: Isolation,
) -> FabricCapabilities {
    FabricCapabilities {
        support_level: support_level(definition.capabilities.support_level),
        lifecycle_source: lifecycle_source(definition.lifecycle.source),
        resumable: definition.capabilities.resumable,
        model_selection: definition.capabilities.model_selection,
        native_subagents: definition.capabilities.native_subagents,
        streaming_events: definition.capabilities.structured_events,
        structured_lifecycle: definition.lifecycle.source == CatalogLifecycle::Structured,
        // Every fronted runner builds a fresh envelope per invocation.
        fresh_context: true,
        workspace_mutation: spec.workspace_mutation,
        isolation,
        max_depth: spec.max_depth,
    }
}

fn support_level(level: CatalogSupport) -> SupportLevel {
    match level {
        CatalogSupport::Process => SupportLevel::Process,
        CatalogSupport::Interactive => SupportLevel::Interactive,
        CatalogSupport::Session => SupportLevel::Session,
        CatalogSupport::Structured => SupportLevel::Structured,
    }
}

/// A definition with no lifecycle source is registered as `Unavailable`, so
/// nothing consults a lifecycle that does not exist.
fn lifecycle_source(source: CatalogLifecycle) -> LifecycleSource {
    match source {
        CatalogLifecycle::Structured => LifecycleSource::Structured,
        CatalogLifecycle::Process => LifecycleSource::Process,
        CatalogLifecycle::Screen => LifecycleSource::Screen,
        CatalogLifecycle::None => LifecycleSource::Unavailable,
    }
}

fn load_harnesses(custom: Option<&Path>) -> Result<Vec<HarnessDefinition>, FabricConfigError> {
    let catalog_error = |source: &str, error: gauntlet_adapter_harness_catalog::CatalogError| {
        FabricConfigError::Catalog {
            source: source.to_owned(),
            detail: error.to_string(),
        }
    };
    let mut all = builtin().map_err(|error| catalog_error("built-in", error))?;
    if let Some(dir) = custom {
        let source = dir.display().to_string();
        let extra = load_custom_dir(dir).map_err(|error| catalog_error(&source, error))?;
        all.extend(extra);
    }
    let mut ids = BTreeSet::new();
    let mut executables = BTreeSet::new();
    for definition in &all {
        if !ids.insert(definition.id.clone()) {
            return Err(FabricConfigError::Duplicate {
                file: "harness catalog".to_owned(),
                key: "id".to_owned(),
                value: definition.id.clone(),
            });
        }
        for executable in &definition.executables {
            if !executables.insert(executable.clone()) {
                return Err(FabricConfigError::Duplicate {
                    file: "harness catalog".to_owned(),
                    key: "executables".to_owned(),
                    value: executable.clone(),
                });
            }
        }
    }
    all.sort_by(|left, right| left.id.cmp(&right.id));
    Ok(all)
}

fn parse_limits(path: &Path) -> Result<FabricLimits, FabricConfigError> {
    let file = path.display().to_string();
    let table = read_table(path)?;
    check_keys(&file, &table, LIMITS_KEYS)?;
    check_schema(&file, &table)?;
    let graph = GraphLimits {
        max_depth: bounded(
            &file,
            "max_depth",
            required_int(&file, &table, "max_depth")?,
        )?,
        max_children_per_parent: bounded(
            &file,
            "max_children_per_parent",
            required_int(&file, &table, "max_children_per_parent")?,
        )?,
    };
    let max_budget = BudgetSlice {
        rounds: bounded(&file, "rounds", required_int(&file, &table, "rounds")?)?,
        wall_seconds: bounded(
            &file,
            "wall_seconds",
            required_int(&file, &table, "wall_seconds")?,
        )?,
        tokens: bounded(&file, "tokens", required_int(&file, &table, "tokens")?)?,
        cost_usd: MicroUsd::new(bounded(
            &file,
            "cost_usd_micros",
            required_int(&file, &table, "cost_usd_micros")?,
        )?),
    };
    let isolation = parse_isolation(
        &file,
        "isolation",
        &required_str(&file, &table, "isolation")?,
    )?;
    let program = parse_program_ceilings(&file, &table)?;
    Ok(FabricLimits {
        graph,
        max_budget,
        isolation,
        program,
    })
}

/// The optional `program.run` ceilings. An absent key takes its documented
/// default; a present key must be an integer in range.
fn parse_program_ceilings(file: &str, table: &Table) -> Result<ProgramCeilings, FabricConfigError> {
    let timeout = match optional_int(file, table, PROGRAM_TIMEOUT_KEY)? {
        None => DEFAULT_PROGRAM_TIMEOUT,
        Some(value) => {
            let millis: u64 = bounded(file, PROGRAM_TIMEOUT_KEY, value)?;
            std::time::Duration::from_millis(millis)
        }
    };
    let max_tool_calls = match optional_int(file, table, PROGRAM_MAX_TOOL_CALLS_KEY)? {
        None => DEFAULT_PROGRAM_MAX_TOOL_CALLS,
        Some(value) => bounded(file, PROGRAM_MAX_TOOL_CALLS_KEY, value)?,
    };
    let max_parallel = match optional_int(file, table, PROGRAM_MAX_PARALLEL_KEY)? {
        None => DEFAULT_PROGRAM_MAX_PARALLEL_CALLS,
        Some(value) => bounded(file, PROGRAM_MAX_PARALLEL_KEY, value)?,
    };
    let max_result_bytes = match optional_int(file, table, PROGRAM_MAX_RESULT_BYTES_KEY)? {
        None => DEFAULT_PROGRAM_MAX_RESULT_BYTES,
        Some(value) => bounded(file, PROGRAM_MAX_RESULT_BYTES_KEY, value)?,
    };
    ProgramCeilings::new(timeout, max_tool_calls, max_parallel, max_result_bytes)
        .map_err(|error| invalid(file, "program_*", error))
}

fn parse_profile(
    path: &Path,
    table: &Table,
    harnesses: &[HarnessDefinition],
) -> Result<ProfileSpec, FabricConfigError> {
    let file = path.display().to_string();
    check_keys(&file, table, PROFILE_KEYS)?;
    check_schema(&file, table)?;

    let id_text = required_str(&file, table, "id")?;
    let id = RunnerProfileId::new(&id_text).map_err(|error| invalid(&file, "id", error))?;

    let harness = required_str(&file, table, "harness")?;
    if !harnesses.iter().any(|definition| definition.id == harness) {
        return Err(FabricConfigError::UnknownHarness { file, harness });
    }

    let token_text = required_str(&file, table, "token")?;
    let token = RunnerToken::parse(&token_text).ok_or_else(|| {
        invalid(
            &file,
            "token",
            format!("'{token_text}' names no runner this composition fronts"),
        )
    })?;

    let command = optional_str(&file, table, "command")?;
    match (&command, token.needs_command()) {
        (None, true) => return Err(missing(&file, "command")),
        (Some(_), false) => {
            return Err(invalid(
                &file,
                "command",
                "this runner token takes no command template",
            ));
        }
        (Some(template), true) => {
            token
                .probe_command(template)
                .map_err(|detail| FabricConfigError::ProbeFailed {
                    file: file.clone(),
                    key: "command".to_owned(),
                    detail,
                })?;
        }
        (None, false) => {}
    }

    let scenario = optional_str(&file, table, "scenario")?;
    match (&scenario, token.needs_scenario()) {
        (None, true) => return Err(missing(&file, "scenario")),
        (Some(_), false) => {
            return Err(invalid(
                &file,
                "scenario",
                "this runner token takes no scenario",
            ));
        }
        _ => {}
    }

    let isolation = match optional_str(&file, table, "isolation")? {
        Some(text) => parse_isolation(&file, "isolation", &text)?,
        None => token.default_isolation(),
    };
    let max_depth = match optional_int(&file, table, "max_depth")? {
        Some(value) => bounded(&file, "max_depth", value)?,
        None => generic_cli_runner::config::DEFAULT_MAX_DEPTH,
    };
    let mcp_bridge = optional_bool(&file, table, "mcp_bridge")?.unwrap_or(false);
    if mcp_bridge && token != RunnerToken::Recursive {
        return Err(invalid(
            &file,
            "mcp_bridge",
            "only the recursive runner token takes an MCP bridge",
        ));
    }

    Ok(ProfileSpec {
        id,
        harness,
        token,
        provider: optional_str(&file, table, "provider")?,
        model: optional_str(&file, table, "model")?,
        command,
        scenario,
        workspace_mutation: optional_bool(&file, table, "workspace_mutation")?.unwrap_or(false),
        isolation,
        max_depth,
        mcp_bridge,
    })
}

fn parse_role(path: &Path, table: &Table) -> Result<(RoleId, RoleGrant), FabricConfigError> {
    let file = path.display().to_string();
    check_keys(&file, table, ROLE_KEYS)?;
    check_schema(&file, table)?;

    let role_text = required_str(&file, table, "role")?;
    let role = RoleId::from_str(&role_text).map_err(|error| invalid(&file, "role", error))?;

    let workspace = required_str(&file, table, "workspace")?;
    if workspace != WORKSPACE_FRESH {
        return Err(invalid(
            &file,
            "workspace",
            format!("'{workspace}' is not implemented; only '{WORKSPACE_FRESH}' is"),
        ));
    }

    let grant = RoleGrant {
        context: ContextPolicy {
            allow: context_kinds(&file, table, "allow")?,
            deny: context_kinds(&file, table, "deny")?,
        },
        permissions: PermissionSetRef::new(&required_str(&file, table, "permissions")?)
            .map_err(|error| invalid(&file, "permissions", error))?,
        workspace: WorkspacePolicy::Fresh,
        may_spawn: optional_bool(&file, table, "may_spawn")?.unwrap_or(false),
    };
    Ok((role, grant))
}

fn context_kinds(
    file: &str,
    table: &Table,
    key: &str,
) -> Result<BTreeSet<ContextItemKind>, FabricConfigError> {
    let mut kinds = BTreeSet::new();
    for token in str_array(file, table, key)? {
        let kind = ContextItemKind::parse(&token)
            .ok_or_else(|| invalid(file, key, format!("'{token}' is not a context kind")))?;
        kinds.insert(kind);
    }
    Ok(kinds)
}

/// The regular files of a configuration directory, sorted by path. A missing
/// directory holds no files; any entry that is not a TOML file is refused.
fn config_files(dir: &Path) -> Result<Vec<PathBuf>, FabricConfigError> {
    let file = dir.display().to_string();
    let meta = match fs::symlink_metadata(dir) {
        Ok(meta) => meta,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => return Err(unreadable(&file, &error)),
    };
    if !meta.is_dir() {
        return Err(FabricConfigError::NotRegular { file });
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(dir).map_err(|error| unreadable(&file, &error))? {
        let path = entry.map_err(|error| unreadable(&file, &error))?.path();
        let is_toml = path.extension().and_then(|ext| ext.to_str()) == Some(CONFIG_EXTENSION);
        if !is_toml {
            return Err(FabricConfigError::UnexpectedEntry {
                file: path.display().to_string(),
            });
        }
        paths.push(path);
    }
    paths.sort();
    Ok(paths)
}

/// Reads one configuration file: refuses symlinks and non-files, bounds the
/// read, then parses the subset.
fn read_table(path: &Path) -> Result<Table, FabricConfigError> {
    let file = path.display().to_string();
    let meta = fs::symlink_metadata(path).map_err(|error| unreadable(&file, &error))?;
    if !meta.is_file() {
        return Err(FabricConfigError::NotRegular { file });
    }
    let handle = fs::File::open(path).map_err(|error| unreadable(&file, &error))?;
    let mut bytes = Vec::new();
    handle
        .take(MAX_CONFIG_BYTES + 1)
        .read_to_end(&mut bytes)
        .map_err(|error| unreadable(&file, &error))?;
    if bytes.len() as u64 > MAX_CONFIG_BYTES {
        return Err(FabricConfigError::Oversized {
            file,
            bytes: meta.len(),
        });
    }
    let text = String::from_utf8(bytes).map_err(|error| FabricConfigError::Syntax {
        file: file.clone(),
        detail: format!("not valid UTF-8: {error}"),
    })?;
    let parsed = Parser::new(&text).map_err(|error| FabricConfigError::Syntax {
        file: file.clone(),
        detail: format!("line {}: {}", error.line, error.kind),
    })?;
    parsed.parse().map_err(|error| FabricConfigError::Syntax {
        file,
        detail: format!("line {}: {}", error.line, error.kind),
    })
}

fn check_keys(file: &str, table: &Table, allowed: &[&str]) -> Result<(), FabricConfigError> {
    match table.keys().find(|key| !allowed.contains(&key.as_str())) {
        Some(key) => Err(FabricConfigError::UnknownKey {
            file: file.to_owned(),
            key: key.clone(),
        }),
        None => Ok(()),
    }
}

fn check_schema(file: &str, table: &Table) -> Result<(), FabricConfigError> {
    match table.get(SCHEMA_KEY) {
        Some(Value::Int(version)) if *version == SCHEMA_VERSION => Ok(()),
        Some(_) => Err(invalid(file, SCHEMA_KEY, "only schema = 1 is accepted")),
        None => Err(missing(file, SCHEMA_KEY)),
    }
}

fn required_str(file: &str, table: &Table, key: &str) -> Result<String, FabricConfigError> {
    optional_str(file, table, key)?.ok_or_else(|| missing(file, key))
}

fn optional_str(file: &str, table: &Table, key: &str) -> Result<Option<String>, FabricConfigError> {
    match table.get(key) {
        None => Ok(None),
        Some(Value::Str(text)) if !text.trim().is_empty() => Ok(Some(text.clone())),
        Some(_) => Err(invalid(file, key, "expected a non-empty string")),
    }
}

fn optional_bool(file: &str, table: &Table, key: &str) -> Result<Option<bool>, FabricConfigError> {
    match table.get(key) {
        None => Ok(None),
        Some(Value::Bool(flag)) => Ok(Some(*flag)),
        Some(_) => Err(invalid(file, key, "expected true or false")),
    }
}

fn optional_int(file: &str, table: &Table, key: &str) -> Result<Option<i64>, FabricConfigError> {
    match table.get(key) {
        None => Ok(None),
        Some(Value::Int(value)) => Ok(Some(*value)),
        Some(_) => Err(invalid(file, key, "expected an integer")),
    }
}

fn required_int(file: &str, table: &Table, key: &str) -> Result<i64, FabricConfigError> {
    optional_int(file, table, key)?.ok_or_else(|| missing(file, key))
}

fn str_array(file: &str, table: &Table, key: &str) -> Result<Vec<String>, FabricConfigError> {
    match table.get(key) {
        None => Ok(Vec::new()),
        Some(Value::Array(items)) => items
            .iter()
            .map(|item| match item {
                Value::Str(text) => Ok(text.clone()),
                _ => Err(invalid(file, key, "expected an array of strings")),
            })
            .collect(),
        Some(_) => Err(invalid(file, key, "expected an array of strings")),
    }
}

fn bounded<T: TryFrom<i64>>(file: &str, key: &str, value: i64) -> Result<T, FabricConfigError> {
    T::try_from(value)
        .map_err(|_| invalid(file, key, format!("{value} is outside the accepted range")))
}

fn parse_isolation(file: &str, key: &str, text: &str) -> Result<Isolation, FabricConfigError> {
    match text {
        "none" => Ok(Isolation::None),
        "process" => Ok(Isolation::Process),
        "container" => Ok(Isolation::Container),
        "vm" => Ok(Isolation::Vm),
        other => Err(invalid(
            file,
            key,
            format!("'{other}' is not an isolation level (none, process, container, vm)"),
        )),
    }
}

fn invalid(file: &str, key: &str, detail: impl std::fmt::Display) -> FabricConfigError {
    FabricConfigError::InvalidValue {
        file: file.to_owned(),
        key: key.to_owned(),
        detail: detail.to_string(),
    }
}

fn missing(file: &str, key: &str) -> FabricConfigError {
    FabricConfigError::MissingKey {
        file: file.to_owned(),
        key: key.to_owned(),
    }
}

fn duplicate(path: &Path, key: &str, value: String) -> FabricConfigError {
    FabricConfigError::Duplicate {
        file: path.display().to_string(),
        key: key.to_owned(),
        value,
    }
}

fn unreadable(file: &str, error: &io::Error) -> FabricConfigError {
    FabricConfigError::Unreadable {
        file: file.to_owned(),
        detail: error.to_string(),
    }
}

#[cfg(test)]
#[allow(clippy::arc_with_non_send_sync)] // the test clock and runners are single-threaded
mod tests {
    use super::*;
    use gauntlet_app::fabric::selection::{Preference, Requirements, select};
    use gauntlet_domain::EpochSeconds;
    use std::time::Duration;

    const CUSTOM_HARNESS_DIR: &str = "harnesses";
    const STATE_DIR_NAME: &str = "gauntlet-fabric-compose-state";
    const ALPHA_HARNESS: &str = "fabric-alpha";
    const BETA_HARNESS: &str = "fabric-beta";
    const ALPHA_EXECUTABLE: &str = "fabric-alpha-exe";
    const BETA_EXECUTABLE: &str = "fabric-beta-exe";
    const ALPHA_CLI: &str = "alpha-cli";
    const ALPHA_INTERACTIVE: &str = "alpha-interactive";
    const BETA_RECURSIVE: &str = "beta-recursive";
    const BETA_SCRIPTED: &str = "beta-scripted";
    const FABRIC_ROLE: &str = "fabric-reviewer";
    const FABRIC_PERMISSIONS: &str = "fabric-permissions";
    const GOOD_TEMPLATE: &str = "agent {prompt_file} {result_path}";
    const BAD_TEMPLATE: &str = "agent {unknown_placeholder}";
    const LIMIT_DEPTH: u32 = 3;
    const LIMIT_CHILDREN: u32 = 4;
    const LIMIT_ROUNDS: u32 = 5;
    const LIMIT_WALL: u64 = 600;
    const LIMIT_TOKENS: u64 = 7000;
    const LIMIT_COST: u64 = 8100;

    /// A scratch root under the temp directory, fresh for this test.
    fn scratch(tag: &str) -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "gauntlet-fabric-compose-{tag}-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("the scratch root is creatable");
        root
    }

    fn write_named(root: &Path, dir: &str, name: &str, body: &str) {
        let directory = root.join(dir);
        fs::create_dir_all(&directory).expect("the config directory is creatable");
        fs::write(directory.join(name), body).expect("the config file is writable");
    }

    fn write_profile(root: &Path, id: &str, body: &str) {
        write_named(root, PROFILES_DIR, &format!("{id}.toml"), body);
    }

    fn harness_toml(id: &str, executable: &str) -> String {
        format!(
            "schema = 1\nid = \"{id}\"\nlabel = \"{id} label\"\nexecutables = [\"{executable}\"]\n\n\
             [launch]\nargs = []\nprompt_mode = \"unknown\"\n\n\
             [session]\nidentity = \"none\"\n\n\
             [lifecycle]\nsource = \"screen\"\n\n\
             [control]\ninterrupt = \"none\"\n\n\
             [capabilities]\ninteractive = false\nresumable = false\nmodel_selection = false\n\
             native_subagents = false\nstructured_events = false\nsupport_level = 0\n"
        )
    }

    fn limits_toml() -> String {
        format!(
            "schema = 1\nmax_depth = {LIMIT_DEPTH}\nmax_children_per_parent = {LIMIT_CHILDREN}\n\
             rounds = {LIMIT_ROUNDS}\nwall_seconds = {LIMIT_WALL}\ntokens = {LIMIT_TOKENS}\n\
             cost_usd_micros = {LIMIT_COST}\nisolation = \"process\"\n"
        )
    }

    /// Limits and the two custom harness definitions, with no profiles.
    fn base(tag: &str) -> PathBuf {
        let root = scratch(tag);
        write_named(&root, "", LIMITS_FILE, &limits_toml());
        write_named(
            &root,
            CUSTOM_HARNESS_DIR,
            "fabric-alpha.toml",
            &harness_toml(ALPHA_HARNESS, ALPHA_EXECUTABLE),
        );
        write_named(
            &root,
            CUSTOM_HARNESS_DIR,
            "fabric-beta.toml",
            &harness_toml(BETA_HARNESS, BETA_EXECUTABLE),
        );
        fs::create_dir_all(root.join(PROFILES_DIR)).expect("profiles dir");
        fs::create_dir_all(root.join(ROLES_DIR)).expect("roles dir");
        root
    }

    /// The four-profile default: two of one harness, one scripted, one recursive.
    fn default_profile_texts() -> Vec<(String, String)> {
        let command = format!("command = \"{GOOD_TEMPLATE}\"\n");
        vec![
            (
                format!("{ALPHA_CLI}.toml"),
                profile(
                    ALPHA_CLI,
                    ALPHA_HARNESS,
                    RunnerToken::CommandTemplate,
                    &format!("{command}provider = \"fabric-provider\"\nmodel = \"fabric-model\"\n"),
                ),
            ),
            (
                format!("{ALPHA_INTERACTIVE}.toml"),
                profile(
                    ALPHA_INTERACTIVE,
                    ALPHA_HARNESS,
                    RunnerToken::Interactive,
                    &command,
                ),
            ),
            (
                format!("{BETA_RECURSIVE}.toml"),
                profile(BETA_RECURSIVE, BETA_HARNESS, RunnerToken::Recursive, ""),
            ),
            (
                format!("{BETA_SCRIPTED}.toml"),
                profile(
                    BETA_SCRIPTED,
                    BETA_HARNESS,
                    RunnerToken::Scripted,
                    "scenario = \"happy\"\n",
                ),
            ),
        ]
    }

    fn fixture(tag: &str) -> PathBuf {
        let root = base(tag);
        for (name, body) in default_profile_texts() {
            write_named(&root, PROFILES_DIR, &name, &body);
        }
        root
    }

    /// A profile document. The token string comes from the adapter constant.
    fn profile(id: &str, harness: &str, token: RunnerToken, extra: &str) -> String {
        format!(
            "schema = 1\nid = \"{id}\"\nharness = \"{harness}\"\ntoken = \"{}\"\n{extra}",
            token.token()
        )
    }

    fn role_toml(allow: &[&str], deny: &[&str]) -> String {
        let list = |items: &[&str]| {
            let quoted: Vec<String> = items.iter().map(|item| format!("\"{item}\"")).collect();
            format!("[{}]", quoted.join(", "))
        };
        format!(
            "schema = 1\nrole = \"{FABRIC_ROLE}\"\nmay_spawn = false\nallow = {}\ndeny = {}\n\
             permissions = \"{FABRIC_PERMISSIONS}\"\n\
             workspace = \"fresh\"\n",
            list(allow),
            list(deny)
        )
    }

    /// A lookup that finds exactly the named executables.
    struct FixedLookup(Vec<String>);

    impl ExecutableLookup for FixedLookup {
        fn locate(&self, executable: &str) -> Option<PathBuf> {
            self.0
                .iter()
                .any(|name| name == executable)
                .then(|| PathBuf::from(executable))
        }
    }

    fn load(root: &Path) -> Result<FabricConfig, FabricConfigError> {
        load_fabric(root, Some(&root.join(CUSTOM_HARNESS_DIR)))
    }

    fn compose(config: &FabricConfig, found: &[&str]) -> Result<ComposedFabric, CompositionError> {
        compose_with_launch(config, found, None)
    }

    fn compose_with_launch(
        config: &FabricConfig,
        found: &[&str],
        mcp_serve: Option<McpServeLaunch>,
    ) -> Result<ComposedFabric, CompositionError> {
        let lookup = FixedLookup(found.iter().map(|name| (*name).to_owned()).collect());
        let clock = mock_runner::DeterministicClock::start_at(EpochSeconds::new(0));
        let deps = FabricDeps {
            state_dir: std::env::temp_dir().join(STATE_DIR_NAME),
            clock: Arc::new(clock),
            containment: None,
            path_lookup: &lookup,
            mcp_serve,
        };
        compose_fabric(config, &deps)
    }

    const BRIDGED_RECURSIVE: &str = "bridged-recursive";

    fn launch_program() -> PathBuf {
        std::env::temp_dir().join("bridge-bin").join("gauntlet")
    }

    fn serve_launch() -> McpServeLaunch {
        McpServeLaunch {
            program: launch_program(),
            args: vec!["fabric".to_owned(), "serve".to_owned()],
        }
    }

    /// `mcp_bridge` is a strict optional key: a recursive profile may enable
    /// it, and every other runner token refuses it at load time.
    #[test]
    fn mcp_bridge_is_recursive_only() {
        let root = fixture("mcp-bridge-strict");
        write_profile(
            &root,
            "bridged-cli",
            &profile(
                "bridged-cli",
                ALPHA_HARNESS,
                RunnerToken::CommandTemplate,
                &format!("command = \"{GOOD_TEMPLATE}\"\nmcp_bridge = true\n"),
            ),
        );
        let error = load(&root).expect_err("a bridge on a command runner is refused");
        assert!(error.to_string().contains("mcp_bridge"), "{error}");
    }

    /// A bridge-enabled recursive profile is registered as unavailable in a
    /// process with no serve launch, so selection refuses it there; the rest of
    /// the composition still builds (admit-root needs no runners).
    #[test]
    fn a_bridged_profile_without_a_serve_launch_is_unavailable() {
        let root = fixture("mcp-bridge-unavailable");
        write_profile(
            &root,
            BRIDGED_RECURSIVE,
            &profile(
                BRIDGED_RECURSIVE,
                BETA_HARNESS,
                RunnerToken::Recursive,
                "mcp_bridge = true\n",
            ),
        );
        let config = load(&root).expect("the fixture loads");
        let composed = compose(&config, &[ALPHA_EXECUTABLE, BETA_EXECUTABLE])
            .expect("composition still succeeds without a launch");
        let bridged = composed
            .report
            .iter()
            .find(|r| r.id.to_string() == BRIDGED_RECURSIVE)
            .expect("the bridged profile is reported");
        assert!(matches!(
            &bridged.availability,
            Availability::Unavailable { reason } if reason.contains("MCP bridge")
        ));
        let pinned = Preference {
            pin: Some(RunnerProfileId::new(BRIDGED_RECURSIVE).expect("valid id")),
            ..Preference::default()
        };
        assert!(select(&composed.registry, &Requirements::default(), &pinned).is_err());

        let with_launch = compose_with_launch(
            &config,
            &[ALPHA_EXECUTABLE, BETA_EXECUTABLE],
            Some(serve_launch()),
        )
        .expect("composes with a launch");
        let launched = with_launch
            .report
            .iter()
            .find(|r| r.id.to_string() == BRIDGED_RECURSIVE)
            .expect("the bridged profile is reported");
        assert_eq!(launched.availability, Availability::Available);
    }

    /// The Prime configuration carries the bridge only for a profile that
    /// enables it, and only with a serve launch present. The launch's argv
    /// prefix is passed through exactly; the runner appends the caller.
    #[test]
    fn the_prime_config_takes_the_bridge_only_when_enabled_and_launched() {
        let root = fixture("mcp-bridge-config");
        write_profile(
            &root,
            BRIDGED_RECURSIVE,
            &profile(
                BRIDGED_RECURSIVE,
                BETA_HARNESS,
                RunnerToken::Recursive,
                "mcp_bridge = true\n",
            ),
        );
        let config = load(&root).expect("the fixture loads");
        let spec_of = |id: &str| {
            config
                .profiles
                .iter()
                .find(|profile| profile.id.to_string() == id)
                .expect("the profile is loaded")
                .clone()
        };
        let lookup = FixedLookup(Vec::new());
        let deps = FabricDeps {
            state_dir: std::env::temp_dir().join(STATE_DIR_NAME),
            clock: Arc::new(mock_runner::DeterministicClock::start_at(
                EpochSeconds::new(0),
            )),
            containment: None,
            path_lookup: &lookup,
            mcp_serve: Some(serve_launch()),
        };
        let bridged = prime_adapter_config(&spec_of(BRIDGED_RECURSIVE), &deps);
        assert_eq!(
            bridged.mcp_bridge,
            Some(recursive_runner::McpBridge::new(
                launch_program(),
                vec!["fabric".to_owned(), "serve".to_owned()],
            ))
        );
        let plain = prime_adapter_config(&spec_of(BETA_RECURSIVE), &deps);
        assert_eq!(
            plain.mcp_bridge, None,
            "an unbridged profile stays unbridged"
        );

        let no_launch = FabricDeps {
            mcp_serve: None,
            ..deps
        };
        assert_eq!(
            prime_adapter_config(&spec_of(BRIDGED_RECURSIVE), &no_launch).mcp_bridge,
            None
        );
    }

    #[test]
    fn two_profiles_of_one_harness_and_one_of_another_load() {
        let root = fixture("load");
        let config = load(&root).expect("the default fixture loads");
        let ids: Vec<String> = config.profiles.iter().map(|p| p.id.to_string()).collect();
        assert_eq!(
            ids,
            vec![ALPHA_CLI, ALPHA_INTERACTIVE, BETA_RECURSIVE, BETA_SCRIPTED]
        );
        let alpha = config
            .profiles
            .iter()
            .filter(|p| p.harness == ALPHA_HARNESS)
            .count();
        assert_eq!(alpha, 2, "two profiles share the alpha harness");
        assert!(
            config.roles.is_empty(),
            "the default fixture carries no role"
        );
    }

    #[test]
    fn unknown_harness_fails_closed() {
        let root = base("unknown-harness");
        write_profile(
            &root,
            "p-one",
            &profile(
                "p-one",
                "fabric-missing",
                RunnerToken::Scripted,
                "scenario = \"happy\"\n",
            ),
        );
        let error = load(&root).expect_err("an unknown harness is refused");
        assert!(
            matches!(error, FabricConfigError::UnknownHarness { .. }),
            "{error}"
        );
    }

    #[test]
    fn unknown_key_fails() {
        let root = base("unknown-key");
        write_profile(
            &root,
            "p-one",
            &profile(
                "p-one",
                ALPHA_HARNESS,
                RunnerToken::Scripted,
                "scenario = \"happy\"\nbogus = 1\n",
            ),
        );
        let error = load(&root).expect_err("an unknown key is refused");
        assert!(
            matches!(&error, FabricConfigError::UnknownKey { key, .. } if key == "bogus"),
            "{error}"
        );
    }

    /// Declared `program.run` ceilings, each distinct from its default.
    const DECLARED_CAP: u64 = 2500;
    const DECLARED_CALLS: u32 = 3;
    const DECLARED_PARALLEL: u32 = 2;
    const DECLARED_RESULT: usize = 1024;

    #[test]
    fn program_ceilings_default_when_absent_and_take_declared_values_when_present() {
        let root = base("program-defaults");
        let config = load(&root).expect("the fabric loads without program keys");
        assert_eq!(
            config.limits.program.max_tool_calls(),
            DEFAULT_PROGRAM_MAX_TOOL_CALLS
        );
        assert_eq!(
            config.limits.program.max_parallel_calls(),
            DEFAULT_PROGRAM_MAX_PARALLEL_CALLS
        );
        assert_eq!(
            config.limits.program.max_result_bytes(),
            DEFAULT_PROGRAM_MAX_RESULT_BYTES
        );
        assert_eq!(config.limits.program.timeout(), DEFAULT_PROGRAM_TIMEOUT);

        let root = base("program-declared");
        write_named(
            &root,
            "",
            LIMITS_FILE,
            &format!(
                "{}program_timeout_ms = {DECLARED_CAP}\nprogram_max_tool_calls = {DECLARED_CALLS}\n\
                 program_max_parallel = {DECLARED_PARALLEL}\nprogram_max_result_bytes = {DECLARED_RESULT}\n",
                limits_toml()
            ),
        );
        let config = load(&root).expect("the declared program ceilings load");
        assert_eq!(
            config.limits.program.timeout(),
            Duration::from_millis(DECLARED_CAP)
        );
        assert_eq!(config.limits.program.max_tool_calls(), DECLARED_CALLS);
        assert_eq!(
            config.limits.program.max_parallel_calls(),
            DECLARED_PARALLEL
        );
        assert_eq!(config.limits.program.max_result_bytes(), DECLARED_RESULT);
    }

    #[test]
    fn an_out_of_range_program_ceiling_fails_closed() {
        let root = base("program-bad");
        write_named(
            &root,
            "",
            LIMITS_FILE,
            &format!("{}program_max_parallel = 0\n", limits_toml()),
        );
        let error = load(&root).expect_err("a zero parallel bound is refused");
        assert!(
            matches!(&error, FabricConfigError::InvalidValue { .. }),
            "{error}"
        );
        assert!(error.to_string().contains("max_parallel_calls"), "{error}");
    }

    #[test]
    fn duplicate_profile_id_fails() {
        let root = base("duplicate-id");
        write_named(
            &root,
            PROFILES_DIR,
            "first.toml",
            &profile(
                "same-id",
                ALPHA_HARNESS,
                RunnerToken::Scripted,
                "scenario = \"happy\"\n",
            ),
        );
        write_named(
            &root,
            PROFILES_DIR,
            "second.toml",
            &profile(
                "same-id",
                ALPHA_HARNESS,
                RunnerToken::Scripted,
                "scenario = \"happy\"\n",
            ),
        );
        let error = load(&root).expect_err("a duplicate id is refused");
        assert!(
            matches!(&error, FabricConfigError::Duplicate { key, .. } if key == "id"),
            "{error}"
        );
    }

    #[test]
    fn command_missing_for_a_process_token_fails() {
        let root = base("missing-command");
        write_profile(
            &root,
            "p-one",
            &profile("p-one", ALPHA_HARNESS, RunnerToken::CommandTemplate, ""),
        );
        let error = load(&root).expect_err("a missing command is refused");
        assert!(
            matches!(&error, FabricConfigError::MissingKey { key, .. } if key == "command"),
            "{error}"
        );
    }

    #[test]
    fn command_with_a_bad_placeholder_fails_the_template_probe() {
        let root = base("bad-placeholder");
        write_profile(
            &root,
            "p-one",
            &profile(
                "p-one",
                ALPHA_HARNESS,
                RunnerToken::CommandTemplate,
                &format!("command = \"{}\"\n", BAD_TEMPLATE),
            ),
        );
        let error = load(&root).expect_err("the probe refuses an unknown placeholder");
        assert!(
            matches!(error, FabricConfigError::ProbeFailed { .. }),
            "{error}"
        );
    }

    #[test]
    fn mock_without_a_scenario_fails() {
        let root = base("missing-scenario");
        write_profile(
            &root,
            "p-one",
            &profile("p-one", ALPHA_HARNESS, RunnerToken::Scripted, ""),
        );
        let error = load(&root).expect_err("a scripted profile needs a scenario");
        assert!(
            matches!(&error, FabricConfigError::MissingKey { key, .. } if key == "scenario"),
            "{error}"
        );
    }

    #[test]
    fn an_unavailable_executable_is_reported_and_select_refuses_it() {
        let root = fixture("unavailable");
        let config = load(&root).expect("the fixture loads");
        // Only the alpha executable is discoverable; the beta recursive
        // profile's executable is not.
        let composed = compose(&config, &[ALPHA_EXECUTABLE]).expect("composes");
        let beta = composed
            .report
            .iter()
            .find(|r| r.id.to_string() == BETA_RECURSIVE)
            .expect("the beta profile is reported");
        assert!(matches!(
            &beta.availability,
            Availability::Unavailable { reason } if reason.contains(BETA_EXECUTABLE)
        ));
        assert!(beta.render().contains("unavailable ("));

        let pinned = Preference {
            pin: Some(RunnerProfileId::new(BETA_RECURSIVE).expect("valid id")),
            ..Preference::default()
        };
        assert!(select(&composed.registry, &Requirements::default(), &pinned).is_err());

        let chosen = select(
            &composed.registry,
            &Requirements::default(),
            &Preference::default(),
        )
        .expect("an available sibling is selected");
        assert_ne!(chosen.instance.profile.id.to_string(), BETA_RECURSIVE);
        assert_eq!(chosen.instance.availability, Availability::Available);
    }

    #[test]
    fn a_level_zero_definition_stays_level_zero_whatever_the_token() {
        let root = fixture("level-zero");
        let config = load(&root).expect("the fixture loads");
        let composed = compose(&config, &[ALPHA_EXECUTABLE, BETA_EXECUTABLE]).expect("composes");
        assert_eq!(composed.registry.instances().len(), 4);
        for instance in composed.registry.instances() {
            assert_eq!(instance.capabilities.support_level, SupportLevel::Process);
            assert_eq!(
                instance.capabilities.lifecycle_source,
                LifecycleSource::Screen
            );
            assert!(instance.capabilities.fresh_context);
        }
    }

    #[test]
    fn an_absent_model_stays_unknown_in_the_report_and_the_facts() {
        let root = fixture("unknown-model");
        let config = load(&root).expect("the fixture loads");
        let composed = compose(&config, &[ALPHA_EXECUTABLE, BETA_EXECUTABLE]).expect("composes");
        let scripted = composed
            .report
            .iter()
            .find(|r| r.id.to_string() == BETA_SCRIPTED)
            .expect("reported");
        assert!(!scripted.model_known);
        assert!(scripted.render().contains("provider UNKNOWN model UNKNOWN"));
        let facts = &composed
            .registry
            .get(&RunnerProfileId::new(BETA_SCRIPTED).expect("valid id"))
            .expect("registered")
            .profile
            .model;
        assert_eq!(facts.model, None);
        assert_eq!(facts.provider, None);

        let known = composed
            .report
            .iter()
            .find(|r| r.id.to_string() == ALPHA_CLI)
            .expect("reported");
        assert!(known.model_known);
        assert!(known.render().contains("model fabric-model"));
    }

    #[test]
    fn a_roles_file_with_an_unknown_kind_token_fails() {
        let root = fixture("bad-kind");
        write_named(
            &root,
            ROLES_DIR,
            "reviewer.toml",
            &role_toml(&["not-a-kind"], &[]),
        );
        let error = load(&root).expect_err("an unknown kind is refused");
        assert!(
            matches!(&error, FabricConfigError::InvalidValue { key, .. } if key == "allow"),
            "{error}"
        );
    }

    #[test]
    fn role_allow_and_deny_produce_the_expected_context_policy() {
        let root = fixture("role-policy");
        write_named(
            &root,
            ROLES_DIR,
            "reviewer.toml",
            &role_toml(&["task-statement", "file-excerpt"], &["file-excerpt"]),
        );
        let config = load(&root).expect("the role loads");
        let composed = compose(&config, &[ALPHA_EXECUTABLE, BETA_EXECUTABLE]).expect("composes");
        let role: RoleId = FABRIC_ROLE.parse().expect("a valid custom role");
        let grant = composed
            .policy
            .roles
            .get(&role)
            .expect("the role is in the policy");
        assert!(grant.context.permits(ContextItemKind::TaskStatement));
        assert!(
            !grant.context.permits(ContextItemKind::FileExcerpt),
            "deny wins"
        );
        assert!(
            !grant.context.permits(ContextItemKind::ResultSchema),
            "absent from allow"
        );
        assert!(!grant.may_spawn);
        assert_eq!(composed.policy.limits.max_depth, LIMIT_DEPTH);
    }

    #[cfg(unix)]
    #[test]
    fn a_symlink_in_a_configuration_directory_is_rejected() {
        let root = fixture("symlink");
        let target = root.join(PROFILES_DIR).join("p-real.toml");
        write_profile(
            &root,
            "p-real",
            &profile(
                "p-real",
                ALPHA_HARNESS,
                RunnerToken::Scripted,
                "scenario = \"happy\"\n",
            ),
        );
        let link = root.join(PROFILES_DIR).join("p-link.toml");
        std::os::unix::fs::symlink(&target, &link).expect("the link is creatable");
        let error = load(&root).expect_err("a symlink is refused");
        assert!(
            matches!(error, FabricConfigError::NotRegular { .. }),
            "{error}"
        );
    }

    #[test]
    fn an_oversized_file_is_rejected() {
        let root = fixture("oversized");
        let padding = "#".repeat(usize::try_from(MAX_CONFIG_BYTES).unwrap_or(usize::MAX) + 1);
        write_named(&root, PROFILES_DIR, "huge.toml", &padding);
        let error = load(&root).expect_err("an oversized file is refused");
        assert!(
            matches!(error, FabricConfigError::Oversized { .. }),
            "{error}"
        );
    }

    #[test]
    fn ordering_is_deterministic_across_directory_listing_order() {
        let forward = base("order-forward");
        let reverse = base("order-reverse");
        // The same profiles, written in opposite orders.
        let profiles = default_profile_texts();
        for (name, body) in profiles.iter() {
            write_named(&forward, PROFILES_DIR, name, body);
        }
        for (name, body) in profiles.iter().rev() {
            write_named(&reverse, PROFILES_DIR, name, body);
        }
        write_named(
            &forward,
            ROLES_DIR,
            "reviewer.toml",
            &role_toml(&["task-statement"], &[]),
        );
        write_named(
            &reverse,
            ROLES_DIR,
            "reviewer.toml",
            &role_toml(&["task-statement"], &[]),
        );

        let a = load(&forward).expect("loads");
        let b = load(&reverse).expect("loads");
        assert_eq!(a.profiles, b.profiles);
        let ca = compose(&a, &[ALPHA_EXECUTABLE]).expect("composes");
        let cb = compose(&b, &[ALPHA_EXECUTABLE]).expect("composes");
        let ra: Vec<String> = ca.report.iter().map(ProfileReport::render).collect();
        let rb: Vec<String> = cb.report.iter().map(ProfileReport::render).collect();
        assert_eq!(ra, rb);
    }

    #[test]
    fn the_composed_set_is_bound_to_its_registry() {
        let root = fixture("runner-set");
        let config = load(&root).expect("the fixture loads");
        let composed = compose(&config, &[ALPHA_EXECUTABLE]).expect("composes");
        assert_eq!(composed.registry.instances().len(), composed.report.len());
        assert_eq!(
            composed.policy.limits.max_children_per_parent,
            LIMIT_CHILDREN
        );
    }

    /// A containment seam that wraps nothing. Its presence is what makes the
    /// command-template runner count as wired; no test runs a child through it.
    #[derive(Debug)]
    struct PassThroughContainment;

    impl generic_cli_runner::CommandContainment for PassThroughContainment {
        fn wrap(
            &self,
            _workspace: &gauntlet_domain::WorkspaceRef,
            _policy: &gauntlet_domain::SandboxPolicyRef,
            command: &str,
        ) -> Result<String, generic_cli_runner::ContainmentRefusal> {
            Ok(command.to_owned())
        }
    }

    fn compose_wired(
        config: &FabricConfig,
        found: &[&str],
        containment: Option<Arc<dyn generic_cli_runner::CommandContainment>>,
    ) -> Result<ComposedFabric, CompositionError> {
        let lookup = FixedLookup(found.iter().map(|name| (*name).to_owned()).collect());
        let clock = mock_runner::DeterministicClock::start_at(EpochSeconds::new(0));
        let deps = FabricDeps {
            state_dir: std::env::temp_dir().join(STATE_DIR_NAME),
            clock: Arc::new(clock),
            containment,
            path_lookup: &lookup,
            mcp_serve: None,
        };
        compose_fabric(config, &deps)
    }

    fn capability_isolation(composed: &ComposedFabric, profile: &str) -> Isolation {
        composed
            .registry
            .instances()
            .into_iter()
            .find(|instance| instance.profile.id.to_string() == profile)
            .expect("the profile is registered")
            .capabilities
            .isolation
    }

    /// (a) With no containment seam, every profile's capability isolation is
    /// capped to `None`, the gate's ceiling is `None`, and the report records
    /// a cap for each profile that declared above `None`.
    #[test]
    fn without_containment_every_profile_is_capped_to_none() {
        let root = fixture("isolation-uncontained");
        let config = load(&root).expect("the fixture loads");
        let composed = compose(&config, &[ALPHA_EXECUTABLE, BETA_EXECUTABLE]).expect("composes");

        for instance in composed.registry.instances() {
            assert_eq!(
                instance.capabilities.isolation,
                Isolation::None,
                "profile {} must not claim a tier no seam contains",
                instance.profile.id
            );
        }
        assert_eq!(composed.policy.isolation, Isolation::None);

        let capped: Vec<&str> = composed
            .isolation_caps
            .iter()
            .map(|cap| cap.profile.as_str())
            .collect();
        assert_eq!(capped, [ALPHA_CLI, ALPHA_INTERACTIVE, BETA_RECURSIVE]);
        for cap in &composed.isolation_caps {
            assert_eq!(cap.declared, Isolation::Process);
            assert_eq!(cap.effective, Isolation::None);
            assert!(cap.render().contains("capped to none"), "{}", cap.render());
        }
        let scripted = composed
            .report
            .iter()
            .find(|r| r.id.to_string() == BETA_SCRIPTED)
            .expect("the scripted profile is reported");
        assert_eq!(scripted.declared_isolation, Isolation::None);
        assert_eq!(scripted.isolation, Isolation::None);
    }

    /// (b) A command-template profile keeps its declared isolation when the
    /// containment seam is wired, and the gate's ceiling follows it.
    #[test]
    fn a_wired_command_template_profile_keeps_its_declared_isolation() {
        let root = fixture("isolation-wired-command");
        let config = load(&root).expect("the fixture loads");
        let seam: Arc<dyn generic_cli_runner::CommandContainment> =
            Arc::new(PassThroughContainment);
        let composed = compose_wired(&config, &[ALPHA_EXECUTABLE, BETA_EXECUTABLE], Some(seam))
            .expect("composes");

        assert_eq!(
            capability_isolation(&composed, ALPHA_CLI),
            Isolation::Process
        );
        assert_eq!(composed.policy.isolation, Isolation::Process);
        assert!(
            composed
                .isolation_caps
                .iter()
                .all(|cap| cap.profile.as_str() != ALPHA_CLI),
            "the wired command-template profile is not capped"
        );
    }

    /// (c) A non-command-template profile declared `process` is capped even
    /// when a containment seam exists: only the command-template runner wires it.
    #[test]
    fn a_non_command_template_profile_is_capped_even_with_a_seam() {
        let root = fixture("isolation-wired-other");
        let config = load(&root).expect("the fixture loads");
        let seam: Arc<dyn generic_cli_runner::CommandContainment> =
            Arc::new(PassThroughContainment);
        let composed = compose_wired(&config, &[ALPHA_EXECUTABLE, BETA_EXECUTABLE], Some(seam))
            .expect("composes");

        assert_eq!(
            capability_isolation(&composed, ALPHA_INTERACTIVE),
            Isolation::None
        );
        assert_eq!(
            capability_isolation(&composed, BETA_RECURSIVE),
            Isolation::None
        );
        let capped: Vec<&str> = composed
            .isolation_caps
            .iter()
            .map(|cap| cap.profile.as_str())
            .collect();
        assert!(capped.contains(&ALPHA_INTERACTIVE), "{capped:?}");
        assert!(capped.contains(&BETA_RECURSIVE), "{capped:?}");
        assert!(!capped.contains(&ALPHA_CLI), "{capped:?}");
    }
}
