//! Command surface composition root (spec §4.1, §20.1): the only place
//! adapters are constructed and wired into the application layer.
//!
//! The crate is a LIBRARY with a thin binary: `gauntlet_cli::run_cli`
//! dispatches the §20.1 command tree, and the binary is a wrapper over
//! it (plan Task 20's thin-library refactor — the E2E builder drives
//! the binary, tests compose against the library). Argument parsing is
//! std `env::args`, small and boring; unknown arguments are a usage
//! error with exit code 2.
//!
//! The §20.1 surface (plan Task 20): `doctor`, `init`, `reference`
//! (form/freeze/show/diff/verify + the existing digest/validate),
//! `run`, `status`, `inspect`, `verify`, `resume`, `escalations`
//! (list/resolve/retry), `roles` (install/verify/list), `skills`
//! (install/verify/list), `improve`
//! (propose/evaluate/baseline/promote/rollback), `export`. Every §20.1
//! command fronts a backing service; the optimizer surface landed with
//! plan Task 23 (spec §19.3/§19.4), the baseline measurement harness
//! with plan Task 26 (AK-069).

pub mod compose;
pub mod context;
pub mod deliver_cmd;
pub mod doctor_cmd;
pub mod escalations_cmd;
pub mod improve_cmd;
pub mod init_cmd;
pub mod operator_action;
pub mod reference_cmd;
pub mod reference_lock;
pub mod report_manifest;
pub mod roles_cmd;
pub mod run_cmd;
pub mod skills_cmd;
pub mod spec_vocab;
pub mod state_cmd;
pub mod verify_cmd;
pub mod writer_lock;

// Plan Task 21 (charter D9): the §20.2 JSON operator surface — compiled
// ONLY under the `surfaces` feature. The DEFAULT build stays CLI-only
// (AK-061): `serve` is an unknown command there and the frozen help
// snapshot is byte-identical.
#[cfg(feature = "surfaces")]
pub mod durable_surface;
#[cfg(feature = "surfaces")]
pub mod serve_cmd;

use std::process::ExitCode;

/// Dispatches one CLI invocation (`args` excludes the program name).
/// The binary's `main` delegates here; tests may call this directly.
pub fn run_cli(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        // The whole-tree help: `--help`/`-h` at the ROOT prints the full
        // command surface and exits 0 (the cli_surface snapshot freezes
        // it). Per-command unknown arguments stay usage errors with exit
        // 2 — the established convention (COR-D12).
        Some("--help") | Some("-h") => {
            usage();
            ExitCode::SUCCESS
        }
        Some("doctor") => doctor_cmd::run(&args[1..]),
        Some("init") => init_cmd::run(&args[1..]),
        Some("reference") => reference_cmd::run(&args[1..]),
        Some("run") => run_cmd::run(&args[1..]),
        Some("status") => state_cmd::status(&args[1..]),
        Some("inspect") => state_cmd::inspect(&args[1..]),
        Some("verify") => verify_cmd::verify(&args[1..]),
        Some("resume") => run_cmd::resume(&args[1..]),
        Some("escalations") => escalations_cmd::escalations(&args[1..]),
        #[cfg(feature = "surfaces")]
        Some("serve") => serve_cmd::serve(&args[1..]),
        Some("roles") => roles_cmd::run(&args[1..]),
        Some("skills") => skills_cmd::run(&args[1..]),
        Some("improve") => improve_cmd::run(&args[1..]),
        Some("export") => state_cmd::export(&args[1..]),
        Some("deliver") => deliver_cmd::run(&args[1..]),
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

/// The whole command tree, one line per surface (the `--help` body;
/// frozen by the `cli_surface` snapshot, plan Task 20).
fn usage() {
    println!("gauntlet <version> — see spec 20.1 for the full surface");
    println!("usage: gauntlet doctor [--config <path>]");
    println!("       gauntlet init");
    println!("       gauntlet reference digest --path <path>");
    println!("       gauntlet reference validate --path <path>");
    println!("       gauntlet reference form [--path <path>]");
    println!("       gauntlet reference freeze [--run <run-id>]");
    println!("       gauntlet reference show");
    println!("       gauntlet reference diff --path <path> --against <path>");
    println!("       gauntlet reference verify");
    println!("       gauntlet run [--dry-run] [--config <path>] \"<intent>\"");
    println!("       gauntlet status --run <run-id>");
    println!(
        "       gauntlet inspect --run <run-id> [--iteration <n>] [--unit <unit-id>] \
         [--verifications] [--settlements]"
    );
    println!("       gauntlet verify --run <run-id> [--unit <unit-id>]");
    println!("       gauntlet verify --all [--report <path>] [--manifest <path>]");
    println!("       gauntlet resume <run-id>");
    println!("       gauntlet escalations list | resolve | retry");
    #[cfg(feature = "surfaces")]
    println!("       gauntlet serve                  (surfaces build; bind: GAUNTLET_API_BIND)");
    println!("       gauntlet roles install --harness <id>");
    println!("       gauntlet roles verify");
    println!("       gauntlet roles list");
    println!("       gauntlet skills install | verify | list");
    println!("       gauntlet improve propose [--from-result <path>] [--from-run <run-id>]");
    println!("       gauntlet improve evaluate <revision> --held-out <dir> --regression <dir> \\");
    println!("                    [--protected <cap,cap>] [--metric <token>]");
    println!("       gauntlet improve baseline --profiles <A,B,C,D> --tasks <dir> \\");
    println!("                    [--out <path>] [--config <path>]");
    println!("       gauntlet improve promote <revision>");
    println!("       gauntlet improve rollback <revision>");
    println!("       gauntlet export --run <run-id> [--out <path>]");
    println!("       gauntlet deliver [--run <run-id>] [--apply]");
}
