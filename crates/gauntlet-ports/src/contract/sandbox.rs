//! The `Sandbox` shared contract suite (spec §6.2; §17.3, §21.3; AK-047,
//! AK-048, AK-050).
//!
//! Pinned semantics: validation precedes every use — a validated policy
//! wraps, and the wrapped command carries its policy and its validation;
//! a wrap that hands over a validation earned by a DIFFERENT policy is a
//! typed refusal (the structural chain: `wrap` requires the
//! [`PolicyValidation`] that `validate` produced for the SAME policy).
//! A findings-carrying or rung-downgraded validation authorizes nothing —
//! there is no path from a failed validation to an executed command
//! (§21.3, AK-047). Above the none rung, containment wraps from process
//! start: the FIRST process is the sandbox, never the bare probe (§17.5.4,
//! AK-048). A backend achieving the none rung validates no tier above
//! contained, and the irreversible tier has no policy at all — the sandbox
//! itself is the last line after the provider's structural refusal
//! (§17.3, AK-050).
//!
//! Cases build their fixtures naming the FACTORY'S OWN backend identity:
//! a conforming adapter refuses to validate a policy naming another
//! backend (cross-wiring defense), so a suite-fixed identity token would
//! make the suite unpassable by every real adapter.

use std::path::Path;

use gauntlet_domain::{BlastRadius, SandboxPolicyRef};

use crate::agent_runner::Isolation;
use crate::contract::ContractFinding;
use crate::deadline::Deadline;
use crate::sandbox::{NetworkPosture, Sandbox, SandboxBackendRef, SandboxPolicy};

/// The suite's name in findings.
pub const SUITE: &str = "sandbox";

/// The suite's bound span, on its own line (numeric literals never share a
/// line with a duration-shaped identifier).
const BOUND_SPAN: u64 = 60;

/// Constructs the sandbox under test. `build` must return an ISOLATED
/// instance: cases assume a clean slate.
pub trait SandboxFactory {
    fn label(&self) -> &'static str;

    fn build(&self) -> Result<Box<dyn Sandbox>, String>;

    /// Materializes one absolute directory the suite may name in a policy,
    /// and returns it. The suite decides the LAYOUT — which segments exist
    /// and how they nest — and the factory decides WHERE, and does the
    /// creating.
    ///
    /// The paths must be real. Spec §6.2 calls `validate` a dry run, and a
    /// backend that honours that cannot grant a writable root which does
    /// not exist — it is RIGHT to refuse one. A suite built on invented
    /// absolute paths could therefore be passed only by a backend that
    /// never exercises the policy it is judging, which is the very defect
    /// the suite exists to catch.
    ///
    /// The creating lives here, on the factory, and not in the suite body,
    /// because `gauntlet-ports` is a pure contract crate that writes
    /// nothing — the same division [`WorkspaceProviderFactory::root`] draws.
    /// Implementations return a process-scoped scratch directory.
    ///
    /// [`WorkspaceProviderFactory::root`]: crate::contract::workspace_provider::WorkspaceProviderFactory::root
    fn fixture_dir(&self, segment: &str) -> std::path::PathBuf;

    /// Runs one command this backend RENDERED — a [`SandboxedCommand`] is
    /// ONE shell-invocable string, which is what makes this well-defined
    /// for every backend — and returns what the payload printed on
    /// standard output.
    ///
    /// Process-space confinement is the one guarantee in this suite that
    /// CANNOT be read off the rendered string.
    /// [`a_nested_denial_survives_the_writable_bind`] and
    /// [`a_relocatable_nested_denial_is_refused`] can be stated over shell
    /// WORDS because the policy's own paths are words the suite supplied.
    /// A process namespace has no such word: the flag that unshares it is
    /// backend vocabulary the port must not know (spec §24.3), and its
    /// ABSENCE looks exactly like a backend that confines the process
    /// space by some other means. The only honest observation is
    /// behavioural — run the wrapped command and ask the payload what it
    /// could reach.
    ///
    /// The RUNNING lives here, on the factory, for the same reason the
    /// creating does: `gauntlet-ports` is a pure contract crate that
    /// spawns nothing.
    ///
    /// `Err` is for the factory's OWN failure to run the command at all.
    /// The payload's exit status is never an error: a payload that ran and
    /// printed is a successful observation whatever it exited with, and
    /// the case reads its verdict off the markers, not off the status.
    ///
    /// [`SandboxedCommand`]: crate::sandbox::SandboxedCommand
    fn run_rendered(&self, command: &str) -> Result<String, String>;
}

fn finding(case: &'static str, rule: &'static str, detail: String) -> ContractFinding {
    ContractFinding {
        suite: SUITE,
        case,
        rule,
        detail,
    }
}

/// A bound the wall clock has not passed yet. Real adapters read the real
/// clock to gate their IO (they own no injected `Clock`), so a fixed epoch
/// instant would be long-expired for them — the suite would report
/// `BackendUnavailable` for every live backend. The suite is test-support
/// surface (it runs only inside test binaries), which is why reading the
/// wall clock here is honest: it is the same fact the adapter under test
/// reads, not production timekeeping.
fn bound() -> Deadline {
    let now_secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .expect("the wall clock reads after the epoch")
        .as_secs();
    Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(now_secs + BOUND_SPAN))
}

/// A well-formed policy for a tier under the factory's own backend. The
/// writable/denied fixtures are ABSOLUTE paths: a process-rung backend
/// refuses relative roots, and the suite must not fail an honest adapter
/// on a fixture shape the adapter is right to refuse. They name SEGMENTS
/// only, which [`SandboxFactory::fixture_dir`] materializes under a root of
/// its own choosing — the suite therefore contains no string literal rooted
/// at `/` (the literals discipline, spec §8.1) and no invented absolute
/// path a dry-running backend would be right to reject.
const SUITE_WRITABLE_SEGMENT: &str = "suite-writable-root";
const SUITE_DENIED_SEGMENT: &str = "suite-denied-path";

/// The segment naming a denied path that lives INSIDE the writable root —
/// the only shape in which "denied even if otherwise writable" has
/// content. A denied path that is a SIBLING of every writable root is
/// already covered by whatever baseline keeps the rest of the filesystem
/// out; only the nested one tests the denial itself.
const SUITE_NESTED_DENIED_SEGMENT: &str = "suite-nested-denied-path";

/// The segment naming an INTERVENING directory between a writable root and
/// a denied path. It is an ordinary directory inside a root the policy
/// declares writable, so RENAMING it is a write the policy explicitly
/// permits — and a denial expressed as a path mask over its child moves
/// with it. This segment is what makes a nested denial RELOCATABLE.
const SUITE_RELOCATABLE_PARENT_SEGMENT: &str = "suite-relocatable-parent";

/// The markers the process-space probe prints. They are read off the
/// payload's STDOUT, never off the rendered command (which necessarily
/// contains both), so a backend cannot satisfy the case by echoing.
const SUITE_HOST_REACHED_MARKER: &str = "gauntlet-suite-host-process-reached";
const SUITE_HOST_REFUSED_MARKER: &str = "gauntlet-suite-host-process-refused";

/// One absolute fixture path, materialized by the factory (see
/// [`SandboxFactory::fixture_dir`] for why it must be real, and why the
/// creating belongs there rather than here).
fn suite_absolute(factory: &dyn SandboxFactory, segment: &str) -> String {
    factory.fixture_dir(segment).to_string_lossy().into_owned()
}

fn policy(
    factory: &dyn SandboxFactory,
    reference: &str,
    backend: &SandboxBackendRef,
) -> SandboxPolicy {
    policy_for_tier(factory, reference, BlastRadius::Contained, backend)
}

/// A well-formed policy naming an explicit tier and the factory's own
/// backend.
fn policy_for_tier(
    factory: &dyn SandboxFactory,
    reference: &str,
    tier: BlastRadius,
    backend: &SandboxBackendRef,
) -> SandboxPolicy {
    let mut policy = policy_with_paths(
        reference,
        backend,
        vec![suite_absolute(factory, SUITE_WRITABLE_SEGMENT)],
        vec![suite_absolute(factory, SUITE_DENIED_SEGMENT)],
    );
    policy.tier = tier;
    policy
}

/// A contained-tier policy under the factory's own backend carrying the
/// given path fixtures — the shape the interpolation case needs, where the
/// paths are the variable under test.
fn policy_with_paths(
    reference: &str,
    backend: &SandboxBackendRef,
    writable_roots: Vec<String>,
    denied_paths: Vec<String>,
) -> SandboxPolicy {
    SandboxPolicy {
        reference: SandboxPolicyRef::new(reference).expect("suite policy ref"),
        tier: BlastRadius::Contained,
        backend: backend.clone(),
        network: NetworkPosture::DenyByDefault,
        writable_roots,
        denied_paths,
    }
}

/// A flag-shaped token the suite hides INSIDE a policy path. No conforming
/// backend has a flag by this name, so if it ever surfaces as a shell word
/// of its own, the only way it got there is policy text being read as
/// command structure.
const SUITE_SMUGGLED_FLAG: &str = "--suite-smuggled-flag";

/// The tail that turns a fixture path into a legitimate path CONTAINING A
/// SPACE. Quoting is the fix for smuggling; refusing every path with a
/// space would trade a security defect for a correctness defect, so the
/// suite pins both halves at once.
const SUITE_SPACED_TAIL: &str = "suite-spaced-tail";

/// Splits a rendered command into POSIX shell WORDS — what the shell that
/// runs this string will actually pass as separate arguments. Single
/// quotes are literal throughout; inside double quotes a backslash escapes
/// the next byte; outside quotes a backslash escapes the next byte and
/// unquoted whitespace separates words. An unterminated quote or a
/// dangling final backslash is not a command at all, and is reported as
/// the error it is rather than silently split.
///
/// The suite needs word semantics, not substring semantics: `'/a b'`
/// contains the bytes of `/a` and of `b`, but is ONE argument, while
/// `/a b` is two. Every assertion about interpolated policy text is stated
/// over these words.
fn shell_words(command: &str) -> Result<Vec<String>, &'static str> {
    let mut words = Vec::new();
    let mut current = String::new();
    let mut started = false;
    let mut chars = command.chars();
    while let Some(ch) = chars.next() {
        match ch {
            ch if ch.is_whitespace() => {
                if started {
                    words.push(std::mem::take(&mut current));
                    started = false;
                }
            }
            '\'' => {
                started = true;
                loop {
                    match chars.next() {
                        Some('\'') => break,
                        Some(inner) => current.push(inner),
                        None => return Err("an unterminated single quote"),
                    }
                }
            }
            '"' => {
                started = true;
                loop {
                    match chars.next() {
                        Some('"') => break,
                        Some('\\') => match chars.next() {
                            Some(escaped) => current.push(escaped),
                            None => return Err("a dangling backslash inside a double quote"),
                        },
                        Some(inner) => current.push(inner),
                        None => return Err("an unterminated double quote"),
                    }
                }
            }
            '\\' => {
                started = true;
                match chars.next() {
                    Some(escaped) => current.push(escaped),
                    None => return Err("a dangling final backslash"),
                }
            }
            other => {
                started = true;
                current.push(other);
            }
        }
    }
    if started {
        words.push(current);
    }
    Ok(words)
}

/// Whether a wrapped command's first word names the backend: either the
/// command begins with the identity token itself, or with a path whose
/// file name IS the identity token (an injected absolute binary path).
/// This is the AK-048 prefix check — containment from process start means
/// the first process is the backend's own invocation.
fn first_word_names_backend(command: &str, identity: &str) -> bool {
    let Some(first) = command.split_whitespace().next() else {
        return false;
    };
    first == identity
        || Path::new(first)
            .file_name()
            .is_some_and(|name| name == identity)
}

/// Case: `validate` then `wrap` — the clean validation authorizes the wrap;
/// the wrapped command carries the same policy reference, the same
/// validation, and the requested invocation text (spec §6.2 `wrap(command,
/// policy)` with the validation threaded through). The invocation must be
/// CARRIED: an identity wrap (none rung) returns the exact text, and a
/// transforming wrap (a rung above none, AK-048) must still carry the
/// invocation inside the rendered command — the suite probe contains no
/// quote characters, so no escaping may have altered it.
fn validation_precedes_every_use(factory: &dyn SandboxFactory) -> Vec<ContractFinding> {
    const CASE: &str = "validation_precedes_every_use";
    const RULE: &str = "spec 6.2 Sandbox: validate(policy) precedes wrap(command, policy); the wrapped command carries its validation";
    let Ok(sandbox) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a sandbox for the case".to_owned(),
        )];
    };
    let backend = sandbox.backend_identity();
    let policy = policy(factory, "policy-contained-01", &backend);
    let validation = match sandbox.validate(&policy, bound()) {
        Ok(validation) => validation,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("validating a well-formed policy failed: {error}"),
            )];
        }
    };
    let mut findings = Vec::new();
    if !validation.is_clean() {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "the validation of a well-formed policy carries {} finding(s)",
                validation.findings.len()
            ),
        ));
    }
    if validation.policy != policy.reference || validation.backend != policy.backend {
        findings.push(finding(
            CASE,
            RULE,
            "the validation does not name the policy and backend it validated".to_owned(),
        ));
    }
    let invocation = "run the probe inside the workspace";
    match sandbox.wrap(invocation, &policy, &validation, bound()) {
        Ok(wrapped) => {
            let carried = if validation.isolation == Isolation::None {
                wrapped.command == invocation
            } else {
                wrapped.command.contains(invocation)
            };
            if !carried {
                findings.push(finding(
                    CASE,
                    RULE,
                    format!(
                        "the wrapped invocation does not carry the one requested: {:?}",
                        wrapped.command
                    ),
                ));
            }
            if wrapped.policy != policy.reference {
                findings.push(finding(
                    CASE,
                    RULE,
                    "the wrapped command names a different policy than the one that was validated"
                        .to_owned(),
                ));
            }
            if wrapped.validation != validation {
                findings.push(finding(
                    CASE,
                    RULE,
                    "the wrapped command does not carry the validation that authorized the wrap"
                        .to_owned(),
                ));
            }
        }
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!("wrapping under a validated policy failed: {error}"),
        )),
    }
    findings
}

/// Case: a wrap that hands over a validation earned by a DIFFERENT policy
/// is a typed refusal — "validation precedes every use" is structural, so a
/// stale or cross-wired validation cannot authorize a wrap (§17.3's
/// posture; the trait contract).
fn wrap_demands_its_own_validation(factory: &dyn SandboxFactory) -> Vec<ContractFinding> {
    const CASE: &str = "wrap_demands_its_own_validation";
    const RULE: &str = "port contract + spec 17.3 posture: wrap requires the validation its own policy earned; a foreign validation is a typed refusal";
    let Ok(sandbox) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a sandbox for the case".to_owned(),
        )];
    };
    let backend = sandbox.backend_identity();
    let one = policy(factory, "policy-contained-01", &backend);
    let two = policy(factory, "policy-contained-02", &backend);
    let validation = match sandbox.validate(&one, bound()) {
        Ok(validation) => validation,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("validating the first policy failed: {error}"),
            )];
        }
    };
    match sandbox.wrap(
        "run the probe inside the workspace",
        &two,
        &validation,
        bound(),
    ) {
        Err(_) => Vec::new(),
        Ok(wrapped) => vec![finding(
            CASE,
            RULE,
            format!(
                "a wrap under policy {} was authorized by a validation for {} — the wrap must \
                 be refused",
                wrapped.policy, validation.policy
            ),
        )],
    }
}

/// Case: a failed validation authorizes NOTHING — a hand-forged
/// findings-carrying `PolicyValidation` for an otherwise valid policy must
/// be refused at `wrap`, and so must a validation claiming a rung BELOW the
/// one the backend achieved for that policy (a downgrade). The rule is that
/// no path exists from a failed validation to an executed command (spec
/// §21.3 "Sandbox validation failure → Escalate; never fall back to
/// unsandboxed"; AK-047). The findings are forged on purpose: an adapter
/// must not launder a failure-shaped validation into authority.
///
/// The downgrade leg only exists above the none rung (nothing is below
/// none); for a none-rung backend that leg is vacuous here, and the
/// above-contained protection it gestures at is pinned by
/// [`capability_mismatch`]'s tier-posture legs.
fn no_unsandboxed_fallback(factory: &dyn SandboxFactory) -> Vec<ContractFinding> {
    const CASE: &str = "no_unsandboxed_fallback";
    const RULE: &str = "spec 21.3 + AK-047: a findings-carrying or rung-downgraded validation authorizes no wrap — there is no path from a failed validation to an executed command";
    let Ok(sandbox) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a sandbox for the case".to_owned(),
        )];
    };
    let backend = sandbox.backend_identity();
    let policy = policy(factory, "policy-contained-01", &backend);
    let validation = match sandbox.validate(&policy, bound()) {
        Ok(validation) => validation,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("validating a well-formed contained policy failed: {error}"),
            )];
        }
    };
    if !validation.is_clean() {
        return vec![finding(
            CASE,
            RULE,
            format!(
                "the validation of a well-formed contained policy carries {} finding(s)",
                validation.findings.len()
            ),
        )];
    }
    let mut findings = Vec::new();
    let mut dirty = validation.clone();
    dirty
        .findings
        .push("suite-forged finding: this failure-shaped validation authorizes nothing".to_owned());
    if sandbox
        .wrap(
            "run the probe inside the workspace",
            &policy,
            &dirty,
            bound(),
        )
        .is_ok()
    {
        findings.push(finding(
            CASE,
            RULE,
            "wrap accepted a findings-carrying validation — a failed validation must escalate, \
             never authorize execution"
                .to_owned(),
        ));
    }
    if validation.isolation > Isolation::None {
        let mut downgraded = validation.clone();
        downgraded.isolation = Isolation::None;
        if sandbox
            .wrap(
                "run the probe inside the workspace",
                &policy,
                &downgraded,
                bound(),
            )
            .is_ok()
        {
            findings.push(finding(
                CASE,
                RULE,
                format!(
                    "wrap accepted a validation downgraded to the none rung under a backend that \
                     achieved {} — a validation below the achieved rung authorizes nothing",
                    validation.isolation.token()
                ),
            ));
        }
    }
    findings
}

/// Case: containment wraps from process start (spec §17.5.4; AK-048) —
/// above the none rung the FIRST process is the sandbox, so the wrapped
/// command is a strictly longer string that begins with the backend's own
/// invocation and carries the probe inside it; the bare probe never runs
/// standalone. At the none rung an identity wrap IS legal, but only
/// because the backend validates no above-contained tier clean there
/// (§17.3): that tier posture is asserted in this case's none branch.
fn wraps_from_process_start(factory: &dyn SandboxFactory) -> Vec<ContractFinding> {
    const CASE: &str = "wraps_from_process_start";
    const RULE: &str = "spec 17.5.4 + AK-048: containment wraps the session from process start — above the none rung the first process is the sandbox; at the none rung an identity wrap is legal only at contained";
    let Ok(sandbox) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a sandbox for the case".to_owned(),
        )];
    };
    let backend = sandbox.backend_identity();
    let policy = policy(factory, "policy-contained-01", &backend);
    let validation = match sandbox.validate(&policy, bound()) {
        Ok(validation) => validation,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("validating a well-formed contained policy failed: {error}"),
            )];
        }
    };
    if !validation.is_clean() {
        return vec![finding(
            CASE,
            RULE,
            format!(
                "the validation of a well-formed contained policy carries {} finding(s)",
                validation.findings.len()
            ),
        )];
    }
    let mut findings = Vec::new();
    let probe = "run the probe inside the workspace from process start";
    if validation.isolation > Isolation::None {
        match sandbox.wrap(probe, &policy, &validation, bound()) {
            Err(error) => findings.push(finding(
                CASE,
                RULE,
                format!("wrapping under a clean validation failed: {error}"),
            )),
            Ok(wrapped) => {
                if wrapped.command == probe {
                    findings.push(finding(
                        CASE,
                        RULE,
                        "the wrapped command IS the bare probe — the probe would execute \
                         unsandboxed; the first process must be the sandbox"
                            .to_owned(),
                    ));
                }
                if !first_word_names_backend(&wrapped.command, backend.as_str()) {
                    findings.push(finding(
                        CASE,
                        RULE,
                        format!(
                            "the wrapped command does not begin with the backend invocation \
                             (identity {:?}): {:?}",
                            backend.as_str(),
                            wrapped.command
                        ),
                    ));
                }
                if !wrapped.command.contains(probe) {
                    findings.push(finding(
                        CASE,
                        RULE,
                        "the probe does not survive inside the wrapped command".to_owned(),
                    ));
                }
                if wrapped.command.len() <= probe.len() {
                    findings.push(finding(
                        CASE,
                        RULE,
                        "the wrapped command is not strictly longer than the probe — the \
                         containment invocation is missing"
                            .to_owned(),
                    ));
                }
            }
        }
    } else {
        // The none-rung tier posture (AK-050's backend half): an identity
        // wrap is legal only because no above-contained policy can earn a
        // clean validation at this backend.
        for tier in [BlastRadius::Local, BlastRadius::CrossCutting] {
            let above = policy_for_tier(factory, "policy-above-contained", tier, &backend);
            if let Ok(above_validation) = sandbox.validate(&above, bound())
                && above_validation.is_clean()
            {
                findings.push(finding(
                    CASE,
                    RULE,
                    format!(
                        "a backend whose contained validation achieves the none rung \
                         clean-validated a {tier} policy — an identity wrap is legal only \
                         at contained (spec 17.3)",
                    ),
                ));
            }
        }
    }
    findings
}

/// Case: the backend's achieved rung must cover the tier (spec §17.3
/// "Runner must report `isolation ≥ process`" for above-contained tiers;
/// §8.5 check 4 `sandbox_capability_mismatch`; AK-050) — a backend whose
/// clean contained validation achieves `Isolation::None` must NOT
/// clean-validate the local or cross_cutting tiers. And for EVERY backend,
/// an `irreversible`-tier policy is refused at `validate` outright: "No
/// policy exists" (§17.3) — the sandbox itself is the last line after the
/// provider's structural refusal.
fn capability_mismatch(factory: &dyn SandboxFactory) -> Vec<ContractFinding> {
    const CASE: &str = "capability_mismatch";
    const RULE: &str = "spec 17.3/8.5 check 4 + AK-050: a backend achieving the none rung validates no tier above contained clean, and the irreversible tier is refused at validate by every backend";
    let Ok(sandbox) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a sandbox for the case".to_owned(),
        )];
    };
    let backend = sandbox.backend_identity();
    let contained = policy_for_tier(
        factory,
        "policy-contained-01",
        BlastRadius::Contained,
        &backend,
    );
    let achieved = match sandbox.validate(&contained, bound()) {
        Ok(validation) if validation.is_clean() => validation.isolation,
        Ok(validation) => {
            return vec![finding(
                CASE,
                RULE,
                format!(
                    "the validation of a well-formed contained policy carries {} finding(s)",
                    validation.findings.len()
                ),
            )];
        }
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("validating a well-formed contained policy failed: {error}"),
            )];
        }
    };
    let mut findings = Vec::new();
    if achieved == Isolation::None {
        for tier in [BlastRadius::Local, BlastRadius::CrossCutting] {
            let above = policy_for_tier(factory, "policy-above-contained", tier, &backend);
            if let Ok(above_validation) = sandbox.validate(&above, bound())
                && above_validation.is_clean()
            {
                findings.push(finding(
                    CASE,
                    RULE,
                    format!(
                        "a none-rung backend clean-validated the {tier} tier — a backend \
                         reporting isolation none is refused for any tier above contained"
                    ),
                ));
            }
        }
    }
    let irreversible = policy_for_tier(
        factory,
        "policy-irreversible",
        BlastRadius::Irreversible,
        &backend,
    );
    if let Ok(validation) = sandbox.validate(&irreversible, bound()) {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "an irreversible-tier policy was not refused at validate (returned Ok with {} \
                 finding(s)) — no policy exists for the top tier (spec 17.3)",
                validation.findings.len()
            ),
        ));
    }
    findings
}

/// Case: policy text is DATA, never command structure (spec §6.2's
/// `wrap(command, policy)`; §17.5.4/AK-047's "no path from a policy to an
/// unsandboxed command"). A backend that interpolates a policy's writable
/// roots or denied paths into a rendered invocation must render each of
/// them as exactly ONE shell word:
///
/// - (a) a path carrying a flag-shaped token after a space must not put
///   that token in the command as a word of its own. Otherwise a policy
///   author smuggles backend flags past a validation that still reports
///   clean — and a backend's own flags are how its containment is
///   configured, so the smuggled flag can undo the containment (a later
///   flag overriding an earlier one) while the validation, the isolation
///   rung, and the policy reference all still look honest.
/// - (b) a legitimate absolute path that CONTAINS a space must still
///   validate, still wrap, and still arrive as one word. Refusing spaces
///   would close (a) by trading a security defect for a correctness
///   defect, so the suite forbids that escape route explicitly.
///
/// A backend is free to refuse the hostile policy outright — a refusal
/// executes nothing — but (b) leaves no room for a blanket space refusal.
/// A backend that interpolates nothing (the identity wrap at the none
/// rung) satisfies (a) and the word half of (b) vacuously, and is still
/// held to (b)'s "must not refuse" half.
fn policy_paths_are_not_shell_words(factory: &dyn SandboxFactory) -> Vec<ContractFinding> {
    const CASE: &str = "policy_paths_are_not_shell_words";
    const RULE: &str = "spec 6.2 + 17.5.4/AK-047: an interpolated policy path is exactly one shell word — a policy cannot smuggle a backend flag, and a legitimate path containing a space still validates, wraps, and binds";
    let Ok(sandbox) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a sandbox for the case".to_owned(),
        )];
    };
    let backend = sandbox.backend_identity();
    let probe = "run the probe inside the workspace";
    let mut findings = Vec::new();

    // (a) The smuggling attempt. Refusal at validate or at wrap is a
    // legitimate answer here; what must never happen is a rendered command
    // in which the smuggled token stands alone as a word.
    let hostile = policy_with_paths(
        "policy-contained-smuggle",
        &backend,
        vec![suite_absolute(
            factory,
            &format!("{SUITE_WRITABLE_SEGMENT} {SUITE_SMUGGLED_FLAG}"),
        )],
        vec![suite_absolute(
            factory,
            &format!("{SUITE_DENIED_SEGMENT} {SUITE_SMUGGLED_FLAG}"),
        )],
    );
    if let Ok(validation) = sandbox.validate(&hostile, bound())
        && validation.is_clean()
        && let Ok(wrapped) = sandbox.wrap(probe, &hostile, &validation, bound())
    {
        match shell_words(&wrapped.command) {
            Err(reason) => findings.push(finding(
                CASE,
                RULE,
                format!(
                    "the wrapped command is not a parsable shell command ({reason}): {:?}",
                    wrapped.command
                ),
            )),
            Ok(words) => {
                if words.iter().any(|word| word == SUITE_SMUGGLED_FLAG) {
                    findings.push(finding(
                        CASE,
                        RULE,
                        format!(
                            "a policy path carrying {SUITE_SMUGGLED_FLAG:?} after a space \
                             rendered that token as a shell word of its own — policy text \
                             became command structure under a validation that reported clean: \
                             {:?}",
                            wrapped.command
                        ),
                    ));
                }
            }
        }
    }

    // (b) The legitimate path containing a space.
    let spaced_writable = suite_absolute(
        factory,
        &format!("{SUITE_WRITABLE_SEGMENT} {SUITE_SPACED_TAIL}"),
    );
    let spaced_denied = suite_absolute(
        factory,
        &format!("{SUITE_DENIED_SEGMENT} {SUITE_SPACED_TAIL}"),
    );
    let spaced = policy_with_paths(
        "policy-contained-spaced",
        &backend,
        vec![spaced_writable.clone()],
        vec![spaced_denied.clone()],
    );
    let validation = match sandbox.validate(&spaced, bound()) {
        Err(error) => {
            findings.push(finding(
                CASE,
                RULE,
                format!(
                    "a well-formed policy whose absolute paths contain a space was refused at \
                     validate ({error}) — quoting is the fix for smuggling; refusing every path \
                     with a space trades a security defect for a correctness defect"
                ),
            ));
            return findings;
        }
        Ok(validation) if !validation.is_clean() => {
            findings.push(finding(
                CASE,
                RULE,
                format!(
                    "a well-formed policy whose absolute paths contain a space earned {} \
                     validation finding(s) — a space is not an offense, it is a path",
                    validation.findings.len()
                ),
            ));
            return findings;
        }
        Ok(validation) => validation,
    };
    match sandbox.wrap(probe, &spaced, &validation, bound()) {
        Err(error) => findings.push(finding(
            CASE,
            RULE,
            format!(
                "a clean validation of a policy whose absolute paths contain a space did not \
                 wrap ({error}) — a path with a space is legitimate and must keep working"
            ),
        )),
        Ok(wrapped) => match shell_words(&wrapped.command) {
            Err(reason) => findings.push(finding(
                CASE,
                RULE,
                format!(
                    "the wrapped command is not a parsable shell command ({reason}): {:?}",
                    wrapped.command
                ),
            )),
            Ok(words) => {
                for path in [&spaced_writable, &spaced_denied] {
                    if let Some(fragment) = path
                        .split_whitespace()
                        .find(|fragment| words.iter().any(|word| word == fragment))
                    {
                        findings.push(finding(
                            CASE,
                            RULE,
                            format!(
                                "the path {path:?} was split across shell words ({fragment:?} \
                                 stands alone) — a legitimate path containing a space must \
                                 arrive as ONE word: {:?}",
                                wrapped.command
                            ),
                        ));
                    }
                }
            }
        },
    }
    findings
}

/// Case (spec §6.2, §17.3; AK-047): a denied path NESTED inside a writable
/// root stays denied. A backend that grants a writable root and masks a
/// path underneath it must order the two so the mask WINS — mount
/// operations are applied in the order they are given, so a bind of the
/// enclosing root issued after the mask re-exposes exactly the subtree the
/// policy denied, under a validation that still reports clean.
///
/// The suite states this over the rendered command's shell WORDS: if the
/// wrapped command carries both the writable root and the nested denied
/// path as words of its own, the LAST occurrence of the denied path must
/// come after the LAST occurrence of the writable root. A backend that
/// interpolates neither (the identity wrap at the none rung) satisfies the
/// order half vacuously — but not the rest: a nested denial is a
/// LEGITIMATE policy, so refusing it at validate or at wrap is a defect,
/// and that half binds every backend.
fn a_nested_denial_survives_the_writable_bind(
    factory: &dyn SandboxFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "a_nested_denial_survives_the_writable_bind";
    const RULE: &str = "spec 6.2 + 17.3/AK-047: a denied path nested inside a writable root is denied even if otherwise writable — the mask must outrank the bind that encloses it";
    let Ok(sandbox) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a sandbox for the case".to_owned(),
        )];
    };
    let backend = sandbox.backend_identity();
    let probe = "run the probe inside the workspace";
    let mut findings = Vec::new();

    let writable = suite_absolute(factory, SUITE_WRITABLE_SEGMENT);
    let nested = Path::new(&writable)
        .join(SUITE_NESTED_DENIED_SEGMENT)
        .to_string_lossy()
        .into_owned();
    let policy = policy_with_paths(
        "policy-contained-nested-denial",
        &backend,
        vec![writable.clone()],
        vec![nested.clone()],
    );

    let validation = match sandbox.validate(&policy, bound()) {
        Err(error) => {
            findings.push(finding(
                CASE,
                RULE,
                format!(
                    "a well-formed policy denying {nested:?} inside writable root \
                     {writable:?} was refused at validate ({error}) — a nested denial is \
                     the ordinary shape of 'denied even if otherwise writable', not an \
                     offense"
                ),
            ));
            return findings;
        }
        Ok(validation) if !validation.is_clean() => {
            findings.push(finding(
                CASE,
                RULE,
                format!(
                    "a well-formed policy denying {nested:?} inside writable root \
                     {writable:?} earned {} validation finding(s) — a nested denial is a \
                     policy a backend must serve, not refuse",
                    validation.findings.len()
                ),
            ));
            return findings;
        }
        Ok(validation) => validation,
    };

    let wrapped = match sandbox.wrap(probe, &policy, &validation, bound()) {
        Err(error) => {
            findings.push(finding(
                CASE,
                RULE,
                format!(
                    "a clean validation of a policy with a nested denial did not wrap \
                     ({error}) — a nested denial must keep working"
                ),
            ));
            return findings;
        }
        Ok(wrapped) => wrapped,
    };

    let words = match shell_words(&wrapped.command) {
        Err(reason) => {
            findings.push(finding(
                CASE,
                RULE,
                format!(
                    "the wrapped command is not a parsable shell command ({reason}): {:?}",
                    wrapped.command
                ),
            ));
            return findings;
        }
        Ok(words) => words,
    };

    let last_of = |needle: &str| {
        words
            .iter()
            .enumerate()
            .filter(|(_, word)| word.as_str() == needle)
            .map(|(at, _)| at)
            .next_back()
    };
    if let (Some(root_at), Some(nested_at)) = (last_of(&writable), last_of(&nested))
        && nested_at < root_at
    {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "the denied path {nested:?} is named at word {nested_at}, BEFORE the \
                 enclosing writable root {writable:?} at word {root_at} — mount operations \
                 apply in argument order, so the later bind of the enclosing root \
                 re-exposes the denied subtree under a validation that reported clean: \
                 {:?}",
                wrapped.command
            ),
        ));
    }
    findings
}

/// Case (spec §6.2, §17.3; AK-047): "denied even if otherwise writable" is
/// a guarantee about CONTENT, and a mask expressed as a PATH cannot deliver
/// it once the contained process can move the content out from under the
/// mask.
///
/// The shape: writable root `R`, denied path `R/intervening/denied`. The
/// intervening directory is an ordinary directory inside `R`, so renaming
/// it is a write the policy EXPLICITLY PERMITS. Session one performs that
/// permitted write. Session two re-validates the IDENTICAL policy: the
/// mask target no longer exists, but it sits inside a writable root, so a
/// backend that creates missing mount points there masks the VACATED path,
/// starts the sandbox, and hands the contained process the payload at its
/// new location — readable and writable, under a validation that reported
/// clean. The failure is OPEN, and it is exactly the case in which the
/// guarantee has content.
///
/// So the contract is stated on the DECISION, not on a mechanism: a
/// backend must not validate such a policy clean and then serve it with a
/// bare path mask. It may hold the denial by some anchor of its own, or it
/// may refuse the policy — a refusal executes nothing and is the honest
/// report of a guarantee this backend cannot deliver. What it may not do is
/// report clean and degrade silently.
///
/// The suite reads that decision off the rendered command's shell WORDS: a
/// backend that validated clean, wrapped, and named BOTH the writable root
/// and the relocatable denied path as words of its own is masking by path
/// and nothing else. A backend that interpolates neither (the identity wrap
/// at the none rung, which contains nothing and whose tier posture is
/// pinned by [`capability_mismatch`]) is vacuously outside this case.
///
/// This case does NOT relax [`a_nested_denial_survives_the_writable_bind`]:
/// a denied path DIRECTLY under a writable root remains a legitimate policy
/// every backend must serve. Only the relocatable shape — one with a
/// renameable directory between the root and the denial — is in scope here.
fn a_relocatable_nested_denial_is_refused(factory: &dyn SandboxFactory) -> Vec<ContractFinding> {
    const CASE: &str = "a_relocatable_nested_denial_is_refused";
    const RULE: &str = "spec 6.2 + 17.3/AK-047: a denied path reachable through a directory the contained process is permitted to rename cannot be held by a path mask — the backend refuses the policy rather than validating it clean and degrading silently";
    let Ok(sandbox) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a sandbox for the case".to_owned(),
        )];
    };
    let backend = sandbox.backend_identity();
    let probe = "run the probe inside the workspace";

    let writable = suite_absolute(factory, SUITE_WRITABLE_SEGMENT);
    let intervening = Path::new(&writable).join(SUITE_RELOCATABLE_PARENT_SEGMENT);
    let relocatable = intervening
        .join(SUITE_NESTED_DENIED_SEGMENT)
        .to_string_lossy()
        .into_owned();
    let policy = policy_with_paths(
        "policy-contained-relocatable-denial",
        &backend,
        vec![writable.clone()],
        vec![relocatable.clone()],
    );

    // An honest refusal — at validate, as findings, or at wrap — delivers
    // the guarantee by declining to pretend. Each is a clean pass here.
    let validation = match sandbox.validate(&policy, bound()) {
        Err(_) => return Vec::new(),
        Ok(validation) if !validation.is_clean() => return Vec::new(),
        Ok(validation) => validation,
    };
    let Ok(wrapped) = sandbox.wrap(probe, &policy, &validation, bound()) else {
        return Vec::new();
    };

    let words = match shell_words(&wrapped.command) {
        Err(reason) => {
            return vec![finding(
                CASE,
                RULE,
                format!(
                    "the wrapped command is not a parsable shell command ({reason}): {:?}",
                    wrapped.command
                ),
            )];
        }
        Ok(words) => words,
    };
    let names = |needle: &str| words.iter().any(|word| word.as_str() == needle);
    if names(&writable) && names(&relocatable) {
        return vec![finding(
            CASE,
            RULE,
            format!(
                "the policy denying {relocatable:?} inside writable root {writable:?} \
                 validated CLEAN and wrapped into a command naming the root as a writable \
                 bind and the denied path as a bare path mask. The contained process may \
                 rename {:?} — a write inside a declared writable root, which the policy \
                 permits — and the denied content survives at its new location; the next \
                 wrap of the IDENTICAL policy then masks only the vacated path, the \
                 sandbox STARTS, and the denial is defeated: {:?}",
                intervening.display(),
                wrapped.command
            ),
        )];
    }
    Vec::new()
}

/// Case (spec §17.3, §17.5.1; AK-050): a backend whose clean validation
/// claims `Isolation::Process` or above confines the PROCESS space, not
/// only the filesystem and the network.
///
/// §17.3 requires an above-contained runner to report `isolation ≥
/// process` and names its failure mode as "host-mutating fallbacks
/// explicitly refused rather than left at the backend's default consent";
/// §17.5.1 confines a unit to "their assigned workspace path". A backend
/// that unshares the network and binds the filesystem read-only but
/// leaves the payload in the HOST process space has not delivered a
/// process rung: the contained process can still enumerate host
/// processes, read what they were invoked with, and signal them —
/// `SIGTERM` kills, `SIGSTOP` suspends. Neither the read-only root nor
/// the network namespace stands in the way, because neither is the
/// process space. Such a backend must either confine the process space or
/// stop claiming the rung; this case pins the first, and
/// [`capability_mismatch`] pins what the rung then authorizes.
///
/// The observation is BEHAVIOURAL — see [`SandboxFactory::run_rendered`]
/// for why no reading of the rendered string can stand in for it. The
/// "process outside the sandbox" is the SUITE'S OWN process, so the case
/// needs no victim of its own and can never touch a process it did not
/// create. The probe is `kill -0`, which delivers no signal at all: it
/// performs exactly the permission-and-existence check every real signal
/// performs and then stops, so a successful probe proves `SIGTERM` would
/// have landed without ever sending one.
///
/// A backend below the process rung is vacuously outside this case — the
/// none rung promises no confinement, and that it may not then serve an
/// above-contained tier is [`capability_mismatch`]'s clause.
fn the_process_space_is_confined(factory: &dyn SandboxFactory) -> Vec<ContractFinding> {
    const CASE: &str = "the_process_space_is_confined";
    const RULE: &str = "spec 17.3 + 17.5.1/AK-050: a backend claiming isolation >= process confines the PROCESS space — a contained payload cannot reach a process outside the sandbox";
    let Ok(sandbox) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a sandbox for the case".to_owned(),
        )];
    };
    let backend = sandbox.backend_identity();
    let policy = policy(factory, "policy-contained-process-space", &backend);
    let validation = match sandbox.validate(&policy, bound()) {
        Ok(validation) => validation,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("validating a well-formed contained policy failed: {error}"),
            )];
        }
    };
    if !validation.is_clean() {
        return vec![finding(
            CASE,
            RULE,
            format!(
                "the validation of a well-formed contained policy carries {} finding(s)",
                validation.findings.len()
            ),
        )];
    }
    if validation.isolation < Isolation::Process {
        // Below the process rung nothing is claimed, so nothing is owed.
        return Vec::new();
    }

    // The suite's own process is the host process the payload tries to
    // reach. `kill -0` sends nothing.
    let host = std::process::id();
    let probe = format!(
        "kill -0 {host} && echo {SUITE_HOST_REACHED_MARKER} || echo {SUITE_HOST_REFUSED_MARKER}"
    );
    let wrapped = match sandbox.wrap(&probe, &policy, &validation, bound()) {
        Ok(wrapped) => wrapped,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("wrapping under a clean validation failed: {error}"),
            )];
        }
    };
    let stdout = match factory.run_rendered(&wrapped.command) {
        Ok(stdout) => stdout,
        Err(reason) => {
            return vec![finding(
                CASE,
                RULE,
                format!(
                    "the wrapped command could not be run ({reason}) — a SandboxedCommand is \
                     one shell-invocable string: {:?}",
                    wrapped.command
                ),
            )];
        }
    };
    if stdout.contains(SUITE_HOST_REACHED_MARKER) {
        return vec![finding(
            CASE,
            RULE,
            format!(
                "the payload of a wrap whose validation claims the {} rung reached process \
                 {host} — a process OUTSIDE the sandbox, and the very process that built it. \
                 The permission check every signal performs succeeded, so SIGTERM would have \
                 killed it and SIGSTOP suspended it. A backend that confines the filesystem \
                 and the network but leaves the payload in the host process space is not \
                 delivering a process rung (spec 17.3): {:?}",
                validation.isolation.token(),
                wrapped.command
            ),
        )];
    }
    if !stdout.contains(SUITE_HOST_REFUSED_MARKER) {
        return vec![finding(
            CASE,
            RULE,
            format!(
                "the wrapped command ran but printed neither probe marker (stdout {stdout:?}) — \
                 the payload of a process-rung wrap must actually execute inside the sandbox \
                 (spec 17.5.4), otherwise the confinement it reports is unobserved: {:?}",
                wrapped.command
            ),
        )];
    }
    Vec::new()
}

/// Runs the full `Sandbox` contract suite against one factory.
pub fn sandbox_suite(factory: &dyn SandboxFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(validation_precedes_every_use(factory));
    findings.extend(wrap_demands_its_own_validation(factory));
    findings.extend(no_unsandboxed_fallback(factory));
    findings.extend(wraps_from_process_start(factory));
    findings.extend(capability_mismatch(factory));
    findings.extend(policy_paths_are_not_shell_words(factory));
    findings.extend(a_nested_denial_survives_the_writable_bind(factory));
    findings.extend(a_relocatable_nested_denial_is_refused(factory));
    findings.extend(the_process_space_is_confined(factory));
    findings
}

#[cfg(test)]
mod tests {
    use super::shell_words;

    #[test]
    fn shell_words_reads_quoting_the_way_a_shell_does() {
        assert_eq!(
            shell_words("a b  c"),
            Ok(vec!["a".to_owned(), "b".to_owned(), "c".to_owned()])
        );
        assert_eq!(
            shell_words("--bind 'one word' tail"),
            Ok(vec![
                "--bind".to_owned(),
                "one word".to_owned(),
                "tail".to_owned()
            ])
        );
        assert_eq!(
            shell_words("'it'\\''s' one"),
            Ok(vec!["it's".to_owned(), "one".to_owned()])
        );
        assert_eq!(
            shell_words("\"two words\" x"),
            Ok(vec!["two words".to_owned(), "x".to_owned()])
        );
        assert_eq!(shell_words("one\\ word"), Ok(vec!["one word".to_owned()]));
        assert!(shell_words("'unterminated").is_err());
        assert!(shell_words("trailing\\").is_err());
    }
}
