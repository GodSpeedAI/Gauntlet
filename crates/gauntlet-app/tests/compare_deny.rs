//! The R2-N1 composed-path teeth (plan Task 20; brief Item 2): a deny
//! set wired through [`ComparisonAuthority::with_role_deny`] refuses —
//! INSIDE the governed chain, BEFORE the envelope is built — any input
//! set carrying a denied context category. The empty deny set reaches
//! the envelope construction instead: the wiring cannot turn a clean
//! composition into a refusal, and a denied symbol cannot reach a built
//! envelope through the composed path.

use std::sync::Arc;

use gauntlet_app::compare::authority::{AdmissionError, ComparisonAuthority};
use gauntlet_app::compare::contract::ComparisonContract;
use gauntlet_app::roles::{ContextCategory, ContextDenySet};
use gauntlet_domain::EvidenceKind;
use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, IterationNo, Limits, MicroUsd, ReferenceId,
    RunId,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::{EvidenceStore, EvidenceStoreError};
use gauntlet_ports::state_store::{
    Checkpoint, ObservationBinding, RunEvent, StateStore, StateStoreError,
};

// ---------------------------------------------------------------------------
// The fixture: one durable run, one bound observation, one evidence store
// serving the frozen reference and the bound bundle.
// ---------------------------------------------------------------------------

const KEY_TEXT: &str = "## Units\n\n### AK-001\nclaim: the composed chain refuses denied context\nverify_by: test\ntier: contained\nauthority: write\nevidence_required: test report at reports/deny.txt\n";

fn bundle_text() -> String {
    use gauntlet_app::observation::bundle::{BundleContent, ClaimsFacet, ObservationBundleDoc};
    use gauntlet_app::observation::digest::observation_digest;
    let content = BundleContent {
        run_id: RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("the fixture run id"),
        iteration_no: IterationNo::new(1).expect("iteration one"),
        observed_at: FIXTURE_EPOCH_BASE,
        project_revision: "deny-fixture-revision".to_owned(),
        environment_fingerprint: gauntlet_app::config::fingerprint::digest_canonical(
            "deny-fixture-env",
        )
        .to_string(),
        observation_digest: gauntlet_app::config::fingerprint::digest_canonical("placeholder"),
        facets: Vec::new(),
        claims_facet: ClaimsFacet::new("the deny fixture's claims".to_owned()),
        unavailable_evidence: Vec::new(),
        observation_failures: Vec::new(),
    };
    let draft = ObservationBundleDoc::new(content);
    let digest = observation_digest(&draft);
    let mut content = draft.content();
    content.observation_digest = digest;
    ObservationBundleDoc::new(content).to_json()
}

fn digest_of(text: &str) -> Digest {
    gauntlet_app::config::fingerprint::digest_canonical(text)
}

/// The evidence store: two refs, both served from the fixture text.
/// `get` resolves either the frozen reference or the bound bundle; a
/// digest check downstream does the rest.
#[derive(Default)]
struct TwoRefStore;

impl EvidenceStore for TwoRefStore {
    fn put(
        &self,
        _bytes: &[u8],
        _kind: EvidenceKind,
        _deadline: Deadline,
    ) -> Result<gauntlet_domain::EvidenceRef, EvidenceStoreError> {
        Err(EvidenceStoreError::WriteFailed {
            detail: "the deny fixture never writes".to_owned(),
        })
    }

    fn pin(
        &self,
        _run_id: &gauntlet_domain::RunId,
        _evidence: &gauntlet_domain::EvidenceRef,
        _deadline: Deadline,
    ) -> Result<(), EvidenceStoreError> {
        Ok(())
    }

    fn get(
        &self,
        evidence: &gauntlet_domain::EvidenceRef,
        _deadline: Deadline,
    ) -> Result<Option<Vec<u8>>, EvidenceStoreError> {
        let text = match evidence.as_str() {
            "evd-deny-frozen-key" => KEY_TEXT.to_owned(),
            "evd-deny-bundle" => bundle_text(),
            other => panic!("the fixture serves two refs, got {other}"),
        };
        Ok(Some(text.into_bytes()))
    }
}

/// The scripted store: exactly the two reads the governed open performs.
#[derive(Default)]
struct DenyStore {
    run: Option<gauntlet_domain::Run>,
    binding: Option<ObservationBinding>,
}

impl StateStore for DenyStore {
    fn append_event(
        &self,
        _event: RunEvent,
        _deadline: Deadline,
    ) -> Result<gauntlet_domain::EventLogOffset, StateStoreError> {
        Err(StateStoreError::WriteFailed {
            detail: "the deny fixture never appends".to_owned(),
        })
    }

    fn load_run(
        &self,
        _run_id: &gauntlet_domain::RunId,
        _deadline: Deadline,
    ) -> Result<Option<gauntlet_domain::Run>, StateStoreError> {
        Ok(self.run.clone())
    }

    fn checkpoint(
        &self,
        _checkpoint: Checkpoint,
        _deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        Err(StateStoreError::WriteFailed {
            detail: "the deny fixture never checkpoints".to_owned(),
        })
    }

    fn resume(
        &self,
        _run_id: &gauntlet_domain::RunId,
        _deadline: Deadline,
    ) -> Result<Option<Checkpoint>, StateStoreError> {
        Ok(None)
    }

    fn export_jsonl(
        &self,
        _run_id: &gauntlet_domain::RunId,
        _deadline: Deadline,
    ) -> Result<String, StateStoreError> {
        Ok(String::new())
    }

    fn observation_binding(
        &self,
        _run_id: &gauntlet_domain::RunId,
        _iteration_no: IterationNo,
        _deadline: Deadline,
    ) -> Result<Option<ObservationBinding>, StateStoreError> {
        Ok(self.binding.clone())
    }
}

// The `let` + named return is deliberate (the literals scanner joins a
// `;`-less tail with following lines; doctor_bound carries the same allow).
#[allow(clippy::let_and_return)]
fn fixture_run() -> gauntlet_domain::Run {
    let reference_version = gauntlet_domain::ReferenceVersion::with_evidence(
        ReferenceId::new("the-deny-fixture-reference").expect("the fixture reference id"),
        1,
        digest_of(KEY_TEXT),
        gauntlet_domain::EvidenceRef::new("evd-deny-frozen-key").expect("the fixture ref"),
    );
    let limits = Limits::new(2, 1, 1, 60, 10_000, MicroUsd::new(1_000))
        .expect("the fixture limits are usable");
    let budget = Budget::new(limits, 15, DiagnosticAllowance::new(100, MicroUsd::new(50)))
        .expect("the fixture budget is usable");
    let run = gauntlet_domain::Run::new(gauntlet_domain::RunDefinition {
        id: RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("the fixture run id"),
        created_at: EpochSeconds::new(FIXTURE_EPOCH_BASE),
        reference_version,
        intent: "the deny wiring teeth".to_owned(),
        config_fingerprint: digest_of("config"),
        environment_fingerprint: digest_of("environment"),
        budget,
        parent_run: None,
    })
    .expect("the fixture run is well formed");
    run
}

#[allow(clippy::let_and_return)] // semicolon breaks the scanner's declaration join
fn fixture_bundle() -> gauntlet_ports::state_store::ObservationBinding {
    let binding = ObservationBinding::new(
        gauntlet_domain::EvidenceRef::new("evd-deny-bundle").expect("the fixture bundle ref"),
        digest_of(&bundle_text()),
    );
    binding
}

#[allow(clippy::let_and_return)] // semicolon breaks the scanner's declaration join
fn contract() -> ComparisonContract {
    let contract = ComparisonContract::parse(
        "version: 1\ndimensions:\n  - functional behavior\n\
     severity_rubric:\n  - blocking | invalidates the iteration\n  - major | real, not \
     invalidating\n  - minor | small\n  - informational | recorded only\ncomparator_receives:\n  \
     - the allowlist members\nresidual_definition:\n  - a difference\n",
    )
    .expect("the minimal contract parses");
    contract
}

/// The fixture's base instant, isolated on its own line.
const FIXTURE_EPOCH_BASE: u64 = 1_700_000_000;

/// The fixture's bound reach, isolated on its own line.
const FIXTURE_LATER: u64 = 60;

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(FIXTURE_EPOCH_BASE + FIXTURE_LATER))
}

fn evidence() -> Arc<dyn EvidenceStore> {
    Arc::new(TwoRefStore)
}

fn authority_with(deny: ContextDenySet) -> ComparisonAuthority {
    ComparisonAuthority::new(
        Arc::new(DenyStore {
            run: Some(fixture_run()),
            binding: Some(fixture_bundle()),
        }),
        evidence(),
    )
    .with_role_deny(deny)
}

// ---------------------------------------------------------------------------
// The teeth
// ---------------------------------------------------------------------------

/// The composed chain REFUSES before building the envelope when the wired
/// deny set names a category the input set carries — `reference` is
/// carried BY CONSTRUCTION (no envelope exists without it).
#[test]
fn a_wired_deny_set_refuses_the_envelope_before_it_is_built() {
    let deny = ContextDenySet::new(vec![ContextCategory::Reference]);
    let governed = authority_with(deny)
        .for_run(
            &RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("the fixture run id"),
            IterationNo::new(1).expect("iteration one"),
            bound(),
        )
        .expect("the governed chain opens over the durable fixture run");
    let refusal = match governed.issue(&contract(), Vec::new(), Vec::new(), bound()) {
        Err(refusal) => refusal,
        Ok(_) => panic!("a denied, always-carried category refuses the envelope"),
    };
    match refusal {
        AdmissionError::EnvelopeRefused(
            gauntlet_app::compare::EnvelopeError::DeniedContextPresent { category },
        ) => assert_eq!(
            category, "reference",
            "the refusal names the denied category"
        ),
        other => panic!("expected the deny refusal, got {other:?}"),
    }
}

/// The EMPTY deny set (deny nothing) reaches the envelope construction —
/// the wiring adds a gate, not a refusal: the same fixture without a
/// denied category fails LATER (at the trusted dispatch over this
/// minimal fixture's contract view), never with DeniedContextPresent.
#[test]
fn an_empty_deny_set_reaches_the_envelope_construction() {
    let governed = authority_with(ContextDenySet::new(Vec::new()))
        .for_run(
            &RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("the fixture run id"),
            IterationNo::new(1).expect("iteration one"),
            bound(),
        )
        .expect("the governed chain opens over the durable fixture run");
    let outcome = governed.issue(&contract(), Vec::new(), Vec::new(), bound());
    match outcome {
        Ok(_) => {}
        Err(AdmissionError::EnvelopeRefused(
            gauntlet_app::compare::EnvelopeError::DeniedContextPresent { .. },
        )) => panic!("the empty deny set must not refuse"),
        Err(_) => {}
    }
}
