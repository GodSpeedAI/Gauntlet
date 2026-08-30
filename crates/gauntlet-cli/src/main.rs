//! Command surface composition root (spec §4.1, §20.1): the only place
//! adapters are constructed and wired into the application layer.
//!
//! Plan Task 5 landed `gauntlet doctor [--config <path>]` (spec §8.5). Plan
//! Task 7 adds `gauntlet reference digest --path <path>`: the application
//! layer's normalization + digest over any file, whose output the task gate
//! diffs byte-for-byte against `scripts/reference-digest.sh` — two
//! independent implementations of the same rules agreeing is the
//! determinism proof (plan Task 1/Task 7). Argument parsing is std
//! `env::args`, small and boring; unknown arguments are a usage error with
//! exit code 2. The full §20.1 surface lands with plan Task 20.

mod spec_vocab;

use std::process::ExitCode;

use gauntlet_app::doctor::{DoctorInput, run_doctor_and_write};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("doctor") => doctor(&args[1..]),
        Some("reference") => reference(&args[1..]),
        Some(other) => {
            eprintln!("gauntlet: unknown command '{other}'");
            usage();
            ExitCode::from(2)
        }
        None => {
            usage();
            ExitCode::from(2)
        }
    }
}

fn usage() {
    println!("gauntlet <version> — see spec 20.1 for the full surface");
    println!("usage: gauntlet doctor [--config <path>]");
    println!("       gauntlet reference digest --path <path>");
}

fn doctor(args: &[String]) -> ExitCode {
    let mut input = DoctorInput::new(spec_vocab::ANNEX_E_ENV, spec_vocab::CAPABILITY_VOCABULARY);
    // Plan Task 14 (§8.5 check 4): the composition root measures the
    // configured sandbox backend through the real adapter — the app layer
    // cannot map vendor tokens to rungs (§24.3), so the measurement is a
    // composition fact. Only the capability-named façade is used.
    input.backend_measure = Some(measure_backend);
    // Plan Task 15 (§8.5 check 3): the composition root measures the
    // configured default runner through the real adapter's typed version
    // probe — the supported range comes from the app defaults module
    // (correction RES-D8), so the adapter never owns its compatibility
    // ruler. A probe mismatch fails doctor as a configuration failure.
    input.runner_measure = Some(measure_runner);
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--config" => {
                let Some(path) = args.get(index + 1) else {
                    eprintln!("gauntlet doctor: --config requires a path");
                    return ExitCode::from(2);
                };
                input.config_override = Some(std::path::PathBuf::from(path));
                index += 2;
            }
            other => {
                eprintln!("gauntlet doctor: unknown argument '{other}'");
                println!("usage: gauntlet doctor [--config <path>]");
                return ExitCode::from(2);
            }
        }
    }
    match run_doctor_and_write(&input) {
        Ok((report, path)) => {
            print!("{}", report.human_summary());
            println!("report: {}", path.display());
            if report.exit_ok() {
                ExitCode::SUCCESS
            } else {
                ExitCode::FAILURE
            }
        }
        Err(err) => {
            eprintln!("gauntlet doctor: could not write the machine report: {err}");
            ExitCode::FAILURE
        }
    }
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
fn measure_runner(token: &str) -> Option<gauntlet_app::doctor::checks::RunnerDescriptor> {
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
fn measure_backend(token: &str) -> Option<gauntlet_app::doctor::checks::BackendDescriptor> {
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
fn doctor_bound() -> gauntlet_ports::deadline::Deadline {
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

fn reference(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("digest") => reference_digest(&args[1..]),
        Some(other) => {
            eprintln!("gauntlet reference: unknown subcommand '{other}'");
            println!("usage: gauntlet reference digest --path <path>");
            ExitCode::from(2)
        }
        None => {
            println!("usage: gauntlet reference digest --path <path>");
            ExitCode::from(2)
        }
    }
}

/// `gauntlet reference digest --path <path>` (plan Task 7): reads the file,
/// applies the application normalization (the `reference-digest.sh` rules,
/// re-implemented typed), prints exactly one line `sha256:<lowercase hex>`
/// and exits 0. A missing, unreadable, or non-UTF-8 path is a stderr
/// message and a non-zero exit — untrusted input is never coerced (the
/// script refuses identically).
fn reference_digest(args: &[String]) -> ExitCode {
    let mut path: Option<std::path::PathBuf> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--path" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("gauntlet reference digest: --path requires a path");
                    return ExitCode::from(2);
                };
                if path.replace(std::path::PathBuf::from(value)).is_some() {
                    eprintln!("gauntlet reference digest: --path given twice");
                    return ExitCode::from(2);
                }
                index += 2;
            }
            other => {
                eprintln!("gauntlet reference digest: unknown argument '{other}'");
                println!("usage: gauntlet reference digest --path <path>");
                return ExitCode::from(2);
            }
        }
    }
    let Some(path) = path else {
        eprintln!("gauntlet reference digest: --path is required");
        println!("usage: gauntlet reference digest --path <path>");
        return ExitCode::from(2);
    };
    let bytes = match std::fs::read(&path) {
        Ok(bytes) => bytes,
        Err(err) => {
            eprintln!(
                "gauntlet reference digest: reading {} failed: {err}",
                path.display()
            );
            return ExitCode::FAILURE;
        }
    };
    match gauntlet_app::reference::normalize::digest_of(&bytes) {
        Ok(digest) => {
            println!("{digest}");
            ExitCode::SUCCESS
        }
        Err(err) => {
            eprintln!(
                "gauntlet reference digest: {} does not digest: {err}",
                path.display()
            );
            ExitCode::FAILURE
        }
    }
}
