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
//!   `allow` and `deny` (context kind tokens), `permissions`,
//!   `sandbox_policy`, and `workspace` (`fresh`, the only implemented policy).
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
use gauntlet_app::config::defaults;
use gauntlet_app::delegation::{
    AgentsPolicy, ContextItemKind, ContextPolicy, RoleGrant, RunnerSet, WorkspacePolicy,
};
use gauntlet_app::fabric::{
    Availability, FabricCapabilities, LifecycleSource, ModelFacts, RunnerInstance, RunnerProfile,
    RunnerRegistry, SupportLevel,
};
use gauntlet_domain::{
    HarnessId, MicroUsd, PermissionSetRef, RoleId, RunnerProfileId, SandboxPolicyRef,
};
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
];
const ROLE_KEYS: &[&str] = &[
    SCHEMA_KEY,
    "role",
    "may_spawn",
    "allow",
    "deny",
    "permissions",
    "sandbox_policy",
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
];

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

/// The graph limits, the per-spawn budget ceiling, and the isolation fact
/// the authority gate consumes (`limits.toml`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FabricLimits {
    pub graph: GraphLimits,
    pub max_budget: BudgetSlice,
    pub isolation: Isolation,
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
            "profile {} harness {} token {} availability {} support {} lifecycle {} executable {} provider {} model {}",
            self.id,
            self.harness,
            self.token,
            availability,
            self.support_level,
            self.lifecycle_source,
            self.executable.as_deref().unwrap_or("none"),
            self.provider.as_deref().unwrap_or(UNKNOWN),
            self.model.as_deref().unwrap_or(UNKNOWN),
        )
    }
}

/// The composed fabric: the registry, the runners bound to it, the affordance
/// policy, and the per-profile report.
pub struct ComposedFabric {
    pub registry: RunnerRegistry,
    pub runners: RunnerSet,
    pub policy: AgentsPolicy,
    pub report: Vec<ProfileReport>,
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
        let availability = availability(spec.token, executable.as_deref(), deps.path_lookup);
        let runner = build_runner(spec, &workspace_root, deps)?;
        let capabilities = capabilities(spec, definition);
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
        isolation: config.limits.isolation,
    };
    Ok(ComposedFabric {
        registry,
        runners,
        policy,
        report,
    })
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
            let config = recursive_runner::PrimeAdapterConfig::default();
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

fn capabilities(spec: &ProfileSpec, definition: &HarnessDefinition) -> FabricCapabilities {
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
        isolation: spec.isolation,
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
    Ok(FabricLimits {
        graph,
        max_budget,
        isolation,
    })
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
        sandbox_policy: SandboxPolicyRef::new(&required_str(&file, table, "sandbox_policy")?)
            .map_err(|error| invalid(&file, "sandbox_policy", error))?,
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
    const FABRIC_SANDBOX: &str = "fabric-sandbox";
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
             permissions = \"{FABRIC_PERMISSIONS}\"\nsandbox_policy = \"{FABRIC_SANDBOX}\"\n\
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
        let lookup = FixedLookup(found.iter().map(|name| (*name).to_owned()).collect());
        let clock = mock_runner::DeterministicClock::start_at(EpochSeconds::new(0));
        let deps = FabricDeps {
            state_dir: std::env::temp_dir().join(STATE_DIR_NAME),
            clock: Arc::new(clock),
            containment: None,
            path_lookup: &lookup,
        };
        compose_fabric(config, &deps)
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
}
