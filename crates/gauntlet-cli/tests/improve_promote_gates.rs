//! `improve promote` gate-input integrity (plan Task 23; COR-1 items C5):
//! the command's condition-4 evidence must be READABLE before any
//! adjudication. A failure is a typed abort — nothing written, the
//! revision NOT rejected — never a silent conversion into "store not at
//! hand" that lets the condition pass on presence alone.
//!
//! Composition: the REAL binary (the e2e pattern — the fixture target
//! project copied per tempdir), the REAL embedded revision ledger, the
//! REAL evidence store. The corruption is the store's own documented
//! refusal shape: an object path that exists but is not a regular file is
//! aliasing, never evidence (`EvidenceStoreError::ReadFailed`).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::EvidenceStore;

/// The real composed binary (same package: `src/bin/gauntlet.rs`).
const GAUNTLET: &str = env!("CARGO_BIN_EXE_gauntlet");

/// The evidence pin's deadline bound — a far-future instant, on its own
/// line (numeric literals never share a line with a duration-shaped
/// identifier).
const PIN_BOUND: u64 = 1_900_000_000;

fn workspace_root() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop();
    path.pop();
    path
}

fn fixture_target() -> PathBuf {
    workspace_root()
        .join("tests")
        .join("fixtures")
        .join("target")
}

/// One scratch world: a fresh copy of the fixture project plus the
/// per-run state/evidence directories under it. Removed on drop.
struct World {
    root: PathBuf,
}

impl World {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "gauntlet-improve-promote-gates-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(&root).expect("the scratch root is creatable");
        copy_tree(&fixture_target(), &root.join("target"));
        let world = Self { root };
        world.ensure_layout();
        world
    }

    fn target(&self) -> PathBuf {
        self.root.join("target")
    }

    fn state_dir(&self) -> PathBuf {
        self.root.join("state")
    }

    fn evidence_dir(&self) -> PathBuf {
        self.root.join("evidence")
    }

    /// The task set's CURRENT configuration: complete (the control run
    /// settles its units within the authored ceilings) with a generous
    /// token ceiling so the scheduler's per-attempt share never binds.
    fn ensure_layout(&self) {
        std::fs::create_dir_all(self.state_dir()).expect("state dir");
        std::fs::create_dir_all(self.evidence_dir()).expect("evidence dir");
        let config = self.target().join("GAUNTLET.md");
        let text = std::fs::read_to_string(&config).expect("the task-set config reads");
        let updated = text.replace("max_tokens: 200000", "max_tokens: 10000000");
        std::fs::write(&config, updated).expect("the task-set config writes");
    }

    fn env(&self) -> Vec<(&'static str, String)> {
        vec![
            ("GAUNTLET_STATE_DIR", self.state_dir().display().to_string()),
            (
                "GAUNTLET_ARTIFACT_DIR",
                self.evidence_dir().display().to_string(),
            ),
            ("GAUNTLET_MOCK_SCENARIO", "e2e-happy".to_owned()),
            ("GAUNTLET_CLOCK", "deterministic".to_owned()),
            ("GAUNTLET_STALL_TIMEOUT_MS", "60000".to_owned()),
            ("GAUNTLET_RETRY_BASE_MS", "100".to_owned()),
            ("GAUNTLET_RETRY_MAX_MS", "2000".to_owned()),
            ("GAUNTLET_MAX_ATTEMPTS_PER_UNIT", "3".to_owned()),
            ("GAUNTLET_TOOL_TEST", self.check_tool()),
        ]
    }

    fn check_tool(&self) -> String {
        let scripts = self.target().join("scripts");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&scripts)
            .expect("the fixture scripts dir reads")
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| path.is_file())
            .collect();
        files.sort();
        files
            .first()
            .expect("the fixture carries a check command")
            .display()
            .to_string()
    }

    fn run(&self, args: &[&str]) -> Output {
        let mut command = Command::new(GAUNTLET);
        command.current_dir(self.target());
        for (key, value) in self.env() {
            command.env(key, value);
        }
        command
            .args(args)
            .output()
            .expect("the gauntlet binary runs")
    }

    /// Pins bytes in the world's REAL evidence store (the §6.2
    /// content-addressed put) and returns the object path the store read.
    fn pin(&self, bytes: &[u8]) -> (String, PathBuf) {
        let store = gauntlet_adapter_evidence_fs::FsEvidenceStore::open(&self.evidence_dir())
            .expect("the evidence store opens");
        let reference = store
            .put(
                bytes,
                gauntlet_domain::EvidenceKind::File,
                Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(PIN_BOUND)),
            )
            .expect("the bytes pin");
        let hex = reference.as_str().trim_start_matches("sha256:");
        let object = self
            .evidence_dir()
            .join("objects")
            .join(&hex[0..2])
            .join(&hex[2..4])
            .join(hex);
        (reference.as_str().to_owned(), object)
    }

    /// Admits an operator-supplied result document; returns the
    /// content-derived revision id.
    fn propose(&self, document: &str) -> String {
        let document_path = self.root.join("candidate-result.json");
        std::fs::write(&document_path, document).expect("the result document writes");
        let output = self.run(&[
            "improve",
            "propose",
            "--from-result",
            document_path.to_str().expect("a plain path"),
        ]);
        let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
        assert!(
            output.status.success(),
            "the proposal admits: stdout={stdout} stderr={}",
            String::from_utf8_lossy(&output.stderr)
        );
        stdout
            .lines()
            .find_map(|line| {
                line.trim()
                    .strip_prefix("improve: revision=")
                    .and_then(|rest| rest.split_whitespace().next())
                    .map(str::to_owned)
            })
            .expect("the proposal prints its revision id")
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn copy_tree(from: &Path, to: &Path) {
    std::fs::create_dir_all(to).expect("the destination dir is creatable");
    for entry in std::fs::read_dir(from).expect("the source dir is readable") {
        let entry = entry.expect("the entry reads");
        let kind = entry.file_type().expect("the file type reads");
        let destination = to.join(entry.file_name());
        if kind.is_dir() {
            copy_tree(&entry.path(), &destination);
        } else {
            std::fs::copy(entry.path(), &destination).expect("the fixture file copies");
        }
    }
}

/// Assembles the optimizer's result document over the given refs.
fn candidate_document(delta_ref: &str, rollback_ref: &str) -> String {
    use gauntlet_ports::json::{JsonNumber, JsonValue};
    let payload = JsonValue::Object(vec![
        (
            "claimed_problem".to_owned(),
            JsonValue::Str("the gate-input integrity probe".to_owned()),
        ),
        (
            "supporting_evidence".to_owned(),
            JsonValue::Array(vec![JsonValue::Str("sha256:feed0000face1111".to_owned())]),
        ),
        (
            "affected_component".to_owned(),
            JsonValue::Str("budget.max_parallel".to_owned()),
        ),
        (
            "predicted_improvement".to_owned(),
            JsonValue::Str("the probe's prediction".to_owned()),
        ),
        (
            "potential_regression".to_owned(),
            JsonValue::Array(vec![JsonValue::Str("the probe's risk".to_owned())]),
        ),
        (
            "candidate_change".to_owned(),
            JsonValue::Str(delta_ref.to_owned()),
        ),
        (
            "evaluation_plan".to_owned(),
            JsonValue::Str("the probe's evaluation plan".to_owned()),
        ),
        (
            "held_out_evaluation".to_owned(),
            JsonValue::Str("the probe's held-out plan".to_owned()),
        ),
        (
            "rollback".to_owned(),
            JsonValue::Str(rollback_ref.to_owned()),
        ),
    ]);
    JsonValue::Object(vec![
        (
            "schema".to_owned(),
            JsonValue::Str(gauntlet_ports::role_result::ROLE_RESULT_SCHEMA.to_owned()),
        ),
        (
            "invocation_id".to_owned(),
            JsonValue::Str("01ARZ3NDEKTSV4RRFFQ69G5FP7".to_owned()),
        ),
        (
            "run_id".to_owned(),
            JsonValue::Str("01ARZ3NDEKTSV4RRFFQ69G5FA3".to_owned()),
        ),
        (
            "iteration_no".to_owned(),
            JsonValue::Number(JsonNumber::Uint(1)),
        ),
        ("role".to_owned(), JsonValue::Str("optimizer".to_owned())),
        (
            "context_envelope_digest".to_owned(),
            JsonValue::Str("sha256:a1b2c3d4e5f6a7b8".to_owned()),
        ),
        ("state".to_owned(), JsonValue::Str("completed".to_owned())),
        (
            "payload_schema".to_owned(),
            JsonValue::Str(gauntlet_ports::role_result::CANDIDATE_REVISION_SCHEMA.to_owned()),
        ),
        ("payload".to_owned(), payload),
        (
            "payment".to_owned(),
            JsonValue::Object(vec![
                (
                    "input_tokens".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(0)),
                ),
                (
                    "output_tokens".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(0)),
                ),
                (
                    "tool_calls".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(0)),
                ),
                (
                    "wall_seconds".to_owned(),
                    JsonValue::Number(JsonNumber::Uint(0)),
                ),
                (
                    "cost_usd".to_owned(),
                    JsonValue::Number(JsonNumber::Float(0.0)),
                ),
            ]),
        ),
        ("unavailable_evidence".to_owned(), JsonValue::Array(vec![])),
        ("errors".to_owned(), JsonValue::Array(vec![])),
    ])
    .to_json_text()
}

fn delta_document(entries: &[(&str, &str)]) -> String {
    use gauntlet_ports::json::JsonValue;
    JsonValue::Array(
        entries
            .iter()
            .map(|(key, value)| {
                JsonValue::Object(vec![
                    ("key".to_owned(), JsonValue::Str((*key).to_owned())),
                    ("value".to_owned(), JsonValue::Str((*value).to_owned())),
                ])
            })
            .collect(),
    )
    .to_json_text()
}

#[test]
fn an_evidence_store_read_failure_aborts_promote_and_writes_nothing() {
    let world = World::new("condition4-read-failure");
    let delta = delta_document(&[("budget.max_parallel", "2")]);
    let rollback = delta_document(&[("budget.max_parallel", "1")]);
    let (delta_ref, _delta_object) = world.pin(delta.as_bytes());
    let (rollback_ref, rollback_object) = world.pin(rollback.as_bytes());
    let revision = world.propose(&candidate_document(&delta_ref, &rollback_ref));

    // THE CORRUPTION: the rollback ref's object path exists but is not a
    // regular file — the store's documented ReadFailed aliasing refusal
    // (not Ok(None): the ref is PRESENT, its evidence is UNREADABLE).
    std::fs::remove_file(&rollback_object).expect("the rollback object removes");
    std::fs::create_dir(&rollback_object).expect("the rollback object path is aliased");

    // THE ABORT: condition 4's evidence must be readable before any
    // adjudication. A store READ FAILURE aborts the command typed —
    // nothing is adjudicated, nothing written — it never becomes "store
    // not at hand" and it never decides on presence alone.
    let output = world.run(&["improve", "promote", &revision]);
    let stdout = String::from_utf8_lossy(&output.stdout).into_owned();
    let stderr = String::from_utf8_lossy(&output.stderr).into_owned();
    assert!(
        !output.status.success(),
        "the abort fails closed: stdout={stdout} stderr={stderr}"
    );
    assert!(
        stderr.contains("improve promote") && stderr.contains("rollback"),
        "the typed abort names the command and the rollback check: {stderr}"
    );
    assert!(
        !stdout.contains("PROMOTED")
            && !stdout.contains("REJECTED")
            && !stdout.contains("PROVISIONAL"),
        "no verdict was adjudicated on unreadable evidence: {stdout}"
    );

    // NOTHING WAS WRITTEN: with the store repaired, the same revision
    // adjudicates from scratch. A written final state (or a rejection)
    // from the aborted run would refuse with the write-once error instead.
    std::fs::remove_dir(&rollback_object).expect("the alias removes");
    std::fs::write(&rollback_object, rollback.as_bytes()).expect("the rollback object restores");
    let repaired = world.run(&["improve", "promote", &revision]);
    let repaired_out = String::from_utf8_lossy(&repaired.stdout).into_owned();
    assert!(
        repaired_out.contains("PROVISIONAL"),
        "with readable evidence the revision adjudicates normally (no evaluation ⇒ \
         provisional): {repaired_out}"
    );
    assert!(
        !repaired_out.contains("write-once past a final state")
            && !repaired_out.contains("AlreadyFinal"),
        "the aborted run wrote no decision: {repaired_out}"
    );
}
