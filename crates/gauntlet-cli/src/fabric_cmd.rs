//! `gauntlet fabric admit-root` and `gauntlet fabric serve` (F2b2): the
//! root admission and the per-caller host process of the agent fabric.
//!
//! - `admit-root` admits a ROOT invocation (parent = None) into the
//!   durable invocation graph and records its unit dispatch in the run's
//!   event log. It prints `invocation_id=<id>` and is idempotent on replay:
//!   the same run and idempotency key derive the same id.
//! - `serve` is one host process for one caller. The caller is fixed by
//!   `--caller` at process start and is never model-supplied. The caller's
//!   node, unit, and authority facts are loaded from durable state before the
//!   first byte is read. The fabric is then served as MCP-subset JSON-RPC over
//!   stdin and stdout. Stdout carries protocol frames only. Diagnostics go to
//!   stderr.
//!
//! Unit facts (blast radius and authority) come from the frozen answer key
//! passed with `--answer-key`, exactly as the run composition reads them. The
//! unit a root was dispatched for is read back from the run's
//! `UnitDispatch` events. Children inherit the root's unit through the graph.
//! No tool argument can name a caller, a parent, or an authority.

use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::process::ExitCode;
use std::str::FromStr;
use std::sync::Arc;

use gauntlet_adapter_agent_prime::SystemClock;
use gauntlet_adapter_state_postgres::{EmbeddedInvocationGraph, EmbeddedStateStore, state_exists};
use gauntlet_adapter_surface_rpc::{ServeEnd, ServeLimits, ServerInfo, serve};
use gauntlet_adapter_workspace_fs::FsWorkspaceProvider;
use gauntlet_app::delegation::{
    AgentsCaller, AgentsPolicy, AgentsService, AgentsSurface, CallClock, ContextItem,
    ContextItemKind, DelegationBroker, RESULT_CONTRACT_NAME, build_envelope, derive_invocation_id,
};
use gauntlet_app::govern::{AuthorityGate, GovernDeps};
use gauntlet_app::reference::parse::{ParsedKey, parse_answer_key};
use gauntlet_domain::{
    AdmissionKey, EpochSeconds, InvocationId, ResultContractPath, RoleId, RunId, RunnerProfileId,
    UnitId, WorkspaceRef,
};
use gauntlet_ports::clock::Clock;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::invocation_graph::{AdmissionOutcome, AdmissionRequest, InvocationGraph};
use gauntlet_ports::json::JsonValue;
use gauntlet_ports::state_store::{RunEvent, RunEventKind, StateStore};
use gauntlet_ports::tool_surface::{ToolListing, ToolOutcome, ToolSurface};
use gauntlet_ports::workspace_provider::{WorkspaceProvider, WorkspaceSubject};

use crate::compose::STATE_DB_NAME;
use crate::fabric_compose::{
    ComposedFabric, FabricDeps, SearchPathLookup, compose_fabric, load_fabric,
};

/// The custom harness definitions a fabric directory may carry.
const CUSTOM_HARNESS_DIR: &str = "harnesses";
/// The bound, in seconds, of one admission or one tool call.
const CALL_BOUND_SPAN: u64 = 60;
/// The idempotency key a root uses when none is given: one root per unit.
const ROOT_KEY_PREFIX: &str = "root-";
const SERVER_NAME: &str = "gauntlet-fabric";
const SERVER_VERSION: &str = env!("CARGO_PKG_VERSION");

const ADMIT_FLAGS: &[&str] = &[
    "state",
    "run",
    "fabric-dir",
    "role",
    "profile",
    "unit",
    "workspace",
    "answer-key",
    "idempotency-key",
];
const SERVE_FLAGS: &[&str] = &[
    "state",
    "run",
    "caller",
    "fabric-dir",
    "workspace",
    "answer-key",
];
const USAGE_ADMIT: &str = "gauntlet fabric admit-root --state <dir> --run <run-id> \
     --fabric-dir <dir> --role <role> --profile <profile-id> --unit <unit> \
     --workspace <path> --answer-key <path> [--idempotency-key <key>]";
const USAGE_SERVE: &str = "gauntlet fabric serve --state <dir> --run <run-id> \
     --caller <invocation-id> --fabric-dir <dir> --workspace <path> --answer-key <path>";

/// Why a fabric command stopped. `Usage` is a bad invocation (exit 2);
/// `Refused` is a fail-closed refusal of a well-formed request (exit 1).
#[derive(Debug)]
pub enum FabricCmdError {
    Usage(String),
    Refused(String),
}

impl fmt::Display for FabricCmdError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Usage(detail) | Self::Refused(detail) => f.write_str(detail),
        }
    }
}

type CmdResult<T> = Result<T, FabricCmdError>;

fn usage(detail: impl Into<String>) -> FabricCmdError {
    FabricCmdError::Usage(detail.into())
}

fn refused(detail: impl fmt::Display) -> FabricCmdError {
    FabricCmdError::Refused(detail.to_string())
}

/// Dispatches `gauntlet fabric <subcommand>`. `args` excludes `fabric`.
pub fn run(args: &[String]) -> ExitCode {
    let outcome = match args.first().map(String::as_str) {
        Some("admit-root") => admit_root(&args[1..]),
        Some("serve") => serve_command(&args[1..]),
        Some(other) => Err(usage(format!("unknown subcommand '{other}'"))),
        None => Err(usage("expected 'admit-root' or 'serve'")),
    };
    match outcome {
        Ok(code) => code,
        Err(FabricCmdError::Usage(detail)) => {
            eprintln!("gauntlet fabric: {detail}");
            eprintln!("usage: {USAGE_ADMIT}");
            eprintln!("       {USAGE_SERVE}");
            ExitCode::from(2)
        }
        Err(FabricCmdError::Refused(detail)) => {
            eprintln!("gauntlet fabric: {detail}");
            ExitCode::FAILURE
        }
    }
}

/// The parsed `--flag value` pairs of one subcommand. Unknown, duplicate,
/// and value-less flags are usage errors.
struct Flags(BTreeMap<String, String>);

impl Flags {
    fn parse(args: &[String], allowed: &[&str]) -> CmdResult<Self> {
        let mut values = BTreeMap::new();
        let mut index = 0;
        while index < args.len() {
            let flag = args[index].as_str();
            let Some(name) = flag.strip_prefix("--") else {
                return Err(usage(format!("unexpected argument '{flag}'")));
            };
            if !allowed.contains(&name) {
                return Err(usage(format!("unknown argument '{flag}'")));
            }
            let Some(value) = args.get(index + 1) else {
                return Err(usage(format!("{flag} requires a value")));
            };
            if values.insert(name.to_owned(), value.clone()).is_some() {
                return Err(usage(format!("{flag} was given more than once")));
            }
            index += 2;
        }
        Ok(Self(values))
    }

    fn require(&self, name: &str) -> CmdResult<&str> {
        self.0
            .get(name)
            .map(String::as_str)
            .ok_or_else(|| usage(format!("--{name} is required")))
    }

    fn optional(&self, name: &str) -> Option<&str> {
        self.0.get(name).map(String::as_str)
    }

    fn typed<T, E: fmt::Display>(
        &self,
        name: &str,
        parse: impl Fn(&str) -> Result<T, E>,
    ) -> CmdResult<T> {
        parse(self.require(name)?).map_err(|error| usage(format!("--{name}: {error}")))
    }
}

/// Composes the fabric from its directory. Constructing runners spawns
/// nothing; `state_dir` only anchors their invocation paths.
fn compose(fabric_dir: &Path, state_dir: &Path) -> CmdResult<ComposedFabric> {
    let custom = fabric_dir.join(CUSTOM_HARNESS_DIR);
    let custom_dir = if custom.is_dir() {
        Some(custom.as_path())
    } else {
        None
    };
    let config = load_fabric(fabric_dir, custom_dir).map_err(refused)?;
    let lookup = SearchPathLookup;
    let deps = FabricDeps {
        state_dir: state_dir.to_path_buf(),
        clock: Arc::new(SystemClock::new()),
        containment: None,
        path_lookup: &lookup,
    };
    compose_fabric(&config, &deps).map_err(refused)
}

fn parse_answer_key_file(path: &Path) -> CmdResult<ParsedKey> {
    let text = fs::read_to_string(path)
        .map_err(|error| refused(format!("--answer-key {}: {error}", path.display())))?;
    Ok(parse_answer_key(&text))
}

/// The durable state and graph for one state directory. Both open the same
/// durable schema, keyed by the state path.
fn open_state(state_dir: &Path) -> CmdResult<(EmbeddedStateStore, EmbeddedInvocationGraph)> {
    let locator = state_dir.join(STATE_DB_NAME);
    let store = EmbeddedStateStore::open_at(&locator).map_err(refused)?;
    let graph = EmbeddedInvocationGraph::open_at(&locator).map_err(refused)?;
    Ok((store, graph))
}

fn now_and_bound(clock: &dyn Clock) -> (EpochSeconds, Deadline) {
    let now = clock.now();
    let bound = EpochSeconds::new(now.get() + CALL_BOUND_SPAN);
    (now, Deadline::at_epoch(bound))
}

fn admit_root(args: &[String]) -> CmdResult<ExitCode> {
    let flags = Flags::parse(args, ADMIT_FLAGS)?;
    let state_dir = PathBuf::from(flags.require("state")?);
    let fabric_dir = PathBuf::from(flags.require("fabric-dir")?);
    let run_id = flags.typed("run", RunId::new)?;
    let role = flags.typed("role", RoleId::from_str)?;
    let profile = flags.typed("profile", RunnerProfileId::new)?;
    let unit = flags.typed("unit", UnitId::new)?;
    let workspace_root = PathBuf::from(flags.require("workspace")?);
    let answer_key = PathBuf::from(flags.require("answer-key")?);
    let key_text = match flags.optional("idempotency-key") {
        Some(key) => key.to_owned(),
        None => format!("{ROOT_KEY_PREFIX}{unit}"),
    };
    let admission_key = AdmissionKey::new(&key_text)
        .map_err(|error| usage(format!("--idempotency-key: {error}")))?;
    let result_contract = ResultContractPath::new(RESULT_CONTRACT_NAME).map_err(refused)?;

    crate::compose::ensure_configured_dir(&state_dir).map_err(refused)?;
    let composed = compose(&fabric_dir, &state_dir)?;
    let grant = composed
        .policy
        .roles
        .get(&role)
        .ok_or_else(|| refused(format!("role '{role}' has no grant in the fabric")))?;
    let instance = composed
        .registry
        .get(&profile)
        .ok_or_else(|| refused(format!("profile '{profile}' is not in the fabric")))?;
    let key = parse_answer_key_file(&answer_key)?;
    let claim = key
        .units
        .iter()
        .find(|candidate| candidate.id() == &unit)
        .map(|candidate| candidate.claim().to_owned())
        .ok_or_else(|| refused(format!("unit {unit} is not in the answer key")))?;

    let (store, graph) = open_state(&state_dir)?;
    let clock = SystemClock::new();
    let (now, deadline) = now_and_bound(&clock);
    // The root's task is the unit's own claim, read from the frozen answer key.
    // Admit-root takes no free-text task, so the context is a fact, not a claim.
    let task = ContextItem {
        kind: ContextItemKind::TaskStatement,
        label: unit.to_string(),
        content: claim.to_owned(),
    };
    let envelope = build_envelope(&grant.context, std::slice::from_ref(&task))
        .map_err(|error| refused(format!("the root envelope: {error:?}")))?;
    let invocation_id = derive_invocation_id(&run_id, &admission_key);

    // The root's workspace follows the spawn rule: a replayed root keeps its
    // workspace, and a new root gets a fresh one from the provider.
    let workspaces = FsWorkspaceProvider::open(&workspace_root).map_err(refused)?;
    let (workspace, created) = match graph.node(&invocation_id, deadline).map_err(refused)? {
        Some(existing) => (existing.workspace, None),
        None => {
            let allocated = workspaces
                .create(&run_id, WorkspaceSubject::Unit(unit.clone()), deadline)
                .map_err(refused)?;
            (allocated.reference.clone(), Some(allocated.reference))
        }
    };
    let request = AdmissionRequest {
        invocation_id: invocation_id.clone(),
        run_id: run_id.clone(),
        parent: None,
        role,
        runner_profile: instance.profile.id.clone(),
        harness: instance.profile.harness.clone(),
        workspace,
        context_digest: envelope.digest().clone(),
        result_contract,
        admission_key,
        budget: composed.policy.max_budget,
        limits: composed.policy.limits,
        at: now,
    };
    let node = match graph.admit(request, deadline) {
        Ok(AdmissionOutcome::Admitted(node)) | Ok(AdmissionOutcome::AlreadyAdmitted(node)) => node,
        Ok(AdmissionOutcome::Refused(refusal)) => {
            release(&workspaces, created.as_ref(), deadline);
            return Err(refused(refusal));
        }
        Err(error) => {
            release(&workspaces, created.as_ref(), deadline);
            return Err(refused(error));
        }
    };

    // The unit dispatch is recorded once per invocation. A replay that finds
    // the dispatch for a different unit is refused rather than re-attributed.
    let facts = store
        .run_lifecycle_facts(&run_id, deadline)
        .map_err(refused)?;
    match facts
        .unit_dispatches
        .iter()
        .find(|(_, dispatched)| dispatched == &node.invocation_id)
    {
        Some((recorded, _)) if recorded != &unit => {
            return Err(refused(format!(
                "invocation {} is already dispatched for unit {recorded}",
                node.invocation_id
            )));
        }
        Some(_) => {}
        None => {
            let event = RunEvent {
                run_id,
                kind: RunEventKind::UnitDispatch {
                    unit,
                    invocation_id: node.invocation_id.clone(),
                },
                at: now,
            };
            store.append_event(event, deadline).map_err(refused)?;
        }
    }
    println!("invocation_id={}", node.invocation_id);
    Ok(ExitCode::SUCCESS)
}

/// Cleans up a workspace allocated for a refused admission. Best effort: the
/// refusal is what the caller needs to see, and a leftover directory is never
/// a graph fact.
fn release(workspaces: &FsWorkspaceProvider, created: Option<&WorkspaceRef>, deadline: Deadline) {
    if let Some(reference) = created {
        let _ = workspaces.cleanup(reference, deadline);
    }
}

/// One caller's tool surface. Each call rebuilds the service with the current
/// instant, so the deadline and the spawn timestamp never go stale in a
/// long-lived process. The caller and every port are fixed at start.
struct Host<'a> {
    caller: AgentsCaller,
    policy: &'a AgentsPolicy,
    broker: &'a DelegationBroker<'a>,
    graph: &'a dyn InvocationGraph,
    gate: &'a AuthorityGate,
    govern: &'a GovernDeps,
    workspaces: &'a dyn WorkspaceProvider,
    clock: &'a dyn Clock,
}

impl Host<'_> {
    fn surface(&self) -> AgentsSurface<'_> {
        let (at, deadline) = now_and_bound(self.clock);
        let service = AgentsService::new(
            self.caller.clone(),
            self.policy,
            self.broker,
            self.graph,
            self.gate,
            self.govern,
            self.workspaces,
        );
        AgentsSurface::new(service, CallClock { at, deadline })
    }
}

impl ToolSurface for Host<'_> {
    fn tools(&self) -> Vec<ToolListing> {
        self.surface().tools()
    }

    fn call(&self, name: &str, arguments: &JsonValue) -> ToolOutcome {
        self.surface().call(name, arguments)
    }
}

fn serve_command(args: &[String]) -> CmdResult<ExitCode> {
    let flags = Flags::parse(args, SERVE_FLAGS)?;
    let state_dir = PathBuf::from(flags.require("state")?);
    let fabric_dir = PathBuf::from(flags.require("fabric-dir")?);
    let run_id = flags.typed("run", RunId::new)?;
    let caller_id = flags.typed("caller", InvocationId::new)?;
    let workspace_root = PathBuf::from(flags.require("workspace")?);
    let answer_key = PathBuf::from(flags.require("answer-key")?);

    let locator = state_dir.join(STATE_DB_NAME);
    if !state_exists(&locator).map_err(refused)? {
        return Err(refused(format!(
            "no run state at {} — admit a root first",
            locator.display()
        )));
    }
    let composed = compose(&fabric_dir, &state_dir)?;
    let (store, graph) = open_state(&state_dir)?;
    let clock = SystemClock::new();
    let (_, deadline) = now_and_bound(&clock);

    // The caller must be a live node of this run. Anything else refuses
    // before the protocol starts.
    let node = graph
        .node(&caller_id, deadline)
        .map_err(refused)?
        .ok_or_else(|| refused(format!("caller {caller_id} is not admitted")))?;
    if node.run_id != run_id {
        return Err(refused(format!(
            "caller {caller_id} belongs to run {}, not {run_id}",
            node.run_id
        )));
    }
    if node.status.is_terminal() {
        return Err(refused(format!(
            "caller {caller_id} is terminal ({})",
            node.status
        )));
    }

    // The caller's unit is the one its root was dispatched for. Its blast
    // radius and authority are read from the frozen answer key, never from
    // any argument.
    let facts = store
        .run_lifecycle_facts(&run_id, deadline)
        .map_err(refused)?;
    let unit_id = facts
        .unit_dispatches
        .iter()
        .find(|(_, dispatched)| dispatched == &node.root)
        .map(|(unit, _)| unit.clone())
        .ok_or_else(|| {
            refused(format!(
                "root {} of caller {caller_id} has no unit dispatch in run {run_id}",
                node.root
            ))
        })?;
    let key = parse_answer_key_file(&answer_key)?;
    let unit = key
        .units
        .iter()
        .find(|candidate| candidate.id() == &unit_id)
        .ok_or_else(|| refused(format!("unit {unit_id} is not in the answer key")))?;
    let caller = AgentsCaller {
        run_id,
        invocation_id: caller_id,
        role: node.role.clone(),
        unit: unit_id,
        workspace: node.workspace.clone(),
        recorded: (unit.blast_radius(), unit.authority()),
    };

    let workspaces = FsWorkspaceProvider::open(&workspace_root).map_err(refused)?;
    let broker = DelegationBroker::new(&graph, &composed.registry, &composed.runners);
    let gate = AuthorityGate::new();
    let govern = GovernDeps::none_backend();
    let host = Host {
        caller,
        policy: &composed.policy,
        broker: &broker,
        graph: &graph,
        gate: &gate,
        govern: &govern,
        workspaces: &workspaces,
        clock: &clock,
    };
    let info = ServerInfo {
        name: SERVER_NAME.to_owned(),
        version: SERVER_VERSION.to_owned(),
    };
    let stdin = io::stdin();
    let stdout = io::stdout();
    match serve(
        &host,
        &info,
        &ServeLimits::default(),
        stdin.lock(),
        stdout.lock(),
    )
    .map_err(refused)?
    {
        ServeEnd::Eof => Ok(ExitCode::SUCCESS),
        ServeEnd::ProtocolViolation(detail) => {
            Err(refused(format!("protocol violation: {detail}")))
        }
    }
}

/// The fabric section of `gauntlet doctor --fabric-dir`: one line per
/// profile, from the composed report (UNKNOWN where a model is unset).
/// Returns `false` when the fabric does not load or compose.
pub fn doctor_fabric_section(fabric_dir: &Path) -> bool {
    match compose(fabric_dir, fabric_dir) {
        Ok(composed) => {
            println!(
                "fabric: {} profile(s) from {}",
                composed.report.len(),
                fabric_dir.display()
            );
            for report in &composed.report {
                println!("fabric: {}", report.render());
            }
            true
        }
        Err(error) => {
            eprintln!("fabric: refused: {error}");
            false
        }
    }
}
