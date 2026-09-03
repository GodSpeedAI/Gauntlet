//! The ONE operator-action execution path (plan Task 21, charter D3/D4):
//! the resolve/retry body both the `escalations resolve|retry` commands
//! and the JSON surface's action endpoints run. The sequence is the
//! settlement service's OWN operator gate, then the governed
//! verification chain's re-mint (the `verify` command's composition
//! pattern — charter D4), then `SettlementService::settle_operator_action_earned`
//! — the SAME settle the automatic path uses, with §16.1 conditions 1–5
//! re-derived inside and the §16.3 commit performed by the service — and
//! ONLY an accepted action resolves the escalation ticket(s) afterwards.
//!
//! No second write path exists by construction: every write below goes
//! through the settlement service (or, after acceptance, the queue's own
//! recorded operator action). A refusal leaves the queue untouched.
//!
//! Every action — accepted AND refused — logs a §20.3 key=value line to
//! the command's stderr (charter D7): action, run_id, unit, operator,
//! outcome, code, tickets. Secrets never appear (the operator identity
//! and note are operator-provided facts, the note is NOT logged — it
//! travels into the ticket record only).

use std::sync::Arc;

use gauntlet_domain::{RunId, UnitId};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::verifier::VerificationRequest;

use crate::compose::{self, open_operator_surface};
use crate::context::RuntimeContext;

/// COR-3 (falsifier round 3, R3-F-1/2/3): the ONE log-sink escaping
/// discipline for the escalations/operator-action command family. Every
/// dynamic untrusted string (argv remnants, ticket ids, operator
/// identities, queue-file stems, error details) reaches a stderr/stdout
/// line through this helper: hostile bytes land in their Rust Debug
/// escapes — a newline can no longer forge a following line, a control
/// byte can no longer ride raw — while already-clean text passes through
/// BYTE-IDENTICAL, keeping every dynamic field greppable and
/// human-readable (the falsifier-confirmed property). Fields already
/// Debug-quoted at their sink (`{:?}`: the operator identity, the list
/// summary, the failed-arm detail, the pre-parse refusals) are the same
/// discipline at those sites and stay as they are.
pub(crate) fn log_quote(value: &str) -> String {
    value.escape_debug().collect()
}

/// An accepted action: the service settled, and (only then) the queue
/// ticket(s) resolved.
#[derive(Debug, Clone, PartialEq)]
pub struct Acceptance {
    pub run_id: RunId,
    pub unit: UnitId,
    /// The tickets resolved AFTER the service accepted (named, never
    /// silent). An action addressed by ticket resolves that ticket; an
    /// API action resolves the run's one open ticket.
    pub tickets_resolved: Vec<String>,
    /// Whether THIS call performed the durable §16.3 commit (`false` =
    /// idempotent re-entry over an already-durable settlement key).
    pub committed_now: bool,
}

/// A typed refusal: nothing was written; the ticket stays open. `code`
/// is a stable token from the settlement failure vocabulary; `message`
/// is the typed refusal's own Display — the SAME text the CLI surfaces,
/// and the text the JSON surface carries verbatim (refusal parity).
#[derive(Debug, Clone, PartialEq)]
pub struct Refusal {
    pub code: String,
    pub message: String,
}

/// The §20.3 log context (charter D7's field parity): the run and the
/// tickets the action addressed, carried out of the decision body so
/// EVERY outcome's log line names them. Empty until the durable reads
/// resolve them — the log never invents a run or a ticket.
#[derive(Debug, Clone, Default)]
struct ActionLogContext {
    run_id: Option<RunId>,
    tickets: Vec<String>,
}

/// The decision the shared path reached.
#[derive(Debug, Clone, PartialEq)]
pub enum ActionDecision {
    Accepted(Acceptance),
    Refused(Refusal),
}

/// Executes one operator action (charter D3): `ticket` is the explicit
/// ticket id the CLI carries (`None` for the API surface, which resolves
/// the run's single open ticket); `run_override` is the API's `?run=`
/// anchor (`None` derives the run from the open tickets — D6's disclosed
/// derivation). `Err` is an infrastructure/formation failure (the config,
/// a store, the queue read, an unusable id or identity) — distinct from a
/// refusal, nothing is claimed either way.
#[allow(clippy::too_many_arguments)] // the operator request's own vocabulary
#[allow(clippy::arc_with_non_send_sync)] // single-threaded surface; the service stores the same Arcs
pub fn execute_operator_action(
    context: &RuntimeContext,
    action: gauntlet_adapter_escalation_fs::OperatorAction,
    ticket: Option<&str>,
    unit: &UnitId,
    operator: &gauntlet_app::settle::OperatorIdentity,
    note: &str,
    run_override: Option<RunId>,
    deadline: Deadline,
) -> Result<ActionDecision, String> {
    let action_word = action.token();
    let mut log = ActionLogContext::default();
    let decision = decide_operator_action(
        context,
        action,
        ticket,
        unit,
        operator,
        note,
        run_override,
        deadline,
        &mut log,
    );
    // The §20.3 action log (charter D7): EVERY action — accepted AND
    // refused AND failed — logs the SAME key=value field set to the
    // command's stderr. The run and the tickets ride on the log context
    // so the refused and failed arms name them exactly like the accepted
    // arm (empty until the durable reads resolve them — never invented).
    // The note is NOT logged (it travels into the ticket record only).
    let run_field = log
        .run_id
        .as_ref()
        .map(|run| run.to_string())
        .unwrap_or_default();
    let tickets_field = log.tickets.join(",");
    match &decision {
        Ok(ActionDecision::Accepted(acceptance)) => eprintln!(
            "operator-action: action={action_word} run_id={} unit={} operator={:?} \
             outcome=accepted tickets={}",
            acceptance.run_id,
            unit,
            operator.as_str(),
            // COR-3 (falsifier round 3, R3-F-1): the ids are grammar-valid
            // but control bytes ride the grammar (the planted ESC id) —
            // the accepted line carried them raw. The family's ONE
            // escaping discipline; clean ids byte-identical.
            log_quote(&acceptance.tickets_resolved.join(","))
        ),
        Ok(ActionDecision::Refused(refusal)) => eprintln!(
            "operator-action: action={action_word} run_id={run_field} unit={} operator={:?} \
             outcome=refused tickets={} code={} — the ticket stays open",
            unit,
            operator.as_str(),
            log_quote(&tickets_field),
            refusal.code
        ),
        // COR-2 (falsifier round 2, R2-F-1): `detail` embeds queue/store
        // error Displays that can carry attacker-shaped strings (a raw ESC
        // ticket id was proven). The LOG sink takes it DEBUG-ESCAPED, like
        // the operator identity beside it; the typed `Err` return — the
        // CLI's human-readable failure line and the JSON surface's 500
        // body — stays the verbatim detail (recorded decision: escape in
        // the log, keep the non-log path unchanged).
        Err(detail) => eprintln!(
            "operator-action: action={action_word} run_id={run_field} unit={} operator={:?} \
             outcome=failed tickets={} code=action_failed — {detail:?}",
            unit,
            operator.as_str(),
            // COR-3: the same grammar-rides-control-bytes class as the
            // accepted arm above — the family's ONE discipline.
            log_quote(&tickets_field)
        ),
    }
    decision
}

#[allow(clippy::too_many_arguments)] // the operator request's own vocabulary
#[allow(clippy::arc_with_non_send_sync)] // single-threaded surface; the service stores the same Arcs
fn decide_operator_action(
    context: &RuntimeContext,
    action: gauntlet_adapter_escalation_fs::OperatorAction,
    ticket: Option<&str>,
    unit: &UnitId,
    operator: &gauntlet_app::settle::OperatorIdentity,
    note: &str,
    run_override: Option<RunId>,
    deadline: Deadline,
    log: &mut ActionLogContext,
) -> Result<ActionDecision, String> {
    let surface = open_operator_surface(context).map_err(|error| error.to_string())?;
    let queue: Arc<gauntlet_adapter_escalation_fs::FsEscalationQueue> = Arc::new(
        gauntlet_adapter_escalation_fs::FsEscalationQueue::open(&surface.escalation_root)
            .map_err(|error| error.to_string())?,
    );

    // The run + tickets the action addresses: the CLI names its ticket
    // (the record's own run anchors the action); the API anchors at the
    // explicit `?run=` or the D6 derivation over the open tickets. Both
    // land in the log context the moment the durable reads name them.
    let (run_id, tickets) = match ticket {
        Some(explicit) => {
            let ticket_id = gauntlet_ports::human_escalation::TicketId::new(explicit)
                .map_err(|error| format!("{explicit} is not a usable ticket id: {error}"))?;
            let record = queue
                .record(&ticket_id)
                .map_err(|error| error.to_string())?;
            log.run_id = Some(record.run_id.clone());
            log.tickets = vec![ticket_id.to_string()];
            (record.run_id.clone(), vec![ticket_id])
        }
        None => {
            let run = match run_override {
                Some(ref explicit) => explicit.clone(),
                None => derived_run(&queue)?,
            };
            log.run_id = Some(run.clone());
            let listing = queue.list().map_err(|error| error.to_string())?;
            let open: Vec<_> = listing
                .records
                .iter()
                .filter(|record| record.is_open() && record.run_id == run)
                .map(|record| record.ticket.clone())
                .collect();
            match open.len() {
                1 => {
                    log.tickets = vec![open[0].to_string()];
                    (run, vec![open[0].clone()])
                }
                // An explicitly-anchored action proceeds even with no
                // open ticket: the settle is the action's effect; the
                // (empty, disclosed) ticket list records that there was
                // nothing to mark. Ambiguity is never resolved silently.
                0 if run_override.is_some() => (run, Vec::new()),
                0 => {
                    return Ok(refused(
                        "no_open_ticket",
                        &format!(
                            "run {run} holds no open escalation ticket — the operator action \
                             has nothing to resolve; anchor the run explicitly to settle \
                             without ticket bookkeeping"
                        ),
                    ));
                }
                count => {
                    return Ok(refused(
                        "ambiguous_ticket",
                        &format!(
                            "run {run} holds {count} open escalation tickets — the API action \
                             cannot choose one; resolve through the CLI with --ticket (never \
                             a silent pick)"
                        ),
                    ));
                }
            }
        }
    };

    // The SAME settlement service construction the run composition uses
    // (§16.3's single write path). Two handles into the SAME stores: the
    // service settles; the queue handle — used only AFTER the service
    // accepted — records the operator action on the ticket.
    let evidence = Arc::new(
        gauntlet_adapter_evidence_fs::FsEvidenceStore::open(&context.resolved.evidence_dir)
            .map_err(|error| error.to_string())?,
    );
    let evidence_for_provider =
        gauntlet_adapter_evidence_fs::FsEvidenceStore::open(&context.resolved.evidence_dir)
            .map_err(|error| error.to_string())?;
    let reference = gauntlet_app::reference::provider::FileReferenceProvider::from_paths(
        surface.answer_key.clone(),
        surface.manifest.clone(),
        std::rc::Rc::new(evidence_for_provider),
    );
    let clock: Arc<dyn gauntlet_ports::clock::Clock> =
        Arc::new(gauntlet_adapter_agent_prime::SystemClock::new());
    let service = gauntlet_app::settle::SettlementService::new(
        surface.state.clone(),
        evidence.clone(),
        Arc::new(reference),
        queue.clone(),
        clock,
    );
    let identity = Some(operator.clone());
    let request = gauntlet_app::settle::OperatorActionRequest {
        run_id: run_id.clone(),
        unit_id: unit.clone(),
        identity,
    };

    // 1. The gate speaks FIRST (charter D4's ordering): no durable PASS
    //    record, no recorded identity — refused before any chain work is
    //    trusted. The SAME typed error the automatic path's admission
    //    produces.
    if let Err(error) = service.settle_operator_action_gate(&request, deadline) {
        return Ok(refused(refusal_code(&error), &error.to_string()));
    }

    // 2. The §16.3 idempotent re-entry shape: the unit's settlement is
    //    ALREADY durable — the action reports the durable truth
    //    (committed_now=false, the service's own AlreadyCommitted
    //    semantics) instead of minting a SECOND document over the same
    //    (unit, evidence, outcome) identity, which the records-integrity
    //    refusal exists to stop. Nothing is written here; the still-open
    //    tickets resolve (disclosed), exactly as after a fresh accept.
    let rows = surface
        .state
        .settlements(&run_id, deadline)
        .map_err(|error| error.to_string())?;
    if rows.iter().any(|row| row.unit_id() == unit) {
        let mut resolved = Vec::new();
        for ticket_id in &tickets {
            if let Err(error) = queue.resolve(ticket_id, operator.as_str(), note, action) {
                // The re-entry branch performed NO settle on this call —
                // the settlement was already durable when the branch
                // found it. The failure names that durable truth, never
                // "the settle committed" (COR-1 C2; the fresh-accept
                // path's failure below keeps its commit wording).
                return Err(format!(
                    "the settlement is already durable; the queue write failed: {error}"
                ));
            }
            resolved.push(ticket_id.to_string());
        }
        return Ok(ActionDecision::Accepted(Acceptance {
            run_id,
            unit: unit.clone(),
            tickets_resolved: resolved,
            committed_now: false,
        }));
    }

    // 3. The re-mint THROUGH THE GOVERNED CHAIN (the verify_cmd pattern):
    //    the frozen unit's own evidence_required, the run's latest
    //    durable iteration, the composed §14.2 rung set. The chain's
    //    refusals are TYPED refusals of the action (nothing settles),
    //    never infrastructure failures.
    let claim = match mint_through_governed_chain(context, &surface, &run_id, unit, deadline) {
        Ok(claim) => claim,
        Err(MintFailure::Refused(refusal)) => return Ok(ActionDecision::Refused(refusal)),
        Err(MintFailure::Failed(detail)) => return Err(detail),
    };

    // 4. The earned arm: the gate runs again INSIDE the one method that
    //    pairs it with the settle (defense in depth, never a caller
    //    shortcut), then §16.1 conditions 1–5 re-derive and the §16.3
    //    commit runs — the SAME settle the automatic path uses.
    let outcome = match service.settle_operator_action_earned(request, &claim, deadline) {
        Ok(outcome) => outcome,
        Err(error) => return Ok(refused(refusal_code(&error), &error.to_string())),
    };

    // 5. The service accepted: NOW the queue records the operator action.
    //    Fail-closed on a queue write failure (non-success, the durable
    //    truth named) — the settle's commit is never re-run to mask it.
    let mut resolved = Vec::new();
    for ticket_id in &tickets {
        if let Err(error) = queue.resolve(ticket_id, operator.as_str(), note, action) {
            return Err(format!(
                "the settle committed but the queue write failed: {error}"
            ));
        }
        resolved.push(ticket_id.to_string());
    }

    Ok(ActionDecision::Accepted(Acceptance {
        run_id,
        unit: unit.clone(),
        tickets_resolved: resolved,
        committed_now: outcome.committed_now,
    }))
}

/// Why the governed re-mint could not hand back a claim: a REFUSED
/// action (the frozen ruler, the governed open, or the chain refused —
/// typed, nothing settles) or an infrastructure failure (a store read
/// failed — nothing is claimed either way).
enum MintFailure {
    Refused(Refusal),
    Failed(String),
}

/// The governed re-mint (charter D4, the `verify` command's composition
/// pattern): the frozen unit's own `evidence_required`, the run's latest
/// durable iteration, the composed §14.2 rung set — readiness minted by
/// the chain, never accepted from the operator's request.
#[allow(clippy::arc_with_non_send_sync)] // single-threaded surface; the authority stores the same Arcs
fn mint_through_governed_chain(
    context: &RuntimeContext,
    surface: &compose::OperatorSurface,
    run_id: &RunId,
    unit: &UnitId,
    deadline: Deadline,
) -> Result<gauntlet_app::verify::authority::VerifiedClaim, MintFailure> {
    // The FROZEN RULER's block for the unit: the requirement side of the
    // request. A unit outside the ruler is a typed refusal — the settling
    // unit is the frozen one (spec 16.1).
    let key_text = std::fs::read_to_string(&surface.answer_key).map_err(|error| {
        MintFailure::Failed(format!("reading {}: {error}", surface.answer_key.display()))
    })?;
    let parsed = gauntlet_app::reference::parse::parse_answer_key(&key_text);
    let frozen = parsed
        .units
        .iter()
        .find(|candidate| candidate.id() == unit)
        .ok_or_else(|| {
            MintFailure::Refused(refusal_of(
                "unit_not_frozen",
                &format!("unit {unit} is not in the frozen ruler — settlement refuses it"),
            ))
        })?;
    // The run's latest iteration anchors the governed open (§14.1); a run
    // with no checkpoint opens at iteration one.
    let iteration = surface
        .state
        .resume(run_id, deadline)
        .map_err(|error| MintFailure::Failed(error.to_string()))?
        .map_or_else(
            || gauntlet_domain::IterationNo::new(1).expect("iterations are 1-based"),
            |checkpoint| checkpoint.iteration_no,
        );
    let evidence = Arc::new(
        gauntlet_adapter_evidence_fs::FsEvidenceStore::open(&context.resolved.evidence_dir)
            .map_err(|error| MintFailure::Failed(error.to_string()))?,
    );
    let ids = gauntlet_adapter_id_entropy::UlidIdGenerator::new()
        .map_err(|error| MintFailure::Failed(error.to_string()))?;
    let ladder = compose::composed_ladder(&surface.workspace_root)
        .map_err(|error| MintFailure::Failed(error.to_string()))?;
    let authority = gauntlet_app::verify::authority::VerificationAuthority::new(
        surface.state.clone(),
        evidence,
        Arc::new(ids),
    );
    let governed = authority
        .for_run(
            run_id,
            iteration,
            ladder,
            surface.workspace_root.clone(),
            deadline,
        )
        .map_err(|error| {
            MintFailure::Refused(refusal_of(
                "verification_open_refused",
                &format!("the governed verification open refused: {error}"),
            ))
        })?;
    let requirement = frozen.evidence_required();
    let request = VerificationRequest {
        id: gauntlet_domain::VerificationRef::new(&format!("verify-operator-{unit}"))
            .map_err(|error| MintFailure::Failed(error.to_string()))?,
        run_id: run_id.clone(),
        unit_id: unit.clone(),
        claim: frozen.claim().to_owned(),
        requirement: requirement.clone(),
        evidence: requirement.locator().clone(),
    };
    governed.verify_claim(&request, deadline).map_err(|error| {
        MintFailure::Refused(refusal_of(
            "verification_refused",
            &format!("the governed verification refused: {error}"),
        ))
    })
}

/// The D6 run derivation over the durable queue: the distinct run ids
/// the OPEN tickets carry; exactly one is the anchor, several resolve to
/// the greatest ULID (ULID order is time order — the newest run), none
/// is an honest absence. The derivation is DISCLOSED by the surface's
/// `run_derivation` field, never a silent guess.
fn derived_run(queue: &gauntlet_adapter_escalation_fs::FsEscalationQueue) -> Result<RunId, String> {
    let listing = queue.list().map_err(|error| error.to_string())?;
    let mut runs: Vec<RunId> = listing
        .records
        .iter()
        .filter(|record| record.is_open())
        .map(|record| record.run_id.clone())
        .collect();
    runs.sort();
    runs.dedup();
    match runs.pop() {
        Some(newest) => Ok(newest),
        None => Err(
            "no active run is derivable from the durable artifacts — no open escalation \
             ticket names a run; pass the run explicitly"
                .to_owned(),
        ),
    }
}

fn refused(code: &str, message: &str) -> ActionDecision {
    ActionDecision::Refused(refusal_of(code, message))
}

fn refusal_of(code: &str, message: &str) -> Refusal {
    Refusal {
        code: code.to_owned(),
        message: message.to_owned(),
    }
}

/// The stable refusal-code vocabulary, one token per typed settlement
/// refusal (the JSON surface's error `code`; the CLI keeps the Display).
fn refusal_code(error: &gauntlet_app::settle::SettleError) -> &'static str {
    use gauntlet_app::settle::SettleError;
    match error {
        SettleError::RunNotDurable { .. } => "run_not_durable",
        SettleError::UnitNotFrozen { .. } => "unit_not_frozen",
        SettleError::AuthorityNotPermitted { .. } => "authority_not_permitted",
        SettleError::RationaleRequired { .. } => "rationale_required",
        SettleError::OperatorIdentityRequired { .. } => "operator_identity_required",
        SettleError::BlockingResidualOpen { .. } => "blocking_residual_open",
        SettleError::DurableBlockDiverged { .. } => "durable_block_diverged",
        SettleError::ReferenceMutated(_) => "reference_mutated",
        SettleError::ReferenceUnverifiable { .. } => "reference_unverifiable",
        SettleError::Reference(_) => "reference_provider",
        SettleError::VerificationRecordDisagrees { .. } => "verification_record_disagrees",
        SettleError::VerificationNotPass { .. } => "verification_not_pass",
        SettleError::OperatorActionRequiresEarnedReadiness { .. } => {
            "operator_action_requires_earned_readiness"
        }
        SettleError::ReadinessNotEarned(_) => "readiness_not_earned",
        SettleError::Admission(_) => "admission_refused",
        SettleError::VerificationRecording(_) => "verification_recording",
        SettleError::State(_) => "state_store",
        SettleError::Evidence(_) => "evidence_store",
        SettleError::Escalation(_) => "escalation_surface",
        SettleError::DomainTransition(_) => "domain_transition",
    }
}
