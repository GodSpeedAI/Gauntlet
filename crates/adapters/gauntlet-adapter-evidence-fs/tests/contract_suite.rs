//! The shared `EvidenceStore` contract suite (spec §7.3) run against this
//! adapter — "Adding an adapter means implementing the trait and running the
//! existing suite, not writing a bespoke one." The suite lives in
//! `gauntlet-ports::contract::evidence_store`; this file only supplies the
//! factory the Task 4 mechanism parameterizes over and asserts the findings
//! are empty.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use gauntlet_adapter_evidence_fs::FsEvidenceStore;
use gauntlet_ports::contract::evidence_store::{EvidenceStoreFactory, evidence_store_suite};
use gauntlet_ports::evidence_store::EvidenceStore;

/// Isolated scratch store per suite case; removed on drop. `tempfile` is not
/// on any layer's external allowlist, and a test helper is not a reason to
/// widen one.
struct Scratch {
    path: PathBuf,
}

static SCRATCH_SEQ: AtomicU32 = AtomicU32::new(0);

impl Scratch {
    fn new(case: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "gauntlet-evidence-fs-suite-{case}-{}-{}",
            std::process::id(),
            SCRATCH_SEQ.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::remove_dir_all(&path).ok();
        std::fs::create_dir_all(&path).expect("suite scratch directory is creatable");
        Self { path }
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.path).ok();
    }
}

struct FsEvidenceFactory;

impl EvidenceStoreFactory for FsEvidenceFactory {
    fn label(&self) -> &'static str {
        "fs-evidence-store"
    }

    fn build(&self) -> Result<Box<dyn EvidenceStore>, String> {
        let scratch = Scratch::new("build");
        let path = scratch.path.clone();
        // Leak the directory: the store under test owns it for the case's
        // lifetime, and the suite process exits after.
        std::mem::forget(scratch);
        FsEvidenceStore::open(&path)
            .map(|store| Box::new(store) as Box<dyn EvidenceStore>)
            .map_err(|error| error.to_string())
    }
}

#[test]
fn fs_evidence_store_passes_the_shared_contract_suite() {
    let findings = evidence_store_suite(&FsEvidenceFactory);
    assert!(
        findings.is_empty(),
        "the fs evidence store must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}
