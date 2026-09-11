/**
 * OpenTUI semantic shell: Environment → Workspace → Surface → View → Focus
 * (plan T12, REQ-TUI-001..004, REQ-UX-002/005, REQ-GOAL-001/005, REQ-OUT-001,
 * DSN-005/006/007/013/015, DESIGN.md interaction ontology + overall shell).
 *
 * Core invariants:
 *   - RENDERER NON-AUTHORITY (REQ-TUI-001, REQ-GOAL-005): every render
 *     function consumes an immutable FullWorkbenchSnapshot plus shell
 *     presentation state and produces a structural render tree. Shell state
 *     is PRESENTATION state only (active view, focused surface, scroll,
 *     palette, navigation); every consequential move is an effect the HOST
 *     routes to canonical admission. This module imports model TYPES only —
 *     never a runtime model.
 *   - PLACEMENTS OVER COPIES (DSN-006): the Navigator outline, Work rows,
 *     agent-context alias rows and Inspector sections all resolve display
 *     state from ONE snapshot through FocusRef identity; no view keeps a
 *     local canonical copy.
 *   - GEOMETRY FLOORS ARE CONFORMANCE (DSN-007, REQ-UX-005): full
 *     three-surface layout at 120x40, one primary pane at 80x24, recoverable
 *     compact-size prompt below 80x24 naming current + required size with a
 *     reachable exit; growing back re-enters with the semantic location
 *     intact.
 *   - NO ROADMAP SURFACES (DSN-015): the view registry cannot contain a
 *     Harness View; registration of a roadmap id throws.
 *   - Lower surfaces materialise ONLY where the snapshot publishes a provider
 *     (REQ-AFF-004 direction); absent providers stay absent with an
 *     inspectable reason.
 */

import {
  type AffordanceObservation,
  type AffordanceSessionContext,
  type AffordanceStatus,
  type PaymentEstimate,
  statusPermitsInvocation,
} from "./affordance";
import {
  type CommandKeyContext,
  commandForKey,
  commandShortcut,
  type SemanticCommandId,
} from "./command-registry";
import type {
  ComposerMode,
  ComposerReplacement,
  ComposerState,
  RenderedComposer,
  TakeoverRequest,
} from "./composer";
import {
  composeKeypress,
  beginTakeover as composerBeginTakeover,
  completeTakeover as composerCompleteTakeover,
  dismissTakeover as composerDismissTakeover,
  expireTakeovers as composerExpireTakeovers,
  renderComposer as composerRender,
  createComposerState,
} from "./composer";
import type { FocusPlacement, FocusRef } from "./focus";
import type { ColumnLayout, Dimensions, GeometryClass } from "./geometry";
import {
  calculateColumns,
  classifyGeometry,
  heightBand,
  renderBelowMinimumPrompt,
} from "./geometry";
import { renderInspector } from "./inspector";
import {
  createLocationHistory,
  currentLocation,
  canGoBack as historyCanGoBack,
  canGoForward as historyCanGoForward,
  type LocationHistory,
  locationBack,
  locationForward,
  locationPush,
} from "./navigation-history";
import type { ProjectionWorld } from "./relations";
import { focusForEvidence, focusForSession, focusForUnit } from "./relations";
import type { RenderedLine, RenderedSpan, ViewContent } from "./render-tree";
import { lineText } from "./render-tree";
import type { ColorCapability, MotionPreference, SemanticToken } from "./theme";
import { runningIndicatorFrames, SEMANTIC_TOKENS } from "./theme";
import {
  createTraceStream,
  streamAppend,
  streamHydrated,
  streamReturnToLive,
  streamScrollDown,
  streamScrollUp,
  streamSelect,
  streamVisibleWindow,
  type TraceStream,
} from "./trace-stream";
import type { ViewContentRenderer } from "./views-chat-trace";
import { defaultViewRegistry } from "./views-chat-trace";
import { materializeAffordances, statusToken } from "./views-work";
import type { FullWorkbenchSnapshot } from "./workbench-model";

// --- Semantic vocabulary (DESIGN.md interaction ontology) --------------------

/** Stable perceptual regions (DESIGN.md Surface). */
export type SurfaceId = "Navigator" | "Primary" | "Inspector" | "Bottom" | "Composer" | "Overlay";

/** Required primary projections (REQ-TUI-003). */
export type PrimaryViewId = "work" | "chat" | "trace";
export const PRIMARY_VIEW_IDS: readonly PrimaryViewId[] = Object.freeze(["work", "chat", "trace"]);

/**
 * Every View id the shell knows. There is deliberately NO `harness` member —
 * the Harness View is roadmap-only and forbidden (DSN-015).
 */
export type ViewId =
  | PrimaryViewId
  | "evidence"
  | "agents"
  | "attention"
  | "files"
  | "kernel"
  | "problems"
  | "terminal"
  | "domain";

export const VIEW_IDS: readonly ViewId[] = Object.freeze([
  "work",
  "chat",
  "trace",
  "evidence",
  "agents",
  "attention",
  "files",
  "kernel",
  "problems",
  "terminal",
  "domain",
]);

export function isPrimaryView(view: ViewId): view is PrimaryViewId {
  return (PRIMARY_VIEW_IDS as readonly string[]).includes(view);
}

/** Case address per DESIGN.md semantic location (no new Case id is minted). */
export interface WorkspaceRef {
  readonly workspace: string;
  readonly reference: string;
  readonly run?: string | undefined;
}

/** Renderer-independent semantic location (DESIGN.md, spec §SemanticLocation). */
export interface SemanticLocation {
  readonly workspace: WorkspaceRef;
  readonly horizon: FocusRef | null;
  readonly surface: SurfaceId;
  readonly view: ViewId;
  readonly focus: FocusRef | null;
}

/** Route-like serialisation of a semantic location (presentation addressing). */
export function formatLocation(location: SemanticLocation): string {
  const params: string[] = [
    `ws=${location.workspace.workspace}`,
    `ref=${location.workspace.reference}`,
  ];
  if (location.workspace.run !== undefined) {
    params.push(`run=${location.workspace.run}`);
  }
  if (location.horizon !== null) {
    params.push(`horizon=${location.horizon.id}`);
  }
  params.push(`view=${location.view}`);
  if (location.focus !== null) {
    params.push(`focus=${location.focus.id}`);
  }
  return `gauntlet://case?${params.join("&")}`;
}

// --- View registry (DSN-015 guard, provider gating) ---------------------------

export type ShellViewRegistry = Readonly<Partial<Record<ViewId, ViewContentRenderer>>>;

/** Roadmap-only ids that must never materialise as surfaces (DSN-015). */
export const ROADMAP_VIEW_IDS: readonly string[] = Object.freeze(["harness"]);

/** Typed error thrown when a roadmap surface is (attempted to be) built. */
export class RoadmapSurfaceError extends Error {
  constructor(detail: string) {
    super(`roadmap surface refused: ${detail}`);
    this.name = "RoadmapSurfaceError";
  }
}

/**
 * Register one view renderer. Roadmap ids are refused at the API (DSN-015);
 * the ViewId union already excludes them, this is the runtime defence.
 */
export function registerShellView(
  registry: ShellViewRegistry,
  view: ViewId,
  renderer: ViewContentRenderer,
): ShellViewRegistry {
  if ((ROADMAP_VIEW_IDS as readonly string[]).includes(view)) {
    throw new RoadmapSurfaceError(`'${view}' is roadmap-only and must not be built (DSN-015)`);
  }
  return Object.freeze({ ...registry, [view]: renderer });
}

/** Structural guard: a registry carrying a roadmap id is nonconforming. */
export function assertNoRoadmapSurfaces(registry: ShellViewRegistry): void {
  for (const key of Object.keys(registry)) {
    if ((ROADMAP_VIEW_IDS as readonly string[]).includes(key)) {
      throw new RoadmapSurfaceError(`registry contains roadmap view '${key}' (DSN-015)`);
    }
  }
}

export interface ViewAvailability {
  readonly view: ViewId;
  readonly status: "available" | "absent";
  readonly reason?: string;
}

/** The snapshot field each lower surface requires as its provider. */
export const LOWER_VIEW_PROVIDERS: Readonly<Partial<Record<ViewId, string>>> = Object.freeze({
  evidence: "evidence",
  agents: "sessions",
  attention: "attention",
  files: "changes",
  kernel: "kernel",
  problems: "problems",
  terminal: "terminal",
  domain: "domain_model",
});

/**
 * Provider-gated view availability (REQ-OUT-001, REQ-AFF-004 direction):
 * primary projections are always available; a lower surface exists only when
 * the snapshot publishes its provider — otherwise it is ABSENT with an
 * inspectable reason, never a dead tab.
 */
export function availableViews(snapshot: FullWorkbenchSnapshot): readonly ViewAvailability[] {
  return VIEW_IDS.map((view) => {
    if (isPrimaryView(view)) {
      return { view, status: "available" } as ViewAvailability;
    }
    const provider = LOWER_VIEW_PROVIDERS[view];
    if (provider === undefined) {
      return { view, status: "absent", reason: "no provider bound" } as ViewAvailability;
    }
    const present = Object.hasOwn(snapshot as object, provider);
    return present
      ? ({ view, status: "available" } as ViewAvailability)
      : ({
          view,
          status: "absent",
          reason: `provider '${provider}' is not published by the current snapshot`,
        } as ViewAvailability);
  });
}

// --- Render-tree shell structures ---------------------------------------------

export interface RenderedSurface {
  readonly id: SurfaceId;
  readonly title: string;
  readonly focused: boolean;
  readonly width: number;
  readonly lines: readonly RenderedLine[];
  /** The FocusRef placements hosted by this surface in this snapshot. */
  readonly placements: readonly FocusPlacement[];
}

export type PaletteEntry =
  | {
      readonly type: "header";
      readonly label: string;
    }
  | {
      readonly type: "view";
      readonly view: ViewId;
      readonly status: "available" | "absent";
      readonly reason?: string;
      /** Semantic command this row activates (ICM §28: rows expose identity). */
      readonly commandId: SemanticCommandId;
      /** Default shortcut for the command, when one exists (ICM §28). */
      readonly shortcut: string | null;
    }
  | {
      readonly type: "affordance";
      readonly operation: string;
      readonly ref: FocusRef;
      readonly status: AffordanceStatus;
      readonly reasons: readonly string[];
      readonly outcome: string;
    }
  | {
      readonly type: "focus";
      readonly ref: FocusRef;
      readonly label: string;
    };

export interface RenderedPalette {
  readonly entries: readonly PaletteEntry[];
  readonly index: number;
  readonly lines: readonly RenderedLine[];
}

export interface RenderedPrompt {
  readonly lines: readonly RenderedLine[];
  readonly exitKey: string;
}

export interface RenderedShell {
  readonly geometryClass: GeometryClass;
  readonly columns: ColumnLayout;
  readonly location: SemanticLocation;
  readonly breadcrumb: readonly string[];
  readonly header: RenderedLine;
  readonly surfaces: readonly RenderedSurface[];
  /** Case-wide attention summary — rendered, never focus-stealing (REQ-UX-002). */
  readonly attention: readonly RenderedLine[];
  readonly help: readonly RenderedLine[];
  readonly palette: RenderedPalette | null;
  readonly prompt: RenderedPrompt | null;
  /** Compact side drawer (V2, UESC Compact: switchable side surfaces). */
  readonly drawer: {
    readonly side: "left" | "right";
    readonly surface: "Navigator" | "Inspector";
    readonly title: string;
    readonly lines: readonly RenderedLine[];
  } | null;
  /** Bottom Surface (UESC §7.5): tabs of AVAILABLE bottom-candidate views
   * (REQ-AFF-004 — absent providers are not tabs). Null = not rendered
   * (Compact, or short heights where Bottom collapses first). */
  readonly bottom: {
    readonly open: boolean;
    readonly tabs: readonly ViewId[];
  } | null;
  readonly motion: MotionPreference;
}

// --- Navigator outline ---------------------------------------------------------

export interface OutlineRow {
  readonly id: string;
  readonly depth: number;
  readonly ref: FocusRef | null;
  readonly kind: string;
  readonly label: string;
  readonly token?: SemanticToken;
  /** Alias/relation indicator for a second placement of one Focus (DSN-006). */
  readonly alias?: string | undefined;
}

/** Resolve display text for a FocusRef from the snapshot (never a local copy). */
export function resolveFocusLabel(snapshot: FullWorkbenchSnapshot, ref: FocusRef): string {
  const unit = snapshot.units.find((candidate) => candidate.unit_id === ref.source_id);
  if (unit !== undefined) {
    return unit.title || ref.source_id;
  }
  const session = snapshot.sessions.find((candidate) => candidate.session_id === ref.source_id);
  if (session !== undefined) {
    return `${session.model} (${session.status})`;
  }
  const evidence = snapshot.evidence.find((candidate) => candidate.evidence_id === ref.source_id);
  if (evidence !== undefined) {
    return `${evidence.kind} · ${evidence.evidence_id}`;
  }
  return ref.human_label ?? ref.source_id;
}

/**
 * The Navigator semantic outline: Case root, WorkItem placements with their
 * evidence closure, Agent sessions, alias rows for extra Navigator placements
 * of one Focus (DSN-006), and Case-wide attention rows. Every row is keyed by
 * a FocusRef — the outline is a set of placements, not copied state.
 */
export function navigatorOutline(snapshot: FullWorkbenchSnapshot): readonly OutlineRow[] {
  const rows: OutlineRow[] = [
    { id: "outline.case", depth: 0, ref: null, kind: "Case", label: "Case outline" },
  ];

  for (const unit of snapshot.units) {
    const ref = focusForUnit(unit);
    rows.push({
      id: `outline.unit.${unit.unit_id}`,
      depth: 1,
      ref,
      kind: "WorkItem",
      label: resolveFocusLabel(snapshot, ref),
      token: statusToken(unit.status),
    });
    for (const evidence of snapshot.evidence.filter((item) => item.unit_id === unit.unit_id)) {
      const evidenceRef = focusForEvidence(evidence);
      rows.push({
        id: `outline.evidence.${evidence.evidence_id}`,
        depth: 2,
        ref: evidenceRef,
        kind: "Evidence",
        label: resolveFocusLabel(snapshot, evidenceRef),
        token: evidence.verified ? "integrity.verified" : "integrity.partial",
      });
    }
  }

  for (const session of snapshot.sessions) {
    const ref = focusForSession(session);
    rows.push({
      id: `outline.session.${session.session_id}`,
      depth: 1,
      ref,
      kind: "Agent",
      label: resolveFocusLabel(snapshot, ref),
      token: session.status === "running" ? "execution.running" : "execution.waiting",
    });
  }

  // Additional Navigator-surface placements of an already-outlined Focus
  // render as ALIAS rows: one Focus, multiple placements, no duplicate
  // visual identity (DESIGN.md Navigator, DSN-006).
  for (const placement of snapshot.placements) {
    if (placement.surface !== "Navigator" || placement.view === "outline") {
      continue;
    }
    rows.push({
      id: `outline.placement.${placement.id}`,
      depth: 1,
      ref: placement.focus,
      kind: placement.focus.kind,
      label: resolveFocusLabel(snapshot, placement.focus),
      token: "structure.neutral",
      alias: placement.relation_role ?? placement.view,
    });
  }

  for (const entry of snapshot.attention) {
    rows.push({
      id: `outline.attention.${entry.focus.id}`,
      depth: 1,
      ref: entry.focus,
      kind: "AttentionItem",
      label: `${entry.reason} → ${resolveFocusLabel(snapshot, entry.focus)}`,
      token: "attention.required",
    });
  }

  return rows;
}

/** ProjectionWorld view over one snapshot (for relation derivation). */
export function worldFromSnapshot(snapshot: FullWorkbenchSnapshot): ProjectionWorld {
  return {
    generation: snapshot.generation,
    units: snapshot.units,
    evidence: snapshot.evidence,
    sessions: snapshot.sessions,
    source_cursors: snapshot.source_cursors,
  };
}

// --- Pointer gestures (ICM §35: two representations, one behavior) ------------

/** A pointer gesture resolved against the renderer's hit map. surfaceId is
 * the hit-map region id; lineId the span-tree line id under the pointer
 * (null when the region row carries no stable identity); x the column
 * offset within the surface (for span-level resolution, e.g. tabs). */
export type PointerGesture =
  | {
      readonly kind: "click" | "double_click";
      readonly surfaceId: string;
      readonly lineId: string | null;
      readonly x: number;
    }
  | { readonly kind: "wheel"; readonly surfaceId: string; readonly delta: number };

/** Wheel scroll step per notch for tall regions (Primary). */
const PRIMARY_WHEEL_LINES = 3;

/**
 * The pointer twin of handleKey: gestures resolve to the SAME semantic
 * commands and effects the keyboard path produces (ICM §24/§35). There is no
 * second behavior pipeline — consequential effects are the same ShellEffects
 * through the same outbound edge.
 */
export function handlePointer(
  state: ShellState,
  snapshot: FullWorkbenchSnapshot,
  gesture: PointerGesture,
  paletteContext: PaletteContext = {},
): { state: ShellState; effects: readonly ShellEffect[] } {
  // Below the minimum the recoverable prompt owns ALL input (DSN-007).
  if (classifyGeometry(state.dimensions) === "BelowMinimum") {
    return { state, effects: [] };
  }

  if (gesture.kind === "wheel") {
    const steps = Math.abs(gesture.delta);
    if (steps === 0) return { state, effects: [] };
    const command =
      gesture.surfaceId === "Overlay"
        ? gesture.delta > 0
          ? ("ui.palette.move.next" as const)
          : ("ui.palette.move.prev" as const)
        : gesture.delta > 0
          ? ("nav.move.next" as const)
          : ("nav.move.prev" as const);
    // The region under the pointer governs: Navigator selection moves when
    // the wheel is over the Navigator, Primary scrolls when over Primary,
    // and other regions scroll nothing. The keyboard twin is nav.move with
    // the matching focused surface.
    let next = state;
    if (gesture.surfaceId === "Navigator") {
      next = { ...state, focusedSurface: "Navigator" };
    } else if (gesture.surfaceId === "Primary") {
      next = { ...state, focusedSurface: "Primary" };
    } else if (gesture.surfaceId === "Overlay" && !state.paletteOpen) {
      return { state, effects: [] };
    } else if (gesture.surfaceId !== "Overlay") {
      return { state, effects: [] };
    }
    let result = { state: next, effects: [] as readonly ShellEffect[] };
    const times = gesture.surfaceId === "Primary" ? steps * PRIMARY_WHEEL_LINES : steps;
    for (let i = 0; i < times; i += 1) {
      result = handleCommand(result.state, snapshot, command, paletteContext);
    }
    // Surface focus itself is presentation-local: restore the caller's focus
    // so a wheel over Navigator does not steal keyboard focus.
    return { state: { ...result.state, focusedSurface: state.focusedSurface }, effects: [] };
  }

  const { kind, surfaceId, lineId } = gesture;

  // Palette rows: click selects the row; double-click activates it — exactly
  // what arrow keys + Enter do (ICM §35.1/§35.2).
  if (surfaceId === "Overlay") {
    if (!state.paletteOpen || lineId === null || !lineId.startsWith("palette.")) {
      return { state, effects: [] };
    }
    const index = Number(lineId.slice("palette.".length));
    if (!Number.isInteger(index) || index < 0) return { state, effects: [] };
    const selected: ShellState = Object.freeze({ ...state, paletteIndex: index });
    if (kind === "click") {
      return { state: Object.freeze({ ...selected, pendingConfirm: null }), effects: [] };
    }
    return handleCommand(selected, snapshot, "ui.palette.activate", paletteContext);
  }

  if (surfaceId === "Navigator" && lineId !== null) {
    const rows = navigatorOutline(snapshot);
    const index = rows.findIndex((row) => row.id === lineId);
    if (index < 0) return { state, effects: [] };
    const moved: ShellState = Object.freeze({
      ...state,
      navigatorIndex: index,
      focusedSurface: "Navigator",
    });
    // Single click selects (ICM §35.1); rows without a ref only move the cursor.
    const row = rows[index];
    if (row?.ref === undefined || row.ref === null) {
      return { state: moved, effects: [] };
    }
    const result = handleCommand(moved, snapshot, "nav.focus.select", paletteContext);
    return {
      state: { ...result.state, focusedSurface: state.focusedSurface },
      effects: result.effects,
    };
  }

  if (surfaceId === "Primary") {
    if (lineId === "primary.tabs") {
      // Span-level resolution: spans[0] is the "PRIMARY " prefix; tab i spans
      // follow in primaryTabViews order (one source with the render).
      const views = primaryTabViews(snapshot);
      let offset = "PRIMARY ".length;
      for (const view of views) {
        const width = `[${view.toUpperCase()}]`.length;
        if (gesture.x >= offset && gesture.x < offset + width + 1) {
          return handleCommand(
            state,
            snapshot,
            `view.activate.${view}` as SemanticCommandId,
            paletteContext,
          );
        }
        offset += width + 1;
      }
      return { state, effects: [] };
    }
    if (lineId !== null && lineId.startsWith("trace.")) {
      const id = lineId.slice("trace.".length);
      // Selection by stable id only (UESC §16.5): the window never moves.
      return {
        state: Object.freeze({ ...state, trace: streamSelect(state.trace, id) }),
        effects: [],
      };
    }
    return { state, effects: [] };
  }

  if (surfaceId === "Composer") {
    return handleCommand(state, snapshot, "ui.composer.focus", paletteContext);
  }

  // Compact drawer rows (V2): the drawer renders placements of the same
  // rows, so clicks route exactly like the docked surface (DSN-06).
  if (surfaceId === "Drawer" && lineId !== null && state.drawer === "Navigator") {
    const rows = navigatorOutline(snapshot);
    const index = rows.findIndex((row) => row.id === lineId);
    if (index < 0) return { state, effects: [] };
    const row = rows[index];
    const moved: ShellState = Object.freeze({ ...state, navigatorIndex: index });
    if (row?.ref === undefined || row.ref === null) {
      return { state: moved, effects: [] };
    }
    return {
      state: Object.freeze({
        ...moved,
        location: Object.freeze({ ...moved.location, focus: row.ref }),
      }),
      effects: [{ type: "select_focus", ref: row.ref }],
    };
  }
  // Inspector drawer rows are detail projections: click only keeps the drawer.
  if (surfaceId === "Drawer") {
    return { state, effects: [] };
  }

  return { state, effects: [] };
}

// --- Shell state machine -------------------------------------------------------

export interface ShellHostConfig {
  readonly workspace: WorkspaceRef;
  readonly motion?: MotionPreference | undefined;
  readonly colorCapability?: ColorCapability | undefined;
}

export interface ShellState {
  readonly location: SemanticLocation;
  readonly focusedSurface: SurfaceId;
  readonly paletteOpen: boolean;
  readonly paletteIndex: number;
  /** Palette text query (UESC §22.8). Empty string = unfiltered. */
  readonly paletteQuery: string;
  /** Bottom Surface visibility (UESC §7.5); collapsed by default below Wide-tall. */
  readonly bottomOpen: boolean;
  /**
   * Inspector pin (UESC §7.4): null = FOLLOW_SELECTION (the Inspector targets
   * the current selection); set = PINNED (the Inspector keeps one Focus until
   * explicitly released). Pinning never mutates selection.
   */
  readonly inspectorPin: { readonly ref: FocusRef } | null;
  /** Trace ledger presentation machine (UESC §16.2); see reconcileTrace. */
  readonly trace: TraceStream;
  /** Compact side drawer (V2): overlays the Navigator or Inspector at
   * 80–119 cols; null when closed. Full/Wide render them docked instead. */
  readonly drawer: "Navigator" | "Inspector" | null;
  /** Back/forward perceptual-location history (ICM §6.6): cursor-only
   * navigation; entries are never rewritten, domain state never mutated. */
  readonly history: LocationHistory;
  /** Staged invoke awaiting a second Enter (harden: no one-Enter consequential invoke). */
  readonly pendingConfirm: { readonly operation: string; readonly ref: FocusRef } | null;
  readonly navigatorIndex: number;
  /** Per-view scroll offsets; preserved across view switches (REQ-UX-005). */
  readonly scroll: Readonly<Partial<Record<ViewId, number>>>;
  readonly dimensions: Dimensions;
  readonly motion: MotionPreference;
  readonly colorCapability: ColorCapability;
}

/** Initial shell state: Work view on the Primary surface, whole-Case world. */
export function createShellState(host: ShellHostConfig, dims: Dimensions): ShellState {
  return Object.freeze({
    location: Object.freeze({
      workspace: host.workspace,
      horizon: null,
      surface: "Primary",
      view: "work",
      focus: null,
    }),
    focusedSurface: "Primary",
    paletteOpen: false,
    paletteIndex: 0,
    paletteQuery: "",
    bottomOpen: false,
    inspectorPin: null,
    trace: createTraceStream(64),
    drawer: null,
    history: createLocationHistory(
      Object.freeze({
        workspace: host.workspace,
        horizon: null,
        surface: "Primary" as const,
        view: "work" as const,
        focus: null,
      }),
    ),
    pendingConfirm: null,
    navigatorIndex: 0,
    scroll: Object.freeze({}),
    dimensions: dims,
    motion: host.motion ?? "full",
    colorCapability: host.colorCapability ?? "truecolor",
  });
}

/** Effects a key press asks the HOST to perform (canonical path only). */
export type ShellEffect =
  | { readonly type: "select_focus"; readonly ref: FocusRef }
  | { readonly type: "hoist"; readonly ref: FocusRef }
  | { readonly type: "dehoist" }
  | { readonly type: "quit" }
  | { readonly type: "invoke_affordance"; readonly operation: string; readonly ref: FocusRef };

export interface ShellInteractions {
  readonly selectFocus?: ((ref: FocusRef) => void) | undefined;
  readonly hoist?: ((ref: FocusRef) => void) | undefined;
  readonly dehoist?: (() => void) | undefined;
  readonly quit?: (() => void) | undefined;
  readonly invokeAffordance?: ((operation: string, ref: FocusRef) => void) | undefined;
  readonly submitComposer?:
    | ((mode: ComposerMode, destination: string, draft: string) => void)
    | undefined;
}

/** Apply effects to host interaction callbacks (the only outbound edge). */
export function applyShellEffects(
  effects: readonly ShellEffect[],
  interactions: ShellInteractions,
): void {
  for (const effect of effects) {
    switch (effect.type) {
      case "select_focus":
        interactions.selectFocus?.(effect.ref);
        break;
      case "hoist":
        interactions.hoist?.(effect.ref);
        break;
      case "dehoist":
        interactions.dehoist?.();
        break;
      case "quit":
        interactions.quit?.();
        break;
      case "invoke_affordance":
        interactions.invokeAffordance?.(effect.operation, effect.ref);
        break;
    }
  }
}

/** Everything the palette's affordance entries resolve against (host-supplied). */
export interface PaletteContext {
  readonly observation?: AffordanceObservation | undefined;
  readonly session?: AffordanceSessionContext | undefined;
  readonly paymentEstimates?: Readonly<Record<string, PaymentEstimate>> | undefined;
}

/** Primary-view activation targets keyed by the view.activate.<view> suffix.
 * The command id is type-validated (SemanticCommandId), so the suffix is
 * always a primary view; lower/promotable views activate through the palette. */
const VIEW_COMMAND_TARGETS: Readonly<Record<string, PrimaryViewId>> = Object.freeze({
  work: "work",
  chat: "chat",
  trace: "trace",
});

/** The semantic command that activates a view (registry-keyed, ICM §4). */
function viewCommandFor(view: ViewId): SemanticCommandId {
  switch (view) {
    case "work":
      return "view.activate.work";
    case "chat":
      return "view.activate.chat";
    case "trace":
      return "view.activate.trace";
    default:
      // Lower/promotable views have no dedicated activation command yet;
      // their palette rows carry the id of the generic activation path.
      return "ui.palette.activate";
  }
}

function visibleSurfaces(geo: GeometryClass): readonly SurfaceId[] {
  if (geo === "Full" || geo === "Wide") {
    return ["Navigator", "Primary", "Inspector", "Composer"];
  }
  return ["Primary", "Composer"];
}

function nextSurface(state: ShellState, geo: GeometryClass, direction = 1): SurfaceId {
  const surfaces = visibleSurfaces(geo);
  const index = surfaces.indexOf(state.focusedSurface);
  return surfaces[(index + direction + surfaces.length) % surfaces.length] ?? "Primary";
}

function switchView(state: ShellState, view: ViewId): ShellState {
  // Scroll offsets are per-view and preserved; only the location moves.
  return Object.freeze({
    ...state,
    location: Object.freeze({ ...state.location, view }),
  });
}

/**
 * Palette entries: every view (available or absent WITH reason) plus, when a
 * Focus is selected and the host supplied an observation, every affordance
 * resolution — including palette-only ones with their reasons (DSN-009).
 * Non-empty groups lead with a non-actionable header row carrying the count,
 * so the list scans as chunks instead of one flat wall.
 */
export function paletteEntries(
  state: ShellState,
  snapshot: FullWorkbenchSnapshot,
  context: PaletteContext,
): readonly PaletteEntry[] {
  const viewRows: PaletteEntry[] = availableViews(snapshot).map((availability) =>
    availability.status === "available"
      ? ({
          type: "view",
          view: availability.view,
          status: "available",
          commandId: viewCommandFor(availability.view),
          shortcut: commandShortcut(viewCommandFor(availability.view)),
        } as PaletteEntry)
      : ({
          type: "view",
          view: availability.view,
          status: "absent",
          reason: availability.reason,
          commandId: viewCommandFor(availability.view),
          shortcut: commandShortcut(viewCommandFor(availability.view)),
        } as PaletteEntry),
  );

  const actionRows: PaletteEntry[] = [];
  const focus = inspectorTarget(state, snapshot);
  if (focus !== null && context.observation !== undefined) {
    for (const item of materializeAffordances(focus, context.observation, {
      session: context.session,
      payment_estimates: context.paymentEstimates,
    })) {
      actionRows.push({
        type: "affordance",
        operation: item.descriptor.operation,
        ref: focus,
        status: item.descriptor.status,
        reasons: item.descriptor.reason_codes,
        outcome: item.outcome,
      });
    }
  }

  // Compact geometry has no Navigator surface: outline Foci join the palette
  // so every journey stays keyboard-only at 80x24 (REQ-UX-005).
  const focusRows: PaletteEntry[] = [];
  if (classifyGeometry(state.dimensions) === "Compact") {
    for (const row of navigatorOutline(snapshot)) {
      if (row.ref !== null) {
        focusRows.push({ type: "focus", ref: row.ref, label: `${row.kind}: ${row.label}` });
      }
    }
  }

  const entries: PaletteEntry[] = [];
  if (viewRows.length > 0) {
    entries.push({ type: "header", label: `Views (${viewRows.length})` }, ...viewRows);
  }
  if (actionRows.length > 0) {
    entries.push({ type: "header", label: `Actions (${actionRows.length})` }, ...actionRows);
  }
  if (focusRows.length > 0) {
    entries.push({ type: "header", label: `Focus (${focusRows.length})` }, ...focusRows);
  }
  return entries;
}

/**
 * Pure keyboard routing: keys are one representation of intent (ICM §34). The
 * physical key resolves through the command registry to a SemanticCommandId,
 * then `handleCommand` performs it. Pointer gestures and future menus call
 * `handleCommand` with the same ids, so representations cannot fork behavior.
 */
export function handleKey(
  state: ShellState,
  snapshot: FullWorkbenchSnapshot,
  key: string,
  paletteContext: PaletteContext = {},
): { state: ShellState; effects: readonly ShellEffect[] } {
  const context: CommandKeyContext = {
    surface:
      classifyGeometry(state.dimensions) === "BelowMinimum"
        ? "below_minimum"
        : state.paletteOpen
          ? "palette_open"
          : "shell",
  };
  // Palette query editing is TEXT INPUT, not command intent (the composer
  // precedent): printable graphemes and backspace edit the query while the
  // palette owns the keyboard. Control keys fall through to the registry.
  if (context.surface === "palette_open") {
    const edited = editPaletteQuery(state, key);
    if (edited !== null) {
      return { state: edited, effects: [] };
    }
  }

  const command = commandForKey(key, context);
  if (command === null) {
    return { state, effects: [] };
  }
  return handleCommand(state, snapshot, command, paletteContext);
}

/** Keys terminalKey reports as literal characters (no modifiers, not a
 * control name). Multi-codepoint graphemes arrive as one key. */
function isPrintableKey(key: string): boolean {
  if (key.length === 0 || key.includes("+") || key.startsWith("ctrl")) return false;
  const controlNames = new Set([
    "escape",
    "enter",
    "return",
    "tab",
    "shift+tab",
    "backspace",
    "delete",
    "left",
    "right",
    "up",
    "down",
    "home",
    "end",
    "pageup",
    "pagedown",
    "space",
  ]);
  return !controlNames.has(key);
}

/** Edit the palette query with one input key; null when the key is not input. */
function editPaletteQuery(state: ShellState, key: string): ShellState | null {
  if (key === "backspace") {
    const chars = Array.from(state.paletteQuery);
    if (chars.length === 0) return state;
    const next = chars.slice(0, -1).join("");
    return Object.freeze({ ...state, paletteQuery: next, paletteIndex: 0, pendingConfirm: null });
  }
  if (isPrintableKey(key)) {
    return Object.freeze({
      ...state,
      paletteQuery: state.paletteQuery + key,
      paletteIndex: 0,
      pendingConfirm: null,
    });
  }
  return null;
}

/**
 * Filter palette entries by the active query (UESC §22.8). Matching is
 * case-insensitive over every identity field a row exposes: view name +
 * command id + shortcut, operation + status + reason codes, focus label.
 * Headers drop out while filtering (groups collapse to matches).
 */
export function filterPaletteEntries(
  entries: readonly PaletteEntry[],
  query: string,
): readonly PaletteEntry[] {
  const q = query.trim().toLowerCase();
  if (q.length === 0) return entries;
  return entries.filter((entry) => {
    switch (entry.type) {
      case "header":
        return false;
      case "view":
        return [entry.view, entry.commandId, entry.shortcut ?? ""]
          .join(" ")
          .toLowerCase()
          .includes(q);
      case "affordance":
        return [entry.operation, entry.status, entry.reasons.join(" ")]
          .join(" ")
          .toLowerCase()
          .includes(q);
      case "focus":
        return entry.label.toLowerCase().includes(q);
      default:
        return false;
    }
  });
}

/** The entries palette movement and activation walk (query-filtered). */
function activePaletteEntries(
  state: ShellState,
  snapshot: FullWorkbenchSnapshot,
  paletteContext: PaletteContext,
): readonly PaletteEntry[] {
  return filterPaletteEntries(paletteEntries(state, snapshot, paletteContext), state.paletteQuery);
}

/**
 * The single semantic dispatcher (ICM §4/§35 convergence point). Keyboard,
 * pointer, palette rows, and future menus all reduce to this function; there
 * is no second behavior pipeline. No model access, no side effects here.
 */
export function handleCommand(
  state: ShellState,
  snapshot: FullWorkbenchSnapshot,
  command: SemanticCommandId,
  paletteContext: PaletteContext = {},
): { state: ShellState; effects: readonly ShellEffect[] } {
  const geo = classifyGeometry(state.dimensions);

  // Below the minimum the recoverable prompt owns the input (DSN-007).
  if (geo === "BelowMinimum") {
    return command === "app.quit" ? { state, effects: [{ type: "quit" }] } : { state, effects: [] };
  }

  if (state.paletteOpen) {
    return handlePaletteCommand(state, snapshot, command, paletteContext);
  }

  // Shell context: palette commands are inert while the palette is closed.
  switch (command) {
    case "app.quit":
      return { state, effects: [{ type: "quit" }] };
    case "view.activate.work":
    case "view.activate.chat":
    case "view.activate.trace": {
      const switchTo = VIEW_COMMAND_TARGETS[command.slice("view.activate.".length)];
      return switchTo !== undefined
        ? { state: switchView(state, switchTo), effects: [] }
        : { state, effects: [] };
    }
    case "ui.surface.prev":
      return {
        state: Object.freeze({ ...state, focusedSurface: nextSurface(state, geo, -1) }),
        effects: [],
      };
    case "ui.surface.next":
      return {
        state: Object.freeze({ ...state, focusedSurface: nextSurface(state, geo) }),
        effects: [],
      };
    case "ui.dismiss":
      // Escape dismisses local UI (REQ-UX-004): the Compact drawer first,
      // then auxiliary surfaces blur back to Primary. It never cancels
      // admitted work.
      if (state.drawer !== null) {
        return { state: Object.freeze({ ...state, drawer: null }), effects: [] };
      }
      return {
        state:
          state.focusedSurface === "Primary"
            ? state
            : Object.freeze({ ...state, focusedSurface: "Primary" }),
        effects: [],
      };
    case "ui.palette.open":
      return {
        state: Object.freeze({
          ...state,
          paletteOpen: true,
          paletteIndex: 0,
          paletteQuery: "",
          focusedSurface: "Overlay",
        }),
        effects: [],
      };
    case "nav.move.prev":
    case "nav.move.next": {
      const delta = command === "nav.move.prev" ? -1 : 1;
      if (state.focusedSurface === "Navigator") {
        const rows = navigatorOutline(snapshot);
        const bound = Math.max(rows.length - 1, 0);
        return {
          state: Object.freeze({
            ...state,
            navigatorIndex: Math.min(Math.max(0, state.navigatorIndex + delta, 0), bound),
          }),
          effects: [],
        };
      }
      if (state.focusedSurface === "Primary") {
        // The Trace view is a ledger window (UESC §16.2): up/down drive the
        // follow/hold machine, not the line scroller. Scrolling up suspends
        // live-follow; reaching the tail resumes it.
        if (state.location.view === "trace") {
          return {
            state: Object.freeze({
              ...state,
              trace: delta < 0 ? streamScrollUp(state.trace) : streamScrollDown(state.trace),
            }),
            effects: [],
          };
        }
        const current = state.scroll[state.location.view] ?? 0;
        return {
          state: Object.freeze({
            ...state,
            scroll: Object.freeze({
              ...state.scroll,
              [state.location.view]: Math.max(0, current + delta),
            }),
          }),
          effects: [],
        };
      }
      return { state, effects: [] };
    }
    case "nav.history.back": {
      if (!historyCanGoBack(state.history)) return { state, effects: [] };
      const back = locationBack(state.history);
      const target = currentLocation(back);
      if (target === null) return { state, effects: [] };
      return {
        state: Object.freeze({
          ...state,
          history: back,
          location: Object.freeze({ ...target }) as SemanticLocation,
        }),
        effects: [],
      };
    }
    case "nav.history.forward": {
      if (!historyCanGoForward(state.history)) return { state, effects: [] };
      const forward = locationForward(state.history);
      const target = currentLocation(forward);
      if (target === null) return { state, effects: [] };
      return {
        state: Object.freeze({
          ...state,
          history: forward,
          location: Object.freeze({ ...target }) as SemanticLocation,
        }),
        effects: [],
      };
    }
    case "trace.follow.resume":
      return {
        state: Object.freeze({ ...state, trace: streamReturnToLive(state.trace) }),
        effects: [],
      };
    case "nav.focus.select": {
      if (state.focusedSurface === "Navigator") {
        const row = navigatorOutline(snapshot)[state.navigatorIndex];
        if (row?.ref !== undefined && row.ref !== null) {
          return {
            state: Object.freeze({
              ...state,
              location: Object.freeze({ ...state.location, focus: row.ref }),
            }),
            effects: [{ type: "select_focus", ref: row.ref }],
          };
        }
        return { state, effects: [] };
      }
      return { state, effects: [] };
    }
    case "nav.horizon.hoist": {
      const target = snapshot.focus ?? state.location.focus;
      if (target !== null) {
        return {
          state: Object.freeze({
            ...state,
            location: Object.freeze({ ...state.location, horizon: target }),
          }),
          effects: [{ type: "hoist", ref: target }],
        };
      }
      return { state, effects: [] };
    }
    case "nav.horizon.dehoist": {
      if (snapshot.active_horizon !== null || state.location.horizon !== null) {
        return {
          state: Object.freeze({
            ...state,
            location: Object.freeze({ ...state.location, horizon: null }),
          }),
          effects: [{ type: "dehoist" }],
        };
      }
      return { state, effects: [] };
    }
    case "ui.composer.focus":
      return { state: Object.freeze({ ...state, focusedSurface: "Composer" }), effects: [] };
    case "ui.bottom.toggle":
      // Bottom Surface visibility is presentation state only (UESC §7.5):
      // collapsing it never stops or detaches anything it hosts.
      return { state: Object.freeze({ ...state, bottomOpen: !state.bottomOpen }), effects: [] };
    case "ui.inspector.pin.toggle": {
      // UESC §7.4 PINNED ↔ FOLLOW_SELECTION. Pinning captures the CURRENT
      // inspector target; it never mutates selection. Unpinning resumes
      // following the live selection.
      if (state.inspectorPin !== null) {
        return { state: Object.freeze({ ...state, inspectorPin: null }), effects: [] };
      }
      const target = inspectorTarget(state, snapshot);
      if (target === null) return { state, effects: [] };
      return {
        state: Object.freeze({ ...state, inspectorPin: Object.freeze({ ref: target }) }),
        effects: [],
      };
    }
    case "ui.inspector.toggle":
      // V2: at Compact the Inspector is a switchable right drawer (UESC
      // Compact); at Full/Wide it is docked and always visible — the toggle
      // is inert there until docking chrome exists.
      if (geo !== "Compact") return { state, effects: [] };
      return {
        state: Object.freeze({
          ...state,
          drawer: state.drawer === "Inspector" ? null : "Inspector",
        }),
        effects: [],
      };
    case "ui.navigator.toggle":
      // V2: the Navigator as a Compact left drawer (keyboard n, edge-visible).
      if (geo !== "Compact") return { state, effects: [] };
      return {
        state: Object.freeze({
          ...state,
          drawer: state.drawer === "Navigator" ? null : "Navigator",
        }),
        effects: [],
      };
    default:
      return { state, effects: [] };
  }
}

/** Derive the trace ledger window for the Trace view: follow state, unseen
 * count, and the snapshot records visible in the stream's current window. */
function traceLedgerWindow(
  state: ShellState,
  snapshot: FullWorkbenchSnapshot,
): import("./views-work").TraceLedgerWindow {
  const window = streamVisibleWindow(state.trace);
  const byId = new Map(snapshot.trace_records.map((record) => [record.trace_id, record]));
  const records = window
    .map((entry) => byId.get(entry.id))
    .filter(
      (record): record is FullWorkbenchSnapshot["trace_records"][number] => record !== undefined,
    );
  return {
    followState: state.trace.followState,
    unseenCount: state.trace.unseenCount,
    records,
  };
}

/** The Inspector's current target: the pinned Focus when PINNED, else the
 * live selection (snapshot first, shell location as presentation fallback). */
function inspectorTarget(state: ShellState, snapshot: FullWorkbenchSnapshot): FocusRef | null {
  return state.inspectorPin?.ref ?? snapshot.focus ?? state.location.focus;
}

/** Palette-open context: the palette owns the keyboard (shell keys inert). */
function handlePaletteCommand(
  state: ShellState,
  snapshot: FullWorkbenchSnapshot,
  command: SemanticCommandId,
  paletteContext: PaletteContext,
): { state: ShellState; effects: readonly ShellEffect[] } {
  switch (command) {
    case "ui.palette.dismiss":
      // Escape backs out one level: staged confirm, then a non-empty query,
      // then the palette itself. It never cancels admitted work.
      if (state.pendingConfirm !== null) {
        return { state: Object.freeze({ ...state, pendingConfirm: null }), effects: [] };
      }
      if (state.paletteQuery.length > 0) {
        return {
          state: Object.freeze({ ...state, paletteQuery: "", paletteIndex: 0 }),
          effects: [],
        };
      }
      return {
        state: Object.freeze({
          ...state,
          paletteOpen: false,
          paletteQuery: "",
          focusedSurface: "Primary",
        }),
        effects: [],
      };
    case "ui.palette.close":
      return {
        state: Object.freeze({
          ...state,
          paletteOpen: false,
          paletteQuery: "",
          focusedSurface: "Primary",
          pendingConfirm: null,
        }),
        effects: [],
      };
    case "ui.palette.move.prev":
    case "ui.palette.move.next": {
      // Navigation walks the same (query-filtered) entry list activation
      // uses, so the highlighted row and the Enter target can never diverge.
      // Moving drops any staged confirm.
      const entries = activePaletteEntries(state, snapshot, paletteContext);
      const last = Math.max(0, entries.length - 1);
      const delta = command === "ui.palette.move.prev" ? -1 : 1;
      return {
        state: Object.freeze({
          ...state,
          paletteIndex: Math.min(Math.max(0, state.paletteIndex + delta), last),
          pendingConfirm: null,
        }),
        effects: [],
      };
    }
    case "ui.palette.activate": {
      const entries = activePaletteEntries(state, snapshot, paletteContext);
      const entry = entries[state.paletteIndex];
      if (entry === undefined || entry.type === "header") {
        return { state, effects: [] };
      }
      if (entry.type === "view" && entry.status === "available") {
        return {
          state: switchView(
            Object.freeze({
              ...state,
              paletteOpen: false,
              focusedSurface: "Primary",
              pendingConfirm: null,
            }),
            entry.view,
          ),
          effects: [],
        };
      }
      if (entry.type === "affordance") {
        // ICM §30 (landed with the ActionControl increment): a row whose
        // FRESH resolution does not permit invocation is VISIBLE_DISABLED —
        // it never stages and never activates; its reasons stay visible in
        // the row itself. Racing changes the fresh resolution cannot see
        // still reach the gateway, whose canonical revalidation denies
        // (the T23 racing teeth are preserved; the visible/stale cases are
        // refused here, before any drive).
        if (!statusPermitsInvocation(entry.status)) {
          return { state: Object.freeze({ ...state, pendingConfirm: null }), effects: [] };
        }
        // Consequential operations never fire on a single Enter: the first
        // stages a confirm naming the invoke, the second performs it. The
        // staged pair revalidates against the fresh entry list, so a stale
        // confirm can never invoke a different operation.
        const staged = state.pendingConfirm;
        if (
          staged !== null &&
          staged.operation === entry.operation &&
          staged.ref.id === entry.ref.id
        ) {
          return {
            state: Object.freeze({
              ...state,
              paletteOpen: false,
              focusedSurface: "Primary",
              pendingConfirm: null,
            }),
            effects: [{ type: "invoke_affordance", operation: entry.operation, ref: entry.ref }],
          };
        }
        return {
          state: Object.freeze({
            ...state,
            pendingConfirm: Object.freeze({ operation: entry.operation, ref: entry.ref }),
          }),
          effects: [],
        };
      }
      if (entry.type === "focus") {
        return {
          state: Object.freeze({
            ...state,
            paletteOpen: false,
            focusedSurface: "Primary",
            pendingConfirm: null,
            location: Object.freeze({ ...state.location, focus: entry.ref }),
          }),
          effects: [{ type: "select_focus", ref: entry.ref }],
        };
      }
      return {
        state: Object.freeze({ ...state, pendingConfirm: null }),
        effects: [],
      };
    }
    default:
      return { state, effects: [] };
  }
}

// --- Shell rendering -------------------------------------------------------------

export interface ShellRenderOptions {
  readonly observation: AffordanceObservation;
  readonly session?: AffordanceSessionContext | undefined;
  readonly paymentEstimates?: Readonly<Record<string, PaymentEstimate>> | undefined;
  readonly composer?: RenderedComposer | undefined;
  /**
   * ICM §30 BUSY_SUBMISSION wiring: the host's in-flight `${operation}:
   * ${focus.id}` keys (WorkbenchHost.pendingOperations), threaded through to
   * the Primary view renderer so it can show a control as busy while its
   * invoke is awaiting admission/result. Absent = no invoke in flight.
   */
  readonly pendingOperations?: ReadonlySet<string> | undefined;
}

const KEY_LEGEND_COMPACT_0 =
  "keys: 1/2/3 work/chat/trace · tab surface · up/down move · enter select";
const KEY_LEGEND_COMPACT_1 =
  "keys: h hoist · u unhoist · p palette · e composer · esc dismiss · q quit";
const KEY_LEGEND =
  "keys: 1/2/3 work·chat·trace · tab surface · up/down move+scroll · enter select · h hoist · u unhoist · p palette · e composer · esc dismiss · q quit";

function promptLines(dims: Dimensions): readonly RenderedLine[] {
  return renderBelowMinimumPrompt(dims)
    .split("\n")
    .map((text, index) => ({
      id: `prompt.${index}`,
      spans: [
        ...(index === 0
          ? [{ text: `! ${text}`, token: "attention.required" as const, emphasis: "bold" as const }]
          : [{ text }]),
      ],
    }));
}

/** One Navigator outline row as a rendered line (shared by the docked
 * surface and the Compact drawer so both render placements of the SAME
 * FocusRef rows — DSN-06). */
function outlineRowLine(row: OutlineRow, highlighted: boolean): RenderedLine {
  const glyph = row.token !== undefined ? SEMANTIC_TOKENS[row.token].glyph : "·";
  return {
    id: row.id,
    spans: [
      ...(highlighted
        ? [{ text: "◉ ", token: "focus.ring" as const, emphasis: "bold" as const }]
        : [{ text: "  " }]),
      { text: "  ".repeat(row.depth) },
      { text: `${glyph} `, token: row.token },
      { text: row.label, emphasis: highlighted ? "bold" : "normal" },
      ...(row.alias !== undefined
        ? [
            {
              text: ` ≈ ${row.alias}`,
              token: "structure.neutral" as const,
              emphasis: "dim" as const,
            },
          ]
        : []),
    ],
  };
}

/** Primary tab order: the three required primaries, then provider-gated
 * promotable views (REQ-AFF-004). ONE source for the tab strip render AND
 * pointer span resolution so they can never diverge. */
function primaryTabViews(snapshot: FullWorkbenchSnapshot): readonly ViewId[] {
  const primaries: readonly ViewId[] = ["work", "chat", "trace"];
  const promotable = availableViews(snapshot)
    .filter(
      (availability) =>
        availability.status === "available" && !primaries.includes(availability.view),
    )
    .map((availability) => availability.view);
  return [...primaries, ...promotable];
}

/**
 * Render the whole shell for one snapshot. Pure: snapshot in, render tree
 * out. Geometry floors are honoured exactly (DSN-007).
 */
export function renderShell(
  state: ShellState,
  snapshot: FullWorkbenchSnapshot,
  registry: ShellViewRegistry,
  options: ShellRenderOptions,
): RenderedShell {
  const geo = classifyGeometry(state.dimensions);
  const columns = calculateColumns(state.dimensions);
  const breadcrumb = [...snapshot.horizon_breadcrumb];

  // Clip-priority order (GUI redesign, DESIGN Responsive/Compact: identity,
  // Case, breadcrumb and attention outlive the tail-clip at 80 cols; run and
  // generation are metadata and clip FIRST because they sit last).
  const header: RenderedLine = {
    id: "shell.header",
    spans: [
      { text: "Gauntlet ", emphasis: "bold" },
      {
        text: `· ${state.location.workspace.workspace}/${state.location.workspace.reference} `,
      },
      { text: `· ${breadcrumb.join(" › ")} `, emphasis: "bold" },
      ...(snapshot.attention.length > 0
        ? [
            {
              text: `· ! attention ${snapshot.attention.length} `,
              token: "attention.required" as const,
              emphasis: "bold" as const,
            },
          ]
        : []),
      ...(state.location.workspace.run !== undefined
        ? [{ text: `· run ${state.location.workspace.run} `, emphasis: "dim" as const }]
        : []),
      // 8-char generation prefix; full id stays visible in Inspector PROVENANCE.
      { text: `· gen ${snapshot.generation.slice(0, 8)}`, emphasis: "dim" },
    ],
  };

  if (geo === "BelowMinimum") {
    const prompt = promptLines(state.dimensions);
    return {
      geometryClass: geo,
      columns,
      location: state.location,
      breadcrumb,
      header,
      surfaces: [],
      drawer: null,
      bottom: null,
      attention: [],
      help: prompt,
      palette: null,
      prompt: { lines: prompt, exitKey: "q" },
      motion: state.motion,
    };
  }

  const multi = geo === "Full" || geo === "Wide";
  const surfaces: RenderedSurface[] = [];

  // Navigator — the semantic outline keyed by FocusRef placements.
  if (multi) {
    const rows = navigatorOutline(snapshot);
    const navigatorEmphasis = state.focusedSurface === "Navigator" ? "bold" : "dim";
    const lines: RenderedLine[] = [
      { id: "navigator.title", spans: [{ text: "NAVIGATOR", emphasis: navigatorEmphasis }] },
    ];
    // Viewport follow (V4 fix, REQ-UX-005 visible focus): the window keeps
    // the selection in view with two rows of context; the renderer used to
    // slice from the top, letting the ◉ highlight scroll out of the pane.
    const viewport = Math.max(6, state.dimensions.rows - 10);
    const clamp = Math.max(0, rows.length - viewport);
    let start = Math.min(clamp, Math.max(0, state.navigatorIndex - 2));
    // Keep the tail reachable: when the selection is near the end, pin the
    // window to the last full page.
    if (state.navigatorIndex >= start + viewport) start = Math.min(clamp, state.navigatorIndex - 2);
    if (start > 0) {
      lines.push({
        id: "navigator.scroll.above",
        spans: [{ text: `↑ ${start} row(s) above`, emphasis: "dim" as const }],
      });
    }
    rows.slice(start, start + viewport).forEach((row, windowIndex) => {
      const index = windowIndex + start;
      const highlighted = state.focusedSurface === "Navigator" && index === state.navigatorIndex;
      lines.push(outlineRowLine(row, highlighted));
    });
    if (rows.length <= 1) {
      lines.push({
        id: "navigator.empty",
        spans: [
          {
            text: "Nothing outlined yet. Bind a source and sync a generation to populate the Case.",
            emphasis: "dim",
          },
        ],
      });
    }
    if (start + viewport < rows.length) {
      lines.push({
        id: "navigator.scroll.below",
        spans: [
          {
            text: `↓ ${rows.length - start - viewport} row(s) below`,
            emphasis: "dim" as const,
          },
        ],
      });
    }
    surfaces.push({
      id: "Navigator",
      title: "NAVIGATOR",
      focused: state.focusedSurface === "Navigator",
      width: columns.navigatorWidth,
      lines,
      placements: snapshot.placements.filter((placement) => placement.surface === "Navigator"),
    });
  }

  // Primary — the active View host.
  const renderer = registry[state.location.view];
  const viewContent: ViewContent =
    renderer !== undefined
      ? renderer({
          snapshot,
          observation: options.observation,
          session: options.session,
          paymentEstimates: options.paymentEstimates,
          pendingOperations: options.pendingOperations,
          ...(state.location.view === "trace"
            ? { traceWindow: traceLedgerWindow(state, snapshot) }
            : {}),
        })
      : {
          view: state.location.view,
          question: "unavailable",
          lines: [
            {
              id: "primary.no-renderer",
              spans: [
                {
                  text: `Provider for '${state.location.view}' is present but no renderer is registered.`,
                  emphasis: "dim",
                },
              ],
            },
          ],
          records: [],
        };

  const scrollOffset = state.scroll[state.location.view] ?? 0;
  const primaryEmphasis = state.focusedSurface === "Primary" ? "bold" : "dim";
  // Tab strip (GUI redesign, UESC §7.3): fixed-order primary tabs plus
  // provider-gated promotable tabs (REQ-AFF-004 — an absent provider is no
  // tab, it is absent with a reason reachable via the palette). The ▸ marker
  // rides the ACTIVE tab so activation state survives monochrome and text
  // capture. Tabs are presentation: closing/switching never stops work.
  const tabOrder = primaryTabViews(snapshot);
  const primaryTabs: RenderedLine = {
    id: "primary.tabs",
    spans: [
      { text: "PRIMARY ", emphasis: primaryEmphasis },
      ...tabOrder.map((view, index, all): RenderedSpan => {
        const active = view === state.location.view;
        return {
          text: `${active ? "▸[" : "["}${view.toUpperCase()}]${
            index === all.length - 1 ? "" : " "
          }`,
          ...(active ? { token: "focus.ring" as const, emphasis: "bold" as const } : {}),
        };
      }),
    ],
  };
  const primaryLines: RenderedLine[] = [
    primaryTabs,
    ...(scrollOffset > 0
      ? [
          {
            id: "primary.scroll.anchor",
            spans: [{ text: `↑ ${scrollOffset} line(s) above (scroll anchored)`, emphasis: "dim" }],
          } as RenderedLine,
        ]
      : []),
    ...viewContent.lines.slice(scrollOffset),
  ];
  surfaces.push({
    id: "Primary",
    title: "PRIMARY",
    focused: state.focusedSurface === "Primary",
    width: columns.primaryWidth,
    lines: primaryLines,
    placements: snapshot.placements.filter((placement) => placement.surface === "Primary"),
  });

  // Inspector — sections from the selected Focus.
  const inspectorFocus = inspectorTarget(state, snapshot);
  const inspectorEmphasis = state.focusedSurface === "Inspector" ? "bold" : "dim";
  const inspectorLines: RenderedLine[] = [
    { id: "inspector.title", spans: [{ text: "INSPECTOR", emphasis: inspectorEmphasis }] },
  ];
  if (inspectorFocus === null) {
    inspectorLines.push({
      id: "inspector.empty",
      spans: [{ text: "No Focus selected. Choose an item in the Navigator.", emphasis: "dim" }],
    });
  } else {
    const sections = renderInspector({
      snapshot,
      focus: inspectorFocus,
      observation: options.observation,
      session: options.session,
      paymentEstimates: options.paymentEstimates,
      // The real value-column budget: inspector width minus the 1-cell gutter,
      // the 12-cell label and its gap. Long provenance wraps instead of clipping.
      valueWidth: columns.inspectorWidth - 14,
    });
    for (const [sectionIndex, section] of sections.sections.entries()) {
      if (sectionIndex > 0) {
        inspectorLines.push({
          id: `inspector.gap.${section.id}`,
          spans: [{ text: " " }],
        });
      }
      inspectorLines.push({
        id: `inspector.section.${section.id}`,
        spans: [{ text: `${section.title} `, emphasis: "bold" }],
      });
      for (const line of section.lines) {
        inspectorLines.push(line);
      }
    }
  }
  if (multi) {
    surfaces.push({
      id: "Inspector",
      title: "INSPECTOR",
      focused: state.focusedSurface === "Inspector",
      width: columns.inspectorWidth,
      lines: inspectorLines,
      placements: snapshot.placements.filter((placement) => placement.surface === "Inspector"),
    });
  }

  // Composer strip.
  const composerLines: readonly RenderedLine[] = options.composer?.lines ?? [
    { id: "composer.default", spans: [{ text: "COMPOSER: press e to focus", emphasis: "normal" }] },
  ];
  surfaces.push({
    id: "Composer",
    title: "COMPOSER",
    focused: state.focusedSurface === "Composer",
    width: state.dimensions.cols,
    lines: composerLines,
    placements: [],
  });

  // Case-wide attention summary — visible without stealing focus (REQ-UX-002).
  const attentionLines: RenderedLine[] = snapshot.attention.map((entry) => ({
    id: `shell.attention.${entry.focus.id}`,
    spans: [
      { text: "! ", token: "attention.required", emphasis: "bold" },
      { text: `${entry.reason} `, token: "attention.required" },
      { text: `${entry.focus.kind}:${entry.focus.source_id}` },
    ],
  }));

  // Compact geometry wraps the legend to two short lines so every bound key
  // stays inside 80 cells; the joined text still names the full binding set.
  const legendLines: readonly RenderedLine[] =
    geo === "Compact"
      ? [
          { id: "shell.help.0", spans: [{ text: KEY_LEGEND_COMPACT_0, emphasis: "dim" }] },
          { id: "shell.help.1", spans: [{ text: KEY_LEGEND_COMPACT_1, emphasis: "dim" }] },
        ]
      : [{ id: "shell.help", spans: [{ text: KEY_LEGEND, emphasis: "dim" }] }];
  const help: RenderedLine[] = [
    ...legendLines,
    ...(state.motion === "reduced"
      ? [
          {
            id: "shell.motion",
            spans: [{ text: "reduced motion: static indicators only", emphasis: "dim" }],
          } as RenderedLine,
        ]
      : []),
  ];

  const palette: RenderedPalette | null = state.paletteOpen
    ? (() => {
        const entries = activePaletteEntries(state, snapshot, {
          observation: options.observation,
          session: options.session,
          paymentEstimates: options.paymentEstimates,
        });
        const lines = entries.map((entry, index): RenderedLine => {
          if (entry.type === "header") {
            return {
              id: `palette.header.${index}`,
              spans: [{ text: `  ${entry.label}`, emphasis: "dim" as const }],
            };
          }
          const selected = index === state.paletteIndex;
          const staged =
            entry.type === "affordance" &&
            state.pendingConfirm !== null &&
            state.pendingConfirm.operation === entry.operation &&
            state.pendingConfirm.ref.id === entry.ref.id;
          if (staged && entry.type === "affordance") {
            return {
              id: `palette.${index}`,
              spans: [
                ...(selected ? [{ text: "◉ ", token: "focus.ring" as const }] : [{ text: "  " }]),
                {
                  text: `Invoke ${entry.operation} on ${entry.ref.kind}:${entry.ref.source_id}? Enter confirms, Esc backs out`,
                  token: "attention.required" as const,
                  emphasis: "bold" as const,
                },
              ],
            };
          }
          const description =
            entry.type === "view"
              ? entry.status === "available"
                ? `view ${entry.view}`
                : `view ${entry.view}: ABSENT (${entry.reason ?? "unavailable"})`
              : entry.type === "affordance"
                ? `action ${entry.operation}: ${entry.status}${entry.reasons.length > 0 ? ` (${entry.reasons.join(", ")})` : ""}`
                : `focus ${entry.label}`;
          // ICM §28: every result exposes its semantic command id and, where
          // one exists, the default shortcut that reaches it.
          const identity =
            entry.type === "view"
              ? entry.shortcut !== null
                ? `${entry.commandId} · ${entry.shortcut}`
                : entry.commandId
              : entry.type === "affordance"
                ? entry.operation
                : "nav.focus.select";
          return {
            id: `palette.${index}`,
            spans: [
              ...(selected ? [{ text: "◉ ", token: "focus.ring" as const }] : [{ text: "  " }]),
              { text: description, emphasis: selected ? "bold" : "normal" },
              { text: `  ${identity}`, emphasis: "dim" as const },
            ],
          };
        });
        return { entries, index: state.paletteIndex, lines };
      })()
    : null;

  // Compact side drawer (V2): the Navigator/Inspector as switchable
  // overlays — the same line builders the docked surfaces use (DSN-06
  // placements, never copies).
  let drawer: RenderedShell["drawer"] = null;
  if (geo === "Compact" && state.drawer !== null) {
    if (state.drawer === "Navigator") {
      const rows = navigatorOutline(snapshot);
      const drawerLines: RenderedLine[] = [
        {
          id: "drawer.navigator.title",
          spans: [{ text: "NAVIGATOR (n closes)", emphasis: "bold" }],
        },
        ...rows.map((row, index) => outlineRowLine(row, index === state.navigatorIndex)),
      ];
      drawer = { side: "left", surface: "Navigator", title: "NAVIGATOR", lines: drawerLines };
    } else {
      drawer = {
        side: "right",
        surface: "Inspector",
        title: "INSPECTOR",
        lines: [
          {
            id: "drawer.inspector.title",
            spans: [{ text: "INSPECTOR (i closes)", emphasis: "bold" }],
          },
          ...inspectorLines.slice(1),
        ],
      };
    }
  }

  // Bottom Surface (UESC §7.5): tabs are the AVAILABLE bottom candidates
  // (REQ-AFF-004); at short heights it collapses first (DESIGN Height
  // constraints) and renders null regardless of bottomOpen.
  let bottom: RenderedShell["bottom"] = null;
  if (geo === "Full" || geo === "Wide") {
    if (heightBand(state.dimensions) === "tall") {
      const candidates: readonly ViewId[] = ["evidence", "agents"];
      const tabs = availableViews(snapshot)
        .filter(
          (availability) =>
            availability.status === "available" && candidates.includes(availability.view),
        )
        .map((availability) => availability.view);
      bottom = { open: state.bottomOpen, tabs };
    }
  }

  return {
    geometryClass: geo,
    columns,
    location: state.location,
    breadcrumb,
    header,
    surfaces,
    drawer,
    bottom,
    attention: attentionLines,
    help,
    palette,
    prompt: null,
    motion: state.motion,
  };
}

/** True when the rendered shell contains any full-motion indicator frame. */
export function containsMotionFrames(rendered: RenderedShell): boolean {
  const frames = runningIndicatorFrames("full");
  const allLines = [
    rendered.header,
    ...rendered.surfaces.flatMap((surface) => surface.lines),
    ...rendered.attention,
    ...rendered.help,
    ...(rendered.palette?.lines ?? []),
    ...(rendered.prompt?.lines ?? []),
  ];
  return allLines.some((line) => frames.some((frame) => lineText(line).includes(frame)));
}

// --- Session ---------------------------------------------------------------------

/** A wired shell: presentation state + composer + effect dispatch. */
export class ShellSession {
  private shell: ShellState;
  private composer: ComposerState;
  private readonly interactions: ShellInteractions;
  private readonly registry: ShellViewRegistry;

  constructor(
    host: ShellHostConfig,
    dims: Dimensions,
    interactions: ShellInteractions = {},
    registry: ShellViewRegistry = defaultViewRegistry(),
  ) {
    assertNoRoadmapSurfaces(registry);
    this.shell = createShellState(host, dims);
    this.composer = createComposerState();
    this.interactions = interactions;
    this.registry = registry;
  }

  get state(): ShellState {
    return this.shell;
  }

  get composerState(): ComposerState {
    return this.composer;
  }

  /** Resize only moves the geometry; semantic location and work are untouched. */
  resize(dims: Dimensions): void {
    this.shell = Object.freeze({ ...this.shell, dimensions: dims });
  }

  /** Commit a reducer result: location changes join the back/forward
   * history (ICM §6.6); history commands themselves never push. */
  private commit(
    before: SemanticLocation,
    result: { state: ShellState; effects: readonly ShellEffect[] },
    viaHistoryCommand: boolean,
  ): { state: ShellState; effects: readonly ShellEffect[] } {
    if (viaHistoryCommand || result.state.location === before) return result;
    return {
      ...result,
      state: Object.freeze({
        ...result.state,
        history: locationPush(result.state.history, result.state.location),
      }),
    };
  }

  /** Route one keypress through takeover → composer → shell precedence. */
  key(raw: string, snapshot: FullWorkbenchSnapshot, paletteContext: PaletteContext = {}): void {
    const geo = classifyGeometry(this.shell.dimensions);

    if (geo === "BelowMinimum") {
      const result = handleKey(this.shell, snapshot, raw);
      this.shell = result.state;
      applyShellEffects(result.effects, this.interactions);
      return;
    }

    // An active takeover OWNS the interaction (REQ-TUI-004).
    if (this.composer.takeover !== null) {
      if (raw === "escape") {
        this.composer = composerDismissTakeover(this.composer);
        this.shell = Object.freeze({ ...this.shell, focusedSurface: "Primary" });
        return;
      }
      if (raw === "tab" || raw === "shift+tab") {
        return;
      }
      const { state, submitted } = composeKeypress(this.composer, raw);
      this.composer = state;
      if (submitted !== null) {
        this.interactions.submitComposer?.(submitted.mode, submitted.destination, submitted.draft);
      }
      if (this.composer.takeover === null) {
        // The takeover completed: normal composition (and focus) returns.
        this.shell = Object.freeze({ ...this.shell, focusedSurface: "Primary" });
      }
      return;
    }

    if (this.shell.focusedSurface === "Composer") {
      if (raw === "escape" || raw === "tab" || raw === "shift+tab") {
        const result = handleKey(this.shell, snapshot, raw);
        this.shell = result.state;
        applyShellEffects(result.effects, this.interactions);
        return;
      }
      const { state, submitted } = composeKeypress(this.composer, raw);
      this.composer = state;
      if (submitted !== null) {
        this.interactions.submitComposer?.(submitted.mode, submitted.destination, submitted.draft);
      }
      return;
    }

    const result = this.commit(
      this.shell.location,
      handleKey(this.shell, snapshot, raw, paletteContext),
      raw === "[" || raw === "]",
    );
    this.shell = result.state;
    applyShellEffects(result.effects, this.interactions);
  }

  /** Pointer gestures route through the same reducer edge as keys. */
  pointer(
    gesture: PointerGesture,
    snapshot: FullWorkbenchSnapshot,
    paletteContext: PaletteContext = {},
  ): void {
    const result = this.commit(
      this.shell.location,
      handlePointer(this.shell, snapshot, gesture, paletteContext),
      false,
    );
    this.shell = result.state;
    applyShellEffects(result.effects, this.interactions);
  }

  /** Pasted data is input text, never a keyboard command or submission. */
  paste(text: string): void {
    if (this.shell.focusedSurface !== "Composer") return;
    if (this.composer.takeover !== null) {
      this.composer = Object.freeze({
        ...this.composer,
        takeoverInput: this.composer.takeoverInput + text,
      });
    } else {
      this.composer = Object.freeze({
        ...this.composer,
        draft: this.composer.draft + text,
        notice: null,
      });
    }
  }

  /** Host admits one takeover request (approval/question/recovery/kernel). */
  beginTakeover(request: TakeoverRequest): void {
    this.composer = composerBeginTakeover(this.composer, request);
    if (this.composer.takeover !== null && this.composer.takeover.requestId === request.requestId) {
      // The takeover owns the interaction: composer takes keyboard focus.
      this.shell = Object.freeze({ ...this.shell, focusedSurface: "Composer" });
    }
  }

  completeTakeover(requestId: string, canonical?: ComposerReplacement | undefined): void {
    this.composer = composerCompleteTakeover(this.composer, requestId, canonical);
    if (this.composer.takeover === null) {
      this.shell = Object.freeze({ ...this.shell, focusedSurface: "Primary" });
    }
  }

  expireTakeovers(nowMs: number): void {
    this.composer = composerExpireTakeovers(this.composer, nowMs);
  }

  /** Render the full shell for one snapshot; consumes it read-only. */
  render(snapshot: FullWorkbenchSnapshot, options: ShellRenderOptions): RenderedShell {
    this.shell = reconcileTrace(this.shell, snapshot);
    return renderShell(this.shell, snapshot, this.registry, {
      ...options,
      composer: composerRender(this.composer),
    });
  }
}

/**
 * Fold snapshot trace records into the ledger machine (UESC §16.2). Pure.
 * Tail-append when the stream's last known id still exists; re-hydrate when
 * history was reset (generation change) so stale ids never linger. Gauntlet
 * execution history is source "gauntlet"; RealityTrace records join through
 * their own future integration lane, never merged here.
 */
export function reconcileTrace(state: ShellState, snapshot: FullWorkbenchSnapshot): ShellState {
  const records = snapshot.trace_records;
  if (records.length === 0 && state.trace.records.length === 0) return state;
  if (records.length === 0) {
    return Object.freeze({ ...state, trace: streamHydrated(state.trace, []) });
  }
  if (state.trace.records.length === 0) {
    return Object.freeze({
      ...state,
      trace: streamHydrated(state.trace, toStreamRecords(records)),
    });
  }
  const lastKnown = state.trace.records[state.trace.records.length - 1]?.id;
  const index = records.findIndex((record) => record.trace_id === lastKnown);
  if (index < 0) {
    return Object.freeze({
      ...state,
      trace: streamHydrated(state.trace, toStreamRecords(records)),
    });
  }
  const fresh = records.slice(index + 1);
  if (fresh.length === 0) return state;
  return Object.freeze({ ...state, trace: streamAppend(state.trace, toStreamRecords(fresh)) });
}

function toStreamRecords(records: readonly FullWorkbenchSnapshot["trace_records"][number][]) {
  return records.map((record) => ({
    id: record.trace_id,
    source: "gauntlet" as const,
    timestampMs: record.timestamp_ms,
  }));
}
