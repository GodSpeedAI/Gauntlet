//! The shared `OperatorSurface` contract suite (spec §6.2/§20.2) run
//! against this adapter — "Adding an adapter means implementing the
//! trait and running the existing suite, not writing a bespoke one." The
//! suite lives, UNMODIFIED, in
//! `gauntlet-ports::contract::operator_surface`; this file only supplies
//! the factory and asserts the findings are empty. The suite's
//! seeded-snapshot constructor is injectable (the P8 pattern): the echo
//! case runs against the adapter's synthetic `MemorySurface` source; the
//! REAL durable source is exercised by the CLI integration tests
//! (`crates/gauntlet-cli/tests/operator_actions_no_bypass.rs`).

use gauntlet_adapter_surface_json::testing::MemorySurfaceFactory;
use gauntlet_ports::contract::operator_surface::operator_surface_suite;

/// The unmodified shared `OperatorSurface` contract suite against this
/// adapter. Empty findings = pass: the snapshot echoes the seeded
/// runtime state exactly (every escalation, invocation, unit, denial,
/// and residual visible — nothing dropped, nothing invented), and the
/// offered actions cover the §20.2 endpoints for the units shown.
#[test]
fn the_json_surface_passes_the_unmodified_shared_contract_suite() {
    let findings = operator_surface_suite(&MemorySurfaceFactory);
    assert!(
        findings.is_empty(),
        "the json surface must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}
