//! The shared contract suites (spec §7.3) run against this adapter — both
//! ports it implements. "Adding an adapter means implementing the trait and
//! running the existing suite, not writing a bespoke one": the suites live
//! in `gauntlet-ports::contract::{state_store,event_log}`, and this file
//! only supplies the factories the Task 4 mechanism parameterizes over.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use gauntlet_adapter_state_sqlite::{
    SqliteCapabilityLedger, SqliteEventLog, SqliteRevisionLedger, SqliteStateStore,
};
use gauntlet_domain::{EpochSeconds, RevisionId};
use gauntlet_ports::capability_ledger::CapabilityLedger;
use gauntlet_ports::contract::capability_ledger::{
    CapabilityLedgerFactory, capability_ledger_suite,
};
use gauntlet_ports::contract::event_log::{EventLogFactory, event_log_suite};
use gauntlet_ports::contract::revision_ledger::{RevisionLedgerFactory, revision_ledger_suite};
use gauntlet_ports::contract::state_store::{StateStoreFactory, state_store_suite};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::event_log::EventLog;
use gauntlet_ports::revision_ledger::{
    DecisionKind, GateDecision, RestoreRecord, RevisionDecisionRecord, RevisionLedger,
    RevisionPromotionEvidence,
};
use gauntlet_ports::state_store::StateStore;

/// Isolated scratch database per suite case. The guard is deliberately
/// LEAKED (`mem::forget` in the factories below) because the store under
/// test must outlive the guard — the `Box<dyn StateStore>` the suite holds
/// keeps the file open. The cost is one bounded scratch file per suite run,
/// left in the temp directory until the OS sweeps it; not removed on drop,
/// by design. `tempfile` is not on any layer's external allowlist, and a
/// test helper is not a reason to widen one.
struct Scratch {
    path: PathBuf,
}

static SCRATCH_SEQ: AtomicU32 = AtomicU32::new(0);

impl Scratch {
    fn new(case: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "gauntlet-state-suite-{case}-{}-{}",
            std::process::id(),
            SCRATCH_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::remove_dir_all(&path).ok();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("suite scratch parent is creatable");
        }
        Self { path }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_file(&self.path).ok();
        std::fs::remove_dir_all(&self.path).ok();
    }
}

struct SqliteStateFactory;

impl StateStoreFactory for SqliteStateFactory {
    fn label(&self) -> &'static str {
        "sqlite-state-store"
    }

    fn build(&self) -> Result<Box<dyn StateStore>, String> {
        let scratch = Scratch::new("state");
        let path = scratch.path.clone();
        std::mem::forget(scratch);
        SqliteStateStore::open(&path)
            .map(|store| Box::new(store) as Box<dyn StateStore>)
            .map_err(|error| error.to_string())
    }
}

struct SqliteEventLogFactory;

impl EventLogFactory for SqliteEventLogFactory {
    fn label(&self) -> &'static str {
        "sqlite-event-log"
    }

    fn build(&self) -> Result<Box<dyn EventLog>, String> {
        let scratch = Scratch::new("log");
        let path = scratch.path.clone();
        std::mem::forget(scratch);
        SqliteEventLog::open(&path)
            .map(|log| Box::new(log) as Box<dyn EventLog>)
            .map_err(|error| error.to_string())
    }
}

struct SqliteCapabilityLedgerFactory;

impl CapabilityLedgerFactory for SqliteCapabilityLedgerFactory {
    fn label(&self) -> &'static str {
        "sqlite-capability-ledger"
    }

    fn build(&self) -> Result<Box<dyn CapabilityLedger>, String> {
        let scratch = Scratch::new("ledger");
        let path = scratch.path.clone();
        std::mem::forget(scratch);
        SqliteCapabilityLedger::open(&path)
            .map(|ledger| Box::new(ledger) as Box<dyn CapabilityLedger>)
            .map_err(|error| error.to_string())
    }
}

#[test]
fn sqlite_state_store_passes_the_shared_contract_suite() {
    let findings = state_store_suite(&SqliteStateFactory);
    assert!(
        findings.is_empty(),
        "the state store must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn sqlite_event_log_passes_the_shared_contract_suite() {
    let findings = event_log_suite(&SqliteEventLogFactory);
    assert!(
        findings.is_empty(),
        "the event log must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

#[test]
fn sqlite_capability_ledger_passes_the_shared_contract_suite() {
    let findings = capability_ledger_suite(&SqliteCapabilityLedgerFactory);
    assert!(
        findings.is_empty(),
        "the capability ledger must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

struct SqliteRevisionLedgerFactory;

impl RevisionLedgerFactory for SqliteRevisionLedgerFactory {
    fn label(&self) -> &'static str {
        "sqlite-revision-ledger"
    }

    fn build(&self) -> Result<Box<dyn RevisionLedger>, String> {
        let scratch = Scratch::new("revision-ledger");
        let path = scratch.path.clone();
        std::mem::forget(scratch);
        SqliteRevisionLedger::open(&path)
            .map(|ledger| Box::new(ledger) as Box<dyn RevisionLedger>)
            .map_err(|error| error.to_string())
    }
}

#[test]
fn sqlite_revision_ledger_passes_the_shared_contract_suite() {
    let findings = revision_ledger_suite(&SqliteRevisionLedgerFactory);
    assert!(
        findings.is_empty(),
        "the revision ledger must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}

/// The cross-run durability pin (plan Task 23 charter D3): the derived
/// promoted-but-rolled-back state survives a full ledger REOPEN — a fresh
/// adapter over the same database file reads the same transition-derived
/// decision and the same restore fact. Nothing is cached; nothing is lost.
#[test]
fn the_derived_rolled_back_state_survives_a_ledger_reopen() {
    let scratch = Scratch::new("revision-ledger-reopen");
    let revision = RevisionId::new("rev-reopen-0001").expect("valid revision id");
    let bound = Deadline::at_epoch(EpochSeconds::new(1_700_100_000));
    let evidence =
        |token: &str| gauntlet_domain::EvidenceRef::new(token).expect("valid evidence ref");
    let proposal = gauntlet_ports::revision_ledger::RevisionProposal::new(
        revision.clone(),
        "correction.selection".to_owned(),
        "reopen leg problem".to_owned(),
        Vec::new(),
        "reopen leg improvement".to_owned(),
        Vec::new(),
        evidence("sha256:aaa11111aaa11111"),
        "held-out plan".to_owned(),
        "regression plan".to_owned(),
        evidence("sha256:bbb22222bbb22222"),
        EpochSeconds::new(1_700_000_000),
        None,
        "the reopen leg's admitted proposal".to_owned(),
    );
    {
        let ledger = SqliteRevisionLedger::open(&scratch.path).expect("the ledger opens");
        ledger
            .admit_proposal(proposal, bound)
            .expect("the admission lands");
        let record = RevisionDecisionRecord::new(
            revision.clone(),
            DecisionKind::Pending,
            GateDecision::promoted(RevisionPromotionEvidence {
                metric: "iterations_per_settlement".to_owned(),
                control_figure: 10_000,
                treatment_figure: 5_000,
                margin_bps: 0,
                control_payment_figure: 10_000,
                treatment_payment_figure: 9_000,
                unit_mix_preserved: true,
                instrument_series_non_weaker: true,
                control_reference_digest: "sha256:0011223344556677".to_owned(),
                treatment_reference_digest: "sha256:0011223344556677".to_owned(),
                regression_units_control_settled: 1,
                regression_units_treatment_settled: 1,
                protected_capabilities: Vec::new(),
            }),
            EpochSeconds::new(1_700_000_100),
        )
        .expect("the promotion shape composes");
        ledger
            .apply_decision(record, bound)
            .expect("the promotion lands");
    }
    {
        let ledger = SqliteRevisionLedger::open(&scratch.path).expect("the ledger reopens");
        let entry = ledger
            .revision(&revision, bound)
            .expect("the derived entry reads")
            .expect("the revision survives the reopen");
        assert_eq!(entry.decision, Some(DecisionKind::Promoted));
        assert!(!entry.rolled_back);
        let restore = RestoreRecord::new(
            revision.clone(),
            DecisionKind::Promoted,
            "the reopen leg's restore".to_owned(),
            EpochSeconds::new(1_700_000_200),
        );
        ledger.record_restore(restore, bound).expect("restores");
    }
    // The SECOND reopen: the promoted-but-rolled-back state is durable.
    let ledger = SqliteRevisionLedger::open(&scratch.path).expect("the ledger reopens again");
    let entry = ledger
        .revision(&revision, bound)
        .expect("the derived entry reads")
        .expect("the revision survives the second reopen");
    assert_eq!(entry.decision, Some(DecisionKind::Promoted));
    assert!(
        entry.rolled_back,
        "the restore fact must be derived from the durable restore row after a reopen"
    );
}
