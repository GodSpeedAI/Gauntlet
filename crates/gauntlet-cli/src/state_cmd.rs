//! `gauntlet status` · `gauntlet inspect` · `gauntlet export` (spec
//! §20.1; plan Task 20, charter D8): operator reads over the SAME
//! store the run wrote, composed from the existing port reads
//! (`load_run` / `resume` / `verification_records` / `settlements` /
//! `correction_selections` / `export_jsonl`). Every line is a record
//! fact (`key=value` per §20.3); nothing here writes run state, and
//! `export` writes only where the operator points `--out`.

use std::process::ExitCode;

use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::state_store::Checkpoint;

use crate::compose::open_operator_surface;
use crate::context::resolve_context;

/// A deadline far from any wall clock the reads can reach: the store
/// reads are local and unbounded only by the port's signature (the same
/// convention the doctor probes use).
fn read_bound() -> Deadline {
    crate::context::doctor_bound()
}

/// `gauntlet status [--run <run-id>]`.
pub fn status(args: &[String]) -> ExitCode {
    let mut run_id: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--run" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("gauntlet status: --run requires a run id");
                    return ExitCode::from(2);
                };
                if run_id.replace(value.clone()).is_some() {
                    eprintln!("gauntlet status: --run given twice");
                    return ExitCode::from(2);
                }
                index += 2;
            }
            other => {
                eprintln!("gauntlet status: unknown argument '{other}'");
                println!("usage: gauntlet status [--run <run-id>]");
                return ExitCode::from(2);
            }
        }
    }
    let Some(run_id) = run_id else {
        // `status` keeps its explicit-run requirement deliberately (the
        // operator names the run under inspection — no guessing which);
        // the served snapshot derives its own context run separately.
        // Truthful, not a silent empty report.
        eprintln!(
            "gauntlet status: a run id is required (--run <run-id>) — find run ids in the \
             run report, the served snapshot's `run`, or .gauntlet/state"
        );
        println!("usage: gauntlet status [--run <run-id>]");
        return ExitCode::from(2);
    };
    let Ok(run_id) = gauntlet_domain::RunId::new(&run_id) else {
        eprintln!("gauntlet status: {run_id} is not a valid run id (ULID grammar)");
        return ExitCode::FAILURE;
    };
    let context = match resolve_context(None) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("gauntlet status: the configuration did not resolve: {error}");
            return ExitCode::FAILURE;
        }
    };
    let surface = match open_operator_surface(&context) {
        Ok(surface) => surface,
        Err(error) => {
            eprintln!("gauntlet status: {error}");
            return ExitCode::FAILURE;
        }
    };
    let deadline = read_bound();
    let run = match surface.state.load_run(&run_id, deadline) {
        Ok(Some(run)) => run,
        Ok(None) => {
            eprintln!("gauntlet status: run {run_id} is not durable in this state store");
            return ExitCode::FAILURE;
        }
        Err(error) => {
            eprintln!("gauntlet status: {error}");
            return ExitCode::FAILURE;
        }
    };
    println!("status: run_id={run_id}");
    println!("status: intent={:?}", run.intent());
    println!(
        "status: reference_digest={}",
        run.reference_version().digest()
    );
    let checkpoint = match surface.state.resume(&run_id, deadline) {
        Ok(checkpoint) => checkpoint,
        Err(error) => {
            eprintln!("gauntlet status: {error}");
            return ExitCode::FAILURE;
        }
    };
    let Some(checkpoint) = checkpoint else {
        println!("status: no checkpoint yet — the run never passed its first boundary");
        return ExitCode::SUCCESS;
    };
    print_checkpoint_summary(&surface, &checkpoint)
}

/// The shared checkpoint summary: units by tier, in-flight, budget burn.
fn print_checkpoint_summary(
    surface: &crate::compose::OperatorSurface,
    checkpoint: &Checkpoint,
) -> ExitCode {
    // Units by tier: the tier is a FROZEN RULER fact — read from the
    // answer key the run froze, never guessed from the status.
    let mut by_tier: Vec<(gauntlet_domain::BlastRadius, usize, usize)> = Vec::new();
    if let Ok(key_text) = std::fs::read_to_string(&surface.answer_key) {
        let parsed = gauntlet_app::reference::parse::parse_answer_key(&key_text);
        for unit in &parsed.units {
            let tier = unit.blast_radius();
            let status = checkpoint
                .unit_states
                .iter()
                .find(|(id, _)| id == unit.id())
                .map(|(_, status)| *status);
            let settled = matches!(status, Some(gauntlet_domain::UnitStatus::Settled));
            match by_tier.iter_mut().find(|(key, _, _)| *key == tier) {
                Some(entry) => {
                    entry.1 += 1;
                    if settled {
                        entry.2 += 1;
                    }
                }
                None => by_tier.push((tier, 1, usize::from(settled))),
            }
        }
    }
    for (tier, total, settled) in &by_tier {
        println!("status: tier={tier} units={total} settled={settled}");
    }
    println!(
        "status: iteration={} in_flight={}",
        checkpoint.iteration_no,
        checkpoint.in_flight.len()
    );
    for (invocation, contract) in &checkpoint.in_flight {
        println!("status: in_flight invocation={invocation} contract={contract}");
    }
    let spent = checkpoint.budget_spent;
    println!(
        "status: budget tokens={} cost_usd_micro={} rounds={}",
        spent.tokens(),
        spent.cost_usd().get(),
        spent.rounds()
    );
    ExitCode::SUCCESS
}

/// `gauntlet inspect --run <run-id> [--iteration <n>] [--unit <unit-id>]
/// [--verifications] [--settlements]`.
pub fn inspect(args: &[String]) -> ExitCode {
    let mut run_id: Option<String> = None;
    let mut iteration: Option<u32> = None;
    let mut unit: Option<String> = None;
    let mut show_verifications = false;
    let mut show_settlements = false;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--run" => match args.get(index + 1) {
                Some(value) => {
                    run_id = Some(value.clone());
                    index += 2;
                }
                None => {
                    eprintln!("gauntlet inspect: --run requires a run id");
                    return ExitCode::from(2);
                }
            },
            "--iteration" => match args.get(index + 1) {
                Some(value) => match value.parse::<u32>() {
                    Ok(number) => {
                        iteration = Some(number);
                        index += 2;
                    }
                    Err(_) => {
                        eprintln!("gauntlet inspect: --iteration requires a number");
                        return ExitCode::from(2);
                    }
                },
                None => {
                    eprintln!("gauntlet inspect: --iteration requires a number");
                    return ExitCode::from(2);
                }
            },
            "--unit" => match args.get(index + 1) {
                Some(value) => {
                    unit = Some(value.clone());
                    index += 2;
                }
                None => {
                    eprintln!("gauntlet inspect: --unit requires a unit id");
                    return ExitCode::from(2);
                }
            },
            "--verifications" => {
                show_verifications = true;
                index += 1;
            }
            "--settlements" => {
                show_settlements = true;
                index += 1;
            }
            other => {
                eprintln!("gauntlet inspect: unknown argument '{other}'");
                println!(
                    "usage: gauntlet inspect --run <run-id> [--iteration <n>] [--unit <unit-id>] [--verifications] [--settlements]"
                );
                return ExitCode::from(2);
            }
        }
    }
    let Some(run_id) = run_id else {
        eprintln!("gauntlet inspect: --run is required");
        println!(
            "usage: gauntlet inspect --run <run-id> [--iteration <n>] [--unit <unit-id>] [--verifications] [--settlements]"
        );
        return ExitCode::from(2);
    };
    let Ok(run_id) = gauntlet_domain::RunId::new(&run_id) else {
        eprintln!("gauntlet inspect: {run_id} is not a valid run id (ULID grammar)");
        return ExitCode::FAILURE;
    };
    let unit_filter = unit.map(|unit| gauntlet_domain::UnitId::new(&unit));
    if let Some(Err(error)) = &unit_filter {
        eprintln!("gauntlet inspect: the --unit value is not a valid unit id: {error}");
        return ExitCode::FAILURE;
    }
    let context = match resolve_context(None) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("gauntlet inspect: the configuration did not resolve: {error}");
            return ExitCode::FAILURE;
        }
    };
    let surface = match open_operator_surface(&context) {
        Ok(surface) => surface,
        Err(error) => {
            eprintln!("gauntlet inspect: {error}");
            return ExitCode::FAILURE;
        }
    };
    let deadline = read_bound();
    if surface
        .state
        .load_run(&run_id, deadline)
        .map(|r| r.is_none())
        .unwrap_or(true)
    {
        eprintln!("gauntlet inspect: run {run_id} is not durable in this state store");
        return ExitCode::FAILURE;
    }
    println!("inspect: run_id={run_id}");
    if let Some(number) = iteration {
        let iteration_no = gauntlet_domain::IterationNo::new(number);
        match iteration_no {
            Ok(iteration_no) => {
                match surface
                    .state
                    .observation_binding(&run_id, iteration_no, deadline)
                {
                    Ok(Some(binding)) => println!(
                        "inspect: iteration={} observation={} digest={}",
                        iteration_no,
                        binding.evidence(),
                        binding.digest()
                    ),
                    Ok(None) => {
                        println!("inspect: iteration={iteration_no} has no bound observation")
                    }
                    Err(error) => {
                        eprintln!("gauntlet inspect: {error}");
                        return ExitCode::FAILURE;
                    }
                }
            }
            Err(error) => {
                eprintln!("gauntlet inspect: --iteration {number} is not valid: {error}");
                return ExitCode::FAILURE;
            }
        }
    }
    if show_verifications || unit_filter.is_some() {
        let records = match surface.state.verification_records(&run_id, deadline) {
            Ok(records) => records,
            Err(error) => {
                eprintln!("gauntlet inspect: {error}");
                return ExitCode::FAILURE;
            }
        };
        for record in &records {
            if let Some(Ok(filter)) = &unit_filter
                && record.unit_id() != filter
            {
                continue;
            }
            println!(
                "inspect: verification unit={} outcome={} evidence={}",
                record.unit_id(),
                record.outcome(),
                record.evidence()
            );
        }
    }
    if show_settlements {
        let rows = match surface.state.settlements(&run_id, deadline) {
            Ok(rows) => rows,
            Err(error) => {
                eprintln!("gauntlet inspect: {error}");
                return ExitCode::FAILURE;
            }
        };
        for row in &rows {
            println!(
                "inspect: settlement unit={} verification_ref={} evidence={}",
                row.unit_id(),
                row.verification(),
                row.evidence()
            );
        }
    }
    ExitCode::SUCCESS
}

/// `gauntlet export --run <run-id> [--out <path>]`.
pub fn export(args: &[String]) -> ExitCode {
    let mut run_id: Option<String> = None;
    let mut out: Option<std::path::PathBuf> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--run" => match args.get(index + 1) {
                Some(value) => {
                    run_id = Some(value.clone());
                    index += 2;
                }
                None => {
                    eprintln!("gauntlet export: --run requires a run id");
                    return ExitCode::from(2);
                }
            },
            "--out" => match args.get(index + 1) {
                Some(value) => {
                    out = Some(std::path::PathBuf::from(value));
                    index += 2;
                }
                None => {
                    eprintln!("gauntlet export: --out requires a path");
                    return ExitCode::from(2);
                }
            },
            other => {
                eprintln!("gauntlet export: unknown argument '{other}'");
                println!("usage: gauntlet export --run <run-id> [--out <path>]");
                return ExitCode::from(2);
            }
        }
    }
    let Some(run_id) = run_id else {
        eprintln!("gauntlet export: --run is required");
        println!("usage: gauntlet export --run <run-id> [--out <path>]");
        return ExitCode::from(2);
    };
    let Ok(run_id) = gauntlet_domain::RunId::new(&run_id) else {
        eprintln!("gauntlet export: {run_id} is not a valid run id (ULID grammar)");
        return ExitCode::FAILURE;
    };
    let context = match resolve_context(None) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("gauntlet export: the configuration did not resolve: {error}");
            return ExitCode::FAILURE;
        }
    };
    let surface = match open_operator_surface(&context) {
        Ok(surface) => surface,
        Err(error) => {
            eprintln!("gauntlet export: {error}");
            return ExitCode::FAILURE;
        }
    };
    match gauntlet_app::state::export::export_run(surface.state.as_ref(), &run_id, read_bound()) {
        Ok(text) => {
            if text.is_empty() {
                eprintln!("gauntlet export: run {run_id} has no durable history to export");
                return ExitCode::FAILURE;
            }
            match out {
                Some(path) => {
                    if let Err(error) = std::fs::write(&path, &text) {
                        eprintln!("gauntlet export: writing {}: {error}", path.display());
                        return ExitCode::FAILURE;
                    }
                    println!(
                        "export: run_id={run_id} out={} bytes={}",
                        path.display(),
                        text.len()
                    );
                }
                None => {
                    print!("{text}");
                }
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("gauntlet export: {error}");
            ExitCode::FAILURE
        }
    }
}
