//! Verification inputs (spec §12.3, I2; plan Task 9 step 4): what
//! downstream comparison and verification may consume from an
//! observation bundle — the INSTRUMENT facets, the digest, the bundle
//! ref, and BOTH gap arrays. The claims facet is excluded from these
//! inputs BY CONSTRUCTION: [`InstrumentFacets`] has a private constructor
//! built only by [`VerificationInputs::of_bundle`], which reads
//! `doc.facets` and the gap arrays and nothing else. There is no `From`
//! impl, no accessor reuse, and no runtime `if kind == claims` anywhere —
//! claims content reaches only [`super::bundle::ClaimsFacet::content`]
//! for forensics.
//!
//! Provenance text is not a verdict (correction C10, Task-9 adjudication):
//! the facet `command` field and the gap records' `detail` fields that
//! appear in this serialization are PROVENANCE and diagnostic records —
//! the command is the instrument-invocation record the Annex-B wire
//! schema itself requires (AK-028), and a detail names why an instrument
//! could not run. The instrument-PRODUCED exit status beside them is the
//! verdict carrier; builder-penned text sitting in a command or detail
//! string cannot flip an outcome — it is inert context a comparator reads
//! alongside the exit status and the digest that covers it.

use gauntlet_domain::{Digest, EvidenceRef};
use gauntlet_ports::json::{JsonNumber, JsonValue};

use super::bundle::{
    FacetRecord, ObservationBundleDoc, ObservationFailureRecord, UnavailableEvidenceRecord,
};

/// The declared field vocabulary of one facet as serialized inside the
/// verification inputs: the REDUCED facet record (Annex B.3's facet minus
/// the timing window and summary — verification consumes identity,
/// command, exit status, and evidence location). The canonical
/// serialization is zipped from this table, so the key order is the
/// declared order by construction.
const FACET_INPUT_FIELDS: &[&str] = &[
    "kind",
    "instrument_name",
    "instrument_version",
    "command",
    "exit_status",
    "evidence_ref",
];

/// The instrument facets verification may consume. The constructor is
/// private to this module: the ONLY way to obtain one is
/// [`VerificationInputs::of_bundle`], so no caller can smuggle a claims
/// facet into the array (it is a different type — there is no conversion).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstrumentFacets {
    records: Vec<FacetRecord>,
}

impl InstrumentFacets {
    fn from_records(records: Vec<FacetRecord>) -> Self {
        Self { records }
    }

    /// The instrument facet records, in observation order.
    pub fn as_slice(&self) -> &[FacetRecord] {
        &self.records
    }

    pub fn iter(&self) -> std::slice::Iter<'_, FacetRecord> {
        self.records.iter()
    }

    pub fn len(&self) -> usize {
        self.records.len()
    }

    pub fn is_empty(&self) -> bool {
        self.records.is_empty()
    }
}

/// What verification consumes from a bundle (plan Task 9 step 4): the
/// instrument facets, the observation digest, the bundle's evidence ref,
/// and the two gap arrays (I8 — gaps are inputs, not noise: a comparator
/// or verifier confronted with recorded unavailable evidence must not
/// treat it as a pass).
#[derive(Debug, Clone, PartialEq)]
pub struct VerificationInputs {
    facets: InstrumentFacets,
    observation_digest: Digest,
    bundle_ref: EvidenceRef,
    unavailable_evidence: Vec<UnavailableEvidenceRecord>,
    observation_failures: Vec<ObservationFailureRecord>,
}

impl VerificationInputs {
    /// Builds the inputs from a bundle document and its evidence ref.
    /// Reads `doc.facets`, the digest, and the gap arrays ONLY — the
    /// claims facet is a different type with no path into this one.
    pub fn of_bundle(doc: &ObservationBundleDoc, bundle_ref: EvidenceRef) -> Self {
        Self {
            facets: InstrumentFacets::from_records(doc.facets().to_vec()),
            observation_digest: doc.observation_digest().clone(),
            bundle_ref,
            unavailable_evidence: doc.unavailable_evidence().to_vec(),
            observation_failures: doc.observation_failures().to_vec(),
        }
    }

    /// The instrument facets verification may consume.
    pub fn facets(&self) -> &InstrumentFacets {
        &self.facets
    }

    pub fn observation_digest(&self) -> &Digest {
        &self.observation_digest
    }

    /// Where the assembled bundle document lives (content-addressed).
    pub fn bundle_ref(&self) -> &EvidenceRef {
        &self.bundle_ref
    }

    pub fn unavailable_evidence(&self) -> &[UnavailableEvidenceRecord] {
        &self.unavailable_evidence
    }

    pub fn observation_failures(&self) -> &[ObservationFailureRecord] {
        &self.observation_failures
    }

    /// Canonical serialization (documented key order: bundle_ref,
    /// observation_digest, facets, unavailable_evidence,
    /// observation_failures) — the serialization the canary tests sweep
    /// for claims text. Each facet entry is zipped from the declared
    /// [`FACET_INPUT_FIELDS`] table, so the wire key order is the declared
    /// order by construction.
    pub fn to_json(&self) -> String {
        JsonValue::Object(vec![
            (
                "bundle_ref".to_owned(),
                JsonValue::Str(self.bundle_ref.as_str().to_owned()),
            ),
            (
                "observation_digest".to_owned(),
                JsonValue::Str(self.observation_digest.to_string()),
            ),
            (
                "facets".to_owned(),
                JsonValue::Array(
                    self.facets
                        .as_slice()
                        .iter()
                        .map(|record| {
                            let values: Vec<JsonValue> = vec![
                                JsonValue::Str(record.kind().to_owned()),
                                JsonValue::Str(record.instrument_name().to_owned()),
                                JsonValue::Str(record.instrument_version().to_owned()),
                                match record.command() {
                                    Some(command) => JsonValue::Str(command.to_owned()),
                                    None => JsonValue::Null,
                                },
                                match record.exit_status() {
                                    Some(status) if status >= 0 => {
                                        JsonValue::Number(JsonNumber::Uint(status.unsigned_abs()))
                                    }
                                    Some(status) => JsonValue::Number(JsonNumber::NegInt(status)),
                                    None => JsonValue::Null,
                                },
                                JsonValue::Str(record.evidence_ref().to_owned()),
                            ];
                            JsonValue::Object(
                                FACET_INPUT_FIELDS
                                    .iter()
                                    .zip(values)
                                    .map(|(field, value)| ((*field).to_owned(), value))
                                    .collect(),
                            )
                        })
                        .collect(),
                ),
            ),
            (
                "unavailable_evidence".to_owned(),
                JsonValue::Array(
                    self.unavailable_evidence
                        .iter()
                        .map(|record| {
                            JsonValue::Object(vec![
                                ("kind".to_owned(), JsonValue::Str(record.kind().to_owned())),
                                (
                                    "reason".to_owned(),
                                    JsonValue::Str(record.reason().to_owned()),
                                ),
                            ])
                        })
                        .collect(),
                ),
            ),
            (
                "observation_failures".to_owned(),
                JsonValue::Array(
                    self.observation_failures
                        .iter()
                        .map(|record| {
                            JsonValue::Object(vec![
                                ("kind".to_owned(), JsonValue::Str(record.kind().to_owned())),
                                (
                                    "error_class".to_owned(),
                                    JsonValue::Str(record.error_class().to_owned()),
                                ),
                                (
                                    "detail".to_owned(),
                                    JsonValue::Str(record.detail().to_owned()),
                                ),
                            ])
                        })
                        .collect(),
                ),
            ),
        ])
        .to_json_text()
    }
}

#[cfg(test)]
mod tests {
    use super::VerificationInputs;
    use crate::observation::bundle::{
        BundleContent, ClaimsFacet, FacetRecord, ObservationBundleDoc,
    };
    use gauntlet_domain::InstrumentName;
    use gauntlet_domain::{Digest, EvidenceRef, IterationNo, RunId};
    use gauntlet_ports::observer::{FacetKind, InstrumentIdentity, ObservationFacet};

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const BASE_INSTANT: u64 = 1_700_000_000;
    const CANARY_CLAIM: &str = "the artifact passes its own declared self-check";
    const OTHER_CLAIM: &str = "a completely different forensic story";

    fn doc_with(claims: &str) -> ObservationBundleDoc {
        let instrument = InstrumentIdentity::new(
            InstrumentName::new("suite-probe").expect("valid instrument name"),
            "1.2.3".to_owned(),
        );
        let evidence: EvidenceRef = EvidenceRef::new("evd-suite-0001").expect("valid evidence ref");
        let facet = ObservationFacet::new(FacetKind::Test, instrument, evidence)
            .with_invocation(Some("probe".to_owned()), Some(1))
            .with_window(
                Some(gauntlet_domain::EpochSeconds::new(BASE_INSTANT)),
                Some(gauntlet_domain::EpochSeconds::new(BASE_INSTANT + 3)),
            )
            .with_summary(Some("one probe, observed".to_owned()));
        ObservationBundleDoc::new(BundleContent {
            run_id: RunId::new(ULID).expect("valid run id"),
            iteration_no: IterationNo::new(1).expect("iterations are 1-based"),
            observed_at: BASE_INSTANT,
            project_revision: "rev-suite-01".to_owned(),
            environment_fingerprint: "sha256:0123abcd".to_owned(),
            observation_digest: Digest::new("sha256", &"ab".repeat(32))
                .expect("valid digest parts"),
            facets: vec![FacetRecord::from_facet(&facet)],
            claims_facet: ClaimsFacet::new(claims.to_owned()),
            unavailable_evidence: Vec::new(),
            observation_failures: Vec::new(),
        })
    }

    fn inputs_of(doc: &ObservationBundleDoc) -> VerificationInputs {
        let bundle_ref = EvidenceRef::new("evd-bundle-0001").expect("valid bundle ref");
        VerificationInputs::of_bundle(doc, bundle_ref)
    }

    #[test]
    fn claims_text_never_reaches_verification_inputs() {
        // The canary (I2): a bundle whose claims facet carries a rich,
        // tempting success story produces inputs whose canonical bytes
        // contain none of it — not the story, not fragments of it.
        let doc = doc_with(CANARY_CLAIM);
        let inputs = inputs_of(&doc);
        let text = inputs.to_json();
        assert!(!text.contains(CANARY_CLAIM));
        assert!(!text.contains("declared self-check"));
        assert!(!text.contains("success story"));
        // The instrument side IS there.
        assert!(text.contains("\"kind\":\"test\""));
        assert!(text.contains("suite-probe"));
        assert!(text.contains("\"exit_status\":1"));
    }

    #[test]
    fn the_facet_and_gap_inputs_are_identical_under_a_claims_swap() {
        // Brief friction, recorded: the brief asks for "inputs equal inputs
        // built from the same bundle with claims content swapped", but
        // VerificationInputs carries the bundle's observation_digest and
        // that digest COVERS the claims (forensic tamper-evidence, §12.4)
        // — so whole-struct equality under a claims swap is unsatisfiable
        // by design. The exclusion property that IS satisfiable and is the
        // point of I2: every INPUT except the digest identifier is a
        // function of the instrument side alone, and the canonical bytes
        // differ only in that digest value.
        let first = inputs_of(&doc_with(CANARY_CLAIM));
        let second = inputs_of(&doc_with(OTHER_CLAIM));
        assert_eq!(first.facets(), second.facets());
        assert_eq!(first.unavailable_evidence(), second.unavailable_evidence());
        assert_eq!(first.observation_failures(), second.observation_failures());
        assert_eq!(first.bundle_ref(), second.bundle_ref());
        let first_text = first.to_json();
        let second_text = second.to_json();
        let strip_digest = |text: String| {
            text.split(',')
                .map(|piece| {
                    if piece.starts_with("\"observation_digest\"") {
                        "\"observation_digest\":<digest>"
                    } else {
                        piece
                    }
                })
                .collect::<Vec<_>>()
                .join(",")
        };
        assert_eq!(
            strip_digest(first_text),
            strip_digest(second_text),
            "the inputs differ only in the digest identifier"
        );
    }

    #[test]
    fn a_claims_change_moves_the_digest_but_not_the_facet_inputs() {
        let canary_doc = doc_with(CANARY_CLAIM);
        let other_doc = doc_with(OTHER_CLAIM);
        let mut canary_content = canary_doc.content();
        canary_content.observation_digest =
            crate::observation::digest::observation_digest(&canary_doc);
        let canary_final = ObservationBundleDoc::new(canary_content);
        let mut other_content = other_doc.content();
        other_content.observation_digest =
            crate::observation::digest::observation_digest(&other_doc);
        let other_final = ObservationBundleDoc::new(other_content);
        assert_ne!(
            canary_final.observation_digest(),
            other_final.observation_digest(),
            "claims ARE covered by the digest (forensic tamper-evidence)"
        );
        assert_eq!(
            inputs_of(&canary_final).facets(),
            inputs_of(&other_final).facets(),
            "the facet inputs are unchanged by the claims"
        );
    }

    #[test]
    fn gaps_are_inputs_not_noise() {
        // I8: recorded gaps ride along with the inputs, so a downstream
        // consumer cannot treat missing evidence as a pass.
        let mut content = doc_with(CANARY_CLAIM).content();
        content.unavailable_evidence.push(
            crate::observation::bundle::UnavailableEvidenceRecord::new(
                "browser".to_owned(),
                "the probe found nothing at this locator".to_owned(),
            ),
        );
        content.observation_failures.push(
            crate::observation::bundle::ObservationFailureRecord::new(
                FacetKind::Test.token().to_owned(),
                "instrument_failed".to_owned(),
                "the instrument exited without a report".to_owned(),
            ),
        );
        let doc = ObservationBundleDoc::new(content);
        let inputs = inputs_of(&doc);
        assert_eq!(inputs.unavailable_evidence().len(), 1);
        assert_eq!(inputs.unavailable_evidence()[0].kind(), "browser");
        assert_eq!(
            inputs.observation_failures()[0].error_class(),
            "instrument_failed"
        );
        assert_eq!(
            inputs.facets().len(),
            1,
            "the healthy facet is still an input"
        );
    }
}
