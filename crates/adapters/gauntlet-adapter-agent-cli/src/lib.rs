//! The generic CLI agent adapter (spec §4.1 `cli-agent`; plan Task 16):
//! a REAL `AgentRunner` driven entirely by a configured command template —
//! any headless coding agent invoked as a subprocess with a prompt and a
//! working directory. No vendor name appears anywhere in this crate:
//! selection is configuration (`GAUNTLET_CLI_AGENT_CMD`), not code (AK-013,
//! AK-014; the arch suite whole-crate-scans this crate for vendor terms).
//!
//! ## Shape
//!
//! - [`template`] renders the configured command template into argv by
//!   shell-word PARSING (never shell interpolation); unknown placeholders
//!   or missing values are typed configuration errors — fail-closed at
//!   admission, never a silently unrendered literal (RES-D8).
//! - [`transport`] is the process seam: the production
//!   [`ProcessChildTransport`](transport::ProcessChildTransport) spawns
//!   real children (`std::process`); the shared contract suite binds
//!   in-memory doubles, so the default battery spawns NO real process
//!   (R-1b). The child inherits the parent environment plus exactly
//!   `GAUNTLET_RESULT_PATH` (RES-D3); credential pruning is the gate/
//!   sandbox composition's business (Task 20), recorded not done here.
//! - [`prompt`] is the machine-parseable prompt file (format
//!   `gauntlet-cli-agent-prompt@1`), anchored inside the invocation's
//!   state directory — never the workspace source tree — and referencing
//!   the context-envelope sibling file (RES-D2).
//! - [`runner`] is the real [`AgentRunner`](gauntlet_ports::AgentRunner).
//!   Classification is a TOTAL ORDER (RES-D8a): the result contract is
//!   the ONE completion fact (§10.3) — a valid matching document is
//!   `Completed` even on a nonzero exit (the exit status is recorded in
//!   adapter-local forensics, never obeyed); a malformed document is
//!   `Failed{malformed_result}`; a mismatched identity is
//!   `Failed{contract_mismatch}`; an EXITED child with no valid document
//!   is `Failed{missing_result}` — decided BEFORE the deadline, because a
//!   dead child can never write (signal-death and nonzero exit collapse
//!   to the same verdict on purpose); only a still-RUNNING silent child
//!   reaches the deadline verdict `TimedOut` (rule 5); otherwise
//!   `Running`. Process liveness is never completion evidence.
//! - [`runner::CommandContainment`] is the containment wrap seam (plan
//!   Task 25, R-2 posture re-adjudication recorded for the operator):
//!   when the composition root installs it (exactly when the posture
//!   fronts a LIVE sandbox backend), a dispatch carrying an admitted
//!   sandbox policy spawns the gate's WRAPPED command — the sandbox is
//!   the child's first process (AK-048) — and the runner honestly
//!   reports `Isolation::Process`. A policy-carrying dispatch with no
//!   seam installed is a typed admission refusal, never an unwrapped
//!   run (§21.3, AK-047) — teeth:
//!   `fr1_a_policy_carrying_dispatch_without_the_seam_is_a_typed_refusal_that_spawns_nothing`.
//! - [`config`] is the typed configuration; there is NO default command
//!   template — a missing `GAUNTLET_CLI_AGENT_CMD` is a typed error.
//! - [`clock`] is the production wall-clock binding; the runner always
//!   takes the INJECTED [`Clock`](gauntlet_ports::Clock) port (RES-D9).
//!
//! The result contract is read by the runner's `poll` — the initial read
//! plus the final re-read an observed exit triggers, which gives the
//! child its fair last chance. `collect` never re-reads the file: it
//! serves the VALIDATED snapshot `poll` classified, so tampering with
//! the file after completion cannot change what the runtime collected
//! (executed observation: the falsifier's post-completion tamper leg).
//!
//! The shared `AgentRunner` contract suite runs against this adapter
//! UNMODIFIED through script-following in-memory doubles (spec §6.3,
//! §7.3); the process-specific never-write shape 2 is pinned by the
//! ports' `exit_without_result_suite`, which this adapter and the mock
//! runner both pass (P8/AK-013: one suite, two adapters, zero per-adapter
//! test bodies). Process-lifecycle obligations (RES-D12): a forensic
//! handle record (pid + invocation identity) is written at admission for
//! §18.3 orphan discovery; the invocation directory and prompt file are
//! RETAINED after terminal states; cancel is SIGTERM, a bounded wait,
//! then SIGKILL; reaping is non-blocking (`try_wait` in poll).

pub mod clock;
pub mod config;
pub mod prompt;
pub mod runner;
pub mod template;
pub mod transport;

#[doc(hidden)]
pub mod testing;

/// The runner token this adapter's composition fronts (§8.2 resolves
/// `runners.default` by name; the composition root compares the resolved
/// token against this constant — the literal lives here, under
/// `crates/adapters/**`, and nowhere else).
pub const RUNNER_TOKEN: &str = "cli-agent";

/// The environment variable carrying the command template (spec §4.1).
pub const COMMAND_ENV_VAR: &str = "GAUNTLET_CLI_AGENT_CMD";

/// The environment channel to the child (spec §10.3 rule 1; RES-D3): the
/// ONLY variable this adapter adds to the child's otherwise-verbatim
/// inherited environment — the exact result-contract path (rule 2).
pub const RESULT_PATH_ENV_VAR: &str = "GAUNTLET_RESULT_PATH";

/// The prompt file format identifier (`prompt.rs` documents the grammar).
pub const PROMPT_FORMAT: &str = "gauntlet-cli-agent-prompt@1";

/// The shell the runner uses to execute a WRAPPED command (plan Task 25):
/// a [`SandboxedCommand`](gauntlet_ports::sandbox::SandboxedCommand) is by
/// definition ONE shell-invocable string whose FIRST word is the sandbox
/// binary (spec §17.5.4, AK-048), so executing it through `sh -c` starts
/// the sandbox as the first real process (a POSIX shell replaces itself
/// with a single simple command).
pub const WRAPPED_SHELL: &str = "sh";

pub use config::{CliAgentConfig, ConfigError};
pub use runner::{CliAgentRunner, CliForensics, CommandContainment, ContainmentRefusal};
pub use template::{
    PLACEHOLDER_MODEL, PLACEHOLDER_PROMPT_FILE, PLACEHOLDER_RESULT_PATH, PLACEHOLDER_WORKSPACE,
    Template, TemplateError, TemplateValues, parse_template, probe_template, render,
};
pub use transport::{
    ChildId, ProcessChildTransport, ProcessSpec, ProcessState, ProcessTransport, TransportError,
};

/// Capability-named façade for composition roots (spec §4.1/§8.4: the
/// runtime names capabilities, never products — the same posture as the
/// process-isolation backend's `ProcessIsolationBackend` re-export).
pub use CliAgentRunner as CommandTemplateRunner;
