import type { AffordanceObservation, ResolveAffordanceOptions } from "../affordance";
import { resolveAffordance } from "../affordance";
import type { FocusRef } from "../focus";
import { operationIds } from "../operation-registry";
import type { EvidenceProjection, UnitProjection } from "../projection-store";
import type { FullWorkbenchSnapshot } from "../workbench-model";

/**
 * The smallest renderer-independent Workbench consumer (plan T19 step 3,
 * REQ-TUI-005, DSN-011, REQ-GOAL-005).
 *
 * This module proves OpenTUI is not in the business-state dependency graph:
 * it consumes ONLY Workbench Model snapshots and the canonical affordance
 * resolver and emits its own plain-text projection. Its import set contains
 * ZERO presentation modules — no shell, views, inspector, composer, theme,
 * render-tree, geometry, and no OpenTUI/React package (enforced by
 * renderer-boundary.test.ts). If its answers match what the OpenTUI views
 * derive from the same snapshots, every semantic View could be rendered by
 * another frontend without changing its meaning (DSN-011) — there is no
 * business-semantic fork, because there is exactly one state layer and one
 * resolver underneath both renderers.
 *
 * Deliberately NOT here: status glyph vocabulary, semantic tokens, colors,
 * emphasis, layout — every presentation decision stays with a renderer. Raw
 * status/reason strings pass through untouched.
 */

/** The view name this projection reports (its own; not a fork of "work"). */
export const MINIMAL_PROJECTION_VIEW = "minimal-work";

/** One machine-checkable record of the minimal projection. */
export interface MinimalRecord {
  readonly id: string;
  readonly kind: "unit" | "evidence" | "attention" | "affordance";
  readonly fields: Readonly<Record<string, string>>;
}

export interface MinimalProjection {
  readonly view: typeof MINIMAL_PROJECTION_VIEW;
  readonly question: string;
  readonly lines: readonly string[];
  readonly records: readonly MinimalRecord[];
}

export interface MinimalUnitStatus {
  readonly found: true;
  readonly unit_id: string;
  readonly status: string;
  readonly authority: string;
  readonly blast_radius: string;
  readonly source_id: string;
}

/**
 * Q1 — unit status: what state is this unit in? Derived from the snapshot's
 * own projected row; answers `null` for an unknown unit (honest absence).
 */
export function minimalUnitStatus(
  snapshot: FullWorkbenchSnapshot,
  unitId: string,
): MinimalUnitStatus | null {
  const unit = snapshot.units.find((candidate) => candidate.unit_id === unitId);
  if (unit === undefined) {
    return null;
  }
  return {
    found: true,
    unit_id: unit.unit_id,
    status: unit.status,
    authority: unit.authority,
    blast_radius: unit.blast_radius,
    source_id: unit.source_id,
  };
}

/**
 * Q2 — evidence presence: which evidence does the projection hold for a unit,
 * each with its canonical verified bit and durable locator.
 */
export function minimalEvidenceFor(
  snapshot: FullWorkbenchSnapshot,
  unitId: string,
): readonly {
  evidence_id: string;
  verified: boolean;
  kind: string;
  locator: string;
  source_id: string;
}[] {
  return snapshot.evidence
    .filter((record: EvidenceProjection) => record.unit_id === unitId)
    .sort((a, b) => a.evidence_id.localeCompare(b.evidence_id))
    .map((record) => ({
      evidence_id: record.evidence_id,
      verified: record.verified,
      kind: record.kind,
      locator: record.locator,
      source_id: record.source_id,
    }));
}

export interface MinimalAffordanceReason {
  readonly operation: string;
  readonly status: string;
  readonly reason_codes: readonly string[];
  readonly required_attention_item: string | undefined;
}

/**
 * Q3 — affordance reasons: every registered operation resolved against the
 * canonical observation, each with its status and machine reason codes. The
 * canonical resolver (T09) is shared state-layer machinery, NOT a fork: views
 * and this consumer must reach identical answers from identical inputs.
 */
export function minimalAffordanceReasons(
  focus: FocusRef,
  observation: AffordanceObservation,
  options: ResolveAffordanceOptions = {},
): readonly MinimalAffordanceReason[] {
  return operationIds().map((operation) => {
    const descriptor = resolveAffordance(operation, focus, observation, options);
    return {
      operation: descriptor.operation,
      status: descriptor.status,
      reason_codes: descriptor.reason_codes,
      required_attention_item: descriptor.required_attention_item,
    };
  });
}

function unitLine(unit: UnitProjection): string {
  return `${unit.unit_id}: status=${unit.status} authority=${unit.authority} blast_radius=${unit.blast_radius}`;
}

/**
 * Render the whole minimal projection for one snapshot: summary, attention,
 * unit rows, evidence, and the affordance bar for the snapshot's focus — all
 * plain text over Model state only.
 */
export function renderMinimalProjection(
  snapshot: FullWorkbenchSnapshot,
  observation: AffordanceObservation,
  options: ResolveAffordanceOptions = {},
): MinimalProjection {
  const lines: string[] = [];
  const records: MinimalRecord[] = [];

  lines.push("MINIMAL WORK — what does the Workbench Model say?");
  const blocked = snapshot.units.filter((unit) => unit.status === "blocked").length;
  const inFlight = snapshot.units.filter(
    (unit) => unit.status === "running" || unit.status === "active",
  ).length;
  lines.push(
    `${snapshot.units.length} units; ${blocked} blocked; ${inFlight} in flight; generation=${snapshot.generation}`,
  );

  for (const entry of snapshot.attention) {
    lines.push(
      `ATTENTION ${entry.severity} ${entry.focus.kind}:${entry.focus.source_id} reason=${entry.reason}`,
    );
    records.push({
      id: `attention:${entry.focus.id}`,
      kind: "attention",
      fields: {
        reason: entry.reason,
        severity: entry.severity,
        target: entry.focus.id,
        horizon_independent: String(entry.horizon_independent),
      },
    });
  }

  for (const unit of snapshot.units) {
    lines.push(unitLine(unit));
    records.push({
      id: `unit:${unit.unit_id}`,
      kind: "unit",
      fields: {
        status: unit.status,
        authority: unit.authority,
        blast_radius: unit.blast_radius,
        source: unit.source_id,
      },
    });
  }

  for (const record of snapshot.evidence) {
    lines.push(
      `EVIDENCE ${record.evidence_id} unit=${record.unit_id} verified=${String(record.verified)} locator=${record.locator}`,
    );
    records.push({
      id: `evidence:${record.evidence_id}`,
      kind: "evidence",
      fields: {
        unit: record.unit_id,
        verified: String(record.verified),
        locator: record.locator,
      },
    });
  }

  const focus: FocusRef | null = snapshot.focus;
  if (focus !== null) {
    for (const reason of minimalAffordanceReasons(focus, observation, options)) {
      lines.push(
        `ACTION ${reason.operation} status=${reason.status} reasons=${reason.reason_codes.join(",") || "none"}`,
      );
      records.push({
        id: `affordance:${reason.operation}:${focus.id}`,
        kind: "affordance",
        fields: {
          operation: reason.operation,
          status: reason.status,
          reasons: reason.reason_codes.join(",") || "none",
          ...(reason.required_attention_item !== undefined
            ? { required_attention_item: reason.required_attention_item }
            : {}),
          target: focus.id,
        },
      });
    }
  } else {
    lines.push("No focus.");
  }

  return {
    view: MINIMAL_PROJECTION_VIEW,
    question: "What does the Workbench Model say?",
    lines,
    records,
  };
}
