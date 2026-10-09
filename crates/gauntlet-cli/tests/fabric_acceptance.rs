//! Phase J slice 3: the offline end-to-end acceptance proof of the Agent Fabric.
//!
//! Hermetic: every test writes its own fabric directory (scripted mock profiles
//! only), answer key, and state directory under the system temp directory. The
//! durable graph and state are the database schema the state path keys, the
//! same substrate `fabric_cli.rs` uses. The `gauntlet` binary is spawned through
//! `CARGO_BIN_EXE_gauntlet`, so `admit-root`, `serve`, and `doctor` run the real
//! wire. The parent's bounded program runs the real Codemode Bun host, and these
//! tests fail (not skip) when bun or the host entry is missing.
//!
//! Names: profile and harness ids are neutral (`profile-parent`, `profile-coder`,
//! `profile-critic`, `profile-worker`, `harness-alpha`, ...), following
//! `fabric_cli.rs`. The composition root's vendor denylist covers test sources,
//! so no product name is spelled here.
//!
//! Scenario fixtures are the frozen set in `tests/fixtures/mock_runner_scenarios`
//! (AK-059 locator); none is added. Mapping used below:
//!
//! - `happy`: builder, writes a valid contract after one in-flight probe.
//! - `critic-pass-without-evidence`: critic, writes a valid contract at once.
//! - `no-result`: builder, silent, times out after three probes.
//! - `malformed`: builder, writes a raw non-document instead of a contract.
//!
//! There is no exit-without-result fixture, so "exited without a contract" is
//! not covered here; the silent child stands for "said nothing bound".
//!
//! Settlement is not a fabric concern. Collect reports INVOCATION completion
//! (a schema-valid contract bound to the node), and the controller's observe,
//! compare, and verify path owns settlement (see `gauntlet-app`'s
//! `delegation/reconcile.rs` module docs). The settlement assertions here check
//! that the fabric wrote none of it.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Output, Stdio};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gauntlet_adapter_state_postgres::{EmbeddedInvocationGraph, EmbeddedStateStore};
use gauntlet_adapter_surface_rpc::METHOD_TOOLS_CALL;
use gauntlet_adapter_tool_program_codemode::{
    DEFAULT_WORKBENCH_DIR, HOST_ENTRY, bun_codemode_host,
};
use gauntlet_app::delegation::DelegationBroker;
use gauntlet_app::delegation::reconcile::DelegationStatus;
use gauntlet_cli::fabric_compose::{
    ExecutableLookup, FabricDeps, RunnerToken, compose_fabric, load_fabric,
};
use gauntlet_domain::{EpochSeconds, InvocationId, RoleId, RunId};
use gauntlet_ports::clock::Clock;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::invocation_graph::{InvocationGraph, InvocationNode, NodeStatus};
use gauntlet_ports::json::{JsonValue, parse};
use gauntlet_ports::state_store::StateStore;

const STATE_DB: &str = "gauntlet-state.db";
const RUN: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAA";
const OTHER_RUN: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAB";
const UNIT: &str = "AK-001";
const MISSING_CALLER: &str = "01ARZ3NDEKTSV4RRFFQ69G5FZZ";
const PROGRAM_RUN: &str = "program.run";

const ROLE_ORCHESTRATOR: &str = "orchestrator";
const ROLE_BUILDER: &str = "builder";
const ROLE_CRITIC: &str = "critic";

const PROFILE_PARENT: &str = "profile-parent";
const PROFILE_CODER: &str = "profile-coder";
const PROFILE_CRITIC: &str = "profile-critic";
const PROFILE_WORKER: &str = "profile-worker";
const PROFILE_MALFORMED: &str = "profile-malformed";
const PROFILE_ROOT_ALT: &str = "profile-root-alt";

const HARNESS_ALPHA: &str = "harness-alpha";
const HARNESS_BETA: &str = "harness-beta";
const HARNESS_GAMMA: &str = "harness-gamma";

/// The fixture limits: depth, children per parent, and the root's budget ceiling.
const MAX_DEPTH: u32 = 2;
const MAX_CHILDREN: u32 = 2;
const ROOT_ROUNDS: u32 = 5;
const ROOT_WALL: u64 = 600;
const ROOT_TOKENS: u64 = 7000;
const ROOT_COST: u64 = 8100;
/// The bound a spawn carries, measured from the call instant.
const SPAWN_BOUND: u64 = 30;
/// The span a test deadline reaches past now.
const HORIZON: u64 = 600;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// A budget slice as the spawn wire carries it.
struct Budget {
    rounds: u32,
    wall: u64,
    tokens: u64,
    cost: u64,
}

const CHILD: Budget = Budget {
    rounds: 2,
    wall: 20,
    tokens: 20,
    cost: 20,
};
const GRANDCHILD: Budget = Budget {
    rounds: 1,
    wall: 10,
    tokens: 10,
    cost: 10,
};

/// The profiles of the main fabric: `(id, harness, scenario)`.
const MAIN_PROFILES: &[(&str, &str, &str)] = &[
    (PROFILE_PARENT, HARNESS_ALPHA, "no-result"),
    (PROFILE_CODER, HARNESS_ALPHA, "happy"),
    (PROFILE_CRITIC, HARNESS_BETA, "critic-pass-without-evidence"),
    (PROFILE_WORKER, HARNESS_GAMMA, "no-result"),
    (PROFILE_MALFORMED, HARNESS_ALPHA, "malformed"),
];

/// One test's directories and its fabric configuration.
struct Fixture {
    base: PathBuf,
    fabric: PathBuf,
    state: PathBuf,
    workspaces: PathBuf,
    answer_key: PathBuf,
    /// When set, every gauntlet process in this fixture gets this PATH.
    path_env: Option<PathBuf>,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        Self::with_profiles(tag, MAIN_PROFILES)
    }

    fn with_profiles(tag: &str, profiles: &[(&str, &str, &str)]) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let base = std::env::temp_dir().join(format!(
            "gauntlet-fabric-acceptance-{tag}-{}-{counter}-{nanos:x}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let fixture = Self {
            fabric: base.join("fabric"),
            state: base.join("state"),
            workspaces: base.join("workspaces"),
            answer_key: base.join("answer-key.md"),
            path_env: None,
            base,
        };
        write(&fixture.fabric.join("limits.toml"), &limits_text());
        for harness in [HARNESS_ALPHA, HARNESS_BETA, HARNESS_GAMMA] {
            write(
                &fixture
                    .fabric
                    .join("harnesses")
                    .join(format!("{harness}.toml")),
                &harness_toml(harness),
            );
        }
        for (id, harness, scenario) in profiles {
            write(
                &fixture.fabric.join("profiles").join(format!("{id}.toml")),
                &format!(
                    "schema = 1\nid = \"{id}\"\nharness = \"{harness}\"\ntoken = \"{}\"\n\
                     scenario = \"{scenario}\"\n",
                    RunnerToken::Scripted.token()
                ),
            );
        }
        write(
            &fixture.fabric.join("roles").join("orchestrator.toml"),
            &role_toml(
                ROLE_ORCHESTRATOR,
                true,
                "task-statement, constraint-statement, reference-excerpt",
            ),
        );
        write(
            &fixture.fabric.join("roles").join("builder.toml"),
            &role_toml(
                ROLE_BUILDER,
                true,
                "task-statement, constraint-statement, file-excerpt",
            ),
        );
        write(
            &fixture.fabric.join("roles").join("critic.toml"),
            &role_toml(
                ROLE_CRITIC,
                false,
                "task-statement, reference-excerpt, evidence-reference",
            ),
        );
        write(&fixture.answer_key, ANSWER_KEY);
        fixture
    }

    fn state_db(&self) -> PathBuf {
        self.state.join(STATE_DB)
    }

    fn graph(&self) -> EmbeddedInvocationGraph {
        EmbeddedInvocationGraph::open_at(&self.state_db()).expect("the graph opens")
    }

    fn store(&self) -> EmbeddedStateStore {
        EmbeddedStateStore::open_at(&self.state_db()).expect("the state store opens")
    }

    /// A `gauntlet` command with this fixture's PATH, when one is set.
    fn command(&self) -> Command {
        let mut command = gauntlet();
        if let Some(path) = &self.path_env {
            command.env("PATH", path);
        }
        command
    }

    /// `gauntlet fabric admit-root` for `profile` in `role`, with extra flags.
    fn admit_root_with(&self, run: &str, profile: &str, role: &str, extra: &[&str]) -> Output {
        let mut command = self.command();
        command.args(["fabric", "admit-root"]);
        command.args(["--state", path(&self.state)]);
        command.args(["--run", run]);
        command.args(["--fabric-dir", path(&self.fabric)]);
        command.args(["--role", role, "--profile", profile, "--unit", UNIT]);
        command.args(["--workspace", path(&self.workspaces)]);
        command.args(["--answer-key", path(&self.answer_key)]);
        command.args(extra);
        command.output().expect("the gauntlet binary runs")
    }

    /// Admits the root of `run` on the parent profile and returns its id.
    fn admit_parent(&self, run: &str) -> String {
        let output = self.admit_root_with(run, PROFILE_PARENT, ROLE_ORCHESTRATOR, &[]);
        assert!(output.status.success(), "{}", stderr(&output));
        invocation_line(&output)
    }

    /// Starts `gauntlet fabric serve` for `caller`. `program` adds the Codemode host.
    fn serve(&self, run: &str, caller: &str, program: bool) -> Serve {
        let mut command = self.command();
        command
            .args(["fabric", "serve"])
            .args(["--state", path(&self.state)])
            .args(["--run", run])
            .args(["--caller", caller])
            .args(["--fabric-dir", path(&self.fabric)])
            .args(["--workspace", path(&self.workspaces)])
            .args(["--answer-key", path(&self.answer_key)]);
        if program {
            let host = workbench_dir();
            command.args(["--program-host-dir", path(&host)]);
        }
        let mut child = command
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("the gauntlet binary spawns");
        let stdin = child.stdin.take().expect("stdin is piped");
        let stdout = BufReader::new(child.stdout.take().expect("stdout is piped"));
        Serve {
            child,
            stdin: Some(stdin),
            stdout,
        }
    }
}

impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.base);
    }
}

/// A running serve process: requests in, one protocol line out per request.
struct Serve {
    child: Child,
    stdin: Option<ChildStdin>,
    stdout: BufReader<ChildStdout>,
}

impl Serve {
    /// Sends one request line and returns the one response line.
    fn call(&mut self, request: &str) -> String {
        self.send(request);
        let mut line = String::new();
        self.stdout.read_line(&mut line).expect("a response line");
        line
    }

    fn send(&mut self, request: &str) {
        let stdin = self.stdin.as_mut().expect("stdin is open");
        stdin
            .write_all(request.as_bytes())
            .expect("the request writes");
        stdin.write_all(b"\n").expect("the terminator writes");
        stdin.flush().expect("the request flushes");
    }

    fn initialize(&mut self) {
        let reply = self.call(&rpc(1, "initialize", "{}"));
        assert!(reply.contains("\"protocolVersion\""), "initialize: {reply}");
        self.send(r#"{"jsonrpc":"2.0","method":"notifications/initialized"}"#);
    }

    /// Calls one tool and returns the raw response line.
    fn tool(&mut self, id: u32, name: &str, arguments: &str) -> String {
        let params = format!(r#"{{"name":"{name}","arguments":{arguments}}}"#);
        self.call(&rpc(id, METHOD_TOOLS_CALL, &params))
    }

    /// Ends the session by closing stdin and waits for a clean exit.
    fn finish(mut self) -> (std::process::ExitStatus, String) {
        drop(self.stdin.take());
        let mut rest = String::new();
        std::io::Read::read_to_string(&mut self.stdout, &mut rest).expect("stdout drains");
        let status = self.child.wait().expect("the child exits");
        (status, rest)
    }

    /// Kills the process without a clean shutdown (SIGKILL on unix).
    fn crash(mut self) {
        self.child.kill().expect("the process is killed");
        let _ = self.child.wait();
    }
}

fn gauntlet() -> Command {
    let exe = env!("CARGO_BIN_EXE_gauntlet");
    Command::new(exe)
}

fn rpc(id: u32, method: &str, params: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{params}}}"#)
}

const TASK_ITEM: &str = r#"{"kind":"task-statement","label":"task","content":"do the unit"}"#;
/// The critic gets its own task text: its fresh context is its own envelope.
const CRITIC_ITEM: &str = r#"{"kind":"task-statement","label":"critic-task","content":"review the unit against its reference"}"#;

/// A spawn request for `role`, pinned to `pin` when given.
fn spawn_args(role: &str, pin: Option<&str>, items: &[&str], key: &str, budget: &Budget) -> String {
    let items = items.join(",");
    let preference = match pin {
        Some(profile) => format!(r#","preference":{{"pin":"{profile}"}}"#),
        None => String::new(),
    };
    format!(
        r#"{{"role":"{role}","items":[{items}],"budget":{},"idempotency_key":"{key}","deadline_seconds":{SPAWN_BOUND}{preference}}}"#,
        budget_json(budget)
    )
}

fn budget_json(budget: &Budget) -> String {
    format!(
        r#"{{"rounds":{},"wall_seconds":{},"tokens":{},"cost_usd_micros":{}}}"#,
        budget.rounds, budget.wall, budget.tokens, budget.cost
    )
}

fn limits_text() -> String {
    format!(
        "schema = 1\nmax_depth = {MAX_DEPTH}\nmax_children_per_parent = {MAX_CHILDREN}\n\
rounds = {ROOT_ROUNDS}\nwall_seconds = {ROOT_WALL}\ntokens = {ROOT_TOKENS}\n\
cost_usd_micros = {ROOT_COST}\nisolation = \"process\"\n"
    )
}

fn harness_toml(id: &str) -> String {
    format!(
        "schema = 1\nid = \"{id}\"\nlabel = \"{id} label\"\nexecutables = [\"{id}-exe\"]\n\n\
         [launch]\nargs = []\nprompt_mode = \"unknown\"\n\n\
         [session]\nidentity = \"none\"\n\n\
         [lifecycle]\nsource = \"screen\"\n\n\
         [control]\ninterrupt = \"none\"\n\n\
         [capabilities]\ninteractive = false\nresumable = false\nmodel_selection = false\n\
         native_subagents = false\nstructured_events = false\nsupport_level = 0\n"
    )
}

fn role_toml(role: &str, may_spawn: bool, allow: &str) -> String {
    format!(
        "schema = 1\nrole = \"{role}\"\nmay_spawn = {may_spawn}\nallow = [{}]\n\
         deny = []\npermissions = \"fabric-permissions\"\n\
         sandbox_policy = \"fabric-sandbox\"\nworkspace = \"fresh\"\n",
        allow
            .split(", ")
            .map(|kind| format!("\"{kind}\""))
            .collect::<Vec<_>>()
            .join(", ")
    )
}

const ANSWER_KEY: &str = "## AK-001\nclaim: The fabric admits a bounded child.\nverify_by: test\n\
blast_radius: contained\nauthority: critic\n\
evidence_required: {kind: test_report, locator: suite-check-0001, producer: instrument}\n";

fn path(value: &Path) -> &str {
    value.to_str().expect("the test path is UTF-8")
}

fn write(file: &Path, body: &str) {
    if let Some(parent) = file.parent() {
        std::fs::create_dir_all(parent).expect("the directory is creatable");
    }
    std::fs::write(file, body).expect("the file is writable");
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn invocation_line(output: &Output) -> String {
    let stdout = String::from_utf8(output.stdout.clone()).expect("stdout is UTF-8");
    stdout
        .lines()
        .find_map(|line| line.strip_prefix("invocation_id="))
        .map(str::to_owned)
        .unwrap_or_else(|| panic!("no invocation_id line in stdout: {stdout:?}"))
}

fn now_seconds() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn deadline() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(now_seconds() + HORIZON))
}

fn id(text: &str) -> InvocationId {
    InvocationId::new(text).expect("the test id is in the grammar")
}

fn node(graph: &EmbeddedInvocationGraph, of: &InvocationId) -> InvocationNode {
    graph
        .node(of, deadline())
        .expect("the graph reads")
        .unwrap_or_else(|| panic!("{of} is in the graph"))
}

fn child_ids(graph: &EmbeddedInvocationGraph, parent: &InvocationId) -> Vec<InvocationId> {
    graph
        .children(parent, deadline())
        .expect("the children read")
        .into_iter()
        .map(|node| node.invocation_id)
        .collect()
}

/// The JSON payload of one successful `tools/call` reply.
fn payload(reply: &str) -> JsonValue {
    let envelope = parse(reply).expect("the reply is JSON");
    let text = envelope
        .get("result")
        .and_then(|result| result.get("content"))
        .and_then(|content| match content {
            JsonValue::Array(items) => items.first(),
            _ => None,
        })
        .and_then(|item| item.get("text"))
        .and_then(|text| match text {
            JsonValue::Str(text) => Some(text.clone()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("no text payload in {reply}"));
    parse(&text).expect("the payload is JSON")
}

/// The string under `key` of an object payload.
fn text_field(value: &JsonValue, key: &str) -> String {
    match value.get(key) {
        Some(JsonValue::Str(text)) => text.clone(),
        other => panic!("{key}: expected a string, got {other:?}"),
    }
}

/// The JavaScript program source for a JSON-free program (single quotes only).
fn run_program(serve: &mut Serve, request: u32, source: &str) -> JsonValue {
    let arguments = format!(r#"{{"source":"{}"}}"#, source.replace('"', "\\\""));
    let reply = serve.tool(request, PROGRAM_RUN, &arguments);
    let run = payload(&reply);
    assert_eq!(text_field(&run, "status"), "completed", "{reply}");
    run
}

/// The repository's workbench, located from this crate's manifest. Tests that
/// run the real host fail rather than skip when its entry is missing.
fn workbench_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|dir| dir.join(DEFAULT_WORKBENCH_DIR))
        .find(|dir| dir.join(HOST_ENTRY).is_file())
        .unwrap_or_else(|| panic!("the codemode host entry ({HOST_ENTRY}) is missing"))
}

/// A PATH directory holding only the program host's runtime, so no other
/// executable on the machine is reachable from the fabric processes. The name
/// comes from the host definition. Fails when that runtime is not on PATH.
fn host_only_path(base: &Path) -> PathBuf {
    let program = bun_codemode_host(&workbench_dir()).executable;
    let program = PathBuf::from(program);
    let target = if program.is_absolute() {
        program.clone()
    } else {
        let search = std::env::var_os("PATH").unwrap_or_default();
        std::env::split_paths(&search)
            .map(|dir| dir.join(&program))
            .find(|candidate| candidate.is_file())
            .unwrap_or_else(|| panic!("the program host runtime is required and not on PATH"))
    };
    let dir = base.join("runtime-only-path");
    std::fs::create_dir_all(&dir).expect("the PATH directory is creatable");
    let name = program.file_name().expect("the runtime has a file name");
    std::os::unix::fs::symlink(&target, dir.join(name)).expect("the runtime link is creatable");
    dir
}

/// A clock for the in-process composition below.
struct TestClock;

impl Clock for TestClock {
    fn now(&self) -> EpochSeconds {
        EpochSeconds::new(now_seconds())
    }

    /// The scenarios never wait: the scripted runner advances by probes, not time.
    fn sleep_until(&self, _at: EpochSeconds) -> Result<(), gauntlet_ports::clock::ClockError> {
        Ok(())
    }
}

/// No executable is discoverable: the scripted profiles need none.
struct NoExecutables;

impl ExecutableLookup for NoExecutables {
    fn locate(&self, _executable: &str) -> Option<PathBuf> {
        None
    }
}

/// The status of `of` read by a fresh in-process broker, the way a process that
/// restarted with no runner memory sees it.
fn restarted_status(fixture: &Fixture, of: &InvocationId) -> DelegationStatus {
    let config = load_fabric(&fixture.fabric, Some(&fixture.fabric.join("harnesses")))
        .expect("the fabric loads");
    let lookup = NoExecutables;
    let state_dir = fixture.state.clone();
    let deps = FabricDeps {
        state_dir,
        clock: Arc::new(TestClock),
        containment: None,
        path_lookup: &lookup,
        mcp_serve: None,
    };
    let composed = compose_fabric(&config, &deps).expect("the fabric composes");
    let graph = fixture.graph();
    let broker = DelegationBroker::new(&graph, &composed.registry, &composed.runners);
    broker.status(of, deadline()).expect("the status reads")
}

// ---- the tree the scenarios share -------------------------------------------

/// The ids of the main tree: the root, its coder child (with a critic grandchild),
/// and a silent worker sibling of the coder.
struct Tree {
    root: String,
    coder: String,
    critic: String,
    worker: String,
}

/// Builds ROOT -> C1 (coder) -> G1 (critic) and ROOT -> W1 (worker), each through
/// the serve process of its own caller, as the wire does.
fn build_tree(fixture: &Fixture) -> Tree {
    let root = fixture.admit_parent(RUN);

    let mut parent = fixture.serve(RUN, &root, false);
    parent.initialize();
    let coder = payload(&parent.tool(
        3,
        "agents.spawn",
        &spawn_args(
            ROLE_BUILDER,
            Some(PROFILE_CODER),
            &[TASK_ITEM],
            "key-coder",
            &CHILD,
        ),
    ));
    let coder = text_field(&coder, "invocation_id");
    assert!(parent.finish().0.success());

    let mut coder_serve = fixture.serve(RUN, &coder, false);
    coder_serve.initialize();
    let critic = payload(&coder_serve.tool(
        3,
        "agents.spawn",
        &spawn_args(
            ROLE_CRITIC,
            Some(PROFILE_CRITIC),
            &[CRITIC_ITEM],
            "key-critic",
            &GRANDCHILD,
        ),
    ));
    let critic = text_field(&critic, "invocation_id");
    assert!(coder_serve.finish().0.success());

    let mut parent = fixture.serve(RUN, &root, false);
    parent.initialize();
    let worker = payload(&parent.tool(
        4,
        "agents.spawn",
        &spawn_args(
            ROLE_BUILDER,
            Some(PROFILE_WORKER),
            &[TASK_ITEM],
            "key-worker",
            &CHILD,
        ),
    ));
    let worker = text_field(&worker, "invocation_id");
    assert!(parent.finish().0.success());

    Tree {
        root,
        coder,
        critic,
        worker,
    }
}

// ---- the acceptance scenarios -----------------------------------------------

/// Steps A to E: admission, the bounded program, the spawn chain across serve
/// processes, sibling placement, and the graph facts with idempotent replay.
#[test]
fn acceptance_admission_program_spawn_chain_and_graph_facts() {
    let fixture = Fixture::new("graph");

    // A. The root is admitted once; a second root in the same run is refused.
    let root = fixture.admit_parent(RUN);
    let second_root = fixture.admit_root_with(
        RUN,
        PROFILE_PARENT,
        ROLE_ORCHESTRATOR,
        &["--idempotency-key", "second-root"],
    );
    assert!(!second_root.status.success(), "one root per run");

    // B. The parent's bounded program reads the agents surface through the
    // trace, sees the same data as a direct call, and has no ambient access.
    let mut parent = fixture.serve(RUN, &root, true);
    parent.initialize();
    let run = run_program(
        &mut parent,
        3,
        "const a = await tools.agents.list({}); \
         const b = await tools.agents.list({}); \
         return { first: a, second: b, \
         ambient: [typeof fetch, typeof process, typeof require, typeof Deno] };",
    );
    let trace = match run.get("trace") {
        Some(JsonValue::Array(trace)) => trace.clone(),
        other => panic!("the trace is an array: {other:?}"),
    };
    assert_eq!(trace.len(), 2, "two inner calls: {trace:?}");
    assert_eq!(text_field(&trace[0], "tool"), "agents.list");
    assert_eq!(text_field(&trace[0], "outcome"), "ok");
    let value = run.get("value").expect("a value");
    assert_eq!(
        value.get("ambient"),
        Some(&JsonValue::Array(vec![
            JsonValue::Str("undefined".to_owned()),
            JsonValue::Str("undefined".to_owned()),
            JsonValue::Str("undefined".to_owned()),
            JsonValue::Str("undefined".to_owned()),
        ])),
        "no fetch, process, require, or Deno inside a program"
    );
    assert!(parent.finish().0.success());

    // C. The coder spawns a fresh-context critic. The critic's role allowlist
    // refuses a file excerpt, so the refusal creates nothing; the allowed
    // spawn creates a grandchild at depth 2 that keeps the root.
    let tree = build_tree(&fixture);
    let graph = fixture.graph();
    let coder = id(&tree.coder);
    let critic = id(&tree.critic);
    let worker = id(&tree.worker);
    let root_id = id(&tree.root);

    // E. Exactly one root and the parent/root/depth facts.
    let roots: Vec<InvocationId> = graph
        .active_nodes(&RunId::new(RUN).expect("the run id is valid"), deadline())
        .expect("the active nodes read")
        .into_iter()
        .filter(|node| node.parent.is_none())
        .map(|node| node.invocation_id)
        .collect();
    assert_eq!(roots, vec![root_id.clone()], "exactly one root in the run");
    assert_eq!(root, tree.root);

    let root_node = node(&graph, &root_id);
    assert_eq!(root_node.depth, 0);
    assert_eq!(root_node.root, root_id);

    let coder_node = node(&graph, &coder);
    assert_eq!(coder_node.parent, Some(root_id.clone()));
    assert_eq!(coder_node.depth, 1);
    assert_eq!(coder_node.root, root_id, "the root is preserved");

    let critic_node = node(&graph, &critic);
    assert_eq!(critic_node.parent, Some(coder.clone()));
    assert_eq!(critic_node.depth, 2, "the grandchild sits at the ceiling");
    assert_eq!(critic_node.root, root_id);
    assert_eq!(critic_node.role, RoleId::Critic);

    let worker_node = node(&graph, &worker);
    assert_eq!(worker_node.parent, Some(root_id.clone()));
    assert_eq!(worker_node.depth, 1, "the worker is a sibling of the coder");
    assert_eq!(worker_node.root, root_id);

    assert_eq!(
        child_ids(&graph, &root_id),
        vec![coder.clone(), worker.clone()],
        "the root has exactly the coder and the worker"
    );
    assert_eq!(child_ids(&graph, &coder), vec![critic.clone()]);
    assert!(child_ids(&graph, &critic).is_empty());

    // Budgets are reserved within the parent: the children fit the root slice
    // and the grandchild fits the coder's.
    assert!(
        coder_node.budget.rounds + worker_node.budget.rounds <= root_node.budget.rounds,
        "the two children together fit the root slice"
    );
    assert!(GRANDCHILD.rounds <= coder_node.budget.rounds);
    assert!(GRANDCHILD.wall <= coder_node.budget.wall_seconds);
    assert!(GRANDCHILD.tokens <= coder_node.budget.tokens);
    assert_eq!(root_node.budget.rounds, ROOT_ROUNDS);
    assert_eq!(coder_node.budget.rounds, CHILD.rounds);
    assert_eq!(worker_node.budget.rounds, CHILD.rounds);
    assert_eq!(critic_node.budget.rounds, GRANDCHILD.rounds);

    // The fresh context: the critic's envelope is built from its own items only,
    // so its digest differs from the coder's and the root's (the digest covers
    // the envelope body, so identical items would give an identical digest).
    assert_ne!(critic_node.context_digest, coder_node.context_digest);
    assert_ne!(critic_node.context_digest, root_node.context_digest);

    // The allowlist: a file excerpt is refused for the critic, and the refusal
    // leaves the graph unchanged.
    let mut coder_serve = fixture.serve(RUN, &tree.coder, false);
    coder_serve.initialize();
    let refused = coder_serve.tool(
        4,
        "agents.spawn",
        &spawn_args(
            ROLE_CRITIC,
            Some(PROFILE_CRITIC),
            &[
                TASK_ITEM,
                r#"{"kind":"file-excerpt","label":"parent-transcript","content":"the parent's scratch"}"#,
            ],
            "key-leaky-critic",
            &GRANDCHILD,
        ),
    );
    assert!(refused.contains("\"isError\":true"), "{refused}");
    assert!(refused.contains("context_not_allowed"), "{refused}");
    assert!(coder_serve.finish().0.success());
    assert_eq!(
        child_ids(&graph, &coder),
        vec![critic.clone()],
        "nothing added"
    );

    // D and idempotency: a replayed spawn with the same key and facts returns
    // the same id and creates no duplicate.
    let mut coder_serve = fixture.serve(RUN, &tree.coder, false);
    coder_serve.initialize();
    let replay = payload(&coder_serve.tool(
        5,
        "agents.spawn",
        &spawn_args(
            ROLE_CRITIC,
            Some(PROFILE_CRITIC),
            &[CRITIC_ITEM],
            "key-critic",
            &GRANDCHILD,
        ),
    ));
    assert_eq!(
        text_field(&replay, "invocation_id"),
        tree.critic,
        "same key, same id"
    );
    assert_eq!(
        replay.get("launched"),
        Some(&JsonValue::Bool(false)),
        "a replay is not launched again"
    );
    assert!(coder_serve.finish().0.success());
    assert_eq!(
        child_ids(&graph, &coder),
        vec![critic.clone()],
        "no duplicate"
    );

    let mut parent = fixture.serve(RUN, &tree.root, false);
    parent.initialize();
    let replay = payload(&parent.tool(
        6,
        "agents.spawn",
        &spawn_args(
            ROLE_BUILDER,
            Some(PROFILE_WORKER),
            &[TASK_ITEM],
            "key-worker",
            &CHILD,
        ),
    ));
    assert_eq!(
        text_field(&replay, "invocation_id"),
        tree.worker,
        "worker replay"
    );
    assert!(parent.finish().0.success());
    assert_eq!(child_ids(&graph, &root_id).len(), 2, "still two children");
}

/// Step F: kill -9 the serve processes, then restart. The graph is read back from
/// the database, and a process without the mock's memory cannot reattach the
/// sessions, so the runner reports unavailable while the graph view stands.
#[test]
fn acceptance_crash_then_restart_reads_the_graph_and_reports_unreattached_sessions() {
    let fixture = Fixture::new("crash");
    let tree = build_tree(&fixture);
    let graph_before = fixture.graph();
    let before_nodes = graph_before
        .active_nodes(&RunId::new(RUN).expect("valid"), deadline())
        .expect("reads");

    // Live serve processes for the root and for the coder, then SIGKILL both.
    let parent = fixture.serve(RUN, &tree.root, false);
    let coder = fixture.serve(RUN, &tree.coder, false);
    parent.crash();
    coder.crash();

    let graph = fixture.graph();
    let after_nodes = graph
        .active_nodes(&RunId::new(RUN).expect("valid"), deadline())
        .expect("reads");
    assert_eq!(
        after_nodes.len(),
        before_nodes.len(),
        "the graph survives the kill"
    );

    // A fresh serve reports the same graph through agents.status.
    let mut coder = fixture.serve(RUN, &tree.coder, false);
    coder.initialize();
    let status = coder.tool(
        3,
        "agents.status",
        &format!(r#"{{"invocation_id":"{}"}}"#, tree.critic),
    );
    assert!(status.contains("\"isError\":false"), "{status}");
    assert!(status.contains(&tree.critic), "{status}");
    assert_eq!(
        text_field(&payload(&status), "parent"),
        tree.coder,
        "the critic's parent is the coder"
    );
    assert!(coder.finish().0.success());

    let mut parent = fixture.serve(RUN, &tree.root, false);
    parent.initialize();
    let status = parent.tool(
        3,
        "agents.status",
        &format!(r#"{{"invocation_id":"{}"}}"#, tree.worker),
    );
    assert!(status.contains(&tree.worker), "{status}");
    assert!(parent.finish().0.success());

    // The sessions the mock cannot reattach: the restarted runner does not know
    // the handles, so the status says unavailable, the graph status stands, and
    // nothing is completed silently.
    for of in [&tree.coder, &tree.critic, &tree.worker] {
        let reading = restarted_status(&fixture, &id(of));
        assert_eq!(
            reading.node.status,
            NodeStatus::Running,
            "{of} keeps its graph status after the restart"
        );
        assert!(
            reading.runner_unavailable,
            "{of} reports runner_unavailable: the restarted runner cannot reattach it"
        );
        assert!(
            reading.runner_status.is_none(),
            "{of} has no runner status to report"
        );
    }
}

/// OD-14a: after a SIGKILL and restart, `agents.collect` on a live node whose
/// session the restarted runner cannot reattach is a typed, non-terminal answer,
/// not an internal error. The node keeps its graph status and is never completed.
#[test]
fn acceptance_collect_after_restart_reports_runner_unavailable_and_never_completes() {
    let fixture = Fixture::new("collect-restart");
    let tree = build_tree(&fixture);

    let parent = fixture.serve(RUN, &tree.root, false);
    let coder = fixture.serve(RUN, &tree.coder, false);
    parent.crash();
    coder.crash();

    let mut coder = fixture.serve(RUN, &tree.coder, false);
    coder.initialize();
    let reply = coder.tool(
        3,
        "agents.collect",
        &format!(r#"{{"invocation_id":"{}"}}"#, tree.critic),
    );
    assert!(reply.contains("\"isError\":false"), "{reply}");
    let value = payload(&reply);
    assert_eq!(text_field(&value, "state"), "still_running", "{reply}");
    assert_eq!(
        value.get("runner_unavailable"),
        Some(&JsonValue::Bool(true)),
        "the restarted runner cannot answer: {reply}"
    );
    assert!(value.get("claim").is_none(), "nothing is claimed: {reply}");
    assert!(coder.finish().0.success());

    let graph = fixture.graph();
    assert_eq!(
        node(&graph, &id(&tree.critic)).status,
        NodeStatus::Running,
        "the graph keeps the critic live"
    );
}

/// Step G: collect reports invocation completion only for a schema-valid contract
/// bound to the node, and the fabric writes no settlement or verification.
///
/// A scripted child lives in the process that launched it, so a collect must come
/// from that process. The root's serve stays open while the coder's own serve
/// spawns and collects the critic, the way two live callers would.
#[test]
fn acceptance_collect_reports_contract_completion_and_writes_no_settlement() {
    let fixture = Fixture::new("completion");
    let root = fixture.admit_parent(RUN);
    let run = RunId::new(RUN).expect("valid");

    let mut parent = fixture.serve(RUN, &root, false);
    parent.initialize();
    let coder = text_field(
        &payload(&parent.tool(
            3,
            "agents.spawn",
            &spawn_args(
                ROLE_BUILDER,
                Some(PROFILE_CODER),
                &[TASK_ITEM],
                "key-coder",
                &CHILD,
            ),
        )),
        "invocation_id",
    );
    let worker = text_field(
        &payload(&parent.tool(
            4,
            "agents.spawn",
            &spawn_args(
                ROLE_BUILDER,
                Some(PROFILE_WORKER),
                &[TASK_ITEM],
                "key-worker",
                &CHILD,
            ),
        )),
        "invocation_id",
    );

    // A silent child (it says nothing bound) is not complete and has no claim.
    let silent = payload(&parent.tool(
        5,
        "agents.collect",
        &format!(r#"{{"invocation_id":"{worker}"}}"#),
    ));
    assert_eq!(
        text_field(&silent, "state"),
        "still_running",
        "silent is not complete"
    );
    assert!(silent.get("claim").is_none(), "no claim without a contract");

    // The coder's contract is valid after one in-flight probe: the first collect
    // is still running.
    let first = payload(&parent.tool(
        6,
        "agents.collect",
        &format!(r#"{{"invocation_id":"{coder}"}}"#),
    ));
    assert_eq!(text_field(&first, "state"), "still_running");

    // The coder's own serve spawns the critic and collects its valid contract.
    let mut coder_serve = fixture.serve(RUN, &coder, false);
    coder_serve.initialize();
    let critic = text_field(
        &payload(&coder_serve.tool(
            3,
            "agents.spawn",
            &spawn_args(
                ROLE_CRITIC,
                Some(PROFILE_CRITIC),
                &[CRITIC_ITEM],
                "key-critic",
                &GRANDCHILD,
            ),
        )),
        "invocation_id",
    );
    let completed = payload(&coder_serve.tool(
        4,
        "agents.collect",
        &format!(r#"{{"invocation_id":"{critic}"}}"#),
    ));
    assert_eq!(
        text_field(&completed, "state"),
        "completed",
        "{completed:?}"
    );
    assert!(
        text_field(&completed, "claim").contains(&critic),
        "the claim names its invocation"
    );
    let again = payload(&coder_serve.tool(
        5,
        "agents.collect",
        &format!(r#"{{"invocation_id":"{critic}"}}"#),
    ));
    assert_eq!(
        text_field(&again, "state"),
        "already_terminal",
        "a collected contract is not claimed twice"
    );

    // A child that writes a raw non-document instead of a contract is not
    // complete: the collect fails it and carries no claim.
    let malformed = text_field(
        &payload(&coder_serve.tool(
            6,
            "agents.spawn",
            &spawn_args(
                ROLE_BUILDER,
                Some(PROFILE_MALFORMED),
                &[TASK_ITEM],
                "key-malformed",
                &GRANDCHILD,
            ),
        )),
        "invocation_id",
    );
    let malformed_collect = payload(&coder_serve.tool(
        7,
        "agents.collect",
        &format!(r#"{{"invocation_id":"{malformed}"}}"#),
    ));
    assert_eq!(
        text_field(&malformed_collect, "state"),
        "failed",
        "{malformed_collect:?}"
    );
    assert!(
        malformed_collect.get("claim").is_none(),
        "no claim from a non-document"
    );
    assert!(coder_serve.finish().0.success());

    // The root's process holds the coder's runner handle: its second poll
    // completes the coder on a valid contract.
    let second = payload(&parent.tool(
        7,
        "agents.collect",
        &format!(r#"{{"invocation_id":"{coder}"}}"#),
    ));
    assert_eq!(text_field(&second, "state"), "completed", "{second:?}");
    assert!(
        text_field(&second, "claim").contains(&coder),
        "the claim names the coder"
    );
    assert!(parent.finish().0.success());

    let graph = fixture.graph();
    let critic_node = node(&graph, &id(&critic));
    assert_eq!(critic_node.status, NodeStatus::Completed);
    assert!(
        critic_node.result_digest.is_some(),
        "the bound contract digest is recorded on the node"
    );
    assert_eq!(node(&graph, &id(&coder)).status, NodeStatus::Completed);
    assert!(
        matches!(node(&graph, &id(&malformed)).status, NodeStatus::Failed(_)),
        "a non-document child fails rather than completing"
    );
    let silent_status = node(&graph, &id(&worker)).status;
    assert!(
        matches!(silent_status, NodeStatus::Admitted | NodeStatus::Running),
        "a silent child stays open: {silent_status:?}"
    );

    // Completion is not settlement: the fabric wrote no settlement, no
    // verification, and no settlement event for these invocations.
    let store = fixture.store();
    assert!(
        store
            .settlements(&run, deadline())
            .expect("reads")
            .is_empty(),
        "no settlement"
    );
    assert!(
        store
            .verification_records(&run, deadline())
            .expect("reads")
            .is_empty(),
        "no verification record"
    );
    let export = store
        .export_jsonl(&run, deadline())
        .expect("the log exports");
    assert!(
        !export.contains("SettlementCommitted"),
        "no settlement event in the run log"
    );
}

/// Step I: cancelling C1 cancels its subtree (G1) in one call, and the sibling
/// W1 is unaffected. A caller cannot cancel outside its own subtree.
#[test]
fn acceptance_cancel_takes_the_subtree_and_not_the_sibling() {
    let fixture = Fixture::new("cancel");
    let tree = build_tree(&fixture);

    let mut coder = fixture.serve(RUN, &tree.coder, false);
    coder.initialize();
    let outside = coder.tool(
        3,
        "agents.cancel",
        &format!(r#"{{"invocation_id":"{}"}}"#, tree.worker),
    );
    assert!(
        outside.contains("\"isError\":true"),
        "not in the subtree: {outside}"
    );
    assert!(coder.finish().0.success());

    let mut parent = fixture.serve(RUN, &tree.root, false);
    parent.initialize();
    let cancelled = payload(&parent.tool(
        3,
        "agents.cancel",
        &format!(r#"{{"invocation_id":"{}"}}"#, tree.coder),
    ));
    let JsonValue::Array(ids) = cancelled
        .get("cancelled")
        .expect("a cancelled list")
        .clone()
    else {
        panic!("cancelled is an array: {cancelled:?}")
    };
    let ids: Vec<String> = ids
        .into_iter()
        .filter_map(|item| match item {
            JsonValue::Str(text) => Some(text),
            _ => None,
        })
        .collect();
    assert!(ids.contains(&tree.coder), "the coder is cancelled: {ids:?}");
    assert!(
        ids.contains(&tree.critic),
        "the grandchild is cancelled: {ids:?}"
    );
    assert!(
        !ids.contains(&tree.worker),
        "the sibling is not named: {ids:?}"
    );
    assert!(parent.finish().0.success());

    let graph = fixture.graph();
    assert_eq!(node(&graph, &id(&tree.coder)).status, NodeStatus::Cancelled);
    assert_eq!(
        node(&graph, &id(&tree.critic)).status,
        NodeStatus::Cancelled
    );
    assert!(
        !node(&graph, &id(&tree.worker)).status.is_terminal(),
        "the sibling stays open"
    );
}

/// Step J: a fabric with no parent-kind profile, and a PATH that holds only bun.
/// The root of another profile admits, spawns a child, and the child spawns a
/// critic. The fabric never asks for any other executable.
#[test]
fn acceptance_fabric_without_the_parent_profile_runs_on_a_host_only_path() {
    let base_profiles: &[(&str, &str, &str)] = &[
        (PROFILE_ROOT_ALT, HARNESS_ALPHA, "no-result"),
        (PROFILE_CODER, HARNESS_ALPHA, "happy"),
        (PROFILE_CRITIC, HARNESS_BETA, "critic-pass-without-evidence"),
    ];
    let mut fixture = Fixture::with_profiles("prime-free", base_profiles);
    fixture.path_env = Some(host_only_path(&fixture.base));

    let root = fixture.admit_root_with(RUN, PROFILE_ROOT_ALT, ROLE_BUILDER, &[]);
    assert!(root.status.success(), "{}", stderr(&root));
    let root = invocation_line(&root);

    let mut serve = fixture.serve(RUN, &root, true);
    serve.initialize();
    let coder = payload(&serve.tool(
        3,
        "agents.spawn",
        &spawn_args(
            ROLE_BUILDER,
            Some(PROFILE_CODER),
            &[TASK_ITEM],
            "k-coder",
            &CHILD,
        ),
    ));
    let coder = text_field(&coder, "invocation_id");
    assert!(serve.finish().0.success());

    let mut coder_serve = fixture.serve(RUN, &coder, false);
    coder_serve.initialize();
    let critic = payload(&coder_serve.tool(
        3,
        "agents.spawn",
        &spawn_args(
            ROLE_CRITIC,
            Some(PROFILE_CRITIC),
            &[CRITIC_ITEM],
            "k-critic",
            &GRANDCHILD,
        ),
    ));
    let critic = text_field(&critic, "invocation_id");
    assert!(coder_serve.finish().0.success());

    let graph = fixture.graph();
    assert_eq!(node(&graph, &id(&critic)).depth, 2);
    assert_eq!(node(&graph, &id(&critic)).root, id(&root));

    // The doctor reads the fabric on the same PATH: the profiles are listed and
    // no probe of an absent executable fails the fabric section.
    let reports = fixture.base.join("reports");
    let doctor = fixture
        .command()
        .arg("doctor")
        .arg("--fabric-dir")
        .arg(path(&fixture.fabric))
        .env("GAUNTLET_REPORT_DIR", path(&reports))
        .output()
        .expect("the gauntlet binary runs");
    let stdout = String::from_utf8_lossy(&doctor.stdout).into_owned();
    let fabric_lines: Vec<&str> = stdout
        .lines()
        .filter(|line| line.starts_with("fabric: profile "))
        .collect();
    assert_eq!(fabric_lines.len(), 3, "each profile is listed: {stdout}");
    assert!(
        fabric_lines.iter().any(|line| line.contains(&format!(
            "fabric: profile {PROFILE_ROOT_ALT} harness {HARNESS_ALPHA}"
        ))),
        "the root profile is listed: {stdout}"
    );
    assert!(
        fabric_lines
            .iter()
            .all(|line| line.contains("availability available")),
        "no profile is unavailable on the host-only path: {stdout}"
    );
    assert!(
        !stdout.contains("fabric: refused"),
        "the fabric section composes: {stdout}"
    );
}

/// Step K: refusals create nothing. The children limit, an unlisted tool, a role
/// that is not granted, a critic that may not spawn, and a program that tries the
/// same from inside its caller.
#[test]
fn acceptance_refusals_leave_the_graph_unchanged() {
    let fixture = Fixture::new("refusals");
    let tree = build_tree(&fixture);
    let run = RunId::new(RUN).expect("valid");
    let before = fixture
        .graph()
        .active_nodes(&run, deadline())
        .expect("reads")
        .len();

    // The children limit: the root already has two live children.
    let mut parent = fixture.serve(RUN, &tree.root, true);
    parent.initialize();
    let third = parent.tool(
        3,
        "agents.spawn",
        &spawn_args(
            ROLE_BUILDER,
            Some(PROFILE_WORKER),
            &[TASK_ITEM],
            "k-third",
            &GRANDCHILD,
        ),
    );
    assert!(
        third.contains("\"isError\":true"),
        "the children limit holds: {third}"
    );

    // A role the policy does not grant.
    let ungranted = parent.tool(
        4,
        "agents.spawn",
        &spawn_args("verifier", None, &[TASK_ITEM], "k-ungranted", &GRANDCHILD),
    );
    assert!(
        ungranted.contains("\"isError\":true"),
        "not granted: {ungranted}"
    );

    // A tool outside the list.
    let unlisted = parent.tool(5, "agents.delete", r#"{"invocation_id":"x"}"#);
    assert!(
        unlisted.contains("\"error\"") || unlisted.contains("\"isError\":true"),
        "an unlisted tool is refused: {unlisted}"
    );

    // A program in the parent: the wire's refusals become program-visible.
    let program = parent.tool(
        6,
        PROGRAM_RUN,
        &format!(
            r#"{{"source":"return await tools.agents.spawn({});"}}"#,
            js(&spawn_args(
                ROLE_BUILDER,
                Some(PROFILE_WORKER),
                &[TASK_ITEM],
                "k-program",
                &GRANDCHILD
            ))
        ),
    );
    let limit = payload(&program);
    assert_eq!(text_field(&limit, "status"), "failed", "{program}");
    assert!(
        text_field(&limit, "message").contains("already holds 2 live children"),
        "the children limit refuses inside the program: {limit:?}"
    );
    assert!(parent.finish().0.success());

    // A critic may not spawn, so a program from the critic is refused with the
    // graph unchanged.
    let mut critic = fixture.serve(RUN, &tree.critic, true);
    critic.initialize();
    let denied = critic.tool(
        3,
        PROGRAM_RUN,
        &format!(
            r#"{{"source":"return await tools.agents.spawn({});"}}"#,
            js(&spawn_args(
                ROLE_BUILDER,
                Some(PROFILE_WORKER),
                &[TASK_ITEM],
                "k-denied",
                &GRANDCHILD
            ))
        ),
    );
    let denied = payload(&denied);
    assert_eq!(text_field(&denied, "status"), "failed");
    assert!(
        text_field(&denied, "message").contains("may not spawn"),
        "a critic's spawn is refused inside the program: {denied:?}"
    );
    assert!(critic.finish().0.success());

    let graph = fixture.graph();
    assert_eq!(
        child_ids(&graph, &id(&tree.root)).len(),
        2,
        "the root still has two children"
    );
    assert!(
        child_ids(&graph, &id(&tree.critic)).is_empty(),
        "the critic has none"
    );
    assert_eq!(
        graph.active_nodes(&run, deadline()).expect("reads").len(),
        before,
        "no refused spawn created a node"
    );
}

/// Serve refuses an unknown caller and the caller of another run before any
/// protocol starts, so the acceptance surface has no ambient caller.
#[test]
fn acceptance_serve_refuses_an_unknown_caller_before_the_protocol_starts() {
    let fixture = Fixture::new("unknown-caller");
    let unknown = fixture.serve(RUN, MISSING_CALLER, false).finish();
    assert!(!unknown.0.success(), "an unknown caller does not start");
    assert!(unknown.1.is_empty());
    let root = fixture.admit_parent(RUN);
    let other = fixture.serve(OTHER_RUN, &root, false).finish();
    assert!(!other.0.success(), "a caller of another run does not start");
}

// Step H (late, stale, or wrong-session lifecycle samples) is not repeated here.
// The mock emits no lifecycle samples and the supervisor is an app-level seam, so
// the acceptance for it is the app-level `lifecycle_sequencing_*` and
// `harness_lifecycle_*` tests in `crates/gauntlet-app/src/delegation/lifecycle_tests.rs`.

// ---- helpers for the program source -----------------------------------------

/// A JSON document as a JavaScript object literal: double quotes become single
/// quotes, so the text can sit inside a program source without escaping.
fn js(json: &str) -> String {
    json.replace('"', "'")
}
