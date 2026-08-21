//! `observation_digest` (spec §12.4; plan Task 9): the digest covers the
//! normalized bundle MINUS TIMESTAMPS — `observed_at` and each facet's
//! `started_at`/`ended_at` — so the same observation re-taken at a
//! different wall-clock time digests identically. Everything else is
//! covered: facet content, claims content, and BOTH gap arrays (a hidden
//! failure changes the digest — tamper-evidence), which is a different
//! concern from claims being excluded from VERIFICATION inputs (I2,
//! `input` module).
//!
//! One documented normalization rule beyond the timestamps: the document's
//! own `observation_digest` field is self-referential, so the normalized
//! form pins it to the empty string before hashing. The digest is
//! therefore a function of every field except the timestamps and the
//! digest field itself — pinned by test.

use gauntlet_domain::Digest;

use super::bundle::ObservationBundleDoc;

/// The normalized canonical bytes: the full canonical object with
/// `observed_at` removed, every facet's `started_at`/`ended_at` removed,
/// and `observation_digest` pinned to `""`. Exposed so tests pin the
/// normalization directly (plan Task 9 step 3).
pub fn normalized_bytes(doc: &ObservationBundleDoc) -> String {
    gauntlet_ports::observation_digest::normalized_bytes(&doc.object_entries())
}

/// The §12.4 digest: SHA-256 over [`normalized_bytes`], carried by the
/// domain `Digest`. Infallible by construction — SHA-256 output is 64
/// lowercase hex characters, exactly the domain's hex grammar (pinned by
/// every test that compares digests).
pub fn observation_digest(doc: &ObservationBundleDoc) -> Digest {
    gauntlet_ports::observation_digest::observation_digest(&doc.object_entries())
}

#[cfg(test)]
mod tests {
    use super::{normalized_bytes, observation_digest};
    use crate::observation::bundle::{
        BundleContent, ClaimsFacet, FacetRecord, ObservationBundleDoc,
    };
    use gauntlet_domain::InstrumentName;
    use gauntlet_domain::{Digest, EvidenceRef, IterationNo, RunId};
    use gauntlet_ports::observer::{FacetKind, InstrumentIdentity, ObservationFacet};

    const ULID: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
    const BASE_INSTANT: u64 = 1_700_000_000;
    const CANARY_CLAIM: &str = "the artifact passes its own declared self-check";

    fn facet_record(kind: FacetKind, exit: Option<i64>) -> FacetRecord {
        let instrument = InstrumentIdentity::new(
            InstrumentName::new("suite-probe").expect("valid instrument name"),
            "1.2.3".to_owned(),
        );
        let evidence: EvidenceRef = EvidenceRef::new("evd-suite-0001").expect("valid evidence ref");
        FacetRecord::from_facet(
            &(ObservationFacet::new(kind, instrument, evidence)
                .with_invocation(Some("probe".to_owned()), exit)
                .with_window(
                    Some(gauntlet_domain::EpochSeconds::new(BASE_INSTANT)),
                    Some(gauntlet_domain::EpochSeconds::new(BASE_INSTANT + 3)),
                )
                .with_summary(Some("one probe, observed".to_owned()))),
        )
    }

    fn doc_with(claims: &str, extra_failure: bool) -> ObservationBundleDoc {
        let mut failures = Vec::new();
        if extra_failure {
            failures.push(crate::observation::bundle::ObservationFailureRecord::new(
                FacetKind::Test.token().to_owned(),
                "instrument_failed".to_owned(),
                "the instrument exited without a report".to_owned(),
            ));
        }
        ObservationBundleDoc::new(BundleContent {
            run_id: RunId::new(ULID).expect("valid run id"),
            iteration_no: IterationNo::new(1).expect("iterations are 1-based"),
            observed_at: BASE_INSTANT,
            project_revision: "rev-suite-01".to_owned(),
            environment_fingerprint: "sha256:0123abcd".to_owned(),
            observation_digest: Digest::new("sha256", &"ab".repeat(32))
                .expect("valid digest parts"),
            facets: vec![facet_record(FacetKind::Test, Some(1))],
            claims_facet: ClaimsFacet::new(claims.to_owned()),
            unavailable_evidence: Vec::new(),
            observation_failures: failures,
        })
    }

    #[test]
    fn shifted_timestamps_do_not_change_the_digest() {
        let early = doc_with(CANARY_CLAIM, false);
        let mut late_content = early.content();
        late_content.observed_at = BASE_INSTANT + 4_000;
        late_content.facets[0] =
            FacetRecord::from_facet(&facet_at(BASE_INSTANT + 4_000, BASE_INSTANT + 4_003));
        let late = ObservationBundleDoc::new(late_content);
        assert_ne!(early.to_json(), late.to_json(), "the full documents differ");
        assert_eq!(
            normalized_bytes(&early),
            normalized_bytes(&late),
            "the normalized forms are identical"
        );
        assert_eq!(
            observation_digest(&early),
            observation_digest(&late),
            "the digest is timestamp-free"
        );
        // The normalization is visible in the bytes: no timestamp keys.
        let normalized = normalized_bytes(&early);
        assert!(!normalized.contains("observed_at"));
        assert!(!normalized.contains("started_at"));
        assert!(!normalized.contains("ended_at"));
    }

    fn facet_at(started: u64, ended: u64) -> gauntlet_ports::observer::ObservationFacet {
        let instrument = InstrumentIdentity::new(
            InstrumentName::new("suite-probe").expect("valid instrument name"),
            "1.2.3".to_owned(),
        );
        let evidence: EvidenceRef = EvidenceRef::new("evd-suite-0001").expect("valid evidence ref");
        ObservationFacet::new(FacetKind::Test, instrument, evidence)
            .with_invocation(Some("probe".to_owned()), Some(1))
            .with_window(
                Some(gauntlet_domain::EpochSeconds::new(started)),
                Some(gauntlet_domain::EpochSeconds::new(ended)),
            )
            .with_summary(Some("one probe, observed".to_owned()))
    }

    #[test]
    fn normalized_bytes_is_the_full_json_minus_the_timestamp_members() {
        let doc = doc_with(CANARY_CLAIM, false);
        // Surgical removal from the FULL form must equal the normalized
        // form byte for byte — the two builders cannot drift apart.
        let full = doc.to_json();
        let without_observed_at = remove_member(&full, "\"observed_at\":");
        let without_facet_windows = without_observed_at
            .replace(",\"started_at\":1700000000", "")
            .replace(",\"ended_at\":1700000003", "");
        let with_digest_pinned =
            without_facet_windows.replace(
                "\"observation_digest\":\"sha256:abababababababababababababababababababababababababababababababab\"",
                "\"observation_digest\":\"\"",
            );
        assert_eq!(normalized_bytes(&doc), with_digest_pinned);
    }

    fn remove_member(text: &str, key: &str) -> String {
        let start = text.find(key).expect("the member is present");
        let value_start = start + key.len();
        let mut depth = 0usize;
        let mut end = value_start;
        let bytes = text.as_bytes();
        while end < bytes.len() {
            match bytes[end] {
                b'[' | b'{' => depth += 1,
                b']' | b'}' => {
                    if depth == 0 {
                        break;
                    }
                    depth -= 1;
                }
                b',' if depth == 0 => break,
                _ => {}
            }
            end += 1;
        }
        // Drop the trailing comma too when one follows.
        let mut removed = String::new();
        removed.push_str(&text[..start]);
        if bytes.get(end) == Some(&b',') {
            removed.push_str(&text[end + 1..]);
        } else {
            removed.push_str(&text[end..]);
        }
        removed
    }

    #[test]
    fn facet_content_claims_and_gap_changes_each_change_the_digest() {
        let base = doc_with(CANARY_CLAIM, false);
        let base_digest = observation_digest(&base);

        // One-byte facet content change.
        let mut other_content = base.content();
        other_content.facets[0] = facet_record(FacetKind::Test, Some(2));
        let other = ObservationBundleDoc::new(other_content);
        assert_ne!(observation_digest(&other), base_digest);

        // Claims content change.
        let other = doc_with("a different story entirely", false);
        assert_ne!(observation_digest(&other), base_digest);

        // Gap-array change.
        let other = doc_with(CANARY_CLAIM, true);
        assert_ne!(observation_digest(&other), base_digest);
    }

    #[test]
    fn the_digest_field_itself_is_pinned_away() {
        let doc = doc_with(CANARY_CLAIM, false);
        let mut swapped = doc.content();
        swapped.observation_digest =
            Digest::new("sha256", &"cd".repeat(32)).expect("valid digest parts");
        let swapped_doc = ObservationBundleDoc::new(swapped);
        assert_eq!(
            observation_digest(&doc),
            observation_digest(&swapped_doc),
            "the normalized form pins the self-referential field to empty"
        );
        assert_ne!(doc.observation_digest(), swapped_doc.observation_digest());
    }
}
