//! The candidate change as a configuration delta (plan Task 23, charter
//! D11): a candidate change is a delta over the effective-config key
//! vocabulary — DECLARED keys only, validated against the config schema —
//! and the gate's verification-bearing classification is a typed TOTAL
//! function over that vocabulary, fail-closed on unknown keys (no
//! default-allow).
//!
//! The delta document is the canonical JSON array
//! `[{"key":"<§8.2 path>","value":"<raw token>"}]`, hand-rolled (the
//! workspace has no serde). The document's bytes are pinned in the
//! evidence store at the payload's `candidate_change` ref by the flow that
//! admits or evaluates the revision; this module owns the PARSE and the
//! CLASSIFICATION, not the storage.

use core::fmt;

use gauntlet_ports::json::{JsonError, JsonValue};
use gauntlet_ports::revision_ledger::GateRefusal;

use crate::config::resolve::ConfigKey;

/// One declared delta entry: the §8.2 key path and the raw token value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeltaEntry {
    pub key: String,
    pub value: String,
}

/// A parsed candidate-configuration delta.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeltaDocument {
    entries: Vec<DeltaEntry>,
}

/// Why a delta document refused to parse.
#[derive(Debug, Clone, PartialEq)]
pub enum DeltaError {
    /// The document is not valid JSON.
    Json(JsonError),
    /// The document is not an array of single-pair objects.
    Shape { detail: String },
    /// A declared key is outside the config vocabulary — the strictness is
    /// the point: a candidate may only change settings the schema names.
    UnknownKey { key: String },
}

impl fmt::Display for DeltaError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Json(error) => write!(f, "the candidate delta is not valid JSON: {error}"),
            Self::Shape { detail } => {
                write!(f, "the candidate delta's shape is wrong: {detail}")
            }
            Self::UnknownKey { key } => write!(
                f,
                "the candidate delta declares key {key:?}, which is outside the §8.2 config \
                 vocabulary — declared keys only (charter D11)"
            ),
        }
    }
}

impl std::error::Error for DeltaError {}

/// The typed class of one config key under the gate's condition 3.
///
/// - [`KeyClass::VerificationBearing`] — the key carries verification or
///   review strength (the deterministic-verification preference, the
///   review mode and critic strength, the observation facets). ANY change
///   that removes, disables, or downgrades it is
///   `SettlementAccessWeakened`; because the durable record does not carry
///   the baseline value the candidate would be measured against, ANY
///   change is refused except where the preserving value is itself typed
///   (`verification.prefer_deterministic = true` is the only §22 posture
///   that preserves settlement access).
/// - [`KeyClass::Inert`] — the key is a budget ceiling, a runner/model
///   selection, or an operator-surface display token: no verification,
///   governance, or durability surface moves when it changes.
/// - [`KeyClass::Protected`] — everything else, INCLUDING unknown keys:
///   fail-closed. A key this classification cannot name as inert cannot
///   be changed by a candidate (no default-allow).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyClass {
    VerificationBearing,
    Inert,
    Protected,
}

impl DeltaDocument {
    /// Strict parse: a canonical JSON array of `{"key","value"}` objects;
    /// unknown fields inside an entry, non-string leaves, and keys outside
    /// the config vocabulary are typed refusals. An EMPTY delta parses —
    /// a candidate that changes nothing is inert, and the gate's margin
    /// condition will refuse it on the merits instead.
    pub fn parse(text: &str) -> Result<Self, DeltaError> {
        let document = gauntlet_ports::json::parse(text).map_err(DeltaError::Json)?;
        let JsonValue::Array(items) = &document else {
            return Err(DeltaError::Shape {
                detail: "the document is a JSON array of {key,value} objects".to_owned(),
            });
        };
        let mut entries = Vec::with_capacity(items.len());
        for item in items {
            let JsonValue::Object(pairs) = item else {
                return Err(DeltaError::Shape {
                    detail: "every delta entry is an object".to_owned(),
                });
            };
            let mut key: Option<String> = None;
            let mut value: Option<String> = None;
            for (name, leaf) in pairs.iter() {
                match name.as_str() {
                    "key" => match leaf {
                        JsonValue::Str(text) => key = Some(text.clone()),
                        _ => {
                            return Err(DeltaError::Shape {
                                detail: "the entry's key is a string".to_owned(),
                            });
                        }
                    },
                    "value" => match leaf {
                        JsonValue::Str(text) => value = Some(text.clone()),
                        _ => {
                            return Err(DeltaError::Shape {
                                detail: "the entry's value is a string".to_owned(),
                            });
                        }
                    },
                    other => {
                        return Err(DeltaError::Shape {
                            detail: format!("the delta entry carries unknown field {other:?}"),
                        });
                    }
                }
            }
            let key = key.ok_or_else(|| DeltaError::Shape {
                detail: "every delta entry carries a key".to_owned(),
            })?;
            let value = value.ok_or_else(|| DeltaError::Shape {
                detail: "every delta entry carries a value".to_owned(),
            })?;
            if ConfigKey::from_path(&key).is_none() {
                return Err(DeltaError::UnknownKey { key });
            }
            entries.push(DeltaEntry { key, value });
        }
        Ok(Self { entries })
    }

    pub fn entries(&self) -> &[DeltaEntry] {
        &self.entries
    }

    /// Condition 3's typed total function over the delta: `Ok(())` when
    /// the delta preserves settlement access; the typed
    /// [`GateRefusal::SettlementAccessWeakened`] — with the offending key
    /// and its class in the detail — otherwise. Unknown keys classify as
    /// protected: fail-closed, no default-allow.
    pub fn settlement_access_verdict(&self) -> Result<(), GateRefusal> {
        for entry in &self.entries {
            match classify_key(&entry.key) {
                KeyClass::Inert => {}
                KeyClass::Protected => {
                    return Err(GateRefusal::SettlementAccessWeakened {
                        detail: format!(
                            "key {} is protected (fail-closed: the classification names it \
                             neither inert nor preservable) — a candidate may not change it",
                            entry.key
                        ),
                    });
                }
                KeyClass::VerificationBearing => {
                    if !preserves_verification(&entry.key, &entry.value) {
                        return Err(GateRefusal::SettlementAccessWeakened {
                            detail: format!(
                                "key {} is verification-bearing and the delta sets it to {:?}, \
                                 which removes, disables, or downgrades verification or review \
                                 strength",
                                entry.key, entry.value
                            ),
                        });
                    }
                }
            }
        }
        Ok(())
    }

    /// The tokenized form the CLI's executor seam applies as configuration
    /// overrides: `(key, value)` pairs, in declaration order.
    pub fn overrides(&self) -> Vec<(String, String)> {
        self.entries
            .iter()
            .map(|entry| (entry.key.clone(), entry.value.clone()))
            .collect()
    }
}

/// The typed total classification over the config key vocabulary (the
/// module docs carry the class rules; this function IS the rule's
/// implementation — every key resolves to exactly one class, unknown keys
/// to `Protected`).
pub fn classify_key(path: &str) -> KeyClass {
    match path {
        "verification.prefer_deterministic"
        | "review.mode"
        | "review.max_critics"
        | "observation.facets" => KeyClass::VerificationBearing,
        "budget.max_rounds"
        | "budget.max_parallel"
        | "budget.max_depth"
        | "budget.max_wall_seconds"
        | "budget.max_tokens"
        | "budget.max_cost_usd"
        | "budget.recovery_reserve_pct"
        | "budget.diagnostic_budget"
        // The per-unit ATTEMPT ceiling is recovery-PERSISTENCE policy: it
        // bounds how many corrective re-dispatches a failing unit may
        // consume, not what evidence a settlement demands. On C1's ground
        // (COR-1, critic m6 — the old "fewer settlements ⇒ the margin
        // refuses" argument only ever held via the deleted sentinel): the
        // WEAKENING direction (fewer attempts) starves a hard unit, so the
        // arm cannot reach a settled terminal, the metric figures are
        // UNDERIVABLE, and the completion gate adjudicates Provisional —
        // never promoted, never rejected. The strengthening direction only
        // adds persistence headroom and pays for it in the run's own
        // booked figures. Pinned by
        // `a_reduced_attempt_ceiling_cannot_promote_through_the_completion_gate`.
        | "max_attempts_per_unit"
        | "runners.default"
        | "operator.surfaces"
        | "interactive" => KeyClass::Inert,
        _ => {
            if path.starts_with("runners.by_role.") {
                // A role's runner/model selection: capability-neutral
                // dispatch plumbing, the same class as `runners.default`.
                let tail = path.strip_prefix("runners.by_role.").unwrap_or(path);
                let is_selection_leaf = tail.ends_with(".runner") || tail.ends_with(".model");
                if is_selection_leaf {
                    return KeyClass::Inert;
                }
            }
            KeyClass::Protected
        }
    }
}

/// Whether the given value for a verification-bearing key PRESERVES
/// settlement access. Only one change survives: setting
/// `verification.prefer_deterministic` to `true` — the §22 posture
/// ("prefer deterministic verification wherever deterministic evidence
/// exists") is the only value of that key that does not remove, disable,
/// or downgrade the deterministic rung. Every other verification-bearing
/// change refuses (the durable record carries no baseline to measure a
/// "strengthening" against, so strengthening is not provable either).
fn preserves_verification(key: &str, value: &str) -> bool {
    key == "verification.prefer_deterministic" && value == "true"
}
