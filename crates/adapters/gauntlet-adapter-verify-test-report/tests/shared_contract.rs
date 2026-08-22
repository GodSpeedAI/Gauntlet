//! The shared `Verifier` contract suite (spec §7.3) run against this
//! adapter — UNMODIFIED, from `gauntlet_ports::contract::verifier`.

use gauntlet_adapter_verify_test_report::TestReportVerifierFactory;
use gauntlet_ports::contract::verifier::verifier_suite;

/// The unmodified Task-4 `Verifier` contract suite (including the Task-11
/// `records_carry_their_instruments_identity` extension) against this
/// adapter.
#[test]
fn the_test_report_verifier_passes_the_unmodified_shared_contract_suite() {
    let factory = TestReportVerifierFactory::new();
    let findings = verifier_suite(&factory);
    assert!(
        findings.is_empty(),
        "the test-report verifier must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}
