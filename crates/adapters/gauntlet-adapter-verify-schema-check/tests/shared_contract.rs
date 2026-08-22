//! The shared `Verifier` contract suite (spec §7.3) run against this
//! adapter — UNMODIFIED, from `gauntlet_ports::contract::verifier`.

use gauntlet_adapter_verify_schema_check::SchemaCheckVerifierFactory;
use gauntlet_ports::contract::verifier::verifier_suite;

/// The unmodified Task-4 `Verifier` contract suite (including the Task-11
/// `records_carry_their_instruments_identity` extension) against this
/// adapter.
#[test]
fn the_schema_check_verifier_passes_the_unmodified_shared_contract_suite() {
    let factory = SchemaCheckVerifierFactory::new();
    let findings = verifier_suite(&factory);
    assert!(
        findings.is_empty(),
        "the schema-check verifier must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}
