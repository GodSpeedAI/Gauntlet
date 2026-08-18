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
