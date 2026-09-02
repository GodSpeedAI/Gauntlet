//! `EnvelopeBuilder` — the §13.2 allowlist construction (plan Task 10 D2;
//! spec §13.2, invariant I4; AK-026's mechanism).
//!
//! The builder accepts EXACTLY the allowlist inputs: the frozen reference
//! digest + rendered reference view, the observation bundle document (for
//! the digest recompute) and its evidence ref, the typed comparison
//! contract, the applicable constraints, and the unresolved-but-relevant
//! residuals. Nothing else enters — there is no `with_extra_context`
//! escape hatch, and the ports envelope type has no member any denied
//! content could ride in (P2's structural assertion runs over the bytes
//! this module produces).
//!
//! Two couplings are enforced HERE, at the construction boundary:
//!
//! 1. **Digest recompute (B-3c, MAJOR Task-9 deferral — closed by this
//!    module):** the builder recomputes
//!    [`crate::observation::digest::observation_digest`] over the bundle
//!    document and REFUSES on any mismatch with the declared digest. A
//!    tampered facet or gap array can no longer flow silently into a
//!    comparison — the consumption boundary is where the tamper-evidence
//!    finally bites.
//! 2. **Envelope digest (§13.2):** the whole envelope is canonically
//!    serialized (the ports type's documented key order) and digested
//!    SHA-256 over those bytes — the same hashing idiom as
//!    `observation::digest`. The digest is carried on the envelope, is
//!    what a critic `RoleInvocation` records as `context_envelope_digest`
//!    (§5.3.5), and is what every `ComparisonRecord` carries back.

use gauntlet_domain::{Digest, Residual};
pub use gauntlet_ports::comparator::FrozenReferenceProjection;
use gauntlet_ports::comparator::{
    ApplicableConstraint, ComparisonEnvelope, RunComparisonAuthority, TrustedComparisonContext,
    calculate_envelope_digest,
};
pub use gauntlet_ports::comparator::{BlindnessViolation, DENY_LISTED_FIELD_NAMES, assert_blind};
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::evidence_store::EvidenceStore;

use crate::observation::bundle::ObservationBundleDoc;
use crate::observation::digest::observation_digest;

use super::contract::ComparisonContract;

/// Envelope-construction failures — typed (spec §21.1: observation /
/// comparison classes; §21.2 error discipline).
#[derive(Debug, Clone, PartialEq)]
pub enum EnvelopeError {
    /// The bundle document's declared `observation_digest` does not match
    /// the recomputed digest of its contents. A tampered facet or gap
    /// array is REFUSED here — never flows into a comparison (B-3c).
    ObservationDigestMismatch {
        declared: Digest,
        recomputed: Digest,
    },
    /// The envelope's declared digest does not cover its canonical bytes.
    /// This is distinct from a bundle mismatch: it names the §13.2
    /// invocation/record coupling boundary.
    EnvelopeDigestMismatch {
        declared: Digest,
        recomputed: Digest,
    },
    /// The canonical envelope violates the P2 structural blindness gate.
    StructuralBlindness { field: &'static str },
    /// The bundle document carries a facet or gap kind token outside the
    /// `FacetKind` vocabulary (a tampered or foreign kind cannot enter the
    /// envelope).
    UnknownFacetKind { kind: String, locus: &'static str },
    /// The frozen reference evidence is absent or cannot be read.
    FrozenReferenceUnavailable { detail: String },
    /// Frozen evidence did not normalize to the run's pinned digest.
    FrozenReferenceDigestMismatch {
        declared: Digest,
        recomputed: Digest,
    },
    /// The opaque port dispatch context rejected a mismatch between the
    /// verified frozen projection and the serializable envelope.
    TrustedDispatchRefused { detail: String },
    /// The role invocation's `context.deny` names a category the input set
    /// carries — a denied category must never enter the envelope (§10.1:
    /// "The `context.deny` list is enforced by the runtime when it builds
    /// the envelope (§13.2)").
    DeniedContextPresent { category: &'static str },
}

impl core::fmt::Display for EnvelopeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::ObservationDigestMismatch {
                declared,
                recomputed,
            } => write!(
                f,
                "observation digest mismatch at the comparison boundary: the bundle declares \
                 {declared} but its contents digest to {recomputed} — a tampered facet or gap \
                 array is refused, never compared"
            ),
            Self::EnvelopeDigestMismatch {
                declared,
                recomputed,
            } => write!(
                f,
                "envelope digest mismatch at the comparison boundary: the envelope declares \
                 {declared} but its canonical bytes digest to {recomputed}"
            ),
            Self::StructuralBlindness { field } => write!(
                f,
                "the trusted envelope build refused deny-listed field {field:?} (P2 structural \
                 blindness)"
            ),
            Self::UnknownFacetKind { kind, locus } => write!(
                f,
                "the bundle's {locus} carries kind token {kind:?}, which is outside the \
                 facet-kind vocabulary"
            ),
            Self::FrozenReferenceUnavailable { detail } => {
                write!(f, "frozen reference evidence is unavailable: {detail}")
            }
            Self::FrozenReferenceDigestMismatch {
                declared,
                recomputed,
            } => write!(
                f,
                "frozen reference evidence digest mismatch: pinned {declared}, bytes digest to {recomputed}"
            ),
            Self::TrustedDispatchRefused { detail } => {
                write!(f, "trusted comparison dispatch refused: {detail}")
            }
            Self::DeniedContextPresent { category } => write!(
                f,
                "the role's context.deny names '{category}' and the input set carries it — a \
                 denied category must not enter the envelope (spec 10.1, 13.2)"
            ),
        }
    }
}

impl std::error::Error for EnvelopeError {}

/// Exactly the §13.2 allowlist inputs (plan Task 10 D2). Every member is
/// one allowlist line; adding a member is a boundary decision (see the
/// ports envelope's module doc).
pub struct EnvelopeInputs<'a> {
    /// The earned authority for one durable run at one iteration. `run_id`
    /// and `iteration_no` are read from it — they are no longer inputs a
    /// caller can choose.
    pub authority: &'a RunComparisonAuthority,
    /// The projection loaded from the DURABLE RUN's pinned frozen evidence
    /// through that same authority. Callers cannot pair an arbitrary view
    /// with a genuine digest, and cannot name a different reference.
    pub frozen_reference: &'a FrozenReferenceProjection,
    /// The observation bundle document as decoded from the DURABLY BOUND
    /// evidence. It is here for one job only — the consumption-boundary
    /// digest recompute. The view the critic sees is projected from the same
    /// bytes inside the port, not from this value.
    pub bundle: &'a ObservationBundleDoc,
    /// The evidence store the bound observation bytes are read from.
    pub evidence: &'a dyn EvidenceStore,
    /// Deadline for that read (§6 preamble).
    pub deadline: Deadline,
    /// The typed comparison contract (§9.5 artifact).
    pub contract: &'a ComparisonContract,
    /// Constraints applicable to this comparison (§13.2 allowlist).
    pub constraints: Vec<ApplicableConstraint>,
    /// Unresolved residuals that remain relevant (§13.2 allowlist).
    pub open_residuals: Vec<Residual>,
}

/// A built envelope: the ports envelope (carrying its digest) plus the
/// canonical bytes the digest covers — so any boundary can re-verify.
#[derive(Debug)]
pub struct BuiltEnvelope {
    trusted: TrustedComparisonContext,
    canonical_bytes: String,
}

impl BuiltEnvelope {
    /// The envelope, ready for a comparator (or a critic dispatch).
    pub fn envelope(&self) -> &ComparisonEnvelope {
        self.trusted.envelope()
    }

    /// The dispatchable provenance-bearing context. Serializable envelopes
    /// are observational only; comparators consume this opaque value.
    pub fn trusted_context(&self) -> &TrustedComparisonContext {
        &self.trusted
    }

    /// The canonical serialized bytes (the ports type's documented key
    /// order; the self-referential digest member excluded — see the ports
    /// type doc). P2's blindness proof asserts over these bytes.
    pub fn canonical_bytes(&self) -> &str {
        &self.canonical_bytes
    }

    /// SHA-256 over the canonical bytes (§13.2) — equals
    /// `envelope.envelope_digest` by construction, and equals what the
    /// critic `RoleInvocation` carries as `context_envelope_digest`.
    pub fn envelope_digest(&self) -> &Digest {
        self.trusted.envelope().envelope_digest()
    }
}

/// Builds the envelope from EXACTLY [`EnvelopeInputs`] — nothing else
/// enters.
///
/// Two boundaries are enforced here and in the port respectively:
///
/// 1. **Here:** the consumption-boundary observation-digest recompute
///    (B-3c). The bundle decoded from the bound evidence must digest to what
///    it declares, so a tampered facet or gap array is refused rather than
///    compared.
/// 2. **In the port:** [`TrustedComparisonContext::issue`] re-reads the
///    DURABLY BOUND bundle bytes, projects the observation view from them,
///    assembles the envelope under the earned authority, digests the
///    canonical bytes, and runs the P2 structural blindness assertion. None
///    of that is reachable any other way, so this function is a caller of the
///    governed path, not a privileged one.
pub(crate) fn build_envelope(inputs: EnvelopeInputs<'_>) -> Result<BuiltEnvelope, EnvelopeError> {
    let recomputed = observation_digest(inputs.bundle);
    if &recomputed != inputs.bundle.observation_digest() {
        return Err(EnvelopeError::ObservationDigestMismatch {
            declared: inputs.bundle.observation_digest().clone(),
            recomputed,
        });
    }

    let trusted = TrustedComparisonContext::issue(
        inputs.authority,
        inputs.frozen_reference,
        inputs.evidence,
        inputs.contract.to_contract_view(),
        inputs.constraints,
        inputs.open_residuals,
        inputs.deadline,
    )
    .map_err(|error| EnvelopeError::TrustedDispatchRefused {
        detail: error.to_string(),
    })?;
    let canonical_bytes = trusted.envelope().to_json();
    Ok(BuiltEnvelope {
        trusted,
        canonical_bytes,
    })
}

/// The consume-side seam for a role's typed `context.deny` set (plan Task 17
/// COR-D07; spec §10.1: "The `context.deny` list is enforced by the runtime
/// when it builds the envelope (§13.2). It is a structural allowlist, not an
/// instruction to the model.").
///
/// REFUSES an input set carrying a category the invocation's role denies.
/// The five §13.2 envelope-permitted categories map one-to-one onto the
/// five context-bearing [`EnvelopeInputs`] members — `reference` →
/// `frozen_reference`, `observation_bundle` → `bundle`, `comparison_contract`
/// → `contract`, `applicable_constraints` → `constraints`, `open_residuals`
/// → `open_residuals`. Reference-shaped members are carried by construction
/// (the envelope cannot exist without them); vector members count only when
/// non-empty. `authority`, `evidence`, and `deadline` are provenance, not
/// context categories, and are not consulted.
///
/// ADDITIVE, WIRED AT PLAN TASK 20: the composed comparison chain is the
/// consumer — `GovernedRun::issue` calls it with the invocation's role
/// deny-set (loaded by the composition root) BEFORE [`build_envelope`]
/// and treats a refusal as a typed admission error, never a silent
/// narrowing to "what the builder permits anyway" (R2-N1).
pub fn assert_within_role_deny_set(
    inputs: &EnvelopeInputs<'_>,
    denies: &crate::roles::ContextDenySet,
) -> Result<(), EnvelopeError> {
    use crate::roles::ContextCategory;
    let carried = [
        (ContextCategory::Reference, true),
        (ContextCategory::ObservationBundle, true),
        (ContextCategory::ComparisonContract, true),
        (
            ContextCategory::ApplicableConstraints,
            !inputs.constraints.is_empty(),
        ),
        (
            ContextCategory::OpenResiduals,
            !inputs.open_residuals.is_empty(),
        ),
    ];
    for (category, present) in carried {
        if present && denies.denies(category) {
            return Err(EnvelopeError::DeniedContextPresent {
                category: category.token(),
            });
        }
    }
    Ok(())
}

#[allow(dead_code)]
/// Recomputes the digest of an envelope's canonical bytes and compares it
/// with the digest the envelope carries — the boundary re-verification
/// (§13.2: the digest is what couples the RoleInvocation, the envelope,
/// and the ComparisonRecord).
pub fn recompute_envelope_digest(envelope: &ComparisonEnvelope) -> Result<Digest, EnvelopeError> {
    let recomputed = calculate_envelope_digest(envelope);
    if recomputed != *envelope.envelope_digest() {
        return Err(EnvelopeError::EnvelopeDigestMismatch {
            declared: envelope.envelope_digest().clone(),
            recomputed,
        });
    }
    Ok(recomputed)
}

#[cfg(test)]
mod tests {
    use super::{
        DENY_LISTED_FIELD_NAMES, EnvelopeError, EnvelopeInputs, assert_blind, build_envelope,
        recompute_envelope_digest,
    };
    use crate::observation::bundle::{
        BundleContent, ClaimsFacet, FacetRecord, ObservationBundleDoc,
    };
    use gauntlet_domain::{
        Digest, EvidenceRef, InstrumentName, IterationNo, Residual, ResidualId, RunId, Severity,
    };
    use gauntlet_ports::observer::{FacetKind, InstrumentIdentity, ObservationFacet};

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const BASE_INSTANT: u64 = 1_700_000_000;
    const CANARY_CLAIM: &str = "the artifact passes its own declared self-check, says the builder";

    fn contract() -> crate::compare::contract::ComparisonContract {
        crate::compare::contract::ComparisonContract::parse(
            "version: 1
dimensions:
  - functional behavior
  - evidence completeness
severity_rubric:
  - blocking | invalidates
  - major | real
  - minor | small
  - informational | recorded
comparator_receives:
  - the allowlist
residual_definition:
  - a difference
",
        )
        .expect("the test contract parses")
    }

    fn bundle() -> ObservationBundleDoc {
        let instrument = InstrumentIdentity::new(
            InstrumentName::new("suite-probe").expect("valid instrument name"),
            "1.2.3".to_owned(),
        );
        let evidence = EvidenceRef::new("evd-suite-0001").expect("valid evidence ref");
        let facet = ObservationFacet::new(FacetKind::Test, instrument, evidence)
            .with_invocation(Some("re-run the declared self-check".to_owned()), Some(1))
            .with_window(
                Some(gauntlet_domain::EpochSeconds::new(BASE_INSTANT)),
                Some(gauntlet_domain::EpochSeconds::new(BASE_INSTANT + 3)),
            );
        let mut content = BundleContent {
            run_id: RunId::new(ULID).expect("valid run id"),
            iteration_no: IterationNo::new(1).expect("iterations are 1-based"),
            observed_at: BASE_INSTANT,
            project_revision: "rev-suite-01".to_owned(),
            environment_fingerprint: "sha256:0123abcd".to_owned(),
            observation_digest: Digest::new("sha256", &"ab".repeat(32))
                .expect("valid digest parts"),
            facets: vec![FacetRecord::from_facet(&facet)],
            claims_facet: ClaimsFacet::new(CANARY_CLAIM.to_owned()),
            unavailable_evidence: Vec::new(),
            observation_failures: Vec::new(),
        };
        content.observation_digest = crate::observation::digest::observation_digest(
            &ObservationBundleDoc::new(content.clone()),
        );
        ObservationBundleDoc::new(content)
    }

    /// The governed fixture: a durable run, a durably bound observation
    /// bundle, an earned authority, and the projection loaded through it.
    /// Neither the reference version nor the observation ref is a fixture
    /// value any more — both come out of durable state.
    fn governed(bundle: &ObservationBundleDoc) -> crate::compare::testing::Governed {
        crate::compare::testing::governed(
            &RunId::new(ULID).expect("valid run id"),
            IterationNo::new(1).expect("iterations are 1-based"),
            "the frozen reference's rendered view\n",
            bundle.to_json().as_bytes(),
        )
    }

    fn inputs<'a>(
        governed: &'a crate::compare::testing::Governed,
        bundle: &'a ObservationBundleDoc,
        contract: &'a crate::compare::contract::ComparisonContract,
    ) -> EnvelopeInputs<'a> {
        EnvelopeInputs {
            authority: &governed.authority,
            frozen_reference: &governed.projection,
            bundle,
            evidence: governed.evidence.as_ref(),
            deadline: crate::compare::testing::bound(),
            contract,
            constraints: Vec::new(),
            open_residuals: vec![Residual::new(
                ResidualId::new("res-000001").expect("valid residual id"),
                Severity::Minor,
                "a previously recorded, still-relevant difference".to_owned(),
                Some("functional behavior".to_owned()),
            )],
        }
    }

    /// Builds through the governed chain the way production does.
    fn build(
        bundle: &ObservationBundleDoc,
        contract: &crate::compare::contract::ComparisonContract,
    ) -> Result<super::BuiltEnvelope, EnvelopeError> {
        let governed = governed(bundle);
        build_envelope(inputs(&governed, bundle, contract))
    }

    #[test]
    fn the_envelope_carries_facet_reality_and_none_of_the_claims() {
        let bundle = bundle();
        let contract = contract();
        let built = build(&bundle, &contract).expect("the envelope builds");
        let bytes = built.canonical_bytes();
        // Facet-level reality IS present (P3's comparison half).
        assert!(bytes.contains("\"exit_status\":1"));
        assert!(bytes.contains("evd-suite-0001"));
        assert!(bytes.contains("functional behavior"));
        // Claims content is absent (I2 carried through the envelope). The
        // facet's COMMAND is provenance (AK-028, C10 discipline): it
        // legitimately spells the re-run instrument invocation — so the
        // canary asserted here is the claims STORY, not the instrument's
        // own command text.
        assert!(!bytes.contains(CANARY_CLAIM));
        assert!(!bytes.contains("says the builder"));
        assert!(!bytes.contains("passes its own"));
        // The open residual (an allowlist member) IS present.
        assert!(bytes.contains("a previously recorded, still-relevant difference"));
        // The self-referential digest member is excluded from the bytes.
        assert!(!bytes.contains("envelope_digest"));
    }

    #[test]
    fn the_envelope_digest_is_recomputable_and_coupling_friendly() {
        let bundle = bundle();
        let contract = contract();
        let built = build(&bundle, &contract).expect("the envelope builds");
        assert_eq!(
            recompute_envelope_digest(built.envelope()).expect("the digest recomputes"),
            *built.envelope_digest(),
            "any boundary can re-verify the digest over the canonical bytes"
        );
        // The projection has no caller-visible view mutation path; a different
        // view requires loading different pinned frozen evidence.
    }

    #[test]
    fn a_critic_role_invocation_carries_the_envelope_digest() {
        // Spec §5.3.5/§13.2: the digest "is stored on both the
        // RoleInvocation and the ComparisonRecord". Prove the composition
        // with a real RoleInvocation construction.
        let bundle = bundle();
        let contract = contract();
        let built = build(&bundle, &contract).expect("the envelope builds");
        let invocation =
            gauntlet_domain::RoleInvocation::admit(gauntlet_domain::RoleInvocationDefinition {
                id: gauntlet_domain::InvocationId::new(ULID).expect("valid invocation id"),
                run_id: RunId::new(ULID).expect("valid run id"),
                iteration_no: IterationNo::new(1).expect("iterations are 1-based"),
                role: gauntlet_domain::RoleId::Critic,
                runner: gauntlet_domain::RunnerIdentity::new("runner-07").expect("valid identity"),
                model: None,
                permissions: gauntlet_domain::PermissionSetRef::new("perm-readonly")
                    .expect("valid ref"),
                context_envelope_digest: built.envelope_digest().clone(),
                workspace_ref: gauntlet_domain::WorkspaceRef::new("workspace-03")
                    .expect("valid ref"),
                tier: gauntlet_domain::BlastRadius::Contained,
                sandbox_policy_ref: None,
                result_contract_path: gauntlet_domain::ResultContractPath::new(
                    "critic-result-document",
                )
                .expect("valid path"),
                admitted_at: Some(gauntlet_domain::EpochSeconds::new(BASE_INSTANT)),
                session_ref: None,
            })
            .expect("a contained critic invocation admits without a sandbox policy");
        assert_eq!(
            invocation.context_envelope_digest(),
            built.envelope_digest(),
            "the RoleInvocation carries exactly the envelope's digest"
        );
        assert_eq!(invocation.role(), &gauntlet_domain::RoleId::Critic);
    }

    #[test]
    fn a_tampered_bundle_is_refused_at_the_consumption_boundary() {
        // B-3c closed: tamper a facet AFTER digesting and the builder
        // refuses with the typed mismatch error.
        let bundle = bundle();
        let contract = contract();
        let mut tampered = bundle.content();
        tampered.facets[0] = FacetRecord::from_facet(
            &(ObservationFacet::new(
                FacetKind::Test,
                InstrumentIdentity::new(
                    InstrumentName::new("suite-probe").expect("valid instrument name"),
                    "1.2.3".to_owned(),
                ),
                EvidenceRef::new("evd-suite-0001").expect("valid evidence ref"),
            )
            .with_invocation(Some("re-run the declared self-check".to_owned()), Some(0))),
        );
        let tampered = ObservationBundleDoc::new(tampered);
        assert_eq!(
            tampered.observation_digest(),
            bundle.observation_digest(),
            "the declared digest still claims the original contents"
        );
        assert!(matches!(
            build(&tampered, &contract),
            Err(EnvelopeError::ObservationDigestMismatch { .. })
        ));
        // The same refusal fires for a tampered gap array.
        let mut gapped = bundle.content();
        gapped.unavailable_evidence.push(
            crate::observation::bundle::UnavailableEvidenceRecord::new(
                "browser".to_owned(),
                "a fabricated gap".to_owned(),
            ),
        );
        let gapped = ObservationBundleDoc::new(gapped);
        assert!(matches!(
            build(&gapped, &contract),
            Err(EnvelopeError::ObservationDigestMismatch { .. })
        ));
    }

    #[test]
    fn legitimate_frozen_prose_is_not_mistaken_for_an_injected_field() {
        // The trusted boundary checks JSON field shape, not words quoted by
        // legitimate frozen text. Provenance of a caller-supplied projection
        // is a separate typed-port gap, not something this lexical gate can
        // honestly establish.
        assert!(
            assert_blind(r#"{"reference_view":"builder_reasoning: quoted frozen text"}"#).is_ok()
        );
        assert!(assert_blind(r#"{"builder_reasoning":"canary","reference_view":"x"}"#).is_err());
    }

    #[test]
    fn a_poisoned_serialization_fails_the_blindness_proof() {
        // AK-027's designed-to-fail pair, at the unit level: every
        // deny-listed field name, injected into otherwise-blind bytes,
        // must fail the proof.
        let bundle = bundle();
        let contract = contract();
        let built = build(&bundle, &contract).expect("the envelope builds");
        let bytes = built.canonical_bytes();
        assert!(assert_blind(bytes).is_ok(), "the honest envelope is blind");
        for field in DENY_LISTED_FIELD_NAMES {
            let poisoned = bytes.replace(
                "\"reference_view\":",
                &format!("\"{field}\":\"x\",\"reference_view\":"),
            );
            assert!(
                assert_blind(&poisoned).is_err(),
                "the proof must reject a poisoned envelope carrying {field:?}"
            );
        }
    }

    #[test]
    fn a_built_envelope_is_usable_by_the_ports_comparator_surface() {
        // The produced envelope is exactly the port's input shape: a
        // domain record built over it couples all three digests (the
        // Task-4 constructor discipline, exercised from the app side).
        let bundle = bundle();
        let contract = contract();
        let built = build(&bundle, &contract).expect("the envelope builds");
        let envelope = built.envelope();
        let record = gauntlet_domain::ComparisonRecord::new(
            envelope.reference_digest().clone(),
            envelope.observation().observation_digest().clone(),
            envelope.envelope_digest().clone(),
            Vec::new(),
            gauntlet_domain::Verdict::Inconclusive,
        )
        .expect("no residuals, no contradiction");
        assert_eq!(record.envelope_digest(), built.envelope_digest());
    }

    #[test]
    fn a_role_deny_set_naming_a_carried_category_refuses_the_input_set() {
        // COR-D07 teeth: spec §10.1 — "The `context.deny` list is enforced
        // by the runtime when it builds the envelope (§13.2)." An input set
        // carrying a category the invocation's role denies is REFUSED; an
        // unrelated deny-set (categories the envelope cannot carry anyway)
        // refuses nothing.
        use crate::roles::{ContextCategory, ContextDenySet};
        let bundle = bundle();
        let contract = contract();
        let governed = governed(&bundle);
        let set = inputs(&governed, &bundle, &contract);
        let deny = ContextDenySet::new(vec![ContextCategory::Reference]);
        let refusal = super::assert_within_role_deny_set(&set, &deny)
            .expect_err("a denied category must not ride the input set");
        assert!(
            matches!(
                &refusal,
                EnvelopeError::DeniedContextPresent { category } if *category == "reference"
            ),
            "the denied category is named, got {refusal:?}"
        );
        // Vector members count only when non-empty: this input set carries
        // no constraints but does carry an open residual.
        let empty_ok = ContextDenySet::new(vec![ContextCategory::ApplicableConstraints]);
        assert!(
            super::assert_within_role_deny_set(&set, &empty_ok).is_ok(),
            "denying an absent member refuses nothing"
        );
        let residual_denied = ContextDenySet::new(vec![ContextCategory::OpenResiduals]);
        assert!(
            super::assert_within_role_deny_set(&set, &residual_denied).is_err(),
            "denying a present member refuses"
        );
        let unrelated = ContextDenySet::new(vec![ContextCategory::BuilderTrajectory]);
        assert!(
            super::assert_within_role_deny_set(&set, &unrelated).is_ok(),
            "blindness categories the envelope cannot carry refuse nothing"
        );
    }
}
