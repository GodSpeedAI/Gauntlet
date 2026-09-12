/**
 * Work view: "What are we trying to move?" (plan T12, REQ-UX-001/003,
 * REQ-TUI-003, DSN-005, DSN-009, DESIGN.md Work view + Affordance state +
 * Grid and spacing / Row density / text.machine tabular alignment).
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
 *   - The ACTIONS bar renders through the ICM §30 ActionControl presentation
 *     object (actionControlFor, increment 5): glyph/label language per state
 *     is preserved exactly; the bar chooses no state itself.
 *   - Group rhythm: one blank line before each non-empty group after the
 *     header block (DESIGN.md space.2/space.3, the Inspector's own rhythm
 *     idiom); an empty group leaves no gap. Empty groups and per-frame
 *     column widths are presentation, never state.
 */

import type { MaterializationOutcome } from "./action-control";
import {
  actionControlFor,
  busySubmission,
  materializationOutcomeFor,
  reasonNextStep,
} from "./action-control";
import type {
  AffordanceDescriptor,
  AffordanceObservation,
  AffordanceSessionContext,
  PaymentEstimate,
} from "./affordance";
import { resolveAffordance } from "./affordance";
import type { FocusRef } from "./focus";
import { cellWidth, padCellText } from "./geometry";
import { operationIds } from "./operation-registry";
import type { UnitProjection } from "./projection-store";
import type { RenderedLine, RenderedRecord, ViewContent } from "./render-tree";
import { SEMANTIC_TOKENS } from "./theme";
import type { FullWorkbenchSnapshot } from "./workbench-model";

// API stability (increment 5): the outcome vocabulary moved to action-control
// (the §30 HIDDEN state is defined by it); the re-export keeps this module's
// and the package barrel's surface unchanged.
export type { MaterializationOutcome } from "./action-control";

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

export interface MaterializedAffordance {
  readonly outcome: MaterializationOutcome;
  readonly descriptor: AffordanceDescriptor;
  /** Valid next step for blocked affordances; null when none is needed. */
  readonly nextStep: string | null;
}

/**
 * Progressive Affordance Materialization (DSN-009, DESIGN.md
 * "Affordance state"): the RESOLVER decides existence and enablement; the
 * WHERE policy (materializationOutcomeFor, action-control.ts — its single
 * source since increment 5) decides only where each resolved descriptor
 * materialises: available/requires_attention/degraded → materialized;
 * unavailable on a visible target → blocked with reason and a next step;
 * unavailable on an illegitimate target → palette-only with its reason.
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
    return {
      outcome: materializationOutcomeFor(descriptor),
      descriptor,
      nextStep:
        descriptor.reason_codes.length > 0
          ? reasonNextStep(descriptor.reason_codes[0] ?? "")
          : null,
    };
  });
}

export interface ViewRenderArgs {
  readonly snapshot: FullWorkbenchSnapshot;
  readonly observation: AffordanceObservation;
  readonly session?: AffordanceSessionContext | undefined;
  readonly paymentEstimates?: Readonly<Record<string, PaymentEstimate>> | undefined;
  /**
   * GUI redesign (UESC §16.2): when the shell drives the Trace view through
   * the TraceStream machine, it passes the derived ledger window here. Absent
   * = legacy whole-history rendering.
   */
  readonly traceWindow?: TraceLedgerWindow | undefined;
  /**
   * ICM §30 BUSY_SUBMISSION wiring: the `${operation}:${focus.id}` keys the
   * host currently has an invoke in flight for (WorkbenchHost.pendingOperations,
   * threaded through ShellRenderOptions). When the current row's key is a
   * member, the bar renders BUSY_SUBMISSION instead of the resolver-derived
   * control — never the other way around (action-control.ts's invariant:
   * BUSY_SUBMISSION is never descriptor-derived).
   */
  readonly pendingOperations?: ReadonlySet<string> | undefined;
  /**
   * RealityTrace (GUI-redesign increment 6): the host's most recently
   * projected ledger lane and the refresh epoch it was projected at, threaded
   * through ShellRenderOptions to the Trace view only. Absent = unbound or
   * never refreshed — unchanged current behavior.
   */
  readonly sxrLane?: import("./reality-trace-lane").RealityTraceLane | undefined;
  readonly sxrRefreshEpoch?: number | undefined;
}

/** The trace ledger projection the shell derives from its TraceStream. */
export interface TraceLedgerWindow {
  readonly followState: "COLD" | "HYDRATING" | "LIVE_FOLLOW" | "INSPECTION_HOLD";
  readonly unseenCount: number;
  /** Snapshot records visible in the current window, in window order. */
  readonly records: readonly import("./workbench-model").TraceRecord[];
}

/** One blank rhythm line before a Work group (DESIGN.md space.2/space.3). */
function gapLine(group: string): RenderedLine {
  return { id: `work.gap.${group}`, spans: [{ text: " " }] };
}

/**
 * Tabular unit rows: glyph | STATUS | id | title | dim authority meta. The
 * status and machine-id columns pad to one width per frame so the title
 * column starts at a stable cell (DESIGN.md text.machine "tabular alignment";
 * "do not visually flatten labels, prose, hashes, and statuses"). Ids longer
 * than the 16-cell cap are not padded — an outlier pushes, it never truncates.
 */
function unitRows(units: readonly UnitProjection[]): readonly RenderedLine[] {
  if (units.length === 0) return [];
  const ordered = workUnitOrder(units);
  const statusWidth = ordered.reduce(
    (width, unit) => Math.max(width, cellWidth(unit.status.toUpperCase())),
    7,
  );
  const idWidth = Math.min(
    16,
    ordered.reduce((width, unit) => Math.max(width, cellWidth(unit.unit_id)), 0),
  );
  return ordered.map((unit) => {
    const token = statusToken(unit.status);
    const glyph = SEMANTIC_TOKENS[token].glyph;
    return {
      id: `work.unit.${unit.unit_id}`,
      spans: [
        { text: `${glyph} `, token },
        {
          text: `${padCellText(unit.status.toUpperCase(), statusWidth)} `,
          token,
          emphasis: "bold",
        },
        { text: `${padCellText(unit.unit_id, idWidth)}  ` },
        { text: unit.title || "unavailable" },
        { text: `  authority ${unit.authority}`, emphasis: "dim" },
      ],
    };
  });
}

/** Render the Work primary projection for one snapshot. */
export function renderWorkView(args: ViewRenderArgs): ViewContent {
  const { snapshot, observation } = args;
  const records: RenderedRecord[] = [];

  // Header block: the view question plus the honest aggregate. The aggregate
  // carries the attention glyph only while escalation work exists ("quiet
  // until judgment is required"), and the budget reason is the LAST span so a
  // narrow column clips the explanation, never the state.
  const blocked = snapshot.units.filter((unit) => unit.status === "blocked").length;
  const inFlight = snapshot.units.filter(
    (unit) => unit.status === "running" || unit.status === "active",
  ).length;
  const headerBlock: readonly RenderedLine[] = [
    {
      id: "work.question",
      spans: [{ text: "WORK: what are we trying to move?", emphasis: "bold" }],
    },
    {
      id: "work.summary",
      spans: [
        { text: `${snapshot.units.length} units · `, emphasis: "dim" },
        ...(blocked > 0
          ? [{ text: `! ${blocked} blocked `, token: "attention.required" as const }]
          : [{ text: `${blocked} blocked `, emphasis: "dim" as const }]),
        { text: `${inFlight} in flight · `, emphasis: "dim" },
        { text: "budget: unavailable", emphasis: "dim" },
        { text: " (host budget read not bound to this projection)", emphasis: "dim" },
      ],
    },
  ];

  // Environment honesty (REQ-UX-006): capabilities the host observed as
  // unavailable stay listed with their gate reasons — never faked available.
  const capabilityLines: RenderedLine[] = [];
  for (const capability of snapshot.capability_reasons) {
    if (capability.status !== "unavailable") {
      continue;
    }
    capabilityLines.push({
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
    // Honest empty state with a named next step (REQ-UX-001/006): statement
    // first, guidance on its own line so the actionable part survives narrow
    // columns instead of clipping mid-sentence.
    const anySource = Object.keys(snapshot.source_cursors).length > 0;
    const [statement, guidance] = anySource
      ? [
          "No units reported yet.",
          "Wait for the authoritative source to admit units, or run a Gauntlet run.",
        ]
      : [
          "No authoritative source has reported.",
          "Start 'gauntlet serve' and bind a source, then sync a generation.",
        ];
    const lines: RenderedLine[] = [...headerBlock];
    if (capabilityLines.length > 0) {
      lines.push(gapLine("capability"), ...capabilityLines);
    }
    lines.push(
      gapLine("empty"),
      { id: "work.empty", spans: [{ text: statement, token: "readiness.degraded" }] },
      { id: "work.empty.next", spans: [{ text: guidance, emphasis: "dim" }] },
    );
    return { view: "work", question: "What are we trying to move?", lines, records };
  }

  // Escalation attention first (REQ-UX-001), independent of the Horizon.
  const attentionLines: RenderedLine[] = [];
  for (const entry of snapshot.attention) {
    attentionLines.push({
      id: `work.attention.${entry.focus.id}`,
      spans: [
        { text: "! ", token: "attention.required", emphasis: "bold" },
        { text: `${entry.reason.toUpperCase()} `, token: "attention.required" },
        { text: `${entry.focus.kind}:${entry.focus.source_id} `, emphasis: "bold" },
        { text: "· open the escalation to resolve", emphasis: "dim" },
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

  const unitLineRows = unitRows(snapshot.units);

  // Affordance bar for the current Focus — resolver-driven (DSN-005).
  const affordanceLines: RenderedLine[] = [];
  const focus: FocusRef | null = snapshot.focus;
  if (focus !== null) {
    const materialized = materializeAffordances(focus, observation, {
      session: args.session,
      payment_estimates: args.paymentEstimates,
    });
    affordanceLines.push({
      id: "work.affordances.header",
      spans: [
        { text: "ACTIONS ", emphasis: "bold" },
        { text: `for ${focus.id}`, emphasis: "dim" },
      ],
    });
    for (const item of materialized) {
      // ICM §30 (increment 5): the bar DERIVES its control state — it never
      // chooses one. Glyph/label language per state is unchanged: enabled
      // rows keep ✓ ready / ! responds-to-attention; disabled rows keep
      // × BLOCKED + reasons + next; hidden rows are absent from the bar but
      // stay palette-discoverable with their reason (DSN-009).
      // ICM §30 BUSY_SUBMISSION (host follow-on): the host — never a
      // resolver-status predicate — decides busy; this substitution happens
      // AFTER the resolver derivation, so the invariant that BUSY_SUBMISSION
      // is never descriptor-derived stays with action-control.ts alone.
      const pendingKey = `${item.descriptor.operation}:${focus.id}`;
      const busy = args.pendingOperations?.has(pendingKey) === true;
      const control = busy
        ? busySubmission(item.descriptor.operation)
        : actionControlFor(item.descriptor, item.outcome);
      if (control.state === "HIDDEN") {
        continue;
      }
      const enabled = control.state === "VISIBLE_ENABLED";
      const attention = enabled && control.attentionItem !== undefined;
      affordanceLines.push({
        id: `work.affordance.${item.descriptor.operation}`,
        spans: [
          {
            text: busy ? "… " : enabled ? (attention ? "! " : "✓ ") : "× ",
            token: busy
              ? "readiness.degraded"
              : attention
                ? "attention.required"
                : enabled
                  ? "readiness.ready"
                  : "readiness.blocked",
          },
          { text: `${control.operation.toUpperCase()} `, emphasis: "bold" },
          {
            text: busy
              ? "submitting… awaiting result"
              : attention
                ? `responds to ${control.attentionItem ?? "an attention item"}`
                : enabled
                  ? "ready"
                  : `BLOCKED: ${control.reasons.join(", ")}`,
            token: busy
              ? "readiness.degraded"
              : enabled
                ? undefined
                : attention
                  ? "attention.required"
                  : "readiness.blocked",
          },
          ...(control.state === "VISIBLE_DISABLED" && control.nextStep !== undefined
            ? [{ text: ` · next: ${control.nextStep}`, emphasis: "dim" as const }]
            : []),
        ],
      });
      records.push({
        id: `affordance:${item.descriptor.operation}:${focus.id}`,
        kind: "affordance",
        token: busy
          ? "readiness.degraded"
          : attention
            ? "attention.required"
            : enabled
              ? "readiness.ready"
              : "readiness.blocked",
        fields: {
          operation: item.descriptor.operation,
          status: item.descriptor.status,
          reasons: item.descriptor.reason_codes.join(",") || "none",
          next_step: control.nextStep ?? "none",
          materialization: item.outcome,
          control_state: control.state,
          target: focus.id,
        },
      });
    }
    const paletteOnly = materialized.filter((item) => item.outcome === "palette_only");
    if (paletteOnly.length > 0) {
      // Absent from the view's action bar but discoverable WITH reasons (DSN-009).
      affordanceLines.push({
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
    affordanceLines.push({
      id: "work.affordances.none",
      spans: [{ text: "No Focus selected. Choose a unit in the Navigator.", emphasis: "dim" }],
    });
  }

  // Rhythm: one blank line before each non-empty group after the header block,
  // the same blank rhythm-line idiom the Inspector sections use; an empty
  // group leaves no gap.
  const lines: RenderedLine[] = [...headerBlock];
  const groups: readonly { readonly id: string; readonly lines: readonly RenderedLine[] }[] = [
    { id: "capability", lines: capabilityLines },
    { id: "attention", lines: attentionLines },
    { id: "units", lines: unitLineRows },
    { id: "affordances", lines: affordanceLines },
  ];
  for (const group of groups) {
    if (group.lines.length === 0) {
      continue;
    }
    lines.push(gapLine(group.id), ...group.lines);
  }

  return { view: "work", question: "What are we trying to move?", lines, records };
}
