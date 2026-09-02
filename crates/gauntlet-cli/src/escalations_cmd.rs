//! `gauntlet escalations list | resolve | retry` (spec §20.1/§20.2; plan
//! Task 20, charter D8/D9): the operator's escalation queue. `list`
//! reads the fs escalation adapter's records — the escalation READ lives
//! with the escalation adapter, never with the StateStore. `resolve` /
//! `retry` go through `SettlementService::settle_operator_action` — the
//! SAME service path as automatic settlement (§16.3, D9: no second
//! writer) — and only a service-accepted action resolves the ticket.

use std::process::ExitCode;

use gauntlet_ports::deadline::Deadline;

use crate::compose::open_operator_surface;
use crate::context::resolve_context;

fn bound() -> Deadline {
    crate::context::doctor_bound()
}

/// `gauntlet escalations list`.
pub fn escalations(args: &[String]) -> ExitCode {
    match args.first().map(String::as_str) {
        Some("list") => list(&args[1..]),
        Some("resolve") => operator_action(
            &args[1..],
            gauntlet_adapter_escalation_fs::OperatorAction::Resolve,
        ),
        Some("retry") => operator_action(
            &args[1..],
            gauntlet_adapter_escalation_fs::OperatorAction::Retry,
        ),
        Some(other) => {
            eprintln!("gauntlet escalations: unknown subcommand '{other}'");
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
    println!("usage: gauntlet escalations list");
    println!(
        "       gauntlet escalations resolve --ticket <id> --unit <unit-id> --operator <who> \
         [--note <text>]"
    );
    println!(
        "       gauntlet escalations retry   --ticket <id> --unit <unit-id> --operator <who> \
         [--note <text>]"
    );
}

fn list(args: &[String]) -> ExitCode {
    if let Some(other) = args.first() {
        eprintln!("gauntlet escalations list: unknown argument '{other}'");
        println!("usage: gauntlet escalations list");
        return ExitCode::from(2);
    }
    let context = match resolve_context(None) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("gauntlet escalations: the configuration did not resolve: {error}");
            return ExitCode::FAILURE;
        }
    };
    let root = context
        .state_dir()
        .join(gauntlet_adapter_escalation_fs::ESCALATIONS_DIR_NAME);
    if !root.is_dir() {
        println!("escalations: queue empty (no queue at {})", root.display());
        return ExitCode::SUCCESS;
    }
    let queue = match gauntlet_adapter_escalation_fs::FsEscalationQueue::open(&root) {
        Ok(queue) => queue,
        Err(error) => {
            eprintln!("gauntlet escalations: {error}");
            return ExitCode::FAILURE;
        }
    };
    // COR-F4: the listing QUARANTINES corrupt ticket files instead of
    // failing the whole call — the operator's inbox must not go blind
    // because one file is corrupt. Valid tickets list as always; each
    // corrupt file appears as a typed quarantined entry (id + error
    // class, no content echoed). The exit stays 0: a readable queue that
    // reports what it could not decode has done its job. Any ACTION on a
    // quarantined ticket fails closed at the queue's read path.
    let listing = match queue.list() {
        Ok(listing) => listing,
        Err(error) => {
            eprintln!("gauntlet escalations: {error}");
            return ExitCode::FAILURE;
        }
    };
    for entry in &listing.quarantined {
        println!(
            "escalations: ticket={} state=QUARANTINED reason={} — the file is not a valid \
             ticket record; actions on it are refused",
            entry.ticket,
            entry.class.token()
        );
    }
    let records = listing.records;
    if records.is_empty() && listing.quarantined.is_empty() {
        println!("escalations: queue empty");
        return ExitCode::SUCCESS;
    }
    if records.is_empty() {
        return ExitCode::SUCCESS;
    }
    for record in records {
        let state = if record.is_open() {
            "open".to_owned()
        } else {
            format!(
                "resolved action={} operator={}",
                record.action.map(|a| a.token()).unwrap_or("?"),
                match &record.state {
                    gauntlet_ports::human_escalation::EscalationState::Resolved {
                        operator,
                        ..
                    } => operator.as_str(),
                    _ => "?",
                }
            )
        };
        println!(
            "escalations: ticket={} run={} state={} summary={:?}",
            record.ticket, record.run_id, state, record.summary
        );
    }
    ExitCode::SUCCESS
}

/// The operator action path (D9): through the settlement service's
/// operator request, with the operator's identity. The service refuses
/// exactly what the automatic path refuses (AK-062's class); only an
/// ACCEPTED action resolves the ticket — a refused action leaves the
/// queue untouched and exits non-zero with the typed refusal.
#[allow(clippy::arc_with_non_send_sync)] // single-threaded surface; the service stores the same Arcs
fn operator_action(
    args: &[String],
    action: gauntlet_adapter_escalation_fs::OperatorAction,
) -> ExitCode {
    let action_word = action.token();
    let mut ticket: Option<String> = None;
    let mut unit: Option<String> = None;
    let mut operator: Option<String> = None;
    let mut note = String::new();
    let mut index = 0;
    while index < args.len() {
        let take =
            |name: &str, slot: &mut Option<String>, args: &[String], index: &mut usize| -> bool {
                match args.get(*index + 1) {
                    Some(value) => {
                        if slot.replace(value.clone()).is_some() {
                            eprintln!("gauntlet escalations {action_word}: --{name} given twice");
                            return false;
                        }
                        *index += 2;
                        true
                    }
                    None => {
                        eprintln!("gauntlet escalations {action_word}: --{name} requires a value");
                        false
                    }
                }
            };
        match args[index].as_str() {
            "--ticket" => {
                if !take("ticket", &mut ticket, args, &mut index) {
                    return ExitCode::from(2);
                }
            }
            "--unit" => {
                if !take("unit", &mut unit, args, &mut index) {
                    return ExitCode::from(2);
                }
            }
            "--operator" => {
                if !take("operator", &mut operator, args, &mut index) {
                    return ExitCode::from(2);
                }
            }
            "--note" => match args.get(index + 1) {
                Some(value) => {
                    note = value.clone();
                    index += 2;
                }
                None => {
                    eprintln!("gauntlet escalations {action_word}: --note requires a value");
                    return ExitCode::from(2);
                }
            },
            other => {
                eprintln!("gauntlet escalations {action_word}: unknown argument '{other}'");
                usage();
                return ExitCode::from(2);
            }
        }
    }
    let (Some(ticket), Some(unit), Some(operator)) = (ticket, unit, operator) else {
        eprintln!(
            "gauntlet escalations {action_word}: --ticket, --unit and --operator are all \
             required (the operator identity is recorded on every action, §20.2)"
        );
        usage();
        return ExitCode::from(2);
    };
    let Ok(ticket_id) = gauntlet_ports::human_escalation::TicketId::new(&ticket) else {
        eprintln!("gauntlet escalations {action_word}: {ticket} is not a usable ticket id");
        return ExitCode::FAILURE;
    };
    let Ok(unit_id) = gauntlet_domain::UnitId::new(&unit) else {
        eprintln!("gauntlet escalations {action_word}: {unit} is not a valid unit id");
        return ExitCode::FAILURE;
    };
    let context = match resolve_context(None) {
        Ok(context) => context,
        Err(error) => {
            eprintln!("gauntlet escalations: the configuration did not resolve: {error}");
            return ExitCode::FAILURE;
        }
    };
    let surface = match open_operator_surface(&context) {
        Ok(surface) => surface,
        Err(error) => {
            eprintln!("gauntlet escalations: {error}");
            return ExitCode::FAILURE;
        }
    };
    let root = &surface.escalation_root;
    let queue: std::sync::Arc<gauntlet_adapter_escalation_fs::FsEscalationQueue> =
        match gauntlet_adapter_escalation_fs::FsEscalationQueue::open(root) {
            Ok(queue) => std::sync::Arc::new(queue),
            Err(error) => {
                eprintln!("gauntlet escalations: {error}");
                return ExitCode::FAILURE;
            }
        };
    let record = match queue.record(&ticket_id) {
        Ok(record) => record,
        Err(error) => {
            eprintln!("gauntlet escalations {action_word}: {error}");
            return ExitCode::FAILURE;
        }
    };
    let identity = match gauntlet_app::settle::OperatorIdentity::new(&operator) {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!(
                "gauntlet escalations {action_word}: the operator identity is not usable: \
                     {error}"
            );
            return ExitCode::FAILURE;
        }
    };
    let request = gauntlet_app::settle::OperatorActionRequest {
        run_id: record.run_id.clone(),
        unit_id,
        identity: Some(identity),
    };
    // The SAME settlement service construction the run composition uses
    // (§16.3's single write path; D9). Two handles into the SAME stores:
    // the service settles, and the queue handle — used only AFTER the
    // service accepted — records the operator action on the ticket.
    let evidence =
        match gauntlet_adapter_evidence_fs::FsEvidenceStore::open(&context.resolved.evidence_dir) {
            Ok(store) => store,
            Err(error) => {
                eprintln!("gauntlet escalations: {error}");
                return ExitCode::FAILURE;
            }
        };
    let evidence_for_provider =
        match gauntlet_adapter_evidence_fs::FsEvidenceStore::open(&context.resolved.evidence_dir) {
            Ok(store) => store,
            Err(error) => {
                eprintln!("gauntlet escalations: {error}");
                return ExitCode::FAILURE;
            }
        };
    let reference = gauntlet_app::reference::provider::FileReferenceProvider::from_paths(
        surface.answer_key.clone(),
        surface.manifest.clone(),
        std::rc::Rc::new(evidence_for_provider),
    );
    let clock: std::sync::Arc<dyn gauntlet_ports::clock::Clock> =
        std::sync::Arc::new(gauntlet_adapter_agent_prime::SystemClock::new());
    let service = gauntlet_app::settle::SettlementService::new(
        surface.state.clone(),
        std::sync::Arc::new(evidence),
        std::sync::Arc::new(reference),
        queue.clone(),
        clock,
    );
    match service.settle_operator_action(request, bound()) {
        Ok(()) => {
            // The service accepted: NOW the queue record resolves.
            if let Err(error) = queue.resolve(&ticket_id, &operator, &note, action) {
                eprintln!(
                    "gauntlet escalations: the settle committed but the queue write \
                           failed: {error}"
                );
                return ExitCode::FAILURE;
            }
            println!(
                "escalations: ticket={} action={} operator={:?} — settled through the \
                 settlement service's operator path",
                ticket, action_word, operator
            );
            ExitCode::SUCCESS
        }
        Err(error) => {
            eprintln!(
                "gauntlet escalations {action_word}: the settlement service refused — the \
                 ticket stays open: {error}"
            );
            ExitCode::FAILURE
        }
    }
}
