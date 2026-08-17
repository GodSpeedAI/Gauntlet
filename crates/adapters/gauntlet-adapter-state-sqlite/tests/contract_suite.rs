//! The shared contract suites (spec §7.3) run against this adapter — both
//! ports it implements. "Adding an adapter means implementing the trait and
//! running the existing suite, not writing a bespoke one": the suites live
//! in `gauntlet-ports::contract::{state_store,event_log}`, and this file
//! only supplies the factories the Task 4 mechanism parameterizes over.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use gauntlet_adapter_state_sqlite::{SqliteEventLog, SqliteStateStore};
use gauntlet_ports::contract::event_log::{EventLogFactory, event_log_suite};
use gauntlet_ports::contract::state_store::{StateStoreFactory, state_store_suite};
use gauntlet_ports::event_log::EventLog;
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
