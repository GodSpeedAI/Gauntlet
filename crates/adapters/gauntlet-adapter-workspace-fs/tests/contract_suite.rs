//! The shared `WorkspaceProvider` contract suite (spec §6.2; §10.4, §17.5)
//! run against this adapter — "Adding an adapter means implementing the
//! trait and running the existing suite, not writing a bespoke one." The
//! suite lives in `gauntlet-ports::contract::workspace_provider`; this file
//! only supplies the factory the suite parameterizes over and asserts the
//! findings are empty.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU32, Ordering};

use gauntlet_adapter_workspace_fs::FsWorkspaceProvider;
use gauntlet_ports::contract::workspace_provider::{
    WorkspaceProviderFactory, workspace_provider_suite,
};
use gauntlet_ports::workspace_provider::WorkspaceProvider;

/// Isolated scratch root per suite case; removed on drop. `tempfile` is not
/// on any layer's external allowlist, and a test helper is not a reason to
/// widen one.
struct Scratch {
    path: PathBuf,
}

static SCRATCH_SEQ: AtomicU32 = AtomicU32::new(0);

impl Scratch {
    fn new(case: &str) -> Self {
        let path = std::env::temp_dir().join(format!(
            "gauntlet-workspace-fs-suite-{case}-{}-{}",
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

/// One factory, ONE root: the suite allocates through `build` and checks
/// containment against `root` — both must name the same directory.
struct FsWorkspaceFactory {
    root: PathBuf,
}

impl WorkspaceProviderFactory for FsWorkspaceFactory {
    fn label(&self) -> &'static str {
        "fs-workspace-provider"
    }

    fn root(&self) -> PathBuf {
        self.root.clone()
    }

    fn build(&self) -> Result<Box<dyn WorkspaceProvider>, String> {
        FsWorkspaceProvider::open(&self.root)
            .map(|provider| Box::new(provider) as Box<dyn WorkspaceProvider>)
            .map_err(|error| error.to_string())
    }
}

#[test]
fn fs_workspace_provider_passes_the_shared_contract_suite() {
    // Held for the suite's lifetime (dropped after): each suite case builds
    // its own ISOLATED provider instance over this one root.
    let scratch = Scratch::new("suite");
    let factory = FsWorkspaceFactory {
        root: scratch.path.clone(),
    };
    let findings = workspace_provider_suite(&factory);
    assert!(
        findings.is_empty(),
        "the fs workspace provider must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}
