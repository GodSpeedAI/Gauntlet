//! `gauntlet improve` (spec §19.3/§19.4; plan Task 23) — the optimizer
//! surface, now wired:
//!
//! - `improve propose [--from-result <path>] [--from-run <run-id>]` —
//!   dispatches the REAL optimizer role invocation through the configured
//!   runner (invoke → handle → bounded poll → collect at the declared
//!   result-contract path; never an await of an answer); `--from-result`
//!   admits an operator-supplied result document through the SAME strict
//!   decode — never a bypass of validation, just of the model call. Every
//!   admission is a durable PENDING revision; the optimizer has no other
//!   write path.
//! - `improve evaluate <revision> --held-out <dir> --regression <dir>
//!   [--protected a,b] [--metric <token>]` — the isolated evaluation: per
//!   task set a CONTROL run (current configuration) and a TREATMENT run
//!   (the candidate delta applied, `parent_run` set), both through the
//!   REAL composed loop (the executor seam over [`crate::compose::
//!   compose_run`]), both against the same frozen reference; the durable
//!   evaluation record lands in the revision ledger and each finished
//!   run's memory is imported through the §19.1 developmental import.
//! - `improve promote <revision>` — the §19.4 gate: ALL EIGHT conditions,
//!   typed per-condition refusals; FAILED ⇒ Rejected with recorded
//!   reasons; evidence missing ⇒ Provisional, NEVER promoted.
//! - `improve rollback <revision>` — the restore surface (append-only;
//!   the derived state flips to promoted-but-rolled-back).
//!
//! The command READS the capability manifest for its own row; the row is
//! wired, so the manifest and this surface agree by construction
//! (RES-15).

use std::process::ExitCode;
use std::sync::Arc;

use gauntlet_adapter_evidence_fs::FsEvidenceStore;
use gauntlet_adapter_state_sqlite::{EmbeddedCapabilityLedger, EmbeddedRevisionLedger};
use gauntlet_app::config::{EnvLayer, resolve as config_resolve};
use gauntlet_app::promote::delta::DeltaDocument;
use gauntlet_app::promote::evaluate::{
    EvaluationExecutor, EvaluationHarness, EvaluationRequest, ExecutionOutcome,
};
use gauntlet_app::promote::gate::{GateInput, GateVerdict, PromotionGate};
use gauntlet_app::promote::metrics::TargetMetric;
use gauntlet_app::promote::proposal::ProposalService;
use gauntlet_app::promote::rollback::RollbackService;
use gauntlet_ports::evidence_store::EvidenceStore;
use gauntlet_ports::revision_ledger::RevisionLedger;

/// `gauntlet improve <subcommand>`.
pub fn run(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("propose") => propose(&args[1..]),
        Some("evaluate") => evaluate(&args[1..]),
        Some("promote") => promote(&args[1..]),
        Some("rollback") => rollback(&args[1..]),
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
    println!("usage: gauntlet improve propose [--from-result <path>] [--from-run <run-id>]");
    println!("       gauntlet improve evaluate <revision> --held-out <dir> --regression <dir> \\");
    println!("                    [--protected <cap,cap>] [--metric <token>]");
    println!("       gauntlet improve promote <revision>");
    println!("       gauntlet improve rollback <revision>");
}

/// The revision ledger over the SAME embedded database as the run state
/// and the capability ledger (cross-run by design, charter D3).
fn revision_ledger(
    context: &crate::context::RuntimeContext,
) -> Result<Arc<EmbeddedRevisionLedger>, String> {
    let path = context.state_dir().join(crate::compose::STATE_DB_NAME);
    EmbeddedRevisionLedger::open(&path)
        .map(Arc::new)
        .map_err(|error| format!("opening the revision ledger at {}: {error}", path.display()))
}

fn capability_ledger(
    context: &crate::context::RuntimeContext,
) -> Result<Arc<EmbeddedCapabilityLedger>, String> {
    let path = context.state_dir().join(crate::compose::STATE_DB_NAME);
    EmbeddedCapabilityLedger::open(&path)
        .map(Arc::new)
        .map_err(|error| {
            format!(
                "opening the capability ledger at {}: {error}",
                path.display()
            )
        })
}

fn evidence_store(
    context: &crate::context::RuntimeContext,
) -> Result<Arc<FsEvidenceStore>, String> {
    FsEvidenceStore::open(&context.resolved.evidence_dir)
        .map(Arc::new)
        .map_err(|error| format!("opening the evidence store: {error}"))
}

/// The command's deadline bound (the doctor-probe convention).
fn improve_bound() -> gauntlet_ports::deadline::Deadline {
    crate::context::doctor_bound()
}

// ---------------------------------------------------------------------------
// propose
// ---------------------------------------------------------------------------

fn propose(args: &[String]) -> ExitCode {
    let mut config: Option<std::path::PathBuf> = None;
    let mut from_result: Option<std::path::PathBuf> = None;
    let mut from_run: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--config" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("gauntlet improve propose: --config requires a path");
                    return ExitCode::from(2);
                };
                config = Some(std::path::PathBuf::from(value));
                index += 2;
            }
            "--from-result" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("gauntlet improve propose: --from-result requires a path");
                    return ExitCode::from(2);
                };
                from_result = Some(std::path::PathBuf::from(value));
                index += 2;
            }
            "--from-run" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("gauntlet improve propose: --from-run requires a run id");
                    return ExitCode::from(2);
                };
                from_run = Some(value.clone());
                index += 2;
            }
            other => {
                eprintln!("gauntlet improve propose: unknown argument '{other}'");
                usage();
                return ExitCode::from(2);
            }
        }
    }
    let context = match crate::context::resolve_context(config.as_deref()) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("gauntlet improve propose: the configuration did not resolve: {error}");
            return ExitCode::FAILURE;
        }
    };
    let _writer_lock = match crate::writer_lock::WriterLock::acquire(context.state_dir()) {
        Ok(lock) => lock,
        Err(error) => {
            return crate::writer_lock::report_failure("gauntlet improve propose", &error);
        }
    };
    let ledger = match revision_ledger(&context) {
        Ok(ledger) => ledger,
        Err(error) => {
            eprintln!("gauntlet improve propose: {error}");
            return ExitCode::FAILURE;
        }
    };
    let service = ProposalService::new(ledger, Arc::new(SystemClock));
    let deadline = improve_bound();
    let admission = match from_result {
        Some(path) => {
            // The operator-supplied document takes the SAME strict decode;
            // --from-result bypasses the model call, never the validation.
            let document = match std::fs::read_to_string(&path) {
                Ok(document) => document,
                Err(error) => {
                    eprintln!(
                        "gauntlet improve propose: reading {}: {error}",
                        path.display()
                    );
                    return ExitCode::FAILURE;
                }
            };
            service
                .admit_document(&document, deadline)
                .map_err(|error| error.to_string())
        }
        None => {
            let source_run: Option<gauntlet_domain::RunId> = match from_run.as_deref() {
                Some(raw) => match gauntlet_domain::RunId::new(raw) {
                    Ok(run_id) => Some(run_id),
                    Err(_) => {
                        eprintln!(
                            "gauntlet improve propose: {raw} is not a valid run id (ULID grammar)"
                        );
                        return ExitCode::FAILURE;
                    }
                },
                None => None,
            };
            dispatch_proposal(&context, source_run, &service, deadline)
                .map_err(|error| error.to_string())
        }
    };
    match admission {
        Ok(admission) => {
            println!(
                "improve: revision={} admitted (idempotent={})",
                admission.revision, admission.idempotent
            );
            println!(
                "improve: the revision is PENDING — evaluate it in isolation before any \
                 promotion (spec 19.4)"
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("gauntlet improve propose: {error}");
            ExitCode::FAILURE
        }
    }
}

/// Dispatches the optimizer role invocation through the configured runner
/// with an allowlisted envelope: the optimizer role's own body plus the
/// operator-named run's durable projection (`--from-run`), never anything
/// from the role's `context.deny`.
fn dispatch_proposal(
    context: &crate::context::RuntimeContext,
    source_run: Option<gauntlet_domain::RunId>,
    service: &ProposalService,
    deadline: gauntlet_ports::deadline::Deadline,
) -> Result<gauntlet_app::promote::proposal::Admission, String> {
    let token = crate::compose::select_runner_token(context).map_err(|error| error.to_string())?;
    let clock: Arc<dyn gauntlet_ports::clock::Clock> =
        match gauntlet_adapter_agent_mock::clock_from_env()
            .map_err(|error| format!("GAUNTLET_CLOCK: {error}"))?
        {
            Some(deterministic) => Arc::new(deterministic),
            None => Arc::new(gauntlet_adapter_agent_prime::SystemClock::new()),
        };
    let runner =
        crate::compose::runner_for(&token, context, &clock).map_err(|error| error.to_string())?;
    // The allowlisted envelope: the role body from the roles dir, plus the
    // projected run history when the operator named a run.
    let roles_path = context.roles_dir().join("optimizer.md");
    let role_body = std::fs::read_to_string(&roles_path)
        .map_err(|error| format!("reading {}: {error}", roles_path.display()))?;
    let projection = match source_run.as_ref() {
        Some(run_id) => {
            let path = context.state_dir().join(crate::compose::STATE_DB_NAME);
            let store = gauntlet_adapter_state_sqlite::EmbeddedStateStore::open(&path)
                .map_err(|error| format!("opening the state store: {error}"))?;
            let store: Arc<dyn gauntlet_ports::state_store::StateStore> = Arc::new(store);
            match store.load_run(run_id, deadline) {
                Ok(Some(run)) => format!(
                    "run {}: intent={:?} terminal={:?} reference={}",
                    run_id,
                    run.intent(),
                    run.terminal_state(),
                    run.reference_version().digest()
                ),
                Ok(None) => format!("run {run_id}: no durable record"),
                Err(error) => format!("run {run_id}: the projection refused: {error}"),
            }
        }
        None => "no specific run was projected (--from-run names one)".to_owned(),
    };
    let body = format!("{role_body}\n\n[run-history projection]\n{projection}\n");
    let digest = gauntlet_app::verify::content_digest(body.as_bytes());
    let envelope = gauntlet_ports::agent_runner::ContextEnvelope::new(body, digest);
    #[allow(clippy::arc_with_non_send_sync)]
    // single-threaded dispatch; the seeded generator carries interior mutability (the composition's documented posture)
    let ids: Arc<dyn gauntlet_ports::id_generator::IdGenerator> =
        match gauntlet_adapter_agent_mock::id_generator_from_env() {
            Some(seeded) => Arc::new(seeded),
            None => Arc::new(
                gauntlet_adapter_id_entropy::UlidIdGenerator::new()
                    .map_err(|error| format!("the id generator refused: {error}"))?,
            ),
        };
    let budget = gauntlet_domain::Budget::new(
        context
            .resolved
            .budget
            .limits()
            .map_err(|error| format!("the budget is unusable: {error}"))?,
        context.resolved.budget.recovery_reserve_pct,
        context.resolved.budget.diagnostic_allowance(),
    )
    .map_err(|error| format!("the budget is unusable: {error}"))?;
    let plan = gauntlet_app::promote::proposal::ProposePlan {
        run_id: source_run.clone().unwrap_or_else(|| ids.run_id()),
        envelope,
        workspace: gauntlet_domain::WorkspaceRef::new("optimizer")
            .map_err(|error| format!("the optimizer workspace ref refused: {error}"))?,
        permissions: gauntlet_domain::PermissionSetRef::new("perm-optimizer")
            .map_err(|error| format!("the optimizer permission ref refused: {error}"))?,
        budget_slice: budget,
        ids,
        model_hint: None,
        sandbox_policy: None,
    };
    service
        .propose(runner.as_ref(), plan, deadline)
        .map_err(|error| error.to_string())
}

// ---------------------------------------------------------------------------
// evaluate
// ---------------------------------------------------------------------------

fn evaluate(args: &[String]) -> ExitCode {
    let mut config: Option<std::path::PathBuf> = None;
    let mut revision: Option<String> = None;
    let mut held_out: Option<std::path::PathBuf> = None;
    let mut regression: Option<std::path::PathBuf> = None;
    let mut protected: Option<String> = None;
    let mut metric: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--config" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("gauntlet improve evaluate: --config requires a path");
                    return ExitCode::from(2);
                };
                config = Some(std::path::PathBuf::from(value));
                index += 2;
            }
            "--held-out" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("gauntlet improve evaluate: --held-out requires a dir");
                    return ExitCode::from(2);
                };
                held_out = Some(std::path::PathBuf::from(value));
                index += 2;
            }
            "--regression" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("gauntlet improve evaluate: --regression requires a dir");
                    return ExitCode::from(2);
                };
                regression = Some(std::path::PathBuf::from(value));
                index += 2;
            }
            "--protected" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("gauntlet improve evaluate: --protected requires capability ids");
                    return ExitCode::from(2);
                };
                protected = Some(value.clone());
                index += 2;
            }
            "--metric" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("gauntlet improve evaluate: --metric requires a token");
                    return ExitCode::from(2);
                };
                metric = Some(value.clone());
                index += 2;
            }
            other if other.starts_with('-') => {
                eprintln!("gauntlet improve evaluate: unknown argument '{other}'");
                usage();
                return ExitCode::from(2);
            }
            other => {
                if revision.replace(other.to_owned()).is_some() {
                    eprintln!("gauntlet improve evaluate: exactly one revision id is required");
                    return ExitCode::from(2);
                }
                index += 1;
            }
        }
    }
    let Some(revision) = revision else {
        eprintln!("gauntlet improve evaluate: a revision id is required");
        usage();
        return ExitCode::from(2);
    };
    let (Some(held_out), Some(regression)) = (held_out, regression) else {
        eprintln!(
            "gauntlet improve evaluate: both --held-out and --regression task-set dirs are \
             required (the operator names the sets; the harness never invents them)"
        );
        usage();
        return ExitCode::from(2);
    };
    let Ok(revision) = gauntlet_domain::RevisionId::new(&revision) else {
        eprintln!("gauntlet improve evaluate: {revision} is not a valid revision id");
        return ExitCode::FAILURE;
    };
    let metric = match metric.as_deref() {
        Some(token) => match TargetMetric::parse(token) {
            Ok(metric) => metric,
            Err(error) => {
                eprintln!("gauntlet improve evaluate: {error}");
                return ExitCode::from(2);
            }
        },
        None => TargetMetric::IterationsPerSettlement,
    };
    let protected = match protected.as_deref() {
        Some(list) => {
            let mut caps = Vec::new();
            for token in list.split(',') {
                match gauntlet_domain::CapabilityId::new(token.trim()) {
                    Ok(capability) => caps.push(capability),
                    Err(_) => {
                        eprintln!(
                            "gauntlet improve evaluate: {token:?} is not a valid capability id"
                        );
                        return ExitCode::FAILURE;
                    }
                }
            }
            caps
        }
        None => Vec::new(),
    };
    let context = match crate::context::resolve_context(config.as_deref()) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("gauntlet improve evaluate: the configuration did not resolve: {error}");
            return ExitCode::FAILURE;
        }
    };
    // NO outer writer lock here, deliberately: the four evaluation runs
    // each take the single-writer admission for their own composition
    // (COR-R3-3), and the operator-named task sets may resolve to THIS
    // project's state dir — an outer lock would refuse the loop's own
    // runs. The harness's only direct write is the final ledger append,
    // after the runs' locks have released.
    let ledger = match revision_ledger(&context) {
        Ok(ledger) => ledger,
        Err(error) => {
            eprintln!("gauntlet improve evaluate: {error}");
            return ExitCode::FAILURE;
        }
    };
    let capability_ledger = match capability_ledger(&context) {
        Ok(ledger) => ledger,
        Err(error) => {
            eprintln!("gauntlet improve evaluate: {error}");
            return ExitCode::FAILURE;
        }
    };
    let evidence = match evidence_store(&context) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("gauntlet improve evaluate: {error}");
            return ExitCode::FAILURE;
        }
    };
    let deadline = improve_bound();
    // The candidate delta resolves from the evidence store at the
    // payload's candidate_change ref (content-addressed: the bytes ARE the
    // ref's content).
    let delta = match resolve_delta(&ledger, &evidence, &revision, deadline) {
        Ok(delta) => delta,
        Err(error) => {
            eprintln!("gauntlet improve evaluate: {error}");
            return ExitCode::FAILURE;
        }
    };
    let harness = EvaluationHarness::new(ledger, capability_ledger, Arc::new(SystemClock));
    // COR-2 (R1): the harness adjudicates against the promoting project's
    // VERIFIED frozen reference — an evaluation arm whose own project
    // (the operator-named regression set freezes its own reference)
    // resolved a DIFFERENT frozen digest is refused at evaluate time,
    // before anything is recorded.
    let lock = match crate::reference_lock::verify_lock(&context.project_root) {
        Ok(lock) => lock,
        Err(error) => {
            eprintln!("gauntlet improve evaluate: the reference lock does not verify: {error}");
            return ExitCode::FAILURE;
        }
    };
    let current_reference_digest = match gauntlet_domain::Digest::parse(&lock.answer_key_digest) {
        Ok(digest) => digest,
        Err(error) => {
            eprintln!("gauntlet improve evaluate: the lock digest does not parse: {error}");
            return ExitCode::FAILURE;
        }
    };
    let request = EvaluationRequest {
        revision: revision.clone(),
        metric,
        current_reference_digest,
        delta,
        held_out_root: held_out,
        regression_root: regression,
        protected,
        intent: "candidate-revision isolated evaluation".to_owned(),
    };
    let executor = ComposedExecutor;
    match harness.evaluate(&request, &executor, deadline) {
        Ok(summary) => {
            let record = &summary.record;
            println!(
                "improve: evaluation of {revision} recorded (idempotent={})",
                summary.idempotent
            );
            println!(
                "improve: held-out control={} treatment={} (parent_run set)",
                record.held_out_control, record.held_out_treatment
            );
            println!(
                "improve: regression control={} treatment={} (parent_run set)",
                record.regression_control, record.regression_treatment
            );
            println!("improve: imported {} memory row(s)", summary.imported);
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("gauntlet improve evaluate: {error}");
            ExitCode::FAILURE
        }
    }
}

fn resolve_delta(
    ledger: &Arc<EmbeddedRevisionLedger>,
    evidence: &Arc<FsEvidenceStore>,
    revision: &gauntlet_domain::RevisionId,
    deadline: gauntlet_ports::deadline::Deadline,
) -> Result<DeltaDocument, String> {
    let proposal = ledger
        .proposal(revision, deadline)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| format!("revision {revision} has no admitted proposal"))?;
    let bytes = evidence
        .get(proposal.diff_ref(), deadline)
        .map_err(|error| error.to_string())?
        .ok_or_else(|| {
            format!(
                "the candidate change {} is not in the evidence store — the optimizer's \
                 candidate-change bytes must be pinned before evaluation",
                proposal.diff_ref()
            )
        })?;
    let text = String::from_utf8(bytes)
        .map_err(|_| "the candidate change is not UTF-8 text".to_owned())?;
    DeltaDocument::parse(&text).map_err(|error| error.to_string())
}

/// The executor seam, implemented at the composition root over the REAL
/// composed loop (charter D4): resolve the task-set project's context with
/// the candidate delta applied as configuration overrides, compose the
/// loop, run it to its terminal state, and hand the harness the run's OWN
/// durable store handle.
struct ComposedExecutor;

impl EvaluationExecutor for ComposedExecutor {
    /// COR-1 (critic C7): the composition declares which delta entries it
    /// can provably apply BEFORE any arm runs — an entry with no Annex E
    /// override variable cannot reach the composed configuration, so a
    /// delta carrying one is refused in full (naming EVERY such entry),
    /// never partially applied.
    fn validate_delta(
        &self,
        delta: &gauntlet_app::promote::delta::DeltaDocument,
    ) -> Result<(), gauntlet_app::promote::evaluate::ExecutionError> {
        let unappliable: Vec<String> = delta
            .overrides()
            .iter()
            .filter(|(key, _)| {
                config_resolve::ConfigKey::from_path(key)
                    .and_then(|config_key| config_key.env())
                    .is_none()
            })
            .map(|(key, _)| key.clone())
            .collect();
        if unappliable.is_empty() {
            Ok(())
        } else {
            Err(gauntlet_app::promote::evaluate::ExecutionError::Failed {
                detail: format!(
                    "delta entries {unappliable:?} name no Annex E override variable — this \
                     composition cannot provably apply them, so nothing is evaluated",
                ),
            })
        }
    }

    fn execute(
        &self,
        request: &gauntlet_app::promote::evaluate::ExecutionRequest,
        deadline: gauntlet_ports::deadline::Deadline,
    ) -> Result<ExecutionOutcome, gauntlet_app::promote::evaluate::ExecutionError> {
        let project_root = request.project_root.clone();
        // The candidate delta applies as configuration overrides (the
        // delta's §8.2 keys translate to their Annex E variables).
        let mut host: Vec<(String, String)> = std::env::vars_os()
            .map(|(key, value)| {
                (
                    key.to_string_lossy().into_owned(),
                    value.to_string_lossy().into_owned(),
                )
            })
            .collect();
        if let Some(delta) = &request.delta {
            for (key, value) in delta.overrides() {
                let variable = config_resolve::ConfigKey::from_path(&key)
                    .and_then(|config_key| config_key.env())
                    .ok_or_else(|| gauntlet_app::promote::evaluate::ExecutionError::Failed {
                        detail: format!("delta key {key:?} names no Annex E override variable"),
                    })?;
                host.push((variable.to_owned(), value));
            }
        }
        let env = EnvLayer::from_parts("", &host, crate::spec_vocab::ANNEX_E_ENV);
        let config_path = config_resolve::discover_config_path(&project_root, None, &env);
        let front_matter = match std::fs::read_to_string(&config_path) {
            Ok(text) => match gauntlet_app::config::gauntlet_md::parse(&text) {
                Ok(parsed) => Some(parsed),
                Err(error) => {
                    return Err(gauntlet_app::promote::evaluate::ExecutionError::Failed {
                        detail: format!("parsing {}: {error}", config_path.display()),
                    });
                }
            },
            Err(err) => {
                return Err(gauntlet_app::promote::evaluate::ExecutionError::Failed {
                    detail: format!(
                        "reading the task-set config {}: {err}",
                        config_path.display()
                    ),
                });
            }
        };
        let resolved = config_resolve::resolve(&config_resolve::ResolveInput {
            project_root: project_root.clone(),
            front_matter,
            env: &env,
            flags: config_resolve::FlagOverrides::default(),
        })
        .map_err(
            |error| gauntlet_app::promote::evaluate::ExecutionError::Failed {
                detail: error.to_string(),
            },
        )?;
        let anchored = resolved
            .effective
            .resolve_against(&env, &project_root)
            .map_err(
                |error| gauntlet_app::promote::evaluate::ExecutionError::Failed {
                    detail: error.to_string(),
                },
            )?;
        let context = crate::context::RuntimeContext {
            project_root: project_root.clone(),
            env,
            effective: resolved.effective,
            resolved: anchored,
        };
        // The single-writer admission for the evaluation run's OWN state
        // dir (the operator's main state lock is held by the evaluate
        // command itself).
        let _writer_lock =
            crate::writer_lock::WriterLock::acquire(context.state_dir()).map_err(|error| {
                gauntlet_app::promote::evaluate::ExecutionError::Failed {
                    detail: error.report("the evaluation run"),
                }
            })?;
        let composed = crate::compose::compose_run(&context, &request.intent).map_err(|error| {
            gauntlet_app::promote::evaluate::ExecutionError::Failed {
                detail: error.to_string(),
            }
        })?;
        let outcome = match &request.parent_run {
            Some(parent) => composed
                .controller
                .start_run_with_parent(parent)
                .map_err(
                    |error| gauntlet_app::promote::evaluate::ExecutionError::Failed {
                        detail: error.to_string(),
                    },
                )?,
            None => composed.controller.start_run().map_err(|error| {
                gauntlet_app::promote::evaluate::ExecutionError::Failed {
                    detail: error.to_string(),
                }
            })?,
        };
        let run_record = composed
            .state
            .load_run(&outcome.run_id, deadline)
            .map_err(
                |error| gauntlet_app::promote::evaluate::ExecutionError::Failed {
                    detail: error.to_string(),
                },
            )?
            .ok_or_else(|| gauntlet_app::promote::evaluate::ExecutionError::Failed {
                detail: format!("run {} left no durable record", outcome.run_id),
            })?;
        Ok(ExecutionOutcome {
            run_id: outcome.run_id,
            terminal: outcome.terminal,
            reference_digest: run_record.reference_version().digest().clone(),
            state: composed.state,
        })
    }
}

// ---------------------------------------------------------------------------
// promote
// ---------------------------------------------------------------------------

fn promote(args: &[String]) -> ExitCode {
    let mut config: Option<std::path::PathBuf> = None;
    let mut revision: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--config" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("gauntlet improve promote: --config requires a path");
                    return ExitCode::from(2);
                };
                config = Some(std::path::PathBuf::from(value));
                index += 2;
            }
            other if other.starts_with('-') => {
                eprintln!("gauntlet improve promote: unknown argument '{other}'");
                usage();
                return ExitCode::from(2);
            }
            other => {
                if revision.replace(other.to_owned()).is_some() {
                    eprintln!("gauntlet improve promote: exactly one revision id is required");
                    return ExitCode::from(2);
                }
                index += 1;
            }
        }
    }
    let Some(revision) = revision else {
        eprintln!("gauntlet improve promote: a revision id is required");
        usage();
        return ExitCode::from(2);
    };
    let Ok(revision) = gauntlet_domain::RevisionId::new(&revision) else {
        eprintln!("gauntlet improve promote: {revision} is not a valid revision id");
        return ExitCode::FAILURE;
    };
    let context = match crate::context::resolve_context(config.as_deref()) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("gauntlet improve promote: the configuration did not resolve: {error}");
            return ExitCode::FAILURE;
        }
    };
    let _writer_lock = match crate::writer_lock::WriterLock::acquire(context.state_dir()) {
        Ok(lock) => lock,
        Err(error) => {
            return crate::writer_lock::report_failure("gauntlet improve promote", &error);
        }
    };
    let ledger = match revision_ledger(&context) {
        Ok(ledger) => ledger,
        Err(error) => {
            eprintln!("gauntlet improve promote: {error}");
            return ExitCode::FAILURE;
        }
    };
    let capability_ledger = match capability_ledger(&context) {
        Ok(ledger) => ledger,
        Err(error) => {
            eprintln!("gauntlet improve promote: {error}");
            return ExitCode::FAILURE;
        }
    };
    let evidence = match evidence_store(&context) {
        Ok(store) => store,
        Err(error) => {
            eprintln!("gauntlet improve promote: {error}");
            return ExitCode::FAILURE;
        }
    };
    let deadline = improve_bound();
    let delta = match resolve_delta(&ledger, &evidence, &revision, deadline) {
        Ok(delta) => delta,
        Err(error) => {
            eprintln!("gauntlet improve promote: {error}");
            return ExitCode::FAILURE;
        }
    };
    // Condition 4's resolution: the rollback ref is checked where the
    // evidence store is at hand. COR-1 (critic M3): a store READ FAILURE
    // is uncertainty, never a pass-deciding "None" — the command aborts
    // typed BEFORE any adjudication and nothing is written. Only a
    // successful read drives the condition (`Ok(true)` holds; `Ok(false)`
    // fails with `RollbackInvalid`).
    let rollback_resolvable = match ledger.proposal(&revision, deadline) {
        Ok(Some(proposal)) => match evidence.get(proposal.rollback_ref(), deadline) {
            Ok(found) => Some(found.is_some()),
            Err(error) => {
                eprintln!(
                    "gauntlet improve promote: the rollback ref {} could not be checked in \
                     the evidence store: {error} — condition 4's evidence must be readable \
                     before any adjudication; nothing was written",
                    proposal.rollback_ref()
                );
                return ExitCode::FAILURE;
            }
        },
        Ok(None) => None,
        Err(error) => {
            eprintln!("gauntlet improve promote: {error}");
            return ExitCode::FAILURE;
        }
    };
    // Condition 2's ruler: the CURRENT frozen reference digest, from the
    // verified lock.
    let lock = match crate::reference_lock::verify_lock(&context.project_root) {
        Ok(lock) => lock,
        Err(error) => {
            eprintln!("gauntlet improve promote: the reference lock does not verify: {error}");
            return ExitCode::FAILURE;
        }
    };
    let current_reference_digest = match gauntlet_domain::Digest::parse(&lock.answer_key_digest) {
        Ok(digest) => digest,
        Err(error) => {
            eprintln!("gauntlet improve promote: the lock digest does not parse: {error}");
            return ExitCode::FAILURE;
        }
    };
    let gate = match PromotionGate::new(
        ledger,
        capability_ledger,
        Arc::new(SystemClock),
        context.resolved.promotion_margin_bps,
    ) {
        Ok(gate) => gate,
        // COR-2 (R3): resolution refuses a margin above the 10 000 bps
        // ceiling already, so this is the second line of defense — the
        // constructor is a public API and refuses the same bar typed.
        Err(error) => {
            eprintln!("gauntlet improve promote: {error}");
            return ExitCode::FAILURE;
        }
    };
    let input = GateInput {
        current_reference_digest,
        delta,
        rollback_resolvable,
    };
    match gate.adjudicate(&revision, &input, deadline) {
        Ok(verdict) => {
            // Fail closed: only an honest promotion exits 0 — a rejection
            // or a provisional is a non-promotion, and an operator script
            // branching on the exit code must never deploy on one.
            let exit = match &verdict {
                GateVerdict::Promoted(_) => ExitCode::SUCCESS,
                _ => ExitCode::FAILURE,
            };
            match verdict {
                GateVerdict::Promoted(evidence) => {
                    println!("improve: revision {revision} PROMOTED");
                    println!(
                        "improve: metric={} control={} treatment={} margin={}bps",
                        evidence.metric,
                        evidence.control_figure,
                        evidence.treatment_figure,
                        evidence.margin_bps
                    );
                }
                GateVerdict::Rejected {
                    reasons,
                    rollback_ref,
                } => {
                    println!("improve: revision {revision} REJECTED");
                    for reason in &reasons {
                        println!("improve: refused — {reason}");
                    }
                    println!("improve: rollback ref {rollback_ref} stands; the loop is unchanged");
                }
                GateVerdict::Provisional { reasons, written } => {
                    println!(
                        "improve: revision {revision} PROVISIONAL (written={written}) — \
                         insufficient evidence is never a promotion (spec 19.4)"
                    );
                    for reason in &reasons {
                        println!("improve: gap — {reason}");
                    }
                }
            }
            exit
        }
        Err(error) => {
            eprintln!("gauntlet improve promote: {error}");
            ExitCode::FAILURE
        }
    }
}

// ---------------------------------------------------------------------------
// rollback
// ---------------------------------------------------------------------------

fn rollback(args: &[String]) -> ExitCode {
    let mut config: Option<std::path::PathBuf> = None;
    let mut revision: Option<String> = None;
    let mut index = 0;
    while index < args.len() {
        match args[index].as_str() {
            "--config" => {
                let Some(value) = args.get(index + 1) else {
                    eprintln!("gauntlet improve rollback: --config requires a path");
                    return ExitCode::from(2);
                };
                config = Some(std::path::PathBuf::from(value));
                index += 2;
            }
            other if other.starts_with('-') => {
                eprintln!("gauntlet improve rollback: unknown argument '{other}'");
                usage();
                return ExitCode::from(2);
            }
            other => {
                if revision.replace(other.to_owned()).is_some() {
                    eprintln!("gauntlet improve rollback: exactly one revision id is required");
                    return ExitCode::from(2);
                }
                index += 1;
            }
        }
    }
    let Some(revision) = revision else {
        eprintln!("gauntlet improve rollback: a revision id is required");
        usage();
        return ExitCode::from(2);
    };
    let Ok(revision) = gauntlet_domain::RevisionId::new(&revision) else {
        eprintln!("gauntlet improve rollback: {revision} is not a valid revision id");
        return ExitCode::FAILURE;
    };
    let context = match crate::context::resolve_context(config.as_deref()) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("gauntlet improve rollback: the configuration did not resolve: {error}");
            return ExitCode::FAILURE;
        }
    };
    let _writer_lock = match crate::writer_lock::WriterLock::acquire(context.state_dir()) {
        Ok(lock) => lock,
        Err(error) => {
            return crate::writer_lock::report_failure("gauntlet improve rollback", &error);
        }
    };
    let ledger = match revision_ledger(&context) {
        Ok(ledger) => ledger,
        Err(error) => {
            eprintln!("gauntlet improve rollback: {error}");
            return ExitCode::FAILURE;
        }
    };
    let service = RollbackService::new(ledger, Arc::new(SystemClock));
    match service.restore(
        &revision,
        "the operator rolled the candidate back".to_owned(),
        improve_bound(),
    ) {
        Ok(entry) => {
            println!(
                "improve: revision {revision} restored (promoted-but-rolled-back, decision={:?})",
                entry.decision
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!("gauntlet improve rollback: {error}");
            ExitCode::FAILURE
        }
    }
}

/// The wall clock, under the composition root's own name (the adapters'
/// system clock is the composition's to spell).
struct SystemClock;

impl gauntlet_ports::clock::Clock for SystemClock {
    fn now(&self) -> gauntlet_domain::EpochSeconds {
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|span| span.as_secs())
            .unwrap_or_default();
        gauntlet_domain::EpochSeconds::new(now_secs)
    }

    fn sleep_until(
        &self,
        _at: gauntlet_domain::EpochSeconds,
    ) -> Result<(), gauntlet_ports::clock::ClockError> {
        std::thread::sleep(std::time::Duration::from_millis(PROBE_REST_SPAN));
        Ok(())
    }
}

/// The dispatch's inter-probe rest quantum, on its own line (numeric
/// literals never share a line with a duration-shaped identifier).
const PROBE_REST_SPAN: u64 = 25;
