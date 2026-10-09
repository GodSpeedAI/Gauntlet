//! The PostgreSQL `InvocationGraph` (E2b): the Gauntlet-owned lineage of role
//! invocations, stored in the same database and transactions as run state.
//!
//! Every trait method is ONE transaction. Refusals are decided against rows
//! read after the serializing lock is taken, so concurrent admissions under
//! one parent cannot both pass the child-count and budget checks:
//!
//! - admission locks the parent row (`FOR UPDATE`); root admission takes a
//!   per-run advisory transaction lock;
//! - `UNIQUE (run_id, admission_key)` plus `ON CONFLICT DO NOTHING` makes a
//!   replayed key an idempotent `AlreadyAdmitted` (or a refusal when the
//!   facts differ), even across parents;
//! - the refusal order is exactly the in-memory reference's: key replay,
//!   invocation-id reuse, then placement (root, parent, run, terminal, depth,
//!   child limit, budget).
//!
//! Decoding never silently defaults: any stored value that does not decode
//! is [`GraphError::Corrupt`].

use std::fmt::Display;
use std::path::Path;
use std::sync::Mutex;
use std::time::{SystemTime, UNIX_EPOCH};

use gauntlet_domain::{
    AdmissionKey, BudgetDimension, Digest, EpochSeconds, HarnessId, InvocationId, MicroUsd,
    ResultContractPath, RoleId, RunId, RunnerProfileId, SessionRef, WorkspaceRef,
};
use gauntlet_ports::agent_runner::InvocationFailure;
use gauntlet_ports::budget_meter::BudgetSlice;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::invocation_graph::{
    AdmissionOutcome, AdmissionRefusal, AdmissionRequest, GraphError, InvocationGraph,
    InvocationNode, NativeSessionBinding, NodeStatus, ResultBinding,
};
use postgres::types::ToSql;
use postgres::{Client, Row, Transaction, error::SqlState};

use crate::db::PostgresConfig;
use crate::migrations;

/// Every column of `invocation_nodes`, in the order `decode_node` reads them
/// (by name, so the order is cosmetic).
const NODE_COLUMNS: &str = "invocation_id, run_id, parent_id, root_id, depth, role, \
    runner_profile, harness, workspace, context_digest, result_contract, admission_key, \
    budget_rounds, budget_wall_seconds, budget_tokens, budget_cost_micro_usd, status, \
    failure_class, session_ref, session_epoch, result_digest, admitted_at, terminal_at";

/// The largest `statement_timeout` PostgreSQL accepts, in milliseconds.
const MAX_STATEMENT_TIMEOUT_MS: u64 = 2_147_483_647;

/// The PostgreSQL-backed invocation graph. One connection behind a mutex;
/// the port is synchronous, so each call holds the connection for its one
/// transaction.
pub struct PostgresInvocationGraph {
    client: Mutex<Client>,
}

impl PostgresInvocationGraph {
    /// Opens the graph of the state instance at `path` (see [`crate::locator`]).
    pub fn open_at(path: &Path) -> Result<Self, GraphError> {
        Self::open(&PostgresConfig::for_state_path(path))
    }

    /// Opens (creating if needed) the graph, applying any outstanding migrations.
    pub fn open(config: &PostgresConfig) -> Result<Self, GraphError> {
        let mut client =
            crate::db::connect(config).map_err(|detail| GraphError::Unavailable { detail })?;
        migrations::apply(&mut client).map_err(|err| GraphError::Unavailable {
            detail: err.to_string(),
        })?;
        Ok(Self {
            client: Mutex::new(client),
        })
    }

    /// Runs `op` inside one transaction. An `Err` from `op` drops the
    /// transaction uncommitted, so a refused or failed call leaves no trace.
    fn run<T>(
        &self,
        deadline: Deadline,
        op: impl FnOnce(&mut Transaction<'_>) -> Result<T, GraphError>,
    ) -> Result<T, GraphError> {
        let mut client = self.client.lock().map_err(|_| GraphError::Unavailable {
            detail: "the graph connection is poisoned".to_owned(),
        })?;
        let mut tx = client.transaction().map_err(write_err)?;
        set_statement_timeout(&mut tx, deadline)?;
        let value = op(&mut tx)?;
        tx.commit().map_err(write_err)?;
        Ok(value)
    }
}

impl InvocationGraph for PostgresInvocationGraph {
    fn admit(
        &self,
        request: AdmissionRequest,
        deadline: Deadline,
    ) -> Result<AdmissionOutcome, GraphError> {
        self.run(deadline, |tx| admit_in(tx, &request))
    }

    fn attach_native_session(
        &self,
        id: &InvocationId,
        session: SessionRef,
        deadline: Deadline,
    ) -> Result<NativeSessionBinding, GraphError> {
        self.run(deadline, |tx| {
            let node = load_for_update(tx, id)?;
            if node.status.is_terminal() {
                return Err(GraphError::IllegalTransition {
                    id: id.clone(),
                    from: node.status,
                    to: node.status,
                });
            }
            let epoch = match node.native_session.as_ref() {
                None => 1,
                Some(bound) => {
                    bound
                        .epoch
                        .checked_add(1)
                        .ok_or_else(|| GraphError::WriteFailed {
                            detail: "native session epoch exhausted".to_owned(),
                        })?
                }
            };
            let epoch_db = i64::from(epoch);
            tx.execute(
                "UPDATE invocation_nodes SET session_ref = $2, session_epoch = $3 \
                 WHERE invocation_id = $1",
                &[&id.as_str(), &session.as_str(), &epoch_db],
            )
            .map_err(write_err)?;
            Ok(NativeSessionBinding { session, epoch })
        })
    }

    fn mark_running(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<InvocationNode, GraphError> {
        self.run(deadline, |tx| {
            let node = load_for_update(tx, id)?;
            if node.status != NodeStatus::Admitted {
                return Err(GraphError::IllegalTransition {
                    id: id.clone(),
                    from: node.status,
                    to: NodeStatus::Running,
                });
            }
            update_node(
                tx,
                "UPDATE invocation_nodes SET status = 'running' WHERE invocation_id = $1",
                &[&id.as_str()],
            )
        })
    }

    fn complete(
        &self,
        id: &InvocationId,
        binding: ResultBinding,
        at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<InvocationNode, GraphError> {
        self.run(deadline, |tx| {
            let node = load_for_update(tx, id)?;
            if node.status.is_terminal() {
                return Err(GraphError::IllegalTransition {
                    id: id.clone(),
                    from: node.status,
                    to: NodeStatus::Completed,
                });
            }
            if binding.contract_path != node.result_contract
                || binding.context_digest != node.context_digest
            {
                return Err(GraphError::ContractMismatch { id: id.clone() });
            }
            let at_db = to_i64(at.get(), "completion instant")?;
            let digest = binding.result_digest.to_string();
            update_node(
                tx,
                "UPDATE invocation_nodes SET status = 'completed', terminal_at = $2, \
                 result_digest = $3 WHERE invocation_id = $1",
                &[&id.as_str(), &at_db, &digest],
            )
        })
    }

    fn fail(
        &self,
        id: &InvocationId,
        class: InvocationFailure,
        at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<InvocationNode, GraphError> {
        self.run(deadline, |tx| {
            let node = load_for_update(tx, id)?;
            if node.status.is_terminal() {
                return Err(GraphError::IllegalTransition {
                    id: id.clone(),
                    from: node.status,
                    to: NodeStatus::Failed(class),
                });
            }
            let at_db = to_i64(at.get(), "failure instant")?;
            let token = class.to_string();
            update_node(
                tx,
                "UPDATE invocation_nodes SET status = 'failed', failure_class = $2, \
                 terminal_at = $3 WHERE invocation_id = $1",
                &[&id.as_str(), &token, &at_db],
            )
        })
    }

    fn time_out(
        &self,
        id: &InvocationId,
        at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<InvocationNode, GraphError> {
        self.run(deadline, |tx| {
            let node = load_for_update(tx, id)?;
            if node.status.is_terminal() {
                return Err(GraphError::IllegalTransition {
                    id: id.clone(),
                    from: node.status,
                    to: NodeStatus::TimedOut,
                });
            }
            let at_db = to_i64(at.get(), "timeout instant")?;
            update_node(
                tx,
                "UPDATE invocation_nodes SET status = 'timed_out', terminal_at = $2 \
                 WHERE invocation_id = $1",
                &[&id.as_str(), &at_db],
            )
        })
    }

    fn cancel_subtree(
        &self,
        id: &InvocationId,
        at: EpochSeconds,
        deadline: Deadline,
    ) -> Result<Vec<InvocationId>, GraphError> {
        self.run(deadline, |tx| {
            let root = load_for_update(tx, id)?;
            // Walk the subtree top-down, locking each child row as it is
            // read. An admission under any node must lock that node first,
            // so it either committed before this read or waits for us.
            let mut collected: Vec<(i64, InvocationNode)> = Vec::new();
            let mut stack = vec![id.as_str().to_owned()];
            while let Some(parent) = stack.pop() {
                let children = query_seq_nodes(
                    tx,
                    &format!(
                        "SELECT admission_seq, {NODE_COLUMNS} FROM invocation_nodes \
                         WHERE parent_id = $1 ORDER BY admission_seq FOR UPDATE"
                    ),
                    &[&parent],
                )?;
                for (seq, child) in children {
                    stack.push(child.invocation_id.as_str().to_owned());
                    collected.push((seq, child));
                }
            }
            // Depth first, then admission order: the `descendants` order.
            collected.sort_by_key(|(seq, node)| (node.depth, *seq));

            let mut targets: Vec<InvocationId> = Vec::new();
            if !root.status.is_terminal() {
                targets.push(root.invocation_id.clone());
            }
            targets.extend(
                collected
                    .into_iter()
                    .map(|(_, node)| node)
                    .filter(|node| !node.status.is_terminal())
                    .map(|node| node.invocation_id),
            );
            if !targets.is_empty() {
                let at_db = to_i64(at.get(), "cancellation instant")?;
                let ids: Vec<String> = targets.iter().map(|t| t.as_str().to_owned()).collect();
                tx.execute(
                    "UPDATE invocation_nodes SET status = 'cancelled', terminal_at = $2 \
                     WHERE invocation_id = ANY($1)",
                    &[&ids, &at_db],
                )
                .map_err(write_err)?;
            }
            Ok(targets)
        })
    }

    fn node(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<Option<InvocationNode>, GraphError> {
        self.run(deadline, |tx| fetch_node(tx, id.as_str()))
    }

    fn children(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<Vec<InvocationNode>, GraphError> {
        self.run(deadline, |tx| {
            require(tx, id)?;
            query_nodes(
                tx,
                &format!(
                    "SELECT {NODE_COLUMNS} FROM invocation_nodes WHERE parent_id = $1 \
                     ORDER BY admission_seq"
                ),
                &[&id.as_str()],
            )
        })
    }

    fn descendants(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<Vec<InvocationNode>, GraphError> {
        self.run(deadline, |tx| {
            require(tx, id)?;
            query_nodes(
                tx,
                &format!(
                    "WITH RECURSIVE sub(invocation_id) AS (\
                        SELECT c.invocation_id FROM invocation_nodes c WHERE c.parent_id = $1 \
                        UNION ALL \
                        SELECT n.invocation_id FROM invocation_nodes n \
                        JOIN sub s ON n.parent_id = s.invocation_id) \
                     SELECT {NODE_COLUMNS} FROM invocation_nodes \
                     WHERE invocation_id IN (SELECT invocation_id FROM sub) \
                     ORDER BY depth, admission_seq"
                ),
                &[&id.as_str()],
            )
        })
    }

    fn ancestors(
        &self,
        id: &InvocationId,
        deadline: Deadline,
    ) -> Result<Vec<InvocationNode>, GraphError> {
        self.run(deadline, |tx| {
            let mut current = require(tx, id)?;
            let mut out = Vec::new();
            while let Some(parent_id) = current.parent.clone() {
                let parent = fetch_node(tx, parent_id.as_str())?
                    .ok_or_else(|| corrupt(format!("the parent {parent_id} of {id} is missing")))?;
                out.push(parent.clone());
                current = parent;
            }
            Ok(out)
        })
    }

    fn active_nodes(
        &self,
        run_id: &RunId,
        deadline: Deadline,
    ) -> Result<Vec<InvocationNode>, GraphError> {
        self.run(deadline, |tx| {
            query_nodes(
                tx,
                &format!(
                    "SELECT {NODE_COLUMNS} FROM invocation_nodes \
                     WHERE run_id = $1 AND status IN ('admitted', 'running') \
                     ORDER BY admission_seq"
                ),
                &[&run_id.as_str()],
            )
        })
    }
}

/// The admission transaction. Refusal order mirrors the in-memory reference.
fn admit_in(
    tx: &mut Transaction<'_>,
    request: &AdmissionRequest,
) -> Result<AdmissionOutcome, GraphError> {
    // 1. Serialize. A child locks its parent row; a root takes the run's lane.
    let parent = match request.parent.as_ref() {
        Some(parent_id) => fetch_node_locked(tx, parent_id.as_str())?,
        None => {
            tx.query_one(
                "SELECT pg_advisory_xact_lock(hashtext($1))",
                &[&request.run_id.as_str()],
            )
            .map_err(write_err)?;
            None
        }
    };

    // 2. Replay of the same key: identical facts are AlreadyAdmitted, any
    //    difference is a refusal. This precedes every placement check.
    if let Some(existing) =
        fetch_by_key(tx, request.run_id.as_str(), request.admission_key.as_str())?
    {
        return Ok(replay_outcome(existing, request));
    }

    // 3. The invocation id is a global identity.
    if fetch_node(tx, request.invocation_id.as_str())?.is_some() {
        return Ok(AdmissionOutcome::Refused(
            AdmissionRefusal::InvocationIdReused,
        ));
    }

    // 4. Placement and budget against the locked state.
    let (root_id, depth) = match request.parent.as_ref() {
        None => {
            if root_exists(tx, request.run_id.as_str())? {
                return Ok(AdmissionOutcome::Refused(
                    AdmissionRefusal::RootAlreadyExists,
                ));
            }
            (request.invocation_id.clone(), 0)
        }
        Some(parent_id) => {
            let Some(parent) = parent else {
                return Ok(AdmissionOutcome::Refused(AdmissionRefusal::UnknownParent));
            };
            if parent.run_id != request.run_id {
                return Ok(AdmissionOutcome::Refused(AdmissionRefusal::WrongRun));
            }
            if parent.status.is_terminal() {
                return Ok(AdmissionOutcome::Refused(AdmissionRefusal::ParentTerminal));
            }
            let would_be = parent.depth.saturating_add(1);
            if would_be > request.limits.max_depth {
                return Ok(AdmissionOutcome::Refused(AdmissionRefusal::DepthLimit {
                    max: request.limits.max_depth,
                    would_be,
                }));
            }
            let live = live_child_budgets(tx, parent_id.as_str())?;
            let max_children =
                usize::try_from(request.limits.max_children_per_parent).unwrap_or(usize::MAX);
            if live.len() >= max_children {
                return Ok(AdmissionOutcome::Refused(AdmissionRefusal::ChildLimit {
                    max: request.limits.max_children_per_parent,
                }));
            }
            if let Err(dimension) = fits_budget(&parent.budget, &live, &request.budget) {
                return Ok(AdmissionOutcome::Refused(
                    AdmissionRefusal::BudgetExceeded { dimension },
                ));
            }
            (parent.root.clone(), would_be)
        }
    };

    // 5. Insert. A conflict here means a concurrent writer won a race the
    //    locks should have prevented; resolve it from the committed rows.
    let budget = &request.budget;
    let inv = request.invocation_id.as_str();
    let run = request.run_id.as_str();
    let parent_text: Option<&str> = request.parent.as_ref().map(|p| p.as_str());
    let root_text = root_id.as_str();
    let depth_db = i64::from(depth);
    let role = request.role.to_string();
    let profile = request.runner_profile.as_str();
    let harness = request.harness.as_str();
    let workspace = request.workspace.as_str();
    let digest = request.context_digest.to_string();
    let contract = request.result_contract.as_str();
    let key = request.admission_key.as_str();
    let rounds = i64::from(budget.rounds);
    let wall = to_i64(budget.wall_seconds, "wall-clock budget")?;
    let tokens = to_i64(budget.tokens, "token budget")?;
    let cost = to_i64(budget.cost_usd.get(), "cost budget")?;
    let admitted_at = to_i64(request.at.get(), "admission instant")?;
    let params: [&(dyn ToSql + Sync); 17] = [
        &inv,
        &run,
        &parent_text,
        &root_text,
        &depth_db,
        &role,
        &profile,
        &harness,
        &workspace,
        &digest,
        &contract,
        &key,
        &rounds,
        &wall,
        &tokens,
        &cost,
        &admitted_at,
    ];
    let inserted = tx
        .query_opt(
            &format!(
                "INSERT INTO invocation_nodes (invocation_id, run_id, parent_id, root_id, depth, \
                 role, runner_profile, harness, workspace, context_digest, result_contract, \
                 admission_key, budget_rounds, budget_wall_seconds, budget_tokens, \
                 budget_cost_micro_usd, status, failure_class, session_ref, session_epoch, \
                 result_digest, admitted_at, terminal_at) \
                 VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13, $14, $15, $16, \
                 'admitted', NULL, NULL, 0, NULL, $17, NULL) \
                 ON CONFLICT DO NOTHING RETURNING {NODE_COLUMNS}"
            ),
            &params,
        )
        .map_err(write_err)?;
    if let Some(row) = inserted {
        return Ok(AdmissionOutcome::Admitted(decode_node(&row)?));
    }
    if let Some(existing) = fetch_by_key(tx, run, key)? {
        return Ok(replay_outcome(existing, request));
    }
    if fetch_node(tx, inv)?.is_some() {
        return Ok(AdmissionOutcome::Refused(
            AdmissionRefusal::InvocationIdReused,
        ));
    }
    Err(GraphError::WriteFailed {
        detail: format!("admission of {inv} conflicted with no resolvable row"),
    })
}

/// The outcome of a request whose admission key is already bound.
fn replay_outcome(existing: InvocationNode, request: &AdmissionRequest) -> AdmissionOutcome {
    if same_facts(&existing, request) {
        AdmissionOutcome::AlreadyAdmitted(existing)
    } else {
        AdmissionOutcome::Refused(AdmissionRefusal::KeyReusedWithDifferentFacts)
    }
}

/// The immutable facts of a stored node match a request (same rule as the reference).
fn same_facts(existing: &InvocationNode, request: &AdmissionRequest) -> bool {
    existing.invocation_id == request.invocation_id
        && existing.run_id == request.run_id
        && existing.parent == request.parent
        && existing.role == request.role
        && existing.runner_profile == request.runner_profile
        && existing.harness == request.harness
        && existing.workspace == request.workspace
        && existing.context_digest == request.context_digest
        && existing.result_contract == request.result_contract
        && existing.admission_key == request.admission_key
        && existing.budget == request.budget
}

/// Componentwise budget rule over the live children, mirroring the reference:
/// saturating sums, so the rule can only over-reject, never over-admit.
fn fits_budget(
    parent: &BudgetSlice,
    live: &[BudgetSlice],
    slice: &BudgetSlice,
) -> Result<(), BudgetDimension> {
    let total = |pick: fn(&BudgetSlice) -> u64| {
        live.iter()
            .fold(0_u64, |acc, budget| acc.saturating_add(pick(budget)))
    };
    let checks = [
        (
            BudgetDimension::Rounds,
            u64::from(parent.rounds),
            total(|b| u64::from(b.rounds)).saturating_add(u64::from(slice.rounds)),
        ),
        (
            BudgetDimension::WallSeconds,
            parent.wall_seconds,
            total(|b| b.wall_seconds).saturating_add(slice.wall_seconds),
        ),
        (
            BudgetDimension::Tokens,
            parent.tokens,
            total(|b| b.tokens).saturating_add(slice.tokens),
        ),
        (
            BudgetDimension::CostUsd,
            parent.cost_usd.get(),
            total(|b| b.cost_usd.get()).saturating_add(slice.cost_usd.get()),
        ),
    ];
    for (dimension, cap, wanted) in checks {
        if wanted > cap {
            return Err(dimension);
        }
    }
    Ok(())
}

/// The budget slices of a parent's live (non-terminal) children.
fn live_child_budgets(
    tx: &mut Transaction<'_>,
    parent: &str,
) -> Result<Vec<BudgetSlice>, GraphError> {
    let rows = tx
        .query(
            "SELECT budget_rounds, budget_wall_seconds, budget_tokens, budget_cost_micro_usd \
             FROM invocation_nodes WHERE parent_id = $1 AND status IN ('admitted', 'running') \
             ORDER BY admission_seq",
            &[&parent],
        )
        .map_err(read_err)?;
    rows.iter().map(decode_budget).collect()
}

fn root_exists(tx: &mut Transaction<'_>, run_id: &str) -> Result<bool, GraphError> {
    Ok(tx
        .query_opt(
            "SELECT 1 FROM invocation_nodes WHERE run_id = $1 AND parent_id IS NULL",
            &[&run_id],
        )
        .map_err(read_err)?
        .is_some())
}

/// Reads one node by id (no lock).
fn fetch_node(tx: &mut Transaction<'_>, id: &str) -> Result<Option<InvocationNode>, GraphError> {
    tx.query_opt(
        &format!("SELECT {NODE_COLUMNS} FROM invocation_nodes WHERE invocation_id = $1"),
        &[&id],
    )
    .map_err(read_err)?
    .as_ref()
    .map(decode_node)
    .transpose()
}

/// Reads one node by id under a row lock (`FOR UPDATE`).
fn fetch_node_locked(
    tx: &mut Transaction<'_>,
    id: &str,
) -> Result<Option<InvocationNode>, GraphError> {
    tx.query_opt(
        &format!("SELECT {NODE_COLUMNS} FROM invocation_nodes WHERE invocation_id = $1 FOR UPDATE"),
        &[&id],
    )
    .map_err(read_err)?
    .as_ref()
    .map(decode_node)
    .transpose()
}

fn fetch_by_key(
    tx: &mut Transaction<'_>,
    run_id: &str,
    key: &str,
) -> Result<Option<InvocationNode>, GraphError> {
    tx.query_opt(
        &format!(
            "SELECT {NODE_COLUMNS} FROM invocation_nodes WHERE run_id = $1 AND admission_key = $2"
        ),
        &[&run_id, &key],
    )
    .map_err(read_err)?
    .as_ref()
    .map(decode_node)
    .transpose()
}

/// Reads one node that must exist, under a row lock.
fn load_for_update(
    tx: &mut Transaction<'_>,
    id: &InvocationId,
) -> Result<InvocationNode, GraphError> {
    fetch_node_locked(tx, id.as_str())?
        .ok_or_else(|| GraphError::UnknownInvocation { id: id.clone() })
}

/// Requires the node to exist (no lock).
fn require(tx: &mut Transaction<'_>, id: &InvocationId) -> Result<InvocationNode, GraphError> {
    fetch_node(tx, id.as_str())?.ok_or_else(|| GraphError::UnknownInvocation { id: id.clone() })
}

fn query_nodes(
    tx: &mut Transaction<'_>,
    sql: &str,
    params: &[&(dyn ToSql + Sync)],
) -> Result<Vec<InvocationNode>, GraphError> {
    tx.query(sql, params)
        .map_err(read_err)?
        .iter()
        .map(decode_node)
        .collect()
}

fn query_seq_nodes(
    tx: &mut Transaction<'_>,
    sql: &str,
    params: &[&(dyn ToSql + Sync)],
) -> Result<Vec<(i64, InvocationNode)>, GraphError> {
    tx.query(sql, params)
        .map_err(read_err)?
        .iter()
        .map(|row| Ok((int(row, "admission_seq")?, decode_node(row)?)))
        .collect()
}

/// Writes one status change to a locked node and returns the fresh row.
fn update_node(
    tx: &mut Transaction<'_>,
    sql: &str,
    params: &[&(dyn ToSql + Sync)],
) -> Result<InvocationNode, GraphError> {
    let row = tx
        .query_one(&format!("{sql} RETURNING {NODE_COLUMNS}"), params)
        .map_err(write_err)?;
    decode_node(&row)
}

/// Bounds the statement at the time left before the deadline. A deadline
/// already passed is a typed `Unavailable`, not a silent unbounded call.
fn set_statement_timeout(tx: &mut Transaction<'_>, deadline: Deadline) -> Result<(), GraphError> {
    let now = now_epoch()?;
    if deadline.exceeded_at(EpochSeconds::new(now)) {
        return Err(GraphError::Unavailable {
            detail: format!("the deadline ({deadline}) passed before the graph operation began"),
        });
    }
    let remaining_secs = deadline.instant().get().saturating_sub(now);
    let millis = remaining_secs
        .saturating_mul(1000)
        .clamp(1, MAX_STATEMENT_TIMEOUT_MS);
    tx.batch_execute(&format!("SET LOCAL statement_timeout = {millis}"))
        .map_err(read_err)
}

fn now_epoch() -> Result<u64, GraphError> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_secs())
        .map_err(|err| GraphError::Unavailable {
            detail: format!("the system clock is before the epoch: {err}"),
        })
}

fn read_err(error: postgres::Error) -> GraphError {
    classify(error, |detail| GraphError::ReadFailed { detail })
}

fn write_err(error: postgres::Error) -> GraphError {
    classify(error, |detail| GraphError::WriteFailed { detail })
}

fn classify(error: postgres::Error, failed: impl FnOnce(String) -> GraphError) -> GraphError {
    // The server's message is the detail; `Display` alone shows only "db error".
    let detail = match error.as_db_error() {
        Some(db) => db.message().to_owned(),
        None => error.to_string(),
    };
    if error.code() == Some(&SqlState::QUERY_CANCELED) {
        GraphError::Unavailable {
            detail: format!("the graph statement ran past its deadline: {detail}"),
        }
    } else {
        failed(detail)
    }
}

fn corrupt(detail: impl Into<String>) -> GraphError {
    GraphError::Corrupt {
        detail: detail.into(),
    }
}

fn domain<T, E: Display>(result: Result<T, E>) -> Result<T, GraphError> {
    result.map_err(|err| corrupt(err.to_string()))
}

fn to_i64(value: u64, what: &str) -> Result<i64, GraphError> {
    i64::try_from(value).map_err(|_| GraphError::WriteFailed {
        detail: format!("the {what} exceeds the database range"),
    })
}

fn text(row: &Row, column: &str) -> Result<String, GraphError> {
    row.try_get::<_, String>(column)
        .map_err(|err| corrupt(format!("column {column}: {err}")))
}

fn opt_text(row: &Row, column: &str) -> Result<Option<String>, GraphError> {
    row.try_get::<_, Option<String>>(column)
        .map_err(|err| corrupt(format!("column {column}: {err}")))
}

fn int(row: &Row, column: &str) -> Result<i64, GraphError> {
    row.try_get::<_, i64>(column)
        .map_err(|err| corrupt(format!("column {column}: {err}")))
}

fn opt_int(row: &Row, column: &str) -> Result<Option<i64>, GraphError> {
    row.try_get::<_, Option<i64>>(column)
        .map_err(|err| corrupt(format!("column {column}: {err}")))
}

fn unsigned64(row: &Row, column: &str) -> Result<u64, GraphError> {
    u64::try_from(int(row, column)?).map_err(|_| corrupt(format!("column {column} is negative")))
}

fn unsigned32(row: &Row, column: &str) -> Result<u32, GraphError> {
    u32::try_from(int(row, column)?)
        .map_err(|_| corrupt(format!("column {column} is out of range")))
}

fn decode_budget(row: &Row) -> Result<BudgetSlice, GraphError> {
    Ok(BudgetSlice {
        rounds: unsigned32(row, "budget_rounds")?,
        wall_seconds: unsigned64(row, "budget_wall_seconds")?,
        tokens: unsigned64(row, "budget_tokens")?,
        cost_usd: MicroUsd::new(unsigned64(row, "budget_cost_micro_usd")?),
    })
}

fn parse_failure(token: &str) -> Result<InvocationFailure, GraphError> {
    match token {
        "malformed_result" => Ok(InvocationFailure::MalformedResult),
        "contract_mismatch" => Ok(InvocationFailure::ContractMismatch),
        "orphaned_by_restart" => Ok(InvocationFailure::OrphanedByRestart),
        "agent_execution" => Ok(InvocationFailure::Execution),
        "missing_result" => Ok(InvocationFailure::MissingResult),
        other => Err(corrupt(format!("unknown failure class {other:?}"))),
    }
}

fn decode_status(status: &str, failure: Option<&str>) -> Result<NodeStatus, GraphError> {
    match (status, failure) {
        ("failed", Some(token)) => Ok(NodeStatus::Failed(parse_failure(token)?)),
        ("failed", None) => Err(corrupt("a failed node has no failure class")),
        (_, Some(_)) => Err(corrupt(format!("a {status} node carries a failure class"))),
        ("admitted", None) => Ok(NodeStatus::Admitted),
        ("running", None) => Ok(NodeStatus::Running),
        ("completed", None) => Ok(NodeStatus::Completed),
        ("timed_out", None) => Ok(NodeStatus::TimedOut),
        ("cancelled", None) => Ok(NodeStatus::Cancelled),
        (other, None) => Err(corrupt(format!("unknown node status {other:?}"))),
    }
}

/// Decodes one `invocation_nodes` row. Every field is checked; nothing defaults.
fn decode_node(row: &Row) -> Result<InvocationNode, GraphError> {
    let status = decode_status(
        &text(row, "status")?,
        opt_text(row, "failure_class")?.as_deref(),
    )?;

    let session_epoch = unsigned32(row, "session_epoch")?;
    let native_session = match opt_text(row, "session_ref")? {
        Some(session) => {
            if session_epoch == 0 {
                return Err(corrupt("a bound native session has epoch 0"));
            }
            Some(NativeSessionBinding {
                session: domain(SessionRef::new(&session))?,
                epoch: session_epoch,
            })
        }
        None => {
            if session_epoch != 0 {
                return Err(corrupt("an unbound native session has a nonzero epoch"));
            }
            None
        }
    };

    let result_digest = match opt_text(row, "result_digest")? {
        Some(digest) => Some(domain(Digest::parse(&digest))?),
        None => None,
    };

    let terminal_at = match opt_int(row, "terminal_at")? {
        Some(value) => Some(EpochSeconds::new(
            u64::try_from(value).map_err(|_| corrupt("terminal_at is negative"))?,
        )),
        None => None,
    };

    Ok(InvocationNode {
        invocation_id: domain(InvocationId::new(&text(row, "invocation_id")?))?,
        run_id: domain(RunId::new(&text(row, "run_id")?))?,
        parent: match opt_text(row, "parent_id")? {
            Some(parent) => Some(domain(InvocationId::new(&parent))?),
            None => None,
        },
        root: domain(InvocationId::new(&text(row, "root_id")?))?,
        depth: unsigned32(row, "depth")?,
        role: domain(RoleId::parse(&text(row, "role")?))?,
        runner_profile: domain(RunnerProfileId::new(&text(row, "runner_profile")?))?,
        harness: domain(HarnessId::new(&text(row, "harness")?))?,
        workspace: domain(WorkspaceRef::new(&text(row, "workspace")?))?,
        context_digest: domain(Digest::parse(&text(row, "context_digest")?))?,
        result_contract: domain(ResultContractPath::new(&text(row, "result_contract")?))?,
        admission_key: domain(AdmissionKey::new(&text(row, "admission_key")?))?,
        budget: decode_budget(row)?,
        status,
        native_session,
        result_digest,
        admitted_at: EpochSeconds::new(unsigned64(row, "admitted_at")?),
        terminal_at,
    })
}
