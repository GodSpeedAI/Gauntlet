//! The mock-runner scenario frontend (plan Task 8; AK-059's frozen locator
//! `tests/fixtures/mock_runner_scenarios`): a strict, purpose-built
//! YAML-subset parser that turns a fixture file into a typed [`Scenario`]
//! of CHILD behavior, plus the renderer that turns a scenario + dispatch
//! facts into a `role_result@v1` result document.
//!
//! This is NOT a YAML implementation (same posture as the manifest reader
//! in `gauntlet-app::reference::manifest`): the dialect is small, the
//! shapes are known, and drift fails loudly with the line — an unknown
//! key, an unexpected shape, or a duplicate key is a typed error, never a
//! skipped field.
//!
//! ## The dialect
//!
//! One scenario per file; values are plain scalars, flow sequences of
//! scalars (`[a, b]`, `[]`), block sequences of scalars (`- text`), or
//! block sequences of mappings (`- key: value` with continuation keys two
//! columns deeper). Full-line `#` comments and blank lines are dropped;
//! inline `#` is content. A tab in the indentation is drift. A block item
//! containing `: ` or ending in `:` is a mapping item; scalar items must
//! not carry an inline colon-space.
//!
//! ```yaml
//! scenario: <token>              # REQUIRED — the scenario's identity
//! role: <role-id>                # REQUIRED — the §5.2 role the child speaks as
//! behavior: write-result|stall   # REQUIRED — what the child does
//! probes_before_write: <0..=23>  # write-result only; OPTIONAL, default 0
//!                                # (a write child terminates one poll after
//!                                #  its last in-flight probe, so 23 keeps
//!                                #  the terminal poll inside 24)
//! timeout_after_probes: <1..=24> # stall only; REQUIRED (times out ON its probe)
//! state: completed|failed        # write-result only; REQUIRED
//! derailment: none|raw-malformed|unknown-envelope-field|unknown-nested-field|foreign-invocation-id|stale-envelope-digest
//!                                # write-result only; OPTIONAL, default none
//! payload:                       # write-result only; REQUIRED
//!   attempted: [AK-001]          # flow seq of §5.2 unit ids
//!   changed_paths: []            # every key below is REQUIRED, even empty
//!   commands_run: []
//!   artifacts:                   # seq of {path, evidence_ref} mappings
//!     - path: out.txt
//!       evidence_ref: evd-000042
//!   claims:                      # seq of strings (block form avoids commas)
//!     - the work is done
//!   self_checks:                 # seq of {command, exit_code} mappings
//!     - command: check.sh
//!       exit_code: 0
//!   blocked_on: []
//! workspace_effects:             # write-result only; OPTIONAL
//!   - path: out.txt              # files the child writes under the
//!     content: FAIL              # workspace root (relative, no `..`)
//! ```
//!
//! All seven `payload` keys are REQUIRED, even when empty — explicit
//! emptiness is the point of the P11-shaped fixtures (a critic that claims
//! `pass` with empty evidence arrays must SAY the arrays are empty).
//!
//! A scenario encodes CHILD behavior only. It never encodes an
//! orchestration decision — the mock returns results; it does not decide
//! anything (plan Task 8's redesign trigger). The two knobs that look
//! runtime-ish are honest child-modeling: `probes_before_write` models
//! work in flight at admission, and `timeout_after_probes` models a child
//! silent past its deadline — `poll` never blocks on real time, so probe
//! counts are the deterministic surrogate for elapsed virtual time.
//!
//! ## The rendered document
//!
//! [`Scenario::result_document_text`] builds the `role_result@v1`
//! envelope (spec Annex B.1) field-for-field from typed parts with the
//! dispatch identity echoed exactly — the envelope is valid BY
//! CONSTRUCTION except where a scenario deliberately derails it
//! ([`Derailment`]): a foreign invocation id, a stale envelope digest, an
//! injected unknown envelope field, an unknown field nested inside
//! `self_checks[0]`, or raw malformed text replacing the document. The
//! envelope's `payment` is zeroed and `unavailable_evidence`/`errors` are
//! empty — the dialect deliberately carries no knobs for them (no
//! scenario needs one); everything a fixture CAN say is typed.

use core::fmt;

use gauntlet_domain::{EvidenceRef, RoleId, UnitId, ValueError};
use gauntlet_ports::agent_runner::RoleInvocationSpec;
use gauntlet_ports::json::{JsonNumber, JsonValue};
use gauntlet_ports::role_result::{BUILDER_RESULT_SCHEMA, ROLE_RESULT_SCHEMA, RoleResultState};

/// The drivers' probe budget and the STALL knob's inclusive ceiling:
/// every driver in the repository stops at 24 polls (the shared suite's
/// INSTANT budget and the playback tests' bounded loop alike). The stall
/// child times out ON its probe (`timeout <= probes`), so 24 is exactly
/// drivable. With [`MAX_SCENARIO_DEPTH`] this keeps the runaway-memory
/// lesson's claim true by construction: every traversal and every
/// scripted advance loop is bounded, with its bound named.
const PROBE_CEILING: u8 = 24;

/// The WRITE knob's inclusive ceiling: a `write-result` child terminates
/// one poll AFTER its last in-flight probe (polls `1..=N` report
/// Running, poll `N+1` writes), so the largest value whose terminal poll
/// still lands inside the drivers' [`PROBE_CEILING`]-poll budget is one
/// less — derived, not a second literal to keep in step. Legal means
/// drivable, per knob.
const WRITE_PROBE_CEILING: u8 = PROBE_CEILING - 1;

/// The recursion cap for the descent (`value_of` / `mapping` /
/// `block_sequence` / `dash_mapping_item`): a staircase deeper than this
/// is a typed [`ScenarioError::DepthExceeded`], never a stack-overflow
/// abort — the same posture as the ports json reader's `MAX_DEPTH` (also
/// 64). The runaway-memory lesson covers DEPTH as well as termination:
/// the descent's stack and memory are bounded, with the bound named.
const MAX_SCENARIO_DEPTH: usize = 64;

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

/// Scenario failures — typed, each carrying the 1-based line of the drift
/// wherever a physical line exists.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ScenarioError {
    /// A line where a `key: value` mapping entry was required.
    NotAMapping { line: usize },
    /// A key outside the dialect (per level: root, payload, item).
    UnknownKey { line: usize, key: String },
    /// A key appears twice in one mapping.
    DuplicateKey { line: usize, key: String },
    /// A value is not a shape the dialect carries.
    BadShape {
        line: usize,
        key: String,
        expected: &'static str,
        got: &'static str,
    },
    /// A scalar that should be an integer is not one.
    NotAnInteger {
        line: usize,
        key: String,
        value: String,
    },
    /// An integer outside its declared bound — each knob carries its own
    /// inclusive range (`probes_before_write` 0..=23,
    /// `timeout_after_probes` 1..=24); the error names the knob's real
    /// range, not a shared one.
    IntegerOutOfRange {
        line: usize,
        key: String,
        value: String,
        min: u8,
        max: u8,
    },
    /// A token outside its closed vocabulary.
    UnknownToken {
        line: usize,
        key: String,
        value: String,
        vocabulary: &'static str,
    },
    /// A typed value (role, unit id, evidence ref, effect path) failed
    /// domain validation.
    ValueInvalid {
        line: usize,
        key: String,
        detail: String,
    },
    /// A required key never appeared at the level that requires it.
    MissingKey {
        level: &'static str,
        key: &'static str,
    },
    /// A field is illegal under the declared behavior (or a behavior's
    /// required field is absent).
    BehaviorConflict { line: usize, detail: String },
    /// A sequence item, flow entry, or indentation run the dialect cannot
    /// accept.
    MalformedItem { line: usize, detail: String },
    /// A line whose indentation no level of the document can place — the
    /// traversal guarantees every line is consumed or named.
    UnplaceableLine { line: usize, indent: usize },
    /// The document nests deeper than the parser's depth bound — a typed
    /// refusal instead of stack exhaustion.
    DepthExceeded { line: usize, depth: usize },
    /// The fixture file could not be read.
    FixtureUnreadable { name: String, detail: String },
    /// The fixture's `scenario:` field disagrees with its file stem.
    NameMismatch { stem: String, scenario: String },
}

impl fmt::Display for ScenarioError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotAMapping { line } => {
                write!(f, "line {line}: a `key: value` mapping entry was required")
            }
            Self::UnknownKey { line, key } => {
                write!(
                    f,
                    "line {line}: key {key:?} is outside the scenario dialect"
                )
            }
            Self::DuplicateKey { line, key } => {
                write!(f, "line {line}: key {key:?} appears twice in one mapping")
            }
            Self::BadShape {
                line,
                key,
                expected,
                got,
            } => write!(f, "line {line}: key {key:?} must be {expected}, got {got}"),
            Self::NotAnInteger { line, key, value } => {
                write!(
                    f,
                    "line {line}: key {key:?} value {value:?} is not an integer"
                )
            }
            Self::IntegerOutOfRange {
                line,
                key,
                value,
                min,
                max,
            } => write!(
                f,
                "line {line}: key {key:?} value {value:?} is outside {min}..={max}"
            ),
            Self::UnknownToken {
                line,
                key,
                value,
                vocabulary,
            } => write!(
                f,
                "line {line}: key {key:?} value {value:?} is not one of {vocabulary}"
            ),
            Self::ValueInvalid { line, key, detail } => {
                write!(f, "line {line}: key {key:?}: {detail}")
            }
            Self::MissingKey { level, key } => {
                write!(f, "the {level} is missing its required key {key:?}")
            }
            Self::BehaviorConflict { line, detail } => {
                write!(f, "line {line}: behavior conflict: {detail}")
            }
            Self::MalformedItem { line, detail } => {
                write!(f, "line {line}: a malformed item: {detail}")
            }
            Self::UnplaceableLine { line, indent } => write!(
                f,
                "line {line}: indentation of {indent} spaces cannot be placed in the \
                 document — the dialect has one root mapping"
            ),
            Self::DepthExceeded { line, depth } => write!(
                f,
                "line {line}: nesting depth {depth} exceeds the scenario parser's bound \
                 of {MAX_SCENARIO_DEPTH} — rejected, not traversed"
            ),
            Self::FixtureUnreadable { name, detail } => {
                write!(f, "fixture {name:?} could not be read: {detail}")
            }
            Self::NameMismatch { stem, scenario } => write!(
                f,
                "fixture file {stem:?} declares scenario {scenario:?} — the stem and the \
                 identity must agree"
            ),
        }
    }
}

impl std::error::Error for ScenarioError {}

// ---------------------------------------------------------------------------
// The scenario model
// ---------------------------------------------------------------------------

/// What the scripted child does (the dialect's `behavior:`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildBehavior {
    /// The child writes a result document (then possibly derails it).
    WriteResult,
    /// The child stays silent forever; the runtime deadline eventually
    /// fires (modeled as a probe count — `poll` never blocks on real
    /// time).
    Stall,
    /// Plan Task 16, never-write shape 2: the child has EXITED without
    /// ever writing a result document. A dead child can never write, so
    /// the mock pins `Failed{missing_result}` on the first probe —
    /// decided before any deadline, unlike [`ChildBehavior::Stall`]'s
    /// deadline verdict.
    ExitWithoutResult,
}

impl ChildBehavior {
    /// The wire tokens, for the error vocabulary.
    const TOKENS: &'static str = "write-result | stall | exit-without-result";

    fn parse(value: &str) -> Option<Self> {
        match value {
            "write-result" => Some(Self::WriteResult),
            "stall" => Some(Self::Stall),
            "exit-without-result" => Some(Self::ExitWithoutResult),
            _ => None,
        }
    }
}

/// How a `write-result` child's document is deliberately derailed. Every
/// variant is CHILD behavior (a confused, lying, or broken child); the
/// runner's classification of the derailed document is the runtime's
/// business (§10.3 rules 4 and 6).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Derailment {
    /// The document is well-formed and echoes dispatch identity exactly.
    None,
    /// The written text is not JSON at all (§10.3 rule 4).
    RawMalformed,
    /// One unknown field rides the envelope (strict decode rejects it).
    UnknownEnvelopeField,
    /// One unknown field rides inside `self_checks[0]` (strict decode
    /// rejects it at every object layer).
    UnknownNestedField,
    /// The document carries a foreign — but grammar-valid —
    /// `invocation_id` (§10.3 rule 6).
    ForeignInvocationId,
    /// The document carries a stale — but well-formed —
    /// `context_envelope_digest` (§10.3 rule 6).
    StaleEnvelopeDigest,
}

impl Derailment {
    /// The wire tokens, for the error vocabulary.
    const TOKENS: &'static str = "none | raw-malformed | unknown-envelope-field | \
         unknown-nested-field | foreign-invocation-id | stale-envelope-digest";

    fn parse(value: &str) -> Option<Self> {
        match value {
            "none" => Some(Self::None),
            "raw-malformed" => Some(Self::RawMalformed),
            "unknown-envelope-field" => Some(Self::UnknownEnvelopeField),
            "unknown-nested-field" => Some(Self::UnknownNestedField),
            "foreign-invocation-id" => Some(Self::ForeignInvocationId),
            "stale-envelope-digest" => Some(Self::StaleEnvelopeDigest),
            _ => None,
        }
    }
}

/// One artifact claim a scripted child makes (Annex B.2 `artifacts`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArtifactPart {
    path: String,
    evidence_ref: String,
}

impl ArtifactPart {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn evidence_ref(&self) -> &str {
        &self.evidence_ref
    }
}

/// One self-check a scripted child reports (Annex B.2 `self_checks`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelfCheckPart {
    command: String,
    exit_code: i64,
}

impl SelfCheckPart {
    pub fn command(&self) -> &str {
        &self.command
    }

    pub fn exit_code(&self) -> i64 {
        self.exit_code
    }
}

/// The typed `builder_result@v1` parts (Annex B.2), field for field.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PayloadParts {
    attempted: Vec<String>,
    changed_paths: Vec<String>,
    commands_run: Vec<String>,
    artifacts: Vec<ArtifactPart>,
    claims: Vec<String>,
    self_checks: Vec<SelfCheckPart>,
    blocked_on: Vec<String>,
}

impl PayloadParts {
    pub fn attempted(&self) -> &[String] {
        &self.attempted
    }

    pub fn changed_paths(&self) -> &[String] {
        &self.changed_paths
    }

    pub fn commands_run(&self) -> &[String] {
        &self.commands_run
    }

    pub fn artifacts(&self) -> &[ArtifactPart] {
        &self.artifacts
    }

    pub fn claims(&self) -> &[String] {
        &self.claims
    }

    pub fn self_checks(&self) -> &[SelfCheckPart] {
        &self.self_checks
    }

    pub fn blocked_on(&self) -> &[String] {
        &self.blocked_on
    }

    /// The minimal well-formed payload the `RunnerScript` mapping uses for
    /// suite-driven children: one attempted unit, one forensic claim, no
    /// evidence. Crate-internal — suite mapping only, never a fixture
    /// shortcut.
    pub(crate) fn scripted() -> Self {
        Self {
            attempted: vec!["AK-014".to_owned()],
            changed_paths: Vec::new(),
            commands_run: Vec::new(),
            artifacts: Vec::new(),
            claims: vec!["the scripted child finished the assigned slice".to_owned()],
            self_checks: Vec::new(),
            blocked_on: Vec::new(),
        }
    }
}

/// One file the scripted child writes under the workspace root.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceEffect {
    path: String,
    content: String,
}

impl WorkspaceEffect {
    pub fn path(&self) -> &str {
        &self.path
    }

    pub fn content(&self) -> &str {
        &self.content
    }
}

/// A parsed scenario: everything the mock runner needs to replay one
/// scripted child. Child behavior only — never an orchestration decision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Scenario {
    name: String,
    role: RoleId,
    behavior: ChildBehavior,
    probes_before_write: u8,
    timeout_after_probes: Option<u8>,
    state: Option<RoleResultState>,
    derailment: Derailment,
    payload: Option<PayloadParts>,
    workspace_effects: Vec<WorkspaceEffect>,
}

impl Scenario {
    pub fn name(&self) -> &str {
        &self.name
    }

    pub fn role(&self) -> &RoleId {
        &self.role
    }

    pub fn behavior(&self) -> ChildBehavior {
        self.behavior
    }

    pub fn probes_before_write(&self) -> u8 {
        self.probes_before_write
    }

    pub fn timeout_after_probes(&self) -> Option<u8> {
        self.timeout_after_probes
    }

    pub fn state(&self) -> Option<RoleResultState> {
        self.state
    }

    pub fn derailment(&self) -> Derailment {
        self.derailment
    }

    pub fn payload(&self) -> Option<&PayloadParts> {
        self.payload.as_ref()
    }

    pub fn workspace_effects(&self) -> &[WorkspaceEffect] {
        &self.workspace_effects
    }

    /// Crate-internal assembly, used by the parser and by the
    /// `RunnerScript` mapping in [`crate::runner`]: both construct
    /// scenarios; neither bypasses validation (the parser validates text,
    /// the mapping constructs already-valid parts).
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn assemble(
        name: String,
        role: RoleId,
        behavior: ChildBehavior,
        probes_before_write: u8,
        timeout_after_probes: Option<u8>,
        state: Option<RoleResultState>,
        derailment: Derailment,
        payload: Option<PayloadParts>,
        workspace_effects: Vec<WorkspaceEffect>,
    ) -> Self {
        Self {
            name,
            role,
            behavior,
            probes_before_write,
            timeout_after_probes,
            state,
            derailment,
            payload,
            workspace_effects,
        }
    }

    /// Renders the result-document text this child writes, echoing the
    /// dispatch identity of `spec` exactly and applying the scenario's
    /// derailment. The envelope is valid by construction (Annex B.1
    /// field-for-field, Annex B.2 payload) except where derailed. Only a
    /// `write-result` scenario has a document; a `stall` scenario never
    /// renders (and would render state `completed` if a future caller
    /// forced it — `unwrap_or`, not `unwrap`, keeps the function total).
    pub fn result_document_text(&self, spec: &RoleInvocationSpec) -> String {
        if self.derailment == Derailment::RawMalformed {
            // Not JSON: a child that wrote prose (or partial output) where
            // the contract demanded a typed document.
            return format!(
                "{{scenario {}: the child wrote this instead of a result document}}",
                self.name
            );
        }
        let payload = self.render_payload();
        let mut entries: Vec<(String, JsonValue)> = vec![
            (
                "schema".to_owned(),
                JsonValue::Str(ROLE_RESULT_SCHEMA.to_owned()),
            ),
            (
                "invocation_id".to_owned(),
                JsonValue::Str(self.invocation_id_value(spec)),
            ),
            ("run_id".to_owned(), JsonValue::Str(spec.run_id.to_string())),
            (
                "iteration_no".to_owned(),
                JsonValue::Number(JsonNumber::Uint(u64::from(spec.iteration_no.get()))),
            ),
            (
                "role".to_owned(),
                JsonValue::Str(self.role.name().to_owned()),
            ),
            (
                "context_envelope_digest".to_owned(),
                JsonValue::Str(self.digest_value(spec)),
            ),
            (
                "state".to_owned(),
                JsonValue::Str(self.state.unwrap_or(RoleResultState::Completed).to_string()),
            ),
            (
                "payload_schema".to_owned(),
                JsonValue::Str(BUILDER_RESULT_SCHEMA.to_owned()),
            ),
            ("payload".to_owned(), payload),
            ("payment".to_owned(), zero_payment()),
            ("unavailable_evidence".to_owned(), JsonValue::Array(vec![])),
            ("errors".to_owned(), JsonValue::Array(vec![])),
        ];
        if self.derailment == Derailment::UnknownEnvelopeField {
            entries.push((
                "fabricated_envelope_field".to_owned(),
                JsonValue::Str("the child added this".to_owned()),
            ));
        }
        JsonValue::Object(entries).to_json_text()
    }

    /// The `invocation_id` the document carries: the dispatched id, or —
    /// under [`Derailment::ForeignInvocationId`] — a different but
    /// grammar-valid ULID (cross-wired child, §10.3 rule 6). The flip is
    /// alphabet-safe by construction ('0' ↔ '1' are both Crockford) and
    /// differs from the dispatched id on either branch.
    fn invocation_id_value(&self, spec: &RoleInvocationSpec) -> String {
        let dispatched = spec.invocation_id.as_str();
        if self.derailment != Derailment::ForeignInvocationId {
            return dispatched.to_owned();
        }
        flip_last_char(dispatched)
    }

    /// The `context_envelope_digest` the document carries: the dispatched
    /// envelope's digest, or — under [`Derailment::StaleEnvelopeDigest`]
    /// — a different but well-formed digest (§10.3 rule 6). Flipping the
    /// first hex digit between '0' and '1' keeps it lowercase hex of the
    /// same even length, different by construction.
    fn digest_value(&self, spec: &RoleInvocationSpec) -> String {
        let digest = spec.context_envelope.digest();
        if self.derailment != Derailment::StaleEnvelopeDigest {
            return digest.to_string();
        }
        let text = digest.to_string();
        let hex_start = text.find(':').map(|position| position + 1).unwrap_or(0);
        flip_char_at(&text, hex_start)
    }

    /// The Annex B.2 payload object from typed parts, applying the
    /// nested-field derailment when scripted.
    fn render_payload(&self) -> JsonValue {
        let parts = self.payload.as_ref();
        let strings = |values: &[String]| {
            JsonValue::Array(
                values
                    .iter()
                    .map(|value| JsonValue::Str(value.clone()))
                    .collect(),
            )
        };
        let empty: &[String] = &[];
        let artifact_objects: Vec<JsonValue> = parts
            .map(|p| p.artifacts())
            .unwrap_or(&[])
            .iter()
            .map(|artifact| {
                JsonValue::Object(vec![
                    ("path".to_owned(), JsonValue::Str(artifact.path.clone())),
                    (
                        "evidence_ref".to_owned(),
                        JsonValue::Str(artifact.evidence_ref.clone()),
                    ),
                ])
            })
            .collect();
        let mut check_objects: Vec<JsonValue> = parts
            .map(|p| p.self_checks())
            .unwrap_or(&[])
            .iter()
            .map(self_check_object)
            .collect();
        if self.derailment == Derailment::UnknownNestedField {
            // Ride one unknown field inside self_checks[0] — substituting
            // a placeholder entry when the scenario scripted none, exactly
            // like a real child that mangles its only check.
            if check_objects.is_empty() {
                check_objects.push(self_check_object(&SelfCheckPart {
                    command: "placeholder-check".to_owned(),
                    exit_code: 0,
                }));
            }
            if let Some(JsonValue::Object(fields)) = check_objects.first_mut() {
                fields.push((
                    "fabricated_nested_field".to_owned(),
                    JsonValue::Str("the child added this".to_owned()),
                ));
            }
        }
        JsonValue::Object(vec![
            (
                "attempted".to_owned(),
                strings(parts.map(|p| p.attempted()).unwrap_or(empty)),
            ),
            (
                "changed_paths".to_owned(),
                strings(parts.map(|p| p.changed_paths()).unwrap_or(empty)),
            ),
            (
                "commands_run".to_owned(),
                strings(parts.map(|p| p.commands_run()).unwrap_or(empty)),
            ),
            ("artifacts".to_owned(), JsonValue::Array(artifact_objects)),
            (
                "claims".to_owned(),
                strings(parts.map(|p| p.claims()).unwrap_or(empty)),
            ),
            ("self_checks".to_owned(), JsonValue::Array(check_objects)),
            (
                "blocked_on".to_owned(),
                strings(parts.map(|p| p.blocked_on()).unwrap_or(empty)),
            ),
        ])
    }
}

/// One rendered self-check object. Exit codes ride the wire as unsigned
/// magnitudes; negative scripted codes carry a minus sign in the string
/// form (the ports reader accepts signed integers).
fn self_check_object(check: &SelfCheckPart) -> JsonValue {
    let exit = if check.exit_code < 0 {
        JsonValue::Number(JsonNumber::NegInt(check.exit_code))
    } else {
        JsonValue::Number(JsonNumber::Uint(check.exit_code.unsigned_abs()))
    };
    JsonValue::Object(vec![
        ("command".to_owned(), JsonValue::Str(check.command.clone())),
        ("exit_code".to_owned(), exit),
    ])
}

/// The zeroed payment object (Annex B.1 shape; the dialect carries no
/// payment knob — no scenario needs one). The wall-clock key is
/// referenced, not written: no scan unit pairs a duration-shaped
/// identifier with a numeric literal (§8.1).
fn zero_payment() -> JsonValue {
    const WALL_SECONDS_KEY: &str = "wall_seconds";
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
            WALL_SECONDS_KEY.to_owned(),
            JsonValue::Number(JsonNumber::Uint(0)),
        ),
        (
            "cost_usd".to_owned(),
            JsonValue::Number(JsonNumber::Uint(0)),
        ),
    ])
}

/// Flips the ASCII byte at `index` between '0' and '1' — a value-safe,
/// guaranteed-different mutation for Crockford ULID tails ('0'→'1',
/// anything else ASCII→'0', both in-alphabet) and lowercase hex alike.
/// A non-ASCII byte (unreachable for domain-validated ULIDs and digests)
/// is left untouched rather than corrupted.
fn flip_char_at(text: &str, index: usize) -> String {
    let mut bytes = text.as_bytes().to_vec();
    if let Some(byte) = bytes.get(index)
        && byte.is_ascii()
    {
        bytes[index] = if *byte == b'0' { b'1' } else { b'0' };
    }
    // Only an ASCII byte was swapped for another ASCII byte, so the text
    // is still valid UTF-8; lossy is the identity here and never panics.
    String::from_utf8_lossy(&bytes).into_owned()
}

/// Flips the final character of a ULID between '0' and '1'.
fn flip_last_char(text: &str) -> String {
    flip_char_at(text, text.len().saturating_sub(1))
}

// ---------------------------------------------------------------------------
// The tree: a strict YAML-subset value
// ---------------------------------------------------------------------------

struct PhysLine {
    number: usize,
    indent: usize,
    text: String,
}

enum Node {
    Scalar {
        line: usize,
        value: String,
    },
    Seq {
        line: usize,
        items: Vec<Node>,
    },
    Map {
        line: usize,
        entries: Vec<(String, Node)>,
    },
}

impl Node {
    fn line(&self) -> usize {
        match self {
            Self::Scalar { line, .. } | Self::Seq { line, .. } | Self::Map { line, .. } => *line,
        }
    }

    fn shape(&self) -> &'static str {
        match self {
            Self::Scalar { .. } => "a scalar",
            Self::Seq { .. } => "a sequence",
            Self::Map { .. } => "a mapping",
        }
    }
}

/// Lexes the text into physical lines. Blank lines and full-line `#`
/// comments are dropped; inline `#` is content. A tab anywhere in the
/// leading whitespace run is drift, not style.
fn lex(text: &str) -> Result<Vec<PhysLine>, ScenarioError> {
    let mut out = Vec::new();
    for (index, raw) in text.split('\n').enumerate() {
        let line = raw.strip_suffix('\r').unwrap_or(raw);
        let body_start = line.len() - line.trim_start_matches([' ', '\t']).len();
        if line[..body_start].contains('\t') {
            return Err(ScenarioError::MalformedItem {
                line: index + 1,
                detail: "a tab in the indentation".to_owned(),
            });
        }
        let body = &line[body_start..];
        if body.is_empty() || body.starts_with('#') {
            continue;
        }
        out.push(PhysLine {
            number: index + 1,
            indent: body_start,
            text: body.to_owned(),
        });
    }
    Ok(out)
}

/// Splits `key: rest` at the first colon: the colon is followed by nothing
/// (nested value) or by exactly one space.
fn key_line(line: &PhysLine) -> Result<(String, String), ScenarioError> {
    let Some((key, rest)) = line.text.split_once(':') else {
        return Err(ScenarioError::NotAMapping { line: line.number });
    };
    if key.is_empty() || key.contains(' ') {
        return Err(ScenarioError::NotAMapping { line: line.number });
    }
    let rest = rest.strip_prefix(' ').unwrap_or(rest);
    Ok((key.to_owned(), rest.to_owned()))
}

/// Parses the value for the key line at `*pos`.
///
/// Position contract (the traversal's single rule — see the Task-7
/// runaway-memory lesson): `*pos` sits ON the key line on entry and every
/// return path leaves it PAST the key line and the key's entire value
/// block. No caller can re-read a line it already handed over, and every
/// loop in the traversal either advances `*pos` or breaks toward the root
/// check, which turns any leftover line into a typed
/// [`ScenarioError::UnplaceableLine`].
///
/// Depth contract: `depth` is the container nesting level of the key's
/// OWNING mapping; a nested container is handed down as `depth + 1`, and
/// the container functions refuse `depth > MAX_SCENARIO_DEPTH` with a
/// typed [`ScenarioError::DepthExceeded`] — the descent is bounded in
/// depth as well as in termination.
fn value_of(
    lines: &[PhysLine],
    pos: &mut usize,
    rest: String,
    key_indent: usize,
    depth: usize,
) -> Result<Node, ScenarioError> {
    let line_number = lines[*pos].number;
    if !rest.is_empty() {
        *pos += 1;
        if rest.starts_with('[') {
            if !rest.ends_with(']') {
                return Err(ScenarioError::MalformedItem {
                    line: line_number,
                    detail: format!("an unterminated flow sequence {rest:?}"),
                });
            }
            return flow_seq(&rest, line_number);
        }
        return Ok(Node::Scalar {
            line: line_number,
            value: rest,
        });
    }
    match lines.get(*pos + 1) {
        Some(next) if next.indent > key_indent => {
            *pos += 1;
            if next.text.starts_with("- ") || next.text == "-" {
                return block_sequence(lines, pos, depth + 1);
            }
            mapping(lines, pos, next.indent, depth + 1)
        }
        _ => {
            *pos += 1;
            Ok(Node::Seq {
                line: line_number,
                items: Vec::new(),
            })
        }
    }
}

/// A mapping at exactly `map_indent`; `*pos` is ON its first key line.
/// Consumes key lines while they sit at `map_indent`; stops at anything
/// else (the root check turns unplaceable lines into typed errors).
fn mapping(
    lines: &[PhysLine],
    pos: &mut usize,
    map_indent: usize,
    depth: usize,
) -> Result<Node, ScenarioError> {
    if depth > MAX_SCENARIO_DEPTH {
        return Err(ScenarioError::DepthExceeded {
            line: lines[*pos].number,
            depth,
        });
    }
    let line_number = lines[*pos].number;
    let mut entries: Vec<(String, Node)> = Vec::new();
    let mut seen: Vec<String> = Vec::new();
    while let Some(line) = lines.get(*pos) {
        if line.indent != map_indent {
            break;
        }
        let (key, rest) = key_line(line)?;
        if seen.contains(&key) {
            return Err(ScenarioError::DuplicateKey {
                line: line.number,
                key,
            });
        }
        seen.push(key.clone());
        let value = value_of(lines, pos, rest, map_indent, depth)?;
        entries.push((key, value));
    }
    Ok(Node::Map {
        line: line_number,
        entries,
    })
}

/// A flow sequence of scalars: `[a, b]` or `[]`. Entries are trimmed;
/// empty entries are refused.
fn flow_seq(text: &str, line: usize) -> Result<Node, ScenarioError> {
    let inner = &text[1..text.len() - 1];
    let mut items = Vec::new();
    if inner.trim().is_empty() {
        return Ok(Node::Seq { line, items });
    }
    for entry in inner.split(',') {
        let trimmed = entry.trim();
        if trimmed.is_empty() {
            return Err(ScenarioError::MalformedItem {
                line,
                detail: format!("an empty flow entry in {text:?}"),
            });
        }
        items.push(Node::Scalar {
            line,
            value: trimmed.to_owned(),
        });
    }
    Ok(Node::Seq { line, items })
}

/// A block sequence at one indent: `- item` lines. An item is a scalar,
/// or a mapping whose first key sits in the item line and whose further
/// keys continue two columns deeper (the frozen shape). `*pos` is ON the
/// first `- ` line.
fn block_sequence(
    lines: &[PhysLine],
    pos: &mut usize,
    depth: usize,
) -> Result<Node, ScenarioError> {
    if depth > MAX_SCENARIO_DEPTH {
        return Err(ScenarioError::DepthExceeded {
            line: lines[*pos].number,
            depth,
        });
    }
    let item_indent = lines[*pos].indent;
    let line_number = lines[*pos].number;
    let mut items = Vec::new();
    while let Some(line) = lines.get(*pos) {
        if line.indent != item_indent {
            break;
        }
        let Some(rest) = line.text.strip_prefix("- ") else {
            break;
        };
        if rest.is_empty() {
            // `- ` alone: the item is the nested block on the next lines.
            let dash_line = line.number;
            *pos += 1;
            match lines.get(*pos) {
                Some(next) if next.indent > item_indent => {
                    items.push(mapping(lines, pos, next.indent, depth + 1)?);
                }
                _ => {
                    return Err(ScenarioError::MalformedItem {
                        line: dash_line,
                        detail: "a bare dash with no item".to_owned(),
                    });
                }
            }
            continue;
        }
        if rest.contains(": ") || rest.ends_with(':') {
            items.push(dash_mapping_item(
                lines,
                pos,
                rest,
                item_indent,
                line.number,
                depth,
            )?);
            continue;
        }
        // A scalar item.
        *pos += 1;
        items.push(Node::Scalar {
            line: line.number,
            value: rest.to_owned(),
        });
    }
    Ok(Node::Seq {
        line: line_number,
        items,
    })
}

/// A `- key: value` mapping item: the first key is rebased to
/// `item_indent + 2`, the item's whole subtree (anything deeper than the
/// dash) is collected with its physical indents, and the mapping is
/// parsed over that slice. Any line the sub-mapping cannot place is a
/// typed error — items do not silently drop content.
fn dash_mapping_item(
    lines: &[PhysLine],
    pos: &mut usize,
    first_rest: &str,
    item_indent: usize,
    line_number: usize,
    depth: usize,
) -> Result<Node, ScenarioError> {
    let key_indent = item_indent + 2;
    let mut sub: Vec<PhysLine> = vec![PhysLine {
        number: line_number,
        indent: key_indent,
        text: first_rest.to_owned(),
    }];
    *pos += 1;
    while let Some(next) = lines.get(*pos) {
        if next.indent <= item_indent {
            break;
        }
        sub.push(PhysLine {
            number: next.number,
            indent: next.indent,
            text: next.text.clone(),
        });
        *pos += 1;
    }
    let mut sub_pos = 0;
    let parsed = mapping(&sub, &mut sub_pos, key_indent, depth + 1)?;
    if sub_pos < sub.len() {
        let stray = &sub[sub_pos];
        return Err(ScenarioError::UnplaceableLine {
            line: stray.number,
            indent: stray.indent,
        });
    }
    Ok(parsed)
}

// ---------------------------------------------------------------------------
// Parsing entry point
// ---------------------------------------------------------------------------

/// Parses and validates a scenario document.
pub fn parse_scenario(text: &str) -> Result<Scenario, ScenarioError> {
    let lines = lex(text)?;
    let mut pos = 0;
    let root = match lines.first() {
        Some(first) if first.indent == 0 => mapping(&lines, &mut pos, 0, 1)?,
        Some(first) => {
            return Err(ScenarioError::UnplaceableLine {
                line: first.number,
                indent: first.indent,
            });
        }
        None => {
            return Err(ScenarioError::MissingKey {
                level: "root",
                key: "scenario",
            });
        }
    };
    if pos < lines.len() {
        let stray = &lines[pos];
        return Err(ScenarioError::UnplaceableLine {
            line: stray.number,
            indent: stray.indent,
        });
    }
    validate_root(root)
}

// ---------------------------------------------------------------------------
// Validators (typed, line-locused)
// ---------------------------------------------------------------------------

/// The root keys the dialect admits, exactly.
const ROOT_KEYS: &[&str] = &[
    "scenario",
    "role",
    "behavior",
    "probes_before_write",
    "timeout_after_probes",
    "state",
    "derailment",
    "payload",
    "workspace_effects",
];

fn validate_root(root: Node) -> Result<Scenario, ScenarioError> {
    let Node::Map { entries, .. } = root else {
        return Err(ScenarioError::NotAMapping { line: 1 });
    };
    for (key, node) in &entries {
        if !ROOT_KEYS.contains(&key.as_str()) {
            return Err(ScenarioError::UnknownKey {
                line: node.line(),
                key: key.clone(),
            });
        }
    }
    let name = require_scalar(&entries, "root", "scenario")?;
    if name.value.contains(char::is_whitespace) {
        return Err(ScenarioError::ValueInvalid {
            line: name.line,
            key: "scenario".to_owned(),
            detail: format!(
                "a scenario identity is a single non-whitespace token, got {:?}",
                name.value
            ),
        });
    }
    let role_raw = require_scalar(&entries, "root", "role")?;
    let role = RoleId::parse(&role_raw.value)
        .map_err(|error| value_error(role_raw.line, "role", error))?;
    let behavior_raw = require_scalar(&entries, "root", "behavior")?;
    let behavior =
        ChildBehavior::parse(&behavior_raw.value).ok_or_else(|| ScenarioError::UnknownToken {
            line: behavior_raw.line,
            key: "behavior".to_owned(),
            value: behavior_raw.value.clone(),
            vocabulary: ChildBehavior::TOKENS,
        })?;
    let probes = optional_probe(&entries, "probes_before_write", 0, WRITE_PROBE_CEILING)?;
    let timeout = optional_probe(&entries, "timeout_after_probes", 1, PROBE_CEILING)?;
    let state_node = entries
        .iter()
        .find(|(key, _)| key == "state")
        .map(|(_, node)| node);
    let state = match state_node {
        Some(Node::Scalar { line, value }) => Some(RoleResultState::parse(value).map_err(
            |error| ScenarioError::ValueInvalid {
                line: *line,
                key: "state".to_owned(),
                detail: error.to_string(),
            },
        )?),
        Some(node) => return Err(bad_shape(node, "state")),
        None => None,
    };
    let derailment_node = entries
        .iter()
        .find(|(key, _)| key == "derailment")
        .map(|(_, node)| node);
    let derailment = match derailment_node {
        None => Derailment::None,
        Some(Node::Scalar { line, value }) => {
            Derailment::parse(value).ok_or_else(|| ScenarioError::UnknownToken {
                line: *line,
                key: "derailment".to_owned(),
                value: value.clone(),
                vocabulary: Derailment::TOKENS,
            })?
        }
        Some(node) => return Err(bad_shape(node, "derailment")),
    };
    let payload_node = entries
        .iter()
        .find(|(key, _)| key == "payload")
        .map(|(_, node)| node);
    let effects_node = entries
        .iter()
        .find(|(key, _)| key == "workspace_effects")
        .map(|(_, node)| node);

    // Behavior-conditional agreement: each behavior admits its own keys
    // and refuses the other's — a silent child writes nothing at all (no
    // document, no workspace file), a writing child never stalls to its
    // deadline.
    let (state, payload, probes_before_write, timeout_after_probes) = match behavior {
        ChildBehavior::WriteResult => {
            if let Some((_, line)) = timeout {
                return Err(ScenarioError::BehaviorConflict {
                    line,
                    detail: "behavior write-result cannot carry timeout_after_probes; a \
                             writing child never stalls to its deadline"
                        .to_owned(),
                });
            }
            let Some(state) = state else {
                return Err(ScenarioError::MissingKey {
                    level: "root",
                    key: "state",
                });
            };
            let Some(payload_node) = payload_node else {
                return Err(ScenarioError::MissingKey {
                    level: "root",
                    key: "payload",
                });
            };
            let payload = validate_payload(payload_node)?;
            (
                Some(state),
                Some(payload),
                probes.map(|(value, _)| value).unwrap_or(0),
                None,
            )
        }
        ChildBehavior::Stall => {
            if let Some((_, line)) = probes {
                return Err(ScenarioError::BehaviorConflict {
                    line,
                    detail: "behavior stall cannot carry probes_before_write; a silent \
                             child writes nothing, ever"
                        .to_owned(),
                });
            }
            if let Some(node) = state_node {
                return Err(ScenarioError::BehaviorConflict {
                    line: node.line(),
                    detail: "behavior stall cannot carry state; a silent child declares \
                             no result state"
                        .to_owned(),
                });
            }
            if let Some(node) = derailment_node {
                return Err(ScenarioError::BehaviorConflict {
                    line: node.line(),
                    detail: "behavior stall cannot carry a derailment; there is no \
                             document to derail"
                        .to_owned(),
                });
            }
            if let Some(node) = payload_node {
                return Err(ScenarioError::BehaviorConflict {
                    line: node.line(),
                    detail: "behavior stall cannot carry a payload; a silent child \
                             writes no document"
                        .to_owned(),
                });
            }
            if let Some(node) = effects_node {
                return Err(ScenarioError::BehaviorConflict {
                    line: node.line(),
                    detail: "behavior stall cannot carry workspace_effects; the \
                             runner's stall arm applies no effects, so the fixture \
                             would claim a write that never happens"
                        .to_owned(),
                });
            }
            let Some((value, _)) = timeout else {
                return Err(ScenarioError::MissingKey {
                    level: "root",
                    key: "timeout_after_probes",
                });
            };
            (None, None, 0, Some(value))
        }
        // Plan Task 16, never-write shape 2: the child has EXITED without
        // ever writing a document. Every writing key is refused exactly
        // like stall's — and the deadline knob is refused TOO, because
        // this verdict is decided by the OBSERVED EXIT, never by the
        // clock (a dead child can never write, so no deadline is ever
        // reached).
        ChildBehavior::ExitWithoutResult => {
            if let Some((_, line)) = probes {
                return Err(ScenarioError::BehaviorConflict {
                    line,
                    detail: "behavior exit-without-result cannot carry probes_before_write; \
                             an exited child writes nothing, ever"
                        .to_owned(),
                });
            }
            if let Some(node) = state_node {
                return Err(ScenarioError::BehaviorConflict {
                    line: node.line(),
                    detail: "behavior exit-without-result cannot carry state; an exited \
                             child declares no result state"
                        .to_owned(),
                });
            }
            if let Some((_, line)) = timeout {
                return Err(ScenarioError::BehaviorConflict {
                    line,
                    detail: "behavior exit-without-result cannot carry timeout_after_probes; \
                             the missing_result verdict is decided by the observed exit, \
                             never by the deadline"
                        .to_owned(),
                });
            }
            if let Some(node) = derailment_node {
                return Err(ScenarioError::BehaviorConflict {
                    line: node.line(),
                    detail: "behavior exit-without-result cannot carry a derailment; there \
                             is no document to derail"
                        .to_owned(),
                });
            }
            if let Some(node) = payload_node {
                return Err(ScenarioError::BehaviorConflict {
                    line: node.line(),
                    detail: "behavior exit-without-result cannot carry a payload; an \
                             exited child writes no document"
                        .to_owned(),
                });
            }
            if let Some(node) = effects_node {
                return Err(ScenarioError::BehaviorConflict {
                    line: node.line(),
                    detail: "behavior exit-without-result cannot carry workspace_effects; \
                             the runner's exit arm applies no effects, so the fixture would \
                             claim a write that never happens"
                        .to_owned(),
                });
            }
            (None, None, 0, None)
        }
    };

    let workspace_effects = match effects_node {
        None => Vec::new(),
        Some(node) => validate_effects(node)?,
    };

    Ok(Scenario::assemble(
        name.value,
        role,
        behavior,
        probes_before_write,
        timeout_after_probes,
        state,
        derailment,
        payload,
        workspace_effects,
    ))
}

/// One scalar answer: its line and value.
struct ScalarAnswer {
    line: usize,
    value: String,
}

fn require_scalar(
    entries: &[(String, Node)],
    level: &'static str,
    key: &'static str,
) -> Result<ScalarAnswer, ScenarioError> {
    match entries.iter().find(|(k, _)| k == key).map(|(_, node)| node) {
        Some(Node::Scalar { line, value }) => Ok(ScalarAnswer {
            line: *line,
            value: value.clone(),
        }),
        Some(node) => Err(bad_shape(node, key)),
        None => Err(ScenarioError::MissingKey { level, key }),
    }
}

/// A probe-count knob, if present: a plain integer within the knob's
/// own inclusive `min..=max` range. The knobs differ at both edges —
/// `probes_before_write` admits 0 (work already done at admission) but
/// caps one below the drivers' budget (the child terminates one poll
/// after its last in-flight probe), while `timeout_after_probes`
/// requires ≥ 1 (zero probes of silence before the deadline fires is
/// not a deadline model) and caps exactly at the budget.
fn optional_probe(
    entries: &[(String, Node)],
    key: &'static str,
    min: u8,
    max: u8,
) -> Result<Option<(u8, usize)>, ScenarioError> {
    match entries.iter().find(|(k, _)| k == key).map(|(_, node)| node) {
        None => Ok(None),
        Some(Node::Scalar { line, value }) => {
            if value.is_empty() || !value.bytes().all(|b| b.is_ascii_digit()) {
                return Err(ScenarioError::NotAnInteger {
                    line: *line,
                    key: key.to_owned(),
                    value: value.clone(),
                });
            }
            let parsed = value
                .parse::<u32>()
                .map_err(|_| ScenarioError::NotAnInteger {
                    line: *line,
                    key: key.to_owned(),
                    value: value.clone(),
                })?;
            if parsed < u32::from(min) || parsed > u32::from(max) {
                return Err(ScenarioError::IntegerOutOfRange {
                    line: *line,
                    key: key.to_owned(),
                    value: value.clone(),
                    min,
                    max,
                });
            }
            Ok(Some((parsed as u8, *line)))
        }
        Some(node) => Err(bad_shape(node, key)),
    }
}

fn bad_shape(node: &Node, key: &str) -> ScenarioError {
    ScenarioError::BadShape {
        line: node.line(),
        key: key.to_owned(),
        expected: "a scalar",
        got: node.shape(),
    }
}

fn value_error(line: usize, key: &str, error: ValueError) -> ScenarioError {
    ScenarioError::ValueInvalid {
        line,
        key: key.to_owned(),
        detail: error.to_string(),
    }
}

/// The payload keys (Annex B.2), exactly; all REQUIRED.
const PAYLOAD_KEYS: &[&str] = &[
    "attempted",
    "changed_paths",
    "commands_run",
    "artifacts",
    "claims",
    "self_checks",
    "blocked_on",
];

fn validate_payload(node: &Node) -> Result<PayloadParts, ScenarioError> {
    let Node::Map { entries, .. } = node else {
        return Err(ScenarioError::BadShape {
            line: node.line(),
            key: "payload".to_owned(),
            expected: "a mapping",
            got: node.shape(),
        });
    };
    for (key, entry) in entries {
        if !PAYLOAD_KEYS.contains(&key.as_str()) {
            return Err(ScenarioError::UnknownKey {
                line: entry.line(),
                key: key.clone(),
            });
        }
    }
    for required in PAYLOAD_KEYS {
        if !entries.iter().any(|(key, _)| key == required) {
            return Err(ScenarioError::MissingKey {
                level: "payload",
                key: required,
            });
        }
    }
    let attempted = unit_id_items(find_seq(entries, "attempted")?)?;
    let changed_paths = string_items(find_seq(entries, "changed_paths")?, "changed_paths")?;
    let commands_run = string_items(find_seq(entries, "commands_run")?, "commands_run")?;
    let claims = string_items(find_seq(entries, "claims")?, "claims")?;
    let blocked_on = string_items(find_seq(entries, "blocked_on")?, "blocked_on")?;
    let artifacts = validate_artifacts(find_seq(entries, "artifacts")?)?;
    let self_checks = validate_self_checks(find_seq(entries, "self_checks")?)?;
    Ok(PayloadParts {
        attempted,
        changed_paths,
        commands_run,
        artifacts,
        claims,
        blocked_on,
        self_checks,
    })
}

fn find_seq<'a>(
    entries: &'a [(String, Node)],
    key: &'static str,
) -> Result<&'a [Node], ScenarioError> {
    match entries.iter().find(|(k, _)| k == key).map(|(_, node)| node) {
        Some(Node::Seq { items, .. }) => Ok(items),
        Some(node) => Err(ScenarioError::BadShape {
            line: node.line(),
            key: key.to_owned(),
            expected: "a sequence",
            got: node.shape(),
        }),
        None => Err(ScenarioError::MissingKey {
            level: "payload",
            key,
        }),
    }
}

fn string_items(items: &[Node], key: &str) -> Result<Vec<String>, ScenarioError> {
    items
        .iter()
        .map(|item| match item {
            Node::Scalar { value, .. } => Ok(value.clone()),
            node => Err(ScenarioError::BadShape {
                line: node.line(),
                key: key.to_owned(),
                expected: "a sequence of scalar strings",
                got: node.shape(),
            }),
        })
        .collect()
}

fn unit_id_items(items: &[Node]) -> Result<Vec<String>, ScenarioError> {
    items
        .iter()
        .map(|item| match item {
            Node::Scalar { line, value } => UnitId::new(value)
                .map(|_| value.clone())
                .map_err(|error| value_error(*line, "attempted", error)),
            node => Err(ScenarioError::BadShape {
                line: node.line(),
                key: "attempted".to_owned(),
                expected: "a sequence of §5.2 unit ids",
                got: node.shape(),
            }),
        })
        .collect()
}

/// Validates one block-sequence-of-mappings item against its fixed field
/// set and returns its entries.
fn item_mapping<'a>(
    item: &'a Node,
    seq_key: &'static str,
    fields: &[&str],
) -> Result<&'a [(String, Node)], ScenarioError> {
    let Node::Map { entries, .. } = item else {
        return Err(ScenarioError::BadShape {
            line: item.line(),
            key: seq_key.to_owned(),
            expected: "a sequence of mappings",
            got: item.shape(),
        });
    };
    for (key, entry) in entries {
        if !fields.contains(&key.as_str()) {
            return Err(ScenarioError::UnknownKey {
                line: entry.line(),
                key: key.clone(),
            });
        }
    }
    Ok(entries)
}

fn validate_artifacts(items: &[Node]) -> Result<Vec<ArtifactPart>, ScenarioError> {
    items
        .iter()
        .map(|item| {
            let entries = item_mapping(item, "artifacts", &["path", "evidence_ref"])?;
            let path = require_scalar(entries, "artifacts", "path")?;
            if path.value.is_empty() {
                return Err(ScenarioError::ValueInvalid {
                    line: path.line,
                    key: "artifacts.path".to_owned(),
                    detail: "an artifact path must not be empty".to_owned(),
                });
            }
            let evidence = require_scalar(entries, "artifacts", "evidence_ref")?;
            EvidenceRef::new(&evidence.value)
                .map_err(|error| value_error(evidence.line, "artifacts.evidence_ref", error))?;
            Ok(ArtifactPart {
                path: path.value,
                evidence_ref: evidence.value,
            })
        })
        .collect()
}

fn validate_self_checks(items: &[Node]) -> Result<Vec<SelfCheckPart>, ScenarioError> {
    items
        .iter()
        .map(|item| {
            let entries = item_mapping(item, "self_checks", &["command", "exit_code"])?;
            let command = require_scalar(entries, "self_checks", "command")?;
            let exit = require_scalar(entries, "self_checks", "exit_code")?;
            // An exit code is an integer, optionally negative (signal
            // deaths report negative codes on some platforms).
            let parsed: i64 = match exit.value.strip_prefix('-') {
                Some(magnitude) => -magnitude
                    .parse::<i64>()
                    .map_err(|_| not_an_integer(&exit, "self_checks.exit_code"))?,
                None => exit
                    .value
                    .parse::<i64>()
                    .map_err(|_| not_an_integer(&exit, "self_checks.exit_code"))?,
            };
            Ok(SelfCheckPart {
                command: command.value,
                exit_code: parsed,
            })
        })
        .collect()
}

fn not_an_integer(answer: &ScalarAnswer, key: &str) -> ScenarioError {
    ScenarioError::NotAnInteger {
        line: answer.line,
        key: key.to_owned(),
        value: answer.value.clone(),
    }
}

fn validate_effects(node: &Node) -> Result<Vec<WorkspaceEffect>, ScenarioError> {
    let Node::Seq { items, .. } = node else {
        return Err(ScenarioError::BadShape {
            line: node.line(),
            key: "workspace_effects".to_owned(),
            expected: "a sequence of {path, content} mappings",
            got: node.shape(),
        });
    };
    items
        .iter()
        .map(|item| {
            let entries = item_mapping(item, "workspace_effects", &["path", "content"])?;
            let path = require_scalar(entries, "workspace_effects", "path")?;
            let content = require_scalar(entries, "workspace_effects", "content")?;
            validate_effect_path(&path)?;
            Ok(WorkspaceEffect {
                path: path.value,
                content: content.value,
            })
        })
        .collect()
}

/// An effect path stays inside the workspace root, lexically: relative,
/// non-empty, no `..` component, no absolute root, no whitespace (the
/// runner re-checks containment at resolution time — two independent
/// guards, one at parse, one at use).
fn validate_effect_path(answer: &ScalarAnswer) -> Result<(), ScenarioError> {
    let invalid = |detail: String| ScenarioError::ValueInvalid {
        line: answer.line,
        key: "workspace_effects.path".to_owned(),
        detail,
    };
    let path = answer.value.as_str();
    if path.is_empty() {
        return Err(invalid("an effect path must not be empty".to_owned()));
    }
    if path.starts_with('/') {
        return Err(invalid(format!(
            "an effect path must be relative to the workspace root, got {path:?}"
        )));
    }
    if path.bytes().any(|b| b.is_ascii_whitespace()) {
        return Err(invalid(format!(
            "an effect path must be whitespace-free, got {path:?}"
        )));
    }
    if std::path::Path::new(path)
        .components()
        .any(|component| matches!(component, std::path::Component::ParentDir))
    {
        return Err(invalid(format!(
            "an effect path must not traverse upward (no `..`), got {path:?}"
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Fixture loading
// ---------------------------------------------------------------------------

/// The scenario fixture directory: `tests/fixtures/mock_runner_scenarios`
/// under the repository root, reached through this crate's manifest dir
/// (the established no-absolute-path pattern).
pub fn scenario_fixtures_dir() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../tests/fixtures/mock_runner_scenarios")
}

/// Loads and parses one scenario fixture by stem (e.g. `"happy"`), also
/// checking that the file's declared `scenario:` identity matches its stem.
///
/// COR-F2 (falsifier round 1, F-2): the stem is a fixture NAME, not a
/// path. Only a simple name — non-empty, no path separators, no `..`
/// component, no whitespace — is accepted, and the refusal fires BEFORE
/// any path join, so the loader's reach is exactly the frozen-locator
/// fixture directory (`tests/fixtures/mock_runner_scenarios`); a traversal
/// stem cannot even name a file outside it, and the `scenario:` identity
/// check stays a consistency pin, never the containment guard.
pub fn load_scenario(name: &str) -> Result<Scenario, ScenarioError> {
    let invalid = |detail: String| ScenarioError::ValueInvalid {
        line: 0,
        key: "scenario".to_owned(),
        detail,
    };
    if name.is_empty() {
        return Err(invalid("the scenario stem must not be empty".to_owned()));
    }
    if name.starts_with('/') {
        return Err(invalid(format!(
            "the scenario stem {name:?} must be a fixture name, not a path"
        )));
    }
    if name.bytes().any(|b| b.is_ascii_whitespace()) {
        return Err(invalid(format!(
            "the scenario stem {name:?} must be whitespace-free"
        )));
    }
    if name.chars().any(std::path::is_separator) {
        return Err(invalid(format!(
            "the scenario stem {name:?} must be a simple fixture name (no path separators) \
             under the scenario fixtures dir"
        )));
    }
    if !std::path::Path::new(name)
        .components()
        .all(|component| matches!(component, std::path::Component::Normal(_)))
    {
        return Err(invalid(format!(
            "the scenario stem {name:?} must be a simple fixture name (no '.'/'..') under \
             the scenario fixtures dir"
        )));
    }
    let path = scenario_fixtures_dir().join(format!("{name}.yaml"));
    let text =
        std::fs::read_to_string(&path).map_err(|error| ScenarioError::FixtureUnreadable {
            name: name.to_owned(),
            detail: error.to_string(),
        })?;
    let scenario = parse_scenario(&text)?;
    if scenario.name() != name {
        return Err(ScenarioError::NameMismatch {
            stem: name.to_owned(),
            scenario: scenario.name().to_owned(),
        });
    }
    Ok(scenario)
}

#[cfg(test)]
mod tests {
    use super::{ChildBehavior, Derailment, ScenarioError, load_scenario, parse_scenario};
    use gauntlet_domain::{
        Budget, DiagnosticAllowance, Digest, EpochSeconds, InvocationId, Limits, MicroUsd,
        PermissionSetRef, ResultContractPath, RoleId, RunId, UnitId, WorkspaceRef,
    };
    use gauntlet_ports::agent_runner::{ContextEnvelope, RoleInvocationSpec};
    use gauntlet_ports::deadline::Deadline;
    use gauntlet_ports::role_result::{
        RoleResult, RoleResultError, RoleResultState, read_result_contract,
    };

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const DIGEST: &str = "sha256:a1b2c3d4e5f6a7b8";

    fn dispatch_spec() -> RoleInvocationSpec {
        let budget = Budget::new(
            Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("suite limits usable"),
            20,
            DiagnosticAllowance::new(100, MicroUsd::new(50)),
        )
        .expect("suite budget usable");
        RoleInvocationSpec {
            invocation_id: InvocationId::new(ULID).expect("valid invocation id"),
            run_id: RunId::new(ULID).expect("valid run id"),
            iteration_no: gauntlet_domain::IterationNo::new(2).expect("1-based iteration"),
            role: RoleId::Builder,
            prompt: "run the assigned slice".to_owned(),
            context_envelope: ContextEnvelope::new(
                "the runtime-built allowlisted context".to_owned(),
                Digest::parse(DIGEST).expect("valid digest"),
            ),
            workspace: WorkspaceRef::new("ws-scenario").expect("valid workspace ref"),
            permissions: PermissionSetRef::new("perm-readonly").expect("valid permission ref"),
            result_contract_path: ResultContractPath::new("scenario-result-document")
                .expect("valid contract path"),
            deadline: Deadline::at_epoch(EpochSeconds::new(1_700_000_090)),
            model_hint: None,
            sandbox_policy: None,
            budget_slice: budget,
        }
    }

    fn happy_text() -> String {
        [
            "scenario: parser-happy",
            "role: builder",
            "behavior: write-result",
            "state: completed",
            "payload:",
            "  attempted: [AK-014, AK-015]",
            "  changed_paths: [src/lib.rs]",
            "  commands_run: []",
            "  artifacts:",
            "    - path: out/report.txt",
            "      evidence_ref: evd-000042",
            "  claims:",
            "    - the slice is done",
            "    - all checks passed",
            "  self_checks:",
            "    - command: just test",
            "      exit_code: 0",
            "  blocked_on: []",
            "workspace_effects:",
            "  - path: out/report.txt",
            "    content: PASS the report exists",
            "# a full-line comment is dropped",
            "",
        ]
        .join("\n")
    }

    #[test]
    fn a_write_result_scenario_parses_field_for_field() {
        let scenario = parse_scenario(&happy_text()).expect("the happy fixture shape parses");
        assert_eq!(scenario.name(), "parser-happy");
        assert_eq!(scenario.role(), &RoleId::Builder);
        assert_eq!(scenario.behavior(), ChildBehavior::WriteResult);
        assert_eq!(scenario.probes_before_write(), 0);
        assert_eq!(scenario.timeout_after_probes(), None);
        assert_eq!(scenario.state(), Some(RoleResultState::Completed));
        assert_eq!(scenario.derailment(), Derailment::None);
        let payload = scenario.payload().expect("write-result carries a payload");
        assert_eq!(payload.attempted().len(), 2);
        assert_eq!(payload.changed_paths(), &["src/lib.rs".to_owned()]);
        assert_eq!(payload.artifacts().len(), 1);
        assert_eq!(payload.artifacts()[0].path(), "out/report.txt");
        assert_eq!(payload.artifacts()[0].evidence_ref(), "evd-000042");
        assert_eq!(payload.claims().len(), 2);
        assert_eq!(payload.self_checks().len(), 1);
        assert_eq!(payload.self_checks()[0].exit_code(), 0);
        let effects = scenario.workspace_effects();
        assert_eq!(effects.len(), 1);
        assert_eq!(effects[0].path(), "out/report.txt");
        assert_eq!(effects[0].content(), "PASS the report exists");
    }

    #[test]
    fn a_stall_scenario_parses_with_its_own_keys() {
        let text = [
            "scenario: parser-stall",
            "role: builder",
            "behavior: stall",
            "timeout_after_probes: 4",
        ]
        .join("\n");
        let scenario = parse_scenario(&text).expect("the stall shape parses");
        assert_eq!(scenario.behavior(), ChildBehavior::Stall);
        assert_eq!(scenario.timeout_after_probes(), Some(4));
        assert_eq!(scenario.payload(), None);
        assert_eq!(scenario.state(), None);
        assert!(scenario.workspace_effects().is_empty());
    }

    #[test]
    fn unknown_root_keys_are_typed_line_errors() {
        let text = happy_text().replace(
            "behavior: write-result",
            "flavour: spicy\nbehavior: write-result",
        );
        let error = parse_scenario(&text).expect_err("unknown keys never admitted");
        assert!(
            matches!(&error, ScenarioError::UnknownKey { line: 3, key } if key == "flavour"),
            "got {error:?}"
        );
    }

    #[test]
    fn duplicate_keys_are_refused_with_the_line() {
        // Replace the state line so the duplicate lands on line 5.
        let text = happy_text().replace("state: completed", "state: completed\nstate: failed");
        let error = parse_scenario(&text).expect_err("duplicates never admitted");
        assert!(
            matches!(&error, ScenarioError::DuplicateKey { line: 5, key } if key == "state"),
            "got {error:?}"
        );
    }

    #[test]
    fn tabs_in_indentation_are_refused() {
        let text = happy_text().replace("  attempted:", "\tattempted:");
        let error = parse_scenario(&text).expect_err("tabs are drift");
        assert!(
            matches!(&error, ScenarioError::MalformedItem { line: 6, .. }),
            "got {error:?}"
        );
    }

    #[test]
    fn write_result_cannot_carry_timeout_after_probes() {
        let text = happy_text().replace(
            "state: completed",
            "state: completed\ntimeout_after_probes: 3",
        );
        let error = parse_scenario(&text).expect_err("a writing child never stalls");
        assert!(
            matches!(&error, ScenarioError::BehaviorConflict { line: 5, .. }),
            "got {error:?}"
        );
    }

    #[test]
    fn stall_cannot_carry_document_keys() {
        for (needle, replacement) in [
            (
                "timeout_after_probes: 4",
                "timeout_after_probes: 4\nstate: completed",
            ),
            (
                "timeout_after_probes: 4",
                "timeout_after_probes: 4\nderailment: none",
            ),
        ] {
            let text = [
                "scenario: parser-stall",
                "role: builder",
                "behavior: stall",
                needle,
            ]
            .join("\n")
            .replace(needle, replacement);
            let error = parse_scenario(&text).expect_err("silent children carry no documents");
            assert!(
                matches!(error, ScenarioError::BehaviorConflict { .. }),
                "got {error:?}"
            );
        }
    }

    #[test]
    fn stall_cannot_carry_workspace_effects() {
        // A silent child writes NOTHING — not a document, not a file. The
        // runner's Stall arm never applies effects, so a fixture carrying
        // them under stall would encode a lie the parser must refuse. The
        // sequence node carries its first item's line (the `- path:` line),
        // not the key line.
        let text = [
            "scenario: parser-stall-effects",
            "role: builder",
            "behavior: stall",
            "timeout_after_probes: 4",
            "workspace_effects:",
            "  - path: out/never.txt",
            "    content: unreachable",
        ]
        .join("\n");
        let error = parse_scenario(&text).expect_err("a silent child writes no files");
        assert!(
            matches!(&error, ScenarioError::BehaviorConflict { line: 6, .. }),
            "got {error:?}"
        );
    }

    #[test]
    fn timeout_after_probes_zero_is_refused_but_zero_write_probes_is_legal() {
        // The deadline model needs at least one probe of silence — the
        // declared range is <1..=PROBE_CEILING>. `probes_before_write: 0`
        // stays legal: work already done at admission is a real child
        // shape (the suite's CompleteImmediately case requires it).
        let text = [
            "scenario: parser-stall-zero",
            "role: builder",
            "behavior: stall",
            "timeout_after_probes: 0",
        ]
        .join("\n");
        let error = parse_scenario(&text).expect_err("zero probes is not a deadline");
        assert!(
            matches!(
                &error,
                ScenarioError::IntegerOutOfRange {
                    line: 4,
                    key,
                    ..
                } if key == "timeout_after_probes"
            ),
            "got {error:?}"
        );
        let zero_write = happy_text().replace(
            "state: completed",
            "state: completed\nprobes_before_write: 0",
        );
        parse_scenario(&zero_write).expect("zero in-flight probes is legal");
    }

    #[test]
    fn stall_requires_timeout_after_probes() {
        let text = ["scenario: parser-stall", "role: builder", "behavior: stall"].join("\n");
        let error = parse_scenario(&text).expect_err("a silent child needs its deadline model");
        assert!(
            matches!(
                error,
                ScenarioError::MissingKey {
                    level: "root",
                    key: "timeout_after_probes"
                }
            ),
            "got {error:?}"
        );
    }

    #[test]
    fn write_result_requires_state_and_payload() {
        let no_state = happy_text().replace("state: completed\n", "");
        let error = parse_scenario(&no_state).expect_err("state is required");
        assert!(
            matches!(
                error,
                ScenarioError::MissingKey {
                    level: "root",
                    key: "state"
                }
            ),
            "got {error:?}"
        );
        // Keep state (line 4); drop everything from `payload:` onward.
        let no_payload = happy_text().lines().take(4).collect::<Vec<_>>().join("\n");
        let error = parse_scenario(&no_payload).expect_err("payload is required");
        assert!(
            matches!(
                error,
                ScenarioError::MissingKey {
                    level: "root",
                    key: "payload"
                }
            ),
            "got {error:?}"
        );
    }

    #[test]
    fn child_declared_runtime_verdicts_are_refused_as_states() {
        let text = happy_text().replace("state: completed", "state: timed_out");
        let error = parse_scenario(&text).expect_err("timed_out is a runtime verdict");
        assert!(
            matches!(&error, ScenarioError::ValueInvalid { line: 4, key, .. } if key == "state"),
            "got {error:?}"
        );
    }

    #[test]
    fn unknown_tokens_report_their_vocabulary() {
        let behavior = happy_text().replace("behavior: write-result", "behavior: procrastinate");
        let error = parse_scenario(&behavior).expect_err("unknown behavior");
        assert!(
            matches!(&error, ScenarioError::UnknownToken { line: 3, key, .. } if key == "behavior"),
            "got {error:?}"
        );
        let derailment =
            happy_text().replace("state: completed", "state: completed\nderailment: explodes");
        let error = parse_scenario(&derailment).expect_err("unknown derailment");
        assert!(
            matches!(&error, ScenarioError::UnknownToken { line: 5, key, .. } if key == "derailment"),
            "got {error:?}"
        );
    }

    #[test]
    fn probe_knobs_are_bounded_integers() {
        // The ceilings are the DRIVABLE bounds, per knob: every driver in
        // the repository (the shared suite's INSTANT budget, the playback
        // tests' bounded loop) stops at 24 polls. A WriteResult child
        // terminates one poll AFTER its last in-flight probe, so the
        // write knob caps at 23 (terminal on poll 24); the stall child
        // times out ON its probe, so the stall knob caps at 24 exactly.
        let over = happy_text().replace(
            "state: completed",
            "state: completed\nprobes_before_write: 24",
        );
        let error = parse_scenario(&over).expect_err("bounded");
        assert!(
            matches!(&error, ScenarioError::IntegerOutOfRange { key, .. } if key == "probes_before_write"),
            "got {error:?}"
        );
        let at_cap = happy_text().replace(
            "state: completed",
            "state: completed\nprobes_before_write: 23",
        );
        parse_scenario(&at_cap).expect("the write cap itself is accepted");
        // The stall timeout knob times out on its own probe: 24 is exact,
        // 25 is beyond every driver.
        let stall = [
            "scenario: parser-stall-cap",
            "role: builder",
            "behavior: stall",
            "timeout_after_probes: 24",
        ]
        .join("\n");
        parse_scenario(&stall).expect("the timeout cap is accepted");
        let stall_over = stall.replace("timeout_after_probes: 24", "timeout_after_probes: 25");
        let error = parse_scenario(&stall_over).expect_err("bounded");
        assert!(
            matches!(&error, ScenarioError::IntegerOutOfRange { key, .. } if key == "timeout_after_probes"),
            "got {error:?}"
        );
        let not_a_number = happy_text().replace(
            "state: completed",
            "state: completed\nprobes_before_write: three",
        );
        let error = parse_scenario(&not_a_number).expect_err("integers only");
        assert!(
            matches!(&error, ScenarioError::NotAnInteger { key, .. } if key == "probes_before_write"),
            "got {error:?}"
        );
    }

    /// A staircase document: `deep:` followed by `levels` nested block
    /// mappings (each `k:` two columns deeper than its parent, the
    /// deepest carrying an inline scalar). Built as a string — the
    /// depth-bound tests never add fixture files. Root mapping + `deep`
    /// + N nested mappings = N+1 container levels on the recursion path.
    fn mapping_staircase(levels: usize) -> String {
        let mut text = String::from(
            "scenario: depth-probe\nrole: builder\nbehavior: stall\ntimeout_after_probes: 1\ndeep:\n",
        );
        for level in 1..=levels {
            let indent = " ".repeat(2 * level);
            if level == levels {
                text.push_str(&indent);
                text.push_str("k: leaf\n");
            } else {
                text.push_str(&indent);
                text.push_str("k:\n");
            }
        }
        text
    }

    #[test]
    fn nesting_at_the_cap_traverses_one_deeper_is_a_typed_error() {
        // `deep:` + 63 nested mappings = exactly MAX_SCENARIO_DEPTH (64)
        // container levels: the descent completes and hands the tree to
        // validation. The dialect's closed vocabulary means no 64-deep
        // document can VALIDATE, so "parses" here is observed as the
        // validation's UnknownKey — proof the traversal finished inside
        // the bound rather than tripping it.
        let at_cap = parse_scenario(&mapping_staircase(63)).expect_err("deep is no root key");
        assert!(
            matches!(&at_cap, ScenarioError::UnknownKey { key, .. } if key == "deep"),
            "got {at_cap:?}"
        );
        // One level deeper (k64 sits on line 69): the typed refusal,
        // naming the line and the depth that tripped it.
        let over = parse_scenario(&mapping_staircase(64)).expect_err("the cap is a cap");
        assert!(
            matches!(
                &over,
                ScenarioError::DepthExceeded {
                    line: 69,
                    depth: 65
                }
            ),
            "got {over:?}"
        );
    }

    #[test]
    fn a_dash_staircase_beyond_the_cap_is_a_typed_error() {
        // The dash-mapping path threads the same counter: each `- k:`
        // level costs two container depths (block_sequence + item
        // mapping), so 40 levels ≈ depth 81 — beyond the cap.
        let mut text = String::from(
            "scenario: dash-stair\nrole: builder\nbehavior: stall\ntimeout_after_probes: 1\ndeep:\n",
        );
        for level in 0..40 {
            let indent = " ".repeat(2 + 4 * level);
            text.push_str(&indent);
            text.push_str("- k:\n");
        }
        let error = parse_scenario(&text).expect_err("beyond the cap");
        assert!(
            matches!(&error, ScenarioError::DepthExceeded { .. }),
            "got {error:?}"
        );
    }

    #[test]
    fn a_monstrous_staircase_on_a_small_stack_is_typed_not_an_abort() {
        // The repository's recorded defect class: an uncapped descent
        // aborts the PROCESS on stack exhaustion (observed live in this
        // round's fail-first run: "fatal runtime error: stack overflow").
        // Parsing a 2000-level staircase on a deliberately small (512 KiB)
        // stack must yield the typed refusal, never an abort — the bound
        // binds long before the stack does. Measured margins on this
        // profile: the capped descent (~65 container levels) needs
        // ~130-144 KiB, while the uncapped 2000-level descent needs
        // megabytes — both sides of the assertion hold with multiples to
        // spare, so the test still discriminates a caps-regression.
        let error = std::thread::Builder::new()
            .stack_size(512 * 1024)
            .spawn(|| {
                let text = mapping_staircase(2000);
                parse_scenario(&text).err()
            })
            .expect("the small-stack thread spawns")
            .join()
            .expect("the capped descent must not overflow the small stack");
        assert!(
            matches!(error, Some(ScenarioError::DepthExceeded { .. })),
            "got {error:?}"
        );
    }

    #[test]
    fn unplaceable_indentation_is_a_typed_error() {
        let text = format!(
            "{}\n      a stray deeper line with no parent\n",
            happy_text()
        );
        // The stray line follows the final (comment/blank-dropped) content;
        // its line number is pinned by construction.
        let error = parse_scenario(&text).expect_err("stray lines never silently drop");
        assert!(
            matches!(
                &error,
                ScenarioError::UnplaceableLine {
                    line: 24,
                    indent: 6
                }
            ),
            "got {error:?}"
        );
    }

    #[test]
    fn effect_paths_are_contained_at_parse_time() {
        for bad in ["../escape.txt", "/absolute/path.txt", "with space.txt"] {
            let text = happy_text().replace(
                "    content: PASS the report exists",
                &format!("    content: x\n  - path: {bad}\n    content: y"),
            );
            let error = parse_scenario(&text).expect_err("effect paths are contained");
            assert!(
                matches!(&error, ScenarioError::ValueInvalid { key, .. } if key == "workspace_effects.path"),
                "path {bad:?}: got {error:?}"
            );
        }
    }

    #[test]
    fn payload_keys_are_required_and_closed() {
        let missing = happy_text().replace("  blocked_on: []\n", "");
        let error = parse_scenario(&missing).expect_err("all payload keys required");
        assert!(
            matches!(
                &error,
                ScenarioError::MissingKey {
                    level: "payload",
                    key: "blocked_on"
                }
            ),
            "got {error:?}"
        );
        let extra = happy_text().replace("  blocked_on: []", "  blocked_on: []\n  bonus: true");
        let error = parse_scenario(&extra).expect_err("payload keys closed");
        assert!(
            matches!(&error, ScenarioError::UnknownKey { key, .. } if key == "bonus"),
            "got {error:?}"
        );
    }

    #[test]
    fn attempted_ids_validate_against_the_domain_grammar() {
        let bad = happy_text().replace("[AK-014, AK-015]", "[ak-14]");
        let error = parse_scenario(&bad).expect_err("unit ids are §5.2");
        assert!(
            matches!(&error, ScenarioError::ValueInvalid { key, .. } if key == "attempted"),
            "got {error:?}"
        );
        assert!(
            UnitId::new("AK-014").is_ok(),
            "the grammar the parser enforces is the domain's own"
        );
    }

    #[test]
    fn artifact_items_need_both_fields_with_a_valid_ref() {
        let missing = happy_text().replace("      evidence_ref: evd-000042\n", "");
        let error = parse_scenario(&missing).expect_err("both fields required");
        assert!(
            matches!(
                &error,
                ScenarioError::MissingKey {
                    level: "artifacts",
                    key: "evidence_ref"
                }
            ),
            "got {error:?}"
        );
        let invalid = happy_text().replace("evd-000042", "not a valid ref");
        let error = parse_scenario(&invalid).expect_err("evidence refs are typed");
        assert!(
            matches!(&error, ScenarioError::ValueInvalid { key, .. } if key == "artifacts.evidence_ref"),
            "got {error:?}"
        );
    }

    #[test]
    fn self_check_exit_codes_are_integers() {
        let bad = happy_text().replace("      exit_code: 0", "      exit_code: zero");
        let error = parse_scenario(&bad).expect_err("exit codes are integers");
        assert!(
            matches!(&error, ScenarioError::NotAnInteger { key, .. } if key == "self_checks.exit_code"),
            "got {error:?}"
        );
        let negative = happy_text().replace("      exit_code: 0", "      exit_code: -1");
        parse_scenario(&negative).expect("signal-death codes are legal");
    }

    #[test]
    fn empty_flow_entries_are_refused() {
        let bad = happy_text().replace("[AK-014, AK-015]", "[AK-014, , AK-015]");
        let error = parse_scenario(&bad).expect_err("empty flow entries refused");
        assert!(
            matches!(&error, ScenarioError::MalformedItem { line: 6, .. }),
            "got {error:?}"
        );
    }

    #[test]
    fn an_empty_document_reports_its_first_missing_key() {
        let error = parse_scenario("").expect_err("no document at all");
        assert!(
            matches!(
                &error,
                ScenarioError::MissingKey {
                    level: "root",
                    key: "scenario"
                }
            ),
            "got {error:?}"
        );
    }

    #[test]
    fn a_missing_fixture_file_is_a_typed_error() {
        let error = load_scenario("no-such-fixture").expect_err("absent fixtures are typed");
        assert!(
            matches!(&error, ScenarioError::FixtureUnreadable { name, .. } if name == "no-such-fixture"),
            "got {error:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Rendering
    // -----------------------------------------------------------------------

    #[test]
    fn the_rendered_document_is_a_valid_result_contract() {
        let scenario = parse_scenario(&happy_text()).expect("parses");
        let text = scenario.result_document_text(&dispatch_spec());
        let result = read_result_contract(
            &text,
            &InvocationId::new(ULID).expect("valid id"),
            &Digest::parse(DIGEST).expect("valid digest"),
        )
        .expect("valid by construction");
        assert_eq!(result.role(), &RoleId::Builder);
        assert_eq!(result.state(), RoleResultState::Completed);
        let payload = result.payload();
        let gauntlet_ports::role_result::RolePayload::BuilderResult(parts) = payload else {
            panic!("the fixture renderer writes the builder payload")
        };
        assert_eq!(parts.attempted().len(), 2);
        assert_eq!(parts.artifacts().len(), 1);
        assert_eq!(parts.claims().len(), 2);
    }

    #[test]
    fn a_foreign_invocation_id_derailment_is_grammar_valid_but_mismatched() {
        let text = happy_text().replace(
            "state: completed",
            "state: completed\nderailment: foreign-invocation-id",
        );
        let scenario = parse_scenario(&text).expect("parses");
        let document = scenario.result_document_text(&dispatch_spec());
        // The document alone decodes (grammar-valid foreign id)…
        let decoded = RoleResult::decode(&document).expect("grammar-valid by construction");
        // …but it fails the identity check with the contract_mismatch class.
        let error = read_result_contract(
            &document,
            &InvocationId::new(ULID).expect("valid id"),
            &Digest::parse(DIGEST).expect("valid digest"),
        )
        .expect_err("foreign identity");
        assert!(
            matches!(&error, RoleResultError::InvocationIdMismatch { .. }),
            "got {error:?}"
        );
        assert_eq!(
            error.failure_class(),
            gauntlet_ports::agent_runner::InvocationFailure::ContractMismatch
        );
        assert_ne!(decoded.invocation_id().as_str(), ULID);
    }

    #[test]
    fn a_stale_digest_derailment_is_well_formed_but_mismatched() {
        let text = happy_text().replace(
            "state: completed",
            "state: completed\nderailment: stale-envelope-digest",
        );
        let scenario = parse_scenario(&text).expect("parses");
        let document = scenario.result_document_text(&dispatch_spec());
        let error = read_result_contract(
            &document,
            &InvocationId::new(ULID).expect("valid id"),
            &Digest::parse(DIGEST).expect("valid digest"),
        )
        .expect_err("stale digest");
        assert!(
            matches!(&error, RoleResultError::EnvelopeDigestMismatch { .. }),
            "got {error:?}"
        );
        assert_eq!(
            error.failure_class(),
            gauntlet_ports::agent_runner::InvocationFailure::ContractMismatch
        );
    }

    #[test]
    fn unknown_field_derailments_render_strict_rejections() {
        for (token, needle) in [
            ("unknown-envelope-field", "fabricated_envelope_field"),
            ("unknown-nested-field", "fabricated_nested_field"),
        ] {
            let text = happy_text().replace(
                "state: completed",
                &format!("state: completed\nderailment: {token}"),
            );
            let scenario = parse_scenario(&text).expect("parses");
            let document = scenario.result_document_text(&dispatch_spec());
            let error = RoleResult::decode(&document).expect_err("strict decode rejects");
            assert!(
                matches!(&error, RoleResultError::UnknownField { field, .. } if field == needle),
                "{token}: got {error:?}"
            );
        }
    }

    #[test]
    fn a_raw_malformed_derailment_is_not_json() {
        let text = happy_text().replace(
            "state: completed",
            "state: completed\nderailment: raw-malformed",
        );
        let scenario = parse_scenario(&text).expect("parses");
        let document = scenario.result_document_text(&dispatch_spec());
        assert!(
            matches!(RoleResult::decode(&document), Err(RoleResultError::Json(_))),
            "raw malformed text never parses"
        );
    }

    #[test]
    fn probes_before_write_models_in_flight_work() {
        let text = happy_text().replace(
            "state: completed",
            "state: completed\nprobes_before_write: 2",
        );
        let scenario = parse_scenario(&text).expect("parses");
        assert_eq!(scenario.probes_before_write(), 2);
    }
}

// ---------------------------------------------------------------------------
// COR-F2 (falsifier round 1, F-2): the loader's stem is a fixture NAME.
// Containment fires BEFORE any path join, so no traversal stem can even
// name a file outside the frozen-locator fixtures dir — the `scenario:`
// identity check stays a consistency pin, never the containment guard.
// ---------------------------------------------------------------------------

#[test]
fn a_traversal_or_pathed_stem_is_refused_before_any_join() {
    // The falsifier's own delivery stem, plus the separator/'.'/traversal
    // classes: every one is a typed ValueInvalid naming the stem rule.
    let refused = [
        "../../../../../../../../../../tmp/falsifier-t20/forged-report",
        "/etc/passwd",
        "sub/dir/happy",
        "./happy",
        "../happy",
        "..",
        ".",
        "happy/",
        "",
        "white space",
    ];
    // (A backslash stem is NOT refused on this platform: `std::path::
    // is_separator` is platform-truthful, and a backslash cannot traverse
    // on a POSIX host — it can only name an absent fixture, which the
    // loader's typed unreadable error already covers.)
    for stem in refused {
        let error = super::load_scenario(stem).expect_err("a non-simple stem is refused");
        assert!(
            matches!(error, ScenarioError::ValueInvalid { ref key, .. } if key == "scenario"),
            "stem {stem:?} must be refused as a stem-grammar violation, got {error:?}"
        );
    }
}

#[test]
fn a_simple_fixture_stem_still_loads_from_the_frozen_locator() {
    // The guard keeps the loader's legitimate reach: real fixture stems
    // load, and the loaded identity still agrees with the stem.
    let scenario = super::load_scenario("e2e-happy").expect("the shipped fixture loads");
    assert_eq!(scenario.name(), "e2e-happy");
    // A simple name that names no fixture is the typed unreadable error —
    // NOT a traversal refusal (the stem grammar passed; the fixture is
    // simply absent).
    let error = super::load_scenario("no-such-fixture").expect_err("absent fixture");
    assert!(matches!(error, ScenarioError::FixtureUnreadable { .. }));
}
