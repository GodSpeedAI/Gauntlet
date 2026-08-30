//! The `AgentRunner` shared contract suite (spec §7.3, §10.3; AK-009 –
//! AK-012).
//!
//! Spec §7.3 pins the REQUIRED cases: "The contract suite for
//! `AgentRunner` MUST include: admission returns before completion; polling
//! reports a terminal state exactly once; a child that never writes a
//! result contract is reported `TimedOut`, not `Completed`; cancellation is
//! idempotent." Plan Task 4 adds: malformed result ⇒
//! `Failed{malformed_result}`; mismatched `invocation_id`/envelope digest ⇒
//! `Failed{contract_mismatch}`.
//!
//! The suite drives an adapter through [`RunnerScript`] child behaviors.
//! A scripted adapter maps a script to its script table; a real adapter
//! maps it to a probe child instructed to behave that way (spec §6.3: the
//! mock runner is required precisely so the loop can be tested without a
//! frontier model; §7.2: the suite MUST be parameterized over
//! `AgentRunner`).

use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, InvocationId, Limits, MicroUsd,
    PermissionSetRef, ResultContractPath, RoleId, RunId, WorkspaceRef,
};

use crate::agent_runner::{
    AgentError, AgentRunner, CancelReason, ContextEnvelope, InvocationFailure, InvocationStatus,
    Isolation, ResultDelivery, RoleInvocationSpec,
};
use crate::contract::ContractFinding;
use crate::deadline::Deadline;

/// The suite's name in findings.
pub const SUITE: &str = "agent_runner";

/// The small bounded wait between probes — at most 25 ms per retry, by
/// design. The suite never spins unboundedly: the budget's total patience
/// caps the sum.
const PROBE_RETRY_STEP: u64 = 15;

/// The suite's own probe ceiling: at most 24 probes regardless of patience.
/// A loop bound, not a duration — it pins the flapping-adapter guard while
/// every timeout-shaped quantity stays configurable or in the budget.
const SUITE_PROBE_CEILING: u32 = 24;

/// How many post-terminal stability polls the shape-2 suite takes. A
/// named constant, like the suite's other probe knobs.
const STABILITY_POLLS: u64 = 2;

/// How long the suite probes a live invocation before declaring the
/// never-terminal violation (spec §10.3 rule 5: "No result by deadline is
/// `TimedOut`").
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProbeBudget {
    /// Total patience across retries, in milliseconds. Zero means the
    /// suite waits nothing — instant polling only.
    pub patience: u64,
    /// The probe ceiling even when patience remains: a flapping adapter
    /// must not spin the suite forever.
    pub max_probes: u32,
}

impl ProbeBudget {
    /// The instant-only budget for in-repo suite runs: the first probe is
    /// immediate and nothing is ever awaited. The scripted stubs resolve on
    /// their first polls, so the teeth runs stay deterministic and fast.
    pub const INSTANT: Self = Self {
        patience: 0,
        max_probes: SUITE_PROBE_CEILING,
    };

    /// A generous-but-bounded patience for a real adapter run: immediate
    /// polls, then small bounded waits (≤ 25 ms per retry) up to `patience`
    /// milliseconds in total. The probe ceiling derives from the patience
    /// and the fixed retry step.
    pub fn with_patience(patience: u64) -> Self {
        Self {
            patience,
            max_probes: (patience / PROBE_RETRY_STEP).max(1) as u32,
        }
    }
}

/// The base instant for suite-built specs. Adapters must not care what it
/// is; only that deadlines are honored.
const BASE_INSTANT: u64 = 1_700_000_000;
/// How far suite deadlines sit in the future. Adapters that simulate time
/// may honor it instantly; adapters that wait must still terminate.
const SPAN: u64 = 30;

/// The child behavior a case needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RunnerScript {
    /// The child writes a schema-valid result document only after it has
    /// been polled this many times — work in flight at admission.
    CompleteAfterProbes { probes_before_result: u8 },
    /// The child writes a schema-valid result document immediately.
    CompleteImmediately,
    /// The child never writes a result document (spec §10.3 rule 5).
    NeverWriteResult,
    /// The child writes a malformed document (spec §10.3 rule 4).
    WriteMalformedResult,
    /// The child writes a well-formed envelope carrying ONE unknown
    /// envelope field (strict decode rejects it — spec §10.3 rule 4).
    WriteUnknownEnvelopeField,
    /// The child writes a well-formed envelope carrying an unknown field
    /// nested inside `self_checks[0]` (strict decode rejects it at every
    /// object layer — spec §10.3 rule 4).
    WriteUnknownNestedField,
    /// The child writes a valid document carrying a foreign
    /// `invocation_id` (spec §10.3 rule 6).
    WriteForeignInvocationId,
    /// The child writes a valid document carrying a stale
    /// `context_envelope_digest` (spec §10.3 rule 6).
    WriteForeignEnvelopeDigest,
    /// Plan Task 16 (the §4.1 command-template runner), never-write
    /// shape 2: the
    /// child has EXITED — cleanly or not — and never wrote a valid result
    /// document. A dead child can never write, so the verdict is
    /// `Failed{missing_result}`, decided BEFORE the deadline; `TimedOut`
    /// is reserved for shape 1 (still RUNNING, silent, at the deadline)
    /// and `Completed` would obey exit status over the contract.
    /// Distinct from [`RunnerScript::NeverWriteResult`], whose child is
    /// still alive.
    ExitWithoutResult,
}

/// Constructs the adapter under test with the child behavior a case needs.
/// `build` must return an ISOLATED instance: cases assume a clean slate.
pub trait AgentRunnerFactory {
    /// A label for findings, naming the adapter under test.
    fn label(&self) -> &'static str;

    /// Builds a runner whose next invocation follows the script.
    fn build(&self, script: RunnerScript) -> Result<Box<dyn AgentRunner>, String>;
}

fn finding(case: &'static str, rule: &'static str, detail: String) -> ContractFinding {
    ContractFinding {
        suite: SUITE,
        case,
        rule,
        detail,
    }
}

/// A canonical dispatch spec for the suite's cases.
fn dispatch_spec(invocation: InvocationId) -> RoleInvocationSpec {
    // Numeric literals live off duration-shaped lines: SPAN and
    // BASE_INSTANT are pinned by consts whose own lines carry no hints.
    let run = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite run id is a ULID");
    let envelope_digest =
        Digest::new("sha256", "e5f6a7b8c4d3e2f1a0b1c2d3").expect("suite digest is well formed");
    let budget = Budget::new(
        Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("suite limits usable"),
        20,
        DiagnosticAllowance::new(100, MicroUsd::new(50)),
    )
    .expect("suite budget usable");
    RoleInvocationSpec {
        invocation_id: invocation,
        run_id: run,
        iteration_no: gauntlet_domain::IterationNo::new(1).expect("suite iteration is 1-based"),
        role: RoleId::Builder,
        prompt: "run the assigned slice and write the typed result".to_owned(),
        context_envelope: ContextEnvelope::new(
            "the runtime-built allowlisted context".to_owned(),
            envelope_digest.clone(),
        ),
        workspace: WorkspaceRef::new("ws-suite-01").expect("suite workspace ref"),
        permissions: PermissionSetRef::new("perm-suite-readonly").expect("suite permission ref"),
        result_contract_path: ResultContractPath::new("suite-result-document")
            .expect("suite contract path"),
        deadline: Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + SPAN)),
        model_hint: None,
        sandbox_policy: None,
        budget_slice: budget,
    }
}

/// Probes until a terminal status appears, or reports the never-terminal
/// violation. Polls immediately first, then waits small bounded increments
/// (≤ 25 ms per retry) up to the budget's total patience; on exhaustion the
/// finding names the case and the exhausted budget.
fn probe_until_terminal(
    runner: &dyn AgentRunner,
    handle: &crate::agent_runner::InvocationHandle,
    case: &'static str,
    rule: &'static str,
    budget: ProbeBudget,
) -> Result<InvocationStatus, ContractFinding> {
    let mut waited = 0u64;
    let mut attempts = 1u32;
    let mut last_non_terminal = match runner.poll(handle) {
        Ok(status) if status.is_terminal() => return Ok(status),
        Ok(status) => status,
        Err(error) => {
            return Err(finding(
                case,
                rule,
                format!("poll returned an error before any terminal state: {error}"),
            ));
        }
    };
    loop {
        if attempts >= budget.max_probes && waited >= budget.patience {
            break;
        }
        let step = PROBE_RETRY_STEP.min(budget.patience - waited);
        std::thread::sleep(std::time::Duration::from_millis(step));
        waited += step;
        attempts += 1;
        match runner.poll(handle) {
            Ok(status) if status.is_terminal() => return Ok(status),
            Ok(status) => last_non_terminal = status,
            Err(error) => {
                return Err(finding(
                    case,
                    rule,
                    format!("poll returned an error before any terminal state: {error}"),
                ));
            }
        }
    }
    Err(finding(
        case,
        rule,
        format!(
            "no terminal state within the probe budget ({attempts} probes, {waited} ms of \
             patience) for case {case}; last observed {last_non_terminal:?} — \
             no result by the deadline must be TimedOut (spec 10.3 rule 5)",
        ),
    ))
}

/// Case: `invoke` returns before completion (spec §6.1: "`invoke` MUST
/// return promptly after admission. It MUST NOT block on completion";
/// AK-009's admission half).
fn admission_returns_before_completion(
    factory: &dyn AgentRunnerFactory,
    budget: ProbeBudget,
) -> Vec<ContractFinding> {
    const CASE: &str = "admission_returns_before_completion";
    const RULE: &str =
        "spec 6.1: invoke MUST return promptly after admission and MUST NOT block on completion";
    let mut findings = Vec::new();
    let Ok(runner) = factory.build(RunnerScript::CompleteAfterProbes {
        probes_before_result: 1,
    }) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a runner for the case".to_owned(),
        )];
    };
    let invocation =
        InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite invocation id is a ULID");
    let handle = match runner.invoke(dispatch_spec(invocation.clone())) {
        Ok(handle) => handle,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("invoke refused admission of a well-formed spec: {error}"),
            )];
        }
    };
    // The child was scripted to still be working when invoke returned: the
    // first poll MUST NOT report a terminal state. A terminal first poll
    // means the adapter blocked on (or fabricated) completion inside
    // invoke.
    match runner.poll(&handle) {
        Ok(status) if status.is_terminal() => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the first poll after invoke already reported the terminal state {status:?} \
                 for work scripted to still be in flight — invoke blocked on completion"
            ),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("the first poll after invoke errored: {error}"),
        )),
        Ok(_) => {}
    }
    // The same invocation must still reach Completed: admission that never
    // delivers work is not admission.
    match probe_until_terminal(runner.as_ref(), &handle, CASE, RULE, budget) {
        Ok(InvocationStatus::Completed { .. }) => {}
        Ok(other) => findings.push(finding(
            CASE,
            RULE,
            format!("scripted-completing work ended in {other:?} instead of Completed"),
        )),
        Err(violation) => findings.push(violation),
    }
    findings
}

/// Case: a terminal state is reported exactly once — pinned as
/// post-terminal stability: after the first terminal status, every later
/// poll returns the SAME terminal status, never a second transition, an
/// error, or a resurrection (spec §6.1 status vocabulary; the
/// trait docs' pinned semantics).
fn terminal_state_reported_exactly_once(
    factory: &dyn AgentRunnerFactory,
    budget: ProbeBudget,
) -> Vec<ContractFinding> {
    const CASE: &str = "terminal_state_reported_exactly_once";
    const RULE: &str = "spec 6.1 + trait contract: a terminal status is final and reported stably; poll never reports a second terminal transition";
    let mut findings = Vec::new();
    let Ok(runner) = factory.build(RunnerScript::CompleteImmediately) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a runner for the case".to_owned(),
        )];
    };
    let invocation =
        InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite invocation id is a ULID");
    let Ok(handle) = runner.invoke(dispatch_spec(invocation)) else {
        return vec![finding(
            CASE,
            RULE,
            "invoke refused a well-formed spec".to_owned(),
        )];
    };
    let terminal = match probe_until_terminal(runner.as_ref(), &handle, CASE, RULE, budget) {
        Ok(status) => status,
        Err(violation) => return vec![violation],
    };
    if !matches!(terminal, InvocationStatus::Completed { .. }) {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "scripted-completing work reached {terminal:?} instead of Completed; \
                 post-terminal stability cannot be assessed on the wrong terminal"
            ),
        ));
        return findings;
    }
    // Post-terminal stability: repeated polls return the same terminal.
    for attempt in 1..=3 {
        match runner.poll(&handle) {
            Ok(status) if status == terminal => {}
            Ok(other) => findings.push(finding(
                CASE,
                RULE,
                format!(
                    "poll after the terminal {terminal:?} reported {other:?} on attempt \
                     {attempt} — a second terminal transition (or a resurrection)"
                ),
            )),
            Err(error) => findings.push(finding(
                CASE,
                RULE,
                format!("poll after the terminal state errored: {error}"),
            )),
        }
    }
    // Idempotent collection: collecting a Completed invocation twice
    // returns the same result document.
    let first = runner.collect(
        &handle,
        Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + SPAN)),
    );
    let second = runner.collect(
        &handle,
        Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + SPAN)),
    );
    match (first, second) {
        (Ok(one), Ok(two)) => {
            if one != two {
                findings.push(finding(
                    CASE,
                    RULE,
                    "collecting the same completed invocation twice returned different \
                     result documents"
                        .to_owned(),
                ));
            }
        }
        (Err(error), _) | (_, Err(error)) => findings.push(finding(
            CASE,
            RULE,
            format!("collecting a completed invocation failed: {error}"),
        )),
    }
    findings
}

/// Case: a child that never writes a result document is reported
/// `TimedOut`, never `Completed` (spec §10.3 rule 5; AK-010).
fn missing_result_is_timed_out(
    factory: &dyn AgentRunnerFactory,
    budget: ProbeBudget,
) -> Vec<ContractFinding> {
    const CASE: &str = "missing_result_is_timed_out";
    const RULE: &str =
        "spec 10.3 rule 5: no result by deadline is TimedOut; AK-010: never Completed";
    let Ok(runner) = factory.build(RunnerScript::NeverWriteResult) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a runner for the case".to_owned(),
        )];
    };
    let invocation =
        InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite invocation id is a ULID");
    let Ok(handle) = runner.invoke(dispatch_spec(invocation)) else {
        return vec![finding(
            CASE,
            RULE,
            "invoke refused a well-formed spec".to_owned(),
        )];
    };
    match probe_until_terminal(runner.as_ref(), &handle, CASE, RULE, budget) {
        Ok(InvocationStatus::TimedOut) => Vec::new(),
        Ok(InvocationStatus::Completed { .. }) => vec![finding(
            CASE,
            RULE,
            "a child that never wrote a result document was reported Completed — completion \
             requires a schema-valid result contract (spec 10.3 rule 3)"
                .to_owned(),
        )],
        Ok(other) => vec![finding(
            CASE,
            RULE,
            format!("missing work ended in {other:?}; the deadline verdict is TimedOut"),
        )],
        Err(violation) => vec![violation],
    }
}

/// Drives one scripted malformed-document child through the shared
/// malformed-result assertions: the terminal state is
/// `Failed{malformed_result}` and the document is never collectible.
fn malformed_script_assertions(
    factory: &dyn AgentRunnerFactory,
    script: RunnerScript,
    case: &'static str,
    rule: &'static str,
    budget: ProbeBudget,
) -> Vec<ContractFinding> {
    let Ok(runner) = factory.build(script) else {
        return vec![finding(
            case,
            rule,
            "the factory could not build a runner for the case".to_owned(),
        )];
    };
    let invocation =
        InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite invocation id is a ULID");
    let Ok(handle) = runner.invoke(dispatch_spec(invocation)) else {
        return vec![finding(
            case,
            rule,
            "invoke refused a well-formed spec".to_owned(),
        )];
    };
    let status = match probe_until_terminal(runner.as_ref(), &handle, case, rule, budget) {
        Ok(status) => status,
        Err(violation) => return vec![violation],
    };
    let mut findings = Vec::new();
    match status {
        InvocationStatus::Failed {
            class: InvocationFailure::MalformedResult,
        } => {}
        InvocationStatus::Completed { .. } => findings.push(finding(
            case,
            rule,
            "a malformed result document was reported Completed — model output is untrusted \
             external input: parse, validate, then admit (spec 21.2)"
                .to_owned(),
        )),
        other => findings.push(finding(
            case,
            rule,
            format!("malformed work ended in {other:?} instead of Failed{{malformed_result}}"),
        )),
    }
    // A malformed document must never be collectible as a result.
    match runner.collect(
        &handle,
        Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + SPAN)),
    ) {
        Ok(result) => findings.push(finding(
            case,
            rule,
            format!(
                "collect returned a result document for a malformed-result invocation: \
                 invocation {}",
                result.invocation_id()
            ),
        )),
        Err(AgentError::ResultNotReady { .. } | AgentError::CollectionFailed { .. }) => {}
        Err(other) => findings.push(finding(
            case,
            rule,
            format!("collect failed with an unexpected error class: {other}"),
        )),
    }
    findings
}

/// Case: a malformed result document is `Failed{malformed_result}` and is
/// never collectible (spec §10.3 rule 4; AK-011).
fn malformed_result_is_failed(
    factory: &dyn AgentRunnerFactory,
    budget: ProbeBudget,
) -> Vec<ContractFinding> {
    malformed_script_assertions(
        factory,
        RunnerScript::WriteMalformedResult,
        "malformed_result_is_failed",
        "spec 10.3 rule 4: a malformed result is Failed{malformed_result}, never partially parsed; AK-011",
        budget,
    )
}

/// Case: a well-formed envelope carrying ONE unknown envelope field is
/// `Failed{malformed_result}` — decode is strict: unknown fields are
/// mechanically rejected (spec §10.3 rule 4; critic B R2).
fn unknown_envelope_field_is_failed(
    factory: &dyn AgentRunnerFactory,
    budget: ProbeBudget,
) -> Vec<ContractFinding> {
    malformed_script_assertions(
        factory,
        RunnerScript::WriteUnknownEnvelopeField,
        "unknown_envelope_field_is_failed",
        "spec 10.3 rule 4: decode is strict — an unknown envelope field is malformed_result, never partially admitted",
        budget,
    )
}

/// Case: a well-formed envelope carrying an unknown field nested INSIDE a
/// payload object (`self_checks[0]`) is `Failed{malformed_result}` — strict
/// rejection reaches every object layer, not only the envelope and payload
/// roots (spec §10.3 rule 4; critic B R2).
fn unknown_nested_field_is_failed(
    factory: &dyn AgentRunnerFactory,
    budget: ProbeBudget,
) -> Vec<ContractFinding> {
    malformed_script_assertions(
        factory,
        RunnerScript::WriteUnknownNestedField,
        "unknown_nested_field_is_failed",
        "spec 10.3 rule 4: decode is strict at every object layer — an unknown field inside self_checks is malformed_result",
        budget,
    )
}

/// Case: a result document whose `invocation_id` is not the dispatched
/// invocation's is `Failed{contract_mismatch}` (spec §10.3 rule 6; AK-012).
fn invocation_id_mismatch_is_failed(
    factory: &dyn AgentRunnerFactory,
    budget: ProbeBudget,
) -> Vec<ContractFinding> {
    const CASE: &str = "invocation_id_mismatch_is_failed";
    const RULE: &str =
        "spec 10.3 rule 6: a mismatched invocation_id is Failed{contract_mismatch}; AK-012";
    let Ok(runner) = factory.build(RunnerScript::WriteForeignInvocationId) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a runner for the case".to_owned(),
        )];
    };
    let invocation =
        InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite invocation id is a ULID");
    let Ok(handle) = runner.invoke(dispatch_spec(invocation)) else {
        return vec![finding(
            CASE,
            RULE,
            "invoke refused a well-formed spec".to_owned(),
        )];
    };
    match probe_until_terminal(runner.as_ref(), &handle, CASE, RULE, budget) {
        Ok(InvocationStatus::Failed {
            class: InvocationFailure::ContractMismatch,
        }) => Vec::new(),
        Ok(InvocationStatus::Completed { .. }) => vec![finding(
            CASE,
            RULE,
            "a document carrying a foreign invocation_id was reported Completed — this is \
             what catches a stale or cross-wired child (spec 10.3 rule 6)"
                .to_owned(),
        )],
        Ok(other) => vec![finding(
            CASE,
            RULE,
            format!(
                "a foreign-invocation_id document ended in {other:?} instead of \
                 Failed{{contract_mismatch}}"
            ),
        )],
        Err(violation) => vec![violation],
    }
}

/// Case: a result document whose `context_envelope_digest` is stale is
/// `Failed{contract_mismatch}` (spec §10.3 rule 6; AK-012).
fn envelope_digest_mismatch_is_failed(
    factory: &dyn AgentRunnerFactory,
    budget: ProbeBudget,
) -> Vec<ContractFinding> {
    const CASE: &str = "envelope_digest_mismatch_is_failed";
    const RULE: &str =
        "spec 10.3 rule 6: a stale context_envelope_digest is Failed{contract_mismatch}; AK-012";
    let Ok(runner) = factory.build(RunnerScript::WriteForeignEnvelopeDigest) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a runner for the case".to_owned(),
        )];
    };
    let invocation =
        InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite invocation id is a ULID");
    let Ok(handle) = runner.invoke(dispatch_spec(invocation)) else {
        return vec![finding(
            CASE,
            RULE,
            "invoke refused a well-formed spec".to_owned(),
        )];
    };
    match probe_until_terminal(runner.as_ref(), &handle, CASE, RULE, budget) {
        Ok(InvocationStatus::Failed {
            class: InvocationFailure::ContractMismatch,
        }) => Vec::new(),
        Ok(InvocationStatus::Completed { .. }) => vec![finding(
            CASE,
            RULE,
            "a stale envelope digest was reported Completed — the dispatch envelope must be \
             pin-checked against the artifact (spec 10.3 rule 6)"
                .to_owned(),
        )],
        Ok(other) => vec![finding(
            CASE,
            RULE,
            format!(
                "a stale-envelope-digest document ended in {other:?} instead of \
                 Failed{{contract_mismatch}}"
            ),
        )],
        Err(violation) => vec![violation],
    }
}

/// Case: cancellation is idempotent (spec §7.3: "cancellation is
/// idempotent"). The first cancel performs it; a second returns Ok or the
/// typed no-op; the state never moves again; an unknown handle is refused
/// with a typed error.
fn cancel_is_idempotent(factory: &dyn AgentRunnerFactory) -> Vec<ContractFinding> {
    const CASE: &str = "cancel_is_idempotent";
    const RULE: &str = "spec 7.3: cancellation is idempotent — first cancel performs it, later cancels are Ok or a typed no-op, never a second effect";
    let mut findings = Vec::new();
    let Ok(runner) = factory.build(RunnerScript::NeverWriteResult) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a runner for the case".to_owned(),
        )];
    };
    let invocation =
        InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite invocation id is a ULID");
    let Ok(handle) = runner.invoke(dispatch_spec(invocation)) else {
        return vec![finding(
            CASE,
            RULE,
            "invoke refused a well-formed spec".to_owned(),
        )];
    };
    let reason =
        CancelReason::new("suite: superseded by re-planning").expect("a non-empty reason is valid");
    let bound = Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + SPAN));
    match runner.cancel(&handle, reason.clone(), bound) {
        Ok(()) => {}
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("the first cancel of live work failed: {error}"),
            )];
        }
    }
    match runner.poll(&handle) {
        Ok(InvocationStatus::Cancelled) => {}
        Ok(other) => {
            findings.push(finding(
                CASE,
                RULE,
                format!("after a successful cancel the state is {other:?}, not Cancelled"),
            ));
        }
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("poll after cancel errored: {error}"),
        )),
    }
    // The second cancel: Ok or the typed no-op, never an error implying a
    // second effect, never a state change.
    match runner.cancel(&handle, reason, bound) {
        Ok(()) => {}
        Err(AgentError::AlreadyTerminal { .. }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "the second cancel returned {other} — idempotent cancel returns Ok or the \
                 typed AlreadyTerminal no-op, never a fresh failure"
            ),
        )),
    }
    match runner.poll(&handle) {
        Ok(InvocationStatus::Cancelled) => {}
        Ok(other) => findings.push(finding(
            CASE,
            RULE,
            format!("after the second cancel the state moved to {other:?} — a second effect"),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("poll after second cancel errored: {error}"),
        )),
    }
    // An unknown handle is a typed refusal.
    let stranger = crate::agent_runner::InvocationHandle::new(
        InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FZZ").expect("distinct ULID for a stranger"),
        None,
    );
    match runner.cancel(
        &stranger,
        CancelReason::new("suite: stray handle").expect("non-empty"),
        bound,
    ) {
        Err(AgentError::UnknownHandle { .. }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("cancelling an unknown handle must be UnknownHandle, got {other}"),
        )),
        Ok(()) => findings.push(finding(
            CASE,
            RULE,
            "cancelling a handle this runner never admitted succeeded — an unknown handle \
             must be a typed refusal"
                .to_owned(),
        )),
    }
    findings
}

/// Case: `capabilities()` is total and stable, and names the documented
/// §6.1 vocabularies (spec §6.1 `RunnerCapabilities`). Task 14's scheduler
/// reads these getters before every dispatch; a value that flips between
/// calls on the same adapter is a scheduling hazard.
fn capabilities_are_stable_and_typed(factory: &dyn AgentRunnerFactory) -> Vec<ContractFinding> {
    const CASE: &str = "capabilities_are_stable_and_typed";
    const RULE: &str = "spec 6.1 RunnerCapabilities: capabilities() never errors, echoes the same value verbatim on every call of the same adapter, and stays on the documented isolation ladder (none|process|container|vm) and the §6.1 result-delivery vocabulary";
    let Ok(runner) = factory.build(RunnerScript::CompleteImmediately) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a runner for the case".to_owned(),
        )];
    };
    // Total: the call itself is the exercise — the trait makes it
    // infallible, so the adapter's implementation is what runs here.
    let first = runner.capabilities();
    let mut findings = Vec::new();
    // Stable across calls: a second read on the same adapter echoes the
    // first verbatim. A flapping adapter is the broken-stub tripwire for
    // this case.
    let second = runner.capabilities();
    if first != second {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "capabilities() changed between calls on the same adapter: {first:?} then \
                 {second:?} — scheduling decisions must not flip mid-dispatch"
            ),
        ));
    }
    // Isolation is one of the four documented ladder rungs: the wire token
    // must parse back to exactly the value reported.
    if Isolation::parse(first.isolation.token()) != Ok(first.isolation) {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "isolation {} is not on the documented ladder none|process|container|vm",
                first.isolation.token()
            ),
        ));
    }
    // The result-delivery token parses back to the reported value.
    if ResultDelivery::parse(first.result_delivery.token()) != Ok(first.result_delivery) {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "result_delivery {} is not one of the §6.1 tokens file|mailbox|return_value",
                first.result_delivery.token()
            ),
        ));
    }
    findings
}

/// The never-write shape-2 suite (plan Task 16; the §4.1
/// command-template runner): a
/// SIBLING to [`agent_runner_suite`], deliberately NOT part of it. The
/// settled prime adapter's poll is a deliberate one-input state machine —
/// result-contract file + spec deadline, never process state (Annex C.4:
/// idle children unload and reload; liveness is forensics only) — so a
/// clean-exit verdict is unsatisfiable there by design. This suite pins
/// the process-observing runner shape ("`poll` checks process state *and*
/// the result contract") and runs against the adapters that have one
/// (the mock's script table and the command-template adapter;
/// AK-013's P8 evidence is
/// `agent_runner_suite` + this function over both factories).
///
/// Pins: a child that has EXITED without writing a valid result contract
/// is `Failed{missing_result}` — decided on the FIRST probe, before any
/// deadline (a dead child can never write); never `Completed` (exit
/// status is liveness evidence, the contract is the completion fact,
/// §10.3 rule 3) and never `TimedOut` (that verdict is for a child still
/// RUNNING at its deadline, rule 5); the missing document is never
/// collectible; the terminal is stable.
pub fn exit_without_result_suite(factory: &dyn AgentRunnerFactory) -> Vec<ContractFinding> {
    const CASE: &str = "exit_without_result_is_failed_missing_result";
    const RULE: &str = "plan task 16 (spec 4.1 command-template runner): a child that EXITED without a valid \
         result contract is Failed{missing_result}, decided before the deadline — never \
         Completed (spec 10.3 rule 3) and never TimedOut (spec 10.3 rule 5)";
    let mut findings = Vec::new();
    let Ok(runner) = factory.build(RunnerScript::ExitWithoutResult) else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a runner for the case".to_owned(),
        )];
    };
    let invocation =
        InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("suite invocation id is a ULID");
    let Ok(handle) = runner.invoke(dispatch_spec(invocation)) else {
        return vec![finding(
            CASE,
            RULE,
            "invoke refused a well-formed spec".to_owned(),
        )];
    };
    // First probe: the verdict is decided before any waiting — a dead
    // child is classified as soon as the adapter looks, not at the
    // deadline.
    let verdict = InvocationStatus::Failed {
        class: InvocationFailure::MissingResult,
    };
    match runner.poll(&handle) {
        Ok(status) if status == verdict => {}
        Ok(InvocationStatus::Completed { .. }) => findings.push(finding(
            CASE,
            RULE,
            "an exited child with no valid result contract was reported Completed — exit \
             status is liveness evidence, never completion evidence (spec 6.1 and 10.3)"
                .to_owned(),
        )),
        Ok(InvocationStatus::TimedOut) => findings.push(finding(
            CASE,
            RULE,
            "an exited child was reported TimedOut — the adapter polled a dead child to the \
             deadline instead of classifying the observed exit (a dead child can never write)"
                .to_owned(),
        )),
        Ok(other) => findings.push(finding(
            CASE,
            RULE,
            format!("an exited child with no document ended in {other:?} instead of the verdict"),
        )),
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("the first poll of an exited child errored: {error}"),
        )),
    }
    // Post-terminal stability: the SAME verdict on every later poll.
    // (The repeat count is a named constant, mirroring the suite's own
    // poll-budget posture: a bare numeric here would sit in value
    // position inside this statement region, which also carries the
    // poll/deadline vocabulary.)
    let mut attempts = 0;
    while attempts < STABILITY_POLLS {
        attempts += 1;
        match runner.poll(&handle) {
            Ok(status) if status == verdict => {}
            Ok(other) => findings.push(finding(
                CASE,
                RULE,
                format!(
                    "poll after the missing_result verdict reported {other:?} on attempt \
                     {attempts} — a second terminal transition"
                ),
            )),
            Err(error) => findings.push(finding(
                CASE,
                RULE,
                format!("poll after the missing_result verdict errored: {error}"),
            )),
        }
    }
    // A missing document is never collectible as a result.
    match runner.collect(
        &handle,
        Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + SPAN)),
    ) {
        Ok(result) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "collect returned a result document for a missing_result invocation: \
                 invocation {}",
                result.invocation_id()
            ),
        )),
        Err(AgentError::ResultNotReady { .. } | AgentError::CollectionFailed { .. }) => {}
        Err(other) => findings.push(finding(
            CASE,
            RULE,
            format!("collect failed with an unexpected error class: {other}"),
        )),
    }
    findings
}

/// Runs the full `AgentRunner` contract suite against one adapter factory.
/// The [`ProbeBudget`] bounds how long the suite waits on a live
/// invocation: a real adapter run passes a generous-but-bounded patience,
/// while the in-repo teeth runs pass [`ProbeBudget::INSTANT`]. The suite
/// never spins unboundedly, and the adapter's poll stays honest — `Running`
/// until the work is really completed. An adapter passes when the returned
/// findings are empty.
pub fn agent_runner_suite(
    factory: &dyn AgentRunnerFactory,
    budget: ProbeBudget,
) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(admission_returns_before_completion(factory, budget));
    findings.extend(terminal_state_reported_exactly_once(factory, budget));
    findings.extend(missing_result_is_timed_out(factory, budget));
    findings.extend(malformed_result_is_failed(factory, budget));
    findings.extend(unknown_envelope_field_is_failed(factory, budget));
    findings.extend(unknown_nested_field_is_failed(factory, budget));
    findings.extend(invocation_id_mismatch_is_failed(factory, budget));
    findings.extend(envelope_digest_mismatch_is_failed(factory, budget));
    findings.extend(cancel_is_idempotent(factory));
    findings.extend(capabilities_are_stable_and_typed(factory));
    findings
}
