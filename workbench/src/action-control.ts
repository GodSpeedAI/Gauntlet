/**
 * ActionControl — the ICM §30 control presentation object (GUI redesign
 * increment 5; `.agents/specs/tui/INTERACTION_COMMAND_MAP.md` §30,
 * `UI_ELEMENT_STATE_CATALOG.md` §22.9).
 *
 * One control renders exactly one of:
 *
 *   HIDDEN              absent from the Work ACTIONS bar, still addressable
 *                       through the command palette with its reason (DSN-009)
 *   VISIBLE_DISABLED    present with machine reasons and a valid next step
 *   VISIBLE_ENABLED     present and invocable; invocation-time revalidation
 *                       still decides (REQ-AFF-003)
 *   BUSY_SUBMISSION     the host is awaiting admission/result for one invoke
 *                       in flight; NEVER a domain-success claim
 *
 * Core invariants:
 *   - `actionControlFor` is PURE and TOTAL over the resolver's status
 *     vocabulary × the DSN-009 materialization outcome: every pair maps to
 *     exactly one non-BUSY state, so no projection can invent a fifth
 *     presentation state.
 *   - BUSY_SUBMISSION is submission lifecycle, not affordance status. It is
 *     NEVER derived from a descriptor; it exists only through the explicit
 *     `busySubmission` constructor for the host to apply while an invoke is
 *     in flight (§30: the UI awaits admission/result appropriate to the
 *     operation; it must not claim domain success).
 *   - Enablement vocabulary comes from the canonical resolver alone
 *     (`statusPermitsInvocation`, DSN-005): this module adds no second
 *     enablement predicate; it only restates resolver truth as control state.
 *   - Reason codes pass through verbatim; next steps use the ONE reason
 *     grammar (`reasonNextStep`, moved here from inspector.ts so the control
 *     object and its next-step vocabulary have a single source — inspector.ts
 *     re-exports it, preserving its public surface and the package barrel).
 *   - The DSN-009 WHERE policy (which resolved descriptors may occupy a bar
 *     row at all) lives here as `materializationOutcomeFor` so the Work bar,
 *     the Inspector records, and the palette derive identical outcomes
 *     (REQ-AFF-002 convergence). views-work.ts re-exports the outcome type.
 */

import type { AffordanceDescriptor } from "./affordance";
import { statusPermitsInvocation } from "./affordance";

export type ActionControlState =
  | "HIDDEN"
  | "VISIBLE_DISABLED"
  | "VISIBLE_ENABLED"
  | "BUSY_SUBMISSION";

/**
 * One control's §30 presentation state. `reasons` are the resolver's machine
 * reason codes verbatim; `nextStep` (grammar text) and `attentionItem` (the
 * escalation this control responds to) attach only when the facts exist —
 * absent means absent, never present-undefined. Immutable (frozen).
 */
export interface ActionControl {
  readonly operation: string;
  readonly state: ActionControlState;
  readonly reasons: readonly string[];
  readonly nextStep?: string;
  readonly attentionItem?: string;
}

/**
 * Map one canonical refusal/reason code to a valid operator next step
 * (DSN-009). The mapping restates the canonical gate vocabulary of T09's
 * resolver in operator language; it is presentation text, never authority.
 * (Moved from inspector.ts with that module's behavior unchanged.)
 */
export function reasonNextStep(reason: string): string {
  switch (reason) {
    case "target_not_visible":
      return "select a unit the current reads name";
    case "surface_unreachable":
      return "restore the canonical surface connection";
    case "actor_authentication_required":
      return "authenticate an operator session";
    case "authority_not_permitted":
      return "obtain authority for this target";
    case "blocking_residual_open":
      return "resolve the open residual denial first";
    case "budget_exhausted":
      return "increase remaining budget or choose a cheaper action";
    case "no_open_ticket":
      return "requires an open escalation ticket for this unit";
    case "readiness_not_earned":
      return "retry requires a failed verification on this unit";
    case "observation_degraded":
      return "refresh reads before acting on degraded data";
    case "unsupported_operation":
      return "served by no canonical operation";
    case "not_action_admitted":
      return "read-shaped; served by the canonical read path";
    default:
      return "inspect the blocking gate in the canonical record";
  }
}

/** Where a resolved descriptor materialises (DSN-009, DESIGN.md "Affordance state"). */
export type MaterializationOutcome = "materialized" | "blocked_with_reason" | "palette_only";

/**
 * The palette-only reasons: a resolution carrying ANY of these never occupies
 * an ACTIONS-bar row; it stays discoverable in the command palette with its
 * reason instead (DSN-009).
 */
export const PALETTE_ONLY_REASONS: readonly string[] = Object.freeze([
  "target_not_visible",
  "unsupported_operation",
  "not_action_admitted",
]);

/**
 * Progressive Affordance Materialization policy (DSN-009): the RESOLVER
 * decides existence and enablement; this pure classifier only decides WHERE a
 * resolved descriptor materialises:
 *   - available / requires_attention / degraded → materialized;
 *   - unavailable on a visible target (a real blocking gate) → blocked with
 *     reason;
 *   - unavailable because the target is not visible or the operation is not
 *     an action → palette-only with its reason.
 */
export function materializationOutcomeFor(
  descriptor: AffordanceDescriptor,
): MaterializationOutcome {
  if (descriptor.status !== "unavailable") {
    return "materialized";
  }
  const paletteOnly = descriptor.reason_codes.some((reason) =>
    PALETTE_ONLY_REASONS.includes(reason),
  );
  return paletteOnly ? "palette_only" : "blocked_with_reason";
}

/** Build one frozen control; optional facts attach only when defined. */
function freezeControl(init: {
  operation: string;
  state: ActionControlState;
  reasons: readonly string[];
  nextStep: string | undefined;
  attentionItem: string | undefined;
}): ActionControl {
  return Object.freeze({
    operation: init.operation,
    state: init.state,
    reasons: Object.freeze([...init.reasons]),
    ...(init.nextStep !== undefined ? { nextStep: init.nextStep } : {}),
    ...(init.attentionItem !== undefined ? { attentionItem: init.attentionItem } : {}),
  });
}

/** The grammar's next step for a reason list; undefined when nothing is refused. */
function nextStepFromReasons(reasons: readonly string[]): string | undefined {
  return reasons.length > 0 ? reasonNextStep(reasons[0] ?? "") : undefined;
}

/**
 * Derive one control's §30 state from a resolved descriptor and its DSN-009
 * materialization outcome. Pure, total, and never BUSY:
 *
 *   palette_only                     → HIDDEN (with reasons; still palette-
 *                                      addressable — precedence over status,
 *                                      so no palette-only resolution can leak
 *                                      an enabled-looking bar row)
 *   statusPermitsInvocation(status)  → VISIBLE_ENABLED (attentionItem rides
 *                                      along when the resolver named one)
 *   otherwise (degraded/unavailable) → VISIBLE_DISABLED (reasons + nextStep)
 *
 * BUSY_SUBMISSION is deliberately unreachable here: it is submission
 * lifecycle owned by the host invoke path, not affordance status. Use
 * `busySubmission(operation)` while an invoke is in flight.
 */
export function actionControlFor(
  descriptor: AffordanceDescriptor,
  outcome: MaterializationOutcome,
): ActionControl {
  if (outcome === "palette_only") {
    return freezeControl({
      operation: descriptor.operation,
      state: "HIDDEN",
      reasons: descriptor.reason_codes,
      nextStep: nextStepFromReasons(descriptor.reason_codes),
      attentionItem: descriptor.required_attention_item,
    });
  }
  if (statusPermitsInvocation(descriptor.status)) {
    return freezeControl({
      operation: descriptor.operation,
      state: "VISIBLE_ENABLED",
      reasons: descriptor.reason_codes,
      nextStep: nextStepFromReasons(descriptor.reason_codes),
      attentionItem: descriptor.required_attention_item,
    });
  }
  return freezeControl({
    operation: descriptor.operation,
    state: "VISIBLE_DISABLED",
    reasons: descriptor.reason_codes,
    nextStep: nextStepFromReasons(descriptor.reason_codes),
    attentionItem: descriptor.required_attention_item,
  });
}

/**
 * The §30 BUSY_SUBMISSION control: the host mints this for one operation
 * while its invoke is in flight, and drops it when the authoritative
 * projection (or a denial with its exact reason) arrives. It carries no
 * reasons and no next step — awaiting admission/result is not a refusal —
 * and it MUST NOT be read as a domain-success claim.
 */
export function busySubmission(operation: string): ActionControl {
  return freezeControl({
    operation,
    state: "BUSY_SUBMISSION",
    reasons: [],
    nextStep: undefined,
    attentionItem: undefined,
  });
}
