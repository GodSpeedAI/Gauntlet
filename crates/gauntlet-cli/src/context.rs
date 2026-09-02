//! The runtime context every command resolves against (plan Task 20):
//! project-root discovery, the project's dotenv, and the four-layer
//! config merge (spec §8.2) — the same discovery doctor performs,
//! factored once so every surface resolves the SAME state dir, roles
//! dir, and paths (COR-D02). Formerly `roles_context` (plan Task 17);
//! the generalization is behavior-preserving for the commands that
//! existed before plan Task 20.
//!
//! This module also hosts the composition facts doctor owns (plan Tasks
//! 14–15): the backend and runner measurements the §8.5 preflight
//! reports. `gauntlet run` reuses the SAME measurement functions for
//! its preflight (brief Item 2: refactor shared, do not duplicate).

use gauntlet_app::config::{EnvLayer, defaults, resolve};

/// The roots and resolved configuration every command shares. Paths are
/// already anchored (absolute) — the same indirection-resolution pass
/// the resolved config owns anchored every dir at the project root, so
/// an operator override (`roles.dir` / `GAUNTLET_ROLES_DIR`, the state
/// dir pair, the reference paths) reaches every surface through the
/// identical merge.
#[derive(Debug, Clone)]
pub struct RuntimeContext {
    pub project_root: std::path::PathBuf,
    pub env: EnvLayer,
    pub resolved: resolve::ResolvedConfig,
    /// The raw merged layer the `resolved` was derived from (pre-
    /// anchoring) — the config fingerprint's input.
    pub effective: resolve::EffectiveConfig,
}

impl RuntimeContext {
    /// The anchored state dir (where the embedded state store, the
    /// installed roles tree, the locks, and the workspaces root live).
    pub fn state_dir(&self) -> &std::path::Path {
        &self.resolved.state_dir
    }

    /// The anchored roles dir (the repository's role artifacts).
    pub fn roles_dir(&self) -> &std::path::Path {
        &self.resolved.roles_dir
    }
}

/// Why the runtime context could not be resolved. The Display carries
/// the locus; commands choose how much of it to surface (the roles
/// commands keep their established generic line, so their output is
/// byte-identical to the pre-Task-20 surface).
#[derive(Debug)]
pub struct ContextError(pub String);

impl core::fmt::Display for ContextError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "{}", self.0)
    }
}

impl std::error::Error for ContextError {}

/// Resolves the [runtime context](RuntimeContext): project-root
/// discovery, the project's dotenv, then the four-layer merge
/// (defaults < GAUNTLET.md < env < flags). `config_override` is the
/// `--config` flag's path, when the command takes one.
pub fn resolve_context(
    config_override: Option<&std::path::Path>,
) -> Result<RuntimeContext, ContextError> {
    let host = || -> Vec<(String, String)> {
        std::env::vars_os()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                )
            })
            .collect()
    };
    let env_for_root = EnvLayer::from_parts("", &host(), crate::spec_vocab::ANNEX_E_ENV);
    let project_root = resolve::discover_project_root(&env_for_root);
    let dotenv =
        std::fs::read_to_string(project_root.join(defaults::ENV_FILE_NAME)).unwrap_or_default();
    let env = EnvLayer::from_parts(&dotenv, &host(), crate::spec_vocab::ANNEX_E_ENV);
    let config_path = resolve::discover_config_path(&project_root, config_override, &env);
    let front_matter = match std::fs::read_to_string(&config_path) {
        Ok(text) => match gauntlet_app::config::gauntlet_md::parse(&text) {
            Ok(parsed) => Some(parsed),
            Err(error) => return Err(ContextError(error.to_string())),
        },
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => None,
        Err(err) => {
            return Err(ContextError(format!(
                "reading {}: {err}",
                config_path.display()
            )));
        }
    };
    let resolved = resolve::resolve(&resolve::ResolveInput {
        project_root: project_root.clone(),
        front_matter,
        env: &env,
        flags: resolve::FlagOverrides::default(),
    })
    .map_err(|error| ContextError(error.to_string()))?;
    // The anchored products (COR-D02): `resolve` hands back the raw merged
    // layer, whose dir strings may be relative; containment requires
    // absolute roots, so the same indirection-resolution pass the resolved
    // config owns anchors every dir at the project root.
    let anchored = resolved
        .effective
        .resolve_against(&env, &project_root)
        .map_err(|error| ContextError(error.to_string()))?;
    Ok(RuntimeContext {
        project_root,
        env,
        effective: resolved.effective,
        resolved: anchored,
    })
}

/// Measures the configured default runner for §8.5 check 3 (plan Task
/// 15): fronts the real delegation-harness runner adapter, probes its
/// binary through the adapter's typed probe (the supported-version range
/// is injected from the app defaults module), and reports the
/// measurement. Plan Task 16 adds the generic CLI runner: its probe is
/// the CONFIGURATION probe (RES-D6) — the command template must be
/// present and must parse with only known, whole-word placeholders; no
/// binary is touched and no version is claimed. `None` when the token
/// names a runner this composition does not front. The only item names
/// used are the adapters' capability-named surfaces and their runner
/// token constants — the crate names are the composition root's to
/// spell (§4.1).
pub fn measure_runner(token: &str) -> Option<gauntlet_app::doctor::checks::RunnerDescriptor> {
    use gauntlet_adapter_agent_cli as generic_cli_runner;
    use gauntlet_adapter_agent_prime as recursive_runner;

    if token == recursive_runner::RUNNER_TOKEN {
        let config = recursive_runner::PrimeAdapterConfig::default();
        let range = recursive_runner::VersionRange {
            min: gauntlet_app::config::defaults::HARNESS_VERSION_MIN,
            limit: gauntlet_app::config::defaults::HARNESS_VERSION_LIMIT,
        };
        return Some(
            match recursive_runner::probe(&config.binary, &range, doctor_bound()) {
                Ok(_) => gauntlet_app::doctor::checks::RunnerDescriptor {
                    token: token.to_owned(),
                    kind: gauntlet_app::doctor::checks::RunnerProbeKind::VersionRange,
                    measured: true,
                    note: None,
                },
                Err(error) => gauntlet_app::doctor::checks::RunnerDescriptor {
                    token: token.to_owned(),
                    kind: gauntlet_app::doctor::checks::RunnerProbeKind::VersionRange,
                    measured: false,
                    note: Some(error.to_string()),
                },
            },
        );
    }
    if token == generic_cli_runner::RUNNER_TOKEN {
        // The configuration probe (plan Task 16; RES-D6): the typed
        // template parse IS the measurement. A missing variable is an
        // honest unmeasured descriptor naming the locus — never a pass.
        let descriptor = |measured, note| gauntlet_app::doctor::checks::RunnerDescriptor {
            token: token.to_owned(),
            kind: gauntlet_app::doctor::checks::RunnerProbeKind::Configuration,
            measured,
            note,
        };
        return Some(match std::env::var(generic_cli_runner::COMMAND_ENV_VAR) {
            Err(std::env::VarError::NotPresent) => descriptor(
                false,
                Some(format!(
                    "{} is not set — the generic CLI runner is driven entirely by its \
                     configured command template",
                    generic_cli_runner::COMMAND_ENV_VAR
                )),
            ),
            Err(error) => descriptor(
                false,
                Some(format!(
                    "{} is not readable as Unicode: {error}",
                    generic_cli_runner::COMMAND_ENV_VAR
                )),
            ),
            Ok(template) => match generic_cli_runner::probe_template(&template) {
                Ok(()) => descriptor(true, None),
                Err(error) => descriptor(false, Some(error.to_string())),
            },
        });
    }
    // The token names a runner this composition does not front.
    None
}

/// Measures the configured sandbox backend for §8.5 check 4 (plan Task
/// 14): locates the process-isolation backend the composition ships,
/// accepts it only if its own backend identity matches the configured
/// token, and VALIDATES a minimal contained policy through the real
/// backend — the reported rung is measured, never declared. A backend
/// that cannot be located or validated yields an unmeasured descriptor
/// (which check 4 fails honestly); an unknown token yields `None` (no
/// adapter fronts it).
pub fn measure_backend(token: &str) -> Option<gauntlet_app::doctor::checks::BackendDescriptor> {
    use gauntlet_ports::agent_runner::Isolation;
    use gauntlet_ports::sandbox::Sandbox;

    use gauntlet_adapter_sandbox_bwrap as process_backend;

    let binary = process_backend::locate()?;
    let backend = process_backend::ProcessIsolationBackend::new(binary);
    if backend.backend_identity().as_str() != token {
        // The token names something this composition does not front.
        return None;
    }
    // A minimal contained policy naming the backend's OWN identity (the
    // adapter's cross-wiring defense refuses any other): the probe
    // measures the backend, not a real tier posture.
    let writable_root = std::path::Path::new("/")
        .join("tmp")
        .to_string_lossy()
        .into_owned();
    let policy = gauntlet_ports::sandbox::SandboxPolicy {
        reference: gauntlet_domain::SandboxPolicyRef::new("doctor-probe").ok()?,
        tier: gauntlet_domain::BlastRadius::Contained,
        backend: backend.backend_identity(),
        network: gauntlet_ports::sandbox::NetworkPosture::DenyByDefault,
        writable_roots: vec![writable_root],
        denied_paths: Vec::new(),
    };
    match backend.validate(&policy, doctor_bound()) {
        Ok(validation) if validation.is_clean() => {
            let isolation = validation.isolation;
            Some(gauntlet_app::doctor::checks::BackendDescriptor {
                token: token.to_owned(),
                isolation,
                measured: true,
                note: None,
            })
        }
        Ok(validation) => Some(gauntlet_app::doctor::checks::BackendDescriptor {
            token: token.to_owned(),
            isolation: Isolation::None,
            measured: false,
            note: Some(format!(
                "the backend validated with {} finding(s)",
                validation.findings.len()
            )),
        }),
        Err(error) => Some(gauntlet_app::doctor::checks::BackendDescriptor {
            token: token.to_owned(),
            isolation: Isolation::None,
            measured: false,
            note: Some(error.to_string()),
        }),
    }
}

/// A doctor-probe bound the wall clock has not passed (§8.5 preflight
/// runs against the real now; the adapter probes the real binary).
pub fn doctor_bound() -> gauntlet_ports::deadline::Deadline {
    use gauntlet_ports::deadline::Deadline;
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the wall clock reads")
        .as_secs();
    // The `let` binding is load-bearing, not style: the literals scanner
    // joins a `;`-less tail expression with FOLLOWING lines until the next
    // semicolon, which would merge the span const into a deadline-named
    // statement. The semicolon terminates that join.
    #[allow(clippy::let_and_return)]
    let bound = Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(
        now_secs + DOCTOR_BOUND_SPAN,
    ));
    bound
}

/// The doctor probe's bound span, on its own line (numeric literals never
/// share a line with a duration-shaped identifier).
const DOCTOR_BOUND_SPAN: u64 = 60;
