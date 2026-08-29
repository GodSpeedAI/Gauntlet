//! Prime-agent harness adapter (spec §4.1, Annex C; plan Task 15): a real
//! `AgentRunner` delegating through the harness's recursive sub-agent
//! mechanism. Delegation returns an admission handle only — results arrive
//! via the result contract (spec §10.3); nothing in this crate may await an
//! answer from the delegation call.
//!
//! ## Shape
//!
//! - [`program`] renders the Annex C.1 kernel program (byte-pinned) and
//!   the child's result-contract instruction.
//! - [`transport`] holds the two harness seams: a
//!   [`KernelTransport`](transport::KernelTransport) whose return type
//!   makes a child answer unrepresentable, and a
//!   [`HarnessControl`](transport::HarnessControl) for stop/list — the
//!   production implementations shell out to the configured binary via
//!   `std::process` only.
//! - [`runner`] is the real [`AgentRunner`](gauntlet_ports::AgentRunner):
//!   state transitions derive from the declared result contract and the
//!   spec's deadline, never from liveness or a mailbox (Annex C.2/C.4).
//! - [`probe`] guards the harness API surface behind a version probe
//!   whose supported range is this crate's declared constant.
//! - [`config`] is the typed configuration; no absolute path or model id
//!   is a literal anywhere in this crate.
//! - [`clock`] is the production wall-clock [`Clock`](gauntlet_ports::Clock)
//!   binding; the runner itself always takes the port.
//!
//! The result contract is read in exactly two places — the runner's
//! `poll` and `collect`. The repository's arch suite pins that with an
//! await-ban source scan carrying its own negative control.
//!
//! The shared `AgentRunner` contract suite runs against this adapter
//! UNMODIFIED through in-memory doubles that execute the rendered
//! program's semantics (spec §6.3, §7.3): see this crate's
//! `tests/contract_suite.rs`.

pub mod clock;
pub mod config;
pub mod probe;
pub mod program;
pub mod runner;
pub mod transport;

#[cfg(test)]
pub(crate) mod test_support;

pub use clock::SystemClock;
pub use config::PrimeAdapterConfig;
pub use probe::{ProbeError, ProbeReport, VersionRange, is_supported, probe};
pub use runner::{InvocationForensics, PrimeAgentRunner};
pub use transport::{
    AdmissionHandle, ControlError, HarnessControl, KernelSubmission, KernelTransport,
    ProcessHarnessControl, ProcessKernelTransport, SubagentRecord, TransportError, parse_listing,
    read_admission,
};

/// The runner token this adapter's composition fronts (§8.2 resolves
/// `runners.default` by name; the composition root compares the resolved
/// token against this constant — the literal lives here, under
/// `crates/adapters/**`, and nowhere else).
pub const RUNNER_TOKEN: &str = "prime-agent";

/// Capability-named façade for composition roots (spec §4.1/§8.4: the
/// runtime names capabilities, never products — the same posture as the
/// process-isolation backend's `ProcessIsolationBackend` re-export).
pub use PrimeAgentRunner as RecursiveSubagentRunner;
