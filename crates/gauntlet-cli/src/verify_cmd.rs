//! `gauntlet verify` (spec §20.1; plan Task 20, charter D8): re-runs
//! ladder verification for `--unit` or a whole `--run` through the
//! [`VerificationAuthority`] + the composed §14.2 rung set. Prints the
//! THREE-STATE outcomes (PASS / FAIL / INCONCLUSIVE stay three states —
//! nothing collapses them) and NEVER settles: this command writes
//! nothing; a verification record here is a read-side re-run, and the
//! durable settlement path stays the loop's alone.

use std::process::ExitCode;

use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::verifier::VerificationRequest;

use crate::compose::{self, open_operator_surface};
use crate::context::resolve_context;

fn bound() -> Deadline {
    crate::context::doctor_bound()
}

/// `gauntlet verify --run <run-id> [--unit <unit-id>]`.
#[allow(clippy::arc_with_non_send_sync)] // single-threaded re-verification surface
pub fn verify(args: &[String]) -> ExitCode {
    let mut run_id: Option<String> = None;
    let mut unit: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--run" => match args.get(index + 1) {
                Some(value) => {
                    run_id = Some(value.clone());
                    index += 2;
                }
                None => {
                    eprintln!("gauntlet verify: --run requires a run id");
                    return ExitCode::from(2);
                }
            },
            "--unit" => match args.get(index + 1) {
                Some(value) => {
                    unit = Some(value.clone());
                    index += 2;
                }
                None => {
                    eprintln!("gauntlet verify: --unit requires a unit id");
                    return ExitCode::from(2);
                }
            },
            other => {
                eprintln!("gauntlet verify: unknown argument '{other}'");
                println!("usage: gauntlet verify --run <run-id> [--unit <unit-id>]");
                return ExitCode::from(2);
            }
        }
    }
    let Some(run_id) = run_id else {
        eprintln!("gauntlet verify: --run is required");
        println!("usage: gauntlet verify --run <run-id> [--unit <unit-id>]");
        return ExitCode::from(2);
    };
    let Ok(run_id) = gauntlet_domain::RunId::new(&run_id) else {
        eprintln!("gauntlet verify: {run_id} is not a valid run id (ULID grammar)");
        return ExitCode::FAILURE;
    };
    let context = match resolve_context(None) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("gauntlet verify: the configuration did not resolve: {error}");
            return ExitCode::FAILURE;
        }
    };
    let surface = match open_operator_surface(&context) {
        Ok(surface) => surface,
        Err(error) => {
            eprintln!("gauntlet verify: {error}");
            return ExitCode::FAILURE;
        }
    };
    // The units to re-verify: the FROZEN RULER's units (the tier facts),
    // filtered by --unit when given.
    let key_text = match std::fs::read_to_string(&surface.answer_key) {
        Ok(text) => text,
        Err(error) => {
            eprintln!(
                "gauntlet verify: reading {}: {error}",
                surface.answer_key.display()
            );
            return ExitCode::FAILURE;
        }
    };
    let parsed = gauntlet_app::reference::parse::parse_answer_key(&key_text);
    let mut units = parsed.units;
    if let Some(unit) = &unit {
        let Ok(filter) = gauntlet_domain::UnitId::new(unit) else {
            eprintln!("gauntlet verify: {unit} is not a valid unit id");
            return ExitCode::FAILURE;
        };
        units.retain(|candidate| candidate.id() == &filter);
        if units.is_empty() {
            eprintln!("gauntlet verify: unit {filter} is not in the frozen ruler");
            return ExitCode::FAILURE;
        }
    }
    // The run's latest iteration anchors the governed open (§14.1); a run
    // with no checkpoint opens at iteration one.
    let iteration = match surface.state.resume(&run_id, bound()) {
        Ok(Some(checkpoint)) => checkpoint.iteration_no,
        Ok(None) => gauntlet_domain::IterationNo::new(1).expect("iteration one is valid"),
        Err(error) => {
            eprintln!("gauntlet verify: {error}");
            return ExitCode::FAILURE;
        }
    };
    let evidence =
        match gauntlet_adapter_evidence_fs::FsEvidenceStore::open(&context.resolved.evidence_dir) {
            Ok(store) => std::sync::Arc::new(store),
            Err(error) => {
                eprintln!("gauntlet verify: {error}");
                return ExitCode::FAILURE;
            }
        };
    let ids = match gauntlet_adapter_id_entropy::UlidIdGenerator::new() {
        Ok(ids) => std::sync::Arc::new(ids),
        Err(error) => {
            eprintln!("gauntlet verify: {error}");
            return ExitCode::FAILURE;
        }
    };
    let ladder = match compose::composed_ladder(&surface.workspace_root) {
        Ok(ladder) => ladder,
        Err(error) => {
            eprintln!("gauntlet verify: {error}");
            return ExitCode::FAILURE;
        }
    };
    let authority = gauntlet_app::verify::authority::VerificationAuthority::new(
        surface.state.clone(),
        evidence,
        ids,
    );
    let governed = match authority.for_run(
        &run_id,
        iteration,
        ladder,
        surface.workspace_root.clone(),
        bound(),
    ) {
        Ok(governed) => governed,
        Err(error) => {
            eprintln!("gauntlet verify: the governed verification open refused: {error}");
            return ExitCode::FAILURE;
        }
    };
    let mut pass = 0usize;
    let mut fail = 0usize;
    let mut inconclusive = 0usize;
    let mut refused = 0usize;
    for candidate in &units {
        let requirement = candidate.evidence_required();
        let request = VerificationRequest {
            id: gauntlet_domain::VerificationRef::new(&format!("verify-cli-{}", candidate.id()))
                .expect("the minted verification ref is valid"),
            run_id: run_id.clone(),
            unit_id: candidate.id().clone(),
            claim: candidate.claim().to_owned(),
            requirement: requirement.clone(),
            evidence: requirement.locator().clone(),
        };
        match governed.verify_claim(&request, bound()) {
            Ok(verified) => {
                let outcome = verified.record().outcome();
                println!("verify: unit={} outcome={outcome}", candidate.id());
                match outcome {
                    gauntlet_domain::Outcome::Pass => pass += 1,
                    gauntlet_domain::Outcome::Fail => fail += 1,
                    gauntlet_domain::Outcome::Inconclusive => inconclusive += 1,
                }
            }
            Err(refusal) => {
                eprintln!("verify: unit={} REFUSED — {}", candidate.id(), refusal);
                refused += 1;
            }
        }
    }
    println!(
        "verify: run_id={run_id} pass={pass} fail={fail} inconclusive={inconclusive} \
         refused={refused} — records printed, NOTHING settled"
    );
    if refused > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}
