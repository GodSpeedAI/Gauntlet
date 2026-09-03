//! The JSON operator surface adapter (spec §6.2, §20.2; plan Task 21,
//! charter D2): a std-only HTTP/1.1 server over `std::net::TcpListener`
//! that serves the `OperatorSurface` port — the runtime snapshot, the
//! §20.2 read endpoints, and the action endpoints.
//!
//! The adapter fronts NO products and NO application layer: its
//! dependencies are `gauntlet-ports` + `gauntlet-domain` only. The
//! runtime's facts arrive through two narrow seams the composition root
//! (the only crate allowed to wire adapters into the application layer)
//! implements:
//!
//! - [`OperatorSurface`] (the existing port) — the snapshot reads;
//! - [`RunScopedReader`] — the run-scoped and unit-scoped reads, over
//!   the SAME durable reads the CLI's status/inspect commands use
//!   (charter D6: no invented read port, no silent guess);
//! - [`OperatorActionHandler`] — the ONE operator-action path (charter
//!   D3): the composition root implements it over the settlement
//!   service's operator gate + governed verification chain, so the API's
//!   actions perform the SAME atomic commit sequence as automatic
//!   settlement (§16.3) and can never settle a Unit lacking PASS
//!   evidence (AK-062). NO settlement type crosses this seam — the
//!   handler speaks in accepted/refused/failed facts only.
//!
//! Routes (spec §20.2, exact): `GET /api/v1/state`,
//! `GET /api/v1/runs/{id}`, `GET /api/v1/units/{id}`,
//! `GET /api/v1/escalated`; `POST /api/v1/refresh`,
//! `POST /api/v1/units/{id}/resolve`, `POST /api/v1/units/{id}/retry`.
//! Errors carry the §20.2 body shape `{"error":{"code":"..","message":".."}}`;
//! unsupported methods are `405` (with `Allow`), unknown routes `404`,
//! malformed requests `400`. Every bind is loopback-only; request sizes
//! are bounded; every request runs under a deadline (§20.2: the surface
//! is additional observability, never load-bearing — per §21.3 a surface
//! failure never crashes the run).
//!
//! JSON follows the in-tree hand-rolled precedent: strings escaped,
//! canonical field order = the run preamble first, then the §20.2
//! sections in their priority order (the escalation queue first among
//! them — this is also the port's own field order), the `run_derivation`
//! disclosure last; no external serialization dependency. (Recorded
//! charter tension: D2's "escalated FIRST" holds among the §20.2
//! sections, not over the whole body — the run preamble precedes them.)

pub mod action;
pub mod http;
pub mod memory;
pub mod views;

pub use action::{
    ActionAccepted, ActionCommand, ActionFailure, ActionKind, ActionOutcome, ActionRefusal,
    OperatorActionHandler, SnapshotDisclosure,
};
pub use http::{SurfaceHttp, SurfaceHttpError};
pub use memory::MemorySurface;
pub use views::{RunScopedReader, RunView, SettlementFact, UnitView, VerificationFact};

/// The suite-facing factory (the P8 pattern): the shared contract suite
/// builds the surface under test over its own seeded snapshot; the echo
/// source makes the projection checkable without any store behind it.
pub mod testing {
    use gauntlet_ports::contract::operator_surface::OperatorSurfaceFactory;
    use gauntlet_ports::operator_surface::{OperatorSurface, RuntimeSnapshot};

    use crate::MemorySurface;

    /// The factory the shared `operator_surface_suite` runs against.
    pub struct MemorySurfaceFactory;

    impl OperatorSurfaceFactory for MemorySurfaceFactory {
        fn label(&self) -> &'static str {
            "memory-surface"
        }

        fn build(&self, seeded: RuntimeSnapshot) -> Result<Box<dyn OperatorSurface>, String> {
            Ok(Box::new(MemorySurface::new(seeded)))
        }
    }
}

use gauntlet_ports::budget_meter::Remaining;
use gauntlet_ports::json::JsonValue;
use gauntlet_ports::operator_surface::{RuntimeSnapshot, UnitSnapshot};

/// Encodes a [`RuntimeSnapshot`] as the `/api/v1/state` response body.
/// Field order: the run preamble first (the port's own first field — not
/// a §20.2 priority section), then the §20.2 sections in their priority
/// order (the escalation queue first among them), and `run_derivation`
/// appended last as the D6 disclosure (the derivation is never a silent
/// guess — the response names how the active run was derived). The
/// tests pin this order.
pub fn snapshot_json(snapshot: &RuntimeSnapshot, run_derivation: &str) -> JsonValue {
    JsonValue::Object(vec![
        (
            "run".to_owned(),
            match &snapshot.run {
                Some(run) => JsonValue::Str(run.to_string()),
                None => JsonValue::Null,
            },
        ),
        (
            "escalated".to_owned(),
            JsonValue::Array(
                snapshot
                    .escalated
                    .iter()
                    .map(|ticket| JsonValue::Str(ticket.to_string()))
                    .collect(),
            ),
        ),
        (
            "in_flight".to_owned(),
            JsonValue::Array(
                snapshot
                    .in_flight
                    .iter()
                    .map(|invocation| JsonValue::Str(invocation.to_string()))
                    .collect(),
            ),
        ),
        ("units".to_owned(), units_json(&snapshot.units)),
        ("budget".to_owned(), budget_json(&snapshot.budget)),
        (
            "sandbox_denials".to_owned(),
            JsonValue::Array(
                snapshot
                    .sandbox_denials
                    .iter()
                    .map(|denial| JsonValue::Str(denial.clone()))
                    .collect(),
            ),
        ),
        (
            "open_residuals".to_owned(),
            JsonValue::Array(
                snapshot
                    .open_residuals
                    .iter()
                    .map(|residual| JsonValue::Str(residual.to_string()))
                    .collect(),
            ),
        ),
        (
            "run_derivation".to_owned(),
            JsonValue::Str(run_derivation.to_owned()),
        ),
    ])
}

/// The units array: completion grouped per unit; `tier` is the frozen
/// ruler's fact, `status` the durable projection's.
pub fn units_json(units: &[UnitSnapshot]) -> JsonValue {
    JsonValue::Array(units.iter().map(unit_json).collect())
}

/// One unit's projection object.
pub fn unit_json(unit: &UnitSnapshot) -> JsonValue {
    JsonValue::Object(vec![
        ("id".to_owned(), JsonValue::Str(unit.id.to_string())),
        ("status".to_owned(), JsonValue::Str(unit.status.to_string())),
        ("tier".to_owned(), JsonValue::Str(unit.tier.to_string())),
    ])
}

/// The budget object: remaining headroom per dimension (§20.2 "budget
/// burn"). Cost carries its µUSD integer as-is (no floats on the wire).
pub fn budget_json(budget: &Remaining) -> JsonValue {
    JsonValue::Object(vec![
        (
            "rounds".to_owned(),
            JsonValue::Number(gauntlet_ports::json::JsonNumber::Uint(budget.rounds as u64)),
        ),
        (
            "parallel".to_owned(),
            JsonValue::Number(gauntlet_ports::json::JsonNumber::Uint(
                budget.parallel as u64,
            )),
        ),
        (
            "depth".to_owned(),
            JsonValue::Number(gauntlet_ports::json::JsonNumber::Uint(budget.depth as u64)),
        ),
        (
            "wall_seconds".to_owned(),
            JsonValue::Number(gauntlet_ports::json::JsonNumber::Uint(budget.wall_seconds)),
        ),
        (
            "tokens".to_owned(),
            JsonValue::Number(gauntlet_ports::json::JsonNumber::Uint(budget.tokens)),
        ),
        (
            "cost_usd_micro".to_owned(),
            JsonValue::Number(gauntlet_ports::json::JsonNumber::Uint(
                budget.cost_usd.get(),
            )),
        ),
    ])
}

/// The `{"error":{"code","message"}}` body (§20.2): every refusal and
/// failure this surface serves, from the 400s through the typed action
/// refusals.
pub fn error_json(code: &str, message: &str) -> JsonValue {
    JsonValue::Object(vec![(
        "error".to_owned(),
        JsonValue::Object(vec![
            ("code".to_owned(), JsonValue::Str(code.to_owned())),
            ("message".to_owned(), JsonValue::Str(message.to_owned())),
        ]),
    )])
}
