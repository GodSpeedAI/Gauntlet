/**
 * The canonical operation registry (plan T09, REQ-AFF-001/002, REQ-OPS-003).
 *
 * This module is the ONE place in the workbench that knows which canonical
 * Gauntlet operations exist and how an operation reaches canonical admission.
 * Every projection (TUI controls, command palette, kernel wrappers, internal
 * adapter shims) references registry IDs only; no projection invents operation
 * semantics or implements a presentation-specific side effect (REQ-AFF-001).
 *
 * The catalog is DERIVED from the canonical Rust surface — never invented:
 *   - `crates/gauntlet-cli/src/durable_surface.rs` `DurableSurface::actions()`
 *     offers exactly `OperatorAction::Refresh`, `OperatorAction::ResolveUnit`,
 *     and `OperatorAction::RetryUnit`.
 *   - `crates/adapters/gauntlet-adapter-surface-json/src/action.rs`
 *     `ActionKind` serves the wire tokens `resolve` / `retry`, and the HTTP
 *     capability handshake lists `action_resolve` / `action_retry` /
 *     `state_refresh`.
 * Operations the canonical surface does not serve (launch, steering,
 * cancellation, evidence admission, session operations) are ABSENT here —
 * REQ-OPS-003: missing operations render unsupported, never a fallback.
 */

/** Canonical operation identities, matching the Rust surface's own vocabulary. */
export const OPERATION_IDS = {
  UNIT_RESOLVE: "unit.resolve",
  UNIT_RETRY: "unit.retry",
  STATE_REFRESH: "state.refresh",
} as const;

export type OperationId = (typeof OPERATION_IDS)[keyof typeof OPERATION_IDS];

/** The action kind the canonical admission path serves (ActionKind tokens / OperatorAction variants). */
export type CanonicalActionKind = "resolve" | "retry" | "refresh";

/**
 * One canonical operation's registry entry.
 * `consequential: false` marks read-shaped operations (state.refresh): they
 * carry no authority or idempotency and are served by the canonical read
 * path, not the action admission path.
 */
export interface OperationSpec {
  readonly id: OperationId;
  readonly action_kind: CanonicalActionKind;
  /** The target vocabulary the canonical command carries (ActionCommand: unit, optional run). */
  readonly target_kind: "Unit" | "Case";
  /** Whether the operation is unit-scoped (carries a unit target). */
  readonly unit_scoped: boolean;
  /** Whether the operation can change consequential state and therefore admits through the action path. */
  readonly consequential: boolean;
  /** Whether canonical admission is the §16.3 idempotent re-entry shape. */
  readonly idempotent_reentry: boolean;
  /** The capability-handshake token the canonical surface lists for this operation. */
  readonly surface_capability: string;
}

/**
 * The frozen catalog. Entries and their fields mirror the canonical Rust
 * surface; the registry never adds a fourth operation or rewords a kind.
 */
export const OPERATION_REGISTRY: Readonly<Record<OperationId, OperationSpec>> = Object.freeze({
  [OPERATION_IDS.UNIT_RESOLVE]: Object.freeze({
    id: OPERATION_IDS.UNIT_RESOLVE,
    action_kind: "resolve",
    target_kind: "Unit",
    unit_scoped: true,
    consequential: true,
    idempotent_reentry: true,
    surface_capability: "action_resolve",
  }),
  [OPERATION_IDS.UNIT_RETRY]: Object.freeze({
    id: OPERATION_IDS.UNIT_RETRY,
    action_kind: "retry",
    target_kind: "Unit",
    unit_scoped: true,
    consequential: true,
    idempotent_reentry: true,
    surface_capability: "action_retry",
  }),
  [OPERATION_IDS.STATE_REFRESH]: Object.freeze({
    id: OPERATION_IDS.STATE_REFRESH,
    action_kind: "refresh",
    target_kind: "Case",
    unit_scoped: false,
    consequential: false,
    idempotent_reentry: false,
    surface_capability: "state_refresh",
  }),
} as Record<OperationId, OperationSpec>);

/** All registered operation IDs in deterministic (catalog) order. */
export function operationIds(): readonly OperationId[] {
  return Object.values(OPERATION_IDS);
}

/** Type guard: whether a raw string is a registered canonical operation identity. */
export function isOperationId(value: string): value is OperationId {
  return Object.hasOwn(OPERATION_REGISTRY, value);
}

/** Registry lookup; null when the value is not a registered operation (render unsupported). */
export function lookupOperation(value: string): OperationSpec | null {
  return isOperationId(value) ? OPERATION_REGISTRY[value] : null;
}
