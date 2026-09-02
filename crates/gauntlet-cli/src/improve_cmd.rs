//! `gauntlet improve` (spec §20.1; charter D8 — HONEST EXPOSURE): the
//! optimizer surface is plan Task 23's slice. The command parses its
//! subcommands (so its shape is real and its help is honest) and
//! returns a typed NOT-WIRED failure naming that plan task — never a
//! fake success, never a panic, never a usage error disguised as a
//! runtime result. The manifest ([crate::report_manifest]) records the
//! same fact — and this command READS it: the not-wired refusal consults
//! `is_wired` so the exit and the manifest cannot disagree by
//! construction (the `cli_surface` test pins the same agreement by
//! instrument; RES-15).

use std::process::ExitCode;

/// `gauntlet improve <subcommand>`.
pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("propose") | Some("evaluate") | Some("promote") | Some("rollback") => {
            not_wired(&format!("improve {}", args[0]))
        }
        Some(other) => {
            eprintln!("gauntlet improve: unknown subcommand '{other}'");
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
    println!("usage: gauntlet improve propose | evaluate | promote | rollback");
    println!("       (the optimizer surface is not wired — it lands with plan Task 23)");
}

/// The typed not-wired failure: non-zero exit, honest stderr naming the
/// plan task that owns the surface. The refusal consults the capability
/// manifest (RES-15): if the manifest ever claimed this surface wired
/// while no backing service exists here, the command refuses the drift
/// loudly instead of pretending either way.
fn not_wired(what: &str) -> ExitCode {
    if crate::report_manifest::is_wired("improve") {
        eprintln!(
            "gauntlet improve: the capability manifest names 'improve' as wired, but this \
             build wires no optimizer — refusing (manifest/command drift, never a fake \
             success)"
        );
        return ExitCode::FAILURE;
    }
    eprintln!(
        "gauntlet improve: '{what}' is NOT WIRED — the process optimizer and the \
         self-improvement gate land with plan Task 23 (spec §19.4: candidate revisions \
         are evaluated in isolation before any promotion); no optimizer write path exists \
         in this build"
    );
    ExitCode::FAILURE
}
