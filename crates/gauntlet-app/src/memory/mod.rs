//! Developmental memory and the capability ledger (plan Task 22; spec
//! §19.1–19.2): the application services over the
//! [`CapabilityLedger`](gauntlet_ports::capability_ledger::CapabilityLedger)
//! port.
//!
//! - [`developmental`] — the import that projects a run's durable
//!   substrate into the observation log. Ungated and idempotent: §19.1's
//!   left column is "Raw. Immediate, append-only", so failures and
//!   INCONCLUSIVEs are memory with the same eagerness as successes.
//! - [`promotion`] — the ONLY composer of elevating status transitions:
//!   the §19.2 gate with per-condition typed refusals (§19.4: insufficient
//!   evidence ⟹ provisional ⟹ not promoted) and the sticky revoke.
//!
//! Both services are read-only over the run substrate: the import reads
//! the `StateStore` through its typed reads and never writes it, so the
//! settlement path (`commit_settlement`, the earned authority) is untouched
//! by this slice.

pub mod developmental;
pub mod promotion;

/// Test doubles for the module's unit tests: a std-only in-memory ledger
/// (the reference semantics the ports crate's contract suite pins), a
/// fixed clock, and nothing else — the promotion service's conditions are
/// proven against the REAL adapter by the adapter's contract leg and the
/// composed p10 proof; these doubles exist so a condition can be broken in
/// isolation.
#[cfg(test)]
pub(crate) mod testing {
    use std::sync::Mutex;
    use std::sync::atomic::{AtomicU64, Ordering};

    use gauntlet_domain::{CapabilityId, CapabilityStatus, EpochSeconds};
    use gauntlet_ports::capability_ledger::{
        CapabilityEntry, CapabilityLedger, CapabilityLedgerError, ObservationAppend,
        ObservationRecord, StatusTransition, TransitionReason, promotion_witness_is_rooted,
    };
    use gauntlet_ports::clock::{Clock, ClockError};
    use gauntlet_ports::deadline::Deadline;

    /// The module tests' fixed instant.
    pub const FIXED_INSTANT: u64 = 1_700_000_000;

    /// A clock frozen at the module's fixed instant.
    #[derive(Debug)]
    pub struct FixedClock;

    impl Clock for FixedClock {
        fn now(&self) -> EpochSeconds {
            EpochSeconds::new(FIXED_INSTANT)
        }

        fn sleep_until(&self, _at: EpochSeconds) -> Result<(), ClockError> {
            Ok(())
        }
    }

    /// The deadline the tests bind with.
    pub fn bound() -> Deadline {
        Deadline::at_epoch(EpochSeconds::new(FIXED_INSTANT + 60))
    }

    struct Stored {
        sequence: u64,
        record: ObservationRecord,
    }

    /// The in-memory ledger: the identity-keyed idempotent observation log
    /// and the transition-derived register, exactly as the port pins them.
    /// Locks instead of cells so the double is `Send + Sync` and can ride
    /// the services' `Arc<dyn CapabilityLedger>` shape.
    #[derive(Default)]
    pub struct MemoryLedger {
        observations: Mutex<Vec<Stored>>,
        transitions: Mutex<Vec<StatusTransition>>,
        sequence: AtomicU64,
    }

    impl MemoryLedger {
        pub fn new() -> Self {
            Self::default()
        }

        fn next_sequence(&self) -> u64 {
            self.sequence.fetch_add(1, Ordering::Relaxed) + 1
        }

        fn current_status(&self, capability: &CapabilityId) -> Option<CapabilityStatus> {
            self.transitions
                .lock()
                .expect("the ledger lock is not poisoned")
                .iter()
                .rev()
                .find(|transition| transition.capability() == capability)
                .map(|transition| transition.to())
                .or(Some(CapabilityStatus::Observed))
        }

        fn entry_of(&self, capability: &CapabilityId) -> Option<CapabilityEntry> {
            let registered_at = self
                .observations
                .lock()
                .expect("the ledger lock is not poisoned")
                .iter()
                .find(|stored| stored.record.identity().capability() == capability)
                .map(|stored| stored.record.at())?;
            let last_transition_at = self
                .transitions
                .lock()
                .expect("the ledger lock is not poisoned")
                .iter()
                .rev()
                .find(|transition| transition.capability() == capability)
                .map(|transition| transition.at());
            Some(CapabilityEntry {
                capability: capability.clone(),
                status: self
                    .current_status(capability)
                    .expect("observed capabilities are registered"),
                registered_at,
                last_transition_at,
            })
        }
    }

    impl CapabilityLedger for MemoryLedger {
        fn append_observation(
            &self,
            observation: ObservationRecord,
            _deadline: Deadline,
        ) -> Result<ObservationAppend, CapabilityLedgerError> {
            let identity = observation.identity().clone();
            let mut observations = self
                .observations
                .lock()
                .expect("the ledger lock is not poisoned");
            if let Some(stored) = observations
                .iter()
                .find(|stored| stored.record.identity() == &identity)
            {
                if stored.record.facts_agree(&observation) {
                    return Ok(ObservationAppend {
                        sequence: stored.sequence,
                        idempotent: true,
                    });
                }
                return Err(CapabilityLedgerError::ObservationConflict { identity });
            }
            let sequence = self.next_sequence();
            observations.push(Stored {
                sequence,
                record: observation,
            });
            Ok(ObservationAppend {
                sequence,
                idempotent: false,
            })
        }

        fn observations(
            &self,
            capability: &CapabilityId,
            _deadline: Deadline,
        ) -> Result<Vec<ObservationRecord>, CapabilityLedgerError> {
            Ok(self
                .observations
                .lock()
                .expect("the ledger lock is not poisoned")
                .iter()
                .filter(|stored| stored.record.identity().capability() == capability)
                .map(|stored| stored.record.clone())
                .collect())
        }

        fn capability(
            &self,
            capability: &CapabilityId,
            _deadline: Deadline,
        ) -> Result<Option<CapabilityEntry>, CapabilityLedgerError> {
            Ok(self.entry_of(capability))
        }

        fn apply_transition(
            &self,
            transition: StatusTransition,
            deadline: Deadline,
        ) -> Result<CapabilityEntry, CapabilityLedgerError> {
            let capability = transition.capability().clone();
            let known = self
                .observations
                .lock()
                .expect("the ledger lock is not poisoned")
                .iter()
                .any(|stored| stored.record.identity().capability() == &capability);
            if !known {
                return Err(CapabilityLedgerError::UnknownCapability { capability });
            }
            let current = self
                .current_status(&capability)
                .expect("observed capabilities are registered");
            if transition.from() != current {
                return Err(CapabilityLedgerError::StaleTransition {
                    capability,
                    current,
                });
            }
            // Log-rooting (COR-1 C3), as the port pins it: an elevating
            // transition's witnesses must be Settlement memories in THIS
            // log — the double keeps the reference semantics the shared
            // suite enforces on every leg.
            if let TransitionReason::Promoted(evidence) = transition.reason() {
                let log: Vec<ObservationRecord> = self
                    .observations
                    .lock()
                    .expect("the ledger lock is not poisoned")
                    .iter()
                    .filter(|stored| stored.record.identity().capability() == &capability)
                    .map(|stored| stored.record.clone())
                    .collect();
                for witness in &evidence.settlements {
                    if !promotion_witness_is_rooted(&log, &capability, witness) {
                        return Err(CapabilityLedgerError::UnrootedPromotion {
                            capability,
                            witness: witness.clone(),
                        });
                    }
                }
            }
            self.transitions
                .lock()
                .expect("the ledger lock is not poisoned")
                .push(transition);
            Ok(self
                .capability(&capability, deadline)?
                .expect("the capability just transitioned"))
        }

        fn transitions(
            &self,
            capability: &CapabilityId,
            _deadline: Deadline,
        ) -> Result<Vec<StatusTransition>, CapabilityLedgerError> {
            Ok(self
                .transitions
                .lock()
                .expect("the ledger lock is not poisoned")
                .iter()
                .filter(|transition| transition.capability() == capability)
                .cloned()
                .collect())
        }
    }
}
