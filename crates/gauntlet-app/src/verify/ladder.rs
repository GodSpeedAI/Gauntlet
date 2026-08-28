//! The §14.2 verification ladder (plan Task 11): cheapest-conclusive-first
//! instrument selection, deterministic before model, with the model rung
//! flagged on every record it mints.
//!
//! ```text
//! file/digest check → schema check → compiler/type check → focused test
//! → … → model judgment → human            (spec §14.2, verbatim order)
//! ```
//!
//! This runtime implements five rungs ([`RungKind`]); the positions mirror
//! the spec's ladder so later rungs slot in without renumbering:
//!
//! | rung | spec ladder position | instrument |
//! |---|---|---|
//! | `FileDigest` | 1 — file/digest check | the Task-11 file-digest adapter crate |
//! | `SchemaCheck` | 2 — schema check | the Task-11 schema-check adapter crate |
//! | `CommandExitCode` | 3 — compiler/type check | the Task-11 command-exit adapter crate |
//! | `TestReport` | 4 — focused test | the Task-11 test-report adapter crate |
//! | `ModelJudgment` | 10 — model judgment | [`ModelRung`] (this module, AgentRunner-backed) |
//!
//! Deterministic-first is STRUCTURAL: positions 1–4 are deterministic
//! instruments, position 10 is the model, and [`Ladder::select_and_verify`]
//! dispatches in ascending position order over the rungs that `supports`
//! the requirement — so when a deterministic rung and the model rung can
//! both decide one claim, the deterministic one runs and ITS identity is
//! what the record carries. The plan's teeth neuter exactly this ordering.
//!
//! The [`ModelRung`] mirrors the Task-10 model comparator adapter: it dispatches
//! a `verifier`-role invocation through the `AgentRunner` port, polls to a
//! terminal status within a bounded run, and strict-parses the
//! `role_result@v1` at the declared path; the payload MUST be
//! `verification_record@v1`. It reports `is_deterministic() == false` and a
//! model record claiming `instrument.deterministic: true` is refused — a
//! model cannot wear a deterministic instrument's flag (§14.2).

use std::sync::Arc;

use gauntlet_domain::{
    Budget, EvidenceLocator, EvidenceRequirement, InstrumentName, IterationNo, ModelIdentity,
    PermissionSetRef, ResultContractPath, RoleId, VerificationRecord, WorkspaceRef,
};
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, ContextEnvelope, InvocationStatus, RoleInvocationSpec,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::id_generator::IdGenerator;
use gauntlet_ports::observer::InstrumentIdentity;
use gauntlet_ports::verification_record::{VERIFICATION_RECORD_SCHEMA, VerificationRecordDoc};
use gauntlet_ports::verifier::{VerificationRequest, Verifier, VerifierError};
use sha2::{Digest as _, Sha256};

/// The model rung's instrument name (the capability is judgment; this names
/// the rung that exercised it).
pub const MODEL_RUNG_NAME: &str = "gauntlet-verify-model";

/// The bounded poll run (the repository's Task-8 convention: polls are
/// non-blocking, so a count stands in for wall-clock patience; `collect`
/// carries the real deadline).
pub const PROBE_CEILING: u32 = 24;

/// The standard verifier instruction (spec §14.1: a verifier "answers
/// exactly one question per claim").
pub const VERIFIER_PROMPT: &str = "decide exactly one claim: does the evidence at the declared \
     locator satisfy the declared requirement? answer PASS, FAIL, or INCONCLUSIVE in a \
     verification_record@v1 result contract at the declared path; decide nothing else";

/// The rung positions this runtime implements. The discriminants are the
/// §14.2 ladder positions (see the module table) — the ordering IS the
/// spec's, and inserting a future rung is a new position, never a renumber.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RungKind {
    /// §14.2 position 1: file/digest check.
    FileDigest,
    /// §14.2 position 2: schema check.
    SchemaCheck,
    /// §14.2 position 3: compiler/type check (a build/command exit).
    CommandExitCode,
    /// §14.2 position 4: focused test.
    TestReport,
    /// §14.2 position 10: model judgment — the rung of last resort, always
    /// flagged non-deterministic on the records it mints.
    ModelJudgment,
}

impl RungKind {
    /// The §14.2 ladder position. Ascending position is
    /// cheapest-conclusive-first.
    pub fn position(self) -> u8 {
        match self {
            Self::FileDigest => 1,
            Self::SchemaCheck => 2,
            Self::CommandExitCode => 3,
            Self::TestReport => 4,
            Self::ModelJudgment => 10,
        }
    }

    /// Whether rungs at this position are deterministic instruments (all
    /// but the model rung today).
    pub fn deterministic(self) -> bool {
        !matches!(self, Self::ModelJudgment)
    }
}

impl core::fmt::Display for RungKind {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        let name = match self {
            Self::FileDigest => "file-digest",
            Self::SchemaCheck => "schema-check",
            Self::CommandExitCode => "command-exit-code",
            Self::TestReport => "test-report",
            Self::ModelJudgment => "model-judgment",
        };
        f.write_str(name)
    }
}

/// One rung: a position and the instrument that occupies it.
#[derive(Clone)]
pub struct LadderRung {
    kind: RungKind,
    verifier: Arc<dyn Verifier>,
}

impl core::fmt::Debug for LadderRung {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        // The instrument behind the rung is opaque; its own identity report
        // is the honest debug view.
        f.debug_struct("LadderRung")
            .field("kind", &self.kind)
            .field("instrument", &self.verifier.instrument().name().as_str())
            .finish()
    }
}

impl LadderRung {
    /// The composition-root constructor. It does not exist in a normal
    /// build of this crate (correction round 1, C1/B-M1): a rung set is
    /// earned authority — injected when a governed verification opens
    /// ([`super::authority`]) — so there is no public constructor a
    /// per-claim caller could hand a ladder through. The `composition`
    /// cargo feature (the audited §7.1 seam, enabled only by composition
    /// roots such as the CLI's proof wiring) and this crate's own tests
    /// are the only builders.
    #[cfg(any(test, feature = "composition"))]
    pub fn new(kind: RungKind, verifier: Arc<dyn Verifier>) -> Self {
        Self { kind, verifier }
    }

    pub fn kind(&self) -> RungKind {
        self.kind
    }

    pub fn verifier(&self) -> &Arc<dyn Verifier> {
        &self.verifier
    }

    /// The instrument's own identity report (spec §14.1) — the ladder
    /// records this on every minted record, never a caller-paired value.
    pub fn identity(&self) -> InstrumentIdentity {
        self.verifier.instrument()
    }
}

/// Ladder construction and dispatch failures — typed (spec §21.2).
#[derive(Debug, Clone, PartialEq)]
pub enum LadderError {
    /// A ladder with no rungs can decide nothing.
    Empty,
    /// Two rungs occupy one §14.2 position; the ladder is mis-configured.
    PositionOccupied {
        position: u8,
        first: RungKind,
        second: RungKind,
    },
    /// A rung's §14.2 position says one thing about determinism and its
    /// instrument says the opposite (correction round 1, C6.6/B-m3): the
    /// "deterministic-first is structural" guarantee rests on the label,
    /// so a mis-labeled rung is refused at construction, never dispatched.
    KindVerifierMismatch {
        kind: RungKind,
        instrument: String,
        kind_is_deterministic: bool,
    },
    /// No rung declared it can decide the requirement (every `supports`
    /// answer was false).
    NoRungCanDecide { detail: String },
    /// A rung that declared support for the requirement FAILED AS AN
    /// INSTRUMENT while deciding it (correction round 1, C2/A-M1). This
    /// TERMINATES selection — the correction loop's business is now "fix
    /// the instrument" (P12's resolution path), never "ask a dearer rung":
    /// model legitimacy is a property of the CLAIM (§14.2 — genuinely not
    /// deterministically measurable), not of the failed attempt. The broken
    /// rung's kind and instrument identity are carried on the refusal.
    RungFailed {
        kind: RungKind,
        instrument: String,
        detail: String,
    },
    /// Every dispatched rung declined (at dispatch) a requirement it had
    /// declared support for — a support/dispatch contradiction in the rung
    /// set, not an instrument failure.
    SupportContradicted { detail: String },
}

impl core::fmt::Display for LadderError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Empty => write!(f, "the verification ladder has no rungs"),
            Self::PositionOccupied {
                position,
                first,
                second,
            } => write!(
                f,
                "two rungs occupy 14.2 position {position} ({first}, {second}); the ladder is \
                 mis-configured"
            ),
            Self::KindVerifierMismatch {
                kind,
                instrument,
                kind_is_deterministic,
            } => write!(
                f,
                "the {kind} rung ({instrument}) is labeled {} but its instrument reports itself \
                 {} — a mis-labeled rung is refused at construction (14.2 determinism is \
                 structural)",
                if *kind_is_deterministic {
                    "deterministic"
                } else {
                    "non-deterministic"
                },
                if *kind_is_deterministic {
                    "non-deterministic"
                } else {
                    "deterministic"
                },
            ),
            Self::NoRungCanDecide { detail } => {
                write!(
                    f,
                    "no rung on the ladder can decide the requirement: {detail}"
                )
            }
            Self::RungFailed {
                kind,
                instrument,
                detail,
            } => write!(
                f,
                "the {kind} rung ({instrument}) failed as an instrument while deciding the \
                 claim: {detail} — selection terminates (improve the instrument; a dearer rung \
                 cannot inherit a broken rung's claim, 14.2)"
            ),
            Self::SupportContradicted { detail } => write!(
                f,
                "every rung that declared support declined the requirement at dispatch: {detail}"
            ),
        }
    }
}

impl std::error::Error for LadderError {}

/// The §14.2 ladder over a configured rung set, ordered
/// cheapest-conclusive-first by construction (ascending §14.2 position;
/// duplicate positions refused).
///
/// `Clone` (plan Task 12 B3): the controller opens a governed verification
/// per iteration anchor, and [`super::VerificationAuthority::for_run`]
/// consumes the ladder by value — so an already-composed ladder must be
/// passable to each open. Cloning an existing ladder mints no new rungs
/// from outside: the rung set still originates only at the composition
/// seam, so the C1 seal is untouched.
#[derive(Clone)]
pub struct Ladder {
    rungs: Vec<LadderRung>,
}

impl Ladder {
    /// The composition-root constructor (same sealing discipline as
    /// [`LadderRung::new`]: no public constructor in a normal build —
    /// correction round 1, C1/B-M1, the `AdmittedComparison` precedent).
    /// Beyond shape (non-empty, distinct positions), construction refuses a
    /// rung whose kind and instrument disagree about determinism — the
    /// §14.2 ordering guarantee rests on the label (C6.6).
    #[cfg(any(test, feature = "composition"))]
    pub fn new(rungs: Vec<LadderRung>) -> Result<Self, LadderError> {
        if rungs.is_empty() {
            return Err(LadderError::Empty);
        }
        for rung in &rungs {
            if rung.kind().deterministic() != rung.verifier().is_deterministic() {
                return Err(LadderError::KindVerifierMismatch {
                    kind: rung.kind(),
                    instrument: rung.identity().name().as_str().to_owned(),
                    kind_is_deterministic: rung.kind().deterministic(),
                });
            }
        }
        let mut ordered = rungs;
        ordered.sort_by_key(|rung| rung.kind().position());
        for pair in ordered.windows(2) {
            if pair[0].kind().position() == pair[1].kind().position() {
                return Err(LadderError::PositionOccupied {
                    position: pair[0].kind().position(),
                    first: pair[0].kind(),
                    second: pair[1].kind(),
                });
            }
        }
        Ok(Self { rungs: ordered })
    }

    /// The rungs in dispatch order (ascending §14.2 position).
    pub fn rungs(&self) -> &[LadderRung] {
        &self.rungs
    }

    /// The §14.2 selection: the CHEAPEST rung (lowest position) that
    /// declares it can conclusively decide the requirement is dispatched.
    /// A dispatched rung's outcomes stand; an instrument that cannot find
    /// the evidence terminates the selection (P12 — no cheaper or dearer
    /// rung can conjure it); a rung that FAILS AS AN INSTRUMENT while
    /// deciding also terminates the selection with a typed refusal naming
    /// it (correction round 1, C2/A-M1: model legitimacy is a property of
    /// the claim, not of the failed attempt — "improve the instrument", P12
    /// resolution, never "ask a dearer rung"). Fall-through past a rung is
    /// legitimate ONLY for `RequirementUnsupported` (the rung's `supports`
    /// claim and its dispatch disagree — the claim was never decided).
    ///
    /// Crate-private: dispatching an instrument is a step INSIDE the earned
    /// chain ([`super::authority`]); a caller cannot drive a bare ladder
    /// over its own request.
    pub(crate) fn select_and_verify(
        &self,
        request: &VerificationRequest,
        deadline: Deadline,
    ) -> Result<LadderDecision, LadderError> {
        let mut dispatched_any = false;
        let mut contradictions = Vec::new();
        for rung in &self.rungs {
            if !rung.verifier().supports(&request.requirement) {
                continue;
            }
            dispatched_any = true;
            match rung.verifier().verify(request, deadline) {
                Ok(record) => {
                    return Ok(LadderDecision::Decided {
                        rung: rung.clone(),
                        record,
                    });
                }
                Err(VerifierError::EvidenceMissing { locator }) => {
                    return Ok(LadderDecision::EvidenceAbsent {
                        locator,
                        rung: rung.clone(),
                    });
                }
                Err(VerifierError::RequirementUnsupported { detail }) => {
                    // The rung's `supports` claim and its dispatch disagree;
                    // record the contradiction and fall through to the next
                    // rung — the claim was never decided.
                    contradictions.push(format!(
                        "{} rung declined the requirement it declared: {detail}",
                        rung.kind()
                    ));
                }
                Err(VerifierError::InstrumentFailed { detail }) => {
                    // C2: a SUPPORTED rung broke mechanically while deciding
                    // the claim. TERMINAL — the broken rung is named, and no
                    // dearer rung (least of all the model) inherits it.
                    return Err(LadderError::RungFailed {
                        kind: rung.kind(),
                        instrument: rung.identity().name().as_str().to_owned(),
                        detail,
                    });
                }
            }
        }
        if dispatched_any {
            Err(LadderError::SupportContradicted {
                detail: contradictions.join("; "),
            })
        } else {
            Err(LadderError::NoRungCanDecide {
                detail: format!(
                    "requirement kind {} at locator {}",
                    request.requirement.kind(),
                    request.requirement.locator()
                ),
            })
        }
    }
}

/// What the ladder concluded for one dispatch.
#[derive(Debug)]
pub(crate) enum LadderDecision {
    /// A rung decided (or recorded its own INCONCLUSIVE). The deciding rung
    /// is retained — its identity is what the chain's record carries.
    Decided {
        rung: LadderRung,
        record: VerificationRecord,
    },
    /// The cheapest deciding rung could not find the evidence at its
    /// locator. Terminal for the claim: the chain mints
    /// INCONCLUSIVE/`evidence_unresolvable` (P12), and the absent rung is
    /// retained for the record's instrument identity.
    EvidenceAbsent {
        locator: EvidenceLocator,
        rung: LadderRung,
    },
}

// ---------------------------------------------------------------------------
// The model rung (§14.2 position 10; spec §14.2: model judgment — legitimate
// only where nothing deterministic can decide)
// ---------------------------------------------------------------------------

/// The AgentRunner-backed model rung. Mirrors the Task-10 model comparator:
/// dispatch → bounded poll → strict `role_result@v1` parse → the payload
/// MUST be `verification_record@v1` → identity cross-checks → adopt the
/// outcome. The composition root injects the runner (a scripted one in
/// tests, a real runner in production) — this rung never names one.
///
/// The rung carries the GOVERNED iteration of the verification it was
/// composed for (correction round 1, C6.3/B-m5a): every dispatch records
/// that iteration on its `RoleInvocationSpec`, and a collected result
/// naming another iteration — or another role — is refused.
pub struct ModelRung {
    runner: Arc<dyn AgentRunner>,
    ids: Arc<dyn IdGenerator>,
    model_hint: Option<ModelIdentity>,
    budget_slice: Budget,
    iteration_no: IterationNo,
}

impl ModelRung {
    /// The composition-root constructor. Sealed with the ladder's own
    /// constructors (correction round 2, F3: `ModelRung::new` was the one
    /// constructor the C1 sealing missed — public in a normal build even
    /// though `rung()`, the only way to give it ladder authority, was
    /// already sealed. Harmless in isolation — a constructed-but-unwrappable
    /// rung decides nothing — but the seal's claim is that NO rung-shaped
    /// value can be built outside the audited §7.1 seam, and one
    /// constructible half is a seam a future refactor could grow on. Gate
    /// it identically: in a normal build the constructor does not exist.)
    #[cfg(any(test, feature = "composition"))]
    pub fn new(
        runner: Arc<dyn AgentRunner>,
        ids: Arc<dyn IdGenerator>,
        model_hint: Option<ModelIdentity>,
        budget_slice: Budget,
        iteration_no: IterationNo,
    ) -> Self {
        Self {
            runner,
            ids,
            model_hint,
            budget_slice,
            iteration_no,
        }
    }

    /// Wraps the rung as a ladder rung at the model-judgment position.
    ///
    /// The `Arc` is the ladder's uniform rung handle; the ports' object
    /// traits are not `Send + Sync` supertrait-bounded (the mock runner's
    /// interior state is deliberately single-threaded), so this carries the
    /// same recorded allow the compare chain's dispatch holders carry.
    ///
    /// Sealed with the ladder's own constructors (C1): no public constructor
    /// in a normal build.
    #[cfg(any(test, feature = "composition"))]
    #[allow(clippy::arc_with_non_send_sync)]
    pub fn rung(self) -> LadderRung {
        LadderRung::new(RungKind::ModelJudgment, Arc::new(self))
    }
}

impl Verifier for ModelRung {
    fn verify(
        &self,
        claim: &VerificationRequest,
        deadline: Deadline,
    ) -> Result<VerificationRecord, VerifierError> {
        let invocation_id = self.ids.invocation_id();
        let context = verifier_envelope(claim);
        let spec = RoleInvocationSpec {
            invocation_id: invocation_id.clone(),
            run_id: claim.run_id.clone(),
            iteration_no: self.iteration_no,
            role: RoleId::Verifier,
            prompt: VERIFIER_PROMPT.to_owned(),
            context_envelope: context,
            workspace: WorkspaceRef::new("verifier-evidence").expect("valid workspace ref"),
            permissions: PermissionSetRef::new("perm-verify-readonly")
                .expect("valid permission ref"),
            result_contract_path: ResultContractPath::new(&format!(
                "verifier-result-{invocation_id}"
            ))
            .expect("a derived result contract path is valid"),
            deadline,
            model_hint: self.model_hint.clone(),
            sandbox_policy: None,
            budget_slice: self.budget_slice.clone(),
        };
        let handle =
            self.runner
                .invoke(spec.clone())
                .map_err(|error| VerifierError::InstrumentFailed {
                    detail: format!("the model rung could not be admitted: {error}"),
                })?;
        let mut probes = 0u32;
        let status = loop {
            if probes >= PROBE_CEILING {
                return Err(VerifierError::InstrumentFailed {
                    detail: format!(
                        "the model rung did not reach a terminal status within {PROBE_CEILING} \
                         polls"
                    ),
                });
            }
            probes += 1;
            let status =
                self.runner
                    .poll(&handle)
                    .map_err(|error| VerifierError::InstrumentFailed {
                        detail: format!("polling the model rung failed: {error}"),
                    })?;
            if status.is_terminal() {
                break status;
            }
        };
        match status {
            InvocationStatus::Completed { .. } => {}
            InvocationStatus::Failed { class } => {
                return Err(VerifierError::InstrumentFailed {
                    detail: format!("the model rung failed: {class}"),
                });
            }
            InvocationStatus::TimedOut => {
                return Err(VerifierError::InstrumentFailed {
                    detail: "the model rung timed out — a timeout is never an outcome".to_owned(),
                });
            }
            InvocationStatus::Cancelled => {
                return Err(VerifierError::InstrumentFailed {
                    detail: "the model rung was cancelled".to_owned(),
                });
            }
            InvocationStatus::Admitted | InvocationStatus::Running { .. } => {
                return Err(VerifierError::InstrumentFailed {
                    detail: "the model rung reported a non-terminal status as terminal".to_owned(),
                });
            }
        }
        let result = self
            .runner
            .collect(&handle, deadline)
            .map_err(collect_error)?;
        // §10.3 rule 6 + §21.2: the result document's identity must bind to
        // THIS dispatch — invocation id, run, and context envelope digest.
        if result.invocation_id() != &invocation_id {
            return Err(VerifierError::InstrumentFailed {
                detail: format!(
                    "the model rung's result names invocation {}, not {}",
                    result.invocation_id(),
                    invocation_id
                ),
            });
        }
        if result.run_id() != &claim.run_id {
            return Err(VerifierError::InstrumentFailed {
                detail: format!(
                    "the model rung's result names run {}, not {}",
                    result.run_id(),
                    claim.run_id
                ),
            });
        }
        // C6.3 (B-m5): the result's ROLE and ITERATION bind to the dispatch
        // — a builder-role or wrong-iteration document is not this
        // verification's judgment, whatever else it echoes.
        if result.role() != &RoleId::Verifier {
            return Err(VerifierError::InstrumentFailed {
                detail: format!(
                    "the model rung's result was delivered under role {}, not the verifier role",
                    result.role()
                ),
            });
        }
        if result.iteration_no() != self.iteration_no {
            return Err(VerifierError::InstrumentFailed {
                detail: format!(
                    "the model rung's result names iteration {}, but this verification is \
                     governed at iteration {}",
                    result.iteration_no().get(),
                    self.iteration_no.get()
                ),
            });
        }
        if result.context_envelope_digest() != spec.context_envelope.digest() {
            return Err(VerifierError::InstrumentFailed {
                detail: "the model rung's result carries a stale context envelope digest"
                    .to_owned(),
            });
        }
        let gauntlet_ports::role_result::RolePayload::Verification(document) = result.payload()
        else {
            return Err(VerifierError::InstrumentFailed {
                detail: format!("the model rung's payload is not {VERIFICATION_RECORD_SCHEMA}"),
            });
        };
        adopt_model_decision(document, claim, self.iteration_no)
    }

    fn is_deterministic(&self) -> bool {
        false
    }

    fn instrument(&self) -> InstrumentIdentity {
        InstrumentIdentity::new(
            InstrumentName::new(MODEL_RUNG_NAME).expect("the rung's own name is a plain token"),
            env!("CARGO_PKG_VERSION").to_owned(),
        )
    }

    fn supports(&self, _requirement: &EvidenceRequirement) -> bool {
        // The rung of last resort declares every requirement — §14.2's
        // ordering, not a capability claim, is what keeps it behind the
        // deterministic rungs.
        true
    }
}

/// Adopts the model's strictly-parsed B.5 decision. Model output is
/// untrusted external input (§21.2): every binding field is cross-checked
/// against the dispatch before anything is admitted — the run, the unit,
/// the requirement, the GOVERNED ITERATION — and a model record claiming
/// `instrument.deterministic: true` is refused outright (§14.2: the flag
/// exists to mark model judgment, and a model cannot shed the mark).
fn adopt_model_decision(
    document: &VerificationRecordDoc,
    claim: &VerificationRequest,
    governed_iteration: IterationNo,
) -> Result<VerificationRecord, VerifierError> {
    if document.run_id() != &claim.run_id || document.unit_id() != &claim.unit_id {
        return Err(VerifierError::InstrumentFailed {
            detail: format!(
                "the model rung's record names {}/{} but was dispatched for {}/{}",
                document.run_id(),
                document.unit_id(),
                claim.run_id,
                claim.unit_id
            ),
        });
    }
    if document.iteration_no() != governed_iteration {
        return Err(VerifierError::InstrumentFailed {
            detail: format!(
                "the model rung's record answers iteration {}, but this verification is governed \
                 at iteration {}",
                document.iteration_no().get(),
                governed_iteration.get()
            ),
        });
    }
    if document.requirement().kind() != claim.requirement.kind()
        || document.requirement().locator() != claim.requirement.locator()
        || document.requirement().producer() != claim.requirement.producer()
    {
        return Err(VerifierError::InstrumentFailed {
            detail: "the model rung's record answers a requirement other than the one dispatched"
                .to_owned(),
        });
    }
    if document.instrument().deterministic() {
        return Err(VerifierError::InstrumentFailed {
            detail: "the model rung's record claims instrument.deterministic=true — a model \
                     cannot wear a deterministic instrument's flag (spec 14.2)"
                .to_owned(),
        });
    }
    Ok(VerificationRecord::new(
        claim.id.clone(),
        claim.run_id.clone(),
        claim.unit_id.clone(),
        document.evidence().clone(),
        document.outcome(),
        InstrumentName::new(MODEL_RUNG_NAME).expect("the rung's own name is a plain token"),
    ))
}

fn collect_error(error: AgentError) -> VerifierError {
    VerifierError::InstrumentFailed {
        detail: format!("collecting the model rung's result failed: {error}"),
    }
}

/// The verifier's context envelope: exactly the claim triple a verifier may
/// see (spec §14.1 answers one question per claim) — the requirement and
/// the evidence locator, digest-covered so the result binds to these bytes.
/// The body is built with the ports' JSON serializer (correction round 1,
/// C6.4/B-m6): a claim or locator containing quotes, backslashes, or
/// control characters cannot corrupt the envelope — the serializer escapes
/// what interpolation would have spliced raw.
fn verifier_envelope(claim: &VerificationRequest) -> ContextEnvelope {
    use gauntlet_ports::json::JsonValue;
    let body = JsonValue::Object(vec![
        ("claim".to_owned(), JsonValue::Str(claim.claim.clone())),
        (
            "requirement".to_owned(),
            JsonValue::Object(vec![
                (
                    "kind".to_owned(),
                    JsonValue::Str(claim.requirement.kind().to_string()),
                ),
                (
                    "locator".to_owned(),
                    JsonValue::Str(claim.requirement.locator().to_string()),
                ),
                (
                    "producer".to_owned(),
                    JsonValue::Str(claim.requirement.producer().to_string()),
                ),
            ]),
        ),
        (
            "evidence".to_owned(),
            JsonValue::Str(claim.evidence.to_string()),
        ),
    ])
    .to_json_text();
    let digest_hex: String = Sha256::digest(body.as_bytes())
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let digest = gauntlet_domain::Digest::new("sha256", &digest_hex)
        .expect("SHA-256 hex satisfies the Digest grammar by construction");
    ContextEnvelope::new(body, digest)
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::path::PathBuf;
    use std::sync::Arc;

    use super::super::testing::*;
    use super::{Ladder, LadderRung, ModelRung, RungKind};
    use gauntlet_domain::{
        Budget, DiagnosticAllowance, EpochSeconds, EvidenceKind, EvidenceLocator,
        EvidenceRequirement, InstrumentName, IterationNo, Limits, MicroUsd, Outcome, Producer,
        RunId, UnitId,
    };
    use gauntlet_ports::agent_runner::{
        AgentError, AgentRunner, InvocationHandle, InvocationStatus, RoleInvocationSpec,
    };
    use gauntlet_ports::observer::InstrumentIdentity;
    use gauntlet_ports::verification_record::{
        BindingClass, RequirementDoc, VerificationContent, VerificationRecordDoc,
        VerifyingInstrument,
    };
    use gauntlet_ports::verifier::{VerificationRequest, Verifier, VerifierError};

    fn requirement(kind: EvidenceKind) -> EvidenceRequirement {
        EvidenceRequirement::new(
            kind,
            EvidenceLocator::new("fixture-report-locator").expect("fixture locator"),
            Producer::Instrument,
        )
    }

    fn request(kind: EvidenceKind) -> VerificationRequest {
        VerificationRequest {
            id: gauntlet_domain::VerificationRef::new("vrf-ladder-0001").expect("ref"),
            run_id: RunId::new(RUN_ULID).expect("fixture run"),
            unit_id: UnitId::new("AK-9001").expect("fixture unit"),
            claim: "the fixture claim".to_owned(),
            requirement: requirement(kind),
            evidence: EvidenceLocator::new("fixture-report-locator").expect("locator"),
        }
    }

    /// A rung-shaped deterministic verifier with a NAME and a kind set.
    struct RungVerifier {
        name: &'static str,
        kinds: &'static [EvidenceKind],
        fail_instrument: bool,
        decline_at_dispatch: bool,
    }

    impl Verifier for RungVerifier {
        fn verify(
            &self,
            claim: &VerificationRequest,
            _: gauntlet_ports::deadline::Deadline,
        ) -> Result<gauntlet_domain::VerificationRecord, VerifierError> {
            if self.fail_instrument {
                return Err(VerifierError::InstrumentFailed {
                    detail: format!("{} broke", self.name),
                });
            }
            if self.decline_at_dispatch {
                return Err(VerifierError::RequirementUnsupported {
                    detail: format!("{} declined at dispatch", self.name),
                });
            }
            Ok(gauntlet_domain::VerificationRecord::new(
                claim.id.clone(),
                claim.run_id.clone(),
                claim.unit_id.clone(),
                gauntlet_domain::EvidenceRef::new(claim.evidence.as_str()).expect("locator as ref"),
                Outcome::Pass,
                InstrumentName::new(self.name).expect("plain token"),
            ))
        }
        fn is_deterministic(&self) -> bool {
            true
        }
        fn instrument(&self) -> InstrumentIdentity {
            InstrumentIdentity::new(
                InstrumentName::new(self.name).expect("plain token"),
                "1".to_owned(),
            )
        }
        fn supports(&self, requirement: &EvidenceRequirement) -> bool {
            self.kinds.contains(&requirement.kind())
        }
    }

    #[test]
    fn the_ladder_refuses_empty_and_duplicate_positions() {
        assert!(matches!(
            Ladder::new(Vec::new()),
            Err(super::LadderError::Empty)
        ));
        let file = LadderRung::new(
            RungKind::FileDigest,
            Arc::new(RungVerifier {
                name: "file-a",
                kinds: &[EvidenceKind::File],
                fail_instrument: false,
                decline_at_dispatch: false,
            }),
        );
        let other_file = LadderRung::new(
            RungKind::FileDigest,
            Arc::new(RungVerifier {
                name: "file-b",
                kinds: &[EvidenceKind::File],
                fail_instrument: false,
                decline_at_dispatch: false,
            }),
        );
        assert!(matches!(
            Ladder::new(vec![file, other_file]),
            Err(super::LadderError::PositionOccupied { position: 1, .. })
        ));
    }

    #[test]
    fn when_a_deterministic_rung_and_the_model_rung_could_both_decide_the_deterministic_one_does() {
        // AK-035's disconfirming case: BOTH rungs declare they can decide
        // the same test_report claim. The deterministic rung sits at 14.2
        // position 4, the model at 10 — the ladder must dispatch the
        // deterministic instrument, and ITS identity is what the decision
        // carries.
        let deterministic = LadderRung::new(
            RungKind::TestReport,
            Arc::new(RungVerifier {
                name: "fixture-report-verifier",
                kinds: &[EvidenceKind::TestReport],
                fail_instrument: false,
                decline_at_dispatch: false,
            }),
        );
        let model = ModelRung::new(
            #[allow(clippy::arc_with_non_send_sync)]
            Arc::new(ScriptedVerifierRunner::passing()),
            Arc::new(SeededIds::new()),
            None,
            budget(),
            governed_iteration(),
        )
        .rung();
        let ladder = Ladder::new(vec![model, deterministic]).expect("ladder");
        // Construction order is irrelevant: dispatch order is by position.
        assert_eq!(ladder.rungs()[0].kind(), RungKind::TestReport);
        assert_eq!(ladder.rungs()[1].kind(), RungKind::ModelJudgment);
        let decision = ladder
            .select_and_verify(&request(EvidenceKind::TestReport), bound())
            .expect("the ladder decides");
        match decision {
            super::LadderDecision::Decided { rung, record } => {
                assert_eq!(rung.kind(), RungKind::TestReport);
                assert_eq!(record.instrument().as_str(), "fixture-report-verifier");
            }
            other => panic!("the deterministic rung should have decided: {other:?}"),
        }
    }

    #[test]
    fn a_rung_declining_at_dispatch_falls_through_to_the_next_that_can_decide() {
        // The ONE legitimate fall-through (correction round 1, C2/A-M1): a
        // rung whose dispatch declines a requirement it declared support
        // for (`RequirementUnsupported`) never decided the claim — the next
        // rung may. A rung that FAILS AS AN INSTRUMENT no longer falls
        // through; that termination is pinned by the C2 test below.
        let declining_schema = LadderRung::new(
            RungKind::SchemaCheck,
            Arc::new(RungVerifier {
                name: "fixture-schema-verifier",
                kinds: &[EvidenceKind::TestReport],
                fail_instrument: false,
                decline_at_dispatch: true,
            }),
        );
        let healthy_test = LadderRung::new(
            RungKind::TestReport,
            Arc::new(RungVerifier {
                name: "fixture-report-verifier",
                kinds: &[EvidenceKind::TestReport],
                fail_instrument: false,
                decline_at_dispatch: false,
            }),
        );
        let ladder = Ladder::new(vec![declining_schema, healthy_test]).expect("ladder");
        let decision = ladder
            .select_and_verify(&request(EvidenceKind::TestReport), bound())
            .expect("the next rung decides");
        match decision {
            super::LadderDecision::Decided { rung, .. } => {
                assert_eq!(rung.kind(), RungKind::TestReport);
            }
            other => panic!("expected the healthy rung to decide: {other:?}"),
        }
    }

    #[test]
    fn a_requirement_no_rung_can_decide_is_a_typed_refusal() {
        let file_only = LadderRung::new(
            RungKind::FileDigest,
            Arc::new(RungVerifier {
                name: "file-only",
                kinds: &[EvidenceKind::File],
                fail_instrument: false,
                decline_at_dispatch: false,
            }),
        );
        let ladder = Ladder::new(vec![file_only]).expect("ladder");
        assert!(matches!(
            ladder.select_and_verify(&request(EvidenceKind::Screenshot), bound()),
            Err(super::LadderError::NoRungCanDecide { .. })
        ));
    }

    #[test]
    fn a_rung_failing_as_an_instrument_is_a_typed_refusal_naming_it_not_an_outcome() {
        // C2's core: the FIRST supported rung that fails mechanically
        // terminates selection — the refusal names the broken rung, and no
        // dearer rung (least of all the model) inherits the claim.
        let a = LadderRung::new(
            RungKind::SchemaCheck,
            Arc::new(RungVerifier {
                name: "schema",
                kinds: &[EvidenceKind::TestReport],
                fail_instrument: true,
                decline_at_dispatch: false,
            }),
        );
        let b = LadderRung::new(
            RungKind::TestReport,
            Arc::new(RungVerifier {
                name: "rung-b",
                kinds: &[EvidenceKind::TestReport],
                fail_instrument: true,
                decline_at_dispatch: false,
            }),
        );
        let ladder = Ladder::new(vec![a, b]).expect("ladder");
        assert!(matches!(
            ladder.select_and_verify(&request(EvidenceKind::TestReport), bound()),
            Err(super::LadderError::RungFailed { kind, ref instrument, .. })
                if kind == RungKind::SchemaCheck && instrument == "schema"
        ));
    }

    // ------------------------------------------------------------------
    // The model rung over a scripted runner (the Task-10 model-rung pattern)
    // ------------------------------------------------------------------

    fn budget() -> Budget {
        Budget::new(
            Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("limits"),
            20,
            DiagnosticAllowance::new(100, MicroUsd::new(50)),
        )
        .expect("budget")
    }

    /// The governed iteration the model rung is composed at (the tests'
    /// fixture anchor; C6.3).
    fn governed_iteration() -> IterationNo {
        IterationNo::new(1).expect("1-based")
    }

    #[derive(Clone, Copy, Debug, PartialEq)]
    enum RungScript {
        Pass,
        InconclusiveStale,
        WrongPayload,
        ForeignInvocation,
        DeterministicLie,
        NeverTerminal,
        /// Writes the role result under ANOTHER role (C6.3: the result's
        /// role must bind to the dispatched verifier role).
        WrongRole,
        /// Writes the role result at ANOTHER iteration (C6.3: the result's
        /// iteration must bind to the governed iteration of the dispatch).
        WrongIteration,
    }

    /// A scripted AgentRunner (the ScriptedCritic pattern): on `invoke` it
    /// writes a `role_result@v1` whose payload is a
    /// `verification_record@v1` minted from the dispatch, applying the
    /// scripted derailment.
    struct ScriptedVerifierRunner {
        script: RungScript,
        requirement: EvidenceRequirement,
        root: PathBuf,
        specs: RefCell<Vec<RoleInvocationSpec>>,
    }

    impl ScriptedVerifierRunner {
        fn passing() -> Self {
            Self::new(RungScript::Pass)
        }

        fn new(script: RungScript) -> Self {
            Self::for_requirement(
                script,
                EvidenceRequirement::new(
                    EvidenceKind::TestReport,
                    EvidenceLocator::new("fixture-report-locator").expect("locator"),
                    Producer::Instrument,
                ),
            )
        }

        fn for_requirement(script: RungScript, requirement: EvidenceRequirement) -> Self {
            let root = std::env::temp_dir().join(format!(
                "gauntlet-verify-model-{}-{:?}",
                std::process::id(),
                script
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).expect("scratch root");
            Self {
                script,
                requirement,
                root,
                specs: RefCell::new(Vec::new()),
            }
        }

        fn document(&self, spec: &RoleInvocationSpec) -> String {
            let outcome = match self.script {
                RungScript::Pass
                | RungScript::WrongPayload
                | RungScript::ForeignInvocation
                | RungScript::DeterministicLie
                | RungScript::NeverTerminal
                | RungScript::WrongRole
                | RungScript::WrongIteration => Outcome::Pass,
                RungScript::InconclusiveStale => Outcome::Inconclusive,
            };
            let instrument =
                VerifyingInstrument::new(super::MODEL_RUNG_NAME.to_owned(), "1".to_owned(), false);
            let payload = VerificationRecordDoc::new(VerificationContent {
                // The payload's own ref is the model's claim; the chain
                // mints the durable document under the dispatch's ref
                // (COR-3), so any valid ref decodes here.
                verification: gauntlet_domain::VerificationRef::new(&format!(
                    "vrf-model-{}",
                    spec.invocation_id.as_str()
                ))
                .expect("valid verification ref"),
                run_id: spec.run_id.clone(),
                // The judgment's own record echoes the dispatch's governed
                // iteration (C6.3: adopt_model_decision binds it).
                iteration_no: spec.iteration_no,
                unit_id: UnitId::new("AK-9001").expect("fixture unit"),
                claim: "the fixture claim".to_owned(),
                requirement: RequirementDoc::new(
                    self.requirement.kind(),
                    self.requirement.locator().clone(),
                    self.requirement.producer(),
                ),
                instrument,
                evidence: gauntlet_domain::EvidenceRef::new("fixture-report-locator")
                    .expect("locator as ref"),
                produced_at: EpochSeconds::new(BASE_INSTANT + 42),
                outcome,
                class: (outcome == Outcome::Inconclusive).then_some(BindingClass::EvidenceStale),
                detail: "the scripted verifier's judgment".to_owned(),
            })
            .expect("the scripted payload is well formed");
            let invocation = match self.script {
                RungScript::ForeignInvocation => "01ARZ3NDEKTSV4RRFFQ69G5FZZ".to_owned(),
                _ => spec.invocation_id.as_str().to_owned(),
            };
            // The runner echoes the dispatch's role and iteration — the two
            // C6.3 binding fields — except when the script derails them.
            let role = match self.script {
                RungScript::WrongRole => "builder".to_owned(),
                _ => "verifier".to_owned(),
            };
            let iteration = match self.script {
                RungScript::WrongIteration => 7u64,
                _ => u64::from(spec.iteration_no.get()),
            };
            let payload_schema = match self.script {
                // A builder payload under the builder schema token — the
                // rung must refuse it as not-a-verification record.
                RungScript::WrongPayload => "builder_result@v1".to_owned(),
                _ => "verification_record@v1".to_owned(),
            };
            let payload_body = match self.script {
                RungScript::WrongPayload => {
                    "{\"attempted\":[],\"changed_paths\":[],\"commands_run\":[],\"artifacts\":\
                     [],\"claims\":[],\"self_checks\":[],\"blocked_on\":[]}"
                        .to_owned()
                }
                RungScript::DeterministicLie => {
                    let mut content = payload.content();
                    content.instrument =
                        VerifyingInstrument::new("model".to_owned(), "1".to_owned(), true);
                    VerificationRecordDoc::new(content)
                        .expect("well formed")
                        .to_payload_json()
                }
                _ => payload.to_payload_json(),
            };
            format!(
                "{{\"schema\":\"role_result@v1\",\"invocation_id\":\"{invocation}\",\"run_id\":\"\
                 {}\",\"iteration_no\":{iteration},\"role\":\"{role}\",\"context_envelope_digest\":\
                 \"{}\",\"state\":\"completed\",\"payload_schema\":\"{payload_schema}\",\"payload\":\
                 {payload_body},\"payment\":{{\"input_tokens\":1,\"output_tokens\":1,\
                 \"tool_calls\":0,\"wall_seconds\":0,\"cost_usd\":0.0}},\
                 \"unavailable_evidence\":[],\"errors\":[]}}",
                spec.run_id.as_str(),
                spec.context_envelope.digest(),
            )
        }
    }

    impl AgentRunner for ScriptedVerifierRunner {
        fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
            self.specs.borrow_mut().push(spec.clone());
            if self.script != RungScript::NeverTerminal {
                let document = self.document(&spec);
                std::fs::write(self.root.join(spec.result_contract_path.as_str()), document)
                    .expect("the scripted verifier writes its contract");
            }
            Ok(InvocationHandle::new(spec.invocation_id, None))
        }
        fn poll(&self, _handle: &InvocationHandle) -> Result<InvocationStatus, AgentError> {
            if self.script == RungScript::NeverTerminal {
                return Ok(InvocationStatus::Running {
                    last_event_at: EpochSeconds::new(BASE_INSTANT),
                    payment_so_far: gauntlet_domain::Payment::zero(),
                });
            }
            let spec = self
                .specs
                .borrow()
                .last()
                .cloned()
                .expect("invoke precedes poll");
            Ok(InvocationStatus::Completed {
                result_ref: spec.result_contract_path,
            })
        }
        fn collect(
            &self,
            handle: &InvocationHandle,
            _: gauntlet_ports::deadline::Deadline,
        ) -> Result<gauntlet_ports::role_result::RoleResult, AgentError> {
            let spec = self
                .specs
                .borrow()
                .last()
                .cloned()
                .expect("invoke precedes collect");
            let raw = std::fs::read_to_string(self.root.join(spec.result_contract_path.as_str()))
                .map_err(|error| AgentError::CollectionFailed {
                invocation_id: handle.invocation_id().clone(),
                detail: error.to_string(),
            })?;
            gauntlet_ports::role_result::RoleResult::decode(&raw).map_err(|error| {
                AgentError::CollectionFailed {
                    invocation_id: handle.invocation_id().clone(),
                    detail: error.to_string(),
                }
            })
        }
        fn cancel(
            &self,
            _handle: &InvocationHandle,
            _reason: gauntlet_ports::agent_runner::CancelReason,
            _deadline: gauntlet_ports::deadline::Deadline,
        ) -> Result<(), AgentError> {
            Ok(())
        }
        fn capabilities(&self) -> gauntlet_ports::agent_runner::RunnerCapabilities {
            gauntlet_ports::agent_runner::RunnerCapabilities {
                supports_native_subagents: false,
                supports_streaming_events: false,
                supports_model_selection: false,
                isolation: gauntlet_ports::agent_runner::Isolation::None,
                result_delivery: gauntlet_ports::agent_runner::ResultDelivery::File,
                max_depth: 1,
            }
        }
    }

    fn model_rung(script: RungScript) -> ModelRung {
        ModelRung::new(
            #[allow(clippy::arc_with_non_send_sync)]
            Arc::new(ScriptedVerifierRunner::new(script)),
            Arc::new(SeededIds::new()),
            None,
            budget(),
            governed_iteration(),
        )
    }

    fn model_rung_for(script: RungScript, requirement: EvidenceRequirement) -> ModelRung {
        ModelRung::new(
            #[allow(clippy::arc_with_non_send_sync)]
            Arc::new(ScriptedVerifierRunner::for_requirement(script, requirement)),
            Arc::new(SeededIds::new()),
            None,
            budget(),
            governed_iteration(),
        )
    }

    #[test]
    fn the_model_rung_reports_non_deterministic_and_its_records_are_flagged() {
        let rung = model_rung(RungScript::Pass);
        assert!(!rung.is_deterministic());
        assert_eq!(rung.instrument().name().as_str(), super::MODEL_RUNG_NAME);
        let record = rung
            .verify(&request(EvidenceKind::TestReport), bound())
            .expect("the scripted pass verifies");
        assert_eq!(record.outcome(), Outcome::Pass);
        assert_eq!(record.instrument().as_str(), super::MODEL_RUNG_NAME);
        // The chain-minted B.5 record over this rung carries the flag —
        // asserted end-to-end in the composed model test below; here the
        // rung's own flag is the contract the chain reads.
        assert!(!rung.is_deterministic());
    }

    #[test]
    fn a_model_record_claiming_determinism_is_refused() {
        let rung = model_rung(RungScript::DeterministicLie);
        assert!(matches!(
            rung.verify(&request(EvidenceKind::TestReport), bound()),
            Err(VerifierError::InstrumentFailed { detail, .. }) if detail.contains("deterministic")
        ));
    }

    #[test]
    fn a_non_verification_payload_is_refused() {
        let rung = model_rung(RungScript::WrongPayload);
        assert!(matches!(
            rung.verify(&request(EvidenceKind::TestReport), bound()),
            Err(VerifierError::InstrumentFailed { detail, .. }) if detail.contains("verification_record")
        ));
    }

    #[test]
    fn a_foreign_invocation_id_is_refused() {
        let rung = model_rung(RungScript::ForeignInvocation);
        assert!(matches!(
            rung.verify(&request(EvidenceKind::TestReport), bound()),
            Err(VerifierError::InstrumentFailed { detail, .. }) if detail.contains("invocation")
        ));
    }

    #[test]
    fn a_never_terminal_runner_is_a_bounded_instrument_failure() {
        let rung = model_rung(RungScript::NeverTerminal);
        assert!(matches!(
            rung.verify(&request(EvidenceKind::TestReport), bound()),
            Err(VerifierError::InstrumentFailed { detail, .. }) if detail.contains("terminal")
        ));
    }

    #[test]
    fn the_model_rung_decides_only_where_no_deterministic_rung_can() {
        // Screenshot evidence: no deterministic rung on this ladder declares
        // it; the model rung (supports-everything, position 10) decides.
        let screenshot = EvidenceRequirement::new(
            EvidenceKind::Screenshot,
            EvidenceLocator::new("fixture-report-locator").expect("locator"),
            Producer::Instrument,
        );
        let model = model_rung_for(RungScript::InconclusiveStale, screenshot).rung();
        let file_only = LadderRung::new(
            RungKind::FileDigest,
            Arc::new(RungVerifier {
                name: "file-only",
                kinds: &[EvidenceKind::File],
                fail_instrument: false,
                decline_at_dispatch: false,
            }),
        );
        let ladder = Ladder::new(vec![model, file_only]).expect("ladder");
        let decision = ladder
            .select_and_verify(&request(EvidenceKind::Screenshot), bound())
            .expect("the model rung decides");
        match decision {
            super::LadderDecision::Decided { rung, record } => {
                assert_eq!(rung.kind(), RungKind::ModelJudgment);
                assert_eq!(record.instrument().as_str(), super::MODEL_RUNG_NAME);
            }
            other => panic!("the model rung should have decided: {other:?}"),
        }
    }

    // ------------------------------------------------------------------
    // Correction round 1 — FAIL-FIRST tests (each fails for its named
    // reason before the fix lands).
    // ------------------------------------------------------------------

    /// C2 (A-M1): a deterministic rung that CAN decide the requirement but
    /// fails MECHANICALLY (a broken instrument, e.g. a malformed report)
    /// must TERMINATE selection with a typed refusal naming it — never fall
    /// through to the model rung. Model legitimacy is a property of the
    /// CLAIM (§14.2: only where the property is genuinely not
    /// deterministically measurable), not of the failed attempt.
    #[test]
    fn a_supported_broken_rung_terminates_selection_and_never_reaches_the_model() {
        let broken_deterministic = LadderRung::new(
            RungKind::SchemaCheck,
            Arc::new(RungVerifier {
                name: "fixture-schema-verifier",
                kinds: &[EvidenceKind::TestReport],
                fail_instrument: true,
                decline_at_dispatch: false,
            }),
        );
        #[allow(clippy::arc_with_non_send_sync)]
        let runner = Arc::new(ScriptedVerifierRunner::passing());
        #[allow(clippy::arc_with_non_send_sync)]
        let model = ModelRung::new(
            runner.clone(),
            Arc::new(SeededIds::new()),
            None,
            budget(),
            governed_iteration(),
        )
        .rung();
        let ladder = Ladder::new(vec![broken_deterministic, model]).expect("ladder");
        match ladder.select_and_verify(&request(EvidenceKind::TestReport), bound()) {
            Err(super::LadderError::RungFailed {
                kind, instrument, ..
            }) => {
                assert_eq!(kind, RungKind::SchemaCheck);
                assert_eq!(instrument, "fixture-schema-verifier");
            }
            other => panic!(
                "a mechanically-broken deciding rung must terminate selection with the typed \
                 refusal naming it, got {other:?}"
            ),
        }
        assert_eq!(
            runner.specs.borrow().len(),
            0,
            "the model rung must not be dispatched below a broken-but-deciding rung"
        );
    }

    /// C6.6 (B-m3): a rung whose KIND claims a deterministic position but
    /// whose instrument reports itself non-deterministic (or vice versa) is
    /// a mis-labeled ladder — the §14.2 ordering guarantee ("deterministic
    /// first is structural") rests on the label. Refused at construction.
    #[test]
    fn a_rung_whose_kind_and_verifier_disagree_is_refused_at_construction() {
        struct NondeterministicRung;
        impl Verifier for NondeterministicRung {
            fn verify(
                &self,
                _: &VerificationRequest,
                _: gauntlet_ports::deadline::Deadline,
            ) -> Result<gauntlet_domain::VerificationRecord, VerifierError> {
                Err(VerifierError::InstrumentFailed {
                    detail: "never dispatched".to_owned(),
                })
            }
            fn is_deterministic(&self) -> bool {
                false
            }
            fn instrument(&self) -> InstrumentIdentity {
                InstrumentIdentity::new(
                    InstrumentName::new("fixture-judgment-rung").expect("plain token"),
                    "1".to_owned(),
                )
            }
        }
        let mislabeled = Ladder::new(vec![LadderRung::new(
            RungKind::FileDigest,
            Arc::new(NondeterministicRung),
        )]);
        assert!(matches!(
            mislabeled,
            Err(super::LadderError::KindVerifierMismatch { kind, .. })
                if kind == RungKind::FileDigest
        ));
        // The mirror image: the model-judgment position paired with a
        // verifier claiming determinism.
        struct LyingDeterministicRung;
        impl Verifier for LyingDeterministicRung {
            fn verify(
                &self,
                _: &VerificationRequest,
                _: gauntlet_ports::deadline::Deadline,
            ) -> Result<gauntlet_domain::VerificationRecord, VerifierError> {
                Err(VerifierError::InstrumentFailed {
                    detail: "never dispatched".to_owned(),
                })
            }
            fn is_deterministic(&self) -> bool {
                true
            }
            fn instrument(&self) -> InstrumentIdentity {
                InstrumentIdentity::new(
                    InstrumentName::new("fixture-lying-rung").expect("plain token"),
                    "1".to_owned(),
                )
            }
        }
        let inverse = Ladder::new(vec![LadderRung::new(
            RungKind::ModelJudgment,
            Arc::new(LyingDeterministicRung),
        )]);
        assert!(matches!(
            inverse,
            Err(super::LadderError::KindVerifierMismatch { kind, .. })
                if kind == RungKind::ModelJudgment
        ));
    }

    /// C6.4 (B-m6): the verifier envelope is built by serializing a JSON
    /// value, so quotes, backslashes, and control characters in a claim or
    /// locator cannot corrupt the envelope body. The body must parse as the
    /// exact claim triple it was built from.
    #[test]
    fn the_verifier_envelope_is_serialized_json_even_with_hostile_text() {
        let hostile = "a claim with \"quotes\", a \\ backslash, a \n newline and a tab\tinside";
        let mut claim = request(EvidenceKind::TestReport);
        claim.claim = hostile.to_owned();
        let envelope = super::verifier_envelope(&claim);
        let parsed = gauntlet_ports::json::parse(envelope.body()).unwrap_or_else(|error| {
            panic!("the envelope body must be valid JSON under hostile claim text: {error}")
        });
        assert_eq!(
            parsed.get("claim"),
            Some(&gauntlet_ports::json::JsonValue::Str(hostile.to_owned())),
            "the claim round-trips through the serialized envelope"
        );
        let requirement = parsed
            .get("requirement")
            .expect("the requirement object is present");
        assert_eq!(
            requirement.get("kind"),
            Some(&gauntlet_ports::json::JsonValue::Str(
                claim.requirement.kind().to_string()
            ))
        );
        assert_eq!(
            requirement.get("locator"),
            Some(&gauntlet_ports::json::JsonValue::Str(
                claim.requirement.locator().to_string()
            ))
        );
        assert_eq!(
            parsed.get("evidence"),
            Some(&gauntlet_ports::json::JsonValue::Str(
                claim.evidence.to_string()
            ))
        );
    }

    /// C6.3 (B-m5a): a model result delivered under ANOTHER role is not a
    /// verifier's judgment — refused.
    #[test]
    fn a_model_result_under_another_role_is_refused() {
        let rung = model_rung(RungScript::WrongRole);
        match rung.verify(&request(EvidenceKind::TestReport), bound()) {
            Err(VerifierError::InstrumentFailed { detail, .. }) => {
                assert!(
                    detail.contains("role"),
                    "the refusal names the role binding: {detail}"
                );
            }
            other => panic!("a role result under another role must be refused, got {other:?}"),
        }
    }

    /// C6.3 (B-m5b): a model result naming ANOTHER iteration than the
    /// governed one is not this dispatch's judgment — refused. (The dispatch
    /// itself must carry the governed iteration, not a hardcoded 1.)
    #[test]
    fn a_model_result_at_another_iteration_is_refused() {
        let rung = model_rung(RungScript::WrongIteration);
        match rung.verify(&request(EvidenceKind::TestReport), bound()) {
            Err(VerifierError::InstrumentFailed { detail, .. }) => {
                assert!(
                    detail.contains("iteration"),
                    "the refusal names the iteration binding: {detail}"
                );
            }
            other => panic!("a role result at another iteration must be refused, got {other:?}"),
        }
    }

    /// C6.3 (B-m5b), the dispatch half: the rung's `RoleInvocationSpec`
    /// carries the GOVERNED iteration of the verification it was composed
    /// for — the pre-correction rung hardcoded iteration 1 on every
    /// dispatch. Composed at iteration 2 over a runner that ECHOES the
    /// dispatch's iteration, the judgment stands; a hardcoded-1 dispatch
    /// would return a wrong-iteration result and be refused (the refusal
    /// is pinned by the test above).
    #[test]
    fn the_model_rung_dispatch_carries_the_governed_iteration() {
        let rung = ModelRung::new(
            #[allow(clippy::arc_with_non_send_sync)]
            Arc::new(ScriptedVerifierRunner::new(RungScript::Pass)),
            Arc::new(SeededIds::new()),
            None,
            budget(),
            IterationNo::new(2).expect("1-based"),
        );
        let record = rung
            .verify(&request(EvidenceKind::TestReport), bound())
            .expect("the judgment that echoes the governed iteration 2 verifies");
        assert_eq!(record.outcome(), Outcome::Pass);
    }
}
