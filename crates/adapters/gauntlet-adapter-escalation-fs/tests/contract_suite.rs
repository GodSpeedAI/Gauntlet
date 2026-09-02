//! The shared `HumanEscalation` contract suite (spec §6.2; §11.6) run
//! against this adapter — "Adding an adapter means implementing the
//! trait and running the existing suite, not writing a bespoke one" —
//! plus the crate tests that prove the DURABILITY and typed-error
//! obligations the suite cannot see (no shared escalation suite exists
//! for the read/resolve surface, so these are the adapter's own).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use gauntlet_adapter_escalation_fs::{
    ESCALATIONS_DIR_NAME, EscalationQueueError, EscalationRecord, FsEscalationQueue,
    OperatorAction, QuarantineClass, TICKET_SCHEMA,
};
use gauntlet_domain::{EpochSeconds, EvidenceRef, RunId};
use gauntlet_ports::contract::ContractFinding;
use gauntlet_ports::contract::human_escalation::{HumanEscalationFactory, human_escalation_suite};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::human_escalation::{
    EscalationItem, EscalationState, HumanEscalation, TicketId,
};

/// Isolated scratch root per test; removed on drop. `tempfile` is not on
/// any layer's external allowlist, and a test helper is not a reason to
/// widen one.
struct Scratch {
    path: PathBuf,
}

static SCRATCH_SEQ: AtomicU32 = AtomicU32::new(0);

impl Scratch {
    fn new(case: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "gauntlet-escalation-fs-{case}-{}-{}",
            std::process::id(),
            SCRATCH_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("suite scratch directory is creatable");
        Self { path }
    }

    fn queue(&self) -> FsEscalationQueue {
        FsEscalationQueue::open(&self.path.join(ESCALATIONS_DIR_NAME))
            .expect("the queue root opens")
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).ok();
    }
}

/// The factory the shared suite parameterizes over: an ISOLATED queue
/// per `build` (the suite's cases assume a clean slate).
struct QueueFactory {
    root: PathBuf,
}

impl HumanEscalationFactory for QueueFactory {
    fn label(&self) -> &'static str {
        "fs-escalation-queue"
    }

    fn build(&self) -> Result<Box<dyn HumanEscalation>, String> {
        let seq = SCRATCH_SEQ.fetch_add(1, Ordering::Relaxed);
        let dir = self
            .root
            .join(format!("suite-case-{seq}"))
            .join(ESCALATIONS_DIR_NAME);
        std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        let queue = FsEscalationQueue::open(&dir).map_err(|error| error.to_string())?;
        Ok(Box::new(queue))
    }
}

fn findings_empty(findings: &[ContractFinding]) {
    assert!(
        findings.is_empty(),
        "the shared contract suite found defects: {findings:?}"
    );
}

#[test]
fn the_shared_human_escalation_suite_passes() {
    let scratch = Scratch::new("shared-suite");
    let factory = QueueFactory {
        root: scratch.path.clone(),
    };
    assert_eq!(factory.label(), "fs-escalation-queue");
    findings_empty(&human_escalation_suite(&factory));
}

fn escalation_item(run: &RunId) -> EscalationItem {
    EscalationItem {
        run_id: run.clone(),
        summary: "unit AK-042 needs an operator decision".to_owned(),
        context: EvidenceRef::new("evd-000042").expect("the fixture evidence ref parses"),
    }
}

fn fixture_run() -> RunId {
    RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("the fixture run id is a ULID")
}

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(1_700_000_000))
}

#[test]
fn a_resolved_ticket_reports_resolved_stably_and_carries_the_operator() {
    let scratch = Scratch::new("resolve-stable");
    let queue = scratch.queue();
    let ticket = queue
        .escalate(escalation_item(&fixture_run()), bound())
        .expect("the escalation is recorded");
    queue
        .resolve(
            ticket.id(),
            "operator-7",
            "approved by hand",
            OperatorAction::Resolve,
        )
        .expect("the open ticket resolves");
    for _ in 0..2 {
        let state = queue.poll(&ticket, bound()).expect("the poll reads");
        assert!(
            state.is_resolved(),
            "a resolved ticket polls Resolved, stably — got {state:?}"
        );
    }
    let record = queue.record(ticket.id()).expect("the record reads");
    let EscalationState::Resolved { operator, note } = record.state else {
        panic!("the record carries the resolved state");
    };
    assert_eq!(operator, "operator-7");
    assert_eq!(note, "approved by hand");
    assert_eq!(record.action, Some(OperatorAction::Resolve));
}

#[test]
fn a_second_resolve_on_a_ticket_is_a_typed_refusal_never_an_overwrite() {
    let scratch = Scratch::new("resolve-once");
    let queue = scratch.queue();
    let ticket = queue
        .escalate(escalation_item(&fixture_run()), bound())
        .expect("the escalation is recorded");
    queue
        .resolve(ticket.id(), "operator-7", "", OperatorAction::Retry)
        .expect("the first resolve lands");
    let refusal = queue
        .resolve(ticket.id(), "operator-8", "", OperatorAction::Retry)
        .expect_err("the second resolve is refused");
    assert_eq!(
        refusal,
        EscalationQueueError::AlreadyResolved {
            ticket: ticket.id().as_str().to_owned()
        }
    );
    // The FIRST resolution stands, byte for byte.
    let record = queue.record(ticket.id()).expect("the record reads");
    assert_eq!(
        record.action,
        Some(OperatorAction::Retry),
        "the first operator action survives"
    );
}

#[test]
fn the_list_is_total_ordered_oldest_first_and_complete() {
    let scratch = Scratch::new("list-order");
    let queue = scratch.queue();
    let first = queue
        .escalate(escalation_item(&fixture_run()), bound())
        .expect("the first escalation is recorded");
    let second = queue
        .escalate(escalation_item(&fixture_run()), bound())
        .expect("the second escalation is recorded");
    let listed = queue.list().expect("the queue lists");
    // (COR-F4 mechanical adaptation: `list` now returns the quarantine
    // listing — valid records plus corrupt files quarantined beside them.
    // This fixture plants only valid records, so the records arm is the
    // one under test here; the quarantine arms have their own tests.)
    assert_eq!(listed.records.len(), 2, "both tickets are listed");
    assert!(listed.quarantined.is_empty(), "nothing is quarantined");
    assert_eq!(listed.records[0].ticket, *first.id(), "oldest first");
    assert_eq!(listed.records[1].ticket, *second.id());
    let EscalationRecord { summary, .. } = &listed.records[0];
    assert!(summary.contains("operator decision"), "{summary:?}");
}

#[test]
fn a_hand_forged_record_is_a_typed_refusal_not_a_silent_member() {
    let scratch = Scratch::new("forged-record");
    let queue = scratch.queue();
    let ticket = queue
        .escalate(escalation_item(&fixture_run()), bound())
        .expect("the escalation is recorded");
    let path = scratch
        .path
        .join(ESCALATIONS_DIR_NAME)
        .join(format!("{}.json", ticket.id().as_str()));
    let original = std::fs::read_to_string(&path).expect("the record file reads");
    let forged = original.replace(
        &format!("\"schema\":\"{TICKET_SCHEMA}\""),
        "\"schema\":\"someone-elses-ticket@v9\"",
    );
    std::fs::write(&path, forged).expect("the forged record is written");
    let refusal = queue
        .record(ticket.id())
        .expect_err("a foreign-schema record is refused");
    assert!(
        matches!(refusal, EscalationQueueError::RecordInvalid { .. }),
        "{refusal}"
    );
}

#[test]
fn a_completed_write_leaves_no_temp_file_behind() {
    let scratch = Scratch::new("no-temp-left");
    let queue = scratch.queue();
    queue
        .escalate(escalation_item(&fixture_run()), bound())
        .expect("the escalation is recorded");
    let leftovers: Vec<_> = std::fs::read_dir(scratch.path.join(ESCALATIONS_DIR_NAME))
        .expect("the queue dir reads")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().and_then(|ext| ext.to_str()) == Some("tmp"))
        .collect();
    assert!(
        leftovers.is_empty(),
        "the atomic write renames its temp away: {leftovers:?}"
    );
}

#[test]
fn a_lost_ticket_is_a_typed_refusal_on_the_read_surface() {
    let scratch = Scratch::new("lost-ticket");
    let queue = scratch.queue();
    let unknown = TicketId::new("esc-never-minted-1").expect("the fixture ticket id is usable");
    let refusal = queue
        .record(&unknown)
        .expect_err("a lost ticket is refused");
    assert!(
        matches!(refusal, EscalationQueueError::RecordInvalid { .. }),
        "{refusal}"
    );
}

// ---------------------------------------------------------------------------
// COR-F3 (falsifier round 1, F-3): the ticket id is an opaque token, never
// a path fragment. The port's grammar refuses separators and traversal
// components, and the QUEUE validates the resolved path's containment on
// BOTH the read and the write path — the belt behind the grammar's
// suspenders.
// ---------------------------------------------------------------------------

#[test]
fn a_traversal_ticket_id_is_refused_by_the_grammar() {
    for payload in [
        "../escaped",
        "..",
        "a/b",
        "/absolute",
        "nested/../escape",
        ".",
        "./here",
    ] {
        let error = TicketId::new(payload)
            .expect_err("a traversal-flavored payload is not a usable ticket id");
        assert!(
            matches!(
                error,
                gauntlet_ports::human_escalation::HumanEscalationError::IdentityInvalid { .. }
            ),
            "{payload:?}: got {error:?}"
        );
    }
    // The legitimate class is untouched.
    TicketId::new("esc-0001f-42a").expect("a plain token stays valid");
}

#[test]
fn an_out_of_root_ticket_file_is_refused_on_the_read_path() {
    // The falsifier's F-3 reproduction, shipped as a tooth: a
    // schema-valid-looking ticket planted ONE LEVEL ABOVE the queue root,
    // addressed by a traversal id. The grammar refuses the id at the CLI
    // boundary; the queue's own containment check refuses any route that
    // could still reach the file. Nothing outside the root is read.
    let scratch = Scratch::new("out-of-root-read");
    let planted = scratch.path.join("escaped.json");
    std::fs::write(
        &planted,
        format!(
            "{{\"schema\":\"{TICKET_SCHEMA}\",\"ticket\":\"../escaped\",\"run_id\":\"{}\",\
             \"summary\":\"forged\",\"context\":\"evd-1\",\"created_at\":1,\"state\":\"open\"}}",
            fixture_run()
        ),
    )
    .expect("the forged out-of-root file is planted");
    let queue = scratch.queue();
    for payload in ["../escaped", "escaped", "./escaped"] {
        let id = match TicketId::new(payload) {
            Ok(id) => id,
            Err(_) => continue,
        };
        let refusal = queue.record(&id).expect_err("the read is refused");
        assert!(
            matches!(refusal, EscalationQueueError::RecordInvalid { .. }),
            "{payload:?}: {refusal}"
        );
    }
}

#[test]
fn a_forged_records_own_traversal_ticket_field_cannot_steer_the_write() {
    // The falsifier's write-back observation: resolve() writes to the
    // RECORD's own ticket field. A record whose decoded ticket field could
    // traverse is now impossible twice over — the grammar refuses it at
    // decode (the file cannot even become a record), and the write path
    // validates containment independently.
    let scratch = Scratch::new("forged-write");
    let queue = scratch.queue();
    let ticket = queue
        .escalate(escalation_item(&fixture_run()), bound())
        .expect("the escalation is recorded");
    // Forge the DECODED record's ticket field through the record API is
    // impossible, so forge at the file layer: a record whose ticket field
    // carries a traversal must be a decode refusal, and resolve() of the
    // ORIGINAL ticket must write only inside the root.
    let path = scratch
        .path
        .join(ESCALATIONS_DIR_NAME)
        .join(format!("{}.json", ticket.id().as_str()));
    let original = std::fs::read_to_string(&path).expect("the record file reads");
    let forged = original.replace(
        &format!("\"ticket\":\"{}\"", ticket.id().as_str()),
        "\"ticket\":\"../escaped\"",
    );
    std::fs::write(&path, forged).expect("the forged record is written");
    let refusal = queue
        .record(ticket.id())
        .expect_err("a record with a traversal ticket field is refused at decode");
    assert!(
        matches!(refusal, EscalationQueueError::RecordInvalid { .. }),
        "{refusal}"
    );
    // Nothing was written outside the queue root.
    assert!(
        !scratch.path.join("escaped.json").exists(),
        "no out-of-root file appeared"
    );
    assert!(
        !scratch
            .path
            .join("escalations")
            .join("escaped.json")
            .exists(),
        "no traversal-named file appeared inside the root"
    );
}

// ---------------------------------------------------------------------------
// COR-F4 (falsifier round 1, F-4): one corrupt file must not blind the
// operator's ENTIRE queue. The listing quarantines corrupt files as typed
// entries; valid tickets still list; actions on quarantined tickets fail
// closed.
// ---------------------------------------------------------------------------

#[test]
fn a_corrupt_file_is_quarantined_beside_the_valid_tickets() {
    let scratch = Scratch::new("quarantine-list");
    let queue = scratch.queue();
    let first = queue
        .escalate(escalation_item(&fixture_run()), bound())
        .expect("the first escalation is recorded");
    // A corrupt file among the valid ones: truncated JSON.
    std::fs::write(
        scratch
            .path
            .join(ESCALATIONS_DIR_NAME)
            .join("esc-broken.json"),
        "{\"garbage",
    )
    .expect("the corrupt file is planted");
    let listing = queue.list().expect("the listing survives the corrupt file");
    assert_eq!(listing.records.len(), 1, "the valid ticket is still listed");
    assert_eq!(listing.records[0].ticket, *first.id());
    assert_eq!(listing.quarantined.len(), 1, "the corrupt file is named");
    let quarantined = &listing.quarantined[0];
    assert_eq!(quarantined.ticket, "esc-broken");
    assert_eq!(quarantined.class, QuarantineClass::RecordInvalid);
}

#[test]
fn an_unreadable_file_quarantines_as_unreadable() {
    let scratch = Scratch::new("quarantine-unreadable");
    let queue = scratch.queue();
    // A directory named like a ticket file: read_to_string refuses it.
    std::fs::create_dir_all(scratch.path.join(ESCALATIONS_DIR_NAME).join("esc-dir.json"))
        .expect("the impostor is planted");
    let listing = queue.list().expect("the listing survives");
    assert_eq!(listing.records.len(), 0);
    assert_eq!(listing.quarantined.len(), 1);
    assert_eq!(listing.quarantined[0].ticket, "esc-dir");
    assert_eq!(listing.quarantined[0].class, QuarantineClass::Unreadable);
}

#[test]
fn an_action_on_a_quarantined_ticket_fails_closed() {
    let scratch = Scratch::new("quarantine-action");
    let queue = scratch.queue();
    std::fs::write(
        scratch
            .path
            .join(ESCALATIONS_DIR_NAME)
            .join("esc-broken.json"),
        "{\"garbage",
    )
    .expect("the corrupt file is planted");
    let id = TicketId::new("esc-broken").expect("the stem is a usable id token");
    let refusal = queue.record(&id).expect_err("the action's read refuses");
    assert!(
        matches!(refusal, EscalationQueueError::RecordInvalid { .. }),
        "{refusal}"
    );
    // And resolve() — the queue's only write — refuses on the same read.
    let refusal = queue
        .resolve(&id, "operator-7", "", OperatorAction::Resolve)
        .expect_err("the write refuses too");
    assert!(
        matches!(refusal, EscalationQueueError::RecordInvalid { .. }),
        "{refusal}"
    );
}

// ---------------------------------------------------------------------------
// COR-R2-2 (falsifier round 2, F-R2-1): ticket IDENTITY is bound to the
// file. A decodable-but-lying record — one whose own `ticket` field names
// ANOTHER ticket — is refused typed on the read path, quarantined out of
// the operator's listing, and can never steer an operator action's
// write-back onto the victim: the write targets the REQUESTED id.
// ---------------------------------------------------------------------------

#[test]
fn a_lying_ticket_file_cannot_clobber_another_tickets_record() {
    let scratch = Scratch::new("lying-ticket");
    let queue = scratch.queue();
    let a = queue
        .escalate(escalation_item(&fixture_run()), bound())
        .expect("ticket A mints");
    let b = queue
        .escalate(escalation_item(&fixture_run()), bound())
        .expect("ticket B mints");
    // THE ATTACK (the falsifier's exact reproduction): A's FILE is
    // rewritten out-of-band to a well-formed `gauntlet-escalation-ticket@v1`
    // record whose `ticket` field names B.
    let path_a = scratch
        .path
        .join(ESCALATIONS_DIR_NAME)
        .join(format!("{}.json", a.id().as_str()));
    let original = std::fs::read_to_string(&path_a).expect("A's record file reads");
    let lying = original.replace(
        &format!("\"ticket\":\"{}\"", a.id().as_str()),
        &format!("\"ticket\":\"{}\"", b.id().as_str()),
    );
    std::fs::write(&path_a, &lying).expect("the lying file is written");
    // The resolve aimed at A refuses typed — the clobbering write never
    // happens, so the operator's action can never land on B's file.
    let refusal = queue
        .resolve(a.id(), "operator-7", "", OperatorAction::Resolve)
        .expect_err("the lying ticket's resolve is refused");
    assert!(
        matches!(refusal, EscalationQueueError::TicketIdentityMismatch { .. }),
        "{refusal}"
    );
    // A's own file stands untouched (the refusal wrote nothing).
    let after = std::fs::read_to_string(&path_a).expect("A's file re-reads");
    assert_eq!(after, lying, "the refused resolve wrote nothing");
    // The victim is untouched: B's own record, still open, under its own
    // id — never overwritten with the forged content.
    let victim = queue.record(b.id()).expect("B's record reads");
    assert_eq!(victim.ticket, *b.id(), "B's record names B");
    assert!(
        !victim.state.is_resolved(),
        "B was never clobbered resolved: {:?}",
        victim.state
    );
    // And B is poll-visible as its own state.
    let polled = queue.poll(&b, bound()).expect("B polls");
    assert!(
        !polled.is_resolved(),
        "B polls its own open state, got {polled:?}"
    );
    // A forged `Resolved` can never be reported through poll either.
    let refusal = queue.poll(&a, bound()).expect_err("the lying poll refuses");
    assert!(
        matches!(
            refusal,
            gauntlet_ports::human_escalation::HumanEscalationError::StateReadFailed { .. }
        ),
        "{refusal:?}"
    );
    // The operator's list stays honest: the lying file quarantines (coarse
    // class, content never echoed) and B lists once, as itself.
    let listing = queue.list().expect("the queue lists");
    assert_eq!(
        listing.records.len(),
        1,
        "only the honest record lists: {:?}",
        listing.records
    );
    assert_eq!(listing.records[0].ticket, *b.id());
    assert!(
        listing
            .quarantined
            .iter()
            .any(|entry| entry.ticket == a.id().as_str()
                && entry.class == QuarantineClass::RecordInvalid),
        "the lying file quarantines as record-invalid: {:?}",
        listing.quarantined
    );
    // The REAL victim still resolves normally — the defense refuses only
    // the cross-wired document, never the operator's legitimate action.
    queue
        .resolve(b.id(), "operator-7", "", OperatorAction::Resolve)
        .expect("the real victim resolves normally");
    let resolved = queue.record(b.id()).expect("B's resolved record reads");
    assert!(resolved.state.is_resolved());
    assert_eq!(resolved.action, Some(OperatorAction::Resolve));
}
