//! Canonical encoding of durable payloads — the one serialization vocabulary
//! for everything this adapter stores (events, checkpoint projections, run
//! identity, budget shapes).
//!
//! Canonical means: field order fixed, numbers unsigned, ids and digests as
//! plain strings, produced only through [`gauntlet_ports::json`]'s writer so
//! the bytes are identical wherever they are re-emitted (export). Decoding is
//! strict: a missing field, a wrong-typed field, or a value the domain
//! constructors refuse is a typed [`DecodeError`] — the caller surfaces it as
//! `Damaged`/`Corrupt`, never as a silent skip.

use gauntlet_domain::{
    Budget, DiagnosticAllowance, Digest, EpochSeconds, EventLogOffset, EvidenceLocator,
    EvidenceRef, InvocationId, InvocationState, IterationNo, Limits, MicroUsd, ResultContractPath,
    RunId, Spent, TerminalState, UnitId, UnitStatus, VerificationRef,
};
use gauntlet_ports::json::{self, JsonNumber, JsonValue};
use gauntlet_ports::state_store::{RunEvent, RunEventKind};

/// Event kind tags (the wire vocabulary of the log).
pub(crate) const KIND_SETTLEMENT: &str = "settlement_committed";
pub(crate) const KIND_INVOCATION_ADMITTED: &str = "invocation_admitted";
pub(crate) const KIND_INVOCATION_TERMINAL: &str = "invocation_terminal";
pub(crate) const KIND_CHECKPOINT_WRITTEN: &str = "checkpoint_written";
pub(crate) const KIND_EVIDENCE_PRODUCED: &str = "evidence_produced";
pub(crate) const KIND_UNIT_DISPATCH: &str = "unit_dispatch";
pub(crate) const KIND_RUN_TERMINAL: &str = "run_terminal";

/// A payload did not decode into the domain types — typed (spec §21.2).
/// Details carry field names, ids, and digests; never record payloads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum DecodeError {
    /// The payload is not a JSON document.
    Json { detail: String },
    /// A required field is absent.
    FieldMissing { field: &'static str },
    /// A field carries the wrong JSON shape.
    FieldType {
        field: &'static str,
        expected: &'static str,
    },
    /// A field's value is not one the domain constructor accepts.
    ValueInvalid { field: &'static str, detail: String },
}

impl core::fmt::Display for DecodeError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Json { detail } => write!(f, "payload is not valid JSON: {detail}"),
            Self::FieldMissing { field } => write!(f, "field {field:?} is missing"),
            Self::FieldType { field, expected } => {
                write!(f, "field {field:?} is not {expected}")
            }
            Self::ValueInvalid { field, detail } => {
                write!(f, "field {field:?} holds an invalid value: {detail}")
            }
        }
    }
}

fn obj(fields: Vec<(&str, JsonValue)>) -> JsonValue {
    JsonValue::Object(fields.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
}

fn text(value: &str) -> JsonValue {
    JsonValue::Str(value.to_owned())
}

fn number(value: u64) -> JsonValue {
    JsonValue::Number(JsonNumber::Uint(value))
}

fn get_str(object: &JsonValue, field: &'static str) -> Result<String, DecodeError> {
    match object.get(field) {
        Some(JsonValue::Str(value)) => Ok(value.clone()),
        Some(_) => Err(DecodeError::FieldType {
            field,
            expected: "a string",
        }),
        None => Err(DecodeError::FieldMissing { field }),
    }
}

fn get_u64(object: &JsonValue, field: &'static str) -> Result<u64, DecodeError> {
    match object.get(field) {
        Some(JsonValue::Number(JsonNumber::Uint(value))) => Ok(*value),
        Some(_) => Err(DecodeError::FieldType {
            field,
            expected: "an unsigned integer",
        }),
        None => Err(DecodeError::FieldMissing { field }),
    }
}

fn get_object(object: &JsonValue, field: &'static str) -> Result<JsonValue, DecodeError> {
    match object.get(field) {
        Some(value @ JsonValue::Object(_)) => Ok(value.clone()),
        Some(_) => Err(DecodeError::FieldType {
            field,
            expected: "an object",
        }),
        None => Err(DecodeError::FieldMissing { field }),
    }
}

fn pair_of_str(entry: &JsonValue) -> Result<(String, String), DecodeError> {
    match entry {
        JsonValue::Array(items) if items.len() == 2 => {
            let first = match &items[0] {
                JsonValue::Str(value) => value.clone(),
                _ => {
                    return Err(DecodeError::FieldType {
                        field: "pair",
                        expected: "a string at position 0",
                    });
                }
            };
            let second = match &items[1] {
                JsonValue::Str(value) => value.clone(),
                _ => {
                    return Err(DecodeError::FieldType {
                        field: "pair",
                        expected: "a string at position 1",
                    });
                }
            };
            Ok((first, second))
        }
        _ => Err(DecodeError::FieldType {
            field: "pair",
            expected: "a two-element array",
        }),
    }
}

// ---------------------------------------------------------------------------
// RunEvent payloads
// ---------------------------------------------------------------------------

/// Encodes one event kind into its `(tag, canonical payload)` pair.
pub(crate) fn encode_event(kind: &RunEventKind) -> (String, String) {
    let value = match kind {
        RunEventKind::SettlementCommitted {
            unit_id,
            evidence,
            verification,
            reference_digest,
        } => obj(vec![
            ("kind", text(KIND_SETTLEMENT)),
            ("unit_id", text(unit_id.as_str())),
            ("evidence", text(evidence.as_str())),
            ("verification", text(verification.as_str())),
            ("reference_digest", text(&reference_digest.to_string())),
        ]),
        RunEventKind::InvocationAdmitted { invocation_id } => obj(vec![
            ("kind", text(KIND_INVOCATION_ADMITTED)),
            ("invocation_id", text(invocation_id.as_str())),
        ]),
        RunEventKind::InvocationTerminal {
            invocation_id,
            state,
        } => obj(vec![
            ("kind", text(KIND_INVOCATION_TERMINAL)),
            ("invocation_id", text(invocation_id.as_str())),
            ("state", text(&state.wire())),
        ]),
        RunEventKind::CheckpointWritten {
            iteration_no,
            event_log_offset,
        } => obj(vec![
            ("kind", text(KIND_CHECKPOINT_WRITTEN)),
            ("iteration_no", number(u64::from(iteration_no.get()))),
            ("event_log_offset", number(event_log_offset.get())),
        ]),
        RunEventKind::EvidenceProduced { evidence, locator } => obj(vec![
            ("kind", text(KIND_EVIDENCE_PRODUCED)),
            ("evidence", text(evidence.as_str())),
            ("locator", text(locator.as_str())),
        ]),
        RunEventKind::UnitDispatch {
            unit,
            invocation_id,
        } => obj(vec![
            ("kind", text(KIND_UNIT_DISPATCH)),
            ("unit_id", text(unit.as_str())),
            ("invocation_id", text(invocation_id.as_str())),
        ]),
        RunEventKind::RunTerminal { state } => obj(vec![
            ("kind", text(KIND_RUN_TERMINAL)),
            ("state", text(&state.wire())),
        ]),
    };
    let tag = match kind {
        RunEventKind::SettlementCommitted { .. } => KIND_SETTLEMENT,
        RunEventKind::InvocationAdmitted { .. } => KIND_INVOCATION_ADMITTED,
        RunEventKind::InvocationTerminal { .. } => KIND_INVOCATION_TERMINAL,
        RunEventKind::CheckpointWritten { .. } => KIND_CHECKPOINT_WRITTEN,
        RunEventKind::EvidenceProduced { .. } => KIND_EVIDENCE_PRODUCED,
        RunEventKind::UnitDispatch { .. } => KIND_UNIT_DISPATCH,
        RunEventKind::RunTerminal { .. } => KIND_RUN_TERMINAL,
    };
    (tag.to_owned(), value.to_json_text())
}

/// Decodes one stored `(tag, payload)` into the event it names, for `run_id`
/// at `at`. Strict: the tag must match the payload's own `kind` field and
/// every domain constructor must accept what it finds.
pub(crate) fn decode_event(
    run_id: RunId,
    at: EpochSeconds,
    tag: &str,
    payload: &str,
) -> Result<RunEvent, DecodeError> {
    let parsed = json::parse(payload).map_err(|err| DecodeError::Json {
        detail: err.to_string(),
    })?;
    let kind_field = get_str(&parsed, "kind")?;
    if kind_field != tag {
        return Err(DecodeError::ValueInvalid {
            field: "kind",
            detail: format!(
                "the payload says {kind_field:?} but the row's kind column says {tag:?}"
            ),
        });
    }
    let kind = match tag {
        KIND_SETTLEMENT => RunEventKind::SettlementCommitted {
            unit_id: UnitId::new(&get_str(&parsed, "unit_id")?).map_err(|err| {
                DecodeError::ValueInvalid {
                    field: "unit_id",
                    detail: err.to_string(),
                }
            })?,
            evidence: EvidenceRef::new(&get_str(&parsed, "evidence")?).map_err(|err| {
                DecodeError::ValueInvalid {
                    field: "evidence",
                    detail: err.to_string(),
                }
            })?,
            verification: VerificationRef::new(&get_str(&parsed, "verification")?).map_err(
                |err| DecodeError::ValueInvalid {
                    field: "verification",
                    detail: err.to_string(),
                },
            )?,
            reference_digest: Digest::parse(&get_str(&parsed, "reference_digest")?).map_err(
                |err| DecodeError::ValueInvalid {
                    field: "reference_digest",
                    detail: err.to_string(),
                },
            )?,
        },
        KIND_INVOCATION_ADMITTED => RunEventKind::InvocationAdmitted {
            invocation_id: InvocationId::new(&get_str(&parsed, "invocation_id")?).map_err(
                |err| DecodeError::ValueInvalid {
                    field: "invocation_id",
                    detail: err.to_string(),
                },
            )?,
        },
        KIND_INVOCATION_TERMINAL => RunEventKind::InvocationTerminal {
            invocation_id: InvocationId::new(&get_str(&parsed, "invocation_id")?).map_err(
                |err| DecodeError::ValueInvalid {
                    field: "invocation_id",
                    detail: err.to_string(),
                },
            )?,
            state: InvocationState::parse(&get_str(&parsed, "state")?).map_err(|err| {
                DecodeError::ValueInvalid {
                    field: "state",
                    detail: err.to_string(),
                }
            })?,
        },
        KIND_CHECKPOINT_WRITTEN => RunEventKind::CheckpointWritten {
            iteration_no: IterationNo::new(
                u32::try_from(get_u64(&parsed, "iteration_no")?).map_err(|_| {
                    DecodeError::ValueInvalid {
                        field: "iteration_no",
                        detail: "the stored iteration number exceeds u32".to_owned(),
                    }
                })?,
            )
            .map_err(|err| DecodeError::ValueInvalid {
                field: "iteration_no",
                detail: err.to_string(),
            })?,
            event_log_offset: EventLogOffset::new(get_u64(&parsed, "event_log_offset")?),
        },
        KIND_EVIDENCE_PRODUCED => RunEventKind::EvidenceProduced {
            evidence: EvidenceRef::new(&get_str(&parsed, "evidence")?).map_err(|err| {
                DecodeError::ValueInvalid {
                    field: "evidence",
                    detail: err.to_string(),
                }
            })?,
            locator: EvidenceLocator::new(&get_str(&parsed, "locator")?).map_err(|err| {
                DecodeError::ValueInvalid {
                    field: "locator",
                    detail: err.to_string(),
                }
            })?,
        },
        KIND_UNIT_DISPATCH => RunEventKind::UnitDispatch {
            unit: UnitId::new(&get_str(&parsed, "unit_id")?).map_err(|err| {
                DecodeError::ValueInvalid {
                    field: "unit_id",
                    detail: err.to_string(),
                }
            })?,
            invocation_id: InvocationId::new(&get_str(&parsed, "invocation_id")?).map_err(
                |err| DecodeError::ValueInvalid {
                    field: "invocation_id",
                    detail: err.to_string(),
                },
            )?,
        },
        KIND_RUN_TERMINAL => RunEventKind::RunTerminal {
            state: TerminalState::parse(&get_str(&parsed, "state")?).map_err(|err| {
                DecodeError::ValueInvalid {
                    field: "state",
                    detail: err.to_string(),
                }
            })?,
        },
        other => {
            return Err(DecodeError::ValueInvalid {
                field: "kind",
                detail: format!("{other:?} is not an event kind this adapter ever wrote"),
            });
        }
    };
    Ok(RunEvent { run_id, kind, at })
}

// ---------------------------------------------------------------------------
// Checkpoint projections
// ---------------------------------------------------------------------------

/// `unit_states` as `[["AK-014","open"],...]`, ordered as written.
pub(crate) fn encode_unit_states(states: &[(UnitId, UnitStatus)]) -> String {
    JsonValue::Array(
        states
            .iter()
            .map(|(unit, status)| JsonValue::Array(vec![text(unit.as_str()), text(&status.wire())]))
            .collect(),
    )
    .to_json_text()
}

/// Decodes [`encode_unit_states`]; unknown status tokens are errors.
pub(crate) fn decode_unit_states(payload: &str) -> Result<Vec<(UnitId, UnitStatus)>, DecodeError> {
    let parsed = json::parse(payload).map_err(|err| DecodeError::Json {
        detail: err.to_string(),
    })?;
    let entries = match parsed {
        JsonValue::Array(items) => items,
        _ => {
            return Err(DecodeError::FieldType {
                field: "unit_states",
                expected: "an array",
            });
        }
    };
    let mut out = Vec::new();
    for entry in &entries {
        let (unit, status) = pair_of_str(entry)?;
        out.push((
            UnitId::new(&unit).map_err(|err| DecodeError::ValueInvalid {
                field: "unit_states",
                detail: err.to_string(),
            })?,
            UnitStatus::parse(&status).map_err(|err| DecodeError::ValueInvalid {
                field: "unit_states",
                detail: err.to_string(),
            })?,
        ));
    }
    Ok(out)
}

/// `in_flight` as `[["<invocation id>","<result-contract path>"],...]`.
pub(crate) fn encode_in_flight(in_flight: &[(InvocationId, ResultContractPath)]) -> String {
    JsonValue::Array(
        in_flight
            .iter()
            .map(|(invocation, path)| {
                JsonValue::Array(vec![text(invocation.as_str()), text(path.as_str())])
            })
            .collect(),
    )
    .to_json_text()
}

/// Decodes [`encode_in_flight`].
pub(crate) fn decode_in_flight(
    payload: &str,
) -> Result<Vec<(InvocationId, ResultContractPath)>, DecodeError> {
    let parsed = json::parse(payload).map_err(|err| DecodeError::Json {
        detail: err.to_string(),
    })?;
    let entries = match parsed {
        JsonValue::Array(items) => items,
        _ => {
            return Err(DecodeError::FieldType {
                field: "in_flight",
                expected: "an array",
            });
        }
    };
    let mut out = Vec::new();
    for entry in &entries {
        let (invocation, path) = pair_of_str(entry)?;
        out.push((
            InvocationId::new(&invocation).map_err(|err| DecodeError::ValueInvalid {
                field: "in_flight",
                detail: err.to_string(),
            })?,
            ResultContractPath::new(&path).map_err(|err| DecodeError::ValueInvalid {
                field: "in_flight",
                detail: err.to_string(),
            })?,
        ));
    }
    Ok(out)
}

/// `Spent` — every §5.3.7 mirrored counter.
pub(crate) fn encode_spent(spent: &Spent) -> String {
    obj(vec![
        ("rounds", number(u64::from(spent.rounds()))),
        ("wall_seconds", number(spent.wall_seconds())),
        ("tokens", number(spent.tokens())),
        ("cost_usd", number(spent.cost_usd().get())),
        ("diagnostic_tokens", number(spent.diagnostic_tokens())),
        (
            "diagnostic_cost_usd",
            number(spent.diagnostic_cost_usd().get()),
        ),
    ])
    .to_json_text()
}

/// Decodes [`encode_spent`]. The counters are stored verbatim and returned
/// as the six numbers, for the caller to admit through its own accounting —
/// the domain's `Budget::restore` (Task 12) is the admission path that
/// enforces the water-mark invariants against the run's ceilings.
pub(crate) fn decode_spent(payload: &str) -> Result<SpentNumbers, DecodeError> {
    let parsed = json::parse(payload).map_err(|err| DecodeError::Json {
        detail: err.to_string(),
    })?;
    Ok(SpentNumbers {
        rounds: get_u64(&parsed, "rounds")?,
        wall_seconds: get_u64(&parsed, "wall_seconds")?,
        tokens: get_u64(&parsed, "tokens")?,
        cost_usd: get_u64(&parsed, "cost_usd")?,
        diagnostic_tokens: get_u64(&parsed, "diagnostic_tokens")?,
        diagnostic_cost_usd: get_u64(&parsed, "diagnostic_cost_usd")?,
    })
}

/// The six decoded spend counters (see [`decode_spent`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct SpentNumbers {
    pub rounds: u64,
    pub wall_seconds: u64,
    pub tokens: u64,
    pub cost_usd: u64,
    pub diagnostic_tokens: u64,
    pub diagnostic_cost_usd: u64,
}

/// The budget ceilings of a run: reserve percentage, limits, diagnostic
/// allowance. Spend is deliberately absent — §18.2 puts it in checkpoints.
pub(crate) fn encode_budget_ceiling(budget: &Budget) -> String {
    let limits = budget.limits();
    let diagnostic = budget.diagnostic_allowance();
    obj(vec![
        (
            "recovery_reserve_pct",
            number(u64::from(budget.recovery_reserve_pct())),
        ),
        (
            "limits",
            obj(vec![
                ("rounds", number(u64::from(limits.max_rounds()))),
                ("parallel", number(u64::from(limits.max_parallel()))),
                ("depth", number(u64::from(limits.max_depth()))),
                ("wall_seconds", number(limits.max_wall_seconds())),
                ("tokens", number(limits.max_tokens())),
                ("cost_usd", number(limits.max_cost_usd().get())),
            ]),
        ),
        (
            "diagnostic",
            obj(vec![
                ("tokens", number(diagnostic.tokens())),
                ("cost_usd", number(diagnostic.cost_usd().get())),
            ]),
        ),
    ])
    .to_json_text()
}

/// The decoded budget ceiling: what [`Budget::new`] needs.
pub(crate) struct BudgetCeiling {
    pub recovery_reserve_pct: u8,
    pub limits: Limits,
    pub diagnostic: DiagnosticAllowance,
}

/// Decodes [`encode_budget_ceiling`].
pub(crate) fn decode_budget_ceiling(payload: &str) -> Result<BudgetCeiling, DecodeError> {
    let parsed = json::parse(payload).map_err(|err| DecodeError::Json {
        detail: err.to_string(),
    })?;
    let reserve = u8::try_from(get_u64(&parsed, "recovery_reserve_pct")?).map_err(|_| {
        DecodeError::ValueInvalid {
            field: "recovery_reserve_pct",
            detail: "the stored reserve percentage exceeds u8".to_owned(),
        }
    })?;
    let limits_json = get_object(&parsed, "limits")?;
    let limits = Limits::new(
        u32::try_from(get_u64(&limits_json, "rounds")?).map_err(|_| DecodeError::ValueInvalid {
            field: "rounds",
            detail: "the stored ceiling exceeds u32".to_owned(),
        })?,
        u32::try_from(get_u64(&limits_json, "parallel")?).map_err(|_| {
            DecodeError::ValueInvalid {
                field: "parallel",
                detail: "the stored ceiling exceeds u32".to_owned(),
            }
        })?,
        u32::try_from(get_u64(&limits_json, "depth")?).map_err(|_| DecodeError::ValueInvalid {
            field: "depth",
            detail: "the stored ceiling exceeds u32".to_owned(),
        })?,
        get_u64(&limits_json, "wall_seconds")?,
        get_u64(&limits_json, "tokens")?,
        MicroUsd::new(get_u64(&limits_json, "cost_usd")?),
    )
    .map_err(|err| DecodeError::ValueInvalid {
        field: "limits",
        detail: err.to_string(),
    })?;
    let diagnostic_json = get_object(&parsed, "diagnostic")?;
    let diagnostic = DiagnosticAllowance::new(
        get_u64(&diagnostic_json, "tokens")?,
        MicroUsd::new(get_u64(&diagnostic_json, "cost_usd")?),
    );
    Ok(BudgetCeiling {
        recovery_reserve_pct: reserve,
        limits,
        diagnostic,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settlement_kind() -> RunEventKind {
        RunEventKind::SettlementCommitted {
            unit_id: UnitId::new("AK-014").expect("valid unit id"),
            evidence: EvidenceRef::new("evd-000042").expect("valid ref"),
            verification: VerificationRef::new("vrf-000042").expect("valid ref"),
            reference_digest: Digest::new("sha256", "c4d3e2f1a0b1c2d3").expect("valid digest"),
        }
    }

    #[test]
    fn events_round_trip_through_the_canonical_payload() {
        let run = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id");
        let kinds = [
            settlement_kind(),
            RunEventKind::InvocationAdmitted {
                invocation_id: InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAW")
                    .expect("valid invocation id"),
            },
            RunEventKind::InvocationTerminal {
                invocation_id: InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAW")
                    .expect("valid invocation id"),
                state: InvocationState::Completed,
            },
            RunEventKind::CheckpointWritten {
                iteration_no: IterationNo::new(2).expect("1-based"),
                event_log_offset: EventLogOffset::new(7),
            },
        ];
        for kind in &kinds {
            let (tag, payload) = encode_event(kind);
            let back = decode_event(
                run.clone(),
                EpochSeconds::new(1_700_000_000),
                &tag,
                &payload,
            )
            .expect("the canonical payload decodes");
            assert_eq!(&back.kind, kind);
        }
    }

    #[test]
    fn a_mismatched_kind_column_is_a_typed_error() {
        let run = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id");
        let (tag, payload) = encode_event(&settlement_kind());
        let _ = tag;
        let error = decode_event(
            run,
            EpochSeconds::new(1_700_000_000),
            KIND_INVOCATION_ADMITTED,
            &payload,
        )
        .expect_err("the tag disagrees with the payload");
        assert!(error.to_string().contains("kind"), "{error}");
    }

    #[test]
    fn garbage_payloads_are_typed_errors_never_panics() {
        let run = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV").expect("valid run id");
        for garbage in ["", "{", "not json at all", "{\"kind\":true}"] {
            let result = decode_event(run.clone(), EpochSeconds::new(0), KIND_SETTLEMENT, garbage);
            assert!(result.is_err(), "{garbage:?} must not decode");
        }
    }

    #[test]
    fn checkpoint_projections_round_trip() {
        let states = vec![
            (
                UnitId::new("AK-014").expect("valid unit id"),
                UnitStatus::Open,
            ),
            (
                UnitId::new("AK-015").expect("valid unit id"),
                UnitStatus::Settled,
            ),
        ];
        assert_eq!(
            decode_unit_states(&encode_unit_states(&states)).expect("decodes"),
            states
        );
        let in_flight = vec![(
            InvocationId::new("01ARZ3NDEKTSV4RRFFQ69G5FAW").expect("valid invocation id"),
            ResultContractPath::new("result-document").expect("valid path"),
        )];
        assert_eq!(
            decode_in_flight(&encode_in_flight(&in_flight)).expect("decodes"),
            in_flight
        );
        // An unknown status token is a typed error (AK-004).
        let error = decode_unit_states(r#"[["AK-014","settled-ish"]]"#).expect_err("unknown token");
        assert!(matches!(error, DecodeError::ValueInvalid { .. }), "{error}");
    }
}

// Stub encode/decode functions for capability ledger and revision ledger
// These are simplified implementations - full implementations would decode
// the domain types from JSON just like encode_event/decode_event do.
