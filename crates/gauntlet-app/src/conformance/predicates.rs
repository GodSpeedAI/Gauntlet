//! The named typed artifact predicates (charter decision 2): `file`/
//! `metric` evidence is judged by NAMED predicates implemented here —
//! never arbitrary shell, never free strings. A predicate sees the
//! artifact's bytes and returns [`Predicate::Holds`] or refutes with a
//! reason (an absent artifact never reaches a predicate — the
//! adjudicator records `no_artifact` INCONCLUSIVE first).
//!
//! Named predicates this sweep ships (widening the set is a recorded
//! change):
//!
//! - `handoff_six_categories` — the AK-067 handoff report: a non-empty
//!   text document that names all six §26 status categories;
//! - `baselines_keys_abcd` — the AK-069 baseline record: a JSON object
//!   with keys EXACTLY `A`, `B`, `C`, `D`, `_meta`, each profile a
//!   non-empty object;
//! - `mutation_record_valid` — the AK-070 mutation record: counts that
//!   reconcile (`caught + unviable + timeout + skipped + not_covered +
//!   missed == total`), ZERO missed, non-empty shards all exiting 0, a
//!   64-hex manifest pin, and a non-empty generation date.

use gauntlet_ports::json::{JsonNumber, JsonValue};

/// A predicate's verdict.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Predicate {
    Holds,
    Refutes { reason: String },
}

/// Every predicate name the manifest vocabulary admits.
pub const PREDICATES: &[&str] = &[
    "handoff_six_categories",
    "baselines_keys_abcd",
    "mutation_record_valid",
];

/// Whether `name` is a predicate this sweep implements.
pub fn is_known(name: &str) -> bool {
    PREDICATES.contains(&name)
}

/// Runs the named predicate over the artifact bytes. Unknown names
/// refuse (fail-closed; the codec also validates the name at parse
/// time — this is the second door).
pub fn evaluate(name: &str, bytes: &[u8]) -> Result<Predicate, String> {
    match name {
        "handoff_six_categories" => Ok(handoff_six_categories(bytes)),
        "baselines_keys_abcd" => Ok(baselines_keys_abcd(bytes)),
        "mutation_record_valid" => Ok(mutation_record_valid(bytes)),
        other => Err(format!("unknown artifact predicate {other:?}")),
    }
}

/// AK-067: the handoff report carries a SECTION for each of the six
/// §26 categories — a `## <category>` heading per category (a trailing
/// suffix such as a unit count is fine; the heading FORM is mandatory).
/// COR-3 F3: a bare mention of the six words (a legend, a word-salad)
/// refutes — the report must be sectioned, not a glossary.
fn handoff_six_categories(bytes: &[u8]) -> Predicate {
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => {
            return Predicate::Refutes {
                reason: "the handoff is not UTF-8 text".to_owned(),
            };
        }
    };
    if text.trim().is_empty() {
        return Predicate::Refutes {
            reason: "the handoff is empty".to_owned(),
        };
    }
    for category in [
        "implemented_and_proven",
        "implemented_but_provisional",
        "configured_but_untested",
        "proposed",
        "blocked",
        "unknown",
    ] {
        let headed = text.lines().any(|line| {
            line.trim()
                .strip_prefix("## ")
                .is_some_and(|rest| rest.split_whitespace().next() == Some(category))
        });
        if !headed {
            return Predicate::Refutes {
                reason: format!("the handoff carries no '## {category}' heading"),
            };
        }
    }
    Predicate::Holds
}

/// AK-069: the baseline record carries keys exactly A/B/C/D/_meta.
fn baselines_keys_abcd(bytes: &[u8]) -> Predicate {
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => {
            return Predicate::Refutes {
                reason: "the baseline record is not UTF-8".to_owned(),
            };
        }
    };
    let parsed = match gauntlet_ports::json::parse(text) {
        Ok(value) => value,
        Err(error) => {
            return Predicate::Refutes {
                reason: format!("the baseline record is not JSON: {error}"),
            };
        }
    };
    let mut keys = parsed.keys();
    keys.sort();
    if keys
        != vec![
            "A".to_owned(),
            "B".to_owned(),
            "C".to_owned(),
            "D".to_owned(),
            "_meta".to_owned(),
        ]
    {
        return Predicate::Refutes {
            reason: format!(
                "the baseline record's keys are not exactly A, B, C, D, _meta: {keys:?}"
            ),
        };
    }
    for profile in ["A", "B", "C", "D"] {
        match parsed.get(profile) {
            Some(JsonValue::Object(entries)) if !entries.is_empty() => {}
            other => {
                return Predicate::Refutes {
                    reason: format!("profile {profile} is not a non-empty object: {other:?}"),
                };
            }
        }
    }
    Predicate::Holds
}

fn count_of(value: &JsonValue) -> Option<u64> {
    match value {
        JsonValue::Number(JsonNumber::Uint(n)) => Some(*n),
        JsonValue::Number(JsonNumber::NegInt(_)) => None,
        JsonValue::Number(JsonNumber::Float(_)) => None,
        _ => None,
    }
}

/// AK-070: the mutation record reconciles and missed zero.
fn mutation_record_valid(bytes: &[u8]) -> Predicate {
    let text = match std::str::from_utf8(bytes) {
        Ok(text) => text,
        Err(_) => {
            return Predicate::Refutes {
                reason: "the mutation record is not UTF-8".to_owned(),
            };
        }
    };
    let parsed = match gauntlet_ports::json::parse(text) {
        Ok(value) => value,
        Err(error) => {
            return Predicate::Refutes {
                reason: format!("the mutation record is not JSON: {error}"),
            };
        }
    };
    let counts = [
        "total",
        "missed",
        "caught",
        "unviable",
        "timeout",
        "skipped",
        "not_covered",
    ];
    let mut values = Vec::new();
    for key in counts {
        match parsed.get(key).and_then(count_of) {
            Some(value) => values.push((key, value)),
            None => {
                return Predicate::Refutes {
                    reason: format!("the mutation record's {key} is not a count"),
                };
            }
        }
    }
    let lookup = |key: &str| {
        values
            .iter()
            .find(|(k, _)| *k == key)
            .map(|(_, v)| *v)
            .unwrap_or(0)
    };
    let (total, missed) = (lookup("total"), lookup("missed"));
    if missed != 0 {
        return Predicate::Refutes {
            reason: format!("the mutation run recorded {missed} missed mutants"),
        };
    }
    if total == 0 {
        return Predicate::Refutes {
            reason: "the mutation run examined nothing".to_owned(),
        };
    }
    let sum: u64 = [
        "caught",
        "unviable",
        "timeout",
        "skipped",
        "not_covered",
        "missed",
    ]
    .iter()
    .map(|key| lookup(key))
    .sum();
    if sum != total {
        return Predicate::Refutes {
            reason: format!("the mutation counts do not reconcile: {sum} of {total}"),
        };
    }
    match parsed.get("shards") {
        Some(JsonValue::Array(shards)) if !shards.is_empty() => {
            for shard in shards {
                match shard.get("shard_exit").and_then(count_of) {
                    Some(0) => {}
                    other => {
                        return Predicate::Refutes {
                            reason: format!("a mutation shard did not exit 0: {other:?}"),
                        };
                    }
                }
            }
        }
        other => {
            return Predicate::Refutes {
                reason: format!("the mutation record carries no shard array: {other:?}"),
            };
        }
    }
    match parsed.get("manifest_sha256") {
        Some(JsonValue::Str(pin)) => {
            if pin.len() != 64 || !pin.chars().all(|c| c.is_ascii_hexdigit()) {
                return Predicate::Refutes {
                    reason: "the mutation record's manifest pin is not a sha256".to_owned(),
                };
            }
        }
        other => {
            return Predicate::Refutes {
                reason: format!("the mutation record's manifest pin is missing: {other:?}"),
            };
        }
    }
    match parsed.get("generated_at") {
        Some(JsonValue::Str(stamp)) if !stamp.is_empty() => {}
        other => {
            return Predicate::Refutes {
                reason: format!("the mutation record is undated: {other:?}"),
            };
        }
    }
    Predicate::Holds
}
