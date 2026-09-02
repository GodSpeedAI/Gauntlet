//! The durable operator surface (plan Task 21, charter D6): the JSON
//! surface adapter's seams implemented over the EXISTING durable reads
//! only — the same reads the CLI's `status` / `inspect` / `escalations`
//! commands use (`open_operator_surface`, the fs escalation queue's
//! `list`, `resume`, `verification_records`, `settlements`, the frozen
//! answer key's tier facts). No new read port is invented; nothing
//! absent is presented as present (honest absence: an empty field means
//! nothing durable names it).
//!
//! The `run_derivation` disclosure (charter D6): the durable read
//! surface names NO run enumeration, so the active run is DERIVED from
//! the open escalation tickets' run ids — exactly one is the anchor;
//! several resolve to the greatest ULID (ULID order is time order — the
//! newest run); none is an honest absence. The rule AND the sources are
//! served with every snapshot, never a silent guess.
//!
//! `sandbox_denials` derives from queue tickets whose summary names the
//! sandbox vocabulary the runtime's own governance gate escalates with
//! ("sandbox policy ... failed validation", runner isolation below the
//! tier's rung). When nothing durable names a denial the field is EMPTY
//! — no denial log is invented.
//!
//! The action handler is the ONE operator-action path
//! ([`crate::operator_action::execute_operator_action`], charter D3) —
//! the settlement service's gate + the governed chain's re-mint + the
//! same §16.3 commit the automatic path uses; no settlement type crosses
//! the adapter seam.

use gauntlet_adapter_surface_json::action::{
    ActionAccepted, ActionCommand, ActionFailure, ActionKind, ActionOutcome, ActionRefusal,
    OperatorActionHandler, SnapshotDisclosure,
};
use gauntlet_adapter_surface_json::views::{
    BudgetSpentFacts, RunScopedError, RunScopedReader, RunView, SettlementFact, UnitView,
    VerificationFact,
};
use gauntlet_domain::{MicroUsd, RunId, UnitId, UnitStatus};
use gauntlet_ports::budget_meter::Remaining;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::operator_surface::{
    OperatorAction, OperatorSurface, OperatorSurfaceError, RuntimeSnapshot, UnitSnapshot,
};

use crate::compose::{self, open_operator_surface};
use crate::context::RuntimeContext;
use crate::operator_action::{self, ActionDecision};

/// The durable surface: the read-side composition (`open_operator_surface`)
/// plus the runtime context the action path composes over.
#[derive(Clone)]
pub struct DurableSurface {
    context: RuntimeContext,
    surface: compose::OperatorSurface,
}

impl DurableSurface {
    /// Opens the surface over the resolved context; the same typed
    /// refusal as the read commands when no run state exists.
    pub fn open(context: RuntimeContext) -> Result<Self, String> {
        let surface = open_operator_surface(&context).map_err(|error| error.to_string())?;
        Ok(Self { context, surface })
    }

    /// The queue's listing, freshly read (the queue is the operator's
    /// inbox; every read re-opens it).
    fn queue(&self) -> Result<gauntlet_adapter_escalation_fs::FsEscalationQueue, String> {
        gauntlet_adapter_escalation_fs::FsEscalationQueue::open(&self.surface.escalation_root)
            .map_err(|error| error.to_string())
    }

    /// The open tickets' records (the inbox) and every record whose
    /// summary names the sandbox vocabulary (the denials view).
    fn inbox(
        &self,
    ) -> Result<Vec<gauntlet_adapter_escalation_fs::EscalationRecord>, OperatorSurfaceError> {
        let queue = self
            .queue()
            .map_err(|error| OperatorSurfaceError::SnapshotFailed { detail: error })?;
        let listing = queue
            .list()
            .map_err(|error| OperatorSurfaceError::SnapshotFailed {
                detail: error.to_string(),
            })?;
        Ok(listing
            .records
            .into_iter()
            .filter(|record| record.is_open())
            .collect())
    }

    /// The D6 run derivation over the open tickets (disclosed, never a
    /// silent guess).
    fn derive_run(
        &self,
        open: &[gauntlet_adapter_escalation_fs::EscalationRecord],
    ) -> Option<RunId> {
        let mut runs: Vec<RunId> = open.iter().map(|record| record.run_id.clone()).collect();
        runs.sort();
        runs.dedup();
        runs.pop()
    }

    /// The frozen ruler's units with their durable statuses (tier facts
    /// from the FROZEN key; status from the checkpoint projection).
    fn units(
        &self,
        checkpoint: Option<&gauntlet_ports::state_store::Checkpoint>,
    ) -> Vec<UnitSnapshot> {
        let Ok(key_text) = std::fs::read_to_string(&self.surface.answer_key) else {
            return Vec::new();
        };
        let parsed = gauntlet_app::reference::parse::parse_answer_key(&key_text);
        parsed
            .units
            .iter()
            .map(|unit| {
                let status = checkpoint
                    .and_then(|checkpoint| {
                        checkpoint
                            .unit_states
                            .iter()
                            .find(|(id, _)| id == unit.id())
                            .map(|(_, status)| *status)
                    })
                    .unwrap_or(UnitStatus::Open);
                UnitSnapshot {
                    id: unit.id().clone(),
                    status,
                    tier: unit.blast_radius(),
                }
            })
            .collect()
    }

    /// The checkpoint of the derived (or given) run, freshly read.
    fn checkpoint(
        &self,
        run: &RunId,
        deadline: Deadline,
    ) -> Result<Option<gauntlet_ports::state_store::Checkpoint>, OperatorSurfaceError> {
        self.surface.state.resume(run, deadline).map_err(|error| {
            OperatorSurfaceError::SnapshotFailed {
                detail: error.to_string(),
            }
        })
    }
}

impl OperatorSurface for DurableSurface {
    /// The runtime snapshot in the port's §20.2 priority order: the
    /// escalation queue first, then in-flight, the frozen-key completion
    /// view, budget burn, sandbox denials, residuals.
    fn snapshot(&self, deadline: Deadline) -> Result<RuntimeSnapshot, OperatorSurfaceError> {
        let open = self.inbox()?;
        let escalated = open.iter().map(|record| record.ticket.clone()).collect();
        let run = self.derive_run(&open);
        let checkpoint = match &run {
            Some(run) => self.checkpoint(run, deadline)?,
            None => None,
        };
        let in_flight = checkpoint
            .as_ref()
            .map(|checkpoint| {
                checkpoint
                    .in_flight
                    .iter()
                    .map(|(invocation, _)| invocation.clone())
                    .collect()
            })
            .unwrap_or_default();
        let units = self.units(checkpoint.as_ref());
        let budget = match (&run, checkpoint.as_ref()) {
            (Some(run), Some(checkpoint)) => {
                // Headroom per dimension: the run's own ceilings minus
                // the checkpoint's spend (concurrency against the
                // checkpoint's in-flight count; depth has no durable
                // spend to subtract).
                let run = self
                    .surface
                    .state
                    .load_run(run, deadline)
                    .map_err(|error| OperatorSurfaceError::SnapshotFailed {
                        detail: error.to_string(),
                    })?;
                let spent = &checkpoint.budget_spent;
                match run {
                    Some(run) => {
                        let limits = run.budget().limits();
                        Remaining {
                            rounds: limits.max_rounds().saturating_sub(spent.rounds()),
                            parallel: limits
                                .max_parallel()
                                .saturating_sub(checkpoint.in_flight.len() as u32),
                            depth: limits.max_depth(),
                            wall_seconds: limits
                                .max_wall_seconds()
                                .saturating_sub(spent.wall_seconds()),
                            tokens: limits.max_tokens().saturating_sub(spent.tokens()),
                            cost_usd: MicroUsd::new(
                                limits
                                    .max_cost_usd()
                                    .get()
                                    .saturating_sub(spent.cost_usd().get()),
                            ),
                        }
                    }
                    None => Remaining {
                        rounds: 0,
                        parallel: 0,
                        depth: 0,
                        wall_seconds: 0,
                        tokens: 0,
                        cost_usd: MicroUsd::new(0),
                    },
                }
            }
            _ => Remaining {
                rounds: 0,
                parallel: 0,
                depth: 0,
                wall_seconds: 0,
                tokens: 0,
                cost_usd: MicroUsd::new(0),
            },
        };
        // Sandbox denials: the tickets whose summary names the runtime's
        // own sandbox-refusal vocabulary; EMPTY when nothing names one.
        let queue = self
            .queue()
            .map_err(|error| OperatorSurfaceError::SnapshotFailed { detail: error })?;
        let listing = queue
            .list()
            .map_err(|error| OperatorSurfaceError::SnapshotFailed {
                detail: error.to_string(),
            })?;
        let sandbox_denials = listing
            .records
            .iter()
            .filter(|record| record.summary.contains("sandbox"))
            .map(|record| record.summary.clone())
            .collect();
        // Open residuals: no durable read in the read surface names
        // residuals — the field stays EMPTY (honest absence, charter D6).
        let open_residuals = Vec::new();
        Ok(RuntimeSnapshot {
            run,
            escalated,
            in_flight,
            units,
            budget,
            sandbox_denials,
            open_residuals,
        })
    }

    /// The actions the surface offers: refresh, and resolve/retry for
    /// every unit the frozen ruler shows.
    fn actions(&self) -> Vec<OperatorAction> {
        let mut actions = vec![OperatorAction::Refresh];
        for unit in self.units(None) {
            actions.push(OperatorAction::ResolveUnit {
                unit: unit.id.clone(),
            });
            actions.push(OperatorAction::RetryUnit { unit: unit.id });
        }
        actions
    }
}

impl SnapshotDisclosure for DurableSurface {
    fn run_derivation(&self) -> String {
        match self.inbox() {
            Ok(open) => {
                let mut runs: Vec<String> = open
                    .iter()
                    .map(|record| record.run_id.to_string())
                    .collect();
                runs.sort();
                runs.dedup();
                if runs.is_empty() {
                    "derived from the open escalation tickets of the durable queue: none are \
                     open, so no active run is derived"
                        .to_owned()
                } else {
                    format!(
                        "derived from the {} open escalation ticket(s) of the durable queue; \
                         the newest run ULID anchors the view: {}",
                        open.len(),
                        runs.join(", ")
                    )
                }
            }
            Err(error) => format!("the queue read failed, so no run was derived: {error}"),
        }
    }
}

impl RunScopedReader for DurableSurface {
    fn run_view(&self, run: &RunId, deadline: Deadline) -> Result<RunView, RunScopedError> {
        let loaded = self
            .surface
            .state
            .load_run(run, deadline)
            .map_err(|error| RunScopedError::ReadFailed {
                detail: error.to_string(),
            })?
            .ok_or_else(|| RunScopedError::NotFound {
                detail: format!("run {run} is not durable in this state store"),
            })?;
        let _ = loaded;
        let checkpoint = self.surface.state.resume(run, deadline).map_err(|error| {
            RunScopedError::ReadFailed {
                detail: error.to_string(),
            }
        })?;
        let units = self.units(checkpoint.as_ref());
        let records = self
            .surface
            .state
            .verification_records(run, deadline)
            .map_err(|error| RunScopedError::ReadFailed {
                detail: error.to_string(),
            })?;
        let verifications = records
            .iter()
            .map(|record| VerificationFact {
                unit_id: record.unit_id().to_string(),
                outcome: record.outcome().to_string(),
                evidence: record.evidence().to_string(),
                verification: record.verification().to_string(),
            })
            .collect();
        let rows = self
            .surface
            .state
            .settlements(run, deadline)
            .map_err(|error| RunScopedError::ReadFailed {
                detail: error.to_string(),
            })?;
        let settlements = rows
            .iter()
            .map(|row| SettlementFact {
                unit_id: row.unit_id().to_string(),
                verification: row.verification().to_string(),
                evidence: row.evidence().to_string(),
            })
            .collect();
        let queue = self
            .queue()
            .map_err(|error| RunScopedError::ReadFailed { detail: error })?;
        let listing = queue.list().map_err(|error| RunScopedError::ReadFailed {
            detail: error.to_string(),
        })?;
        let open_tickets = listing
            .records
            .iter()
            .filter(|record| record.is_open() && record.run_id == *run)
            .map(|record| record.ticket.to_string())
            .collect();
        let budget_spent = checkpoint.as_ref().map(|checkpoint| {
            let spent = &checkpoint.budget_spent;
            BudgetSpentFacts {
                rounds: spent.rounds(),
                wall_seconds: spent.wall_seconds(),
                tokens: spent.tokens(),
                cost_usd_micro: spent.cost_usd().get(),
            }
        });
        Ok(RunView {
            iteration: checkpoint
                .as_ref()
                .map(|checkpoint| checkpoint.iteration_no.get()),
            units,
            open_tickets,
            verifications,
            settlements,
            budget_spent,
        })
    }

    fn unit_view(
        &self,
        run: &RunId,
        unit: &UnitId,
        deadline: Deadline,
    ) -> Result<UnitView, RunScopedError> {
        // The run must be durable (the unit reads are run-anchored).
        let checkpoint = self.surface.state.resume(run, deadline).map_err(|error| {
            RunScopedError::ReadFailed {
                detail: error.to_string(),
            }
        })?;
        let records = self
            .surface
            .state
            .verification_records(run, deadline)
            .map_err(|error| RunScopedError::ReadFailed {
                detail: error.to_string(),
            })?;
        let verifications = records
            .iter()
            .filter(|record| record.unit_id() == unit)
            .map(|record| VerificationFact {
                unit_id: record.unit_id().to_string(),
                outcome: record.outcome().to_string(),
                evidence: record.evidence().to_string(),
                verification: record.verification().to_string(),
            })
            .collect();
        let rows = self
            .surface
            .state
            .settlements(run, deadline)
            .map_err(|error| RunScopedError::ReadFailed {
                detail: error.to_string(),
            })?;
        let settled = rows.iter().any(|row| row.unit_id() == unit);
        let snapshot_unit = self
            .units(checkpoint.as_ref())
            .into_iter()
            .find(|candidate| &candidate.id == unit);
        Ok(UnitView {
            unit: snapshot_unit,
            verifications,
            settled,
        })
    }
}

impl OperatorActionHandler for DurableSurface {
    /// The ONE operator-action path (charter D3): the composition root's
    /// shared execution over the settlement service. The identity is
    /// validated here through the service's typed constructor (D7) so
    /// missing/blank identities refuse BEFORE any store is opened.
    fn perform(&self, command: ActionCommand, deadline: Deadline) -> ActionOutcome {
        let kind = match command.kind {
            ActionKind::Resolve => gauntlet_adapter_escalation_fs::OperatorAction::Resolve,
            ActionKind::Retry => gauntlet_adapter_escalation_fs::OperatorAction::Retry,
        };
        let Ok(unit) = UnitId::new(&command.unit) else {
            return ActionOutcome::Failed(ActionFailure {
                code: "unit_unparsable".to_owned(),
                message: format!("{} is not a valid unit id", command.unit),
            });
        };
        let identity = match gauntlet_app::settle::OperatorIdentity::new(&command.operator) {
            Ok(identity) => identity,
            Err(error) => {
                return ActionOutcome::Refused(ActionRefusal {
                    code: "operator_identity_required".to_owned(),
                    message: error.to_string(),
                });
            }
        };
        let run_override = match &command.run {
            Some(run) => match RunId::new(run) {
                Ok(run_id) => Some(run_id),
                Err(error) => {
                    return ActionOutcome::Failed(ActionFailure {
                        code: "run_unparsable".to_owned(),
                        message: error.to_string(),
                    });
                }
            },
            None => None,
        };
        let decision = operator_action::execute_operator_action(
            &self.context,
            kind,
            None,
            &unit,
            &identity,
            &command.note,
            run_override,
            deadline,
        );
        match decision {
            Ok(ActionDecision::Accepted(acceptance)) => ActionOutcome::Accepted(ActionAccepted {
                run_id: acceptance.run_id.to_string(),
                unit_id: acceptance.unit.to_string(),
                tickets_resolved: acceptance.tickets_resolved,
                committed_now: acceptance.committed_now,
            }),
            Ok(ActionDecision::Refused(refusal)) => ActionOutcome::Refused(ActionRefusal {
                code: refusal.code,
                message: refusal.message,
            }),
            Err(detail) => ActionOutcome::Failed(ActionFailure {
                code: "action_failed".to_owned(),
                message: detail,
            }),
        }
    }
}
