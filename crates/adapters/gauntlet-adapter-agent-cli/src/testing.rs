//! The shared-suite factory and its script-following process doubles.
//!
//! The doubles are what the suite stands in for — the CHILD, never the
//! adapter's logic (spec §6.3): [`ScriptedProcessTransport`] maps a
//! [`RunnerScript`](gauntlet_ports::contract::agent_runner::RunnerScript)
//! to in-memory child behavior, and the child's result document is built
//! by PARsing the REAL prompt file the runner wrote (the same grammar the
//! live probe child reads), so the suite exercises the real prompt
//! format, the real anchored files, and the real strict contract codec.
//! The default battery spawns NO real process (R-1b).
//!
//! [`CliRunnerFactory`] builds an ISOLATED runner per case with a fresh
//! scratch state root, a virtual stepping clock (RES-D9: the runner's
//! only time source is the injected `Clock`), and its OWN command
//! template literal (RES-D10: never the test host's
//! `GAUNTLET_CLI_AGENT_CMD` — the battery must not be
//! environment-sensitive).

use core::cell::{Cell, RefCell};
use core::fmt;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;

use gauntlet_domain::EpochSeconds;
use gauntlet_ports::clock::Clock;
use gauntlet_ports::contract::agent_runner::{AgentRunnerFactory, RunnerScript};
use gauntlet_ports::deadline::Deadline;

use crate::config::CliAgentConfig;
use crate::runner::CliAgentRunner;
use crate::transport::{ChildId, ProcessSpec, ProcessState, ProcessTransport, TransportError};

/// The suite's command template (RES-D10: a constructor-injected literal,
/// never host environment). `cli-probe` is a fake program name; the
/// double never executes it. The word order is fixed and known to the
/// double: prompt file at argv[1], result path at argv[3].
pub const SUITE_TEMPLATE: &str = "cli-probe {prompt_file} {workspace} {result_path}";

/// The virtual clock's base instant — the suite's pinned base.
pub const BASE_INSTANT: u64 = 1_700_000_000;

/// Virtual seconds per `now()` call: invoke reads the clock once
/// (admission), each poll once, so the suite's deadline (BASE+30) fires
/// within the 24-probe ceiling — deterministic TimedOut with no waiting.
const CLOCK_STEP_SECONDS: u64 = 2;

/// The virtual clock: one step per `now()` (prime's suite-clock posture).
#[derive(Debug)]
pub struct SteppingClock {
    current: Cell<u64>,
}

impl SteppingClock {
    pub fn new() -> Self {
        Self {
            current: Cell::new(0),
        }
    }
}

impl Default for SteppingClock {
    fn default() -> Self {
        Self::new()
    }
}

impl Clock for SteppingClock {
    fn now(&self) -> EpochSeconds {
        let next = self.current.get() + CLOCK_STEP_SECONDS;
        self.current.set(next);
        EpochSeconds::new(BASE_INSTANT + next)
    }

    fn sleep_until(&self, at: EpochSeconds) -> Result<(), gauntlet_ports::clock::ClockError> {
        self.current.set(
            at.get()
                .saturating_sub(BASE_INSTANT)
                .max(self.current.get()),
        );
        Ok(())
    }
}

/// The dispatch facts the double parses out of the REAL prompt file.
#[derive(Debug, Clone)]
struct PromptFacts {
    invocation_id: String,
    run_id: String,
    iteration_no: u64,
    role: String,
    envelope_digest: String,
    result_path: PathBuf,
}

fn parse_prompt_file(path: &Path) -> Result<PromptFacts, String> {
    let text = std::fs::read_to_string(path).map_err(|error| {
        format!(
            "the double could not read the prompt file {}: {error}",
            path.display()
        )
    })?;
    let mut fields = BTreeMap::new();
    for line in text.lines() {
        if line == "BODY-BEGIN" {
            break;
        }
        if line.starts_with('#') || line.is_empty() {
            continue;
        }
        let Some((key, value)) = line.split_once(": ") else {
            return Err(format!("unparseable prompt field line {line:?}"));
        };
        fields.insert(key.to_owned(), value.to_owned());
    }
    let field = |key: &str| {
        fields
            .get(key)
            .cloned()
            .ok_or_else(|| format!("the prompt file is missing {key}"))
    };
    Ok(PromptFacts {
        invocation_id: field("INVOCATION-ID")?,
        run_id: field("RUN-ID")?,
        iteration_no: field("ITERATION-NO")?
            .parse::<u64>()
            .map_err(|e| e.to_string())?,
        role: field("ROLE")?,
        envelope_digest: field("ENVELOPE-DIGEST")?,
        result_path: PathBuf::from(field("RESULT-PATH")?),
    })
}

/// A minimal VALID `role_result@v1` document echoing the dispatch facts
/// the child parsed from the prompt file (§10.3 rule 6: the identity
/// echo is the contract's job).
fn valid_document(facts: &PromptFacts) -> String {
    format!(
        "{{\"schema\":\"role_result@v1\",\"invocation_id\":\"{inv}\",\
         \"run_id\":\"{run}\",\"iteration_no\":{it},\"role\":\"{role}\",\
         \"context_envelope_digest\":\"{digest}\",\"state\":\"completed\",\
         \"payload_schema\":\"builder_result@v1\",\
         \"payload\":{{\"attempted\":[],\"changed_paths\":[],\"commands_run\":[],\
         \"artifacts\":[],\"claims\":[],\"self_checks\":[],\"blocked_on\":[]}},\
         \"payment\":{{\"input_tokens\":0,\"output_tokens\":0,\"tool_calls\":0,\
         \"wall_seconds\":0,\"cost_usd\":0}},\"unavailable_evidence\":[],\
         \"errors\":[]}}",
        inv = facts.invocation_id,
        run = facts.run_id,
        it = facts.iteration_no,
        role = facts.role,
        digest = facts.envelope_digest,
    )
}

/// The (possibly derailed) document a script's child writes, or `None`
/// when it never does.
fn document_for(script: RunnerScript, facts: &PromptFacts) -> Option<String> {
    use RunnerScript::*;
    let valid = || valid_document(facts);
    match script {
        CompleteAfterProbes { .. } | CompleteImmediately => Some(valid()),
        NeverWriteResult | ExitWithoutResult => None,
        WriteMalformedResult => Some("this is not json {{{".to_owned()),
        WriteUnknownEnvelopeField => {
            Some(valid().replace("\"errors\":[]", "\"errors\":[],\"fabricated\":true"))
        }
        WriteUnknownNestedField => Some(valid().replace(
            "\"self_checks\":[]",
            "\"self_checks\":[{\"command\":\"x\",\"exit_code\":0,\"fabricated\":true}]",
        )),
        WriteForeignInvocationId => {
            Some(valid().replace(&facts.invocation_id, "01ARZ3NDEKTSV4RRFFQ69G5FAB"))
        }
        WriteForeignEnvelopeDigest => {
            Some(valid().replace(&facts.envelope_digest, "sha256:a1b2c3d4e5f6a7b8"))
        }
    }
}

/// Whether the script's child writes its document AT SPAWN time (and is
/// born already exited).
fn writes_at_spawn(script: RunnerScript) -> bool {
    use RunnerScript::*;
    matches!(
        script,
        CompleteImmediately
            | WriteMalformedResult
            | WriteUnknownEnvelopeField
            | WriteUnknownNestedField
            | WriteForeignInvocationId
            | WriteForeignEnvelopeDigest
    )
}

struct InnerDouble {
    script: RunnerScript,
    /// The exit code a dying child reports (a test may poison it to prove
    /// the exit status is never obeyed).
    exit_code: Cell<i32>,
    children: RefCell<BTreeMap<u64, ChildDouble>>,
    next_child: Cell<u64>,
    spawned: RefCell<Vec<Vec<String>>>,
    stopped: RefCell<Vec<u64>>,
}

struct ChildDouble {
    state_calls: u32,
    wrote: bool,
    facts: PromptFacts,
    alive: bool,
    exit_code: i32,
}

impl fmt::Debug for InnerDouble {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("InnerDouble")
            .field("script", &self.script)
            .finish()
    }
}

impl InnerDouble {
    fn write_document(&self, child: &mut ChildDouble) {
        if child.wrote {
            return;
        }
        if let Some(document) = document_for(self.script, &child.facts) {
            if let Some(parent) = child.facts.result_path.parent() {
                std::fs::create_dir_all(parent).expect("the inv dir is creatable");
            }
            std::fs::write(&child.facts.result_path, document)
                .expect("the result document is writable");
        }
        child.wrote = true;
    }

    /// Whether the child, probed NOW, is already exited — and writes its
    /// document first if the script says so (write-then-exit, the real
    /// child's ordering).
    fn probe(&self, child: &mut ChildDouble) -> ProcessState {
        child.state_calls += 1;
        use RunnerScript::*;
        match self.script {
            // Born exited; the document (if any) landed at spawn.
            CompleteImmediately
            | WriteMalformedResult
            | WriteUnknownEnvelopeField
            | WriteUnknownNestedField
            | WriteForeignInvocationId
            | WriteForeignEnvelopeDigest => ProcessState::Exited {
                code: Some(child.exit_code),
            },
            // Writes after `probes_before_result` probes, then exits.
            CompleteAfterProbes {
                probes_before_result,
            } => {
                if child.state_calls > u32::from(probes_before_result) {
                    self.write_document(child);
                    ProcessState::Exited {
                        code: Some(child.exit_code),
                    }
                } else {
                    ProcessState::Running
                }
            }
            // Silent forever: the deadline decides (shape 1).
            NeverWriteResult => ProcessState::Running,
            // Exited from the first probe, never writes (shape 2).
            ExitWithoutResult => ProcessState::Exited {
                code: Some(child.exit_code),
            },
        }
    }
}

/// The script-following in-memory process double. `Rc`-shared on purpose:
/// the suite is single-threaded (the mock runner's own posture), and the
/// test/factory keeps a handle to observe spawned argv.
#[derive(Debug, Clone)]
pub struct ScriptedProcessTransport {
    inner: Rc<InnerDouble>,
}

impl ScriptedProcessTransport {
    pub fn new(script: RunnerScript) -> Self {
        Self {
            inner: Rc::new(InnerDouble {
                script,
                exit_code: Cell::new(0),
                children: RefCell::new(BTreeMap::new()),
                next_child: Cell::new(1),
                spawned: RefCell::new(Vec::new()),
                stopped: RefCell::new(Vec::new()),
            }),
        }
    }

    /// Poisons the exit code a dying child reports — the falsifier's
    /// proof that the adapter never obeys exit status.
    pub fn set_exit_code(&self, code: i32) {
        self.inner.exit_code.set(code);
    }

    /// The argv the runner rendered for each spawn, in spawn order — the
    /// template-resistance evidence surface.
    pub fn spawned_argv(&self) -> Vec<Vec<String>> {
        self.inner.spawned.borrow().clone()
    }

    /// The children a cancel/stop actually reached.
    pub fn stopped_children(&self) -> Vec<u64> {
        self.inner.stopped.borrow().clone()
    }
}

impl ProcessTransport for ScriptedProcessTransport {
    fn spawn(&self, spec: &ProcessSpec) -> Result<ChildId, TransportError> {
        self.inner.spawned.borrow_mut().push(spec.argv.clone());
        // SUITE_TEMPLATE's word order: prompt file at argv[1], result
        // path at argv[3]. The double parses the REAL prompt file.
        let facts = parse_prompt_file(Path::new(&spec.argv[1])).map_err(|detail| {
            TransportError::SpawnFailed {
                program: spec.argv.first().cloned().unwrap_or_default(),
                detail,
            }
        })?;
        let id = self.inner.next_child.get();
        self.inner.next_child.set(id + 1);
        let exit_code = self.inner.exit_code.get();
        let mut child = ChildDouble {
            state_calls: 0,
            wrote: false,
            facts,
            alive: true,
            exit_code,
        };
        // Write-at-spawn scripts: the document lands BEFORE the first
        // probe (a born-exited child).
        if writes_at_spawn(self.inner.script) {
            self.inner.write_document(&mut child);
        }
        self.inner.children.borrow_mut().insert(id, child);
        Ok(ChildId::new(id))
    }

    fn state(&self, child: ChildId) -> Result<ProcessState, TransportError> {
        let mut children = self.inner.children.borrow_mut();
        let Some(double) = children.get_mut(&child.raw()) else {
            return Err(TransportError::UnknownChild { child });
        };
        if !double.alive {
            return Ok(ProcessState::Exited {
                code: Some(double.exit_code),
            });
        }
        Ok(self.inner.probe(double))
    }

    fn stop(&self, child: ChildId, _deadline: Deadline) -> Result<Option<i32>, TransportError> {
        let mut children = self.inner.children.borrow_mut();
        let Some(double) = children.get_mut(&child.raw()) else {
            return Err(TransportError::UnknownChild { child });
        };
        self.inner.stopped.borrow_mut().push(child.raw());
        double.alive = false;
        // A stopped double reports the scripted exit code (unknown = None
        // is equally valid; the runner must not care).
        Ok(Some(double.exit_code))
    }
}

/// The suite-facing factory: builds an ISOLATED runner per case (spec
/// §7.3) over the doubles — no real process, no host environment
/// (RES-D10).
#[derive(Debug, Default)]
pub struct CliRunnerFactory;

impl CliRunnerFactory {
    fn scratch(tag: &str) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static SEQUENCE: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "gauntlet-cli-adapter-suite-{tag}-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::remove_dir_all(&dir).ok();
        std::fs::create_dir_all(&dir).expect("the suite scratch is creatable");
        dir
    }
}

impl AgentRunnerFactory for CliRunnerFactory {
    fn label(&self) -> &'static str {
        "cli-agent-adapter"
    }

    // The doubles are single-threaded by design (the mock runner's own
    // posture: one runner replays one child, driven from one thread), so
    // the Arc handoff is knowingly non-Send/Sync (prime's suite-file
    // allow, applied at the one shipped site).
    #[allow(clippy::arc_with_non_send_sync)]
    fn build(
        &self,
        script: RunnerScript,
    ) -> Result<Box<dyn gauntlet_ports::agent_runner::AgentRunner>, String> {
        let runner = CliAgentRunner::new(
            CliAgentConfig::new(SUITE_TEMPLATE),
            Self::scratch("state"),
            Self::scratch("ws"),
            ScriptedProcessTransport::new(script),
            Arc::new(SteppingClock::new()),
        )
        .map_err(|error| error.to_string())?;
        Ok(Box::new(runner))
    }
}
