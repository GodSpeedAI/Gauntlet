//! The harness-neutral affordance catalog for delegated agents (F1).
//!
//! Each descriptor names one model-facing operation and its effect class. The
//! catalog is neutral to every projection (a Python binding, a JavaScript
//! binding, a tool-protocol server, or a UI): it carries JSON Schema text and
//! nothing projection-specific. A projection maps each descriptor to its own
//! calling convention and must keep the schema's strictness.
//!
//! The schemas are strict: every object declares `additionalProperties: false`
//! and lists its required fields. No schema names a parent. The parent of a
//! spawned child is always the calling invocation, so a caller cannot choose it.

pub use gauntlet_domain::EffectClass;

/// One model-facing operation. Schemas are JSON Schema documents as text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AffordanceDescriptor {
    pub name: &'static str,
    pub summary: &'static str,
    pub input_schema: &'static str,
    pub output_schema: &'static str,
    pub effect: EffectClass,
}

const TARGET_INPUT: &str = r#"{"type":"object","additionalProperties":false,"required":["invocation_id"],"properties":{"invocation_id":{"type":"string"}}}"#;

const LIST_INPUT: &str = r#"{"type":"object","additionalProperties":false,"properties":{}}"#;

const SPAWN_INPUT: &str = concat!(
    r#"{"type":"object","additionalProperties":false,"required":["role","items","budget","idempotency_key","deadline_seconds"],"properties":{"role":{"type":"string"},"items":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["kind","label","content"],"properties":{"kind":{"type":"string","enum":["task-statement","reference-excerpt","file-excerpt","evidence-reference","constraint-statement","result-schema"]},"label":{"type":"string"},"content":{"type":"string"}}}},"#,
    r#""requirements":{"type":"object","additionalProperties":false,"properties":{"workspace_mutation":{"type":"string","enum":["required","forbidden","any"]},"min_support_level":{"type":"integer","minimum":0,"description":"the support ladder rung: 0 (process) through 4 (native)"},"needs_resume":{"type":"boolean"},"needs_model_selection":{"type":"boolean"},"needs_native_subagents":{"type":"boolean"},"needs_streaming":{"type":"boolean"},"needs_structured_lifecycle":{"type":"boolean"},"min_isolation":{"type":"string","enum":["none","process","container","vm"]},"min_depth_support":{"type":"integer","minimum":0,"description":"the depth the runner must support, at most 255"}}},"#,
    r#""preference":{"type":"object","additionalProperties":false,"properties":{"pin":{"type":"string"},"preferred_harness":{"type":"string"},"preferred_model":{"type":"string"}}},"budget":{"type":"object","additionalProperties":false,"required":["rounds","wall_seconds","tokens","cost_usd_micros"],"properties":{"rounds":{"type":"integer","minimum":0},"wall_seconds":{"type":"integer","minimum":0},"tokens":{"type":"integer","minimum":0},"cost_usd_micros":{"type":"integer","minimum":0}}},"idempotency_key":{"type":"string"},"#,
    r#""deadline_seconds":{"type":"integer","#,
    r#""minimum":1}}}"#,
);

const SPAWN_OUTPUT: &str = r#"{"type":"object","additionalProperties":false,"required":["invocation_id","depth","runner_profile","harness","launched"],"properties":{"invocation_id":{"type":"string"},"depth":{"type":"integer"},"runner_profile":{"type":"string"},"harness":{"type":"string"},"launched":{"type":"boolean"}}}"#;

const AGENT_VIEW: &str = r#"{"type":"object","additionalProperties":false,"required":["invocation_id","status","depth","harness","runner_profile","parent","native_session_epoch"],"properties":{"invocation_id":{"type":"string"},"status":{"type":"string","enum":["admitted","running","completed","failed","timed_out","cancelled"]},"depth":{"type":"integer","minimum":0},"harness":{"type":"string"},"runner_profile":{"type":"string"},"parent":{"type":["string","null"]},"native_session_epoch":{"type":["integer","null"],"minimum":0}}}"#;

const COLLECT_OUTPUT: &str = r#"{"type":"object","additionalProperties":false,"required":["state","note"],"properties":{"state":{"type":"string","enum":["still_running","completed","failed","timed_out","cancelled","already_terminal"]},"claim":{"type":"string","description":"an untrusted claim: the child completed its contract, which is not settlement"},"runner_unavailable":{"type":"boolean","description":"true only when the node is live but its runner could not answer; the node is unchanged"},"note":{"type":"string"}}}"#;

const CANCEL_OUTPUT: &str = r#"{"type":"object","additionalProperties":false,"required":["cancelled","unconfirmed"],"properties":{"cancelled":{"type":"array","items":{"type":"string"}},"unconfirmed":{"type":"array","items":{"type":"string"}}}}"#;

const LIST_OUTPUT: &str = r#"{"type":"object","additionalProperties":false,"required":["children"],"properties":{"children":{"type":"array","items":{"type":"object","additionalProperties":false,"required":["invocation_id","status","depth","harness","runner_profile","parent","native_session_epoch"],"properties":{"invocation_id":{"type":"string"},"status":{"type":"string","enum":["admitted","running","completed","failed","timed_out","cancelled"]},"depth":{"type":"integer","minimum":0},"harness":{"type":"string"},"runner_profile":{"type":"string"},"parent":{"type":["string","null"]},"native_session_epoch":{"type":["integer","null"],"minimum":0}}}}}}"#;

/// The five agent affordances, in the order a projection presents them.
pub const AGENTS_AFFORDANCES: &[AffordanceDescriptor] = &[
    AffordanceDescriptor {
        name: "agents.spawn",
        summary: "Start one delegated child of the calling invocation with fresh context.",
        input_schema: SPAWN_INPUT,
        output_schema: SPAWN_OUTPUT,
        effect: EffectClass::ControlPlane,
    },
    AffordanceDescriptor {
        name: "agents.status",
        summary: "Read the state of one of your descendants.",
        input_schema: TARGET_INPUT,
        output_schema: AGENT_VIEW,
        effect: EffectClass::Read,
    },
    AffordanceDescriptor {
        name: "agents.collect",
        summary: "Poll one of your descendants; a completed result is an untrusted claim.",
        input_schema: TARGET_INPUT,
        output_schema: COLLECT_OUTPUT,
        effect: EffectClass::Read,
    },
    AffordanceDescriptor {
        name: "agents.cancel",
        summary: "Cancel one of your descendants and its subtree, leaf-first.",
        input_schema: TARGET_INPUT,
        output_schema: CANCEL_OUTPUT,
        effect: EffectClass::ControlPlane,
    },
    AffordanceDescriptor {
        name: "agents.list",
        summary: "List your direct children with their state.",
        input_schema: LIST_INPUT,
        output_schema: LIST_OUTPUT,
        effect: EffectClass::Read,
    },
];

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use gauntlet_ports::json::{JsonValue, parse};

    use super::{AGENTS_AFFORDANCES, EffectClass};
    use crate::delegation::ContextItemKind;

    fn descriptor(name: &str) -> &'static super::AffordanceDescriptor {
        AGENTS_AFFORDANCES
            .iter()
            .find(|descriptor| descriptor.name == name)
            .expect("the catalog names the operation")
    }

    /// Every object that declares properties is closed and lists only declared
    /// names as required. Recurses through nested objects and item schemas.
    fn assert_strict(value: &JsonValue, path: &str) {
        let JsonValue::Object(_) = value else {
            return;
        };
        if let Some(JsonValue::Object(declared)) = value.get("properties") {
            assert_eq!(
                value.get("additionalProperties"),
                Some(&JsonValue::Bool(false)),
                "{path} must set additionalProperties to false"
            );
            if let Some(JsonValue::Array(required)) = value.get("required") {
                for name in required {
                    let JsonValue::Str(name) = name else {
                        panic!("{path}: required entries are strings");
                    };
                    assert!(
                        declared.iter().any(|(key, _)| key == name),
                        "{path}: required '{name}' is not a declared property"
                    );
                }
            }
            for (key, child) in declared {
                assert_strict(child, &format!("{path}.{key}"));
            }
        }
        if let Some(items) = value.get("items") {
            assert_strict(items, &format!("{path}[]"));
        }
    }

    fn parsed(text: &str) -> JsonValue {
        parse(text).expect("the schema is valid JSON")
    }

    #[test]
    fn agents_catalog_has_five_unique_descriptors() {
        let names: Vec<&str> = AGENTS_AFFORDANCES.iter().map(|d| d.name).collect();
        assert_eq!(
            names,
            [
                "agents.spawn",
                "agents.status",
                "agents.collect",
                "agents.cancel",
                "agents.list",
            ]
        );
        let unique: BTreeSet<&str> = names.iter().copied().collect();
        assert_eq!(unique.len(), names.len(), "names are unique");
        for entry in AGENTS_AFFORDANCES {
            assert!(
                !entry.summary.trim().is_empty(),
                "{} has a summary",
                entry.name
            );
        }
    }

    #[test]
    fn agents_catalog_effect_classes_match_the_spec() {
        assert_eq!(descriptor("agents.spawn").effect, EffectClass::ControlPlane);
        assert_eq!(
            descriptor("agents.cancel").effect,
            EffectClass::ControlPlane
        );
        assert_eq!(descriptor("agents.status").effect, EffectClass::Read);
        assert_eq!(descriptor("agents.collect").effect, EffectClass::Read);
        assert_eq!(descriptor("agents.list").effect, EffectClass::Read);
    }

    #[test]
    fn agents_catalog_schemas_parse_and_are_strict() {
        for entry in AGENTS_AFFORDANCES {
            assert_strict(
                &parsed(entry.input_schema),
                &format!("{}.input", entry.name),
            );
            assert_strict(
                &parsed(entry.output_schema),
                &format!("{}.output", entry.name),
            );
        }
    }

    #[test]
    fn agents_spawn_schema_rejects_extra_properties() {
        let text = descriptor("agents.spawn").input_schema;
        assert!(
            text.contains(r#""additionalProperties":false"#),
            "the root is closed textually"
        );
        let schema = parsed(text);
        assert_eq!(
            schema.get("additionalProperties"),
            Some(&JsonValue::Bool(false)),
            "the root rejects extra properties"
        );
        let Some(JsonValue::Object(declared)) = schema.get("properties") else {
            panic!("the spawn input declares properties");
        };
        let keys: Vec<&str> = declared.iter().map(|(key, _)| key.as_str()).collect();
        for forbidden in ["parent", "parent_id", "invocation_parent"] {
            assert!(
                !keys.contains(&forbidden),
                "spawn never names a parent: {forbidden}"
            );
        }
    }

    #[test]
    fn agents_spawn_kind_enum_matches_context_item_kinds() {
        let schema = parsed(descriptor("agents.spawn").input_schema);
        let Some(JsonValue::Object(properties)) = schema.get("properties") else {
            panic!("the spawn input declares properties");
        };
        let items = properties
            .iter()
            .find(|(key, _)| key == "items")
            .map(|(_, value)| value)
            .and_then(|value| value.get("items"))
            .expect("items declares an item schema");
        let Some(JsonValue::Object(item_properties)) = items.get("properties") else {
            panic!("an item declares properties");
        };
        let kind = item_properties
            .iter()
            .find(|(key, _)| key == "kind")
            .map(|(_, value)| value)
            .expect("an item declares its kind");
        let Some(JsonValue::Array(enumerated)) = kind.get("enum") else {
            panic!("the kind is an enumeration");
        };
        let tokens: BTreeSet<&str> = enumerated
            .iter()
            .filter_map(|value| match value {
                JsonValue::Str(token) => Some(token.as_str()),
                _ => None,
            })
            .collect();
        let expected: BTreeSet<&str> = [
            ContextItemKind::TaskStatement,
            ContextItemKind::ReferenceExcerpt,
            ContextItemKind::FileExcerpt,
            ContextItemKind::EvidenceReference,
            ContextItemKind::ConstraintStatement,
            ContextItemKind::ResultSchema,
        ]
        .into_iter()
        .map(ContextItemKind::token)
        .collect();
        assert_eq!(
            tokens, expected,
            "the kind enumeration is the closed vocabulary"
        );
    }
}
