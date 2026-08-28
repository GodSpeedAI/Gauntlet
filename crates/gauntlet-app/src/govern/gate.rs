//! The §17.1 authority gate and its containment conjuncts (plan Task 14;
//! AK-044, AK-045, AK-047, AK-048, AK-050's runner half; defect D2's
//! correction: a check that runs after the builder's turn is an audit, not
//! a gate).
//!
//! The gate runs at DISPATCH time, before the runner is invoked — after the
//! scheduler's own §11.2 eligibility (which already refuses irreversible
//! units, conjunct 3) and before [`crate::control`] builds the
//! `RoleInvocationSpec`. Its refusal is a GOVERNANCE fact: the controller
//! records it durably and escalates (§21.3: "Sandbox validation failure |
//! Escalate; never fall back to unsandboxed"), it never silently skips the
//! unit and never runs it anyway.
//!
//! The conjuncts, in evaluation order (the order §17.1 itself fixes):
//!
//! 1. `irreversible` never dispatches — "scheduler-level, unconditional,
//!    independent of what any prompt says". The scheduler is the first
//!    line; this is the dispatch-time second line, so a forged or stale
//!    plan input cannot smuggle the tier through (AK-044).
//! 2. The unit's authority must be within the run's exercisable ceiling —
//!    "escalate before any transform" (§17.1 line 2).
//! 3. The dispatch's unit facts may not rank BELOW the recorded answer-key
//!    facts — the runtime never honors a lattice decrease, and an
//!    agent-originated movement is refused in every direction (AK-046; see
//!    [`super::posture`]). In the composed happy path plan facts and
//!    recorded facts coincide by construction; the conjunct guards drift
//!    and forgery between plan assembly and dispatch.
//! 4. The runner's reported isolation must meet the tier's required rung
//!    (§17.3 "Runner must report `isolation ≥ process`" for
//!    above-contained tiers; §22.2's honesty rule is what makes the mock's
//!    `none` refusable) — AK-050.
//! 5. Above `contained`, a backend posture must exist: under the §8.6
//!    conservative `none` backend, "any higher tier without a backend
//!    refuses to dispatch".
//! 6. Above `contained`, the tier's policy resolves and validates CLEAN;
//!    a validation failure is an ESCALATION, never an unsandboxed
//!    fallback (AK-047). A clean validation is carried on the admission so
//!    the wrap that follows (and the invocation record) can prove
//!    validation preceded use (AK-048).
//!
//! Admission is CONJUNCTIVE, never scored: a dispatch maximal on every
//! other merit dimension is still refused when exactly one hard conjunct
//! fails (AK-045 — a high-quality result is rejected, not averaged into a
//! pass).

use std::sync::Arc;

use gauntlet_domain::{Authority, BlastRadius, SandboxPolicyRef, UnitId, WorkspaceRef};
use gauntlet_ports::agent_runner::Isolation;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::policy_provider::{PolicyProvider, PolicyProviderError};
use gauntlet_ports::sandbox::{
    PolicyValidation, Sandbox, SandboxError, SandboxPolicy, SandboxedCommand,
};

use super::posture::{self, ChangeOrigin, LatticeMovement};

/// The governance posture the controller is composed with. Construction is
/// the ONLY way to an inconsistent posture: a backend token other than the
/// §8.6 conservative default REQUIRES both a sandbox and a policy provider,
/// and the conservative token FORBIDS both — an optional gate would be an
/// unsandboxed fallback wearing a type-system costume (task-14 prereg R1).
#[derive(Clone)]
pub struct GovernDeps {
    /// The resolved `sandbox.backend` token. The §8.6 default token names
    /// the conservative posture (no backend); any other token names a real
    /// backend and demands the instances below.
    backend_token: String,
    sandbox: Option<Arc<dyn Sandbox>>,
    policies: Option<Arc<dyn PolicyProvider>>,
    /// The highest authority an unattended run may exercise (§17.1's
    /// critic ceiling — "authority ≠ critic and closure needed" escalates).
    /// Carried as a fact, not config: raising it is an operator composition
    /// decision this task does not expose.
    exercisable_authority: Authority,
    /// The workspace the policy resolution anchors to.
    workspace: WorkspaceRef,
}

/// Why a [`GovernDeps`] could not be built — composition-time failures,
/// typed (spec §21.2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GovernError {
    /// A real backend was configured without the sandbox instance that
    /// fronts it.
    BackendWithoutSandbox { backend_token: String },
    /// A real backend was configured without the policy provider that
    /// resolves its tier policies.
    BackendWithoutPolicies { backend_token: String },
    /// The conservative no-backend posture was configured alongside a
    /// sandbox or policy instance — an inconsistent posture, refused rather
    /// than guessed at.
    NoneBackendWithInstances,
}

impl core::fmt::Display for GovernError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::BackendWithoutSandbox { backend_token } => write!(
                f,
                "sandbox backend '{backend_token}' is configured but no Sandbox instance was \
                 provided — refusing to compose an ungated dispatch path"
            ),
            Self::BackendWithoutPolicies { backend_token } => write!(
                f,
                "sandbox backend '{backend_token}' is configured but no PolicyProvider instance \
                 was provided — tier policies could never resolve"
            ),
            Self::NoneBackendWithInstances => write!(
                f,
                "the conservative no-backend posture carries a Sandbox or PolicyProvider \
                 instance — the posture is inconsistent and is refused"
            ),
        }
    }
}

impl std::error::Error for GovernError {}

impl GovernDeps {
    /// The §8.6 conservative posture: backend `none`, no sandbox and no
    /// policy provider, critic-exercisable. Contained-tier dispatches run
    /// ungated by containment (which is legal ONLY at this tier); every
    /// above-contained tier refuses dispatch.
    pub fn none_backend(workspace: WorkspaceRef) -> Self {
        Self {
            backend_token: crate::config::defaults::SANDBOX_BACKEND.to_owned(),
            sandbox: None,
            policies: None,
            exercisable_authority: Authority::Critic,
            workspace,
        }
    }

    /// The same conservative posture without a workspace anchor: under
    /// this posture no policy ever resolves (the workspace field exists
    /// for the real-backend posture's policy resolution), so the anchor is
    /// never read and need not be supplied.
    pub fn none_workspace() -> Self {
        Self::none_backend(WorkspaceRef::new("none-posture").expect("valid workspace ref"))
    }

    /// A real-backend posture: the token names the backend, and BOTH the
    /// sandbox fronting it and the policy provider resolving its tier
    /// policies are REQUIRED — an inconsistent posture (a backend without
    /// one of its instances, or the conservative token WITH instances) is
    /// a typed construction error, never a guessed-at default (prereg R1:
    /// an optional gate would be an unsandboxed fallback wearing a
    /// type-system costume).
    #[allow(clippy::result_large_err)]
    pub fn with_backend(
        backend_token: &str,
        sandbox: Option<Arc<dyn Sandbox>>,
        policies: Option<Arc<dyn PolicyProvider>>,
        exercisable_authority: Authority,
        workspace: WorkspaceRef,
    ) -> Result<Self, GovernError> {
        if backend_token == crate::config::defaults::SANDBOX_BACKEND {
            return Err(GovernError::NoneBackendWithInstances);
        }
        if backend_token.trim().is_empty() {
            return Err(GovernError::BackendWithoutSandbox {
                backend_token: backend_token.to_owned(),
            });
        }
        let Some(sandbox) = sandbox else {
            return Err(GovernError::BackendWithoutSandbox {
                backend_token: backend_token.to_owned(),
            });
        };
        let Some(policies) = policies else {
            return Err(GovernError::BackendWithoutPolicies {
                backend_token: backend_token.to_owned(),
            });
        };
        Ok(Self {
            backend_token: backend_token.to_owned(),
            sandbox: Some(sandbox),
            policies: Some(policies),
            exercisable_authority,
            workspace,
        })
    }

    /// The resolved backend token this posture was composed with.
    pub fn backend_token(&self) -> &str {
        &self.backend_token
    }

    /// Whether this is the conservative no-backend posture.
    pub fn is_none_posture(&self) -> bool {
        self.backend_token == crate::config::defaults::SANDBOX_BACKEND
    }

    /// The exercisable-authority ceiling (§17.1).
    pub fn exercisable_authority(&self) -> Authority {
        self.exercisable_authority
    }
}

/// The dispatch facts the gate decides on. Every field is a REAL fact the
/// caller holds at dispatch time — the unit's recorded identity, the
/// runner's reported capabilities, and (for the lattice conjunct) the
/// answer-key facts as last parsed.
#[derive(Debug, Clone)]
pub struct GateFacts {
    pub unit: UnitId,
    pub tier: BlastRadius,
    pub authority: Authority,
    /// The runner's REPORTED isolation (§22.2: honest by construction — a
    /// harness that is not itself wrapped by a sandbox reports `none`, and
    /// that honesty is exactly what this conjunct consumes).
    pub runner_isolation: Isolation,
}

/// A dispatch the gate admitted: the containment facts that authorize the
/// invocation, carried so the invocation record (and any later wrap) can
/// prove validation preceded use (AK-048).
///
/// EARNED, never assembled (the Task-13 `AdmittedSettlement` precedent):
/// every field is private and no constructor is reachable outside this
/// module, so the only way to hold a `GateAdmission` is to have passed
/// [`AuthorityGate::admit`]'s six conjuncts. A caller cannot mint one, and
/// cannot mutate one it was handed.
#[derive(Debug, Clone)]
pub struct GateAdmission {
    unit: UnitId,
    tier: BlastRadius,
    containment: Containment,
}

/// What an admission authorizes — the two shapes §5.3.5 allows, and no
/// third. Before the correction the same facts were two parallel
/// `Option`s, which made "a policy with no validation" REPRESENTABLE: a
/// malformed admission that a wrap could not distinguish from the
/// legitimate no-wrap posture. One enum makes the mixed shape
/// unrepresentable rather than merely rejected.
///
/// Private on purpose: a public variant would be a constructor for the
/// admission's own contents.
#[derive(Debug, Clone)]
enum Containment {
    /// A contained-tier dispatch under the conservative no-backend posture
    /// (§8.6; §5.3.5's "null only when tier=contained and backend=none").
    /// No policy exists, so there is nothing to wrap — and this is the ONLY
    /// admission shape for which that is true.
    NoBackendContained,
    /// A real backend: the resolved tier policy together with the CLEAN
    /// validation it earned. Together, always. Boxed because the pair
    /// dwarfs the other variant, and an admission is passed by reference
    /// far more often than it is built.
    Validated(Box<ValidatedContainment>),
}

/// The real-backend containment pair. Its fields are `pub(self)` by
/// omission: the type is private to this module, so it is no more a
/// constructor for an admission than [`Containment`] is.
#[derive(Debug, Clone)]
struct ValidatedContainment {
    policy: SandboxPolicy,
    validation: PolicyValidation,
}

impl GateAdmission {
    /// The unit this admission authorizes.
    pub fn unit(&self) -> &UnitId {
        &self.unit
    }

    /// The tier the dispatch was admitted at.
    pub fn tier(&self) -> BlastRadius {
        self.tier
    }

    /// The resolved tier policy, or none under the contained/no-backend
    /// posture.
    pub fn policy(&self) -> Option<&SandboxPolicy> {
        match &self.containment {
            Containment::NoBackendContained => None,
            Containment::Validated(inner) => Some(&inner.policy),
        }
    }

    /// The clean validation the policy earned — present exactly when
    /// [`GateAdmission::policy`] is, by construction.
    pub fn validation(&self) -> Option<&PolicyValidation> {
        match &self.containment {
            Containment::NoBackendContained => None,
            Containment::Validated(inner) => Some(&inner.validation),
        }
    }

    /// The invocation's `sandbox_policy` ref: the resolved policy's
    /// reference, or none under the contained/none-backend posture.
    pub fn policy_ref(&self) -> Option<SandboxPolicyRef> {
        self.policy().map(|policy| policy.reference.clone())
    }
}

/// The command a caller must run for an admitted dispatch. Both arms are a
/// command — there is no arm that means "I have nothing for you, use what
/// you had", because that is the arm a caller turns into an unsandboxed
/// fallback (§21.3, AK-047).
#[derive(Debug, Clone, PartialEq)]
pub enum BuilderCommand {
    /// The wrapped invocation: the FIRST process is the sandbox (§17.5.4,
    /// AK-048). Run exactly this.
    Sandboxed(SandboxedCommand),
    /// The §8.6 contained/no-backend posture: no policy exists to wrap
    /// under, so the gate hands the command back verbatim. This is NOT a
    /// fallback — it is reachable only from an admission the gate itself
    /// minted for the contained tier under the conservative posture, where
    /// §5.3.5 says the invocation legitimately carries no policy.
    Unwrapped { command: String },
}

/// Why a wrap was refused. Every arm is a REFUSAL to produce a command:
/// none of them is convertible into "run it unwrapped" (§21.3: escalate,
/// never fall back).
#[derive(Debug, Clone, PartialEq)]
pub enum WrapRefusal {
    /// A validated admission was presented to a posture holding no sandbox
    /// instance. The admission was earned elsewhere, or the composition
    /// degraded between admission and wrap; either way the wrap cannot
    /// happen and the command must not run.
    PostureLostItsSandbox { unit: UnitId, backend_token: String },
    /// A no-backend admission was presented to a REAL-backend posture —
    /// cross-wiring that would downgrade a backed dispatch to an unwrapped
    /// one.
    AdmissionPostureMismatch { unit: UnitId, backend_token: String },
    /// The backend refused the wrap (the port's own typed error).
    Backend { unit: UnitId, cause: SandboxError },
}

impl WrapRefusal {
    /// The unit this refusal names.
    pub fn unit(&self) -> &UnitId {
        match self {
            Self::PostureLostItsSandbox { unit, .. }
            | Self::AdmissionPostureMismatch { unit, .. }
            | Self::Backend { unit, .. } => unit,
        }
    }
}

impl core::fmt::Display for WrapRefusal {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::PostureLostItsSandbox {
                unit,
                backend_token,
            } => write!(
                f,
                "unit {unit} holds a validated admission but the '{backend_token}' posture \
                 presented to the wrap has no Sandbox instance — refusing to hand back an \
                 unwrapped command (spec 21.3, AK-047)"
            ),
            Self::AdmissionPostureMismatch {
                unit,
                backend_token,
            } => write!(
                f,
                "unit {unit} holds a no-backend admission but the posture presented to the wrap \
                 names backend '{backend_token}' — the admission was not earned under this \
                 posture (spec 21.3, AK-047)"
            ),
            Self::Backend { unit, cause } => {
                write!(f, "the sandbox refused to wrap unit {unit}: {cause}")
            }
        }
    }
}

impl std::error::Error for WrapRefusal {}

/// Why the gate refused a dispatch — one typed arm per hard conjunct, each
/// carrying the blocking fact. These are GOVERNANCE EVENTS: the controller
/// records them durably and escalates; none of them is ever converted into
/// "dispatch anyway" (§21.3).
#[derive(Debug, Clone, PartialEq)]
pub enum GateRefusal {
    /// §17.1 line 1 / AK-044: irreversible work is never an automatic
    /// dispatch candidate. The scheduler refuses it first; this arm is the
    /// dispatch-time second line (a forged or stale plan input cannot
    /// smuggle the tier through).
    Irreversible { unit: UnitId },
    /// §17.1 line 2: the unit needs an authority the run cannot exercise —
    /// escalate BEFORE any transform.
    AuthorityAboveCeiling {
        unit: UnitId,
        required: Authority,
        exercisable: Authority,
    },
    /// §17.1 / AK-046: the dispatch's unit facts rank below the recorded
    /// answer-key facts (or an agent-originated movement was proposed) —
    /// the runtime never honors a lattice decrease.
    LatticeMovementRefused { movement: LatticeMovement },
    /// §17.3 / AK-050: the runner's reported isolation is below the tier's
    /// required rung.
    CapabilityMismatch {
        unit: UnitId,
        tier: BlastRadius,
        runner_isolation: Isolation,
        required: Isolation,
    },
    /// §8.6: the tier is above contained and the posture is the
    /// conservative no-backend one — "any higher tier without a backend
    /// refuses to dispatch".
    NoBackendForTier { unit: UnitId, tier: BlastRadius },
    /// The tier's policy could not be resolved (AK-049's structural
    /// refusal for the top tier arrives here as `NoPolicyForTier` if any
    /// path ever asks; other causes are incomplete policy artifacts).
    PolicyUnresolved {
        unit: UnitId,
        tier: BlastRadius,
        cause: PolicyProviderError,
    },
    /// AK-047: validation FAILED — escalation, never an unsandboxed
    /// fallback. The findings name every offense.
    ValidationEscalated {
        unit: UnitId,
        policy: SandboxPolicyRef,
        findings: Vec<String>,
    },
    /// The configured backend could not be probed at validation time
    /// (§21.3's escalation class for an unusable backend).
    BackendUnusable {
        unit: UnitId,
        backend: String,
        detail: String,
    },
}

impl GateRefusal {
    /// The unit this refusal names — the controller's escalation key.
    pub fn unit(&self) -> &UnitId {
        match self {
            Self::Irreversible { unit }
            | Self::AuthorityAboveCeiling { unit, .. }
            | Self::CapabilityMismatch { unit, .. }
            | Self::NoBackendForTier { unit, .. }
            | Self::PolicyUnresolved { unit, .. }
            | Self::ValidationEscalated { unit, .. }
            | Self::BackendUnusable { unit, .. } => unit,
            Self::LatticeMovementRefused { movement } => match movement {
                LatticeMovement::Refused { unit, .. } => unit,
                LatticeMovement::Allowed => {
                    unreachable!("a refusal arm never carries an allowed movement")
                }
            },
        }
    }

    /// The single-line summary for the escalation ticket and the durable
    /// governance event.
    pub fn summary(&self) -> String {
        match self {
            Self::Irreversible { unit } => format!(
                "unit {unit} is irreversible: never an automatic dispatch candidate (spec 17.1, \
                 AK-044)"
            ),
            Self::AuthorityAboveCeiling {
                unit,
                required,
                exercisable,
            } => format!(
                "unit {unit} requires {required} authority; the run exercises {exercisable} — \
                 escalating before any transform (spec 17.1)"
            ),
            Self::LatticeMovementRefused { movement } => match movement {
                LatticeMovement::Refused { rule, .. } => {
                    format!("lattice movement refused: {rule}")
                }
                LatticeMovement::Allowed => unreachable!("checked by unit()"),
            },
            Self::CapabilityMismatch {
                unit,
                tier,
                runner_isolation,
                required,
            } => format!(
                "runner reports isolation {runner_isolation}, below the {tier} tier's required \
                 rung {required} — refused for unit {unit} (spec 17.3, AK-050)"
            ),
            Self::NoBackendForTier { unit, tier } => format!(
                "the {tier} tier refuses dispatch without a backend — unit {unit} escalates \
                 (spec 8.6)"
            ),
            Self::PolicyUnresolved { unit, tier, cause } => {
                format!("the {tier} tier policy could not be resolved for unit {unit}: {cause}")
            }
            Self::ValidationEscalated {
                unit,
                policy,
                findings,
            } => format!(
                "sandbox policy {policy} failed validation for unit {unit} ({} finding(s)) — \
                 escalation, never an unsandboxed fallback (spec 21.3, AK-047)",
                findings.len()
            ),
            Self::BackendUnusable {
                unit,
                backend,
                detail,
            } => format!(
                "sandbox backend '{backend}' is unusable for unit {unit}: {detail} (spec 21.3)"
            ),
        }
    }
}

/// The §17.1 authority gate (plan Task 14). Stateless: every fact arrives
/// per admission, fresh.
#[derive(Debug, Clone)]
pub struct AuthorityGate;

impl AuthorityGate {
    /// No `Default` on purpose (the domain `DraftUnit` precedent): a default
    /// gate would imply a default governance posture, and postures are
    /// composed facts, not implicit state.
    #[allow(clippy::new_without_default)]
    pub fn new() -> Self {
        Self
    }

    /// Admits or refuses one dispatch. `recorded` is the answer-key
    /// (blast radius, authority) pair as last parsed, for the lattice
    /// conjunct; `None` skips that conjunct (a caller with no recorded
    /// facts cannot fabricate them, and the gate never invents a floor).
    pub fn admit(
        &self,
        deps: &GovernDeps,
        facts: &GateFacts,
        recorded: Option<(BlastRadius, Authority)>,
        deadline: Deadline,
    ) -> Result<GateAdmission, GateRefusal> {
        // Conjunct 1 — irreversible never dispatches (AK-044).
        if facts.tier == BlastRadius::Irreversible {
            return Err(GateRefusal::Irreversible {
                unit: facts.unit.clone(),
            });
        }
        // Conjunct 2 — the unit's authority must be within the exercisable
        // ceiling (§17.1 line 2): escalate before any transform.
        if gauntlet_domain::authority_rank(facts.authority)
            > gauntlet_domain::authority_rank(deps.exercisable_authority())
        {
            return Err(GateRefusal::AuthorityAboveCeiling {
                unit: facts.unit.clone(),
                required: facts.authority,
                exercisable: deps.exercisable_authority(),
            });
        }
        // Conjunct 3 — the dispatch facts may not rank below the recorded
        // facts (AK-046's dispatch-time defense). The movement is reviewed
        // as agent-originated — through the runtime, a DISPATCH can never
        // be the operator's editing surface; the operator's path to moving
        // a tier is the ruler, outside this gate entirely.
        if let Some(recorded) = recorded {
            let movement = posture::review_lattice_movement(
                facts.unit.clone(),
                recorded,
                (facts.tier, facts.authority),
                &ChangeOrigin::AgentRole(gauntlet_domain::RoleId::Builder),
            );
            if let LatticeMovement::Refused { .. } = movement {
                return Err(GateRefusal::LatticeMovementRefused { movement });
            }
        }
        // Conjunct 4 — the runner's reported isolation meets the tier's
        // required rung (AK-050).
        let required = posture::required_isolation(facts.tier);
        if facts.runner_isolation < required {
            return Err(GateRefusal::CapabilityMismatch {
                unit: facts.unit.clone(),
                tier: facts.tier,
                runner_isolation: facts.runner_isolation,
                required,
            });
        }
        // Contained under the conservative posture needs no policy (§8.6/
        // §5.3.5): the admission carries none and that shape is legal ONLY
        // here.
        if deps.is_none_posture() {
            if facts.tier == BlastRadius::Contained {
                return Ok(GateAdmission {
                    unit: facts.unit.clone(),
                    tier: facts.tier,
                    containment: Containment::NoBackendContained,
                });
            }
            // Conjunct 5 — above contained without a backend refuses
            // dispatch (§8.6).
            return Err(GateRefusal::NoBackendForTier {
                unit: facts.unit.clone(),
                tier: facts.tier,
            });
        }
        let (sandbox, policies) = match (&deps.sandbox, &deps.policies) {
            (Some(sandbox), Some(policies)) => (sandbox, policies),
            // Construction guarantees this arm is unreachable; kept total
            // so no future edit can make the gate guess.
            _ => {
                return Err(GateRefusal::BackendUnusable {
                    unit: facts.unit.clone(),
                    backend: deps.backend_token().to_owned(),
                    detail: "the posture lost its sandbox or policy instances between \
                             composition and dispatch"
                        .to_owned(),
                });
            }
        };
        // Conjunct 6 — the tier's policy resolves and validates CLEAN
        // (AK-047: a failure escalates, never falls back).
        let policy = policies
            .policy_for(facts.tier, &deps.workspace, deadline)
            .map_err(|cause| GateRefusal::PolicyUnresolved {
                unit: facts.unit.clone(),
                tier: facts.tier,
                cause,
            })?;
        let validation = sandbox
            .validate(&policy, deadline)
            .map_err(|error| match error {
                SandboxError::ValidationFailed { policy, findings } => {
                    GateRefusal::ValidationEscalated {
                        unit: facts.unit.clone(),
                        policy,
                        findings,
                    }
                }
                SandboxError::BackendUnavailable { backend, detail } => {
                    GateRefusal::BackendUnusable {
                        unit: facts.unit.clone(),
                        backend: backend.to_string(),
                        detail,
                    }
                }
                other => GateRefusal::BackendUnusable {
                    unit: facts.unit.clone(),
                    backend: deps.backend_token().to_owned(),
                    detail: other.to_string(),
                },
            })?;
        if !validation.is_clean() {
            return Err(GateRefusal::ValidationEscalated {
                unit: facts.unit.clone(),
                policy: policy.reference.clone(),
                findings: validation.findings,
            });
        }
        Ok(GateAdmission {
            unit: facts.unit.clone(),
            tier: facts.tier,
            containment: Containment::Validated(Box::new(ValidatedContainment {
                policy,
                validation,
            })),
        })
    }

    /// Wraps a concrete builder command under an admission's validated
    /// policy (plan Task 14 step 4: "wrap the builder invocation from
    /// process start"). The admission's validation is the one the wrap
    /// consumes — validation precedes use structurally (the port pins it;
    /// this method is the composed surface Tasks 15-16's process runners
    /// call).
    ///
    /// Three outcomes, and no fourth: the wrapped command
    /// ([`BuilderCommand::Sandboxed`]), the command handed back verbatim
    /// under the §8.6 contained/no-backend posture that legitimately has
    /// no policy ([`BuilderCommand::Unwrapped`]), or a typed
    /// [`WrapRefusal`]. What this method deliberately CANNOT return is an
    /// empty success: an `Option` here is the shape a caller writes
    /// `if let Some(c) = wrap(..) { run(c) } else { run(raw) }` around, and
    /// that turns a cross-wired or degraded posture into an unsandboxed
    /// execution — precisely what AK-047 and §21.3 forbid. An admission
    /// the presented posture did not earn is a REFUSAL, never a quiet
    /// no-wrap.
    pub fn wrap_builder_command(
        &self,
        deps: &GovernDeps,
        admission: &GateAdmission,
        command: &str,
        deadline: Deadline,
    ) -> Result<BuilderCommand, WrapRefusal> {
        match (&admission.containment, &deps.sandbox) {
            // The §8.6 legal no-wrap answer — and only under the posture
            // that earned it.
            (Containment::NoBackendContained, None) => Ok(BuilderCommand::Unwrapped {
                command: command.to_owned(),
            }),
            (Containment::NoBackendContained, Some(_)) => {
                Err(WrapRefusal::AdmissionPostureMismatch {
                    unit: admission.unit.clone(),
                    backend_token: deps.backend_token().to_owned(),
                })
            }
            (Containment::Validated(inner), Some(sandbox)) => sandbox
                .wrap(command, &inner.policy, &inner.validation, deadline)
                .map(BuilderCommand::Sandboxed)
                .map_err(|cause| WrapRefusal::Backend {
                    unit: admission.unit.clone(),
                    cause,
                }),
            (Containment::Validated(_), None) => Err(WrapRefusal::PostureLostItsSandbox {
                unit: admission.unit.clone(),
                backend_token: deps.backend_token().to_owned(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::govern::posture::LatticeMovement;
    use gauntlet_domain::SandboxPolicyRef;
    use gauntlet_ports::sandbox::{NetworkPosture, SandboxBackendRef};

    fn unit(n: u16) -> UnitId {
        UnitId::new(&format!("AK-{n:03}")).expect("valid unit id")
    }

    fn workspace() -> WorkspaceRef {
        WorkspaceRef::new("ws-govern-test").expect("valid workspace ref")
    }

    fn facts(n: u16, tier: BlastRadius, authority: Authority, isolation: Isolation) -> GateFacts {
        GateFacts {
            unit: unit(n),
            tier,
            authority,
            runner_isolation: isolation,
        }
    }

    fn far_deadline() -> Deadline {
        let now_secs = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("the wall clock reads")
            .as_secs();
        // The `let` binding is load-bearing, not style: the literals
        // scanner joins a `;`-less tail expression with FOLLOWING lines
        // until the next semicolon, which would merge the span const below
        // into a deadline-named statement. The semicolon terminates that
        // join.
        #[allow(clippy::let_and_return)]
        let bound = Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(
            now_secs + TEST_BOUND_SPAN,
        ));
        bound
    }

    /// The suite's bound span, on its own line (numeric literals never
    /// share a line with a duration-shaped identifier).
    const TEST_BOUND_SPAN: u64 = 60;

    // -- an in-memory sandbox + policy provider for the composed conjuncts --

    struct RecordingSandbox {
        refuse_validation: bool,
    }

    impl Sandbox for RecordingSandbox {
        fn backend_identity(&self) -> gauntlet_ports::sandbox::SandboxBackendRef {
            SandboxBackendRef::new("test-rung").expect("suite backend ref")
        }
        fn validate(
            &self,
            policy: &SandboxPolicy,
            _deadline: Deadline,
        ) -> Result<PolicyValidation, SandboxError> {
            if self.refuse_validation {
                return Err(SandboxError::ValidationFailed {
                    policy: policy.reference.clone(),
                    findings: vec!["the test backend refuses this policy".to_owned()],
                });
            }
            Ok(PolicyValidation {
                policy: policy.reference.clone(),
                backend: policy.backend.clone(),
                isolation: Isolation::Process,
                findings: Vec::new(),
            })
        }
        fn wrap(
            &self,
            command: &str,
            policy: &SandboxPolicy,
            validation: &PolicyValidation,
            _deadline: Deadline,
        ) -> Result<SandboxedCommand, SandboxError> {
            if validation.policy != policy.reference {
                return Err(SandboxError::WrapFailed {
                    detail: "foreign validation".to_owned(),
                });
            }
            Ok(SandboxedCommand {
                command: format!("test-rung -- {command}"),
                policy: policy.reference.clone(),
                validation: validation.clone(),
            })
        }
    }

    struct StaticPolicies {
        refuse_top_tier: bool,
    }

    impl PolicyProvider for StaticPolicies {
        fn policy_for(
            &self,
            tier: BlastRadius,
            _workspace: &WorkspaceRef,
            _deadline: Deadline,
        ) -> Result<SandboxPolicy, PolicyProviderError> {
            if tier == BlastRadius::Irreversible && self.refuse_top_tier {
                return Err(PolicyProviderError::NoPolicyForTier { tier });
            }
            Ok(SandboxPolicy {
                reference: SandboxPolicyRef::new(&format!("tier-{}-test", tier.wire()))
                    .expect("test ref"),
                tier,
                backend: SandboxBackendRef::new("test-rung").expect("test backend ref"),
                network: NetworkPosture::DenyByDefault,
                writable_roots: vec![
                    std::path::Path::new("/")
                        .join("test-workspace")
                        .to_string_lossy()
                        .into_owned(),
                ],
                denied_paths: Vec::new(),
            })
        }
        fn permissions_for(
            &self,
            _role: &gauntlet_domain::RoleId,
            _unit: &UnitId,
            _deadline: Deadline,
        ) -> Result<gauntlet_ports::policy_provider::PermissionSet, PolicyProviderError> {
            Err(PolicyProviderError::Unavailable {
                detail: "test provider resolves sandbox policies only".to_owned(),
            })
        }
    }

    fn backend_deps(exercisable: Authority) -> GovernDeps {
        GovernDeps::with_backend(
            "test-rung",
            Some(Arc::new(RecordingSandbox {
                refuse_validation: false,
            })),
            Some(Arc::new(StaticPolicies {
                refuse_top_tier: true,
            })),
            exercisable,
            workspace(),
        )
        .expect("a consistent backend posture composes")
    }

    #[test]
    fn the_conservative_posture_admits_contained_critic_and_refuses_every_above_contained_tier() {
        let deps = GovernDeps::none_backend(workspace());
        let gate = AuthorityGate::new();
        // Contained + critic + any runner isolation: admitted with NO
        // policy (the §5.3.5-legal shape).
        let admission = gate
            .admit(
                &deps,
                &facts(
                    1,
                    BlastRadius::Contained,
                    Authority::Critic,
                    Isolation::None,
                ),
                Some((BlastRadius::Contained, Authority::Critic)),
                far_deadline(),
            )
            .expect("contained/critic under none-backend admits");
        assert!(admission.policy().is_none());
        assert!(admission.validation().is_none());
        assert!(admission.policy_ref().is_none());
        // Local + cross_cutting: NoBackendForTier (§8.6).
        for tier in [BlastRadius::Local, BlastRadius::CrossCutting] {
            let request = facts(2, tier, Authority::Critic, Isolation::Process);
            let refusal = gate
                .admit(
                    &deps,
                    &request,
                    Some((tier, Authority::Critic)),
                    far_deadline(),
                )
                .expect_err("above contained refuses dispatch without a backend");
            assert!(
                matches!(refusal, GateRefusal::NoBackendForTier { .. }),
                "{refusal:?}"
            );
        }
    }

    #[test]
    fn irreversible_is_refused_before_every_other_conjunct() {
        // AK-044: even with a real backend, a process runner, and human
        // exercisable authority — facts that would satisfy every LATER
        // conjunct — the first conjunct refuses.
        let deps = backend_deps(Authority::Human);
        let refusal = AuthorityGate::new()
            .admit(
                &deps,
                &facts(
                    1,
                    BlastRadius::Irreversible,
                    Authority::Human,
                    Isolation::Vm,
                ),
                Some((BlastRadius::Irreversible, Authority::Human)),
                far_deadline(),
            )
            .expect_err("irreversible never dispatches");
        assert!(matches!(refusal, GateRefusal::Irreversible { .. }));
    }

    #[test]
    fn authority_above_the_ceiling_escalates_before_any_transform() {
        // §17.1 line 2: everything else maximal (real backend, process
        // runner, valid policy would resolve) — the authority conjunct
        // still refuses.
        let deps = backend_deps(Authority::Critic);
        let refusal = AuthorityGate::new()
            .admit(
                &deps,
                &facts(
                    1,
                    BlastRadius::Local,
                    Authority::LeadAgent,
                    Isolation::Process,
                ),
                Some((BlastRadius::Local, Authority::LeadAgent)),
                far_deadline(),
            )
            .expect_err("beyond-critic authority escalates");
        assert!(matches!(
            refusal,
            GateRefusal::AuthorityAboveCeiling {
                required: Authority::LeadAgent,
                exercisable: Authority::Critic,
                ..
            }
        ));
    }

    #[test]
    fn a_none_isolation_runner_is_refused_for_any_tier_above_contained() {
        // AK-050, the runner half — maximal everywhere else (backend real,
        // policy would resolve, authority within ceiling).
        let deps = backend_deps(Authority::Critic);
        let gate = AuthorityGate::new();
        for tier in [BlastRadius::Local, BlastRadius::CrossCutting] {
            let request = facts(1, tier, Authority::Critic, Isolation::None);
            let refusal = gate
                .admit(
                    &deps,
                    &request,
                    Some((tier, Authority::Critic)),
                    far_deadline(),
                )
                .expect_err("a none-isolation runner is refused above contained");
            assert!(
                matches!(
                    refusal,
                    GateRefusal::CapabilityMismatch {
                        runner_isolation: Isolation::None,
                        required: Isolation::Process,
                        ..
                    }
                ),
                "{refusal:?}"
            );
        }
        // The high-quality dimension (AK-045's shape): a CONTAINED dispatch
        // with the SAME none runner admits — the refusal above is the
        // capability conjunct alone, not a quality judgment.
        let contained_request = facts(
            2,
            BlastRadius::Contained,
            Authority::Critic,
            Isolation::None,
        );
        gate.admit(
            &deps,
            &contained_request,
            Some((BlastRadius::Contained, Authority::Critic)),
            far_deadline(),
        )
        .expect("contained admits the none runner under a backend too (policy resolves)");
    }

    #[test]
    fn a_dispatch_fact_below_the_recorded_facts_is_refused_as_lattice_movement() {
        // AK-046's dispatch-time defense: a plan input claiming a LOWER
        // tier than the recorded answer key facts is refused — the runtime
        // never honors a decrease, whatever assembled the plan.
        let deps = backend_deps(Authority::Critic);
        let refusal = AuthorityGate::new()
            .admit(
                &deps,
                &facts(
                    1,
                    BlastRadius::Contained,
                    Authority::Critic,
                    Isolation::None,
                ),
                Some((BlastRadius::Local, Authority::LeadAgent)),
                far_deadline(),
            )
            .expect_err("a downgraded dispatch is refused");
        assert!(
            matches!(
                refusal,
                GateRefusal::LatticeMovementRefused {
                    movement: LatticeMovement::Refused { .. }
                }
            ),
            "{refusal:?}"
        );
        assert!(
            refusal.summary().contains("agent role"),
            "the recorded event names the AK-046 rule: {}",
            refusal.summary()
        );
    }

    #[test]
    fn a_validation_failure_escalates_and_never_falls_back() {
        // AK-047 at the gate: maximal everywhere else — the backend is
        // real, the policy resolves, authority and capability fit — and
        // the validation alone refuses. No arm of GateAdmission exists
        // that could carry "dispatch anyway".
        let deps = GovernDeps::with_backend(
            "test-rung",
            Some(Arc::new(RecordingSandbox {
                refuse_validation: true,
            })),
            Some(Arc::new(StaticPolicies {
                refuse_top_tier: true,
            })),
            Authority::Critic,
            workspace(),
        )
        .expect("composes");
        let request = facts(1, BlastRadius::Local, Authority::Critic, Isolation::Process);
        let refusal = AuthorityGate::new()
            .admit(
                &deps,
                &request,
                Some((BlastRadius::Local, Authority::Critic)),
                far_deadline(),
            )
            .expect_err("a failed validation escalates");
        match &refusal {
            GateRefusal::ValidationEscalated { findings, .. } => {
                assert!(!findings.is_empty(), "the findings name the offense");
            }
            other => panic!("expected ValidationEscalated, got {other:?}"),
        }
        assert!(refusal.summary().contains("never an unsandboxed fallback"));
    }

    #[test]
    fn a_real_backend_posture_admits_above_contained_with_the_validated_policy_carried() {
        // AK-048's gate half: the admission carries the resolved policy and
        // its CLEAN validation — the facts the invocation record and any
        // wrap consume, proving validation preceded use.
        let deps = backend_deps(Authority::Critic);
        let gate = AuthorityGate::new();
        let request = facts(1, BlastRadius::Local, Authority::Critic, Isolation::Process);
        let admission = gate
            .admit(
                &deps,
                &request,
                Some((BlastRadius::Local, Authority::Critic)),
                far_deadline(),
            )
            .expect("local/critic/process under a real backend admits");
        let policy = admission.policy().expect("the policy resolved");
        let validation = admission.validation().expect("the validation is carried");
        assert!(validation.is_clean());
        assert_eq!(validation.policy, policy.reference);
        assert_eq!(admission.policy_ref(), Some(policy.reference.clone()));
        // And the wrap consumes exactly that admission's validation.
        let BuilderCommand::Sandboxed(wrapped) = gate
            .wrap_builder_command(&deps, &admission, "probe the workspace", far_deadline())
            .expect("the wrap renders")
        else {
            panic!("a real-backend posture wraps; it never hands back an unwrapped command");
        };
        assert!(wrapped.command.starts_with("test-rung"));
        assert!(wrapped.command.contains("probe the workspace"));
    }

    /// AK-047 / falsifier F4: an admission presented to a posture that did
    /// not earn it must be a TYPED REFUSAL, never a quiet "no wrap needed".
    /// `Ok(None)` is exactly the shape a caller writing
    /// `if let Some(c) = wrap(..) { run(c) } else { run(raw) }` turns into
    /// an unsandboxed fallback — the thing §21.3 forbids.
    #[test]
    fn an_admission_the_posture_did_not_earn_is_a_typed_refusal() {
        let gate = AuthorityGate::new();
        let backend = backend_deps(Authority::Critic);
        let unbacked = GovernDeps::none_backend(workspace());
        // A validated admission, earned under the REAL backend...
        let validated = gate
            .admit(
                &backend,
                &facts(1, BlastRadius::Local, Authority::Critic, Isolation::Process),
                Some((BlastRadius::Local, Authority::Critic)),
                far_deadline(),
            )
            .expect("local/critic/process under a real backend admits");
        // ...presented to a posture that holds no sandbox at all.
        let refused =
            gate.wrap_builder_command(&unbacked, &validated, "probe the workspace", far_deadline());
        assert!(
            matches!(refused, Err(WrapRefusal::PostureLostItsSandbox { .. })),
            "a validated admission under a sandbox-less posture must be a typed refusal, not a \
             silent no-wrap: {refused:?}"
        );
        // And the converse cross-wiring: a no-backend admission presented
        // to a real-backend posture, which would otherwise DOWNGRADE the
        // dispatch to unwrapped.
        // The facts are their own statement: the literals scanner joins a
        // `;`-less tail expression with following lines, and a numeric
        // literal must never share a statement with a duration-shaped
        // identifier.
        let request = facts(
            2,
            BlastRadius::Contained,
            Authority::Critic,
            Isolation::None,
        );
        let no_backend_admission = gate
            .admit(
                &unbacked,
                &request,
                Some((BlastRadius::Contained, Authority::Critic)),
                far_deadline(),
            )
            .expect("contained/critic under the conservative posture admits");
        let refused = gate.wrap_builder_command(
            &backend,
            &no_backend_admission,
            "probe the workspace",
            far_deadline(),
        );
        assert!(
            matches!(refused, Err(WrapRefusal::AdmissionPostureMismatch { .. })),
            "a no-backend admission presented to a real-backend posture must be a typed refusal, \
             not a silent downgrade to unwrapped: {refused:?}"
        );
        // The LEGITIMATE no-wrap answer stays distinguishable from both
        // refusals — and it is a command, not an absence.
        assert_eq!(
            gate.wrap_builder_command(
                &unbacked,
                &no_backend_admission,
                "probe the workspace",
                far_deadline()
            ),
            Ok(BuilderCommand::Unwrapped {
                command: "probe the workspace".to_owned()
            })
        );
    }

    #[test]
    fn construction_refuses_inconsistent_postures() {
        let sandbox: Arc<dyn Sandbox> = Arc::new(RecordingSandbox {
            refuse_validation: false,
        });
        let policies: Arc<dyn PolicyProvider> = Arc::new(StaticPolicies {
            refuse_top_tier: true,
        });
        // A backend token without the sandbox instance.
        assert!(matches!(
            GovernDeps::with_backend(
                "some-backend",
                None,
                Some(policies.clone()),
                Authority::Critic,
                workspace()
            ),
            Err(GovernError::BackendWithoutSandbox { .. })
        ));
        // A backend token without the policy provider.
        assert!(matches!(
            GovernDeps::with_backend(
                "some-backend",
                Some(sandbox.clone()),
                None,
                Authority::Critic,
                workspace()
            ),
            Err(GovernError::BackendWithoutPolicies { .. })
        ));
        // The conservative token WITH instances.
        assert!(matches!(
            GovernDeps::with_backend(
                crate::config::defaults::SANDBOX_BACKEND,
                Some(sandbox),
                Some(policies),
                Authority::Critic,
                workspace()
            ),
            Err(GovernError::NoneBackendWithInstances)
        ));
        // An empty token is not a backend at all.
        assert!(matches!(
            GovernDeps::with_backend("  ", None, None, Authority::Critic, workspace()),
            Err(GovernError::BackendWithoutSandbox { .. })
        ));
    }
}
