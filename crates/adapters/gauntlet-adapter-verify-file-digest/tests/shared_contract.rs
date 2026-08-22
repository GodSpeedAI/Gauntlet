//! The shared `Verifier` contract suite (spec §7.3) run against this
//! adapter — UNMODIFIED, from `gauntlet_ports::contract::verifier` — plus
//! this instrument's own provenance pin.

use gauntlet_adapter_verify_file_digest::FileDigestVerifierFactory;
use gauntlet_ports::contract::verifier::verifier_suite;

/// The unmodified Task-4 `Verifier` contract suite (including the Task-11
/// `records_carry_their_instruments_identity` extension) against this
/// adapter.
#[test]
fn the_file_digest_verifier_passes_the_unmodified_shared_contract_suite() {
    let factory = FileDigestVerifierFactory::new();
    let findings = verifier_suite(&factory);
    assert!(
        findings.is_empty(),
        "the file-digest verifier must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}
