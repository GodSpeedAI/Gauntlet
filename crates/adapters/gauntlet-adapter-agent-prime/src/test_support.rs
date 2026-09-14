//! Unit-test support for this crate's own `#[cfg(test)]` modules — never
//! shipped in the library binary (the ports' stub posture: test machinery
//! is test-gated). The integration test target builds its own doubles.

use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, InvocationId, Limits, MicroUsd,
    PermissionSetRef, ResultContractPath, RoleId, RunId, WorkspaceRef,
};
use gauntlet_ports::agent_runner::{ContextEnvelope, RoleInvocationSpec};
use gauntlet_ports::deadline::Deadline;

/// The base instant the shared suite pins; unit tests share it so the
/// same facts hold on both sides of the port.
pub const BASE_INSTANT: u64 = 1_700_000_000;

/// The canonical dispatch spec a unit test dispatches, mirroring the
/// shared suite's `dispatch_spec` shape.
pub fn dispatch_spec() -> RoleInvocationSpec {
    let ulid = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    let budget = Budget::new(
        Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("limits usable"),
        20,
        DiagnosticAllowance::new(100, MicroUsd::new(50)),
    )
    .expect("budget usable");
    RoleInvocationSpec {
        invocation_id: InvocationId::new(ulid).expect("valid invocation id"),
        run_id: RunId::new(ulid).expect("valid run id"),
        iteration_no: gauntlet_domain::IterationNo::new(1).expect("iterations are 1-based"),
        role: RoleId::Builder,
        prompt: "run the assigned slice and write the typed result".to_owned(),
        context_envelope: ContextEnvelope::new(
            "the runtime-built allowlisted context".to_owned(),
            Digest::new("sha256", "e5f6a7b8c4d3e2f1a0b1c2d3").expect("digest well formed"),
        ),
        workspace: WorkspaceRef::new("ws-suite-01").expect("suite workspace ref"),
        permissions: PermissionSetRef::new("perm-suite-readonly").expect("suite permission ref"),
        result_contract_path: ResultContractPath::new("suite-result-document")
            .expect("suite contract path"),
        deadline: Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + 30)),
        model_hint: None,
        sandbox_policy: None,
        budget_slice: budget,
    }
}

/// Marks a freshly written fixture script executable and waits until the
/// kernel will actually exec it. Tests run on parallel threads: a fork
/// taken by another test while this script's write descriptor was still
/// open inherits that descriptor until its own exec, and an exec of the
/// script inside that window fails with ETXTBSY ("Text file busy"). A
/// no-op spawn is retried until the error is anything but ETXTBSY.
#[cfg(unix)]
pub fn make_executable_ready(script: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    use std::process::{Command, Stdio};
    const ETXTBSY: i32 = 26;
    std::fs::set_permissions(script, std::fs::Permissions::from_mode(0o755))
        .expect("the fixture script is chmodable");
    for _ in 0..200 {
        match Command::new(script)
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status()
        {
            Err(error) if error.raw_os_error() == Some(ETXTBSY) => {
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            _ => return,
        }
    }
    panic!("the fixture script {} stayed busy", script.display());
}
