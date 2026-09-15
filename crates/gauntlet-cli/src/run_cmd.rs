//! `gauntlet run` / `gauntlet resume` (spec §20.1; plan Task 20, brief
//! Item 2 — the milestone): the composed control path from the ONE
//! canonical `ControllerDeps` literal ([crate::compose]) through the
//! EXISTING [`RunController`] — doctor → freeze → plan → dispatch →
//! observe → compare → verify → select → settle → checkpoint → report.
//!
//! `--dry-run` (charter D1): the FULL read-only preflight — doctor
//! clean, config resolved, reference parsed AND verified unchanged
//! against `.gauntlet/reference.lock`, scheduler plan preview printed —
//! and ZERO writes: no freeze, no state rows, no evidence pins, no
//! runner required. Exits 0 with the preview on stdout.
//!
//! COR-R3-3 (falsifier round 3, M-R3-1): every composed path takes the
//! state dir's WRITER role exclusively for its lifetime (the
//! [crate::writer_lock] guard) — a second concurrent run/resume refuses
//! typed, naming the holder pid and the manual stale-lock recovery; the
//! dry-run stays read-only and takes no lock.

use std::process::ExitCode;

use gauntlet_app::control::controller::RunOutcome;
use gauntlet_app::control::scheduler::{Concurrency, PlanInput, PlannedDispatch, UnitScheduler};

use crate::compose::{self, CompositionError};
use crate::context::{RuntimeContext, resolve_context};

/// `gauntlet run [--dry-run] [--config <path>] "<intent>"`.
pub fn run(args: &[String]) -> ExitCode {
    let mut dry_run = false;
    let mut config: Option<std::path::PathBuf> = None;
    let mut intent_parts: Vec<String> = Vec::new();
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--dry-run" => {
                if dry_run {
                    eprintln!("gauntlet run: --dry-run given twice");
                    return ExitCode::from(2);
                }
                dry_run = true;
                index += 1;
            }
            "--config" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("gauntlet run: --config requires a path");
                    return ExitCode::from(2);
                };
                if config.replace(std::path::PathBuf::from(value)).is_some() {
                    eprintln!("gauntlet run: --config given twice");
                    return ExitCode::from(2);
                }
                index += 2;
            }
            other if other.starts_with('-') => {
                eprintln!("gauntlet run: unknown argument '{other}'");
                println!("usage: gauntlet run [--dry-run] [--config <path>] \"<intent>\"");
                return ExitCode::from(2);
            }
            other => {
                intent_parts.push(other.to_owned());
                index += 1;
            }
        }
    }
    if intent_parts.is_empty() {
        eprintln!("gauntlet run: the operator intent is required (a quoted phrase)");
        println!("usage: gauntlet run [--dry-run] [--config <path>] \"<intent>\"");
        return ExitCode::from(2);
    }
    let intent = intent_parts.join(" ");
    let context = match resolve_context(config.as_deref()) {
        Ok(context) => context,
        Err(error) => {
            eprintln!(
                "gauntlet run: the configuration did not resolve: {error} (see gauntlet doctor)"
            );
            return ExitCode::FAILURE;
        }
    };
    if dry_run {
        dry_run_preview(&context, config.as_deref(), &intent)
    } else {
        composed_run(&context, config.as_deref(), &intent)
    }
}

/// `gauntlet resume <run_id> [--config <path>]`: the run composition's
/// resume path (controller.resume_run, §18.3).
pub fn resume(args: &[String]) -> ExitCode {
    let mut config: Option<std::path::PathBuf> = None;
    let mut run_id: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--config" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("gauntlet resume: --config requires a path");
                    return ExitCode::from(2);
                };
                if config.replace(std::path::PathBuf::from(value)).is_some() {
                    eprintln!("gauntlet resume: --config given twice");
                    return ExitCode::from(2);
                }
                index += 2;
            }
            other if other.starts_with('-') => {
                eprintln!("gauntlet resume: unknown argument '{other}'");
                println!("usage: gauntlet resume <run_id> [--config <path>]");
                return ExitCode::from(2);
            }
            other => {
                if run_id.replace(other.to_owned()).is_some() {
                    eprintln!("gauntlet resume: exactly one run id is required");
                    return ExitCode::from(2);
                }
                index += 1;
            }
        }
    }
    let Some(run_id) = run_id else {
        eprintln!("gauntlet resume: a run id is required");
        println!("usage: gauntlet resume <run_id> [--config <path>]");
        return ExitCode::from(2);
    };
    let Ok(run_id) = gauntlet_domain::RunId::new(&run_id) else {
        eprintln!("gauntlet resume: {run_id} is not a valid run id (ULID grammar)");
        return ExitCode::FAILURE;
    };
    let context = match resolve_context(config.as_deref()) {
        Ok(context) => context,
        Err(error) => {
            eprintln!(
                "gauntlet resume: the configuration did not resolve: {error} (see gauntlet doctor)"
            );
            return ExitCode::FAILURE;
        }
    };
    if let Some(code) = doctor_prefail(&context, config.as_deref(), "gauntlet resume") {
        return code;
    }
    // COR-R3-3: the single-writer admission (the guard's lifetime is this
    // command's; released on every exit path by the handle's drop).
    let _writer_lock = match crate::writer_lock::WriterLock::acquire(context.state_dir()) {
        Ok(lock) => lock,
        Err(error) => return crate::writer_lock::report_failure("gauntlet resume", &error),
    };
    let composed = match compose::compose_run(&context, "resume") {
        Ok(composed) => composed,
        Err(error) => {
            eprintln!("gauntlet resume: {error}");
            return ExitCode::FAILURE;
        }
    };
    match composed.controller.resume_run(&run_id) {
        Ok(outcome) => {
            print_outcome(&outcome);
            print_run_report(&composed, &outcome);
            exit_of(&outcome)
        }
        Err(error) => {
            eprintln!("gauntlet resume: run {run_id}: {error}");
            ExitCode::FAILURE
        }
    }
}

/// The doctor gate every composed path shares (brief Item 2: reuse the
/// existing measurement plumbing). Returns `Some(FAILURE)` when the
/// preflight is not clean.
fn doctor_prefail(
    _context: &RuntimeContext,
    config: Option<&std::path::Path>,
    surface: &str,
) -> Option<ExitCode> {
    let report = doctor_report(config);
    if report.exit_ok() {
        None
    } else {
        eprintln!("{surface}: the §8.5 preflight is not clean — refusing to run");
        eprintln!("{}", report.human_summary());
        Some(ExitCode::FAILURE)
    }
}

/// The doctor report for THIS composition (read-only; the report FILE is
/// only written by the `doctor` command itself). The `--config` override
/// reaches the preflight the same way it reaches every other surface.
fn doctor_report(config: Option<&std::path::Path>) -> gauntlet_app::doctor::DoctorReport {
    let mut input = gauntlet_app::doctor::DoctorInput::new(
        crate::spec_vocab::ANNEX_E_ENV,
        crate::spec_vocab::CAPABILITY_VOCABULARY,
    );
    input.backend_measure = Some(crate::context::measure_backend);
    input.runner_measure = Some(crate::context::measure_runner);
    input.config_override = config.map(std::path::PathBuf::from);
    gauntlet_app::doctor::run_doctor(&input)
}

/// The D1 dry-run: preflight, reference parse + lock verification, plan
/// preview — ZERO writes, exit 0.
fn dry_run_preview(
    context: &RuntimeContext,
    config: Option<&std::path::Path>,
    intent: &str,
) -> ExitCode {
    let report = doctor_report(config);
    if !report.exit_ok() {
        eprintln!("gauntlet run: the §8.5 preflight is not clean — the dry-run stops here");
        eprintln!("{}", report.human_summary());
        return ExitCode::FAILURE;
    }
    println!("run: dry-run — preflight clean, zero writes ahead");
    // Reference: strict parse (the frozen ruler must be a valid ruler).
    let key_text = match std::fs::read_to_string(&context.resolved.reference.answer_key) {
        Ok(text) => text,
        Err(error) => {
            eprintln!(
                "gauntlet run: reading {}: {error}",
                context.resolved.reference.answer_key.display()
            );
            return ExitCode::FAILURE;
        }
    };
    let parsed = gauntlet_app::reference::parse::parse_answer_key(&key_text);
    if !parsed.failures.is_empty() || parsed.units.is_empty() {
        eprintln!(
            "gauntlet run: the answer key does not strictly parse ({} failure(s), {} unit(s))",
            parsed.failures.len(),
            parsed.units.len()
        );
        return ExitCode::FAILURE;
    }
    println!(
        "reference: {} unit(s) parsed from {}",
        parsed.units.len(),
        context.resolved.reference.answer_key.display()
    );
    // Reference: unchanged against the lock (the typed `just ruler` twin;
    // digest agreement without freezing — no writes).
    match crate::reference_lock::verify_lock(&context.project_root) {
        Ok(lock) => {
            println!(
                "reference: lock verified ({} unit(s) frozen)",
                lock.unit_count
                    .map(|n| n.to_string())
                    .unwrap_or_else(|| "?".to_owned())
            );
        }
        Err(error) => {
            eprintln!("gauntlet run: {error}");
            return ExitCode::FAILURE;
        }
    }
    // Plan preview: the scheduler's validation plan over the parsed units.
    match plan_preview(context, intent, &parsed.units) {
        Ok(lines) => {
            for line in lines {
                println!("{line}");
            }
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("gauntlet run: {error}");
            ExitCode::FAILURE
        }
    }
}

/// The plan preview lines (the scheduler's FIRST-iteration plan, printed
/// `key=value` per §20.3). The preview builds no controller, opens no
/// store, and writes nothing.
fn plan_preview(
    context: &RuntimeContext,
    intent: &str,
    units: &[gauntlet_domain::Unit],
) -> Result<Vec<String>, CompositionError> {
    let limits =
        context
            .resolved
            .budget
            .limits()
            .map_err(|error| CompositionError::Configuration {
                key: "budget.max_rounds",
                env: "GAUNTLET_MAX_ROUNDS",
                detail: error.to_string(),
            })?;
    let reserve = context.resolved.budget.recovery_reserve_pct;
    let diagnostic = context.resolved.budget.diagnostic_allowance();
    let budget = gauntlet_domain::Budget::new(limits, reserve, diagnostic).map_err(|error| {
        CompositionError::Configuration {
            key: "budget.max_tokens",
            env: "GAUNTLET_MAX_TOKENS",
            detail: error.to_string(),
        }
    })?;
    let meter = gauntlet_adapter_meter_memory::MeteredBudget::new(budget);
    let estimates: std::collections::BTreeMap<
        gauntlet_domain::UnitId,
        gauntlet_ports::budget_meter::BudgetSlice,
    > = units
        .iter()
        .map(|unit| (unit.id().clone(), per_attempt_share(&limits, reserve)))
        .collect();
    let mut tier_capacity: Vec<(gauntlet_domain::BlastRadius, u32)> = Vec::new();
    for unit in units {
        let tier = unit.blast_radius();
        if let Some(entry) = tier_capacity.iter_mut().find(|(key, _)| key == &tier) {
            entry.1 += 1;
        } else {
            tier_capacity.push((tier, 1));
        }
    }
    let input = PlanInput {
        units: units.to_vec(),
        claimed: Vec::new(),
        concurrency: Concurrency {
            global_capacity: limits.max_parallel(),
            global_in_flight: 0,
            tier_capacity,
            tier_in_flight: Vec::new(),
        },
        // §11.3 slicing has NO producer on any shipped surface (charter
        // D7, the pinned record): the map is empty by structure, and the
        // preview invents no cohort key.
        cohorts: std::collections::BTreeMap::new(),
        estimates,
        statuses: std::collections::BTreeMap::new(),
        configured_roles: vec![gauntlet_domain::RoleId::Builder],
    };
    let planned =
        UnitScheduler::new()
            .plan(&input, &meter)
            .map_err(|error| CompositionError::Plan {
                detail: error.to_string(),
            })?;
    let mut lines = vec![
        format!("run: intent={intent:?}"),
        format!(
            "plan: {} dispatch(es) planned for the first iteration (parallelism={})",
            planned.len(),
            limits.max_parallel()
        ),
    ];
    for dispatch in &planned {
        lines.push(preview_line(dispatch));
    }
    lines.push(
        "run: dry-run complete — no freeze, no state rows, no evidence pins, no dispatch"
            .to_owned(),
    );
    Ok(lines)
}

fn preview_line(dispatch: &PlannedDispatch) -> String {
    format!(
        "plan: units=[{}] tier={} authority={} depth={}",
        dispatch
            .units
            .iter()
            .map(|unit| unit.to_string())
            .collect::<Vec<_>>()
            .join(","),
        dispatch.blast_radius,
        dispatch.authority,
        dispatch.depth
    )
}

/// The per-attempt share the preview budgets per unit — the same forward
/// share expression the controller's per-attempt estimate uses (the
/// reserve removed first, the remainder divided by the rounds): one
/// round's worth of the run's own ceilings.
fn per_attempt_share(
    limits: &gauntlet_domain::Limits,
    reserve_pct: u8,
) -> gauntlet_ports::budget_meter::BudgetSlice {
    let forward_pct = 100u64.saturating_sub(u64::from(reserve_pct));
    let rounds = u64::from(limits.max_rounds().max(1));
    let share = |ceiling: u64| -> u64 {
        ceiling
            .saturating_mul(forward_pct)
            .saturating_div(100)
            .saturating_div(rounds)
    };
    gauntlet_ports::budget_meter::BudgetSlice {
        rounds: 1,
        wall_seconds: share(limits.max_wall_seconds()),
        tokens: share(limits.max_tokens()),
        cost_usd: gauntlet_domain::MicroUsd::new(share(limits.max_cost_usd().get())),
    }
}

/// The composed run: doctor gate → writer-lock admission → composition →
/// start_run → report.
fn composed_run(
    context: &RuntimeContext,
    config: Option<&std::path::Path>,
    intent: &str,
) -> ExitCode {
    if let Some(code) = doctor_prefail(context, config, "gauntlet run") {
        return code;
    }
    // COR-R3-3: the single-writer admission (the guard's lifetime is this
    // command's; released on every exit path by the handle's drop).
    let _writer_lock = match crate::writer_lock::WriterLock::acquire(context.state_dir()) {
        Ok(lock) => lock,
        Err(error) => return crate::writer_lock::report_failure("gauntlet run", &error),
    };
    let composed = match compose::compose_run(context, intent) {
        Ok(composed) => composed,
        Err(error) => {
            eprintln!("gauntlet run: {error}");
            return ExitCode::FAILURE;
        }
    };
    match composed.controller.start_run() {
        Ok(outcome) => {
            print_outcome(&outcome);
            print_run_report(&composed, &outcome);
            print_delivery_hint(context, &outcome.run_id);
            exit_of(&outcome)
        }
        Err(error) => {
            eprintln!("gauntlet run: {error}");
            ExitCode::FAILURE
        }
    }
}

fn print_outcome(outcome: &RunOutcome) {
    println!("run: run_id={}", outcome.run_id);
    println!(
        "run: terminal={} iterations={}",
        outcome.terminal, outcome.iterations
    );
    if let Some(breach) = &outcome.reference_breach {
        println!("run: reference_breach={breach:?}");
    }
    if let Some(summary) = &outcome.escalation_summary {
        println!("run: escalated — {summary}");
    }
}

/// W5: when this run's integration tree differs from its baseline, name
/// the delivery command — the operator's own path to get a settled work
/// product out of `<state_dir>/integration/` and into the project. Never
/// fails the run: a read failure here prints nothing.
fn print_delivery_hint(context: &RuntimeContext, run_id: &gauntlet_domain::RunId) {
    if crate::deliver_cmd::has_pending_delivery(context, run_id) {
        println!("delivery: gauntlet deliver --run {run_id} --apply");
    }
}

fn exit_of(outcome: &RunOutcome) -> ExitCode {
    match outcome.terminal {
        gauntlet_domain::TerminalState::Settled => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

/// The six-category report after a terminal state (brief Item 4): the
/// CLI's real command table is the capability manifest (charter D8 —
/// the manifest declares, the records decide).
fn print_run_report(composed: &compose::ComposedRun, outcome: &RunOutcome) {
    let manifest = crate::report_manifest::cli_capability_manifest();
    let deadline = crate::context::doctor_bound();
    let store = composed.state.clone();
    match gauntlet_app::report::build_report(store.as_ref(), &outcome.run_id, &manifest, deadline) {
        Ok(report) => {
            println!("{}", report.human_summary());
            println!("{}", report.to_json());
        }
        Err(error) => {
            eprintln!("gauntlet: the run report could not be built from the records: {error}");
        }
    }
}
