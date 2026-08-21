//! Comparator adapter (spec §4.1, §13; plan Task 10): deterministic diff
//! comparison emitting `comparison_record@v1`. No model in this path — it
//! is the instrument fallback for residuals already settled conclusively
//! by bytes (spec §13.3 adaptive step 1: "Run cheap deterministic probes").
//!
//! Derivation rules, under the repository's frozen comparison contract
//! (`.gauntlet/comparison-contract.yaml`, spec §9.5):
//!
//! - an instrument facet recording a NON-ZERO exit status on an
//!   INSTANTIATED dimension is a residual at the rubric's blocking
//!   severity ("any instrument failure on an instantiated dimension is
//!   blocking");
//! - every recorded gap (`unavailable_evidence`, `observation_failures`)
//!   on an instantiated dimension is a residual at the rubric's major
//!   severity, and a gap-bearing comparison is `inconclusive`, never a
//!   pass (spec §12.3, §21.3, I8);
//! - a facet or gap whose dimension the contract does NOT instantiate
//!   produces no residual: applicability is the frozen contract's
//!   declaration, and changing that declaration mid-run is ruler movement
//!   (I1) — the freeze is the guard, not this adapter.
//!
//! Verdict: any blocking residual ⇒ `fail`; else any gap ⇒ `inconclusive`;
//! else `pass` with zero residuals (instruments found no difference —
//! which settles nothing about conformance; that is exactly why §13.3's
//! adaptive mode fans undecided dimensions out to model critics).
//!
//! The kind→dimension mapping below is this adapter's documented
//! deterministic policy over the `FacetKind` vocabulary — every kind maps,
//! so no instrument failure vanishes for want of a dimension:
//!
//! | facet kind | dimension |
//! |---|---|
//! | `test`, `build`, `browser`, `http` | functional behavior |
//! | `bench` | performance |
//! | `scanner` | security |
//! | `filesystem`, `versioning` | evidence completeness |
//!
//! No model, no clock dependence, no randomness: the same envelope always
//! yields byte-identical output. Unknown comparison-contract versions are
//! refused ([`ComparatorError::EnvelopeInvalid`]) — never a guessed run.

use gauntlet_domain::{ComparisonRecord, EvidenceRef, Residual, ResidualId, Severity, Verdict};
use gauntlet_ports::comparator::{
    Comparator, ComparatorError, ComparisonEnvelope, TrustedComparisonContext,
    verify_envelope_digest,
};
use gauntlet_ports::comparison_record::{
    ComparatorIdentity, ComparisonRecordDoc, RecordContent, RecordVerdict, ResidualDoc,
};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::observer::FacetKind;

/// The comparison-contract version this adapter runs under (Annex B.4
/// `comparison_contract_version`; must equal the frozen artifact's).
pub const SUPPORTED_CONTRACT_VERSION: &str = "1";

/// This adapter's own identity (AK-028-class provenance: the record names
/// its producer).
pub const ADAPTER_IDENTITY: &str = "gauntlet-adapter-compare-diff";

/// The documented kind→dimension policy (see the module doc table).
fn dimension_of(kind: FacetKind) -> &'static str {
    match kind {
        FacetKind::Test | FacetKind::Build | FacetKind::Browser | FacetKind::Http => {
            "functional behavior"
        }
        FacetKind::Bench => "performance",
        FacetKind::Scanner => "security",
        FacetKind::Filesystem | FacetKind::Versioning => "evidence completeness",
    }
}

/// Deterministic residual ids: FNV-1a over the merge key, hex — a pure
/// function of the finding, so re-comparisons yield stable ids.
fn residual_id(key: &str) -> ResidualId {
    ResidualId::new(&format!("diff-{:016x}", fnv_hex(key)))
        .expect("the id grammar holds by construction")
}

fn fnv_hex(key: &str) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in key.bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x100_0000_01b3);
    }
    hash
}

/// The deterministic diff comparator. Constructed either across all
/// dimensions or scoped to one (the review modes' routing question — see
/// `gauntlet-app` `compare::review`).
#[derive(Debug, Clone)]
pub struct DiffComparator {
    dimension: Option<String>,
}

impl DiffComparator {
    /// A comparator judging across all instantiated dimensions.
    pub fn across_dimensions() -> Self {
        Self { dimension: None }
    }

    /// A comparator scoped to one dimension (§13.3 fanout/adaptive).
    pub fn for_dimension(dimension: &str) -> Self {
        Self {
            dimension: Some(dimension.to_owned()),
        }
    }

    /// Emits the full `comparison_record@v1` document (Annex B.4) for the
    /// envelope — the wire form the domain record is the summary of.
    pub fn comparison_document(
        &self,
        context: &TrustedComparisonContext,
        deadline: Deadline,
    ) -> Result<ComparisonRecordDoc, ComparatorError> {
        let envelope = context.envelope();
        // Recompute at this adapter's consumption boundary: never derive a
        // record from bytes whose declared dispatch digest is stale.
        verify_envelope_digest(envelope)?;
        let derived = self.derive(envelope, deadline)?;
        Ok(ComparisonRecordDoc::new(RecordContent {
            run_id: envelope.run_id().clone(),
            iteration_no: envelope.iteration_no(),
            reference_digest: envelope.reference_digest().clone(),
            observation_digest: envelope.observation().observation_digest().clone(),
            envelope_digest: envelope.envelope_digest().clone(),
            contract_version: envelope.contract().version().to_owned(),
            identity: ComparatorIdentity::new(
                ADAPTER_IDENTITY.to_owned(),
                String::new(),
                self.dimension.clone().unwrap_or_default(),
            ),
            residuals: derived.residual_docs,
            hard_gate_results: Vec::new(),
            verdict: RecordVerdict::new(derived.verdict, derived.rationale),
        }))
    }

    fn derive(
        &self,
        envelope: &ComparisonEnvelope,
        _deadline: Deadline,
    ) -> Result<Derived, ComparatorError> {
        if envelope.contract().version() != SUPPORTED_CONTRACT_VERSION {
            return Err(ComparatorError::EnvelopeInvalid {
                detail: format!(
                    "contract version {} is not {} — never a guessed run",
                    envelope.contract().version(),
                    SUPPORTED_CONTRACT_VERSION
                ),
            });
        }
        let instantiated = |dimension: &str| {
            self.dimension
                .as_deref()
                .is_none_or(|scoped| scoped == dimension)
                && envelope
                    .contract()
                    .dimensions()
                    .iter()
                    .any(|d| d == dimension)
        };
        let mut residual_docs = Vec::new();
        let mut domain_residuals = Vec::new();
        let mut blocking = 0usize;
        let mut gaps = 0usize;

        for facet in envelope.observation().facets() {
            let dimension = dimension_of(facet.kind());
            if !instantiated(dimension) {
                continue;
            }
            let Some(status) = facet.exit_status() else {
                continue;
            };
            if status == 0 {
                continue;
            }
            let key = format!("{dimension}|instrument success|{}", facet.evidence_ref());
            let expected = format!(
                "the {} facet's instrument exits 0 on {dimension}",
                facet.kind().token()
            );
            let observed = format!("{} exited {status}", facet.evidence_ref());
            let command = facet.command().unwrap_or_default().to_owned();
            let evidence = EvidenceRef::new(facet.evidence_ref()).map_err(|source| {
                ComparatorError::ComparisonFailed {
                    detail: source.to_string(),
                }
            })?;
            let doc = ResidualDoc::new(
                residual_id(&key),
                dimension.to_owned(),
                Vec::new(),
                expected,
                observed,
                vec![evidence],
                Severity::Blocking,
                1.0,
                false,
                format!("command: {command}"),
                command,
                Vec::new(),
            );
            domain_residuals.push(domain_of(&doc));
            residual_docs.push(doc);
            blocking += 1;
        }

        for gap in envelope.observation().unavailable_evidence() {
            let dimension = dimension_of(gap.kind());
            if !instantiated(dimension) {
                continue;
            }
            let doc = ResidualDoc::new(
                residual_id(&format!(
                    "{dimension}|evidence available|unavailable:{}",
                    gap.kind().token()
                )),
                dimension.to_owned(),
                Vec::new(),
                format!("evidence for the {} probe is available", gap.kind().token()),
                format!("unavailable:{}", gap.kind().token()),
                Vec::new(),
                Severity::Major,
                1.0,
                false,
                gap.reason().to_owned(),
                String::new(),
                Vec::new(),
            );
            domain_residuals.push(domain_of(&doc));
            residual_docs.push(doc);
            gaps += 1;
        }
        for gap in envelope.observation().observation_failures() {
            let dimension = dimension_of(gap.kind());
            if !instantiated(dimension) {
                continue;
            }
            let doc = ResidualDoc::new(
                residual_id(&format!(
                    "{dimension}|instrument success|failed:{}",
                    gap.kind().token()
                )),
                dimension.to_owned(),
                Vec::new(),
                format!("the {} instrument succeeds", gap.kind().token()),
                format!("failed:{}", gap.kind().token()),
                Vec::new(),
                Severity::Major,
                1.0,
                false,
                format!("{}: {}", gap.error_class(), gap.detail()),
                String::new(),
                Vec::new(),
            );
            domain_residuals.push(domain_of(&doc));
            residual_docs.push(doc);
            gaps += 1;
        }

        let verdict = if blocking > 0 {
            Verdict::Fail
        } else if gaps > 0 {
            Verdict::Inconclusive
        } else {
            Verdict::Pass
        };
        let rationale = format!(
            "deterministic diff: {blocking} blocking instrument failure(s), {gaps} recorded \
             gap(s) across the {} instantiated dimension(s) in scope",
            envelope.contract().dimensions().len()
        );
        Ok(Derived {
            residual_docs,
            domain_residuals,
            verdict,
            rationale,
        })
    }
}

/// The domain residual is the wire residual's summary: same id, severity,
/// and dimension; the detail composes the expected/observed pair and the
/// diagnosis.
fn domain_of(doc: &ResidualDoc) -> Residual {
    Residual::new(
        doc.id().clone(),
        doc.severity(),
        format!(
            "expected {}; observed {}; {}",
            doc.expected(),
            doc.observed(),
            doc.diagnosis()
        ),
        Some(doc.dimension().to_owned()).filter(|dimension| !dimension.is_empty()),
    )
}

struct Derived {
    residual_docs: Vec<ResidualDoc>,
    domain_residuals: Vec<Residual>,
    verdict: Verdict,
    rationale: String,
}

impl Comparator for DiffComparator {
    fn compare(
        &self,
        context: &TrustedComparisonContext,
        deadline: Deadline,
    ) -> Result<ComparisonRecord, ComparatorError> {
        let envelope = context.envelope();
        // `compare` is a separate public consumption path from the B.4
        // document method; both must reject stale bytes.
        verify_envelope_digest(envelope)?;
        let derived = self.derive(envelope, deadline)?;
        ComparisonRecord::new(
            envelope.reference_digest().clone(),
            envelope.observation().observation_digest().clone(),
            envelope.envelope_digest().clone(),
            derived.domain_residuals,
            derived.verdict,
        )
        .map_err(|error| ComparatorError::ComparisonFailed {
            detail: error.to_string(),
        })
    }

    fn comparison_document(
        &self,
        context: &TrustedComparisonContext,
        deadline: Deadline,
    ) -> Result<Option<ComparisonRecordDoc>, ComparatorError> {
        DiffComparator::comparison_document(self, context, deadline).map(Some)
    }
}

#[cfg(test)]
mod tests {
    use super::{ADAPTER_IDENTITY, DiffComparator, SUPPORTED_CONTRACT_VERSION, fnv_hex};
    use gauntlet_domain::{
        Digest, EvidenceKind, EvidenceRef, IterationNo, ReferenceId, ReferenceVersion, RunId,
        Severity, Verdict,
    };
    use gauntlet_ports::comparator::{
        Comparator, ComparatorError, ContractView, FacetSummary, FailureGap,
        FrozenReferenceProjection, RunComparisonAuthority, TrustedComparisonContext,
        UnavailableGap,
    };
    use gauntlet_ports::comparison_record::ComparisonRecordDoc;
    use gauntlet_ports::deadline::Deadline;
    use gauntlet_ports::evidence_store::{EvidenceStore, EvidenceStoreError};
    use gauntlet_ports::observer::FacetKind;

    const BASE_INSTANT: u64 = 1_700_000_000;

    fn bound() -> Deadline {
        Deadline::at_epoch(gauntlet_domain::EpochSeconds::new(BASE_INSTANT + 60))
    }

    fn facet(kind: FacetKind, exit: Option<i64>, evidence: &str) -> FacetSummary {
        FacetSummary::new(
            kind,
            "suite-probe".to_owned(),
            "1.2.3".to_owned(),
            Some("re-run the declared self-check".to_owned()),
            exit,
            evidence.to_owned(),
        )
    }

    const FROZEN_VIEW: &str = "the rendered reference\n";

    /// SHA-256 of [`FROZEN_VIEW`]. Pinned rather than computed: this adapter
    /// crate deliberately carries no hashing dependency (the external-crate
    /// allowlist is an audited inventory), and `FrozenReferenceProjection::load`
    /// re-derives and checks the real digest anyway — a wrong constant here
    /// fails the fixture loudly.
    const FROZEN_VIEW_DIGEST: &str =
        "5605fa116f3e94cfb7e1406ce5cf64b0647a1896c00e34690640325fa5163565";

    fn fixed_digest(hex: &str) -> Digest {
        Digest::new("sha256", hex).expect("a pinned sha-256 hex digest")
    }

    /// The durable anchor. There is no public way to mint a
    /// `RunComparisonAuthority` without one — in this test module or anywhere.
    struct OneRunStore(gauntlet_domain::Run, String);
    impl gauntlet_ports::state_store::StateStore for OneRunStore {
        fn append_event(
            &self,
            _: gauntlet_ports::state_store::RunEvent,
            _: Deadline,
        ) -> Result<gauntlet_domain::EventLogOffset, gauntlet_ports::state_store::StateStoreError>
        {
            Ok(gauntlet_domain::EventLogOffset::new(0))
        }
        fn load_run(
            &self,
            run_id: &RunId,
            _: Deadline,
        ) -> Result<Option<gauntlet_domain::Run>, gauntlet_ports::state_store::StateStoreError>
        {
            Ok((run_id == self.0.id()).then(|| self.0.clone()))
        }
        fn observation_binding(
            &self,
            _: &RunId,
            _: gauntlet_domain::IterationNo,
            _: Deadline,
        ) -> Result<
            Option<gauntlet_ports::state_store::ObservationBinding>,
            gauntlet_ports::state_store::StateStoreError,
        > {
            let document =
                gauntlet_ports::json::parse(&self.1).expect("the fixture bundle is JSON");
            Ok(Some(gauntlet_ports::state_store::ObservationBinding::new(
                EvidenceRef::new(BUNDLE_REF).expect("the fixture bundle ref is valid"),
                gauntlet_ports::observation_digest::observation_digest(&document),
            )))
        }

        fn checkpoint(
            &self,
            _: gauntlet_ports::state_store::Checkpoint,
            _: Deadline,
        ) -> Result<(), gauntlet_ports::state_store::StateStoreError> {
            Ok(())
        }
        fn resume(
            &self,
            _: &RunId,
            _: Deadline,
        ) -> Result<
            Option<gauntlet_ports::state_store::Checkpoint>,
            gauntlet_ports::state_store::StateStoreError,
        > {
            Ok(None)
        }
        fn export_jsonl(
            &self,
            _: &RunId,
            _: Deadline,
        ) -> Result<String, gauntlet_ports::state_store::StateStoreError> {
            Ok(String::new())
        }
    }

    const BUNDLE_REF: &str = "evd-observation-bundle-0001";
    const REFERENCE_REF: &str = "evd-frozen-reference";

    /// Serves exactly the two pinned objects the governed chain reads: the
    /// frozen reference bytes and the DURABLY BOUND observation bundle.
    struct FixtureEvidence {
        reference: &'static str,
        bundle: String,
    }

    impl EvidenceStore for FixtureEvidence {
        fn put(
            &self,
            _: &[u8],
            _: EvidenceKind,
            _: Deadline,
        ) -> Result<EvidenceRef, EvidenceStoreError> {
            unreachable!("fixture only loads")
        }
        fn get(
            &self,
            reference: &EvidenceRef,
            _: Deadline,
        ) -> Result<Option<Vec<u8>>, EvidenceStoreError> {
            Ok(match reference.as_str() {
                REFERENCE_REF => Some(self.reference.as_bytes().to_vec()),
                BUNDLE_REF => Some(self.bundle.as_bytes().to_vec()),
                _ => None,
            })
        }
        fn pin(&self, _: &RunId, _: &EvidenceRef, _: Deadline) -> Result<(), EvidenceStoreError> {
            unreachable!("fixture only loads")
        }
    }

    fn quoted(text: &str) -> String {
        format!("\"{}\"", text.replace('\\', "\\\\").replace('"', "\\\""))
    }

    /// The §5.3.6 bundle wire document the fixture run binds. The envelope's
    /// observation view is projected from THESE bytes inside the port, so an
    /// adapter test cannot present a facet list the observation never had.
    fn bundle_json(
        run: &str,
        facets: &[FacetSummary],
        unavailable: &[UnavailableGap],
        failures: &[FailureGap],
    ) -> String {
        let facets: Vec<String> = facets
            .iter()
            .map(|facet| {
                format!(
                    "{{\"kind\":{},\"instrument\":{{\"name\":{},\"version\":{}}},\
                     \"command\":{},\"exit_status\":{},\"evidence_ref\":{}}}",
                    quoted(&facet.kind().token()),
                    quoted(facet.instrument_name()),
                    quoted(facet.instrument_version()),
                    facet
                        .command()
                        .map(quoted)
                        .unwrap_or_else(|| "null".to_owned()),
                    facet
                        .exit_status()
                        .map(|status| status.to_string())
                        .unwrap_or_else(|| "null".to_owned()),
                    quoted(facet.evidence_ref()),
                )
            })
            .collect();
        let unavailable: Vec<String> = unavailable
            .iter()
            .map(|gap| {
                format!(
                    "{{\"kind\":{},\"reason\":{}}}",
                    quoted(&gap.kind().token()),
                    quoted(gap.reason())
                )
            })
            .collect();
        let failures: Vec<String> = failures
            .iter()
            .map(|gap| {
                format!(
                    "{{\"kind\":{},\"error_class\":{},\"detail\":{}}}",
                    quoted(&gap.kind().token()),
                    quoted(gap.error_class()),
                    quoted(gap.detail())
                )
            })
            .collect();
        // The declared digest must be the one the bundle's own bytes yield
        // (§12.4), because the comparison port re-derives it. Build the body
        // with the self-referential member empty — exactly what the
        // normalization pins it to — then stamp in what that hashes to.
        let body = |digest: &str| {
            format!(
                "{{\"schema\":\"observation_bundle@v1\",\"run_id\":{},\"iteration_no\":2,\
                 \"observation_digest\":\"{digest}\",\"facets\":[{}],\
                 \"unavailable_evidence\":[{}],\"observation_failures\":[{}]}}",
                quoted(run),
                facets.join(","),
                unavailable.join(","),
                failures.join(",")
            )
        };
        let probe = gauntlet_ports::json::parse(&body("")).expect("the fixture bundle is JSON");
        body(&gauntlet_ports::observation_digest::observation_digest(&probe).to_string())
    }

    /// Records a run whose frozen reference IS `view` and whose iteration
    /// durably binds `bundle`, then EARNS the authority and loads the
    /// projection through the production calls.
    fn governed(
        run: &str,
        view: &'static str,
        digest_hex: &str,
        bundle: String,
    ) -> (
        RunComparisonAuthority,
        FrozenReferenceProjection,
        FixtureEvidence,
    ) {
        let run_id = RunId::new(run).expect("valid run id");
        let version = ReferenceVersion::with_evidence(
            ReferenceId::new("fixture-reference").expect("ref"),
            1,
            fixed_digest(digest_hex),
            EvidenceRef::new(REFERENCE_REF).expect("ref"),
        );
        let durable = gauntlet_domain::Run::new(gauntlet_domain::RunDefinition {
            id: run_id.clone(),
            created_at: gauntlet_domain::EpochSeconds::new(BASE_INSTANT),
            reference_version: version,
            intent: "diff comparator fixture".to_owned(),
            config_fingerprint: fixed_digest(&"ab".repeat(32)),
            environment_fingerprint: fixed_digest(&"cd".repeat(32)),
            budget: fixture_budget(),
            parent_run: None,
        })
        .expect("the fixture run is valid");
        let evidence = FixtureEvidence {
            reference: view,
            bundle,
        };
        let authority = RunComparisonAuthority::open(
            &OneRunStore(durable, evidence.bundle.clone()),
            &run_id,
            IterationNo::new(2).expect("iterations are 1-based"),
            bound(),
        )
        .expect("the durable run yields authority");
        let projection = FrozenReferenceProjection::load(&authority, &evidence, bound())
            .expect("the pinned frozen evidence projects");
        (authority, projection, evidence)
    }

    fn fixture_budget() -> gauntlet_domain::Budget {
        gauntlet_domain::Budget::new(
            gauntlet_domain::Limits::new(
                8,
                4,
                4,
                600,
                1_000_000,
                gauntlet_domain::MicroUsd::new(1_000_000),
            )
            .expect("non-zero limits"),
            10,
            gauntlet_domain::DiagnosticAllowance::new(1_000, gauntlet_domain::MicroUsd::new(1_000)),
        )
        .expect("the fixture budget is valid")
    }

    fn all_dimensions() -> Vec<String> {
        vec![
            "functional behavior".to_owned(),
            "performance".to_owned(),
            "security".to_owned(),
            "evidence completeness".to_owned(),
        ]
    }

    /// The whole governed fixture: durable run + durably bound observation ->
    /// authority -> projection -> the single minting path -> dispatch
    /// capability. There is no other way to obtain one, in a test or anywhere.
    struct Fixture {
        run: &'static str,
        view: &'static str,
        digest_hex: &'static str,
        /// The run the BOUND BUNDLE declares. Equal to `run` for honest
        /// fixtures; a different value is the provenance-substitution probe.
        bundle_run: &'static str,
        /// A digest the bundle DECLARES about itself that its bytes do not
        /// yield. `None` stamps the honest one.
        misdeclared_digest: Option<&'static str>,
        facets: Vec<FacetSummary>,
        unavailable: Vec<UnavailableGap>,
        failures: Vec<FailureGap>,
        version: String,
        dimensions: Vec<String>,
    }

    impl Default for Fixture {
        fn default() -> Self {
            Self {
                run: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
                view: FROZEN_VIEW,
                digest_hex: FROZEN_VIEW_DIGEST,
                bundle_run: "01ARZ3NDEKTSV4RRFFQ69G5FAV",
                misdeclared_digest: None,
                facets: Vec::new(),
                unavailable: Vec::new(),
                failures: Vec::new(),
                version: SUPPORTED_CONTRACT_VERSION.to_owned(),
                dimensions: all_dimensions(),
            }
        }
    }

    impl Fixture {
        fn issue(self) -> Result<TrustedComparisonContext, ComparatorError> {
            let honest = bundle_json(
                self.bundle_run,
                &self.facets,
                &self.unavailable,
                &self.failures,
            );
            // The durable binding always records the digest of the REAL bytes;
            // only what the document says about itself is varied, so the
            // self-consistency check is what a probe reaches.
            let (authority, projection, mut evidence) =
                governed(self.run, self.view, self.digest_hex, honest.clone());
            if let Some(lie) = self.misdeclared_digest {
                let truth = gauntlet_ports::observation_digest::observation_digest(
                    &gauntlet_ports::json::parse(&honest).expect("the fixture bundle is JSON"),
                )
                .to_string();
                evidence.bundle = honest.replace(&truth, lie);
            }
            TrustedComparisonContext::issue(
                &authority,
                &projection,
                &evidence,
                ContractView::new(self.version, self.dimensions, Vec::new()),
                Vec::new(),
                Vec::new(),
                bound(),
            )
        }
    }

    /// Tooth: an observation bundle's own `observation_digest` is re-derived
    /// from its bytes inside the port, never believed. Without this the digest
    /// that lands in the comparison record — and in durable state — would be
    /// whatever the bytes at the bound ref asserted about themselves, while the
    /// reference side of the same envelope has always been verified.
    #[test]
    fn a_bundle_that_misdeclares_its_own_observation_digest_is_refused() {
        let refusal = Fixture {
            misdeclared_digest: Some(
                "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee",
            ),
            ..Fixture::default()
        }
        .issue();
        let detail = match refusal {
            Err(ComparatorError::EnvelopeInvalid { detail }) => detail,
            other => panic!("a misdeclared observation digest must be refused, got {other:?}"),
        };
        assert!(
            detail.contains("own bytes digest to"),
            "the refusal states that the document's claim about itself was re-derived: {detail}"
        );
    }

    fn trusted(
        facets: Vec<FacetSummary>,
        unavailable: Vec<UnavailableGap>,
        failures: Vec<FailureGap>,
        version: &str,
    ) -> TrustedComparisonContext {
        Fixture {
            facets,
            unavailable,
            failures,
            version: version.to_owned(),
            ..Fixture::default()
        }
        .issue()
        .expect("the governed chain mints the fixture capability")
    }

    #[test]
    fn a_failing_facet_yields_a_blocking_residual_from_the_instrument() {
        // P3's comparison half: the residual is DERIVED from the instrument
        // failure (exit status + evidence ref), never from claims.
        let context = trusted(
            vec![facet(FacetKind::Test, Some(1), "evd-failing-0001")],
            Vec::new(),
            Vec::new(),
            SUPPORTED_CONTRACT_VERSION,
        );
        let record = DiffComparator::across_dimensions()
            .compare(&context, bound())
            .expect("the diff comparator runs");
        assert_eq!(record.verdict(), Verdict::Fail);
        assert_eq!(record.residuals().len(), 1);
        let residual = &record.residuals()[0];
        assert_eq!(residual.severity(), Severity::Blocking);
        assert_eq!(residual.dimension(), Some("functional behavior"));
        assert!(residual.detail().contains("evd-failing-0001"));
        assert!(residual.detail().contains("exited 1"));
        // All three digests couple.
        assert_eq!(
            record.reference_digest(),
            context.envelope().reference_digest()
        );
        assert_eq!(
            record.observation_digest(),
            context.envelope().observation().observation_digest()
        );
        assert_eq!(
            record.envelope_digest(),
            context.envelope().envelope_digest()
        );
        // The wire document is the same finding in Annex-B.4 form.
        let doc = DiffComparator::across_dimensions()
            .comparison_document(
                &trusted(
                    vec![facet(FacetKind::Test, Some(1), "evd-failing-0001")],
                    Vec::new(),
                    Vec::new(),
                    SUPPORTED_CONTRACT_VERSION,
                ),
                bound(),
            )
            .expect("the document emits");
        assert_eq!(doc.identity().runner(), ADAPTER_IDENTITY);
        assert_eq!(doc.identity().model(), "");
        assert_eq!(doc.residuals().len(), 1);
        assert_eq!(doc.residuals()[0].evidence_refs().len(), 1);
        assert!(doc.to_json().contains("evd-failing-0001"));
        assert_eq!(
            ComparisonRecordDoc::decode(&doc.to_json()).expect("strict round-trip"),
            doc
        );
    }

    #[test]
    fn an_observation_that_is_not_this_runs_cannot_be_compared() {
        // The attacker can no longer CHOOSE a reference view (assemble reads
        // it from the verified projection), nor an observation (the chain
        // reads the ref the RUN bound). What is left to try is a bundle that
        // is well formed, correctly pinned at the bound ref, and simply
        // belongs to another run. The projection refuses it — no deny list,
        // no string check on content.
        let refusal = Fixture {
            bundle_run: "01ARZ3NDEKTSV4RRFFQ69G5FAW",
            ..Fixture::default()
        }
        .issue();
        let detail = match refusal {
            Err(ComparatorError::EnvelopeInvalid { detail }) => detail,
            other => panic!("a foreign observation must be refused, got {other:?}"),
        };
        assert!(
            detail.contains("declares run 01ARZ3NDEKTSV4RRFFQ69G5FAW"),
            "the refusal names the provenance mismatch: {detail}"
        );
        // ... and the honest pairing still works, so the refusal is specific.
        assert!(Fixture::default().issue().is_ok());
    }

    #[test]
    fn gaps_forward_as_major_residuals_and_inconclusive_verdicts() {
        // I8: recorded gaps ride into the comparison; gap-bearing is
        // inconclusive, never a pass.
        let context = trusted(
            Vec::new(),
            vec![UnavailableGap::new(
                FacetKind::Browser,
                "the probe found nothing at this locator".to_owned(),
            )],
            vec![FailureGap::new(
                FacetKind::Test,
                "instrument_failed".to_owned(),
                "the instrument exited without a report".to_owned(),
            )],
            SUPPORTED_CONTRACT_VERSION,
        );
        let record = DiffComparator::across_dimensions()
            .compare(&context, bound())
            .expect("the diff comparator runs");
        assert_eq!(record.verdict(), Verdict::Inconclusive);
        assert_eq!(record.residuals().len(), 2);
        assert!(
            record
                .residuals()
                .iter()
                .all(|residual| residual.severity() == Severity::Major)
        );
    }

    #[test]
    fn clean_instruments_settle_nothing_and_scope_rules_hold() {
        // Clean: pass with zero residuals — the honest deterministic
        // verdict that adaptive review treats as "undecided".
        let clean = trusted(
            vec![facet(FacetKind::Test, Some(0), "evd-clean-0001")],
            Vec::new(),
            Vec::new(),
            SUPPORTED_CONTRACT_VERSION,
        );
        let record = DiffComparator::across_dimensions()
            .compare(&clean, bound())
            .expect("the diff comparator runs");
        assert_eq!(record.verdict(), Verdict::Pass);
        assert_eq!(record.residuals().len(), 0);
        // A facet whose dimension the contract does NOT instantiate is out
        // of comparison scope (the frozen contract declares applicability;
        // moving it mid-run is ruler movement, I1).
        let narrow = Fixture {
            facets: vec![facet(FacetKind::Test, Some(0), "evd-clean-0001")],
            dimensions: vec!["maintainability".to_owned()],
            ..Fixture::default()
        }
        .issue()
        .expect("fixture context");
        let record = DiffComparator::across_dimensions()
            .compare(&narrow, bound())
            .expect("the diff comparator runs");
        assert_eq!(record.residuals().len(), 0);
        // Scoped comparators see only their dimension.
        let scoped = DiffComparator::for_dimension("evidence completeness");
        let mixed = trusted(
            vec![
                facet(FacetKind::Test, Some(1), "evd-failing-0001"),
                facet(FacetKind::Filesystem, None, "evd-fs-0001"),
            ],
            Vec::new(),
            Vec::new(),
            SUPPORTED_CONTRACT_VERSION,
        );
        let record = scoped.compare(&mixed, bound()).expect("scoped run");
        assert_eq!(
            record.residuals().len(),
            0,
            "the failing test facet is out of the scoped dimension"
        );
    }

    #[test]
    fn unknown_contract_versions_are_refused_and_output_is_deterministic() {
        let unknown = trusted(
            vec![facet(FacetKind::Test, Some(1), "evd-failing-0001")],
            Vec::new(),
            Vec::new(),
            "suite-version-not-known-9",
        );
        assert!(matches!(
            DiffComparator::across_dimensions().compare(&unknown, bound()),
            Err(ComparatorError::EnvelopeInvalid { .. })
        ));
        // Determinism: the same envelope twice yields byte-identical
        // documents (no clock, no randomness), and residual ids are a
        // pure function of the merge key.
        let facets = || vec![facet(FacetKind::Test, Some(1), "evd-failing-0001")];
        let first = DiffComparator::across_dimensions()
            .comparison_document(
                &trusted(facets(), Vec::new(), Vec::new(), SUPPORTED_CONTRACT_VERSION),
                bound(),
            )
            .expect("first emission");
        let second = DiffComparator::across_dimensions()
            .comparison_document(
                &trusted(facets(), Vec::new(), Vec::new(), SUPPORTED_CONTRACT_VERSION),
                bound(),
            )
            .expect("second emission");
        assert_eq!(first.to_json(), second.to_json());
        assert_eq!(first.residuals()[0].id(), second.residuals()[0].id());
        let key = "functional behavior|instrument success|evd-failing-0001";
        assert_eq!(
            first.residuals()[0].id().as_str(),
            format!("diff-{:016x}", fnv_hex(key)),
            "stable ids are a pure function of the merge key"
        );
    }

    /// The adapter passes the UNMODIFIED shared comparator suite (spec
    /// §7.3) — the Task-4/8 factory pattern.
    #[test]
    fn the_diff_comparator_passes_the_unmodified_shared_contract_suite() {
        struct Factory;
        impl gauntlet_ports::contract::comparator::ComparatorFactory for Factory {
            fn label(&self) -> &'static str {
                "gauntlet-adapter-compare-diff"
            }

            fn supported_version(&self) -> String {
                SUPPORTED_CONTRACT_VERSION.to_owned()
            }

            fn build(&self) -> Result<Box<dyn Comparator>, String> {
                Ok(Box::new(DiffComparator::across_dimensions()))
            }
        }
        let findings = gauntlet_ports::contract::comparator::comparator_suite(&Factory);
        assert!(
            findings.is_empty(),
            "the shared comparator suite must pass: {findings:?}"
        );
    }
}
