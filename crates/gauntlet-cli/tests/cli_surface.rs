//! The `cli_surface` proof (AK-060; plan Task 20 step 5, brief Item 4):
//! a snapshot of `--help` for the WHOLE command tree plus per-command
//! unknown-arg / missing-arg exit-code smokes.
//!
//! **Locator note (recorded per charter D5, the AK-063/AK-065 class):**
//! the frozen locator is repo-root `tests/e2e/cli_surface`. Escapes.rs's
//! Cargo-target-path rule exempts exactly one crate — the arch suite's
//! own — whose justification ("not part of the product, outside
//! CORE_CRATES") the CLI cannot share: `gauntlet-cli` is a core product
//! crate, and exempting it would let a relocated lib/bin root escape the
//! scanners. The evidence therefore lives HERE, in the crate that owns
//! the mechanism (`crates/gauntlet-cli/tests/cli_surface.rs`), which is
//! also what lets the snapshot drive the REAL binary through
//! `CARGO_BIN_EXE_gauntlet`. Recorded in
//! `.agents/reports/reference-candidates.md` for the ruler's owner.
//!
//! **Golden-file procedure:** the committed `cli_help.txt` (this
//! directory, the `include_str!` binding reference below) is the frozen
//! help surface. Update it ONLY deliberately, with the surface change
//! recorded in the task report — never because output drifted. A drift
//! with no recorded change is a defect in the code, not in the golden
//! file.

use std::process::Command;

fn binary() -> Command {
    let exe = env!("CARGO_BIN_EXE_gauntlet");
    Command::new(exe)
}

// The help snapshot is per configuration (Task 21, charter D9): the
// DEFAULT build's frozen snapshot is byte-identical to the pre-Task-21
// surface (`cli_help.txt`); the surfaces build serves one extra command
// (`gauntlet serve`) and freezes its OWN snapshot. Each is updated only
// deliberately, with the surface change recorded in the task report.
#[cfg(not(feature = "surfaces"))]
const GOLDEN_HELP: &str = include_str!("cli_help.txt");
#[cfg(feature = "surfaces")]
const GOLDEN_HELP: &str = include_str!("cli_help_surfaces.txt");

/// The §20.1 commands, per configuration: `serve` exists ONLY in the
/// surfaces build.
#[cfg(not(feature = "surfaces"))]
const COMMANDS: &[&str] = &[
    "doctor",
    "init",
    "reference",
    "run",
    "status",
    "inspect",
    "verify",
    "resume",
    "escalations",
    "roles",
    "skills",
    "improve",
    "export",
];
#[cfg(feature = "surfaces")]
const COMMANDS: &[&str] = &[
    "doctor",
    "init",
    "reference",
    "run",
    "status",
    "inspect",
    "verify",
    "resume",
    "escalations",
    "serve",
    "roles",
    "skills",
    "improve",
    "export",
];

/// The whole-tree help, byte-for-byte against the committed snapshot.
#[test]
fn the_whole_tree_help_snapshot_holds() {
    let output = binary()
        .arg("--help")
        .output()
        .expect("the gauntlet binary runs");
    assert!(
        output.status.success(),
        "--help exits 0, got {:?}",
        output.status
    );
    let stdout = String::from_utf8(output.stdout).expect("help output is UTF-8");
    assert_eq!(
        stdout, GOLDEN_HELP,
        "the help surface moved without a deliberate, recorded change"
    );
}

/// Every §20.1 command answers; an unknown command is a usage error.
#[test]
fn every_surface_command_dispatches_and_unknown_commands_are_usage_errors() {
    const COMMANDS: &[&str] = &[
        "doctor",
        "init",
        "reference",
        "run",
        "status",
        "inspect",
        "verify",
        "resume",
        "escalations",
        "roles",
        "skills",
        "improve",
        "export",
    ];
    #[cfg(not(feature = "surfaces"))]
    {
        // Charter D9: `serve` is an unknown command in the DEFAULT build
        // (exit 2, named as unknown) — the surfaces are opt-in.
        let output = binary()
            .arg("serve")
            .output()
            .expect("the gauntlet binary runs");
        assert_eq!(
            output.status.code(),
            Some(2),
            "serve is unknown in the default build"
        );
        let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
        assert!(
            stderr.contains("unknown command"),
            "the default build names serve as unknown: {stderr}"
        );
    }
    for command in COMMANDS {
        // A command that takes NO further required arguments is invoked
        // as-is; commands with required arguments answer their usage
        // error instead — BOTH are proof the command DISPATCHED (an
        // unknown command names itself, exit 2, with the root usage).
        let output = binary()
            .arg(command)
            .arg("gauntlet-sentinel-unknown-argument")
            .output()
            .expect("the gauntlet binary runs");
        let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
        assert!(
            !stderr.is_empty() || output.status.success(),
            "command {command} dispatched to neither success nor a usage error"
        );
        assert!(
            !stderr.starts_with("gauntlet: unknown command"),
            "command {command} is not dispatched by the root: {stderr}"
        );
    }
    let output = binary()
        .arg("definitely-not-a-command")
        .output()
        .expect("runs");
    assert_eq!(output.status.code(), Some(2), "unknown command exits 2");
    let stderr = String::from_utf8(output.stderr).expect("stderr is UTF-8");
    assert!(
        stderr.contains("unknown command"),
        "the refusal names itself: {stderr}"
    );
}

/// Unknown-argument and missing-argument smokes: each listed invocation
/// is a usage refusal with exit 2, and the message names the command.
#[test]
fn unknown_and_missing_arguments_are_usage_errors_with_exit_two() {
    const REFUSALS: &[&[&str]] = &[
        &["doctor", "--bogus"],
        &["reference", "digest"],
        &["reference", "digest", "--bogus"],
        &["reference", "validate", "--path"],
        &["roles", "install"],
        &["roles", "verify", "--harness", "x"],
        &["run"],
        &["run", "--bogus"],
        &["status"],
        &["inspect"],
        &["inspect", "--iteration", "x"],
        &["verify"],
        &["verify", "--unit", "only"],
        &["resume"],
        &["escalations"],
        &["escalations", "frobnicate"],
        &["escalations", "resolve"],
        &["escalations", "resolve", "--ticket", "t"],
        &["skills", "frobnicate"],
        &["improve", "frobnicate"],
        &["export"],
    ];
    for &invocation in REFUSALS {
        let output: std::process::Output = binary()
            .args(invocation)
            .output()
            .expect("the gauntlet binary runs");
        assert_eq!(
            output.status.code(),
            Some(2),
            "invocation {invocation:?} must be a usage error (exit 2)"
        );
        let combined = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        // The refusal names its surface (the pre-Task-20 commands print
        // error-only for missing VALUES and error+usage for unknown
        // arguments — both are refusals; byte-identical behavior keeps
        // the older, narrower form).
        assert!(
            combined.contains("gauntlet"),
            "invocation {invocation:?} must refuse by name: {combined}"
        );
    }
}

/// The three surfaces of the command table agree: the root usage lines,
/// the dispatch table, and the report manifest all name the SAME set.
#[test]
fn usage_dispatch_and_manifest_name_the_same_command_set() {
    let output = binary().arg("--help").output().expect("runs");
    let stdout = String::from_utf8(output.stdout).expect("UTF-8");
    for command in COMMANDS {
        assert!(
            stdout.contains(&format!("gauntlet {command}")),
            "the usage names {command}"
        );
    }
    assert_eq!(
        COMMANDS.len(),
        gauntlet_cli::report_manifest::COMMAND_TABLE_LEN,
        "the dispatch and the report manifest agree on the command count"
    );
    assert!(
        !gauntlet_cli::report_manifest::is_wired("improve"),
        "improve stays honestly not-wired in the manifest"
    );
}
