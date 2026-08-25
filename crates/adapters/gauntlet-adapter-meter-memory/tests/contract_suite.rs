//! The shared BudgetMeter contract suite (spec §7.3) runs against this
//! adapter through the UNMODIFIED suite entry point — "Adding an adapter
//! means implementing the trait and running the existing suite, not
//! writing a bespoke one." The suite lives in
//! `gauntlet_ports::contract::budget_meter`; this file only supplies the
//! factory the suite is parameterized over.

use gauntlet_adapter_meter_memory::MeteredBudget;
use gauntlet_domain::{Budget, DiagnosticAllowance, Limits};
use gauntlet_ports::budget_meter::BudgetMeter;
use gauntlet_ports::contract::budget_meter::{BudgetMeterFactory, budget_meter_suite};

struct MeteredBudgetFactory;

impl BudgetMeterFactory for MeteredBudgetFactory {
    fn label(&self) -> &'static str {
        "metered-budget"
    }

    fn build(
        &self,
        limits: Limits,
        recovery_reserve_pct: u8,
        diagnostic: DiagnosticAllowance,
    ) -> Result<Box<dyn BudgetMeter>, String> {
        let budget =
            Budget::new(limits, recovery_reserve_pct, diagnostic).map_err(|e| e.to_string())?;
        Ok(Box::new(MeteredBudget::new(budget)))
    }
}

#[test]
fn metered_budget_passes_the_shared_budget_meter_suite() {
    let findings = budget_meter_suite(&MeteredBudgetFactory);
    assert!(
        findings.is_empty(),
        "the metered budget must pass the shared suite:\n{}",
        findings
            .iter()
            .map(|finding| finding.to_string())
            .collect::<Vec<_>>()
            .join("\n")
    );
}
