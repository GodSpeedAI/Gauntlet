//! The codemode tool-program runtime (Phase H slice 2b): the
//! `ToolProgramRuntime` port run by the Bun codemode host.
//!
//! [`CodemodeRuntime`] spawns the host once per run (`bun run codemode-host`
//! in the workbench, by default) with a minimal environment and its own
//! process group, sends the admitted tools and the limits, and answers each
//! `call` through the port's `ProgramHost`. The trace always comes from that
//! host. A run that exceeds its timeout plus a grace is cancelled, its group
//! is killed, and it is reaped.
//!
//! The host protocol is normative in `workbench/src/codemode/PROTOCOL.md`.
//! This crate depends only on the domain and ports; the composition root
//! wires it.

mod probe;
mod process;
mod protocol;
mod runtime;

pub use probe::{
    CodemodeProbe, DEFAULT_WORKBENCH_DIR, HOST_ENTRY, ProbeCheck, bun_codemode_host,
    probe_codemode_in, probe_host,
};
pub use process::HostCommand;
pub use runtime::{CodemodeRuntime, DEFAULT_GRACE, DEFAULT_PROGRAM_TIMEOUT};
