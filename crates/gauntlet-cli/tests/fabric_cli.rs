//! F2b2 proofs for `gauntlet fabric admit-root` and `gauntlet fabric serve`.
//!
//! Hermetic and offline: every test writes its own fabric directory (mock
//! scripted profiles only), answer key, and state directory under the system
//! temp directory. Durable state is the schema the state path keys
//! (the same pg0 substrate the other composition tests use through
//! `GAUNTLET_DATABASE_URL` or the adapter default). The binary is spawned
//! through `CARGO_BIN_EXE_gauntlet`, so these tests drive the real wire.

use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Output, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use gauntlet_adapter_state_postgres::EmbeddedInvocationGraph;
use gauntlet_adapter_surface_rpc::{METHOD_TOOLS_CALL, METHOD_TOOLS_LIST};
use gauntlet_adapter_tool_program_codemode::{DEFAULT_WORKBENCH_DIR, HOST_ENTRY};
use gauntlet_cli::fabric_compose::RunnerToken;
use gauntlet_domain::{EpochSeconds, InvocationId};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::invocation_graph::{InvocationGraph, NodeStatus};
use gauntlet_ports::json::{JsonValue, parse};

const STATE_DB: &str = "gauntlet-state.db";
const RUN: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAA";
const OTHER_RUN: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAB";
const UNIT: &str = "AK-001";
const ROLE: &str = "builder";
const PROFILE: &str = "fabric-scripted";
const HARNESS: &str = "fabric-beta";
const EXECUTABLE: &str = "fabric-beta-exe";
const MISSING_CALLER: &str = "01ARZ3NDEKTSV4RRFFQ69G5FZZ";
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
/// The bound a spawn carries, measured from the call instant.
const SPAWN_BOUND: u64 = 30;
/// The fixture limits: depth, children per parent, and the root's budget ceiling.
const MAX_DEPTH: u32 = 2;
const MAX_CHILDREN: u32 = 2;
const ROOT_ROUNDS: u32 = 5;
const ROOT_WALL: u64 = 600;
const ROOT_TOKENS: u64 = 7000;
const ROOT_COST: u64 = 8100;
/// The request id of the codec probe.
const SMUGGLED_REQUEST_ID: u32 = 41;
/// The span a test deadline reaches past now.
const HORIZON: u64 = 600;

static COUNTER: AtomicU64 = AtomicU64::new(0);

/// One test's directories: fabric config, answer key, state, and workspaces.
struct Fixture {
    base: PathBuf,
    fabric: PathBuf,
    state: PathBuf,
    workspaces: PathBuf,
    answer_key: PathBuf,
}

impl Fixture {
    fn new(tag: &str) -> Self {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let counter = COUNTER.fetch_add(1, Ordering::SeqCst);
        let base = std::env::temp_dir().join(format!(
            "gauntlet-fabric-cli-{tag}-{}-{counter}-{nanos:x}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&base);
        let fixture = Self {
            fabric: base.join("fabric"),
            state: base.join("state"),
            workspaces: base.join("workspaces"),
            answer_key: base.join("answer-key.md"),
            base,
        };
        write(&fixture.fabric.join("limits.toml"), &limits_text());
        write(
            &fixture.fabric.join("harnesses").join("fabric-beta.toml"),
            &harness_toml(HARNESS, EXECUTABLE),
        );
        write(
            &fixture
                .fabric
                .join("profiles")
                .join(format!("{PROFILE}.toml")),
            &format!(
                "schema = 1\nid = \"{PROFILE}\"\nharness = \"{HARNESS}\"\ntoken = \"{}\"\n\
                 scenario = \"happy\"\n",
                RunnerToken::Scripted.token()
            ),
        );
        write(
            &fixture.fabric.join("roles").join("builder.toml"),
            &format!(
                "schema = 1\nrole = \"{ROLE}\"\nmay_spawn = true\nallow = [\"task-statement\"]\n\
                 deny = []\npermissions = \"fabric-permissions\"\nworkspace = \"fresh\"\n"
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

    /// `gauntlet fabric admit-root` with the fixture's paths and `extra` flags.
    fn admit_root(&self, run: &str, extra: &[&str]) -> Output {
        let mut command = gauntlet();
        command.args(["fabric", "admit-root"]);
        command.args(["--state", path(&self.state)]);
        command.args(["--run", run]);
        command.args(["--fabric-dir", path(&self.fabric)]);
        command.args(["--role", ROLE, "--profile", PROFILE, "--unit", UNIT]);
        command.args(["--workspace", path(&self.workspaces)]);
        command.args(["--answer-key", path(&self.answer_key)]);
        command.args(extra);
        command.output().expect("the gauntlet binary runs")
    }

    /// Admits a root and returns its invocation id.
    fn root(&self, run: &str) -> String {
        let output = self.admit_root(run, &[]);
        assert!(output.status.success(), "{}", stderr(&output));
        invocation_line(&output)
    }

    /// Starts `gauntlet fabric serve` for `caller` with stdio pipes.
    fn serve(&self, run: &str, caller: &str) -> Serve {
        self.serve_with(run, caller, &[])
    }

    /// Starts `gauntlet fabric serve` with `extra` flags appended.
    fn serve_with(&self, run: &str, caller: &str, extra: &[&str]) -> Serve {
        let mut child = gauntlet()
            .args(["fabric", "serve"])
            .args(["--state", path(&self.state)])
            .args(["--run", run])
            .args(["--caller", caller])
            .args(["--fabric-dir", path(&self.fabric)])
            .args(["--workspace", path(&self.workspaces)])
            .args(["--answer-key", path(&self.answer_key)])
            .args(extra)
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
}

fn gauntlet() -> Command {
    let exe = env!("CARGO_BIN_EXE_gauntlet");
    Command::new(exe)
}

fn rpc(id: u32, method: &str, params: &str) -> String {
    format!(r#"{{"jsonrpc":"2.0","id":{id},"method":"{method}","params":{params}}}"#)
}

fn spawn_args(key: &str, budget: &Budget) -> String {
    format!(
        r#"{{"role":"{ROLE}","items":[{{"kind":"task-statement","label":"task","content":"do the unit"}}],"budget":{},"idempotency_key":"{key}","deadline_seconds":{SPAWN_BOUND}}}"#,
        budget_json(budget)
    )
}

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

fn deadline() -> Deadline {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Deadline::at_epoch(EpochSeconds::new(now + HORIZON))
}

fn id(text: &str) -> InvocationId {
    InvocationId::new(text).expect("the test id is in the grammar")
}

/// The limits file: a root budget ceiling that holds two children and one
/// grandchild.
fn limits_text() -> String {
    format!(
        "schema = 1\nmax_depth = {MAX_DEPTH}\nmax_children_per_parent = {MAX_CHILDREN}\n\
rounds = {ROOT_ROUNDS}\nwall_seconds = {ROOT_WALL}\ntokens = {ROOT_TOKENS}\n\
cost_usd_micros = {ROOT_COST}\nisolation = \"process\"\n"
    )
}

fn budget_json(budget: &Budget) -> String {
    format!(
        r#"{{"rounds":{},"wall_seconds":{},"tokens":{},"cost_usd_micros":{}}}"#,
        budget.rounds, budget.wall, budget.tokens, budget.cost
    )
}

const ANSWER_KEY: &str = "## AK-001\nclaim: The fabric admits a bounded child.\nverify_by: test\n\
blast_radius: contained\nauthority: critic\n\
evidence_required: {kind: test_report, locator: suite-check-0001, producer: instrument}\n";

fn harness_toml(id: &str, executable: &str) -> String {
    format!(
        "schema = 1\nid = \"{id}\"\nlabel = \"{id} label\"\nexecutables = [\"{executable}\"]\n\n\
         [launch]\nargs = []\nprompt_mode = \"unknown\"\n\n\
         [session]\nidentity = \"none\"\n\n\
         [lifecycle]\nsource = \"screen\"\n\n\
         [control]\ninterrupt = \"none\"\n\n\
         [capabilities]\ninteractive = false\nresumable = false\nmodel_selection = false\n\
         native_subagents = false\nstructured_events = false\nsupport_level = 0\n"
    )
}

fn child_ids(graph: &EmbeddedInvocationGraph, parent: &InvocationId) -> Vec<InvocationId> {
    graph
        .children(parent, deadline())
        .expect("the children read")
        .into_iter()
        .map(|node| node.invocation_id)
        .collect()
}

#[test]
fn admit_root_replay_with_the_same_key_gives_the_same_id_and_a_new_key_a_new_id() {
    let fixture = Fixture::new("replay");
    let first = fixture.admit_root(RUN, &["--idempotency-key", "root-key-1"]);
    assert!(first.status.success(), "{}", stderr(&first));
    let first_id = invocation_line(&first);

    let replay = fixture.admit_root(RUN, &["--idempotency-key", "root-key-1"]);
    assert!(replay.status.success(), "{}", stderr(&replay));
    assert_eq!(invocation_line(&replay), first_id, "same key, same id");

    // The graph admits one root per run (one budget slice), so a second key in
    // the same run is refused rather than admitted as a second root.
    let second_root = fixture.admit_root(RUN, &["--idempotency-key", "root-key-2"]);
    assert!(!second_root.status.success(), "one root per run");
    assert!(
        stderr(&second_root).contains("root"),
        "{}",
        stderr(&second_root)
    );

    // A different key derives a different id in another run.
    let other_run = fixture.admit_root(OTHER_RUN, &["--idempotency-key", "root-key-2"]);
    assert!(other_run.status.success(), "{}", stderr(&other_run));
    assert_ne!(
        invocation_line(&other_run),
        first_id,
        "a different key is a different id"
    );

    let node = fixture
        .graph()
        .node(&id(&first_id), deadline())
        .expect("the graph reads")
        .expect("the root is in the graph");
    assert_eq!(node.parent, None, "a root has no parent");
    assert_eq!(node.depth, 0);
    assert_eq!(node.root, id(&first_id), "a root is its own root");
    assert_eq!(node.status, NodeStatus::Admitted);
}

#[test]
fn serve_lists_the_four_agents_tools_over_stdio() {
    let fixture = Fixture::new("tools");
    let root = fixture.root(RUN);
    let mut serve = fixture.serve(RUN, &root);
    serve.initialize();
    let listing = serve.call(&rpc(2, METHOD_TOOLS_LIST, "{}"));
    for tool in [
        "agents.spawn",
        "agents.status",
        "agents.collect",
        "agents.cancel",
    ] {
        assert!(listing.contains(tool), "{tool} is listed: {listing}");
    }
    let (status, rest) = serve.finish();
    assert!(status.success(), "EOF is a clean shutdown: {status:?}");
    assert!(rest.is_empty(), "no unread protocol frames: {rest:?}");
}

#[test]
fn serve_spawn_admits_a_child_of_the_caller_at_depth_one() {
    let fixture = Fixture::new("spawn");
    let root = fixture.root(RUN);
    let mut serve = fixture.serve(RUN, &root);
    serve.initialize();
    let reply = serve.tool(3, "agents.spawn", &spawn_args("child-1", &CHILD));
    assert!(
        reply.contains("\"isError\":false"),
        "spawn succeeds: {reply}"
    );
    let (status, _) = serve.finish();
    assert!(status.success(), "{status:?}");

    let children = child_ids(&fixture.graph(), &id(&root));
    assert_eq!(children.len(), 1, "one child of the root: {children:?}");
    let child = fixture
        .graph()
        .node(&children[0], deadline())
        .expect("the graph reads")
        .expect("the child is in the graph");
    assert_eq!(child.parent, Some(id(&root)));
    assert_eq!(child.depth, 1);
    assert_eq!(child.root, id(&root));
}

#[test]
fn recursion_across_processes_keeps_the_root_and_refuses_past_the_depth_ceiling() {
    let fixture = Fixture::new("recursion");
    let root = fixture.root(RUN);

    let mut root_serve = fixture.serve(RUN, &root);
    root_serve.initialize();
    let spawned = root_serve.tool(3, "agents.spawn", &spawn_args("c1", &CHILD));
    assert!(spawned.contains("\"isError\":false"), "C1: {spawned}");
    assert!(root_serve.finish().0.success());
    let c1 = child_ids(&fixture.graph(), &id(&root))[0].clone();

    let mut c1_serve = fixture.serve(RUN, c1.as_str());
    c1_serve.initialize();
    let spawned = c1_serve.tool(3, "agents.spawn", &spawn_args("g1", &GRANDCHILD));
    assert!(spawned.contains("\"isError\":false"), "G1: {spawned}");
    assert!(c1_serve.finish().0.success());
    let graph = fixture.graph();
    let g1 = child_ids(&graph, &c1)[0].clone();
    let grandchild = graph
        .node(&g1, deadline())
        .expect("reads")
        .expect("G1 exists");
    assert_eq!(grandchild.parent, Some(c1.clone()));
    assert_eq!(grandchild.depth, 2, "the grandchild sits at the ceiling");
    assert_eq!(grandchild.root, id(&root), "the root is preserved");

    // Depth 3 is past max_depth = 2: a spawn from G1 is refused, nothing admitted.
    let mut g1_serve = fixture.serve(RUN, g1.as_str());
    g1_serve.initialize();
    let refused = g1_serve.tool(3, "agents.spawn", &spawn_args("deep", &GRANDCHILD));
    assert!(
        refused.contains("\"isError\":true"),
        "past the depth ceiling: {refused}"
    );
    assert!(g1_serve.finish().0.success());
    assert!(child_ids(&graph, &g1).is_empty());
}

#[test]
fn the_children_limit_refuses_the_third_child_of_one_parent() {
    let fixture = Fixture::new("children");
    let root = fixture.root(RUN);
    let mut serve = fixture.serve(RUN, &root);
    serve.initialize();
    for (index, key) in ["k1", "k2"].iter().enumerate() {
        let reply = serve.tool(
            10 + index as u32,
            "agents.spawn",
            &spawn_args(key, &GRANDCHILD),
        );
        assert!(reply.contains("\"isError\":false"), "{key}: {reply}");
    }
    let refused = serve.tool(20, "agents.spawn", &spawn_args("k3", &GRANDCHILD));
    assert!(
        refused.contains("\"isError\":true"),
        "the ceiling holds: {refused}"
    );
    assert!(serve.finish().0.success());
    assert_eq!(child_ids(&fixture.graph(), &id(&root)).len(), 2);
}

#[test]
fn a_restarted_serve_reconstructs_the_child_from_durable_state() {
    let fixture = Fixture::new("restart");
    let root = fixture.root(RUN);

    let mut first = fixture.serve(RUN, &root);
    first.initialize();
    let spawned = first.tool(3, "agents.spawn", &spawn_args("persist", &CHILD));
    assert!(spawned.contains("\"isError\":false"), "{spawned}");
    let child = child_ids(&fixture.graph(), &id(&root))[0].clone();
    // The process dies without a clean shutdown: nothing lives in its memory.
    first.child.kill().expect("the process is killed");
    let _ = first.child.wait();

    let mut second = fixture.serve(RUN, &root);
    second.initialize();
    let status = second.tool(
        3,
        "agents.status",
        &format!(r#"{{"invocation_id":"{}"}}"#, child.as_str()),
    );
    assert!(
        status.contains("\"isError\":false"),
        "status reads the child: {status}"
    );
    assert!(
        status.contains(child.as_str()),
        "the child is named: {status}"
    );
    assert!(second.finish().0.success());
}

#[test]
fn serve_refuses_an_unknown_caller_and_a_terminal_caller() {
    let fixture = Fixture::new("refuse");
    let unknown = fixture.serve(RUN, MISSING_CALLER).finish();
    assert!(!unknown.0.success(), "an unknown caller does not start");

    let root = fixture.root(RUN);
    fixture
        .graph()
        .cancel_subtree(&id(&root), EpochSeconds::new(0), deadline())
        .expect("the root is cancelled");
    let terminal = fixture.serve(RUN, &root).finish();
    assert!(!terminal.0.success(), "a terminal caller does not start");
    assert!(terminal.1.is_empty(), "nothing reaches stdout on refusal");

    let wrong_run = fixture.serve(OTHER_RUN, &root).finish();
    assert!(
        !wrong_run.0.success(),
        "a caller of another run does not start"
    );
}

#[test]
fn a_tool_argument_naming_a_caller_or_parent_is_rejected_by_the_strict_codec() {
    let fixture = Fixture::new("codec");
    let root = fixture.root(RUN);
    let mut serve = fixture.serve(RUN, &root);
    serve.initialize();
    let smuggled = serve.tool(
        SMUGGLED_REQUEST_ID,
        "agents.spawn",
        &format!(
            r#"{{"role":"{ROLE}","items":[],"budget":{},"idempotency_key":"x","deadline_seconds":{SPAWN_BOUND},"parent":"{}"}}"#,
            budget_json(&CHILD),
            root
        ),
    );
    assert!(
        smuggled.contains("unknown field"),
        "unknown key rejected: {smuggled}"
    );
    assert!(
        smuggled.contains("\"error\""),
        "a protocol error, not a result: {smuggled}"
    );
    let caller = serve.tool(
        5,
        "agents.status",
        &format!(r#"{{"invocation_id":"{root}","caller":"{MISSING_CALLER}"}}"#),
    );
    assert!(
        caller.contains("unknown field"),
        "caller key rejected: {caller}"
    );
    assert!(serve.finish().0.success());
    assert!(child_ids(&fixture.graph(), &id(&root)).is_empty());
}

#[test]
fn doctor_lists_each_profile_with_an_unknown_model() {
    let fixture = Fixture::new("doctor");
    let reports = fixture.base.join("reports");
    let output = gauntlet()
        .arg("doctor")
        .arg("--fabric-dir")
        .arg(path(&fixture.fabric))
        .env("GAUNTLET_REPORT_DIR", path(&reports))
        .output()
        .expect("the gauntlet binary runs");
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        stdout.contains(&format!("fabric: profile {PROFILE} harness {HARNESS}")),
        "the profile is listed: {stdout}"
    );
    assert!(
        stdout.contains("model UNKNOWN"),
        "the unset model is UNKNOWN: {stdout}"
    );
}

// ---- `program.run` over stdio (Phase I slice 1a) ----

const PROGRAM_RUN: &str = "program.run";

/// The repository's workbench, located from this crate's manifest. These tests
/// run the real host, so they fail rather than skip when its entry is missing.
fn workbench_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .map(|dir| dir.join(DEFAULT_WORKBENCH_DIR))
        .find(|dir| dir.join(HOST_ENTRY).is_file())
        .unwrap_or_else(|| panic!("the codemode host entry ({HOST_ENTRY}) is missing"))
}

/// Starts a serve process with the program host directory given.
fn serve_with_program(fixture: &Fixture, caller: &str) -> Serve {
    let host = workbench_dir();
    fixture.serve_with(RUN, caller, &["--program-host-dir", path(&host)])
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

/// A JSON document as a JavaScript object literal: double quotes become single
/// quotes, so the text can sit inside a program source without escaping.
fn js(json: &str) -> String {
    json.replace('"', "'")
}

#[test]
fn program_run_is_listed_only_when_a_host_directory_is_given() {
    let fixture = Fixture::new("program-listing");
    let root = fixture.root(RUN);

    let mut without = fixture.serve(RUN, &root);
    without.initialize();
    let listing = without.call(&rpc(2, METHOD_TOOLS_LIST, "{}"));
    assert!(
        !listing.contains(PROGRAM_RUN),
        "absent without a host directory: {listing}"
    );
    assert!(without.finish().0.success());

    let mut with = serve_with_program(&fixture, &root);
    with.initialize();
    let listing = with.call(&rpc(2, METHOD_TOOLS_LIST, "{}"));
    assert!(
        listing.contains(PROGRAM_RUN) && listing.contains("agents.spawn"),
        "listed with a host directory, beside the agents tools: {listing}"
    );
    let (status, rest) = with.finish();
    assert!(status.success() && rest.is_empty(), "{status:?} {rest:?}");
}

#[test]
fn a_program_reads_the_same_data_as_a_direct_call_and_the_trace_records_it() {
    let fixture = Fixture::new("program-list");
    let root = fixture.root(RUN);
    let mut serve = serve_with_program(&fixture, &root);
    serve.initialize();
    let spawned = serve.tool(3, "agents.spawn", &spawn_args("listed-child", &CHILD));
    assert!(spawned.contains("\"isError\":false"), "{spawned}");

    let direct = payload(&serve.tool(4, "agents.list", "{}"));
    let reply = serve.tool(
        5,
        PROGRAM_RUN,
        r#"{"source":"return await tools.agents.list({});"}"#,
    );
    let run = payload(&reply);
    assert_eq!(text_field(&run, "status"), "completed", "{reply}");
    assert_eq!(
        run.get("value"),
        Some(&direct),
        "the same data as a direct call"
    );
    let JsonValue::Array(trace) = run.get("trace").expect("a trace") else {
        panic!("the trace is an array: {run:?}")
    };
    assert_eq!(trace.len(), 1, "one inner call: {trace:?}");
    assert_eq!(text_field(&trace[0], "tool"), "agents.list");
    assert_eq!(text_field(&trace[0], "outcome"), "ok");
    assert!(serve.finish().0.success());
}

#[test]
fn a_program_cannot_name_program_run_or_a_tool_outside_its_allow_list() {
    let fixture = Fixture::new("program-names");
    let root = fixture.root(RUN);
    let mut serve = serve_with_program(&fixture, &root);
    serve.initialize();

    let no_namespace = payload(&serve.tool(
        3,
        PROGRAM_RUN,
        r#"{"source":"return typeof tools.program;"}"#,
    ));
    assert_eq!(
        no_namespace.get("value"),
        Some(&JsonValue::Str("undefined".to_owned())),
        "program.run is not a name inside a program"
    );

    let unlisted = payload(&serve.tool(
        4,
        PROGRAM_RUN,
        r#"{"source":"return typeof tools.agents.spawn;","allow":["agents.list"]}"#,
    ));
    assert_eq!(
        unlisted.get("value"),
        Some(&JsonValue::Str("undefined".to_owned())),
        "a tool outside allow is undefined"
    );

    let naming_itself = serve.tool(
        5,
        PROGRAM_RUN,
        r#"{"source":"return 1;","allow":["program.run"]}"#,
    );
    assert!(
        naming_itself.contains("\"error\""),
        "allow cannot name program.run: {naming_itself}"
    );
    assert!(serve.finish().0.success());
    assert!(child_ids(&fixture.graph(), &id(&root)).is_empty());
}

#[test]
fn a_program_spawn_creates_the_child_under_the_caller() {
    let fixture = Fixture::new("program-spawn");
    let root = fixture.root(RUN);
    let mut serve = serve_with_program(&fixture, &root);
    serve.initialize();
    let source = format!(
        "return await tools.agents.spawn({});",
        js(&spawn_args("program-child", &CHILD))
    );
    let reply = serve.tool(3, PROGRAM_RUN, &format!(r#"{{"source":"{source}"}}"#));
    let run = payload(&reply);
    assert_eq!(text_field(&run, "status"), "completed", "{reply}");
    assert!(serve.finish().0.success());

    let children = child_ids(&fixture.graph(), &id(&root));
    assert_eq!(children.len(), 1, "one child of the caller: {children:?}");
    let child = fixture
        .graph()
        .node(&children[0], deadline())
        .expect("the graph reads")
        .expect("the child is in the graph");
    assert_eq!(child.parent, Some(id(&root)), "the caller is the parent");
    assert_eq!(child.root, id(&root));
    assert_eq!(child.depth, 1);
}

#[test]
fn program_run_rejects_caller_and_parent_arguments() {
    let fixture = Fixture::new("program-codec");
    let root = fixture.root(RUN);
    let mut serve = serve_with_program(&fixture, &root);
    serve.initialize();
    let caller = serve.tool(
        3,
        PROGRAM_RUN,
        &format!(r#"{{"source":"return 1;","caller":"{MISSING_CALLER}"}}"#),
    );
    assert!(caller.contains("\"error\""), "caller refused: {caller}");
    let parent = serve.tool(
        4,
        PROGRAM_RUN,
        &format!(r#"{{"source":"return 1;","parent":"{root}"}}"#),
    );
    assert!(parent.contains("\"error\""), "parent refused: {parent}");
    assert!(serve.finish().0.success());
    assert!(child_ids(&fixture.graph(), &id(&root)).is_empty());
}

/// The operator's program timeout ceiling, and a request far above it.
const PROGRAM_CEILING: u64 = 300;
const REQUESTED: u64 = 100_000_000;

#[test]
fn the_program_timeout_is_clamped_to_the_operator_ceiling() {
    let fixture = Fixture::new("program-timeout");
    write(
        &fixture.fabric.join("limits.toml"),
        &format!("{}program_timeout_ms = {PROGRAM_CEILING}\n", limits_text()),
    );
    let root = fixture.root(RUN);
    let mut serve = serve_with_program(&fixture, &root);
    serve.initialize();
    // The request asks for far more than the ceiling, which is what binds.
    let arguments = format!(r#"{{"source":"while (true) {{}}","timeout_ms":{REQUESTED}}}"#);
    let reply = serve.tool(3, PROGRAM_RUN, &arguments);
    let run = payload(&reply);
    assert_eq!(text_field(&run, "status"), "timed_out", "{reply}");
    assert!(serve.finish().0.success());
}

#[test]
fn a_completed_program_changes_no_invocation_status() {
    let fixture = Fixture::new("program-settle");
    let root = fixture.root(RUN);
    let before = fixture
        .graph()
        .node(&id(&root), deadline())
        .expect("the graph reads")
        .expect("the root is in the graph")
        .status;
    let mut serve = serve_with_program(&fixture, &root);
    serve.initialize();
    let reply = serve.tool(
        3,
        PROGRAM_RUN,
        r#"{"source":"return { settled: true, completed: true };"}"#,
    );
    let run = payload(&reply);
    assert_eq!(text_field(&run, "status"), "completed", "{reply}");
    assert!(serve.finish().0.success());

    let after = fixture
        .graph()
        .node(&id(&root), deadline())
        .expect("the graph reads")
        .expect("the root is in the graph")
        .status;
    assert_eq!(after, before, "a program value is data, not settlement");
    assert!(child_ids(&fixture.graph(), &id(&root)).is_empty());
}
