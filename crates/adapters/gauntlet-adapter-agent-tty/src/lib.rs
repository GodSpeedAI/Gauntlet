//! The interactive TTY agent adapter (spec §4.1 `tty-agent`; plan Task 17):
//! a REAL `AgentRunner` driven by a single-threaded interactive transport.
//!
//! The adapter serves REPL-like agents that communicate via stdin/stdout/stderr
//! in a streaming fashion, with liveness signals conveyed through lifecycle
//! state observation (Working, Blocked, Idle, Done) rather than process exit.
//!
//! ## Shape
//!
//! - [`template`] renders the configured command template into argv by
//!   shell-word PARSING (never shell interpolation); unknown placeholders
//!   or missing values are typed configuration errors — fail-closed at
//!   admission, never a silently unrendered literal.
//! - [`transport`] is the process seam: the production
//!   [`InteractiveChildTransport`] manages sessions with the ability to
//!   send input and read output; the shared contract suite binds
//!   in-memory doubles, so the default battery spawns NO real process (R-1b).
//! - [`lifecycle`] classifies tail output to detect Idle (prompt-suffix
//!   match), Blocked (marker substring), Working (visible output), or
//!   Unknown. It assigns no sequence: each reading leaves through
//!   `AgentRunner::lifecycle_samples` for the Gauntlet supervisor to stamp.
//! - [`runner`] is the real [`AgentRunner`](gauntlet_ports::AgentRunner).
//!   Classification uses lifecycle state for liveness inference, but NEVER
//!   treats it as completion evidence. The result contract is the ONE
//!   completion fact — valid matching document ⇒ `Completed`; no contract
//!   before deadline ⇒ `TimedOut`; malformed ⇒ `Failed{malformed_result}`.
//! - [`testing`] (pub, for tests): `ScriptedInteractiveTransport` that
//!   replays a script and can write the result contract file at a scripted moment.
//!
//! ## Single-threaded design
//!
//! The transport and runner are single-threaded (not `Send`/`Sync`). Sessions
//! are identified by opaque handles and stored in a RefCell map. The runner
//! polls session state and reads the result contract, never relying on
//! lifecycle observation for completion verdicts.

pub mod clock;
pub mod config;
pub mod lifecycle;
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
pub const RUNNER_TOKEN: &str = "tty-agent";

/// The environment variable carrying the command template (spec §4.1).
pub const COMMAND_ENV_VAR: &str = "GAUNTLET_TTY_AGENT_CMD";

/// The environment channel to the child (spec §10.3 rule 1; RES-D3): the
/// ONLY variable this adapter adds to the child's otherwise-verbatim
/// inherited environment — the exact result-contract path (rule 2).
pub const RESULT_PATH_ENV_VAR: &str = "GAUNTLET_RESULT_PATH";

/// The prompt file format identifier (`prompt.rs` documents the grammar).
pub const PROMPT_FORMAT: &str = "gauntlet-tty-agent-prompt@1";

/// The shell the runner uses to execute a WRAPPED command (plan Task 25):
/// a [`SandboxedCommand`](gauntlet_ports::sandbox::SandboxedCommand) is by
/// definition ONE shell-invocable string whose FIRST word is the sandbox
/// binary (spec §17.5.4, AK-048), so executing it through `sh -c` starts
/// the sandbox as the first real process (a POSIX shell replaces itself
/// with a single simple command).
pub const WRAPPED_SHELL: &str = "sh";

pub use config::{ConfigError, InteractiveAgentConfig, PromptDelivery};
pub use runner::{
    CommandContainment, ContainmentRefusal, InteractiveAgentRunner, InteractiveForensics,
};
pub use template::{
    PLACEHOLDER_MODEL, PLACEHOLDER_PROMPT_FILE, PLACEHOLDER_RESULT_PATH, PLACEHOLDER_WORKSPACE,
    Template, TemplateError, TemplateValues, parse_template, probe_template, render,
};
pub use transport::{
    InteractiveChildId, InteractiveChildTransport, InteractiveProcessSpec, InteractiveProcessState,
    InteractiveTransport, TailSnapshot, TransportError,
};

/// Capability-named façade for composition roots (spec §4.1/§8.4: the
/// runtime names capabilities, never products — the same posture as the
/// process-isolation backend's `ProcessIsolationBackend` re-export).
pub use InteractiveAgentRunner as CommandTemplateRunner;
