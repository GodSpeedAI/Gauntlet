//! The synthetic snapshot source: an `OperatorSurface` over an owned
//! [`RuntimeSnapshot`]. It is the suite's injectable constructor (the P8
//! pattern): `operator_surface_suite`'s echo case builds a surface over
//! a seeded snapshot and requires the exact projection back — nothing
//! dropped, nothing invented. It is also the adapter's own unit-test
//! source. It fronts no store: `snapshot` clones what it was built with.

use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::operator_surface::{
    OperatorAction, OperatorSurface, OperatorSurfaceError, RuntimeSnapshot,
};

/// The echo surface: `snapshot` reproduces the seeded snapshot exactly.
#[derive(Debug, Clone)]
pub struct MemorySurface {
    seeded: RuntimeSnapshot,
}

impl MemorySurface {
    /// Builds the surface over the seeded runtime state.
    pub fn new(seeded: RuntimeSnapshot) -> Self {
        Self { seeded }
    }
}

impl OperatorSurface for MemorySurface {
    fn snapshot(&self, _deadline: Deadline) -> Result<RuntimeSnapshot, OperatorSurfaceError> {
        Ok(self.seeded.clone())
    }

    fn actions(&self) -> Vec<OperatorAction> {
        let mut actions = vec![OperatorAction::Refresh];
        for unit in &self.seeded.units {
            actions.push(OperatorAction::ResolveUnit {
                unit: unit.id.clone(),
            });
            actions.push(OperatorAction::RetryUnit {
                unit: unit.id.clone(),
            });
        }
        actions
    }
}
