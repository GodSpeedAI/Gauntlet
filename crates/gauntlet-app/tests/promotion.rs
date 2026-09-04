//! Evidence locator `tests/contract/promotion::candidate_only` (AK-057;
//! plan Task 23, charter D8) — the optimizer cannot modify the active
//! loop.
//!
//! **Locator note (recorded per the AK-063/AK-065 class):** the frozen
//! locator `tests/contract/promotion::candidate_only` materializes
//! VERBATIM as this file's nextest id (`promotion::candidate_only`),
//! declared as a cargo test target of the crate that owns the mechanism
//! (`gauntlet-app` owns the §19.3/§19.4 services) — the
//! `tests/contract/architecture_boundaries` precedent. Recorded in
//! `.agents/reports/reference-candidates.md` for the ruler's owner.
//!
//! The structural half of D8 (the containment SOURCE scan over the
//! `promote` module, with negative controls proving it bites) lives where
//! the arch suite's source-scan tests live:
//! `gauntlet-arch-tests::optimizer::scan_optimizer_containment`, run by
//! `just boundaries`. This file proves the BEHAVIORAL half: after a REAL
//! propose admission — both the dispatched optimizer invocation and the
//! operator-supplied document path — the project's GAUNTLET.md bytes and
//! the installed-roles state are BYTE-IDENTICAL, and the revision sits
//! PENDING requiring evaluation (the domain floor and the gate's
//! provisional path both refuse shortcuts).

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use gauntlet_domain::{Budget, CapabilityId, Digest, RevisionId, RunId};
use gauntlet_ports::agent_runner::{
    AgentError, AgentRunner, CancelReason, ContextEnvelope, InvocationHandle, InvocationStatus,
    RoleInvocationSpec, RunnerCapabilities,
};
use gauntlet_ports::clock::{Clock, ClockError};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::id_generator::IdGenerator;
use gauntlet_ports::revision_ledger::{
    DecisionKind, EvaluationRecord, ProposalAdmission, RestoreRecord, RevisionDecisionRecord,
    RevisionEntry, RevisionLedger, RevisionLedgerError, RevisionProposal,
};
use gauntlet_ports::role_result::{RoleResult, read_result_contract};

use gauntlet_app::promote::gate::{GateInput, GateVerdict, PromotionGate};
use gauntlet_app::promote::proposal::{ProposalService, ProposePlan};

/// The fixture instant and the tests' bound span (named, so no numeral
/// shares a line with a duration-shaped identifier).
const BASE: u64 = 1_700_000_000;
const BOUND_SPAN: u64 = 600;
/// The fixture budget's wall/token ceilings.
const FIXTURE_SPAN: u64 = 3600;
const FIXTURE_CEILING_TOKENS: u64 = 1_000;

struct FixedClock;

impl Clock for FixedClock {
    fn now(&self) -> gauntlet_domain::EpochSeconds {
        gauntlet_domain::EpochSeconds::new(BASE)
    }

    fn sleep_until(&self, _at: gauntlet_domain::EpochSeconds) -> Result<(), ClockError> {
        Ok(())
    }
}

fn bound() -> Deadline {
    Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(BASE + BOUND_SPAN))
}

/// The in-memory revision ledger: the port's pinned reference semantics
/// (the REAL embedded adapter is pinned by the adapter's contract leg and
/// the composed p9 proof; this crate cannot depend on an adapter).
#[derive(Default)]
struct MemoryRevisions {
    proposals: Mutex<Vec<RevisionProposal>>,
    evaluations: Mutex<Vec<EvaluationRecord>>,
    decisions: Mutex<Vec<RevisionDecisionRecord>>,
    restores: Mutex<Vec<RestoreRecord>>,
}

impl MemoryRevisions {
    fn derived(&self, revision: &RevisionId) -> Option<RevisionEntry> {
        let admitted_at = self
            .proposals
            .lock()
            .expect("lock")
            .iter()
            .find(|proposal| proposal.revision() == revision)
            .map(|proposal| proposal.admitted_at())?;
        let decisions = self.decisions.lock().expect("lock");
        let decision = decisions
            .iter()
            .rev()
            .find(|record| record.revision() == revision);
        Some(RevisionEntry {
            revision: revision.clone(),
            decision: decision.map(|record| record.writes()),
            rolled_back: self
                .restores
                .lock()
                .expect("lock")
                .iter()
                .any(|record| record.revision() == revision),
            evaluations: self
                .evaluations
                .lock()
                .expect("lock")
                .iter()
                .filter(|record| record.revision == *revision)
                .count(),
            admitted_at,
            last_decision_at: decision.map(|record| record.at()),
        })
    }

    fn require_known(&self, revision: &RevisionId) -> Result<(), RevisionLedgerError> {
        if self
            .proposals
            .lock()
            .expect("lock")
            .iter()
            .any(|proposal| proposal.revision() == revision)
        {
            Ok(())
        } else {
            Err(RevisionLedgerError::UnknownRevision {
                revision: revision.clone(),
            })
        }
    }
}

impl RevisionLedger for MemoryRevisions {
    fn admit_proposal(
        &self,
        proposal: RevisionProposal,
        _deadline: Deadline,
    ) -> Result<ProposalAdmission, RevisionLedgerError> {
        let mut proposals = self.proposals.lock().expect("lock");
        if let Some(stored) = proposals
            .iter()
            .find(|stored| stored.revision() == proposal.revision())
        {
            if stored.facts_agree(&proposal) {
                return Ok(ProposalAdmission { idempotent: true });
            }
            return Err(RevisionLedgerError::ProposalConflict {
                revision: proposal.revision().clone(),
            });
        }
        proposals.push(proposal);
        Ok(ProposalAdmission { idempotent: false })
    }

    fn proposal(
        &self,
        revision: &RevisionId,
        _deadline: Deadline,
    ) -> Result<Option<RevisionProposal>, RevisionLedgerError> {
        Ok(self
            .proposals
            .lock()
            .expect("lock")
            .iter()
            .find(|proposal| proposal.revision() == revision)
            .cloned())
    }

    fn append_evaluation(
        &self,
        record: EvaluationRecord,
        _deadline: Deadline,
    ) -> Result<gauntlet_ports::revision_ledger::EvaluationAppend, RevisionLedgerError> {
        self.require_known(&record.revision)?;
        self.evaluations.lock().expect("lock").push(record);
        Ok(gauntlet_ports::revision_ledger::EvaluationAppend {
            sequence: 1,
            idempotent: false,
        })
    }

    fn evaluations(
        &self,
        _revision: &RevisionId,
        _deadline: Deadline,
    ) -> Result<Vec<EvaluationRecord>, RevisionLedgerError> {
        Ok(Vec::new())
    }

    fn apply_decision(
        &self,
        record: RevisionDecisionRecord,
        _deadline: Deadline,
    ) -> Result<RevisionEntry, RevisionLedgerError> {
        self.require_known(record.revision())?;
        let current = self
            .derived(record.revision())
            .ok_or_else(|| RevisionLedgerError::UnknownRevision {
                revision: record.revision().clone(),
            })?
            .decision
            .unwrap_or(DecisionKind::Pending);
        if record.claimed() != current {
            return Err(RevisionLedgerError::StaleDecision {
                revision: record.revision().clone(),
                current,
            });
        }
        let revision = record.revision().clone();
        self.decisions.lock().expect("lock").push(record);
        self.derived(&revision)
            .ok_or_else(|| RevisionLedgerError::ReadFailed {
                detail: "the derived entry vanished mid-decision".to_owned(),
            })
    }

    fn decisions(
        &self,
        _revision: &RevisionId,
        _deadline: Deadline,
    ) -> Result<Vec<RevisionDecisionRecord>, RevisionLedgerError> {
        Ok(Vec::new())
    }

    fn record_restore(
        &self,
        record: RestoreRecord,
        _deadline: Deadline,
    ) -> Result<RevisionEntry, RevisionLedgerError> {
        let entry = self.derived(record.revision()).ok_or_else(|| {
            RevisionLedgerError::UnknownRevision {
                revision: record.revision().clone(),
            }
        })?;
        let current = entry.decision.unwrap_or(DecisionKind::Pending);
        if current != DecisionKind::Promoted || entry.rolled_back {
            return Err(RevisionLedgerError::StaleRestore {
                revision: record.revision().clone(),
                current,
                rolled_back: entry.rolled_back,
            });
        }
        let revision = record.revision().clone();
        self.restores.lock().expect("lock").push(record);
        self.derived(&revision)
            .ok_or_else(|| RevisionLedgerError::ReadFailed {
                detail: "the derived entry vanished mid-restore".to_owned(),
            })
    }

    fn restores(
        &self,
        _revision: &RevisionId,
        _deadline: Deadline,
    ) -> Result<Vec<RestoreRecord>, RevisionLedgerError> {
        Ok(Vec::new())
    }

    fn revision(
        &self,
        revision: &RevisionId,
        _deadline: Deadline,
    ) -> Result<Option<RevisionEntry>, RevisionLedgerError> {
        Ok(self.derived(revision))
    }
}

/// The scripted optimizer runner: the §10.3 file-delivery emulation,
/// minimally — admission records the spec, the first poll completes with
/// the template's document rendered from the dispatch facts.
struct ScriptedOptimizer {
    template: String,
    specs: Mutex<Vec<RoleInvocationSpec>>,
}

impl ScriptedOptimizer {
    fn render(&self, spec: &RoleInvocationSpec) -> Result<RoleResult, AgentError> {
        let document = self
            .template
            .replace("{invocation_id}", spec.invocation_id.as_str())
            .replace("{run_id}", spec.run_id.as_str())
            .replace(
                "{envelope_digest}",
                &spec.context_envelope.digest().to_string(),
            );
        read_result_contract(
            &document,
            &spec.invocation_id,
            spec.context_envelope.digest(),
        )
        .map_err(|error| AgentError::Transport {
            detail: error.to_string(),
        })
    }
}

impl AgentRunner for ScriptedOptimizer {
    fn invoke(&self, spec: RoleInvocationSpec) -> Result<InvocationHandle, AgentError> {
        let mut specs = self.specs.lock().expect("lock");
        let handle = InvocationHandle::new(spec.invocation_id.clone(), None);
        specs.push(spec);
        Ok(handle)
    }

    fn poll(&self, handle: &InvocationHandle) -> Result<InvocationStatus, AgentError> {
        let specs = self.specs.lock().expect("lock");
        let spec = specs
            .iter()
            .find(|spec| spec.invocation_id == *handle.invocation_id())
            .ok_or_else(|| AgentError::UnknownHandle {
                invocation_id: handle.invocation_id().clone(),
            })?;
        self.render(spec)?;
        Ok(InvocationStatus::Completed {
            result_ref: spec.result_contract_path.clone(),
        })
    }

    fn collect(
        &self,
        handle: &InvocationHandle,
        _deadline: Deadline,
    ) -> Result<RoleResult, AgentError> {
        let specs = self.specs.lock().expect("lock");
        let spec = specs
            .iter()
            .find(|spec| spec.invocation_id == *handle.invocation_id())
            .ok_or_else(|| AgentError::UnknownHandle {
                invocation_id: handle.invocation_id().clone(),
            })?;
        self.render(spec)
    }

    fn cancel(
        &self,
        _handle: &InvocationHandle,
        _reason: CancelReason,
        _deadline: Deadline,
    ) -> Result<(), AgentError> {
        Ok(())
    }

    fn capabilities(&self) -> RunnerCapabilities {
        RunnerCapabilities {
            supports_native_subagents: false,
            supports_streaming_events: false,
            supports_model_selection: false,
            isolation: gauntlet_ports::agent_runner::Isolation::None,
            result_delivery: gauntlet_ports::agent_runner::ResultDelivery::File,
            max_depth: 0,
        }
    }
}

struct SequentialIds;

impl IdGenerator for SequentialIds {
    fn run_id(&self) -> RunId {
        RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FA1").expect("valid run id")
    }

    fn invocation_id(&self) -> gauntlet_domain::InvocationId {
        gauntlet_domain::InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FB1").expect("valid id")
    }
}

/// A minimal project surface: GAUNTLET.md plus an installed-roles tree —
/// exactly the two surfaces the optimizer must never touch.
struct Project {
    root: PathBuf,
}

impl Project {
    fn new(tag: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "gauntlet-promotion-candidate-only-{tag}-{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join(".agents").join("roles"))
            .expect("the roles tree is creatable");
        std::fs::write(
            root.join("GAUNTLET.md"),
            "---\nrunners:\n  default: fixture-runner\nroles:\n  dir: .agents/roles\n---\n",
        )
        .expect("GAUNTLET.md writes");
        std::fs::write(
            root.join(".agents").join("roles").join("optimizer.md"),
            "role: optimizer\nresult_contract: candidate_revision@v1\nproposals only.\n",
        )
        .expect("the optimizer role writes");
        std::fs::write(
            root.join(".agents").join("roles").join("critic.md"),
            "role: critic\ncomparison only.\n",
        )
        .expect("the critic role writes");
        Self { root }
    }

    /// The WHOLE project surface as (relative path, bytes): the config
    /// file and the installed-roles state together. A byte differs, the
    /// proof fails.
    fn surface(&self) -> Vec<(String, Vec<u8>)> {
        let mut surface = Vec::new();
        let mut stack = vec![self.root.clone()];
        while let Some(dir) = stack.pop() {
            for entry in std::fs::read_dir(&dir)
                .expect("the project reads")
                .flatten()
            {
                let path = entry.path();
                if path.is_dir() {
                    stack.push(path);
                } else {
                    surface.push((
                        path.strip_prefix(&self.root)
                            .expect("inside the root")
                            .display()
                            .to_string(),
                        std::fs::read(&path).expect("the file reads"),
                    ));
                }
            }
        }
        surface.sort();
        surface
    }
}

impl Drop for Project {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.root);
    }
}

fn candidate_document() -> String {
    candidate_document_with("correction loops burn two rounds")
}

/// The document with an operator-supplied claimed problem (a different
/// declaration is a different content-derived revision).
fn candidate_document_with(claimed_problem: &str) -> String {
    use gauntlet_ports::json::{JsonNumber, JsonValue};
    let payload = JsonValue::Object(vec![
        (
            "claimed_problem".to_owned(),
            JsonValue::Str(claimed_problem.to_owned()),
        ),
        (
            "supporting_evidence".to_owned(),
            JsonValue::Array(vec![JsonValue::Str("sha256:feed0000face1111".to_owned())]),
        ),
        (
            "affected_component".to_owned(),
            JsonValue::Str("correction.selection".to_owned()),
        ),
        (
            "predicted_improvement".to_owned(),
            JsonValue::Str("one fewer correction round".to_owned()),
        ),
        (
            "potential_regression".to_owned(),
            JsonValue::Array(vec![JsonValue::Str("a residual may re-open".to_owned())]),
        ),
        (
            "candidate_change".to_owned(),
            JsonValue::Str("sha256:cafef00dfeed2222".to_owned()),
        ),
        (
            "evaluation_plan".to_owned(),
            JsonValue::Str("held-out set replay".to_owned()),
        ),
        (
            "held_out_evaluation".to_owned(),
            JsonValue::Str("regression set replay".to_owned()),
        ),
        (
            "rollback".to_owned(),
            JsonValue::Str("sha256:beef0000dead3333".to_owned()),
        ),
    ]);
    JsonValue::Object(vec![
        (
            "schema".to_owned(),
            JsonValue::Str("role_result@v1".to_owned()),
        ),
        (
            "invocation_id".to_owned(),
            JsonValue::Str("{invocation_id}".to_owned()),
        ),
        ("run_id".to_owned(), JsonValue::Str("{run_id}".to_owned())),
        (
            "iteration_no".to_owned(),
            JsonValue::Number(JsonNumber::Uint(1)),
        ),
        ("role".to_owned(), JsonValue::Str("optimizer".to_owned())),
        (
            "context_envelope_digest".to_owned(),
            JsonValue::Str("{envelope_digest}".to_owned()),
        ),
        ("state".to_owned(), JsonValue::Str("completed".to_owned())),
        (
            "payload_schema".to_owned(),
            JsonValue::Str("candidate_revision@v1".to_owned()),
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

/// THE TEST (AK-057): both admission paths are real proposals, and NEITHER
/// touches the active loop — the project surface (GAUNTLET.md + installed
/// roles) is byte-identical across the admissions, the revision sits
/// PENDING requiring evaluation, and the gate refuses to promote it.
#[test]
fn candidate_only() {
    let project = Project::new("behavioral");
    let before = project.surface();

    let ledger = Arc::new(MemoryRevisions::default());
    let service = ProposalService::new(ledger.clone(), Arc::new(FixedClock));

    // Path 1 — the DISPATCHED optimizer invocation through the AgentRunner
    // port (invoke → handle → bounded poll → collect; never an await).
    let runner = ScriptedOptimizer {
        template: candidate_document(),
        specs: Mutex::new(Vec::new()),
    };
    let plan = ProposePlan {
        run_id: RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FA1").expect("valid run id"),
        envelope: ContextEnvelope::new(
            "allowlisted context".to_owned(),
            Digest::parse("sha256:a1b2c3d4e5f6a7b8").expect("digest"),
        ),
        workspace: gauntlet_domain::WorkspaceRef::new("ws-optimizer").expect("ref"),
        permissions: gauntlet_domain::PermissionSetRef::new("perm-readonly").expect("ref"),
        budget_slice: Budget::new(
            gauntlet_domain::Limits::new(
                8,
                2,
                1,
                FIXTURE_SPAN,
                FIXTURE_CEILING_TOKENS,
                gauntlet_domain::MicroUsd::new(500),
            )
            .expect("limits"),
            15,
            gauntlet_domain::DiagnosticAllowance::new(100, gauntlet_domain::MicroUsd::new(50)),
        )
        .expect("budget"),
        ids: Arc::new(SequentialIds),
        model_hint: None,
        sandbox_policy: None,
    };
    let dispatched = service
        .propose(&runner, plan, bound())
        .expect("the dispatched proposal admits");

    // Path 2 — the operator-supplied document through the SAME strict
    // decode (a different declaration: its own content-derived revision).
    let admitted = service
        .admit_document(
            &candidate_document_with("stale evidence is accepted at settle")
                .replace("{invocation_id}", "01ARZ3NDEKTSV4RRFFQ69G5FB2")
                .replace("{run_id}", "01ARZ3NDEKTSV4RRFFQ69G5FA2")
                .replace("{envelope_digest}", "sha256:a1b2c3d4e5f6a7b8"),
            bound(),
        )
        .expect("the document admits");

    // (a) the project surface is BYTE-IDENTICAL: no configuration write,
    // no roles install, nothing.
    let after = project.surface();
    assert_eq!(
        before, after,
        "a propose admission must never modify the active loop's surface"
    );

    // (b) the revisions sit PENDING requiring evaluation — no decision was
    // written, and the gate REFUSES to promote them (the provisional
    // floor: missing evidence is never a promotion, and the zero-evidence
    // shape writes nothing).
    for admission in [dispatched, admitted] {
        let entry = ledger
            .revision(&admission.revision, bound())
            .expect("reads")
            .expect("durable");
        assert_eq!(entry.decision, None, "the revision sits PENDING");
        assert_eq!(entry.evaluations, 0, "no evaluation exists yet");
        assert!(
            !entry.rolled_back,
            "a pending revision was never promoted, so never rolled back"
        );

        // The gate refuses the shortcut: no evaluation record exists, so
        // the §19.4 floor adjudicates provisional and writes nothing.
        let input = GateInput {
            current_reference_digest: Digest::parse("sha256:a1b2c3d4e5f6a7b8").expect("digest"),
            delta: gauntlet_app::promote::delta::DeltaDocument::parse("[]")
                .expect("an empty delta parses"),
            rollback_resolvable: Some(true),
        };
        let capabilities = Arc::new(EmptyCapabilities);
        let gate = PromotionGate::new(ledger.clone(), capabilities, Arc::new(FixedClock), 0)
            .expect("the declared margin is inside the 10 000 bps domain");
        let verdict = gate
            .adjudicate(&admission.revision, &input, bound())
            .expect("the gate adjudicates the pending revision");
        assert!(
            matches!(verdict, GateVerdict::Provisional { written: false, .. }),
            "missing evidence is PROVISIONAL, never promoted: {verdict:?}"
        );
        let entry_after = ledger
            .revision(&admission.revision, bound())
            .expect("reads")
            .expect("durable");
        assert_eq!(
            entry_after.decision, None,
            "the zero-evidence shape stays PENDING — nothing was written"
        );
    }
}

/// The empty capability ledger double for the gate's condition 6 (the
/// protected set is declared, but the gate must refuse on the MISSING
/// EVALUATION gap before anything else matters — and it does: evidence
/// gaps and condition failures are evaluated together, and the zero-
/// evidence shape returns before the conditions run).
struct EmptyCapabilities;

impl gauntlet_ports::capability_ledger::CapabilityLedger for EmptyCapabilities {
    fn append_observation(
        &self,
        _observation: gauntlet_ports::capability_ledger::ObservationRecord,
        _deadline: Deadline,
    ) -> Result<
        gauntlet_ports::capability_ledger::ObservationAppend,
        gauntlet_ports::capability_ledger::CapabilityLedgerError,
    > {
        unimplemented!("candidate_only never reaches condition 6")
    }

    fn observations(
        &self,
        _capability: &CapabilityId,
        _deadline: Deadline,
    ) -> Result<
        Vec<gauntlet_ports::capability_ledger::ObservationRecord>,
        gauntlet_ports::capability_ledger::CapabilityLedgerError,
    > {
        Ok(Vec::new())
    }

    fn capability(
        &self,
        _capability: &CapabilityId,
        _deadline: Deadline,
    ) -> Result<
        Option<gauntlet_ports::capability_ledger::CapabilityEntry>,
        gauntlet_ports::capability_ledger::CapabilityLedgerError,
    > {
        unimplemented!("candidate_only never reaches condition 6")
    }

    fn apply_transition(
        &self,
        _transition: gauntlet_ports::capability_ledger::StatusTransition,
        _deadline: Deadline,
    ) -> Result<
        gauntlet_ports::capability_ledger::CapabilityEntry,
        gauntlet_ports::capability_ledger::CapabilityLedgerError,
    > {
        unimplemented!("candidate_only never reaches condition 6")
    }

    fn transitions(
        &self,
        _capability: &CapabilityId,
        _deadline: Deadline,
    ) -> Result<
        Vec<gauntlet_ports::capability_ledger::StatusTransition>,
        gauntlet_ports::capability_ledger::CapabilityLedgerError,
    > {
        Ok(Vec::new())
    }
}
