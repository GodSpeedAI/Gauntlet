//! `gauntlet doctor [--config <path>]` (spec §8.5, plan Task 5): the
//! §8.5 preflight checks, a machine report at
//! `$GAUNTLET_REPORT_DIR/doctor.json`, and a human summary. Behavior is
//! byte-identical to the pre-Task-20 surface; only the module home
//! moved (the thin-library refactor).

use std::process::ExitCode;

use gauntlet_app::doctor::{DoctorInput, run_doctor_and_write};

pub fn run(args: &[String]) -> ExitCode {
    let mut input = DoctorInput::new(
        crate::spec_vocab::ANNEX_E_ENV,
        crate::spec_vocab::CAPABILITY_VOCABULARY,
    );
    // Plan Task 14 (§8.5 check 4): the composition root measures the
    // configured sandbox backend through the real adapter — the app layer
    // cannot map vendor tokens to rungs (§24.3), so the measurement is a
    // composition fact. Only the capability-named façade is used.
    input.backend_measure = Some(crate::context::measure_backend);
    // Plan Task 15 (§8.5 check 3): the composition root measures the
    // configured default runner through the real adapter's typed version
    // probe — the supported range comes from the app defaults module
    // (correction RES-D8), so the adapter never owns its compatibility
    // ruler. A probe mismatch fails doctor as a configuration failure.
    input.runner_measure = Some(crate::context::measure_runner);
    let mut fabric_dir: Option<std::path::PathBuf> = None;
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
            // F2b2: the fabric section lists each configured runner profile.
            "--fabric-dir" => {
                let Some(path) = args.get(index + 1) else {
                    eprintln!("gauntlet doctor: --fabric-dir requires a path");
                    return ExitCode::from(2);
                };
                fabric_dir = Some(std::path::PathBuf::from(path));
                index += 2;
            }
            other => {
                eprintln!("gauntlet doctor: unknown argument '{other}'");
                println!("usage: gauntlet doctor [--config <path>] [--fabric-dir <path>]");
                return ExitCode::from(2);
            }
        }
    }
    match run_doctor_and_write(&input) {
        Ok((report, path)) => {
            print!("{}", report.human_summary());
            // Phase H slice 2b: optional capability, reported plainly; it never
            // changes the exit status (absence is not an error).
            println!(
                "codemode (optional): {}",
                crate::context::measure_codemode().summary()
            );
            println!("report: {}", path.display());
            let fabric_ok = fabric_dir
                .as_deref()
                .is_none_or(crate::fabric_cmd::doctor_fabric_section);
            if report.exit_ok() && fabric_ok {
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
