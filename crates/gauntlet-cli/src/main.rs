//! Command surface composition root (spec §4.1, §20.1): the only place
//! adapters are constructed and wired into the application layer.
//!
//! Plan Task 5 lands the first real command: `gauntlet doctor
//! [--config <path>]` (spec §8.5). Argument parsing is std `env::args`,
//! small and boring; unknown arguments are a usage error with exit code 2.
//! The full §20.1 surface lands with plan Task 20.

mod spec_vocab;

use std::process::ExitCode;

use gauntlet_app::doctor::{DoctorInput, run_doctor_and_write};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("doctor") => doctor(&args[1..]),
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
