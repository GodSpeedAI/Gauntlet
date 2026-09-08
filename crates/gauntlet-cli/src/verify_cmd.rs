//! `gauntlet verify` (spec §20.1; plan Task 20, charter D8; task 27 for
//! `--all`): re-runs ladder verification for `--unit` or a whole `--run`
//! through the [`VerificationAuthority`] + the composed §14.2 rung set.
//! Prints the THREE-STATE outcomes (PASS / FAIL / INCONCLUSIVE stay
//! three states — nothing collapses them) and NEVER settles: this
//! command writes nothing; a verification record here is a read-side
//! re-run, and the durable settlement path stays the loop's alone.
//!
//! `gauntlet verify --all [--report <path>] [--manifest <path>]`
//! (task 27, charter decision 1) is the conformance sweep: a READ-ONLY
//! instrument surface over [`gauntlet_app::conformance`] that
//! adjudicates every frozen-ruler unit against the authored manifest
//! and writes ONLY the report + the evidence directory. `--all` and
//! `--run` are mutually exclusive (usage error, exit 2); the sweep
//! exits 0 iff it completed with zero FAIL outcomes (INCONCLUSIVE is
//! allowed — honest reporting); any refusal or FAIL exits non-zero
//! with no half-written report. The process toolchain (the repository
//! test runner and its compile-lock discipline) is composed by the
//! adapter layer ([`sweep_toolchain`]) — this composition root wires,
//! it does not spell tool names.

use std::process::ExitCode;

use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::verifier::VerificationRequest;

use crate::compose::{self, open_operator_surface};
use crate::context::resolve_context;

fn bound() -> Deadline {
    crate::context::doctor_bound()
}

/// The sweep's package vocabulary: the core workspace packages the
/// application layer pins by default, EXTENDED with the two runner
/// adapters this composition root wires (their contract suites and
/// fixture playback are conformance evidence). Adapter names live here
/// and only here — the composition root, outside the adapter-identifier
/// denylist's scope.
pub fn sweep_packages() -> &'static [&'static str] {
    const PACKAGES: &[&str] = &[
        "gauntlet-domain",
        "gauntlet-ports",
        "gauntlet-app",
        "gauntlet-cli",
        "gauntlet-arch-tests",
        "gauntlet-adapter-agent-mock",
        "gauntlet-adapter-agent-cli",
    ];
    PACKAGES
}

/// The sweep's FEATURE-edge vocabulary, discovered from the package
/// allowlist's own `[features]` tables: gauntlet-app declares
/// `composition`, gauntlet-cli declares `surfaces`, and every other
/// allowlisted package (domain, ports, arch-tests, both runner
/// adapters) declares none; `default` is implicit and deliberately
/// absent. The application layer's default pins the same core edges
/// (and a test pins THAT const to the Cargo.tomls); this composition
/// root list is what the committed manifest validates against.
pub fn sweep_features() -> &'static [&'static str] {
    const FEATURES: &[&str] = &["composition", "surfaces"];
    FEATURES
}

/// The sweep's process executor: maps the application layer's
/// STRUCTURAL commands onto adapter-composed invocations. Nextest arms
/// run under the repository compile-lock discipline with the lock
/// acquisition bounded and the wrapped exit status preserved; the
/// FLOCK-LEVEL outcomes — a lock that stayed busy, and a wrapper that
/// could not execute its command — are the typed environmental refusal
/// (INCONCLUSIVE, charter decision 3), never FAIL-by-default; the
/// wrapper remaps an inner command's own reserved code so the
/// flock-level pair stays unambiguous (COR-1). Bare-program arms (the
/// fixture world) run directly — no compile, no lock, no reserved-code
/// reinterpretation: their exit is their own.
struct SweepProcessExecutor;

impl gauntlet_app::conformance::ConformanceExecutor for SweepProcessExecutor {
    fn execute(
        &mut self,
        command: &gauntlet_app::conformance::SweepCommand,
    ) -> Result<gauntlet_app::conformance::CommandOutcome, gauntlet_app::conformance::ExecuteError>
    {
        use gauntlet_adapter_verify_command_exit_code::sweep_toolchain as chain;
        use gauntlet_app::conformance::NextestSelector;

        // Only the locked arm's exit codes are flock-interpreted; the
        // bare-program arm owns every code it produces.
        let locked = matches!(
            command,
            gauntlet_app::conformance::SweepCommand::Nextest { .. }
        );
        let invocation = match command {
            gauntlet_app::conformance::SweepCommand::Nextest {
                package,
                selector,
                filter,
                features,
            } => {
                let (target, lib) = match selector {
                    NextestSelector::Test(name) => (Some(name.as_str()), false),
                    NextestSelector::Lib => (None, true),
                };
                let edges: Vec<&str> = features.iter().map(String::as_str).collect();
                let inner =
                    chain::nextest_invocation(package, target, lib, filter.as_deref(), &edges);
                chain::lock_wrapped(&inner)
            }
            gauntlet_app::conformance::SweepCommand::Program { path } => chain::ToolInvocation {
                program: path.clone(),
                args: Vec::new(),
                display: path.clone(),
            },
        };
        let started = std::time::Instant::now();
        let run = std::process::Command::new(&invocation.program)
            .args(&invocation.args)
            .output();
        let elapsed = started.elapsed().as_millis() as u64;
        match run {
            Ok(status) => {
                // COR-3 F10: a signal-killed command (the OOM-killer
                // shape) has NO exit code — fabricating one would turn
                // an environmental kill into a FAIL-by-default, the
                // exact shape charter decision 3 forbids. The typed
                // environmental refusal names the signal; no evidence
                // log is written for a command that never completed
                // (so no log can record a fabricated exit either).
                let Some(code) = status.status.code() else {
                    #[cfg(unix)]
                    let signal = std::os::unix::process::ExitStatusExt::signal(&status.status);
                    #[cfg(not(unix))]
                    let signal: Option<i32> = None;
                    let signal = signal.unwrap_or(0);
                    return Err(gauntlet_app::conformance::ExecuteError::CannotRun {
                        reason: format!(
                            "the sweep command was killed by signal {signal} (no exit \
                             code exists)"
                        ),
                    });
                };
                // COR-4 R2-F1/R2-F2: on a LOCKED arm the exit codes are
                // flock-interpreted through the toolchain's classification
                // (the wrapper remaps the inner command's own 69/86 to
                // 169/186 and an inner signal death to 200+signal, so the
                // lock-level pair {86, 69} stays unambiguous). A bare
                // program's exit is its own.
                let classification = if locked {
                    Some(chain::classify_exit(code))
                } else {
                    None
                };
                match classification {
                    Some(
                        chain::ExitClassification::LockBusy
                        | chain::ExitClassification::LockExecFailed,
                    ) => {
                        let detail = String::from_utf8_lossy(&status.stderr);
                        let first = detail
                            .lines()
                            .find(|line| !line.trim().is_empty())
                            .unwrap_or_default()
                            .to_owned();
                        let reason = if code == chain::LOCK_BUSY_EXIT {
                            "the repository compile lock stayed busy past the wrapper's \
                             bounded wait"
                                .to_owned()
                        } else {
                            format!("the lock wrapper could not execute the sweep command: {first}")
                        };
                        return Err(gauntlet_app::conformance::ExecuteError::CannotRun { reason });
                    }
                    Some(chain::ExitClassification::InnerSignal { signal }) => {
                        // The inner command itself was killed (the
                        // realistic OOM victim); the wrapper's band remap
                        // preserved the signal fact. Environmental, never
                        // FAIL-by-default.
                        return Err(gauntlet_app::conformance::ExecuteError::CannotRun {
                            reason: format!(
                                "the sweep command was killed by signal {signal} (the \
                                 wrapper's shell encoded it as {}; the band remap carried \
                                 the signal through)",
                                128 + signal
                            ),
                        });
                    }
                    _ => {}
                }
                // The exit note carries the remap provenance into the
                // FAIL reason (COR-4 R2-F1: an inner own-86 must FAIL,
                // naming what happened — never hide as environmental).
                let exit_note = match classification {
                    Some(chain::ExitClassification::InnerFailure { inner, .. })
                        if inner != code =>
                    {
                        format!(
                            "the wrapper remaps an inner exit of {inner}; a direct exit of \
                             {code} is indistinguishable from the remapped form (disclosed)"
                        )
                    }
                    _ => String::new(),
                };
                Ok(gauntlet_app::conformance::CommandOutcome {
                    display: invocation.display,
                    exit_code: code,
                    stdout: status.stdout,
                    stderr: status.stderr,
                    duration_ms: elapsed,
                    exit_note,
                })
            }
            Err(error) => Err(gauntlet_app::conformance::ExecuteError::CannotRun {
                reason: format!("the sweep command could not be spawned: {error}"),
            }),
        }
    }
}

/// `gauntlet verify --run <run-id> [--unit <unit-id>]` or
/// `gauntlet verify --all [--report <path>] [--manifest <path>]`.
#[allow(clippy::arc_with_non_send_sync)] // single-threaded re-verification surface
pub fn verify(args: &[String]) -> ExitCode {
    let mut run_id: Option<String> = None;
    let mut unit: Option<String> = None;
    let mut all = false;
    let mut report: Option<String> = None;
    let mut manifest: Option<String> = None;
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
            "--all" => {
                all = true;
                index += 1;
            }
            "--report" => match args.get(index + 1) {
                Some(value) => {
                    report = Some(value.clone());
                    index += 2;
                }
                None => {
                    eprintln!("gauntlet verify: --report requires a path");
                    println!("usage: gauntlet verify --all [--report <path>] [--manifest <path>]");
                    return ExitCode::from(2);
                }
            },
            "--manifest" => match args.get(index + 1) {
                Some(value) => {
                    manifest = Some(value.clone());
                    index += 2;
                }
                None => {
                    eprintln!("gauntlet verify: --manifest requires a path");
                    println!("usage: gauntlet verify --all [--report <path>] [--manifest <path>]");
                    return ExitCode::from(2);
                }
            },
            other => {
                eprintln!("gauntlet verify: unknown argument '{other}'");
                if all {
                    println!("usage: gauntlet verify --all [--report <path>] [--manifest <path>]");
                } else {
                    println!("usage: gauntlet verify --run <run-id> [--unit <unit-id>]");
                }
                return ExitCode::from(2);
            }
        }
    }
    if all {
        if run_id.is_some() {
            eprintln!("gauntlet verify: --all and --run are mutually exclusive");
            println!("usage: gauntlet verify --all [--report <path>] [--manifest <path>]");
            return ExitCode::from(2);
        }
        if unit.is_some() {
            eprintln!("gauntlet verify: --unit belongs to --run, not --all");
            println!("usage: gauntlet verify --all [--report <path>] [--manifest <path>]");
            return ExitCode::from(2);
        }
        return verify_all(report.as_deref(), manifest.as_deref());
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

/// The conformance sweep (task 27): read-only, writes ONLY the report
/// and the evidence directory, never settles anything.
fn verify_all(report: Option<&str>, manifest: Option<&str>) -> ExitCode {
    let context = match resolve_context(None) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("gauntlet verify --all: the configuration did not resolve: {error}");
            return ExitCode::FAILURE;
        }
    };
    let root = context.project_root.clone();
    let reports = gauntlet_app::config::defaults::report_dir_at(&root);
    let report_path = match report {
        Some(path) => root.join(path),
        None => reports.join("conformance.json"),
    };
    let manifest_path = match manifest {
        Some(path) => root.join(path),
        None => reports.join("conformance-manifest.yaml"),
    };
    let answer_key = context.resolved.reference.answer_key.clone();
    let ruler_text = match std::fs::read_to_string(&answer_key) {
        Ok(text) => text,
        Err(error) => {
            eprintln!(
                "gauntlet verify --all: reading {}: {error}",
                answer_key.display()
            );
            return ExitCode::FAILURE;
        }
    };
    let manifest_text = match std::fs::read_to_string(&manifest_path) {
        Ok(text) => text,
        Err(error) => {
            eprintln!(
                "gauntlet verify --all: reading {}: {error}",
                manifest_path.display()
            );
            return ExitCode::FAILURE;
        }
    };
    let tree_commit = match head_commit(&root) {
        Ok(commit) => commit,
        Err(error) => {
            eprintln!("gauntlet verify --all: {error}");
            return ExitCode::FAILURE;
        }
    };
    let (generated_at, evidence_stamp) = utc_now();
    let display_answer_key = display_path(&root, &answer_key);
    let display_manifest = display_path(&root, &manifest_path);
    let inputs = gauntlet_app::conformance::SweepInputs {
        ruler_text: &ruler_text,
        answer_key_path: display_answer_key,
        manifest_text: &manifest_text,
        manifest_path: display_manifest,
        workspace_root: root.as_path(),
        allowed_packages: sweep_packages(),
        allowed_features: sweep_features(),
        generated_at,
        evidence_stamp,
        tree_commit,
    };
    let mut executor = SweepProcessExecutor;
    let outcome = match gauntlet_app::conformance::run_sweep(&mut executor, &inputs) {
        Ok(outcome) => outcome,
        Err(refusal) => {
            eprintln!("gauntlet verify --all: REFUSED — {refusal}");
            eprintln!("gauntlet verify --all: no report was written");
            return ExitCode::FAILURE;
        }
    };
    // Write the evidence directory first, then the report, so the
    // report's evidence refs exist on disk the moment it lands.
    let parent = report_path
        .parent()
        .map(std::path::Path::to_path_buf)
        .unwrap_or_else(|| root.clone());
    for (relative, bytes) in &outcome.evidence_files {
        let target = parent.join(relative);
        if let Some(directory) = target.parent()
            && let Err(error) = std::fs::create_dir_all(directory)
        {
            eprintln!(
                "gauntlet verify --all: creating {}: {error}",
                directory.display()
            );
            return ExitCode::FAILURE;
        }
        if let Err(error) = std::fs::write(&target, bytes) {
            eprintln!(
                "gauntlet verify --all: writing {}: {error}",
                target.display()
            );
            return ExitCode::FAILURE;
        }
    }
    let mut report_text = outcome.report.to_json_text();
    report_text.push('\n');
    if let Err(error) = std::fs::write(&report_path, report_text.as_bytes()) {
        eprintln!(
            "gauntlet verify --all: writing {}: {error}",
            report_path.display()
        );
        return ExitCode::FAILURE;
    }
    println!(
        "verify --all: units={} pass={} inconclusive={} fail={} commands={} — report at {}",
        outcome.pass + outcome.inconclusive + outcome.fail,
        outcome.pass,
        outcome.inconclusive,
        outcome.fail,
        outcome.commands_executed,
        report_path.display()
    );
    println!("verify --all: read-only sweep — NOTHING settled");
    if outcome.fail > 0 {
        ExitCode::FAILURE
    } else {
        ExitCode::SUCCESS
    }
}

/// The tree's head commit, read from the working tree's git directory
/// (HEAD → ref file → packed-refs fallback). Read-only.
fn head_commit(root: &std::path::Path) -> Result<String, String> {
    let git = root.join(".git");
    let head = std::fs::read_to_string(git.join("HEAD"))
        .map_err(|error| format!("reading the tree's HEAD: {error}"))?;
    let head = head.trim();
    if let Some(reference) = head.strip_prefix("ref: ") {
        let direct = git.join(reference);
        if let Ok(commit) = std::fs::read_to_string(&direct) {
            return Ok(commit.trim().to_owned());
        }
        let packed = std::fs::read_to_string(git.join("packed-refs")).unwrap_or_default();
        for line in packed.lines() {
            if let Some((commit, name)) = line.split_once(' ')
                && name.trim() == reference
            {
                return Ok(commit.to_owned());
            }
        }
        return Err(format!("the ref {reference} could not be resolved"));
    }
    if head.len() == 40 && head.chars().all(|c| c.is_ascii_hexdigit()) {
        return Ok(head.to_owned());
    }
    Err("the tree's HEAD has an unrecognized shape".to_owned())
}

/// The repo-relative display spelling of an anchored path (absolute
/// paths under the root lose the root prefix; others pass through).
fn display_path(root: &std::path::Path, path: &std::path::Path) -> String {
    match path.strip_prefix(root) {
        Ok(relative) => relative
            .components()
            .map(|part| part.as_os_str().to_string_lossy().to_string())
            .collect::<Vec<String>>()
            .join("/"),
        Err(_) => path.display().to_string(),
    }
}

/// Days-since-epoch to the civil date (the standard era arithmetic —
/// boring, inspectable, no dependencies).
fn civil_from_days(day_number: i64) -> (i64, u32, u32) {
    let shifted = day_number + 719_468;
    let era = shifted.div_euclid(146_097);
    let day_of_era = shifted.rem_euclid(146_097);
    let year_of_era = (day_of_era - day_of_era.div_euclid(1_461) + day_of_era.div_euclid(36_524)
        - day_of_era.div_euclid(146_096))
    .div_euclid(365);
    let year = year_of_era + era * 400;
    let day_of_year =
        day_of_era - (365 * year_of_era + year_of_era.div_euclid(4) - year_of_era.div_euclid(100));
    let mp = (5 * day_of_year + 2).div_euclid(153);
    let day = day_of_year - (153 * mp + 2).div_euclid(5) + 1;
    let month = if mp < 10 { mp + 3 } else { mp - 9 };
    let year = if month <= 2 { year + 1 } else { year };
    (year, month as u32, day as u32)
}

/// The current UTC instant as (RFC3339, compact stamp). Caller-resolved
/// time: the application layer owns no wall clock.
fn utc_now() -> (String, String) {
    let total = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|since| since.as_secs() as i64)
        .unwrap_or_default();
    let (days, within_day) = (total.div_euclid(86_400), total.rem_euclid(86_400));
    let (year, month, day) = civil_from_days(days);
    let (hour, minute, second) = (
        within_day.div_euclid(3_600),
        within_day.rem_euclid(3_600).div_euclid(60),
        within_day.rem_euclid(60),
    );
    (
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}Z"),
        format!("{year:04}{month:02}{day:02}T{hour:02}{minute:02}{second:02}Z"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use gauntlet_app::conformance::ConformanceExecutor as _;

    /// COR-3 F10 (RED batch): a signal-killed sweep command (the
    /// OOM-killer shape) is the typed environmental refusal naming the
    /// signal — never a fabricated "exit -1" FAIL. The stub kills
    /// ITSELF with signal 9; the executor must report CannotRun.
    #[test]
    fn a_signal_killed_command_is_the_typed_environmental_refusal() {
        let sep = std::path::MAIN_SEPARATOR;
        let mut body = String::new();
        body.push_str("#!");
        body.push(sep);
        body.push_str("bin");
        body.push(sep);
        body.push_str("sh\nkill -9 $$\n");
        let dir =
            std::env::temp_dir().join(format!("gauntlet-sweep-signal-{}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("scratch dir");
        let stub = dir.join("self-kill");
        std::fs::write(&stub, body).expect("stub write");
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&stub, std::fs::Permissions::from_mode(0o755))
                .expect("stub is made executable");
        }
        let mut executor = SweepProcessExecutor;
        let command = gauntlet_app::conformance::SweepCommand::Program {
            path: stub.to_string_lossy().to_string(),
        };
        match executor.execute(&command) {
            Err(gauntlet_app::conformance::ExecuteError::CannotRun { reason }) => {
                assert!(reason.contains("signal"), "{reason}");
                assert!(reason.contains('9'), "{reason}");
            }
            other => panic!("a killed command refuses as environmental, got {other:?}"),
        }
        let _ = std::fs::remove_dir_all(&dir);
    }
}
