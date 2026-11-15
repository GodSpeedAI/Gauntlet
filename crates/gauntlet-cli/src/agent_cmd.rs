//! `gauntlet agent doctor | setup <adapter>` (WP-63): the optional,
//! opt-in management surface for the agent adapters. The CLI only
//! composes: detection, version compatibility and the installer
//! mechanics live in the adapter crate; the supported range and the pin
//! come from the application defaults module. Nothing here installs
//! anything without an explicit confirmation, and nothing here downloads.

use std::ffi::OsString;
use std::io::{BufRead, IsTerminal, Write};
use std::process::ExitCode;

use gauntlet_adapter_agent_cli as generic_cli_runner;
use gauntlet_adapter_agent_mock as mock_runner;
use gauntlet_adapter_agent_prime as recursive_runner;
use gauntlet_adapter_agent_tty as tty_runner;
use gauntlet_app::config::defaults;

/// The harness version range, from the single source of truth.
pub fn supported_range() -> recursive_runner::VersionRange {
    recursive_runner::VersionRange {
        min: defaults::HARNESS_VERSION_MIN,
        limit: defaults::HARNESS_VERSION_LIMIT,
    }
}

/// One adapter's status line.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AdapterRow {
    pub token: &'static str,
    pub state: RowState,
    pub detail: String,
}

/// An adapter's readiness. Only `Broken` is a problem: an optional
/// dependency that is simply not installed is `Missing`, never an error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RowState {
    Ready,
    Missing,
    Unconfigured,
    Broken,
}

impl RowState {
    fn label(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Missing => "not installed (optional)",
            Self::Unconfigured => "not configured",
            Self::Broken => "PROBLEM",
        }
    }
}

/// The inputs the report reads, explicit so tests need no process env.
#[derive(Debug, Clone, Default)]
pub struct ReportInputs {
    pub harness_binary: Option<OsString>,
    pub path_env: Option<OsString>,
    pub cli_command: Option<String>,
    pub tty_command: Option<String>,
}

impl ReportInputs {
    pub fn from_env() -> Self {
        Self {
            harness_binary: std::env::var_os(recursive_runner::managed::BINARY_ENV),
            path_env: std::env::var_os("PATH"),
            cli_command: std::env::var(generic_cli_runner::COMMAND_ENV_VAR).ok(),
            tty_command: std::env::var(tty_runner::COMMAND_ENV_VAR).ok(),
        }
    }
}

/// Builds one row per adapter. Pure over `inputs` (plus the bounded
/// `--version` run of the located harness binary).
pub fn adapter_report(inputs: &ReportInputs) -> Vec<AdapterRow> {
    let mut rows = vec![AdapterRow {
        token: mock_runner::RUNNER_TOKEN,
        state: RowState::Ready,
        detail: "built in; replays the scenario named by the mock-scenario hook".to_owned(),
    }];
    let range = supported_range();
    rows.push(
        match recursive_runner::managed::detect(
            inputs.harness_binary.as_deref(),
            inputs.path_env.as_deref(),
            &range,
            crate::context::doctor_bound(),
        ) {
            Ok(found) => AdapterRow {
                token: recursive_runner::RUNNER_TOKEN,
                state: RowState::Ready,
                detail: format!("{}; supported {range}", found.summary()),
            },
            Err(error) if error.is_absent() => AdapterRow {
                token: recursive_runner::RUNNER_TOKEN,
                state: RowState::Missing,
                detail: error.to_string(),
            },
            Err(error) => AdapterRow {
                token: recursive_runner::RUNNER_TOKEN,
                state: RowState::Broken,
                detail: error.to_string(),
            },
        },
    );
    rows.push(template_row(
        generic_cli_runner::RUNNER_TOKEN,
        generic_cli_runner::COMMAND_ENV_VAR,
        inputs.cli_command.as_deref(),
        |t| generic_cli_runner::probe_template(t).map_err(|e| e.to_string()),
    ));
    rows.push(template_row(
        tty_runner::RUNNER_TOKEN,
        tty_runner::COMMAND_ENV_VAR,
        inputs.tty_command.as_deref(),
        |t| tty_runner::probe_template(t).map_err(|e| e.to_string()),
    ));
    rows
}

fn template_row(
    token: &'static str,
    env: &'static str,
    command: Option<&str>,
    probe: impl Fn(&str) -> Result<(), String>,
) -> AdapterRow {
    match command {
        None => AdapterRow {
            token,
            state: RowState::Unconfigured,
            detail: format!("set {env} to a command template to use it"),
        },
        Some(template) => match probe(template) {
            Ok(()) => AdapterRow {
                token,
                state: RowState::Ready,
                detail: format!("command template from {env} parses"),
            },
            Err(detail) => AdapterRow {
                token,
                state: RowState::Broken,
                detail: format!("{env}: {detail}"),
            },
        },
    }
}

/// The report as text, one block per adapter.
pub fn render_report(rows: &[AdapterRow], default_runner: Option<&str>) -> String {
    let mut out = String::from("agent adapters:\n");
    for row in rows {
        out.push_str(&format!(
            "  {}: {} - {}\n",
            row.token,
            row.state.label(),
            row.detail
        ));
    }
    out.push_str(&format!(
        "default runner: {}\n",
        default_runner.unwrap_or("(not set)")
    ));
    out
}

/// `gauntlet agent ...`.
pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("doctor") if args.len() == 1 => doctor(),
        Some("setup") => setup(&args[1..]),
        _ => usage_error(),
    }
}

fn usage_error() -> ExitCode {
    eprintln!("gauntlet agent: unknown or missing subcommand");
    println!("usage: gauntlet agent doctor");
    println!("       gauntlet agent setup <adapter> [--installer <path> --sha256 <hex>] [--yes]");
    ExitCode::from(2)
}

fn doctor() -> ExitCode {
    let rows = adapter_report(&ReportInputs::from_env());
    let default_runner = std::env::var("GAUNTLET_AGENT_RUNNER").ok();
    print!("{}", render_report(&rows, default_runner.as_deref()));
    if rows.iter().any(|row| row.state == RowState::Broken) {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// How setup was confirmed.
fn confirmed(yes: bool, input: &mut dyn BufRead, interactive: bool) -> bool {
    if yes {
        return true;
    }
    if !interactive {
        return false;
    }
    print!("Run the installer now? Type 'yes' to continue: ");
    let _ = std::io::stdout().flush();
    let mut line = String::new();
    input.read_line(&mut line).is_ok() && line.trim().eq_ignore_ascii_case("yes")
}

fn setup(args: &[String]) -> ExitCode {
    let Some(name) = args.first() else {
        return usage_error();
    };
    if !recursive_runner::managed::is_setup_name(name) {
        eprintln!(
            "gauntlet agent setup: '{name}' has no managed setup (only optional external dependencies do)"
        );
        return ExitCode::from(2);
    }
    let mut installer: Option<std::path::PathBuf> = None;
    let mut sha256: Option<String> = None;
    let mut yes = false;
    let mut index = 1;
    while index < args.len() {
        match args[index].as_str() {
            "--yes" => {
                yes = true;
                index += 1;
            }
            "--installer" | "--sha256" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("gauntlet agent setup: {} requires a value", args[index]);
                    return ExitCode::from(2);
                };
                if args[index] == "--installer" {
                    installer = Some(value.into());
                } else {
                    sha256 = Some(value.clone());
                }
                index += 2;
            }
            other => {
                eprintln!("gauntlet agent setup: unknown argument '{other}'");
                return ExitCode::from(2);
            }
        }
    }
    let range = supported_range();
    let pinned = defaults::HARNESS_VERSION_PINNED;
    let Some(installer) = installer else {
        // No installer supplied: print the manual steps. Nothing runs.
        println!(
            "{}",
            recursive_runner::managed::manual_instructions(pinned, &range)
        );
        return ExitCode::SUCCESS;
    };
    let Some(sha256) = sha256 else {
        eprintln!(
            "gauntlet agent setup: --installer requires --sha256 <hex> (the digest you recorded when you reviewed it)"
        );
        return ExitCode::from(2);
    };
    if let Err(error) = recursive_runner::managed::verify_installer(&installer, &sha256) {
        eprintln!("gauntlet agent setup: {error}");
        return ExitCode::FAILURE;
    }
    let prefix = match recursive_runner::managed::user_prefix(std::env::var_os("HOME").as_deref()) {
        Ok(prefix) => prefix,
        Err(error) => {
            eprintln!("gauntlet agent setup: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!(
        "Installer verified. It will install version {}.{}.{} into {} and may contact the network.",
        pinned.0,
        pinned.1,
        pinned.2,
        prefix.display()
    );
    let stdin = std::io::stdin();
    if !confirmed(yes, &mut stdin.lock(), stdin.is_terminal()) {
        eprintln!(
            "gauntlet agent setup: not confirmed; nothing was installed (pass --yes to confirm non-interactively)"
        );
        return ExitCode::from(2);
    }
    match recursive_runner::managed::run_installer(&installer, pinned, &prefix) {
        Ok(()) => {
            println!("Installed. Run `gauntlet agent doctor` to verify.");
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("gauntlet agent setup: {error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_pin_sits_inside_the_supported_range() {
        let range = supported_range();
        assert!(recursive_runner::is_supported(
            defaults::HARNESS_VERSION_PINNED,
            &range
        ));
    }

    #[test]
    fn render_names_the_default_runner() {
        let text = render_report(&[], Some("mock-agent"));
        assert!(text.contains("default runner: mock-agent"));
    }

    #[test]
    fn confirmation_requires_yes_flag_or_an_interactive_yes() {
        let mut none: &[u8] = b"";
        assert!(!confirmed(false, &mut none, false));
        assert!(confirmed(true, &mut none, false));
        let mut no: &[u8] = b"no\n";
        assert!(!confirmed(false, &mut no, true));
        let mut yes: &[u8] = b"YES\n";
        assert!(confirmed(false, &mut yes, true));
    }
}
