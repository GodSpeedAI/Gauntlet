//! The `RevisionLedger` adapter over PostgreSQL (spec §19.3/§19.4) — the
//! durable candidate-revision store in the same database as the run state
//! and the capability ledger.
//!
//! Cross-run by design (charter D3): revisions are evaluated and
//! adjudicated across runs, so the ledger deliberately outlives them.
//!
//! - [`PostgresRevisionLedger`] — INSERT-only over migration 0001's four
//!   tables. Proposal admissions are idempotent through the `revision`
//!   UNIQUE key (a same-id re-admission whose facts agree is a no-op; a
//!   disagreeing one is the typed `ProposalConflict` with the stored row
//!   standing); evaluation appends are idempotent on the four-run
//!   identity. The current decision is DERIVED from the latest decision
//!   record's verdict — there is no separately mutable decision column
//!   anywhere — and the rolled-back fact is the EXISTENCE of a restore
//!   row, so a promoted-and-restored revision reads
//!   promoted-but-rolled-back, never silently active.
//! - Stored payloads are the typed records' canonical JSON
//!   ([`gauntlet_ports::revision_ledger`] codecs); a stored payload that
//!   does not decode is a typed `Corrupt` error, never a degraded record.
//!
//! Port methods carry a `Deadline` (spec §6 preamble); like the state
//! store, this adapter runs single bounded statements with no retry loop
//! and no clock of its own, so the bound is noted and not consulted.

use std::sync::Mutex;

use gauntlet_domain::RevisionId;
use gauntlet_ports::deadline::Deadline;
use gauntlet_ports::revision_ledger::{
    DecisionKind, EvaluationAppend, EvaluationRecord, ProposalAdmission, RestoreRecord,
    RevisionDecisionRecord, RevisionEntry, RevisionLedger, RevisionLedgerError, RevisionProposal,
};
use postgres::Client;

use crate::db::PostgresConfig;
use crate::migrations;

/// The revision ledger over PostgreSQL (spec §19.3/§19.4).
pub struct PostgresRevisionLedger {
    client: Mutex<Client>,
}

impl PostgresRevisionLedger {
    /// Opens (creating if needed) the ledger by connecting and applying any
    /// migrations not yet recorded.
    pub fn open(config: &PostgresConfig) -> Result<Self, RevisionLedgerError> {
        let mut client = crate::db::connect(config)
            .map_err(|detail| RevisionLedgerError::AppendFailed { detail })?;
        migrations::apply(&mut client).map_err(|err| RevisionLedgerError::AppendFailed {
            detail: err.to_string(),
        })?;
        Ok(Self {
            client: Mutex::new(client),
        })
    }

    /// The revision's derived entry: the decision from the LATEST decision
    /// record's verdict, the rolled-back fact from a restore row's
    /// existence — both derived, neither separately mutable.
    fn entry_of(
        client: &mut Client,
        revision: &RevisionId,
    ) -> Result<Option<RevisionEntry>, RevisionLedgerError> {
        // Get the proposal payload
        let proposal_row = client
            .query_opt(
                "SELECT payload FROM revision_proposals WHERE revision = $1",
                &[&revision.as_str()],
            )
            .map_err(|err| read_failed("the stored proposal", revision, err))?;

        let Some(proposal_row) = proposal_row else {
            return Ok(None);
        };

        let proposal_payload: String = proposal_row.get(0);
        let proposal = RevisionProposal::decode(&proposal_payload).map_err(|error| {
            RevisionLedgerError::Corrupt {
                detail: format!(
                    "the stored proposal of revision {revision} does not decode: {error}"
                ),
            }
        })?;

        let admitted_at = i64::try_from(proposal.admitted_at().get()).map_err(|_| {
            RevisionLedgerError::Corrupt {
                detail: "a negative admission instant is tamper evidence".to_owned(),
            }
        })?;

        // Get the latest decision record
        let latest_row = client
            .query_opt(
                "SELECT payload FROM revision_decisions WHERE revision = $1 \
                 ORDER BY seq DESC LIMIT 1",
                &[&revision.as_str()],
            )
            .map_err(|err| read_failed("the latest decision record", revision, err))?;

        let (decision, last_decision_at) = match latest_row {
            Some(row) => {
                let payload: String = row.get(0);
                let record = RevisionDecisionRecord::decode(&payload).map_err(|error| {
                    RevisionLedgerError::Corrupt {
                        detail: format!(
                            "the latest decision of revision {revision} does not decode: {error}"
                        ),
                    }
                })?;
                (Some(record.writes()), Some(record.at()))
            }
            // A proposal without any decision: the ledger's floor state —
            // pending, never adjudicated.
            None => (None, None),
        };

        // Count evaluations
        let eval_row = client
            .query_one(
                "SELECT COUNT(*) FROM revision_evaluations WHERE revision = $1",
                &[&revision.as_str()],
            )
            .map_err(|err| read_failed("the evaluation count", revision, err))?;
        let evaluations: i64 = eval_row.get(0);

        // Check for restore records
        let restore_row = client
            .query_opt(
                "SELECT EXISTS(SELECT 1 FROM revision_restores WHERE revision = $1)",
                &[&revision.as_str()],
            )
            .map_err(|err| read_failed("the restore history", revision, err))?;

        let rolled_back = match restore_row {
            Some(row) => row.get::<_, bool>(0),
            None => false,
        };

        Ok(Some(RevisionEntry {
            revision: revision.clone(),
            decision,
            rolled_back,
            evaluations: usize::try_from(evaluations).map_err(|_| {
                RevisionLedgerError::Corrupt {
                    detail: "a negative evaluation count is tamper evidence".to_owned(),
                }
            })?,
            admitted_at: gauntlet_domain::EpochSeconds::new(u64::try_from(admitted_at).map_err(
                |_| RevisionLedgerError::Corrupt {
                    detail: "a negative admission instant is tamper evidence".to_owned(),
                },
            )?),
            last_decision_at,
        }))
    }
}

/// Transaction-aware version of entry_of for use within transactions.
fn entry_of_from_transaction(
    transaction: &mut postgres::Transaction,
    revision: &RevisionId,
) -> Result<Option<RevisionEntry>, RevisionLedgerError> {
    // Get the proposal payload
    let proposal_row = transaction
        .query_opt(
            "SELECT payload FROM revision_proposals WHERE revision = $1",
            &[&revision.as_str()],
        )
        .map_err(|err| read_failed("the stored proposal", revision, err))?;

    let Some(proposal_row) = proposal_row else {
        return Ok(None);
    };

    let proposal_payload: String = proposal_row.get(0);
    let proposal = RevisionProposal::decode(&proposal_payload).map_err(|error| {
        RevisionLedgerError::Corrupt {
            detail: format!("the stored proposal of revision {revision} does not decode: {error}"),
        }
    })?;

    let admitted_at =
        i64::try_from(proposal.admitted_at().get()).map_err(|_| RevisionLedgerError::Corrupt {
            detail: "a negative admission instant is tamper evidence".to_owned(),
        })?;

    // Get the latest decision record
    let latest_row = transaction
        .query_opt(
            "SELECT payload FROM revision_decisions WHERE revision = $1 \
             ORDER BY seq DESC LIMIT 1",
            &[&revision.as_str()],
        )
        .map_err(|err| read_failed("the latest decision record", revision, err))?;

    let (decision, last_decision_at) = match latest_row {
        Some(row) => {
            let payload: String = row.get(0);
            let record = RevisionDecisionRecord::decode(&payload).map_err(|error| {
                RevisionLedgerError::Corrupt {
                    detail: format!(
                        "the latest decision of revision {revision} does not decode: {error}"
                    ),
                }
            })?;
            (Some(record.writes()), Some(record.at()))
        }
        // A proposal without any decision: the ledger's floor state —
        // pending, never adjudicated.
        None => (None, None),
    };

    // Count evaluations
    let eval_row = transaction
        .query_one(
            "SELECT COUNT(*) FROM revision_evaluations WHERE revision = $1",
            &[&revision.as_str()],
        )
        .map_err(|err| read_failed("the evaluation count", revision, err))?;
    let evaluations: i64 = eval_row.get(0);

    // Check for restore records
    let restore_row = transaction
        .query_opt(
            "SELECT EXISTS(SELECT 1 FROM revision_restores WHERE revision = $1)",
            &[&revision.as_str()],
        )
        .map_err(|err| read_failed("the restore history", revision, err))?;

    let rolled_back = match restore_row {
        Some(row) => row.get::<_, bool>(0),
        None => false,
    };

    Ok(Some(RevisionEntry {
        revision: revision.clone(),
        decision,
        rolled_back,
        evaluations: usize::try_from(evaluations).map_err(|_| RevisionLedgerError::Corrupt {
            detail: "a negative evaluation count is tamper evidence".to_owned(),
        })?,
        admitted_at: gauntlet_domain::EpochSeconds::new(u64::try_from(admitted_at).map_err(
            |_| RevisionLedgerError::Corrupt {
                detail: "a negative admission instant is tamper evidence".to_owned(),
            },
        )?),
        last_decision_at,
    }))
}

/// Transaction-aware version of validate_claim for use within transactions.
fn validate_claim_in_transaction(
    transaction: &mut postgres::Transaction,
    revision: &RevisionId,
    claimed: DecisionKind,
    rolled_back_ok: bool,
) -> Result<(), RevisionLedgerError> {
    let Some(entry) = entry_of_from_transaction(transaction, revision)? else {
        return Err(RevisionLedgerError::UnknownRevision {
            revision: revision.clone(),
        });
    };
    let current = entry.decision.unwrap_or(DecisionKind::Pending);
    if rolled_back_ok {
        // Restore records: the claim must be the promoted, unrestored
        // state itself.
        if claimed != DecisionKind::Promoted
            || entry.rolled_back
            || current != DecisionKind::Promoted
        {
            return Err(RevisionLedgerError::StaleRestore {
                revision: revision.clone(),
                current,
                rolled_back: entry.rolled_back,
            });
        }
    } else if claimed != current {
        return Err(RevisionLedgerError::StaleDecision {
            revision: revision.clone(),
            current,
        });
    }
    Ok(())
}

fn read_failed(what: &str, revision: &RevisionId, err: postgres::Error) -> RevisionLedgerError {
    RevisionLedgerError::ReadFailed {
        detail: format!("reading {what} of revision {revision} failed: {err}"),
    }
}

impl RevisionLedger for PostgresRevisionLedger {
    /// Admits one proposal. Idempotent through the revision UNIQUE key.
    fn admit_proposal(
        &self,
        proposal: RevisionProposal,
        _deadline: Deadline,
    ) -> Result<ProposalAdmission, RevisionLedgerError> {
        let revision = proposal.revision().clone();
        let payload = proposal.to_json();
        let mut client = self
            .client
            .lock()
            .expect("the ledger connection is not poisoned");

        // Check if proposal already exists
        let existing_row = client
            .query_opt(
                "SELECT payload FROM revision_proposals WHERE revision = $1",
                &[&revision.as_str()],
            )
            .map_err(|err| read_failed("the stored proposal", &revision, err))?;

        if let Some(existing) = existing_row {
            let stored_payload: String = existing.get(0);
            let stored = RevisionProposal::decode(&stored_payload)?;
            if stored.facts_agree(&proposal) {
                return Ok(ProposalAdmission { idempotent: true });
            }
            return Err(RevisionLedgerError::ProposalConflict { revision });
        }

        // Insert new proposal (uses ON CONFLICT DO NOTHING pattern)
        let insert_result = client.execute(
            "INSERT INTO revision_proposals (revision, payload) VALUES ($1, $2) \
             ON CONFLICT (revision) DO NOTHING",
            &[&revision.as_str(), &payload],
        );

        match insert_result {
            Ok(0) => {
                // Row already existed and conflict was handled
                let stored_row = client
                    .query_one(
                        "SELECT payload FROM revision_proposals WHERE revision = $1",
                        &[&revision.as_str()],
                    )
                    .map_err(|err| read_failed("the stored proposal", &revision, err))?;
                let stored_payload: String = stored_row.get(0);
                let stored = RevisionProposal::decode(&stored_payload)?;
                if stored.facts_agree(&proposal) {
                    Ok(ProposalAdmission { idempotent: true })
                } else {
                    Err(RevisionLedgerError::ProposalConflict { revision })
                }
            }
            Ok(_) => Ok(ProposalAdmission { idempotent: false }),
            Err(err) => Err(RevisionLedgerError::AppendFailed {
                detail: format!(
                    "admitting revision {revision} failed: {err} — nothing was recorded"
                ),
            }),
        }
    }

    fn proposal(
        &self,
        revision: &RevisionId,
        _deadline: Deadline,
    ) -> Result<Option<RevisionProposal>, RevisionLedgerError> {
        let mut client = self
            .client
            .lock()
            .expect("the ledger connection is not poisoned");

        let proposal_row = client
            .query_opt(
                "SELECT payload FROM revision_proposals WHERE revision = $1",
                &[&revision.as_str()],
            )
            .map_err(|err| read_failed("the stored proposal", revision, err))?;

        proposal_row
            .map(|row| {
                let payload: String = row.get(0);
                RevisionProposal::decode(&payload).map_err(|error| RevisionLedgerError::Corrupt {
                    detail: format!(
                        "the stored proposal of revision {revision} does not decode: {error}"
                    ),
                })
            })
            .transpose()
    }

    /// Appends one evaluation record. Idempotent on the four-run identity.
    fn append_evaluation(
        &self,
        record: EvaluationRecord,
        _deadline: Deadline,
    ) -> Result<EvaluationAppend, RevisionLedgerError> {
        let revision = record.revision.clone();
        let identity = record.identity();
        let payload = record.to_json();
        let mut client = self
            .client
            .lock()
            .expect("the ledger connection is not poisoned");

        // Check if revision exists
        let exists_row = client
            .query_opt(
                "SELECT 1 FROM revision_proposals WHERE revision = $1",
                &[&revision.as_str()],
            )
            .map_err(|err| read_failed("the stored proposal", &revision, err))?;

        if exists_row.is_none() {
            return Err(RevisionLedgerError::UnknownRevision {
                revision: revision.clone(),
            });
        }

        // Check if evaluation already exists
        let existing_row = client
            .query_opt(
                "SELECT payload FROM revision_evaluations WHERE revision = $1 \
                 AND held_out_control = $2 AND held_out_treatment = $3 \
                 AND regression_control = $4 AND regression_treatment = $5",
                &[
                    &revision.as_str(),
                    &identity.0.as_str(),
                    &identity.1.as_str(),
                    &identity.2.as_str(),
                    &identity.3.as_str(),
                ],
            )
            .map_err(|err| read_failed("the stored evaluation", &revision, err))?;

        // Get current max sequence for later use
        let seq_row = client
            .query_one(
                "SELECT COALESCE(MAX(seq), 0) FROM revision_evaluations",
                &[],
            )
            .map_err(|err| RevisionLedgerError::ReadFailed {
                detail: format!("reading the evaluation sequence failed: {err}"),
            })?;
        let sequence: i64 = seq_row.get(0);
        let sequence = u64::try_from(sequence).map_err(|_| RevisionLedgerError::Corrupt {
            detail: "a negative evaluation sequence is tamper evidence".to_owned(),
        })?;

        if let Some(existing) = existing_row {
            let stored_payload: String = existing.get(0);
            let stored = EvaluationRecord::decode(&stored_payload)?;
            if stored.facts_agree(&record) {
                return Ok(EvaluationAppend {
                    sequence,
                    idempotent: true,
                });
            }
            return Err(RevisionLedgerError::EvaluationConflict { revision });
        }

        // Insert new evaluation (uses ON CONFLICT DO NOTHING pattern)
        let insert_result = client.execute(
            "INSERT INTO revision_evaluations (revision, held_out_control, held_out_treatment, \
             regression_control, regression_treatment, payload) \
             VALUES ($1, $2, $3, $4, $5, $6) \
             ON CONFLICT DO NOTHING",
            &[
                &revision.as_str(),
                &identity.0.as_str(),
                &identity.1.as_str(),
                &identity.2.as_str(),
                &identity.3.as_str(),
                &payload,
            ],
        );

        match insert_result {
            Ok(0) => {
                // Row already existed and conflict was handled
                let stored_row = client
                    .query_one(
                        "SELECT payload FROM revision_evaluations WHERE revision = $1 \
                         AND held_out_control = $2 AND held_out_treatment = $3 \
                         AND regression_control = $4 AND regression_treatment = $5",
                        &[
                            &revision.as_str(),
                            &identity.0.as_str(),
                            &identity.1.as_str(),
                            &identity.2.as_str(),
                            &identity.3.as_str(),
                        ],
                    )
                    .map_err(|err| read_failed("the stored evaluation", &revision, err))?;
                let stored_payload: String = stored_row.get(0);
                let stored = EvaluationRecord::decode(&stored_payload)?;
                if stored.facts_agree(&record) {
                    Ok(EvaluationAppend {
                        sequence,
                        idempotent: true,
                    })
                } else {
                    Err(RevisionLedgerError::EvaluationConflict { revision })
                }
            }
            Ok(_) => Ok(EvaluationAppend {
                sequence,
                idempotent: false,
            }),
            Err(err) => Err(RevisionLedgerError::AppendFailed {
                detail: format!(
                    "appending an evaluation of {revision} failed: {err} — nothing was recorded"
                ),
            }),
        }
    }

    fn evaluations(
        &self,
        revision: &RevisionId,
        _deadline: Deadline,
    ) -> Result<Vec<EvaluationRecord>, RevisionLedgerError> {
        let mut client = self
            .client
            .lock()
            .expect("the ledger connection is not poisoned");

        let rows = client
            .query(
                "SELECT payload FROM revision_evaluations WHERE revision = $1 ORDER BY seq ASC",
                &[&revision.as_str()],
            )
            .map_err(|err| read_failed("the evaluation records", revision, err))?;

        let mut records = Vec::new();
        for row in rows {
            let payload: String = row.get(0);
            records.push(EvaluationRecord::decode(&payload).map_err(|error| {
                RevisionLedgerError::Corrupt {
                    detail: format!(
                        "a stored evaluation of revision {revision} does not decode: {error}"
                    ),
                }
            })?);
        }
        Ok(records)
    }

    /// Applies one decision record — the ledger's ONLY decision writer.
    /// One transaction: the staleness check reads the same snapshot the
    /// history insert lands in.
    fn apply_decision(
        &self,
        record: RevisionDecisionRecord,
        _deadline: Deadline,
    ) -> Result<RevisionEntry, RevisionLedgerError> {
        let revision = record.revision().clone();
        let payload = record.to_json();
        let mut client = self
            .client
            .lock()
            .expect("the ledger connection is not poisoned");

        let mut transaction =
            client
                .transaction()
                .map_err(|err| RevisionLedgerError::AppendFailed {
                    detail: format!("beginning the decision transaction failed: {err}"),
                })?;

        // Validate the claim and state
        validate_claim_in_transaction(&mut transaction, &revision, record.claimed(), false)?;

        // Insert the decision record
        transaction
            .execute(
                "INSERT INTO revision_decisions (revision, payload) VALUES ($1, $2)",
                &[&revision.as_str(), &payload],
            )
            .map_err(|err| RevisionLedgerError::AppendFailed {
                detail: format!(
                    "applying a decision for revision {revision} failed: {err} — nothing was \
                     recorded"
                ),
            })?;

        // Retrieve the updated entry
        let new_entry =
            entry_of_from_transaction(&mut transaction, &revision)?.ok_or_else(|| {
                RevisionLedgerError::ReadFailed {
                    detail: format!("the derived entry of {revision} vanished mid-decision"),
                }
            })?;

        // Commit the transaction
        transaction
            .commit()
            .map_err(|err| RevisionLedgerError::AppendFailed {
                detail: format!("committing the decision failed: {err}"),
            })?;

        Ok(new_entry)
    }

    fn decisions(
        &self,
        revision: &RevisionId,
        _deadline: Deadline,
    ) -> Result<Vec<RevisionDecisionRecord>, RevisionLedgerError> {
        let mut client = self
            .client
            .lock()
            .expect("the ledger connection is not poisoned");

        let rows = client
            .query(
                "SELECT payload FROM revision_decisions WHERE revision = $1 ORDER BY seq ASC",
                &[&revision.as_str()],
            )
            .map_err(|err| read_failed("the decision history", revision, err))?;

        let mut history = Vec::new();
        for row in rows {
            let payload: String = row.get(0);
            history.push(RevisionDecisionRecord::decode(&payload).map_err(|error| {
                RevisionLedgerError::Corrupt {
                    detail: format!(
                        "a stored decision of revision {revision} does not decode: {error}"
                    ),
                }
            })?);
        }
        Ok(history)
    }

    /// Records one restore transition (append-only). One transaction, the
    /// decision shape: the promoted-and-unrestored claim is validated
    /// against the same snapshot the row lands in.
    fn record_restore(
        &self,
        record: RestoreRecord,
        _deadline: Deadline,
    ) -> Result<RevisionEntry, RevisionLedgerError> {
        let revision = record.revision().clone();
        let payload = record.to_json();
        let mut client = self
            .client
            .lock()
            .expect("the ledger connection is not poisoned");

        let mut transaction =
            client
                .transaction()
                .map_err(|err| RevisionLedgerError::AppendFailed {
                    detail: format!("beginning the restore transaction failed: {err}"),
                })?;

        // Validate the claim and state (rolled_back_ok=true)
        validate_claim_in_transaction(&mut transaction, &revision, record.claimed(), true)?;

        // Insert the restore record
        transaction
            .execute(
                "INSERT INTO revision_restores (revision, payload) VALUES ($1, $2)",
                &[&revision.as_str(), &payload],
            )
            .map_err(|err| RevisionLedgerError::AppendFailed {
                detail: format!(
                    "recording the restore of revision {revision} failed: {err} — nothing was \
                     recorded"
                ),
            })?;

        // Retrieve the updated entry
        let new_entry =
            entry_of_from_transaction(&mut transaction, &revision)?.ok_or_else(|| {
                RevisionLedgerError::ReadFailed {
                    detail: format!("the derived entry of {revision} vanished mid-restore"),
                }
            })?;

        // Commit the transaction
        transaction
            .commit()
            .map_err(|err| RevisionLedgerError::AppendFailed {
                detail: format!("committing the restore failed: {err}"),
            })?;

        Ok(new_entry)
    }

    fn restores(
        &self,
        revision: &RevisionId,
        _deadline: Deadline,
    ) -> Result<Vec<RestoreRecord>, RevisionLedgerError> {
        let mut client = self
            .client
            .lock()
            .expect("the ledger connection is not poisoned");

        let rows = client
            .query(
                "SELECT payload FROM revision_restores WHERE revision = $1 ORDER BY seq ASC",
                &[&revision.as_str()],
            )
            .map_err(|err| read_failed("the restore history", revision, err))?;

        let mut history = Vec::new();
        for row in rows {
            let payload: String = row.get(0);
            history.push(RestoreRecord::decode(&payload).map_err(|error| {
                RevisionLedgerError::Corrupt {
                    detail: format!(
                        "a stored restore of revision {revision} does not decode: {error}"
                    ),
                }
            })?);
        }
        Ok(history)
    }

    fn revision(
        &self,
        revision: &RevisionId,
        _deadline: Deadline,
    ) -> Result<Option<RevisionEntry>, RevisionLedgerError> {
        let mut client = self
            .client
            .lock()
            .expect("the ledger connection is not poisoned");
        Self::entry_of(&mut client, revision)
    }
}
