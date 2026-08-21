//! The `Comparator` shared contract suite (spec §6.2, invariant I4 and
//! §5.3.6 coupling; plan Task 10 widened-envelope cases).
//!
//! Pinned semantics: the record a comparator returns MUST carry the
//! envelope's own digests — the reference digest it compared against, the
//! observation digest, and the digest of the serialized envelope itself
//! (spec §13.2: "the digest is stored on both the RoleInvocation and the
//! ComparisonRecord") — so the I1 cross-check stays possible at every
//! later boundary. An envelope carrying a comparison-contract version the
//! adapter does not know is a typed [`ComparatorError::EnvelopeInvalid`]
//! refusal, never a guess. And the envelope's facet-level observation
//! reality is VISIBLE to the comparator: a failing instrument facet rides
//! in the canonical bytes, and a comparison over it is never a clean pass
//! — the P3 comparison-consumption coupling (an instrument-settled failure
//! is not re-decided into nothing, spec §13.3).
//!
//! The suite EARNS its dispatch capability exactly the way production does:
//! it stands up an in-suite durable run with a bound observation bundle,
//! opens a [`RunComparisonAuthority`] against it, loads the frozen
//! projection from its evidence, and mints the context through
//! [`TrustedComparisonContext::issue`]. There is no fixture constructor —
//! the suite is a public entry point in this crate, so a fabricating one
//! would lend forged comparison authority to every crate that depends on
//! `gauntlet-ports`. Issuing here means the suite's own envelopes carry a
//! verified reference digest, a projected (never composed) observation, and
//! the blindness assertion, like everyone else's.

use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, EventLogOffset, EvidenceKind, EvidenceRef,
    IterationNo, Limits, MicroUsd, ReferenceId, ReferenceVersion, Run, RunDefinition, RunId,
    Severity,
};
use sha2::{Digest as _, Sha256};

use crate::comparator::{
    Comparator, ComparatorError, ContractView, FrozenReferenceProjection, RubricEntry,
    RunComparisonAuthority, TrustedComparisonContext,
};
use crate::contract::ContractFinding;
use crate::deadline::Deadline;
use crate::evidence_store::{EvidenceStore, EvidenceStoreError};
use crate::observer::FacetKind;
use crate::state_store::{Checkpoint, ObservationBinding, RunEvent, StateStore, StateStoreError};

/// The suite's name in findings.
pub const SUITE: &str = "comparator";

const BASE_INSTANT: u64 = 1_700_000_000;
/// The suite's bound span, on its own line (numeric literals never share a
/// line with a duration-shaped identifier).
const BOUND_SPAN: u64 = 60;

const FIXTURE_RUN: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const FIXTURE_ITERATION: u32 = 3;
/// The frozen reference bytes the suite's run is pinned to. Their digest is
/// computed from these bytes at fixture time, so the pinned digest and the
/// stored evidence cannot drift apart into a fixture that would not survive
/// [`FrozenReferenceProjection::load`].
const FIXTURE_REFERENCE_VIEW: &str = "the runtime-rendered view of the frozen standard\n";
const FIXTURE_REFERENCE_EVIDENCE: &str = "evd-contract-fixture-reference";
const FIXTURE_BUNDLE_EVIDENCE: &str = "evd-suite-bundle-0001";
const FIXTURE_FAILING_EVIDENCE: &str = "evd-suite-failing-0001";
const FIXTURE_CONFIG_DIGEST: &str = "a1b2c3d4e5f6a7b8";
const FIXTURE_ENVIRONMENT_DIGEST: &str = "e5f6a7b8c4d3e2f1";

/// Constructs the comparator under test. `build` must return an ISOLATED
/// instance: cases assume a clean slate.
pub trait ComparatorFactory {
    fn label(&self) -> &'static str;

    /// The comparison-contract version this adapter claims to run under
    /// (Annex B.4 `comparison_contract_version`) — the value the record
    /// path runs with.
    fn supported_version(&self) -> String;

    fn build(&self) -> Result<Box<dyn Comparator>, String>;
}

fn finding(case: &'static str, rule: &'static str, detail: String) -> ContractFinding {
    ContractFinding::new(SUITE, case, rule, detail)
}

fn bound() -> Deadline {
    Deadline::at_epoch(EpochSeconds::new(BASE_INSTANT + BOUND_SPAN))
}

/// The suite's rubric: all four severities with pinned definitions.
fn rubric() -> Vec<RubricEntry> {
    vec![
        RubricEntry::new(
            Severity::Blocking,
            "a difference that invalidates the iteration".to_owned(),
        ),
        RubricEntry::new(
            Severity::Major,
            "a real difference, not iteration-invalidating".to_owned(),
        ),
        RubricEntry::new(Severity::Minor, "a small difference".to_owned()),
        RubricEntry::new(
            Severity::Informational,
            "worth recording, not worth correcting on its own".to_owned(),
        ),
    ]
}

/// The suite's durable fixture: one run pinned to the frozen reference
/// bytes, one iteration with a DURABLY BOUND observation bundle, and the
/// evidence both live in. It implements the two ports the comparison chain
/// reads, so the suite earns its authority instead of asserting it.
struct FixtureStores {
    run: Run,
    bundle: String,
}

impl FixtureStores {
    /// When `failing_facet` is set, the bound bundle carries one instrument
    /// facet whose exit status is a recorded failure with a distinct
    /// evidence ref — the reality a comparator must be able to see (P3's
    /// comparison half). The facet reaches the envelope by PROJECTION from
    /// these bytes, never by composition.
    fn new(failing_facet: bool) -> Result<Self, String> {
        let reference = ReferenceVersion::with_evidence(
            ReferenceId::new("ref-contract-fixture").map_err(|error| error.to_string())?,
            1,
            reference_digest(),
            EvidenceRef::new(FIXTURE_REFERENCE_EVIDENCE).map_err(|error| error.to_string())?,
        );
        let run = Run::new(RunDefinition {
            id: RunId::new(FIXTURE_RUN).map_err(|error| error.to_string())?,
            created_at: EpochSeconds::new(BASE_INSTANT),
            reference_version: reference,
            intent: "prove the slice against the frozen standard".to_owned(),
            config_fingerprint: Digest::new("sha256", FIXTURE_CONFIG_DIGEST)
                .map_err(|error| error.to_string())?,
            environment_fingerprint: Digest::new("sha256", FIXTURE_ENVIRONMENT_DIGEST)
                .map_err(|error| error.to_string())?,
            budget: Budget::new(
                Limits::new(8, 2, 2, 3600, 1000, MicroUsd::new(500))
                    .map_err(|error| error.to_string())?,
                20,
                DiagnosticAllowance::new(100, MicroUsd::new(50)),
            )
            .map_err(|error| error.to_string())?,
            parent_run: None,
        })
        .map_err(|error| error.to_string())?;
        Ok(Self {
            run,
            bundle: bundle_json(failing_facet),
        })
    }

    fn bundle_ref(&self) -> EvidenceRef {
        EvidenceRef::new(FIXTURE_BUNDLE_EVIDENCE).expect("the fixture bundle ref is well-formed")
    }

    fn run_id(&self) -> &RunId {
        self.run.id()
    }

    fn iteration(&self) -> IterationNo {
        IterationNo::new(FIXTURE_ITERATION).expect("the fixture iteration is 1-based")
    }
}

/// The digest of the fixture's frozen reference bytes, under the same
/// normalization [`FrozenReferenceProjection::load`] applies.
fn reference_digest() -> Digest {
    Digest::new(
        "sha256",
        &Sha256::digest(FIXTURE_REFERENCE_VIEW.as_bytes())
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>(),
    )
    .expect("sha-256 output is a valid digest")
}

/// The §5.3.6 observation-bundle wire document the fixture binds. The claims
/// facet is absent by construction (Task 9's structural exclusion).
fn bundle_json(failing_facet: bool) -> String {
    let facets = if failing_facet {
        format!(
            "{{\"kind\":\"{}\",\"instrument\":{{\"name\":\"suite-probe\",\
             \"version\":\"1.2.3\"}},\"command\":\"re-run the declared self-check\",\
             \"exit_status\":1,\"evidence_ref\":\"{FIXTURE_FAILING_EVIDENCE}\"}}",
            FacetKind::Test.token()
        )
    } else {
        String::new()
    };
    // The declared digest must be what these bytes hash to (§12.4): the
    // comparison port re-derives it rather than believing the document.
    let body = |digest: &str| {
        format!(
            "{{\"schema\":\"observation_bundle@v1\",\"run_id\":\"{FIXTURE_RUN}\",\
             \"iteration_no\":{FIXTURE_ITERATION},\
             \"observation_digest\":\"{digest}\",\"facets\":[{facets}],\
             \"unavailable_evidence\":[],\"observation_failures\":[]}}"
        )
    };
    let probe = crate::json::parse(&body("")).expect("the fixture bundle is JSON");
    body(&crate::observation_digest::observation_digest(&probe).to_string())
}

impl EvidenceStore for FixtureStores {
    fn put(
        &self,
        _bytes: &[u8],
        _kind: EvidenceKind,
        _deadline: Deadline,
    ) -> Result<EvidenceRef, EvidenceStoreError> {
        Err(EvidenceStoreError::WriteFailed {
            detail: "the comparator suite's fixture evidence is fixed".to_owned(),
        })
    }

    fn get(
        &self,
        reference: &EvidenceRef,
        _deadline: Deadline,
    ) -> Result<Option<Vec<u8>>, EvidenceStoreError> {
        if reference.as_str() == FIXTURE_REFERENCE_EVIDENCE {
            return Ok(Some(FIXTURE_REFERENCE_VIEW.as_bytes().to_vec()));
        }
        if reference.as_str() == FIXTURE_BUNDLE_EVIDENCE {
            return Ok(Some(self.bundle.as_bytes().to_vec()));
        }
        Ok(None)
    }

    fn pin(
        &self,
        _run_id: &RunId,
        _reference: &EvidenceRef,
        _deadline: Deadline,
    ) -> Result<(), EvidenceStoreError> {
        Ok(())
    }
}

impl StateStore for FixtureStores {
    fn append_event(
        &self,
        _event: RunEvent,
        _deadline: Deadline,
    ) -> Result<EventLogOffset, StateStoreError> {
        Err(StateStoreError::WriteFailed {
            detail: "the comparator suite's fixture state is read-only".to_owned(),
        })
    }

    fn load_run(
        &self,
        run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Option<Run>, StateStoreError> {
        if run_id == self.run_id() {
            return Ok(Some(self.run.clone()));
        }
        Ok(None)
    }

    fn checkpoint(
        &self,
        _checkpoint: Checkpoint,
        _deadline: Deadline,
    ) -> Result<(), StateStoreError> {
        Err(StateStoreError::WriteFailed {
            detail: "the comparator suite's fixture state is read-only".to_owned(),
        })
    }

    fn resume(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<Option<Checkpoint>, StateStoreError> {
        Ok(None)
    }

    fn observation_binding(
        &self,
        run_id: &RunId,
        iteration_no: IterationNo,
        _deadline: Deadline,
    ) -> Result<Option<ObservationBinding>, StateStoreError> {
        if run_id == self.run_id() && iteration_no == self.iteration() {
            let document = crate::json::parse(&self.bundle).expect("the fixture bundle is JSON");
            return Ok(Some(ObservationBinding::new(
                self.bundle_ref(),
                crate::observation_digest::observation_digest(&document),
            )));
        }
        Ok(None)
    }

    fn export_jsonl(
        &self,
        _run_id: &RunId,
        _deadline: Deadline,
    ) -> Result<String, StateStoreError> {
        Ok(String::new())
    }
}

/// Earns a dispatch capability the production way: durable run → durable
/// observation binding → verified frozen projection → issued context. The
/// contract version is a parameter because that is the one envelope member
/// a case legitimately varies; run, iteration, reference and observation are
/// not parameters anywhere in this chain.
fn issued_context(
    version: String,
    failing_facet: bool,
) -> Result<TrustedComparisonContext, String> {
    let stores = FixtureStores::new(failing_facet)?;
    let authority =
        RunComparisonAuthority::open(&stores, stores.run_id(), stores.iteration(), bound())
            .map_err(|error| error.to_string())?;
    let projection = FrozenReferenceProjection::load(&authority, &stores, bound())
        .map_err(|error| error.to_string())?;
    TrustedComparisonContext::issue(
        &authority,
        &projection,
        &stores,
        ContractView::new(
            version,
            vec![
                "functional behavior".to_owned(),
                "evidence completeness".to_owned(),
            ],
            rubric(),
        ),
        Vec::new(),
        Vec::new(),
        bound(),
    )
    .map_err(|error| error.to_string())
}

/// Case: the returned record couples all three of the envelope's digests
/// (spec §5.3.6 + §13.2: the record carries `reference_digest`,
/// `observation_digest`, and the serialized envelope's digest; the I1
/// cross-check is impossible at later boundaries without them).
fn record_digests_couple_the_envelope(factory: &dyn ComparatorFactory) -> Vec<ContractFinding> {
    const CASE: &str = "record_digests_couple_the_envelope";
    const RULE: &str = "spec 5.3.6 + 13.2: the comparison record MUST carry the envelope's reference_digest, observation_digest, and envelope_digest";
    let Ok(comparator) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a comparator for the case".to_owned(),
        )];
    };
    let context = match issued_context(factory.supported_version(), false) {
        Ok(context) => context,
        Err(detail) => {
            return vec![finding(
                CASE,
                RULE,
                format!("the suite could not earn a dispatch capability: {detail}"),
            )];
        }
    };
    let observation_digest = context
        .envelope()
        .observation()
        .observation_digest()
        .clone();
    let reference_digest = context.envelope().reference_digest().clone();
    let envelope_digest = context.envelope().envelope_digest().clone();
    let record = match context.compare_with(comparator.as_ref(), bound()) {
        Ok(record) => record,
        Err(error) => {
            return vec![finding(
                CASE,
                RULE,
                format!("comparing the canonical envelope failed: {error}"),
            )];
        }
    };
    let mut findings = Vec::new();
    if record.reference_digest() != &reference_digest {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "the record carries reference_digest {} but the envelope's is {}",
                record.reference_digest(),
                reference_digest
            ),
        ));
    }
    if record.observation_digest() != &observation_digest {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "the record carries observation_digest {} but the envelope's is {}",
                record.observation_digest(),
                observation_digest
            ),
        ));
    }
    if record.envelope_digest() != &envelope_digest {
        findings.push(finding(
            CASE,
            RULE,
            format!(
                "the record carries envelope_digest {} but the envelope's is {}",
                record.envelope_digest(),
                envelope_digest
            ),
        ));
    }
    findings
}

/// Case: an envelope under a contract version the adapter does not know is
/// a typed [`ComparatorError::EnvelopeInvalid`], never a comparison run
/// under a guessed contract.
fn unknown_contract_versions_are_rejected(factory: &dyn ComparatorFactory) -> Vec<ContractFinding> {
    const CASE: &str = "unknown_contract_versions_are_rejected";
    const RULE: &str = "spec 6.2 + port contract: a contract version the adapter does not know is EnvelopeInvalid, never a guessed run";
    let Ok(comparator) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a comparator for the case".to_owned(),
        )];
    };
    // The contract version is issued INTO the envelope, so the digest labels
    // the bytes the adapter actually receives: the case reaches the version
    // refusal rather than exercising the separate stale-envelope defense.
    let context = match issued_context("suite-version-not-known-9".to_owned(), false) {
        Ok(context) => context,
        Err(detail) => {
            return vec![finding(
                CASE,
                RULE,
                format!("the suite could not earn a dispatch capability: {detail}"),
            )];
        }
    };
    match context.compare_with(comparator.as_ref(), bound()) {
        Err(ComparatorError::EnvelopeInvalid { .. }) => Vec::new(),
        Err(other) => vec![finding(
            CASE,
            RULE,
            format!(
                "an unknown contract version was refused with {other} instead of EnvelopeInvalid"
            ),
        )],
        Ok(_) => vec![finding(
            CASE,
            RULE,
            "an unknown contract version was compared instead of refused".to_owned(),
        )],
    }
}

/// Case: facet-level observation reality is visible to the comparator
/// (plan Task 10; the P3 comparison-consumption coupling). The canonical
/// envelope carries one instrument facet with a RECORDED FAILING exit
/// status and a distinct evidence ref; the canonical bytes must contain
/// both (what the comparator receives includes the reality), and a
/// comparison over that envelope is never a clean pass — spec §12.3/§21.3:
/// recorded instrument failure is never silently dropped, never a `PASS`.
fn facet_reality_is_visible_to_the_comparator(
    factory: &dyn ComparatorFactory,
) -> Vec<ContractFinding> {
    const CASE: &str = "facet_reality_is_visible_to_the_comparator";
    const RULE: &str = "spec 12.3 + 13.2: the envelope carries the failing instrument facet in its canonical bytes, and a comparison over it is never a clean pass";
    let Ok(comparator) = factory.build() else {
        return vec![finding(
            CASE,
            RULE,
            "the factory could not build a comparator for the case".to_owned(),
        )];
    };
    let context = match issued_context(factory.supported_version(), true) {
        Ok(context) => context,
        Err(detail) => {
            return vec![finding(
                CASE,
                RULE,
                format!("the suite could not earn a dispatch capability: {detail}"),
            )];
        }
    };
    let bytes = context.envelope().to_json();
    let mut findings = Vec::new();
    if !bytes.contains("\"exit_status\":1") {
        findings.push(finding(
            CASE,
            RULE,
            "the canonical envelope bytes do not carry the failing facet's exit status".to_owned(),
        ));
    }
    if !bytes.contains(FIXTURE_FAILING_EVIDENCE) {
        findings.push(finding(
            CASE,
            RULE,
            "the canonical envelope bytes do not carry the failing facet's evidence ref".to_owned(),
        ));
    }
    match context.compare_with(comparator.as_ref(), bound()) {
        Ok(record) => {
            let clean_pass =
                record.verdict() == gauntlet_domain::Verdict::Pass && record.residuals().is_empty();
            if clean_pass {
                findings.push(finding(
                    CASE,
                    RULE,
                    "a comparison over an envelope carrying a recorded instrument failure \
                     returned a clean pass — the facet reality was not consumed"
                        .to_owned(),
                ));
            }
        }
        Err(error) => {
            findings.push(finding(
                CASE,
                RULE,
                format!("comparing the facet-carrying envelope failed: {error}"),
            ));
        }
    }
    findings
}

/// Runs the full `Comparator` contract suite against one factory.
pub fn comparator_suite(factory: &dyn ComparatorFactory) -> Vec<ContractFinding> {
    let mut findings = Vec::new();
    findings.extend(record_digests_couple_the_envelope(factory));
    findings.extend(unknown_contract_versions_are_rejected(factory));
    findings.extend(facet_reality_is_visible_to_the_comparator(factory));
    findings
}
