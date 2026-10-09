//! Deterministic tests for the catalog builder, the schema subset, and the
//! semantic runtime. Every refusal test asserts that the handler's call counter
//! stayed at zero, so a refused call provably never reached a handler.

use std::cell::Cell;

use gauntlet_domain::{EffectClass, InvocationId};
use gauntlet_ports::json::{JsonValue, parse};
use gauntlet_ports::tool_catalog::{
    CatalogBuilder, CatalogError, CatalogSurface, ToolCatalog, ToolInvocation, ToolInvoker,
    ToolName, ToolOperation,
};
use gauntlet_ports::tool_surface::{ToolOutcome, ToolSurface};

use super::runtime::{
    EFFECT_NOT_ADMITTED, EffectPolicy, MAX_RESULT_BYTES, NO_HANDLER, RESULT_TOO_LARGE,
    RuntimeError, SemanticToolRuntime,
};
use super::schema::{self, MAX_ARGUMENT_DEPTH, SchemaError};
use crate::delegation::agents_wire::AgentsCatalog;
use crate::fabric::affordance::AGENTS_AFFORDANCES;

const CALLER_TEXT: &str = "01ARZ3NDEKTSV4RRFFQ69G5FAV";
const OTHER_CALLER_TEXT: &str = "01BX5ZZKBKACTAV9WEVGEMMVRZ";

const TYPED_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"required":["id"],"properties":{"id":{"type":"string"},"n":{"type":"integer","minimum":0}}}"#;
const EMPTY_SCHEMA: &str = r#"{"type":"object","additionalProperties":false,"properties":{}}"#;

fn caller() -> InvocationId {
    InvocationId::new(CALLER_TEXT).expect("a valid invocation id")
}

fn name(text: &str) -> ToolName {
    ToolName::new(text).expect("a valid tool name")
}

fn operation(text: &str, effect: EffectClass, schema: &str) -> ToolOperation {
    ToolOperation {
        name: name(text),
        description: format!("{text} description"),
        input_schema: parse(schema).expect("the schema is JSON"),
        effect,
    }
}

/// A catalog with a fixed operation list, in the order given.
struct FixedCatalog(Vec<ToolOperation>);

impl ToolCatalog for FixedCatalog {
    fn operations(&self) -> Vec<ToolOperation> {
        self.0.clone()
    }

    fn get(&self, target: &ToolName) -> Option<ToolOperation> {
        self.0.iter().find(|op| &op.name == target).cloned()
    }
}

fn invocation(text: &str, args: &str) -> ToolInvocation {
    ToolInvocation {
        caller: caller(),
        name: name(text),
        args: parse(args).expect("the arguments are JSON"),
    }
}

fn ok_value() -> ToolOutcome {
    ToolOutcome::Ok(JsonValue::Object(vec![(
        "done".to_string(),
        JsonValue::Bool(true),
    )]))
}

#[test]
fn duplicate_names_are_rejected_across_catalogs() {
    let first = FixedCatalog(vec![operation("ops.run", EffectClass::Read, EMPTY_SCHEMA)]);
    let second = FixedCatalog(vec![operation("ops.run", EffectClass::Read, EMPTY_SCHEMA)]);
    let error = CatalogBuilder::new()
        .with(&first)
        .and_then(|builder| builder.with(&second))
        .expect_err("a duplicate name is refused");
    assert_eq!(error, CatalogError::DuplicateName(name("ops.run")));
}

#[test]
fn a_catalog_that_repeats_a_name_is_rejected_by_the_runtime() {
    let doubled = FixedCatalog(vec![
        operation("ops.run", EffectClass::Read, EMPTY_SCHEMA),
        operation("ops.run", EffectClass::Read, EMPTY_SCHEMA),
    ]);
    let built = SemanticToolRuntime::new(&doubled, EffectPolicy::read_only());
    assert!(matches!(
        built,
        Err(RuntimeError::DuplicateOperation(ref text)) if text == "ops.run"
    ));
}

#[test]
fn builder_keeps_insertion_order_and_is_deterministic() {
    let first = FixedCatalog(vec![
        operation("zeta.one", EffectClass::Read, EMPTY_SCHEMA),
        operation("alpha.two", EffectClass::Read, EMPTY_SCHEMA),
    ]);
    let second = FixedCatalog(vec![operation(
        "mid.three",
        EffectClass::Read,
        EMPTY_SCHEMA,
    )]);
    let composite = CatalogBuilder::new()
        .with(&first)
        .and_then(|builder| builder.with(&second))
        .expect("disjoint names merge")
        .build();
    let names: Vec<String> = composite
        .operations()
        .iter()
        .map(|op| op.name.as_str().to_string())
        .collect();
    assert_eq!(names, ["zeta.one", "alpha.two", "mid.three"]);
    assert_eq!(composite.operations(), composite.operations());
    assert_eq!(
        composite.get(&name("alpha.two")).map(|op| op.name),
        Some(name("alpha.two"))
    );
}

#[test]
fn tool_names_follow_the_grammar() {
    assert!(ToolName::new("agents.spawn").is_ok());
    assert!(ToolName::new("a_b.c9").is_ok());
    for bad in [
        "",
        "Agents.spawn",
        "9agents",
        "agents-spawn",
        "agents spawn",
    ] {
        assert!(ToolName::new(bad).is_err(), "{bad:?} is refused");
    }
    let too_long = "a".repeat(65);
    assert!(ToolName::new(&too_long).is_err(), "65 bytes is refused");
    assert!(
        ToolName::new(&"a".repeat(64)).is_ok(),
        "64 bytes is admitted"
    );
}

#[test]
fn unknown_tool_is_refused_before_any_handler() {
    let catalog = FixedCatalog(vec![operation("ops.run", EffectClass::Read, EMPTY_SCHEMA)]);
    let calls = Cell::new(0_u32);
    let mut runtime =
        SemanticToolRuntime::new(&catalog, EffectPolicy::read_only()).expect("the catalog builds");
    runtime
        .register(&name("ops.run"), |_: &ToolInvocation| {
            calls.set(calls.get() + 1);
            ok_value()
        })
        .expect("the operation is registered");
    let outcome = runtime.invoke(&invocation("ops.teleport", "{}"));
    assert_eq!(
        outcome,
        ToolOutcome::Invalid {
            detail: "unknown tool: ops.teleport".to_string()
        }
    );
    assert_eq!(calls.get(), 0);
}

#[test]
fn schema_violations_are_refused_before_the_handler_runs() {
    let catalog = FixedCatalog(vec![operation("ops.run", EffectClass::Read, TYPED_SCHEMA)]);
    let calls = Cell::new(0_u32);
    let mut runtime =
        SemanticToolRuntime::new(&catalog, EffectPolicy::read_only()).expect("the catalog builds");
    runtime
        .register(&name("ops.run"), |_: &ToolInvocation| {
            calls.set(calls.get() + 1);
            ok_value()
        })
        .expect("registered");

    let deep = format!("{}{}", "[".repeat(20), "]".repeat(20));
    let cases: Vec<(&str, String, &str)> = vec![
        ("missing required", "{}".to_string(), "missing field id"),
        (
            "extra key",
            r#"{"id":"x","extra":1}"#.to_string(),
            "arguments.extra: unknown field",
        ),
        (
            "wrong type",
            r#"{"id":7}"#.to_string(),
            "arguments.id: expected a string",
        ),
        (
            "below minimum",
            r#"{"id":"x","n":-1}"#.to_string(),
            "arguments.n: below the minimum 0",
        ),
        (
            "oversize",
            format!(r#"{{"id":"{}"}}"#, "x".repeat(schema::MAX_ARGUMENT_BYTES)),
            "arguments: exceeds",
        ),
        (
            "too deep",
            format!(r#"{{"id":{deep}}}"#),
            "arguments: nests deeper",
        ),
    ];
    for (label, args, expected) in cases {
        let outcome = runtime.invoke(&invocation("ops.run", &args));
        match outcome {
            ToolOutcome::Invalid { detail } => {
                assert!(detail.contains(expected), "{label}: {detail}");
            }
            other => panic!("{label} is Invalid, got {other:?}"),
        }
    }
    assert_eq!(calls.get(), 0, "no refused call reached the handler");
}

#[test]
fn a_fractional_number_is_not_an_integer() {
    let catalog = FixedCatalog(vec![operation("ops.run", EffectClass::Read, TYPED_SCHEMA)]);
    let runtime =
        SemanticToolRuntime::new(&catalog, EffectPolicy::read_only()).expect("the catalog builds");
    let outcome = runtime.invoke(&invocation("ops.run", r#"{"id":"x","n":1.5}"#));
    assert!(matches!(
        outcome,
        ToolOutcome::Invalid { ref detail } if detail == "arguments.n: expected an integer"
    ));
}

#[test]
fn caller_identity_cannot_be_supplied_by_arguments() {
    let catalog = FixedCatalog(vec![operation("ops.run", EffectClass::Read, TYPED_SCHEMA)]);
    let calls = Cell::new(0_u32);
    let mut runtime =
        SemanticToolRuntime::new(&catalog, EffectPolicy::read_only()).expect("the catalog builds");
    runtime
        .register(&name("ops.run"), |_: &ToolInvocation| {
            calls.set(calls.get() + 1);
            ok_value()
        })
        .expect("registered");
    let outcome = runtime.invoke(&invocation(
        "ops.run",
        &format!(r#"{{"id":"x","caller":"{OTHER_CALLER_TEXT}"}}"#),
    ));
    assert!(matches!(
        outcome,
        ToolOutcome::Invalid { ref detail } if detail == "arguments.caller: unknown field"
    ));
    assert_eq!(calls.get(), 0);
}

#[test]
fn effect_refusal_is_typed_and_runs_nothing() {
    let catalog = FixedCatalog(vec![
        operation("ops.read", EffectClass::Read, EMPTY_SCHEMA),
        operation("ops.spawn", EffectClass::ControlPlane, EMPTY_SCHEMA),
    ]);
    let calls = Cell::new(0_u32);
    let mut runtime =
        SemanticToolRuntime::new(&catalog, EffectPolicy::read_only()).expect("the catalog builds");
    for text in ["ops.read", "ops.spawn"] {
        runtime
            .register(&name(text), |_: &ToolInvocation| {
                calls.set(calls.get() + 1);
                ok_value()
            })
            .expect("registered");
    }
    assert!(matches!(
        runtime.invoke(&invocation("ops.read", "{}")),
        ToolOutcome::Ok(_)
    ));
    assert_eq!(calls.get(), 1);
    match runtime.invoke(&invocation("ops.spawn", "{}")) {
        ToolOutcome::Refused { code, .. } => assert_eq!(code, EFFECT_NOT_ADMITTED),
        other => panic!("a read-only caller is refused, got {other:?}"),
    }
    assert_eq!(calls.get(), 1, "the refused call did not run");
}

#[test]
fn a_missing_handler_is_a_typed_refusal() {
    let catalog = FixedCatalog(vec![operation("ops.run", EffectClass::Read, EMPTY_SCHEMA)]);
    let runtime =
        SemanticToolRuntime::new(&catalog, EffectPolicy::read_only()).expect("the catalog builds");
    match runtime.invoke(&invocation("ops.run", "{}")) {
        ToolOutcome::Refused { code, .. } => assert_eq!(code, NO_HANDLER),
        other => panic!("no handler is refused, got {other:?}"),
    }
}

#[test]
fn an_oversize_result_is_refused_not_truncated() {
    let catalog = FixedCatalog(vec![operation("ops.run", EffectClass::Read, EMPTY_SCHEMA)]);
    let mut runtime =
        SemanticToolRuntime::new(&catalog, EffectPolicy::read_only()).expect("the catalog builds");
    runtime
        .register(&name("ops.run"), |_: &ToolInvocation| {
            ToolOutcome::Ok(JsonValue::Str("x".repeat(MAX_RESULT_BYTES)))
        })
        .expect("registered");
    match runtime.invoke(&invocation("ops.run", "{}")) {
        ToolOutcome::Refused { code, .. } => assert_eq!(code, RESULT_TOO_LARGE),
        other => panic!("an oversize result is refused, got {other:?}"),
    }
}

#[test]
fn schemas_outside_the_subset_fail_the_build() {
    let open = FixedCatalog(vec![operation(
        "ops.run",
        EffectClass::Read,
        r#"{"type":"object","properties":{}}"#,
    )]);
    assert!(matches!(
        SemanticToolRuntime::new(&open, EffectPolicy::read_only()),
        Err(RuntimeError::Schema { .. })
    ));
    let keyword = FixedCatalog(vec![operation(
        "ops.run",
        EffectClass::Read,
        r#"{"type":"object","additionalProperties":false,"properties":{},"pattern":"x"}"#,
    )]);
    assert!(matches!(
        SemanticToolRuntime::new(&keyword, EffectPolicy::read_only()),
        Err(RuntimeError::Schema { .. })
    ));
    let bad = parse(
        r#"{"type":"object","additionalProperties":false,"properties":{},"required":["ghost"]}"#,
    )
    .expect("a well-formed document");
    assert!(matches!(
        schema::check_subset(&bad),
        Err(SchemaError::Malformed { .. })
    ));
}

#[test]
fn argument_depth_bound_is_enforced_at_the_boundary() {
    let at_bound = nested(MAX_ARGUMENT_DEPTH);
    assert!(schema::check_argument_bounds(&at_bound).is_ok());
    let past_bound = nested(MAX_ARGUMENT_DEPTH + 1);
    assert!(schema::check_argument_bounds(&past_bound).is_err());
}

fn nested(levels: usize) -> JsonValue {
    let mut value = JsonValue::Null;
    for _ in 0..levels {
        value = JsonValue::Array(vec![value]);
    }
    value
}

#[test]
fn catalog_surface_lists_exactly_the_catalog() {
    let catalog = FixedCatalog(vec![
        operation("ops.one", EffectClass::Read, TYPED_SCHEMA),
        operation("ops.two", EffectClass::Read, EMPTY_SCHEMA),
    ]);
    let runtime =
        SemanticToolRuntime::new(&catalog, EffectPolicy::read_only()).expect("the catalog builds");
    let surface = CatalogSurface::new(&catalog, &runtime, caller());
    let listed: Vec<(String, String)> = surface
        .tools()
        .into_iter()
        .map(|listing| (listing.name, listing.input_schema))
        .collect();
    let expected: Vec<(String, String)> = catalog
        .operations()
        .iter()
        .map(|op| (op.name.as_str().to_string(), op.input_schema.to_json_text()))
        .collect();
    assert_eq!(listed, expected);
}

#[test]
fn one_catalog_drives_two_projections() {
    let catalog = FixedCatalog(vec![
        operation("ops.read", EffectClass::Read, TYPED_SCHEMA),
        operation("ops.spawn", EffectClass::ControlPlane, EMPTY_SCHEMA),
    ]);
    let runtime = SemanticToolRuntime::new(
        &catalog,
        EffectPolicy::new(&[EffectClass::Read, EffectClass::ControlPlane]),
    )
    .expect("the catalog builds");
    let protocol: Vec<String> = CatalogSurface::new(&catalog, &runtime, caller())
        .tools()
        .into_iter()
        .map(|listing| listing.name)
        .collect();
    // A second projection: a plain signature list, not a protocol.
    let signatures: Vec<String> = catalog
        .operations()
        .iter()
        .map(|op| op.name.as_str().to_string())
        .collect();
    assert_eq!(protocol, signatures);
    assert_eq!(protocol, ["ops.read", "ops.spawn"]);
}

#[test]
fn agents_catalog_listing_is_byte_identical_to_the_descriptors() {
    let catalog = AgentsCatalog;
    let operations = catalog.operations();
    assert_eq!(
        operations.len(),
        AGENTS_AFFORDANCES.len(),
        "no operation is dropped"
    );
    for (operation, descriptor) in operations.iter().zip(AGENTS_AFFORDANCES) {
        assert_eq!(operation.name.as_str(), descriptor.name);
        assert_eq!(operation.description, descriptor.summary);
        assert_eq!(operation.effect, descriptor.effect);
        assert_eq!(
            operation.input_schema.to_json_text(),
            descriptor.input_schema,
            "{} schema text is canonical",
            descriptor.name
        );
    }
}
