/**
 * AffordanceDescriptor resolution (plan T09, REQ-AFF-001..005, REQ-CONFIG-011,
 * DSN-005/009 direction).
 *
 * One descriptor is one currently evaluated path from a Focus to a canonical
 * operation, including whether it is available and why. Resolution is a PURE
 * function of (operation, focus, current observation, context): it predicts,
 * it never decides — invocation always revalidates (REQ-AFF-003, REQ-SAFE-002).
 *
 * Core invariants:
 *   - Installed capability is INVENTORY ONLY and never implies status
 *     `available` (REQ-AFF-004): capability observations never enter the gate
 *     evaluation as evidence of a passing current path.
 *   - Unavailable operations stay inspectable: the descriptor is returned with
 *     machine-readable reason codes rather than thrown away (REQ-AFF-005,
 *     DSN-009).
 *   - Reason codes preserve the canonical refusal vocabulary wherever the
 *     canonical surface names one (`authority_not_permitted`,
 *     `blocking_residual_open`, `no_open_ticket`, `readiness_not_earned`,
 *     `budget_exhausted` — the `refusal_code` vocabulary of
 *     crates/gauntlet-cli/src/operator_action.rs and the surface's own codes),
 *     so a denial means the same thing in every projection.
 *   - The resolution is deterministic for identical inputs (wall-clock-free).
 */

import type { FocusRef } from "./focus";
import { isOperationId, lookupOperation } from "./operation-registry";
import type { CapabilityObservation } from "./workbench-model";

export type AffordanceStatus = "available" | "unavailable" | "requires_attention" | "degraded";

/** The payment dimensions an operation is estimated to cost. */
export interface PaymentEstimate {
  readonly tokens: number;
  readonly cost_usd_micro: number;
}

/**
 * The spec-typed AffordanceDescriptor (spec §core_domain_model).
 * `status`/`reason_codes` are the current resolution result; `presentation_hint`,
 * `estimated_payment`, and `required_attention_item` are optional presentation
 * and journey facts. Immutable (frozen) once resolved.
 */
export interface AffordanceDescriptor {
  /** Stable operation-affordance identity for the current context. */
  readonly id: string;
  /** Canonical Gauntlet operation identity (registry vocabulary). */
  readonly operation: string;
  /** Object to which this affordance applies. */
  readonly focus: FocusRef;
  readonly status: AffordanceStatus;
  /** Machine-readable visibility/reachability/payment/governance/settlement/recovery reasons. */
  readonly reason_codes: readonly string[];
  readonly presentation_hint?: string;
  readonly estimated_payment?: PaymentEstimate;
  /** The attention item (e.g. escalation ticket) this affordance responds to. */
  readonly required_attention_item?: string;
}

/** One canonical unit fact the observation carries (the surface's own units read). */
export interface AffordanceUnitObservation {
  readonly unit_id: string;
  readonly status: string;
}

/** One open escalation the observation carries. */
export interface AffordanceEscalationObservation {
  readonly ticket_id: string;
  /** Binding unit; null when the read names the ticket without a unit binding. */
  readonly unit_id: string | null;
}

/** The budget facts the surface serves (RuntimeSnapshot.budget_remaining). */
export interface AffordanceBudgetObservation {
  readonly usd_micro: number;
  readonly tokens: number;
}

/**
 * The current canonical observation an affordance resolves against. Every
 * field is an honest read: absence means the read does not name it (an empty
 * `authority_revoked_units` is honest absence of a revocation READ, never
 * evidence that authority is granted — admission re-checks regardless).
 */
export interface AffordanceObservation {
  /** Whether the canonical surface was reachable for this read. */
  readonly reachable: boolean;
  /** Whether the read itself reported degraded/partial state. */
  readonly degraded: boolean;
  readonly units: readonly AffordanceUnitObservation[];
  readonly open_escalations: readonly AffordanceEscalationObservation[];
  readonly budget_remaining: AffordanceBudgetObservation | null;
  /** Units the current reads name under an open governance/residual denial. */
  readonly governance_denied_units: readonly string[];
  /** Units the current reads name with revoked authority. */
  readonly authority_revoked_units: readonly string[];
  /**
   * Installed-capability INVENTORY (REQ-AFF-004). Never consulted as
   * availability evidence; carried so resolution stays honest about what the
   * environment observed.
   */
  readonly capabilities: readonly CapabilityObservation[];
}

/** The caller-side session context (T01's config/auth binding), not a canonical read. */
export interface AffordanceSessionContext {
  readonly actor: string;
  readonly authenticated: boolean;
}

/** The empty observation: nothing reachable, nothing named (honest absence). */
export const EMPTY_AFFORDANCE_OBSERVATION: AffordanceObservation = Object.freeze({
  reachable: false,
  degraded: true,
  units: Object.freeze([]),
  open_escalations: Object.freeze([]),
  budget_remaining: null,
  governance_denied_units: Object.freeze([]),
  authority_revoked_units: Object.freeze([]),
  capabilities: Object.freeze([]),
});

export interface ResolveAffordanceOptions {
  /** Session context for settlement-access evaluation (null = no session presented). */
  readonly session?: AffordanceSessionContext | undefined;
  /** Per-operation payment estimates supplied by the caller (registry-neutral). */
  readonly payment_estimates?: Readonly<Record<string, PaymentEstimate>> | undefined;
  /** Presentation hint carried through to projections (DSN-005 direction). */
  readonly presentation_hint?: string | undefined;
}

function descriptorId(operation: string, focus: FocusRef): string {
  return `affordance:${operation}:${focus.id}`;
}

/**
 * Build one frozen descriptor. Optional fields are attached ONLY when defined
 * (exactOptionalPropertyTypes: an absent fact is absent, never present-undefined).
 */
function freezeDescriptor(init: {
  operation: string;
  focus: FocusRef;
  status: AffordanceStatus;
  reasonCodes: readonly string[];
  presentationHint: string | undefined;
  estimatedPayment: PaymentEstimate | undefined;
  requiredAttentionItem: string | undefined;
}): AffordanceDescriptor {
  return Object.freeze({
    id: descriptorId(init.operation, init.focus),
    operation: init.operation,
    focus: init.focus,
    status: init.status,
    reason_codes: Object.freeze([...init.reasonCodes]),
    ...(init.presentationHint !== undefined ? { presentation_hint: init.presentationHint } : {}),
    ...(init.estimatedPayment !== undefined ? { estimated_payment: init.estimatedPayment } : {}),
    ...(init.requiredAttentionItem !== undefined
      ? { required_attention_item: init.requiredAttentionItem }
      : {}),
  });
}

/**
 * Resolve one affordance. Pure and deterministic. Returns a frozen descriptor
 * for EVERY input — unknown operations and failed gates resolve to
 * `unavailable` descriptors that stay inspectable (REQ-AFF-005), never thrown.
 */
export function resolveAffordance(
  operation: string,
  focus: FocusRef,
  observation: AffordanceObservation,
  options: ResolveAffordanceOptions = {},
): AffordanceDescriptor {
  const reasons: string[] = [];

  // Registry gate: an operation the canonical surface does not serve renders
  // unsupported — never a fallback (REQ-OPS-003, REQ-AFF-001).
  const spec = isOperationId(operation) ? lookupOperation(operation) : null;
  if (spec === null) {
    return freezeDescriptor({
      operation,
      focus,
      status: "unavailable",
      reasonCodes: ["unsupported_operation"],
      presentationHint: options.presentation_hint,
      estimatedPayment: undefined,
      requiredAttentionItem: undefined,
    });
  }

  const unitScoped = spec.unit_scoped;

  // A registered but READ-SHAPED operation (state.refresh) is never an action
  // affordance: it resolves unavailable with the typed `not_action_admitted`
  // reason and is served by the canonical read path, never the action
  // admission path (Correction Round 1 MINOR 2 — never toward `available`).
  if (!spec.consequential) {
    return freezeDescriptor({
      operation,
      focus,
      status: "unavailable",
      reasonCodes: ["not_action_admitted"],
      presentationHint: options.presentation_hint,
      estimatedPayment: undefined,
      requiredAttentionItem: undefined,
    });
  }

  // Visibility: a unit-scoped operation applies to a unit the current reads
  // name; an unknown target is not visible.
  if (unitScoped) {
    const visible = observation.units.some((unit) => unit.unit_id === focus.source_id);
    if (!visible) {
      reasons.push("target_not_visible");
    }
  }

  // Reachability: the canonical surface must be reachable for invocation-time
  // revalidation to even be possible (REQ-SAFE-002 direction).
  if (!observation.reachable) {
    reasons.push("surface_unreachable");
  }

  // Settlement access: consequential operations need an authenticated session
  // and authority the current reads do not name as revoked. Read-shaped
  // operations (refresh) carry no settlement access requirement.
  if (unitScoped) {
    const session = options.session;
    if (session === undefined || !session.authenticated || session.actor.length === 0) {
      reasons.push("actor_authentication_required");
    }
    if (observation.authority_revoked_units.includes(focus.source_id)) {
      reasons.push("authority_not_permitted");
    }
    // Governance: an open governance/residual denial naming the target.
    if (observation.governance_denied_units.includes(focus.source_id)) {
      reasons.push("blocking_residual_open");
    }
    // Payment: an estimate exceeding the observed remaining budget is
    // unaffordable. No estimate, or no observable budget constraint, is honest
    // absence — never invented.
    const estimate = options.payment_estimates?.[operation];
    const budget = observation.budget_remaining;
    if (estimate !== undefined && budget !== null) {
      if (estimate.tokens > budget.tokens || estimate.cost_usd_micro > budget.usd_micro) {
        reasons.push("budget_exhausted");
      }
    }
    // Operation-specific canonical preconditions, observable from reads:
    //   resolve addresses an OPEN escalation on the target;
    //   retry needs a failed (not earned/passing) unit (readiness).
    //
    //   DISCLOSED RESOLVER POLICY (Correction Round 1 MINOR 3): the
    //   no_open_ticket prediction is deliberately STRICTER than the anchored
    //   admission shape. The real admission refuses no_open_ticket only for
    //   the UNANCHORED action (crates/gauntlet-cli/src/operator_action.rs:232)
    //   and lets an EXPLICITLY-ANCHORED action proceed with zero open tickets
    //   (:246-249, "the settle is the action's effect; the (empty, disclosed)
    //   ticket list records that there was nothing to mark"). The workbench
    //   resolve control is the UNANCHORED shape, so the resolver predicts the
    //   unanchored refusal; an operator who genuinely wants the anchored
    //   set-with-nothing-to-mark uses the CLI/API shape directly. Fail-closed
    //   by design; pinned by test so the strictness cannot silently drift.
    if (operation === "unit.resolve") {
      const hasOpenTicket = observation.open_escalations.some(
        (escalation) => escalation.unit_id === focus.source_id || escalation.unit_id === null,
      );
      if (!hasOpenTicket) {
        reasons.push("no_open_ticket");
      }
    } else if (operation === "unit.retry") {
      const target = observation.units.find((unit) => unit.unit_id === focus.source_id);
      if (target !== undefined && target.status !== "fail") {
        reasons.push("readiness_not_earned");
      }
    }
  }

  const hardFailed = reasons.length > 0;
  // Unreachability is already a hard failure above, so `degraded` here means
  // the read itself reported degraded/partial state while otherwise passing.
  const degraded = observation.degraded;

  const estimate = options.payment_estimates?.[operation];
  let status: AffordanceStatus;
  let attentionItem: string | undefined;
  if (hardFailed) {
    status = "unavailable";
  } else if (degraded) {
    status = "degraded";
    reasons.push("observation_degraded");
  } else if (operation === "unit.resolve") {
    // The target's open escalation is the attention item this operation
    // responds to (REQ-UX-003 journey).
    const ticket = observation.open_escalations.find(
      (escalation) => escalation.unit_id === focus.source_id || escalation.unit_id === null,
    );
    if (ticket !== undefined) {
      status = "requires_attention";
      attentionItem = ticket.ticket_id;
    } else {
      status = "available";
    }
  } else {
    status = "available";
  }

  return freezeDescriptor({
    operation,
    focus,
    status,
    reasonCodes: reasons,
    presentationHint: options.presentation_hint,
    estimatedPayment: estimate,
    requiredAttentionItem: attentionItem,
  });
}

/**
 * Whether a descriptor's status permits an invocation ATTEMPT to be routed to
 * canonical admission. `available` and `requires_attention` may proceed (the
 * canonical path decides); `unavailable` and `degraded` fail closed.
 */
export function statusPermitsInvocation(status: AffordanceStatus): boolean {
  return status === "available" || status === "requires_attention";
}
