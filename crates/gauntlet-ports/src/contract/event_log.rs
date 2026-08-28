//! The `EventLog` shared contract suite (spec §6.2: "Append-only,
//! fsync-on-append"; §16.3 durability spine).
//!
//! Pinned semantics: appends return strictly increasing offsets; `read_from`
//! replays entries at or after the given offset in append order, exactly as
//! appended; an offset past the end replays empty; `last_offset` is `None`
//! on an empty log and the last append's offset otherwise. (fsync-on-append
//! itself is adapter behavior; crash-injection testing arrives with the
//! state-store task.)

use gauntlet_domain::{
    Digest, EpochSeconds, EventLogOffset, EvidenceRef, InvocationId, IterationNo, RunId, UnitId,
    VerificationRef,
};

use crate::contract::ContractFinding;
use crate::deadline::Deadline;
use crate::event_log::{EventLog, EventLogError, LogEntry};
use crate::state_store::{RunEvent, RunEventKind};

/// The suite's name in findings.
pub const SUITE: &str = "event_log";

const BASE_INSTANT: u64 = 1_700_000_000;
/// The suite's bound span, on its own line (numeric literals never share a
/// line with a duration-shaped identifier).
const BOUND_SPAN: u64 = 60;

/// Constructs the log under test. `build` must return an ISOLATED, EMPTY
/// instance.
pub trait EventLogFactory {
    fn label(&self) -> &'static str;

    fn build(&self) -> Result<Box<dyn EventLog>, String>;
}

fn finding(case: &'static str, rule: &'static str, detail: String) -> ContractFinding {
    ContractFinding::new(SUITE, case, rule, detail)
}

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + BOUND_SPAN))
}

fn event(run: &RunId, sequence: usize) -> RunEvent {
    // Three distinct shapes so replay exactness is checked across kinds,
    // not just across copies of one event. COR3: ordinary runtime kinds
    // only — the settlement kind is earned through the store's
    // `commit_settlement` and refused on this surface (pinned by the
    // refusal case below).
    let kind = match sequence {
        0 => RunEventKind::InvocationAdmitted {
            invocation_id: InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV")
                .expect("suite id is a ULID"),
        },
        1 => RunEventKind::EvidenceProduced {
            evidence: EvidenceRef::new("evd-000042").expect("suite evidence ref"),
            locator: gauntlet_domain::EvidenceLocator::new("suite-evidence-report.json")
                .expect("suite locator"),
        },
        _ => RunEventKind::CheckpointWritten {
            iteration_no: IterationNo::new(1).expect("suite iteration"),
            event_log_offset: EventLogOffset::new(7),
        },
    };
    RunEvent {
        run_id: run.clone(),
        kind,
        at: EpochSeconds::new(BASE_INSTANT + sequence as u64),
    }
}

fn entry(offset: u64, run: &RunId, sequence: usize) -> LogEntry {
    LogEntry {
        offset: EventLogOffset::new(offset),
        event: event(run, sequence),
    }
}

/// Case: an empty log reports `last_offset() == None`; three appends return
/// strictly increasing offsets.
fn append_returns_monotonic_offsets(factory: &dyn EventLogFactory) -> Vec<ContractFinding> {
    const CASE: &str = "append_returns_monotonic_offsets";
    const RULE: &str = "spec 6.2 EventLog: offsets are monotonic; an empty log has no last offset";
    let mut findings = Vec::new();
    let Ok(log) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a log for the case".to_owned(),
        )];
    };
    match log.last_offset(bound()) {
        Ok(None) => {}
        Ok(Some(offset)) => findings.push(finding(
            CASE,
            RULE,
            format!("a fresh log reported last offset {offset:?} — it must be empty"),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("last_offset on an empty log errored: {error}"),
        )),
    }
    let run = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite run id is a ULID");
    let mut previous: Option<EventLogOffset> = None;
    for sequence in 0..3 {
        match log.append(event(&run, sequence), bound()) {
            Ok(offset) => {
                if let Some(prior) = previous
                    && offset <= prior
                {
                    findings.push(finding(
                        CASE,
                        RULE,
                        format!(
                            "append {sequence} returned offset {}, not greater than the \
                             previous {}",
                            offset.get(),
                            prior.get()
                        ),
                    ));
                }
                previous = Some(offset);
            }
            Err(error) => {
                findings.push(finding(
                    CASE,
                    RULE,
                    format!("append {sequence} failed: {error}"),
                ));
                return findings;
            }
        }
    }
    findings
}

/// Case: `read_from` replays exactly what was appended, in order, from any
/// offset; an offset past the end replays empty; `last_offset` agrees with
/// the final append.
fn read_from_replays_exact(factory: &dyn EventLogFactory) -> Vec<ContractFinding> {
    const CASE: &str = "read_from_replays_exact";
    const RULE: &str = "spec 6.2 EventLog: read_from(offset) replays entries at or after the offset, in append order";
    let mut findings = Vec::new();
    let Ok(log) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a log for the case".to_owned(),
        )];
    };
    let run = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite run id is a ULID");
    let mut offsets = Vec::new();
    for sequence in 0..3 {
        match log.append(event(&run, sequence), bound()) {
            Ok(offset) => offsets.push(offset),
            Err(error) => {
                findings.push(finding(
                    CASE,
                    RULE,
                    format!("append {sequence} failed: {error}"),
                ));
                return findings;
            }
        }
    }
    let expected_all: Vec<LogEntry> = (0..3)
        .map(|sequence| entry(offsets[sequence].get(), &run, sequence))
        .collect();
    match log.read_from(EventLogOffset::new(0), bound()) {
        Ok(replay) if replay == expected_all => {}
        Ok(replay) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "full replay returned {} entries instead of 3 exactly as appended",
                replay.len()
            ),
        )),
        Err(error) => findings.push(finding(CASE, RULE, format!("full replay errored: {error}"))),
    }
    // The tail from the second offset.
    let expected_tail: Vec<LogEntry> = (1..3)
        .map(|sequence| entry(offsets[sequence].get(), &run, sequence))
        .collect();
    match log.read_from(offsets[1], bound()) {
        Ok(replay) if replay == expected_tail => {}
        Ok(replay) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "replay from the second offset returned {} entries instead of the exact tail",
                replay.len()
            ),
        )),
        Err(error) => findings.push(finding(CASE, RULE, format!("tail replay errored: {error}"))),
    }
    // Past the end replays empty.
    match log.read_from(EventLogOffset::new(offsets[2].get() + 1), bound()) {
        Ok(replay) if replay.is_empty() => {}
        Ok(replay) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "replay from past the end returned {} entries instead of none",
                replay.len()
            ),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("replay from past the end errored: {error}"),
        )),
    }
    // last_offset agrees with the final append.
    match log.last_offset(bound()) {
        Ok(Some(offset)) if offset == offsets[2] => {}
        Ok(other) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "last_offset returned {other:?} after appending up to {last}",
                last = offsets[2].get()
            ),
        )),
        Err(error) => findings.push(finding(CASE, RULE, format!("last_offset errored: {error}"))),
    }
    findings
}

/// Case: COR3 (confirmation round 1, F-1) — the settlement event kind is
/// authority-minted and EARNED through the store's `commit_settlement`
/// (consuming an `AdmittedSettlement`); this log is the same durability
/// spine the §18.3 replay mints settlement rows from, so a raw
/// `SettlementCommitted` append here forges the durable settlement
/// substrate. The append must refuse with the typed
/// `SettlementEventNotEarned` error and write nothing durable.
fn settlement_kind_appends_are_refused(factory: &dyn EventLogFactory) -> Vec<ContractFinding> {
    const CASE: &str = "settlement_kind_appends_are_refused";
    const RULE: &str = "spec 16.3 via COR3: the settlement kind reaches the log only through the earned commit — the log's generic append refuses it with SettlementEventNotEarned and writes no row";
    let mut findings = Vec::new();
    let Ok(log) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a log for the case".to_owned(),
        )];
    };
    let run = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite run id is a ULID");
    let forged = RunEvent {
        run_id: run.clone(),
        kind: RunEventKind::SettlementCommitted {
            unit_id: UnitId::new("AK-990").expect("suite unit id"),
            evidence: EvidenceRef::new("evd-forged-evidence").expect("suite evidence ref"),
            verification: VerificationRef::new("vrf-forged-0001").expect("suite verification ref"),
            reference_digest: Digest::new("sha256", "c4d3e2f1a0b1c2d3").expect("suite digest"),
        },
        at: EpochSeconds::new(BASE_INSTANT),
    };
    match log.append(forged, bound()) {
        Err(EventLogError::SettlementEventNotEarned { .. }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the raw settlement append was refused for the wrong reason (want \
                 SettlementEventNotEarned): {other}"
            ),
        )),
        Ok(offset) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "a raw settlement event appended at offset {} — the kind is earned-only \
                 (COR3, F-1)",
                offset.get()
            ),
        )),
    }
    // The refused append wrote no row: the log is empty.
    match log.last_offset(bound()) {
        Ok(None) => {}
        Ok(Some(offset)) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the refused append left a durable row at offset {}",
                offset.get()
            ),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("last_offset errored after the refusal: {error}"),
        )),
    }
    findings
}

/// Runs the full `EventLog` contract suite against one factory.
pub fn event_log_suite(factory: &dyn EventLogFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(append_returns_monotonic_offsets(factory));
    findings.extend(read_from_replays_exact(factory));
    findings.extend(settlement_kind_appends_are_refused(factory));
    findings
}
