/**
 * Work view: "What are we trying to move?" (plan T12, REQ-UX-001/003,
 * REQ-TUI-003, DSN-005, DSN-009, DESIGN.md Work view + Affordance state).
 *
 * Core invariants:
 *   - Escalation-first ordering matches OperatorSurface priorities
 *     (REQ-UX-001): escalation/attention, in-flight work, then remaining
 *     units by tier.
 *   - Affordance existence and enablement come from the T09 canonical
 *     resolver (resolveAffordance), NEVER from a component-local predicate
 *     over domain status (DSN-005). This module contains no status-string
 *     enablement branch; `statusToken` maps state to a presentation token
 *     for DISPLAY only.
 *   - Blocked-but-informative affordances materialise with their blocking
 *     gate and a next step; not-currently-legitimate ones stay absent from
 *     the view but discoverable in the palette with their reason (DSN-009).
 */

import type {
  AffordanceDescriptor,
  AffordanceObservation,
  AffordanceSessionContext,
  PaymentEstimate,
} from "./affordance";
import { resolveAffordance } from "./affordance";
import type { FocusRef } from "./focus";
import { reasonNextStep } from "./inspector";
import { operationIds } from "./operation-registry";
import type { UnitProjection } from "./projection-store";
import type { RenderedLine, RenderedRecord, ViewContent } from "./render-tree";
import { SEMANTIC_TOKENS } from "./theme";
import type { FullWorkbenchSnapshot } from "./workbench-model";

/** Presentation-only mapping from a projected status word to a semantic token. */
export function statusToken(status: string): SemanticTokenForStatus {
  switch (status) {
    case "blocked":
      return "readiness.blocked";
    case "running":
    case "active":
      return "execution.running";
    case "pass":
    case "settled":
      return "settlement.accepted";
    case "fail":
      return "evidence.failed";
    default:
      return "readiness.degraded";
  }
}

export type SemanticTokenForStatus =
  | "readiness.blocked"
  | "execution.running"
  | "settlement.accepted"
  | "evidence.failed"
  | "readiness.degraded";

/** Tier ordering: the WorkItem projection carries no tier field (honest gap). */
/**
 * Escalation-first Work ordering (REQ-UX-001): blocked units (escalation
 * attention) first, then in-flight (running/active), then the rest. The
 * projection carries no tier or budget field, so completion-by-tier and
 * remaining budget render as explicitly unavailable rather than invented.
 */
export function workUnitOrder(units: readonly UnitProjection[]): readonly UnitProjection[] {
  const classOf = (unit: UnitProjection): number =>
    unit.status === "blocked" ? 0 : unit.status === "running" || unit.status === "active" ? 1 : 2;
  return [...units].sort((a, b) => {
    const classDelta = classOf(a) - classOf(b);
    if (classDelta !== 0) {
      return classDelta;
    }
    return a.unit_id.localeCompare(b.unit_id);
  });
}

export type MaterializationOutcome = "materialized" | "blocked_with_reason" | "palette_only";

export interface MaterializedAffordance {
  readonly outcome: MaterializationOutcome;
  readonly descriptor: AffordanceDescriptor;
  /** Valid next step for blocked affordances; null when none is needed. */
  readonly nextStep: string | null;
}

const PALETTE_ONLY_REASONS: readonly string[] = Object.freeze([
  "target_not_visible",
  "unsupported_operation",
  "not_action_admitted",
]);

/**
 * Progressive Affordance Materialization policy (DSN-009, DESIGN.md
 * "Affordance state"): the RESOLVER decides existence and enablement; this
 * policy only decides WHERE a resolved descriptor materialises.
 *   - available / requires_attention / degraded → materialized;
 *   - unavailable on a visible target (a real blocking gate) → blocked with
 *     reason and a next step;
 *   - unavailable because the target is not visible or the operation is not
 *     an action → palette-only with its reason.
 */
export function materializeAffordances(
  focus: FocusRef,
  observation: AffordanceObservation,
  options: {
    session?: AffordanceSessionContext | undefined;
    payment_estimates?: Readonly<Record<string, PaymentEstimate>> | undefined;
  } = {},
): readonly MaterializedAffordance[] {
  return operationIds().map((operation) => {
    const descriptor = resolveAffordance(operation, focus, observation, options);
    if (descriptor.status !== "unavailable") {
      return {
        outcome: "materialized" as const,
        descriptor,
        nextStep:
          descriptor.reason_codes.length > 0
            ? reasonNextStep(descriptor.reason_codes[0] ?? "")
            : null,
      };
    }
    const paletteOnly = descriptor.reason_codes.some((reason) =>
      PALETTE_ONLY_REASONS.includes(reason),
    );
    if (paletteOnly) {
      return {
        outcome: "palette_only" as const,
        descriptor,
        nextStep: reasonNextStep(descriptor.reason_codes[0] ?? ""),
      };
    }
    return {
      outcome: "blocked_with_reason" as const,
      descriptor,
      nextStep: reasonNextStep(descriptor.reason_codes[0] ?? ""),
    };
  });
}

export interface ViewRenderArgs {
  readonly snapshot: FullWorkbenchSnapshot;
  readonly observation: AffordanceObservation;
  readonly session?: AffordanceSessionContext | undefined;
  readonly paymentEstimates?: Readonly<Record<string, PaymentEstimate>> | undefined;
}

function statusGlyphLine(unit: UnitProjection): RenderedLine {
  const token = statusToken(unit.status);
  const glyph = SEMANTIC_TOKENS[token].glyph;
  return {
    id: `work.unit.${unit.unit_id}`,
    spans: [
      { text: `${glyph} `, token },
      { text: `${unit.status.toUpperCase()} `, token, emphasis: "bold" },
      { text: `${unit.unit_id} `, emphasis: "normal" },
      { text: unit.title || "unavailable" },
      { text: `  authority ${unit.authority}`, emphasis: "dim" },
    ],
  };
}

/** Render the Work primary projection for one snapshot. */
export function renderWorkView(args: ViewRenderArgs): ViewContent {
  const { snapshot, observation } = args;
  const lines: RenderedLine[] = [];
  const records: RenderedRecord[] = [];

  lines.push({
    id: "work.question",
    spans: [{ text: "WORK — what are we trying to move?", emphasis: "bold" }],
  });

  // Honest aggregate header: counts + what the projection does NOT carry.
  const blocked = snapshot.units.filter((unit) => unit.status === "blocked").length;
  const inFlight = snapshot.units.filter(
    (unit) => unit.status === "running" || unit.status === "active",
  ).length;
  lines.push({
    id: "work.summary",
    spans: [
      { text: `${snapshot.units.length} units · `, emphasis: "dim" },
      { text: `! ${blocked} blocked `, token: "attention.required" },
      { text: `${inFlight} in flight · `, emphasis: "dim" },
      {
        text: "budget: unavailable (host budget read not bound to this projection)",
        emphasis: "dim",
      },
    ],
  });

  // Environment honesty (REQ-UX-006): capabilities the host observed as
  // unavailable stay listed with their gate reasons — never faked available.
  for (const capability of snapshot.capability_reasons) {
    if (capability.status !== "unavailable") {
      continue;
    }
    lines.push({
      id: `work.capability.${capability.capability}`,
      spans: [
        { text: "○ ", token: "capability.attempted" },
        { text: `${capability.capability} `, emphasis: "bold" },
        {
          text: `unavailable (${capability.reason_codes.join(", ") || "gates not passed"})`,
          token: "capability.attempted",
          emphasis: "dim",
        },
      ],
    });
  }

  if (snapshot.units.length === 0) {
    // Honest empty state with a named next step (REQ-UX-001/006).
    const anySource = Object.keys(snapshot.source_cursors).length > 0;
    lines.push({
      id: "work.empty",
      spans: [
        {
          text: anySource
            ? "No units reported yet. Wait for the authoritative source to admit units, or run a Gauntlet run."
            : "No authoritative source has reported. Start 'gauntlet serve' and bind a source, then sync a generation.",
          token: "readiness.degraded",
        },
      ],
    });
    return { view: "work", question: "What are we trying to move?", lines, records };
  }

  // Escalation attention first (REQ-UX-001), independent of the Horizon.
  for (const entry of snapshot.attention) {
    lines.push({
      id: `work.attention.${entry.focus.id}`,
      spans: [
        { text: "! ", token: "attention.required", emphasis: "bold" },
        { text: `${entry.reason.toUpperCase()} `, token: "attention.required" },
        { text: `${entry.focus.kind}:${entry.focus.source_id} `, emphasis: "bold" },
        { text: "— open the escalation to resolve", emphasis: "dim" },
      ],
    });
    records.push({
      id: `attention:${entry.focus.id}`,
      kind: "attention",
      token: "attention.required",
      fields: {
        reason: entry.reason,
        severity: entry.severity,
        target: entry.focus.id,
        horizon_independent: String(entry.horizon_independent),
      },
    });
  }

  // Unit rows, escalation-first.
  for (const unit of workUnitOrder(snapshot.units)) {
    lines.push(statusGlyphLine(unit));
  }

  // Affordance bar for the current Focus — resolver-driven (DSN-005).
  const focus: FocusRef | null = snapshot.focus;
  if (focus !== null) {
    const materialized = materializeAffordances(focus, observation, {
      session: args.session,
      payment_estimates: args.paymentEstimates,
    });
    lines.push({
      id: "work.affordances.header",
      spans: [
        { text: "ACTIONS ", emphasis: "bold" },
        { text: `for ${focus.id}`, emphasis: "dim" },
      ],
    });
    for (const item of materialized) {
      if (item.outcome === "palette_only") {
        continue;
      }
      const available = item.descriptor.status === "available";
      const attention = item.descriptor.status === "requires_attention";
      lines.push({
        id: `work.affordance.${item.descriptor.operation}`,
        spans: [
          {
            text: available ? "✓ " : attention ? "! " : "× ",
            token: available
              ? "readiness.ready"
              : attention
                ? "attention.required"
                : "readiness.blocked",
          },
          { text: `${item.descriptor.operation.toUpperCase()} `, emphasis: "bold" },
          {
            text: attention
              ? `responds to ${item.descriptor.required_attention_item ?? "an attention item"}`
              : available
                ? "ready"
                : `BLOCKED — ${item.descriptor.reason_codes.join(", ")}`,
            token: available ? undefined : attention ? "attention.required" : "readiness.blocked",
          },
          ...(item.nextStep !== null && !available
            ? [{ text: ` · next: ${item.nextStep}`, emphasis: "dim" as const }]
            : []),
        ],
      });
      records.push({
        id: `affordance:${item.descriptor.operation}:${focus.id}`,
        kind: "affordance",
        token: available
          ? "readiness.ready"
          : attention
            ? "attention.required"
            : "readiness.blocked",
        fields: {
          operation: item.descriptor.operation,
          status: item.descriptor.status,
          reasons: item.descriptor.reason_codes.join(",") || "none",
          next_step: item.nextStep ?? "none",
          materialization: item.outcome,
          target: focus.id,
        },
      });
    }
    const paletteOnly = materialized.filter((item) => item.outcome === "palette_only");
    if (paletteOnly.length > 0) {
      // Absent from the view's action bar but discoverable WITH reasons (DSN-009).
      lines.push({
        id: "work.affordances.palette",
        spans: [
          { text: `${paletteOnly.length} more in command palette `, emphasis: "dim" },
          {
            text: `(${paletteOnly
              .map(
                (item) => `${item.descriptor.operation}: ${item.descriptor.reason_codes.join("/")}`,
              )
              .join("; ")})`,
            emphasis: "dim",
          },
        ],
      });
    }
  } else {
    lines.push({
      id: "work.affordances.none",
      spans: [{ text: "No Focus selected — choose a unit in the Navigator.", emphasis: "dim" }],
    });
  }

  return { view: "work", question: "What are we trying to move?", lines, records };
}
