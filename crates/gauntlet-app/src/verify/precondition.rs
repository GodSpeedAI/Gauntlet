//! The settlement-precondition layer (plan Task 11 step 4, bounded; spec
//! §16.1 conditions 1–2 only).
//!
//! Settlement itself is Task 13. What this layer proves NOW, at the layer
//! Task 11 owns, is the P12 half of §16.1: **a Unit whose verification is
//! not a PASS record meeting the evidence conditions cannot even approach
//! settlement.**
//!
//! Conditions checked here (spec §16.1, quoted):
//!
//! 1. "A `VerificationRecord` with `Outcome = PASS` exists for it **in this
//!    run**" — the offered [`VerifiedClaim`] must carry PASS and name the
//!    settling run.
//! 2. "Its evidence ref resolves and is pinned in the evidence store" —
//!    re-derived HERE from the store the claim's OWN chain holds (the
//!    claim carries it; there is no store parameter — correction round 1,
//!    C4/B-M5), never taken from the record's say-so and never from a
//!    caller-presented oracle. An unpinned or unresolvable ref refuses.
//!
//! Conditions deliberately NOT checked (Task 13's scope, recorded):
//!
//! - condition 3 — the authority chain (critic/lead_agent/human closure
//!   permissions);
//! - condition 4 — blocking residuals referencing the unit;
//! - condition 5 — the reference digest being unchanged (the I1 tripwire
//!   re-verification at settlement).
//!
//! [`SettlementReadiness`] has no public constructor: the only way to
//! obtain one is to pass a chain-minted [`VerifiedClaim`] through
//! [`settlement_precondition`], so "the precondition held" is a fact the
//! layer earned, not a value a caller assembled.

use gauntlet_domain::{EvidenceRef, Outcome, RunId, VerificationRef};
use gauntlet_ports::deadline::Deadline;

use super::authority::VerifiedClaim;

/// Why a claim cannot approach settlement (spec §21.2: typed, and the
/// three-state rule is preserved — `NotPass` names the exact outcome, it
/// never collapses INCONCLUSIVE into FAIL or into "not passing enough").
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreconditionFailure {
    /// §16.1 condition 1, first half: the outcome is not PASS. Carries the
    /// exact outcome so INCONCLUSIVE stays visibly distinct from FAIL
    /// (AK-005: INCONCLUSIVE never satisfies a settlement requirement — it
    /// is not a FAIL that happens to fail the same check).
    NotPass { outcome: Outcome },
    /// §16.1 condition 1, second half: the PASS record names a run other
    /// than the one settling.
    ForeignRun {
        record_run: RunId,
        settling_run: RunId,
    },
    /// §16.1 condition 2, first half: the evidence ref does not resolve in
    /// the store.
    EvidenceUnresolvable { reference: EvidenceRef },
    /// §16.1 condition 2, second half: the ref resolves but the run never
    /// pinned it.
    EvidenceUnpinned { reference: EvidenceRef },
    /// The store cannot answer the pin-membership question. Fails CLOSED
    /// (I8): unknown pin state is refusing state, never a pass.
    PinMembershipUnknown {
        reference: EvidenceRef,
        detail: String,
    },
}

impl core::fmt::Display for PreconditionFailure {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NotPass { outcome } => write!(
                f,
                "the verification outcome is {outcome} — only PASS approaches settlement \
                 (spec 16.1 condition 1; an INCONCLUSIVE is not a soft PASS)"
            ),
            Self::ForeignRun {
                record_run,
                settling_run,
            } => write!(
                f,
                "the PASS record belongs to run {record_run}, not the settling run \
                 {settling_run} (spec 16.1 condition 1: a PASS record in THIS run)"
            ),
            Self::EvidenceUnresolvable { reference } => write!(
                f,
                "evidence {reference} does not resolve in the evidence store (spec 16.1 \
                 condition 2)"
            ),
            Self::EvidenceUnpinned { reference } => write!(
                f,
                "evidence {reference} resolves but is not pinned to the settling run (spec \
                 16.1 condition 2)"
            ),
            Self::PinMembershipUnknown { reference, detail } => write!(
                f,
                "the pin membership of evidence {reference} cannot be confirmed ({detail}) — \
                 refusing settlement rather than guessing (I8)"
            ),
        }
    }
}

impl std::error::Error for PreconditionFailure {}

/// The earned "conditions 1–2 held" token. Settlement (Task 13) will
/// require one of these alongside ITS conditions (authority, blocking
/// residuals, reference digest). No public constructor: only
/// [`settlement_precondition`] mints it, and only over a chain-minted
/// [`VerifiedClaim`].
pub struct SettlementReadiness {
    verification: VerificationRef,
    evidence: EvidenceRef,
}

impl SettlementReadiness {
    /// The verification the readiness was earned over.
    pub fn verification(&self) -> &VerificationRef {
        &self.verification
    }

    /// The resolved-and-pinned evidence ref (§16.1 condition 2).
    pub fn evidence(&self) -> &EvidenceRef {
        &self.evidence
    }
}

/// Evaluates §16.1 conditions 1–2 for one claim in one settling run. The
/// evidence facts are RE-DERIVED from the store the claim's OWN chain
/// holds (correction round 1, C4/B-M5): [`VerifiedClaim`] carries its
/// chain's evidence store, and the precondition consults THAT — there is
/// no store parameter, so a caller cannot satisfy condition 2 by
/// presenting a stooge store that resolves and pins everything. The
/// claim's own record says what it says, but settlement state is never
/// admitted on say-so — nor on a stranger's oracle.
pub fn settlement_precondition(
    claim: &VerifiedClaim,
    settling_run: &RunId,
    deadline: Deadline,
) -> Result<SettlementReadiness, PreconditionFailure> {
    let evidence = claim.chain_evidence();
    let record = claim.record();
    if record.outcome() != Outcome::Pass {
        return Err(PreconditionFailure::NotPass {
            outcome: record.outcome(),
        });
    }
    if record.run_id() != settling_run {
        return Err(PreconditionFailure::ForeignRun {
            record_run: record.run_id().clone(),
            settling_run: settling_run.clone(),
        });
    }
    let reference = record.evidence().clone();
    match evidence.get(&reference, deadline) {
        Ok(Some(_)) => {}
        Ok(None) => {
            return Err(PreconditionFailure::EvidenceUnresolvable { reference });
        }
        Err(error) => {
            return Err(PreconditionFailure::PinMembershipUnknown {
                reference,
                detail: format!("resolving the evidence failed: {error}"),
            });
        }
    }
    match evidence.is_pinned(settling_run, &reference, deadline) {
        Ok(true) => {}
        Ok(false) => {
            return Err(PreconditionFailure::EvidenceUnpinned { reference });
        }
        Err(error) => {
            return Err(PreconditionFailure::PinMembershipUnknown {
                reference,
                detail: error.to_string(),
            });
        }
    }
    Ok(SettlementReadiness {
        verification: claim.domain_record().id().clone(),
        evidence: reference,
    })
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::super::authority::{GovernedVerification, VerificationAuthority};
    use super::super::ladder::{Ladder, LadderRung, RungKind};
    use super::super::testing::*;
    use super::{PreconditionFailure, settlement_precondition};
    use gauntlet_domain::{
        EvidenceKind, EvidenceLocator, EvidenceRef, EvidenceRequirement, IterationNo, Outcome,
        Producer, RunId, UnitId,
    };
    use gauntlet_ports::evidence_store::EvidenceStore as _;
    use gauntlet_ports::verifier::VerificationRequest;

    fn unit_request(unit: &str) -> VerificationRequest {
        VerificationRequest {
            id: gauntlet_domain::VerificationRef::new("vrf-precondition-0001").expect("ref"),
            run_id: RunId::new(RUN_ULID).expect("fixture run"),
            unit_id: UnitId::new(unit).expect("fixture unit"),
            claim: "the caller's claim".to_owned(),
            requirement: EvidenceRequirement::new(
                EvidenceKind::TestReport,
                EvidenceLocator::new("fixture-report-locator").expect("locator"),
                Producer::Instrument,
            ),
            evidence: EvidenceLocator::new("fixture-report-locator").expect("locator"),
        }
    }

    fn passing_ladder() -> Ladder {
        Ladder::new(vec![LadderRung::new(
            RungKind::TestReport,
            Arc::new(FakeDeterministicVerifier {
                name: "fixture-verifier",
                kinds: &[EvidenceKind::TestReport],
                passing: vec!["fixture-report-locator".to_owned()],
            }),
        )])
        .expect("ladder")
    }

    /// A governed verification at iteration 2 whose claim PASSES (fresh
    /// production at iteration 2, resolving bytes in the CHAIN's OWN
    /// store, which the caller may then pin or not). The chain store is
    /// returned with it: it is the only oracle the precondition consults
    /// (C4).
    fn governed_pass() -> (GovernedVerification, Arc<PinningEvidence>) {
        let (state, store) = authority_parts();
        state.record(
            "evd-pinned",
            "fixture-report-verifier",
            "fixture-report-locator",
            2,
            b"test report bytes",
        );
        store.insert("evd-pinned", b"test report bytes");
        state.checkpoint_at(2);
        let authority =
            VerificationAuthority::new(state.clone(), store.clone(), Arc::new(SeededIds::new()));
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("run"),
                IterationNo::new(2).expect("1-based"),
                passing_ladder(),
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("the fixture run opens");
        (governed, store)
    }

    #[test]
    fn a_pass_record_with_resolving_pinned_evidence_is_ready() {
        let (governed, store) = governed_pass();
        let claim = governed
            .verify_claim(&unit_request("AK-9001"), bound())
            .expect("the chain mints a PASS");
        // NOT pinned yet in the claim's own chain store: readiness refuses
        // (the stooge-store shape, C4 — there is no other store to ask).
        assert!(matches!(
            settlement_precondition(&claim, &RunId::new(RUN_ULID).expect("run"), bound()),
            Err(PreconditionFailure::EvidenceUnpinned { .. })
        ));
        // The run pins the evidence in ITS store; now conditions 1–2 hold.
        store.pin(
            &RunId::new(RUN_ULID).expect("run"),
            &EvidenceRef::new("evd-pinned").expect("ref"),
        );
        let readiness =
            settlement_precondition(&claim, &RunId::new(RUN_ULID).expect("run"), bound())
                .expect("PASS + resolves + pinned = ready");
        assert_eq!(readiness.evidence().as_str(), "evd-pinned");
    }

    #[test]
    fn an_inconclusive_record_is_refused_and_stays_distinct_from_fail() {
        let (state, store) = authority_parts();
        // No production at all: INCONCLUSIVE/evidence_unresolvable.
        let authority =
            VerificationAuthority::new(state.clone(), store, Arc::new(SeededIds::new()));
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("run"),
                IterationNo::new(1).expect("1-based"),
                passing_ladder(),
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("opens");
        let claim = governed
            .verify_claim(&unit_request("AK-9001"), bound())
            .expect("the chain mints INCONCLUSIVE");
        assert_eq!(claim.record().outcome(), Outcome::Inconclusive);
        let refused = settlement_precondition(&claim, &RunId::new(RUN_ULID).expect("run"), bound());
        assert!(matches!(
            &refused,
            Err(PreconditionFailure::NotPass {
                outcome: Outcome::Inconclusive
            })
        ));
        // The three states stay distinct through the layer: FAIL is a
        // different refusal than INCONCLUSIVE, both named.
        assert_ne!(
            PreconditionFailure::NotPass {
                outcome: Outcome::Fail
            },
            PreconditionFailure::NotPass {
                outcome: Outcome::Inconclusive
            }
        );
    }

    #[test]
    fn a_pass_record_from_another_run_is_refused() {
        let (governed, _store) = governed_pass();
        let claim = governed
            .verify_claim(&unit_request("AK-9001"), bound())
            .expect("PASS");
        let other = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAB").expect("other run");
        assert!(matches!(
            settlement_precondition(&claim, &other, bound()),
            Err(PreconditionFailure::ForeignRun { .. })
        ));
    }

    #[test]
    fn unpinned_or_unresolvable_evidence_is_refused() {
        // Unpinned: the chain's own store holds the bytes but the run never
        // pinned them (also the C4 stooge proof — no other store exists).
        let (governed, _store) = governed_pass();
        let claim = governed
            .verify_claim(&unit_request("AK-9001"), bound())
            .expect("PASS");
        assert!(matches!(
            settlement_precondition(&claim, &RunId::new(RUN_ULID).expect("run"), bound()),
            Err(PreconditionFailure::EvidenceUnpinned { .. })
        ));
        // Unresolvable: the bytes vanish from the claim's own store between
        // the verification and the settlement approach.
        let (governed, store) = governed_pass();
        let claim = governed
            .verify_claim(&unit_request("AK-9001"), bound())
            .expect("PASS");
        store.remove("evd-pinned");
        assert!(matches!(
            settlement_precondition(&claim, &RunId::new(RUN_ULID).expect("run"), bound()),
            Err(PreconditionFailure::EvidenceUnresolvable { .. })
        ));
    }

    #[test]
    fn an_unanswerable_pin_query_fails_closed() {
        // The chain's own store cannot answer pin membership: construct it
        // that way up front (the verify path never asks).
        let (state, store) = authority_parts();
        state.record(
            "evd-pinned",
            "fixture-report-verifier",
            "fixture-report-locator",
            2,
            b"test report bytes",
        );
        store.insert("evd-pinned", b"test report bytes");
        state.checkpoint_at(2);
        store.fail_pin_queries();
        let authority =
            VerificationAuthority::new(state.clone(), store, Arc::new(SeededIds::new()));
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("run"),
                IterationNo::new(2).expect("1-based"),
                passing_ladder(),
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("opens");
        let claim = governed
            .verify_claim(&unit_request("AK-9001"), bound())
            .expect("PASS");
        assert!(matches!(
            settlement_precondition(&claim, &RunId::new(RUN_ULID).expect("run"), bound()),
            Err(PreconditionFailure::PinMembershipUnknown { .. })
        ));
    }

    /// C4 (B-M5): the precondition's evidence oracle is the claim's OWN
    /// chain store, carried inside the `VerifiedClaim` — there is no store
    /// parameter for a stooge to travel through. FAIL-FIRST evidence
    /// (2026-08-21): before the fix, a per-call store that resolved and
    /// pinned everything satisfied conditions 1–2 while the chain's own
    /// store held no pin.
    #[test]
    fn a_stooge_store_cannot_satisfy_the_settlement_precondition() {
        let (governed, chain_store) = governed_pass();
        let claim = governed
            .verify_claim(&unit_request("AK-9001"), bound())
            .expect("PASS");
        // The chain's own store holds the bytes but never pinned them.
        // Pre-correction, a stooge store (an independent instance holding
        // the ref AND a pin) satisfied the precondition here; now the only
        // store consulted is the chain's own, and the missing pin refuses.
        let reference = EvidenceRef::new("evd-pinned").expect("ref");
        let holds_but_never_pinned = chain_store
            .get(&reference, bound())
            .expect("the chain store resolves the bytes")
            .is_some();
        assert!(holds_but_never_pinned);
        assert!(matches!(
            settlement_precondition(&claim, &RunId::new(RUN_ULID).expect("run"), bound()),
            Err(PreconditionFailure::EvidenceUnpinned { reference }) if reference.as_str() == "evd-pinned"
        ));
    }

    #[test]
    fn a_chain_minted_model_record_carries_the_non_deterministic_flag() {
        // §14.2, composed: the chain verifies over the MODEL rung (no
        // deterministic rung on this ladder can decide) and the minted B.5
        // record is flagged — instrument.deterministic=false and the model
        // rung's identity.
        use super::super::ladder::ModelRung;
        use gauntlet_domain::{Budget, DiagnosticAllowance, Limits, MicroUsd};

        let (state, store) = authority_parts();
        state.record(
            "evd-model",
            "gauntlet-verify-model",
            "fixture-report-locator",
            1,
            b"a judgment's basis",
        );
        store.insert("evd-model", b"a judgment's basis");
        let budget = Budget::new(
            Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500)).expect("limits"),
            20,
            DiagnosticAllowance::new(100, MicroUsd::new(50)),
        )
        .expect("budget");
        let model = ModelRung::new(
            #[allow(clippy::arc_with_non_send_sync)]
            Arc::new(ScriptedModelRunner {
                root: std::cell::RefCell::new(None),
            }),
            Arc::new(SeededIds::new()),
            None,
            budget,
            IterationNo::new(1).expect("1-based"),
        )
        .rung();
        let ladder = Ladder::new(vec![model]).expect("ladder");
        let authority =
            VerificationAuthority::new(state.clone(), store, Arc::new(SeededIds::new()));
        let governed = authority
            .for_run(
                &RunId::new(RUN_ULID).expect("run"),
                IterationNo::new(1).expect("1-based"),
                ladder,
                state.workspace_root().to_owned(),
                bound(),
            )
            .expect("opens");
        let claim = governed
            .verify_claim(&unit_request("AK-9001"), bound())
            .expect("the model rung decides");
        assert_eq!(claim.record().instrument().name(), "gauntlet-verify-model");
        assert!(
            !claim.record().instrument().deterministic(),
            "every model-produced record is flagged non-deterministic (B.5)"
        );
    }

    /// A scripted AgentRunner for the composed model test: writes a
    /// verification payload echoing the dispatch (a passing judgment).
    struct ScriptedModelRunner {
        root: std::cell::RefCell<Option<std::path::PathBuf>>,
    }

    impl ScriptedModelRunner {
        fn document_for(&self, spec: &gauntlet_ports::agent_runner::RoleInvocationSpec) -> String {
            let payload = gauntlet_ports::verification_record::VerificationRecordDoc::new(
                gauntlet_ports::verification_record::VerificationContent {
                    run_id: spec.run_id.clone(),
                    iteration_no: gauntlet_domain::IterationNo::new(1).expect("1-based"),
                    unit_id: UnitId::new("AK-9001").expect("unit"),
                    claim: "the fixture claim is verified by a test report".to_owned(),
                    requirement: gauntlet_ports::verification_record::RequirementDoc::new(
                        EvidenceKind::TestReport,
                        EvidenceLocator::new("fixture-report-locator").expect("locator"),
                        Producer::Instrument,
                    ),
                    instrument: gauntlet_ports::verification_record::VerifyingInstrument::new(
                        "gauntlet-verify-model".to_owned(),
                        "1".to_owned(),
                        false,
                    ),
                    evidence: gauntlet_domain::EvidenceRef::new("fixture-report-locator")
                        .expect("locator as ref"),
                    produced_at: gauntlet_domain::EpochSeconds::new(BASE_INSTANT + 7),
                    outcome: Outcome::Pass,
                    class: None,
                    detail: "the scripted judgment".to_owned(),
                },
            )
            .expect("the fixture content is well formed");
            format!(
                "{{\"schema\":\"role_result@v1\",\"invocation_id\":\"{}\",\"run_id\":\"{}\",\
                 \"iteration_no\":1,\"role\":\"verifier\",\"context_envelope_digest\":\"{}\",\
                 \"state\":\"completed\",\"payload_schema\":\"verification_record@v1\",\
                 \"payload\":{},\"payment\":{{\"input_tokens\":1,\"output_tokens\":1,\
                 \"tool_calls\":0,\"wall_seconds\":0,\"cost_usd\":0.0}},\
                 \"unavailable_evidence\":[],\"errors\":[]}}",
                spec.invocation_id.as_str(),
                spec.run_id.as_str(),
                spec.context_envelope.digest(),
                payload.to_payload_json(),
            )
        }
    }

    impl gauntlet_ports::agent_runner::AgentRunner for ScriptedModelRunner {
        fn invoke(
            &self,
            spec: gauntlet_ports::agent_runner::RoleInvocationSpec,
        ) -> Result<
            gauntlet_ports::agent_runner::InvocationHandle,
            gauntlet_ports::agent_runner::AgentError,
        > {
            let root = std::env::temp_dir().join(format!(
                "gauntlet-verify-model-composed-{}",
                std::process::id()
            ));
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).expect("scratch root");
            std::fs::write(
                root.join(spec.result_contract_path.as_str()),
                self.document_for(&spec),
            )
            .expect("writes");
            *self.root.borrow_mut() = Some(root);
            Ok(gauntlet_ports::agent_runner::InvocationHandle::new(
                spec.invocation_id,
                None,
            ))
        }
        fn poll(
            &self,
            handle: &gauntlet_ports::agent_runner::InvocationHandle,
        ) -> Result<
            gauntlet_ports::agent_runner::InvocationStatus,
            gauntlet_ports::agent_runner::AgentError,
        > {
            Ok(gauntlet_ports::agent_runner::InvocationStatus::Completed {
                result_ref: gauntlet_domain::ResultContractPath::new(&format!(
                    "verifier-result-{}",
                    handle.invocation_id().as_str()
                ))
                .expect("path"),
            })
        }
        fn collect(
            &self,
            handle: &gauntlet_ports::agent_runner::InvocationHandle,
            _: gauntlet_ports::deadline::Deadline,
        ) -> Result<gauntlet_ports::role_result::RoleResult, gauntlet_ports::agent_runner::AgentError>
        {
            let root = self.root.borrow().clone().expect("invoke precedes collect");
            let path = root.join(format!(
                "verifier-result-{}",
                handle.invocation_id().as_str()
            ));
            let raw = std::fs::read_to_string(path).map_err(|error| {
                gauntlet_ports::agent_runner::AgentError::CollectionFailed {
                    invocation_id: handle.invocation_id().clone(),
                    detail: error.to_string(),
                }
            })?;
            gauntlet_ports::role_result::RoleResult::decode(&raw).map_err(|error| {
                gauntlet_ports::agent_runner::AgentError::CollectionFailed {
                    invocation_id: handle.invocation_id().clone(),
                    detail: error.to_string(),
                }
            })
        }
        fn cancel(
            &self,
            _handle: &gauntlet_ports::agent_runner::InvocationHandle,
            _reason: gauntlet_ports::agent_runner::CancelReason,
            _deadline: gauntlet_ports::deadline::Deadline,
        ) -> Result<(), gauntlet_ports::agent_runner::AgentError> {
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
}
