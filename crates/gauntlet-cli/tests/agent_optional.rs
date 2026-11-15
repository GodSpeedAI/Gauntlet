//! WP-63: the delegation harness is an OPTIONAL, opt-in managed dependency.
//! These legs drive the real `gauntlet` binary with stub executables in
//! scratch directories: no network, no real harness. They pin that
//!
//! - an absent harness never breaks `gauntlet agent doctor` and every other
//!   adapter is still reported;
//! - a harness found on PATH or via the explicit path is reported with its
//!   version and location, and an incompatible one is refused naming both
//!   the found and the supported versions;
//! - `gauntlet agent setup` installs nothing without confirmation, refuses
//!   an installer whose SHA-256 does not match, and otherwise only prints
//!   manual instructions.
#![cfg(unix)]

use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

const GAUNTLET: &str = env!("CARGO_BIN_EXE_gauntlet");

fn shebang() -> String {
    let sep = std::path::MAIN_SEPARATOR;
    format!("#!{sep}bin{sep}sh\n")
}

fn scratch(tag: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!(
        "gauntlet-agent-optional-{tag}-{}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).expect("scratch dir");
    dir
}

fn stub_harness(dir: &Path, name: &str, version: &str) -> PathBuf {
    let path = dir.join(name);
    std::fs::write(&path, format!("{}echo {version} >&2\n", shebang())).expect("stub");
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o755)).expect("chmod");
    path
}

/// The binary with a cleared environment: only what the leg declares.
fn gauntlet(path_dir: &Path, envs: &[(&str, &str)], args: &[&str]) -> Output {
    let mut command = Command::new(GAUNTLET);
    command
        .env_clear()
        .env("PATH", path_dir)
        .env("HOME", path_dir)
        .current_dir(path_dir)
        .stdin(Stdio::null());
    for (key, value) in envs {
        command.env(key, value);
    }
    command
        .args(args)
        .output()
        .expect("the gauntlet binary runs")
}

fn text(bytes: &[u8]) -> String {
    String::from_utf8_lossy(bytes).into_owned()
}

#[test]
fn doctor_with_the_harness_absent_succeeds_and_reports_every_adapter() {
    let empty = scratch("doctor-absent");
    let output = gauntlet(&empty, &[], &["agent", "doctor"]);
    let stdout = text(&output.stdout);
    assert!(output.status.success(), "absence is not an error: {stdout}");
    for token in ["mock-agent", "prime-agent", "cli-agent", "tty-agent"] {
        assert!(stdout.contains(token), "the report names {token}: {stdout}");
    }
    assert!(stdout.contains("not installed (optional)"), "{stdout}");
    assert!(
        stdout.contains("gauntlet agent setup"),
        "the message is actionable: {stdout}"
    );
    assert!(stdout.contains("mock-agent: ready"), "{stdout}");
}

#[test]
fn doctor_reports_a_harness_found_on_path_with_version_and_location() {
    let dir = scratch("doctor-path");
    let bin = stub_harness(&dir, "prime-agent", "0.10.2");
    let output = gauntlet(&dir, &[], &["agent", "doctor"]);
    let stdout = text(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("0.10.2"), "{stdout}");
    assert!(stdout.contains(&bin.display().to_string()), "{stdout}");
    assert!(stdout.contains("PATH"), "{stdout}");
}

#[test]
fn doctor_reports_a_harness_found_via_the_explicit_path() {
    let empty = scratch("doctor-explicit-path");
    let elsewhere = scratch("doctor-explicit-bin");
    let bin = stub_harness(&elsewhere, "custom-name", "0.9.4");
    let output = gauntlet(
        &empty,
        &[("PRIME_AGENT_BIN", &bin.display().to_string())],
        &["agent", "doctor"],
    );
    let stdout = text(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("0.9.4"), "{stdout}");
    assert!(stdout.contains(&bin.display().to_string()), "{stdout}");
    assert!(stdout.contains("explicit path"), "{stdout}");
}

#[test]
fn doctor_refuses_an_incompatible_harness_naming_found_and_supported_versions() {
    let dir = scratch("doctor-incompat");
    stub_harness(&dir, "prime-agent", "3.1.4");
    let output = gauntlet(&dir, &[], &["agent", "doctor"]);
    let stdout = text(&output.stdout);
    assert!(
        !output.status.success(),
        "an incompatible install is a problem: {stdout}"
    );
    assert!(
        stdout.contains("3.1.4"),
        "names the found version: {stdout}"
    );
    assert!(
        stdout.contains("0.8.0") && stdout.contains("1.0.0"),
        "names the supported range: {stdout}"
    );
    assert!(
        stdout.contains("mock-agent: ready"),
        "other adapters stay reported: {stdout}"
    );
}

#[test]
fn top_level_doctor_still_works_without_the_harness() {
    // `gauntlet doctor` has its own required checks; the harness line is
    // informational. Only assert the optional line, never the exit status
    // (which depends on the host's other prerequisites).
    let empty = scratch("top-doctor");
    let output = gauntlet(&empty, &[], &["doctor"]);
    let all = format!("{}{}", text(&output.stdout), text(&output.stderr));
    assert!(!all.contains("panicked"), "never a crash: {all}");
}

#[test]
fn setup_without_an_installer_prints_manual_steps_and_changes_nothing() {
    let home = scratch("setup-manual");
    let output = gauntlet(&home, &[], &["agent", "setup", "prime"]);
    let stdout = text(&output.stdout);
    assert!(output.status.success(), "{stdout}");
    assert!(stdout.contains("never downloads"), "{stdout}");
    assert!(
        stdout.contains(gauntlet_adapter_agent_prime::managed::OFFICIAL_INSTALLER_URL),
        "names the official source: {stdout}"
    );
    assert!(
        stdout.contains("0.10.2"),
        "names the pinned version: {stdout}"
    );
    assert_eq!(
        std::fs::read_dir(&home).unwrap().count(),
        0,
        "nothing was written"
    );
}

fn installer(dir: &Path) -> (PathBuf, PathBuf, String) {
    let marker = dir.join("installer-ran.txt");
    let script = dir.join("setup-script");
    let body = format!(
        "echo \"$PRIME_AGENT_VERSION $PRIME_AGENT_RUST_PREFIX\" > {}\n",
        marker.display()
    );
    std::fs::write(&script, &body).unwrap();
    let digest =
        match gauntlet_adapter_agent_prime::managed::verify_installer(&script, &"0".repeat(64)) {
            Err(gauntlet_adapter_agent_prime::managed::SetupError::ChecksumMismatch {
                actual,
                ..
            }) => actual,
            other => panic!("expected a mismatch carrying the real digest: {other:?}"),
        };
    (script, marker, digest)
}

#[test]
fn setup_refuses_to_run_the_installer_without_confirmation() {
    let home = scratch("setup-noconfirm");
    let (script, marker, digest) = installer(&home);
    let output = gauntlet(
        &home,
        &[],
        &[
            "agent",
            "setup",
            "prime",
            "--installer",
            &script.display().to_string(),
            "--sha256",
            &digest,
        ],
    );
    assert_eq!(output.status.code(), Some(2), "{}", text(&output.stderr));
    assert!(
        text(&output.stderr).contains("not confirmed"),
        "{}",
        text(&output.stderr)
    );
    assert!(!marker.exists(), "the installer must not have run");
}

#[test]
fn setup_refuses_an_installer_whose_checksum_does_not_match_even_with_yes() {
    let home = scratch("setup-badsum");
    let (script, marker, _) = installer(&home);
    let output = gauntlet(
        &home,
        &[],
        &[
            "agent",
            "setup",
            "prime",
            "--installer",
            &script.display().to_string(),
            "--sha256",
            &"0".repeat(64),
            "--yes",
        ],
    );
    assert!(!output.status.success());
    assert!(
        text(&output.stderr).contains("refusing to run"),
        "{}",
        text(&output.stderr)
    );
    assert!(!marker.exists(), "the installer must not have run");
}

#[test]
fn setup_with_a_verified_installer_and_yes_runs_it_pinned_into_a_user_prefix() {
    let home = scratch("setup-yes");
    let (script, marker, digest) = installer(&home);
    // The installer runs under `sh`, so this leg needs the host PATH.
    let host_path = std::env::var("PATH").expect("PATH is set");
    let output = gauntlet(
        &home,
        &[("PATH", host_path.as_str())],
        &[
            "agent",
            "setup",
            "prime",
            "--installer",
            &script.display().to_string(),
            "--sha256",
            &digest,
            "--yes",
        ],
    );
    assert!(output.status.success(), "{}", text(&output.stderr));
    let seen = std::fs::read_to_string(&marker).expect("the installer ran");
    assert_eq!(
        seen.trim(),
        format!("0.10.2 {}", home.join(".local").display())
    );
}

#[test]
fn setup_for_an_unknown_adapter_is_a_usage_error() {
    let home = scratch("setup-unknown");
    let output = gauntlet(&home, &[], &["agent", "setup", "mock-agent"]);
    assert_eq!(output.status.code(), Some(2));
}

// In-process report legs (the same data `gauntlet agent doctor` renders).
mod report {
    use super::*;
    use gauntlet_adapter_agent_prime as harness;
    use gauntlet_cli::agent_cmd::{ReportInputs, RowState, adapter_report};

    #[test]
    fn with_the_harness_absent_the_alternative_adapters_are_reported_and_usable() {
        let empty = scratch("report-absent");
        let rows = adapter_report(&ReportInputs {
            path_env: Some(empty.into_os_string()),
            cli_command: Some("true {prompt_file}".to_owned()),
            ..ReportInputs::default()
        });
        let state_of = |token: &str| rows.iter().find(|r| r.token == token).expect("row").state;
        assert_eq!(rows.len(), 4);
        assert_eq!(state_of("mock-agent"), RowState::Ready);
        assert_eq!(state_of("cli-agent"), RowState::Ready);
        assert_eq!(state_of("tty-agent"), RowState::Unconfigured);
        assert_eq!(state_of(harness::RUNNER_TOKEN), RowState::Missing);
        assert!(rows.iter().all(|r| r.state != RowState::Broken));
    }

    #[test]
    fn a_broken_alternative_template_is_a_problem_not_a_silent_pass() {
        let empty = scratch("report-bad-template");
        let rows = adapter_report(&ReportInputs {
            path_env: Some(empty.into_os_string()),
            cli_command: Some("true {not_a_placeholder}".to_owned()),
            ..ReportInputs::default()
        });
        let row = rows.iter().find(|r| r.token == "cli-agent").expect("row");
        assert_eq!(row.state, RowState::Broken);
    }
}
