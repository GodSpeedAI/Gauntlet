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
use crate::event_log::{EventLog, LogEntry};
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
    // not just across copies of one event.
    let kind = match sequence {
        0 => RunEventKind::InvocationAdmitted {
            invocation_id: InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV")
                .expect("suite id is a ULID"),
        },
        1 => RunEventKind::SettlementCommitted {
            unit_id: UnitId::new("AK-014").expect("suite unit id"),
            evidence: EvidenceRef::new("evd-000042").expect("suite evidence ref"),
            verification: VerificationRef::new("vrf-000042").expect("suite verification ref"),
            reference_digest: Digest::new("sha256", "a1b2c3d4e5f6a7b8").expect("suite digest"),
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

/// Runs the full `EventLog` contract suite against one factory.
pub fn event_log_suite(factory: &dyn EventLogFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(append_returns_monotonic_offsets(factory));
    findings.extend(read_from_replays_exact(factory));
    findings
}
