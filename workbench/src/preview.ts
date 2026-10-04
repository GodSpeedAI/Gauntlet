/**
 * Early rendered read-only Gauntlet operator journey.
 * Implements T00A vertical slice over actual gauntlet serve HTTP read surface.
 * Conforms to REQ-UX-001..007, DSN-002, DSN-006, DSN-007.
 */

import type { RuntimeSnapshot, RunView, UnitView } from "./client";
import {
  classifyGeometry,
  clipCell,
  type Dimensions,
  PREVIEW_COMPACT_COLUMNS,
  PREVIEW_FULL_COLUMNS,
  renderBelowMinimumPrompt,
} from "./geometry";
import { formatStatus } from "./theme";

export interface PreviewState {
  readonly dimensions: Dimensions;
  readonly snapshot: RuntimeSnapshot | null;
  readonly runView: RunView | null;
  readonly unitView: UnitView | null;
  readonly selectedUnitIndex: number;
  readonly activeSurface: "Navigator" | "Primary" | "Inspector";
  readonly compactTab: "Work" | "Escalations" | "Inspector";
  readonly focusedUnitId: string | null;
  readonly isInspecting: boolean;
  readonly lastActionMessage: string | null;
}

export function createInitialState(dims: Dimensions = { cols: 120, rows: 40 }): PreviewState {
  return {
    dimensions: dims,
    snapshot: null,
    runView: null,
    unitView: null,
    selectedUnitIndex: 0,
    activeSurface: "Primary",
    compactTab: "Work",
    focusedUnitId: null,
    isInspecting: false,
    lastActionMessage: null,
  };
}

export type PreviewAction =
  | { type: "RESIZE"; dimensions: Dimensions }
  | { type: "LOAD_SNAPSHOT"; snapshot: RuntimeSnapshot }
  | { type: "LOAD_RUN"; runView: RunView }
  | { type: "LOAD_UNIT"; unitView: UnitView }
  | { type: "NAVIGATE_DOWN" }
  | { type: "NAVIGATE_UP" }
  | { type: "SWITCH_SURFACE" }
  | { type: "SET_COMPACT_TAB"; tab: "Work" | "Escalations" | "Inspector" }
  | { type: "INSPECT_FOCUSED" }
  | { type: "RETURN_TO_LIST" }
  | { type: "ATTEMPT_MUTATION"; actionName: string };

export function previewReducer(state: PreviewState, action: PreviewAction): PreviewState {
  switch (action.type) {
    case "RESIZE":
      return { ...state, dimensions: action.dimensions };

    case "LOAD_SNAPSHOT": {
      const units = action.snapshot.units;
      const focusedId = units[0]?.unit_id ?? null;
      return {
        ...state,
        snapshot: action.snapshot,
        selectedUnitIndex: 0,
        focusedUnitId: focusedId,
      };
    }

    case "LOAD_RUN":
      return {
        ...state,
        runView: action.runView,
      };

    case "LOAD_UNIT":
      return {
        ...state,
        unitView: action.unitView,
      };

    case "NAVIGATE_DOWN": {
      const total = state.snapshot?.units.length ?? 0;
      if (total === 0) return state;
      const nextIndex = Math.min(total - 1, state.selectedUnitIndex + 1);
      const nextId = state.snapshot?.units[nextIndex]?.unit_id ?? null;
      return {
        ...state,
        selectedUnitIndex: nextIndex,
        focusedUnitId: nextId,
      };
    }

    case "NAVIGATE_UP": {
      const total = state.snapshot?.units.length ?? 0;
      if (total === 0) return state;
      const prevIndex = Math.max(0, state.selectedUnitIndex - 1);
      const prevId = state.snapshot?.units[prevIndex]?.unit_id ?? null;
      return {
        ...state,
        selectedUnitIndex: prevIndex,
        focusedUnitId: prevId,
      };
    }

    case "SWITCH_SURFACE": {
      const cycle: Record<PreviewState["activeSurface"], PreviewState["activeSurface"]> = {
        Navigator: "Primary",
        Primary: "Inspector",
        Inspector: "Navigator",
      };
      return { ...state, activeSurface: cycle[state.activeSurface] };
    }

    case "SET_COMPACT_TAB":
      return { ...state, compactTab: action.tab };

    case "INSPECT_FOCUSED":
      return {
        ...state,
        isInspecting: true,
        activeSurface: "Inspector",
        compactTab: "Inspector",
      };

    case "RETURN_TO_LIST":
      return {
        ...state,
        isInspecting: false,
        activeSurface: "Primary",
        compactTab: "Work",
      };

    case "ATTEMPT_MUTATION":
      // Explicit read-only contract: all mutation intents are refused and reported as read-only preview
      return {
        ...state,
        lastActionMessage: `[READ-ONLY PREVIEW] Action '${action.actionName}' is disabled in T00A preview mode.`,
      };
  }
}

/**
 * Pure renderer producing the text matrix for the operator journey.
 */
export function renderPreview(state: PreviewState): string {
  const geo = classifyGeometry(state.dimensions);

  // 1. BelowMinimum prompt
  if (geo === "BelowMinimum") {
    return renderBelowMinimumPrompt(state.dimensions);
  }

  // 2. Disconnected / Empty state
  if (!state.snapshot || state.snapshot.status === "disconnected") {
    return [
      "╔════════════════════════════════════════════════════════════════════════════════╗",
      "║ GAUNTLET PRIME WORKBENCH: [PREVIEW / READ-ONLY]                               ║",
      "╚════════════════════════════════════════════════════════════════════════════════╝",
      "",
      "  Status: DISCONNECTED (127.0.0.1:4000 unreachable)",
      "",
      "  Next step:",
      "    Run: GAUNTLET_API_BIND=127.0.0.1:4000 gauntlet serve",
      "",
      "  Controls: [q] Quit  [r] Retry connection",
    ].join("\n");
  }

  if (state.snapshot.status === "empty" || state.snapshot.units.length === 0) {
    return [
      "╔════════════════════════════════════════════════════════════════════════════════╗",
      "║ GAUNTLET PRIME WORKBENCH: [PREVIEW / READ-ONLY]                               ║",
      "╚════════════════════════════════════════════════════════════════════════════════╝",
      "",
      `  Run: ${state.snapshot.run ?? "No active run"}`,
      "  Status: EMPTY WORKSPACE (0 units found)",
      "",
      "  Next step:",
      "    Execute a gauntlet run to materialize verification records and units.",
      "",
      "  Controls: [q] Quit  [r] Refresh",
    ].join("\n");
  }

  // 3. Normal layout: Full (120x40) or Compact (80x24)
  if (geo === "Full" || geo === "Wide") {
    return renderFullLayout(state);
  }

  return renderCompactLayout(state);
}

function renderFullLayout(state: PreviewState): string {
  const lines: string[] = [];
  const runName = state.snapshot?.run ?? "run-0";
  const runDerivation = state.snapshot?.run_derivation ?? "direct";

  lines.push(
    "┌─ GAUNTLET WORKBENCH ────────────────────────────────────────────────────────── [PREVIEW / READ-ONLY] ─ 120x40 FULL ─┐",
  );
  const runHeader = clipCell(
    `│ Run: ${runName} (${runDerivation}) | Escalations: ${state.snapshot?.escalated.length ?? 0} open | Focus: [${state.focusedUnitId ?? "none"}]`,
    PREVIEW_FULL_COLUMNS.runHeader,
  );
  lines.push(`${runHeader}│`);
  lines.push(
    "├────────────────────────────┬────────────────────────────────────────────┬───────────────────────────────────────────┤",
  );
  lines.push(
    "│ NAVIGATOR                  │ PRIMARY (WORK UNITS)                       │ INSPECTOR                                 │",
  );
  lines.push(
    "├────────────────────────────┼────────────────────────────────────────────┼───────────────────────────────────────────┤",
  );

  const units = state.snapshot?.units ?? [];
  const escalated = state.snapshot?.escalated ?? [];
  const focusedUnit = units[state.selectedUnitIndex] ?? null;

  const maxRows = 30;
  for (let r = 0; r < maxRows; r++) {
    // Col 1: Navigator (Escalations first per DSN-006 / spec §20.2)
    let col1 = "";
    if (r === 0) {
      col1 = `! Escalations (${escalated.length})`;
    } else if (r <= escalated.length) {
      const ticket = escalated[r - 1] ?? "";
      col1 = `  • ${ticket}`;
    } else if (r === escalated.length + 1) {
      col1 = "─ Runs ──────────────────";
    } else if (r === escalated.length + 2) {
      col1 = `  ▶ ${runName}`;
    }

    // Col 2: Primary Work Units
    let col2 = "";
    if (r < units.length) {
      const u = units[r];
      if (u) {
        const isSelected = r === state.selectedUnitIndex;
        const pointer = isSelected ? "▶ " : "  ";
        const st = formatStatus(u.status);
        const tier = u.tier ? `[${u.tier}] ` : "";
        col2 = `${pointer}${tier}${u.unit_id} [${st.glyph} ${st.label}]`;
      }
    }

    // Col 3: Inspector
    let col3 = "";
    if (focusedUnit) {
      if (r === 0) {
        col3 = `Unit: ${focusedUnit.unit_id}`;
      } else if (r === 1) {
        const st = formatStatus(focusedUnit.status);
        col3 = `Status: ${st.glyph} ${st.label}`;
      } else if (r === 2) {
        col3 = `Tier: ${focusedUnit.tier ?? "default"}`;
      } else if (r === 3) {
        col3 = "─ Verifications ────────";
      } else if (r === 4) {
        if (state.unitView && state.unitView.verifications.length > 0) {
          const v = state.unitView.verifications[0];
          col3 = v ? `• Ev: ${v.evidence || "none"}` : "• (none)";
        } else {
          col3 = "• Ev: (metadata preview)";
        }
      } else if (r === 5) {
        col3 = "─ Denial Explanation ───";
      } else if (r === 6) {
        // Explicitly labeled unavailable per T00A requirement
        col3 = "Explanation: unavailable";
      } else if (r === 7) {
        col3 = "(T01 read contract required)";
      }
    }

    const pad1 = clipCell(col1, PREVIEW_FULL_COLUMNS.nav);
    const pad2 = clipCell(col2, PREVIEW_FULL_COLUMNS.primary);
    const pad3 = clipCell(col3, PREVIEW_FULL_COLUMNS.inspector);
    lines.push(`│ ${pad1} │ ${pad2} │ ${pad3} │`);
  }

  lines.push(
    "├────────────────────────────┴────────────────────────────────────────────┴───────────────────────────────────────────┤",
  );
  lines.push(
    "│ Controls: [↑/↓] Select Unit  [Enter] Inspect  [Tab] Cycle Surface  [Esc] Return  [q] Quit                          │",
  );
  if (state.lastActionMessage) {
    lines.push(`│ Note: ${clipCell(state.lastActionMessage, PREVIEW_FULL_COLUMNS.note)} │`);
  }
  lines.push(
    "└─────────────────────────────────────────────────────────────────────────────────────────────────────────────────────┘",
  );

  return lines.join("\n");
}

function renderCompactLayout(state: PreviewState): string {
  const lines: string[] = [];
  const runName = state.snapshot?.run ?? "run-0";
  const units = state.snapshot?.units ?? [];
  const focusedUnit = units[state.selectedUnitIndex] ?? null;

  lines.push("┌─ GAUNTLET WORKBENCH ──────────── [PREVIEW / READ-ONLY] ─ 80x24 COMPACT ─┐");
  lines.push(
    `│ Run: ${runName} | Focus: [${state.focusedUnitId ?? "none"}] | Escalations: ${state.snapshot?.escalated.length ?? 0}`,
  );
  lines.push("├────────────────────────────────────────────────────────────────────────┤");

  // Tab strip
  const t1 = state.compactTab === "Work" ? "[1: WORK (★)]" : " 1: Work ";
  const t2 = state.compactTab === "Escalations" ? "[2: ESCALATIONS (★)]" : " 2: Escalations ";
  const t3 = state.compactTab === "Inspector" ? "[3: INSPECTOR (★)]" : " 3: Inspector ";
  lines.push(`│ TABS: ${t1}  ${t2}  ${t3}`);
  lines.push("├────────────────────────────────────────────────────────────────────────┤");

  if (state.compactTab === "Work") {
    lines.push("│ UNITS IN WORKSPACE:                                                    │");
    const maxRows = 14;
    for (let i = 0; i < maxRows; i++) {
      if (i < units.length) {
        const u = units[i];
        if (u) {
          const isSelected = i === state.selectedUnitIndex;
          const pointer = isSelected ? "▶ " : "  ";
          const st = formatStatus(u.status);
          const line = `${pointer}${u.unit_id} [${st.glyph} ${st.label}] ${u.tier ? `(${u.tier})` : ""}`;
          lines.push(`│ ${clipCell(line, PREVIEW_COMPACT_COLUMNS.unit)} │`);
        }
      } else {
        lines.push("│                                                                        │");
      }
    }
  } else if (state.compactTab === "Escalations") {
    lines.push("│ ESCALATION INBOX:                                                      │");
    const escalated = state.snapshot?.escalated ?? [];
    for (let i = 0; i < 14; i++) {
      if (i < escalated.length) {
        const t = escalated[i];
        lines.push(`│   • Ticket: ${clipCell(String(t), PREVIEW_COMPACT_COLUMNS.ticket)} │`);
      } else {
        lines.push("│                                                                        │");
      }
    }
  } else {
    // Inspector Tab
    lines.push("│ UNIT DETAILS:                                                          │");
    if (focusedUnit) {
      const st = formatStatus(focusedUnit.status);
      lines.push(
        `│ Unit ID:      ${clipCell(focusedUnit.unit_id, PREVIEW_COMPACT_COLUMNS.fieldId)} │`,
      );
      lines.push(
        `│ Status:       ${st.glyph} ${clipCell(st.label, PREVIEW_COMPACT_COLUMNS.fieldStatus)} │`,
      );
      lines.push(
        `│ Tier:         ${clipCell(focusedUnit.tier ?? "default", PREVIEW_COMPACT_COLUMNS.fieldId)} │`,
      );
      lines.push("│ Evidence:     (metadata preview mode)                                  │");
      lines.push("│ Explanation:  unavailable (T01 read contract required)                 │");
    } else {
      lines.push("│ No unit focused. Use [1] Work to select a unit.                        │");
    }
    for (let i = 0; i < 9; i++) {
      lines.push("│                                                                        │");
    }
  }

  lines.push("├────────────────────────────────────────────────────────────────────────┤");
  lines.push("│ Controls: [↑/↓] Select  [Enter] Inspect  [Esc] Back  [1/2/3] Tab  [q] Quit │");
  lines.push("└────────────────────────────────────────────────────────────────────────┘");

  return lines.join("\n");
}
