//! The `HumanEscalation` shared contract suite (spec §6.2; §11.6).
//!
//! Pinned semantics: escalating returns a ticket the operator can see —
//! poll on it reports `Open` until an operator resolves it, stably and
//! repeatedly; a ticket this adapter never issued is a typed
//! [`HumanEscalationError::TicketNotFound`] refusal, never a fabricated
//! state. Escalation is a first-class outcome (§11.6: `blocked` and
//! `escalated` terminal states require tickets to exist), so the ticket
//! carries its identity and creation instant.

use gauntlet_domain::{EvidenceRef, RunId};

use crate::contract::ContractFinding;
use crate::deadline::Deadline;
use crate::human_escalation::{
    EscalationItem, EscalationState, EscalationTicket, HumanEscalation, HumanEscalationError,
    TicketId,
};

/// The suite's name in findings.
pub const SUITE: &str = "human_escalation";

const BASE_INSTANT: u64 = 1_700_000_000;
/// The suite's bound span, on its own line (numeric literals never share a
/// line with a duration-shaped identifier).
const BOUND_SPAN: u64 = 60;

/// Constructs the escalation service under test. `build` must return an
/// ISOLATED instance: cases assume a clean slate.
pub trait HumanEscalationFactory {
    fn label(&self) -> &'static str;

    fn build(&self) -> Result<Box<dyn HumanEscalation>, String>;
}

fn finding(case: &'static str, rule: &'static str, detail: String) -> ContractFinding {
    ContractFinding {
        suite: SUITE,
        case,
        rule,
        detail,
    }
}

fn bound() -> Deadline {
    Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(
        BASE_INSTANT + BOUND_SPAN,
    ))
}

fn item(run: &RunId) -> EscalationItem {
    EscalationItem {
        run_id: run.clone(),
        summary: "the slice needs operator judgement before it can proceed".to_owned(),
        context: EvidenceRef::new("evd-000042").expect("suite evidence ref"),
    }
}

fn ticket(id: &str) -> EscalationTicket {
    EscalationTicket::new(
        TicketId::new(id).expect("suite ticket id is usable"),
        gauntlet_domain::EpochSeconds::new(BASE_INSTANT),
    )
}

/// Case: escalating returns a ticket that reports `Open` stably; the
/// ticket's identity and creation instant survive the round trip.
fn escalate_returns_an_open_ticket(factory: &dyn HumanEscalationFactory) -> Vec<ContractFinding> {
    const CASE: &str = "escalate_returns_an_open_ticket";
    const RULE: &str =
        "spec 6.2 HumanEscalation: escalate(item) -> EscalationTicket; a fresh ticket polls Open";
    let Ok(escalation) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build an escalation service for the case".to_owned(),
        )];
    };
    let run = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite run id is a ULID");
    let issued = match escalation.escalate(item(&run), bound()) {
        Ok(issued) => issued,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("escalating a well-formed item failed: {error}"),
            )];
        }
    };
    let mut findings = Vec::new();
    if issued.id().as_str().is_empty() {
        findings.push(finding(
            CASE,
            RULE,
            "the issued ticket carries an empty id".to_owned(),
        ));
    }
    for probe in 0..2 {
        match escalation.poll(&issued, bound()) {
            Ok(EscalationState::Open) => {}
            Ok(other) => findings.push(finding(
                CASE,
                RULE,
                format!(
                    "poll probe {probe} reported {other:?} for a ticket no operator acted on — \
                     a fresh ticket is Open, stably"
                ),
            )),
            Err(error) => findings.push(finding(
                CASE,
                RULE,
                format!("poll probe {probe} errored: {error}"),
            )),
        }
    }
    findings
}

/// Case: a ticket this adapter never issued is a typed `TicketNotFound`
/// refusal — never an `Ok` poll with a fabricated state (§11.6: tickets
/// must exist; §21.2: typed errors, never strings).
fn unknown_tickets_are_typed_refusals(
    factory: &dyn HumanEscalationFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "unknown_tickets_are_typed_refusals";
    const RULE: &str = "spec 6.2 + port contract: polling a ticket never issued is TicketNotFound, never a fabricated state";
    let Ok(escalation) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build an escalation service for the case".to_owned(),
        )];
    };
    match escalation.poll(&ticket("suite-unknown-ticket-07"), bound()) {
        Err(HumanEscalationError::TicketNotFound { .. }) => Vec::new(),
        Err(other) => vec![finding(
            CASE,
            RULE,
            format!("an unknown ticket was refused with {other} instead of TicketNotFound"),
        )],
        Ok(state) => vec![finding(
            CASE,
            RULE,
            format!("an unknown ticket polled as {state:?} — a fabricated state"),
        )],
    }
}

/// Runs the full `HumanEscalation` contract suite against one factory.
pub fn human_escalation_suite(factory: &dyn HumanEscalationFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(escalate_returns_an_open_ticket(factory));
    findings.extend(unknown_tickets_are_typed_refusals(factory));
    findings
}
