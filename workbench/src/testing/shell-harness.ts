/**
 * T12 shell test harness: deterministic in-memory fixtures wiring a real
 * WorkbenchModel through ShellSession interaction callbacks. No terminal, no
 * I/O; every fact is asserted on rendered structures, state transitions, and
 * model snapshots.
 */

import type { AffordanceObservation, AffordanceSessionContext } from "../affordance";
import type { Dimensions } from "../geometry";
import type { ShellInteractions, ShellRenderOptions } from "../shell";
import { ShellSession } from "../shell";
import type { FullWorkbenchSnapshot } from "../workbench-model";
import { WorkbenchModel } from "../workbench-model";

export const GEN = "gen-t12";

/** An authenticated operator session for affordance resolution in tests. */
export const OPERATOR_SESSION: AffordanceSessionContext = {
  actor: "operator",
  authenticated: true,
};

const FULL: Dimensions = { cols: 120, rows: 40 };
const COMPACT: Dimensions = { cols: 80, rows: 24 };

/** One authoritative unit event on the seeded source. */
export function unitEvent(
  unitId: string,
  title: string,
  cursor: string,
  status: string,
  authority = "builder",
): Parameters<WorkbenchModel["applyEvent"]>[0] {
  return {
    generation_id: GEN,
    source_id: "gauntlet",
    cursor,
    event_type: "unit_updated",
    payload: { unit_id: unitId, title, status, authority, blast_radius: "isolated" },
    timestamp_ms: 1000,
  };
}

/**
 * The seeded Case: three units (u-root active with verified+unverified
 * evidence, u-other active, u-far BLOCKED), two agent sessions (s-2 child of
 * s-1), and chat projection enabled.
 */
export function seededModel(): WorkbenchModel {
  const model = new WorkbenchModel();
  model.startGeneration(GEN);
  model.applyEvent(unitEvent("u-root", "Root Unit", "c-1", "active"));
  model.applyEvent({
    generation_id: GEN,
    source_id: "gauntlet",
    cursor: "c-2",
    event_type: "evidence_added",
    payload: { evidence_id: "ev-1", unit_id: "u-root", kind: "report", verified: true },
    timestamp_ms: 1100,
  });
  model.applyEvent({
    generation_id: GEN,
    source_id: "gauntlet",
    cursor: "c-3",
    event_type: "evidence_added",
    payload: { evidence_id: "ev-2", unit_id: "u-root", kind: "log", verified: false },
    timestamp_ms: 1200,
  });
  model.applyEvent(unitEvent("u-other", "Other Unit", "c-4", "active"));
  model.applyEvent({
    generation_id: GEN,
    source_id: "gauntlet",
    cursor: "c-5",
    event_type: "evidence_added",
    payload: { evidence_id: "ev-3", unit_id: "u-other", kind: "report", verified: true },
    timestamp_ms: 1400,
  });
  model.applyEvent(unitEvent("u-far", "Far Unit", "c-6", "blocked", "orchestrator"));
  model.applyEvent({
    generation_id: GEN,
    source_id: "gauntlet",
    cursor: "c-7",
    event_type: "session_updated",
    payload: { session_id: "s-1", parent_id: null, model: "model-a", status: "running" },
    timestamp_ms: 1600,
  });
  model.applyEvent({
    generation_id: GEN,
    source_id: "gauntlet",
    cursor: "c-8",
    event_type: "session_updated",
    payload: { session_id: "s-2", parent_id: "s-1", model: "model-a", status: "running" },
    timestamp_ms: 1700,
  });
  model.applyEvent({
    generation_id: GEN,
    source_id: "gauntlet",
    cursor: "c-9",
    event_type: "unit_updated",
    payload: {
      unit_id: "u-root",
      title: "Root Unit",
      status: "active",
      authority: "builder",
      blast_radius: "isolated",
    },
    timestamp_ms: 1800,
  });
  model.markReady(GEN);
  return model;
}

/** An empty (cold) model: a started generation with no admitted events. */
export function coldModel(): WorkbenchModel {
  const model = new WorkbenchModel();
  model.startGeneration(GEN);
  model.markReady(GEN);
  return model;
}

/** A reachable, healthy canonical observation over the model's own reads. */
export function observationFor(
  model: WorkbenchModel,
  overrides: Partial<AffordanceObservation> = {},
): AffordanceObservation {
  const snapshot = model.getSnapshot();
  return {
    reachable: true,
    degraded: false,
    units: snapshot.units.map((unit) => ({ unit_id: unit.unit_id, status: unit.status })),
    open_escalations: [],
    budget_remaining: { usd_micro: 5_000_000, tokens: 1_000_000 },
    governance_denied_units: [],
    authority_revoked_units: [],
    capabilities: [],
    ...overrides,
  };
}

/** Recording interactions: effects are journalled, never applied to a model. */
export function recordingInteractions(): {
  interactions: ShellInteractions;
  journal: { calls: { effect: string; detail: string }[] };
} {
  const journal = { calls: [] as { effect: string; detail: string }[] };
  const interactions: ShellInteractions = {
    selectFocus: (ref) => journal.calls.push({ effect: "select_focus", detail: ref.id }),
    hoist: (ref) => journal.calls.push({ effect: "hoist", detail: ref.id }),
    dehoist: () => journal.calls.push({ effect: "dehoist", detail: "" }),
    quit: () => journal.calls.push({ effect: "quit", detail: "" }),
    invokeAffordance: (operation, ref) =>
      journal.calls.push({ effect: "invoke_affordance", detail: `${operation}:${ref.id}` }),
    submitComposer: (mode, destination, draft) =>
      journal.calls.push({ effect: "submit_composer", detail: `${mode}:${destination}:${draft}` }),
  };
  return { interactions, journal };
}

/** A session whose interactions feed a real WorkbenchModel (composition path). */
export function modelWiredSession(model: WorkbenchModel, dims: Dimensions = FULL): ShellSession {
  return new ShellSession(
    { workspace: { workspace: "gauntlet", reference: "ref-1d457ec9", run: "R17" } },
    dims,
    {
      selectFocus: (ref) => model.selectFocus(ref),
      hoist: (ref) => model.hoistToFocus(ref),
      dehoist: () => model.dehoist(),
    },
  );
}

export function renderOptions(
  model: WorkbenchModel,
  overrides: Partial<AffordanceObservation> = {},
): ShellRenderOptions {
  return { observation: observationFor(model, overrides) };
}

/** Flatten a rendered shell to comparable text lines with surface prefixes. */
export function shellText(
  rendered: ReturnType<ShellSession["render"]>,
  surfaceId?: string,
): string[] {
  const surfaces = surfaceId
    ? rendered.surfaces.filter((surface) => surface.id === surfaceId)
    : rendered.surfaces;
  return surfaces.flatMap((surface) =>
    surface.lines.map((line) => line.spans.map((s) => s.text).join("")),
  );
}

export type { FullWorkbenchSnapshot };
export { COMPACT as COMPACT_DIMS, FULL as FULL_DIMS };
