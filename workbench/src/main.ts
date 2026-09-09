/**
 * Gauntlet workbench terminal entry — the release form's runnable host loop
 * (plan T20, REQ-BUN-005, REQ-UX-005/006; evidence T20/release-qualification).
 *
 * What this entry IS:
 *   - The REAL shell stack driven from a real terminal: a WorkbenchModel,
 *     a ShellSession, and the structural render tree rendered as text into
 *     the host terminal. Keyboard-only (REQ-UX-005): every move is a key.
 *   - A COLD START, honestly (REQ-UX-001): no authoritative source is
 *     contacted at startup and no work is launched. The canonical observation
 *     is "not yet observed" (reachable: false), so affordances render their
 *     honest unavailable reasons with next steps — core inspection stays
 *     usable without any provider (REQ-UX-006). Binding a live endpoint is
 *     the host loop's next step, not this entry's silent invention.
 *   - Configuration-driven (REQ-CONFIG-005): `resolveConfig` classifies the
 *     transport (local loopback vs remote) from the documented GAUNTLET
 *     environment; the classification is reported on stderr, never rendered
 *     into canonical surfaces.
 *
 * What this entry is NOT:
 *   - NOT an OpenTUI React render loop. The structural ShellSession render is
 *     the workbench's renderer-independent contract (T12); the embedded
 *     OpenTUI viewport wiring remains T18+ territory (see T13 evidence).
 *   - NOT a canonical authority. Quit restores terminal modes; nothing is
 *     admitted from here (read-only inspection slice, REQ-UX-007).
 *
 * Non-TTY stdin renders ONE frame at the current size and exits 0 — the
 * release manifest uses this to smoke the built artifact deterministically.
 */

import type { AffordanceObservation } from "./affordance";
import { resolveConfig } from "./config";
import type { Dimensions } from "./geometry";
import { classifyGeometry } from "./geometry";
import { renderInspector } from "./inspector";
import type { RenderedLine } from "./render-tree";
import { ShellSession } from "./shell";
import type { FullWorkbenchSnapshot } from "./workbench-model";
import { WorkbenchModel } from "./workbench-model";

// --- Host terminal state -----------------------------------------------------

let rawModeEnabled = false;

function enableRawMode(): void {
  const stdin = process.stdin as { isTTY?: boolean; setRawMode?: (mode: boolean) => void };
  if (stdin.isTTY === true && typeof stdin.setRawMode === "function") {
    stdin.setRawMode(true);
    rawModeEnabled = true;
  }
}

function restoreTerminal(): void {
  const stdin = process.stdin as { setRawMode?: (mode: boolean) => void };
  if (rawModeEnabled && typeof stdin.setRawMode === "function") {
    stdin.setRawMode(false);
    rawModeEnabled = false;
  }
  process.stdout.write("\x1b[?25h"); // show cursor
}

function currentDimensions(): Dimensions {
  const cols = process.stdout.columns;
  const rows = process.stdout.rows;
  if (typeof cols === "number" && typeof rows === "number") {
    return { cols: Math.max(20, cols), rows: Math.max(6, rows) };
  }
  return { cols: 80, rows: 24 };
}

// --- The cold, honest observation (nothing has been observed yet) ------------

function coldObservation(): AffordanceObservation {
  return {
    reachable: false,
    degraded: false,
    units: [],
    open_escalations: [],
    budget_remaining: null,
    governance_denied_units: [],
    authority_revoked_units: [],
    capabilities: [],
  };
}

// --- Frame rendering (deliberate viewport; this process owns the host TTY) ---

function lineText(line: RenderedLine): string {
  return line.spans.map((span) => span.text).join("");
}

function fit(text: string, width: number): string {
  return text.length <= width ? text.padEnd(width) : `${text.slice(0, Math.max(0, width - 1))}…`;
}

function renderFrame(snapshot: FullWorkbenchSnapshot, session: ShellSession): string {
  const dims = currentDimensions();
  const { cols: width, rows: height } = dims;
  const geo = classifyGeometry(dims);
  const observation = coldObservation();
  const rendered = session.render(snapshot, { observation });

  const rows: string[] = [fit(lineText(rendered.header), width)];

  if (rendered.prompt !== null) {
    // Below-minimum: the recoverable prompt IS the frame (DSN-007).
    rows.push("");
    for (const line of rendered.prompt.lines) {
      rows.push(fit(lineText(line), width));
    }
    rows.push("");
    rows.push(fit(`[geometry: ${geo}]`, width));
    return rows.slice(0, height).join("\n");
  }

  const navigatorSurface = rendered.surfaces.find((surface) => surface.id === "Navigator");
  const primarySurface = rendered.surfaces.find((surface) => surface.id === "Primary");
  const inspectorSurface = rendered.surfaces.find((surface) => surface.id === "Inspector");
  const composerSurface = rendered.surfaces.find((surface) => surface.id === "Composer");
  const composerStrip = `${composerSurface?.focused ? "▸" : " "} COMPOSER — press e to focus · q quits`;

  if (navigatorSurface !== undefined && inspectorSurface !== undefined) {
    // Full three-surface layout, side by side.
    const navWidth = 30;
    const insWidth = width >= 90 ? width - 30 - Math.max(20, width - 90) : 20;
    const priWidth = Math.max(20, width - navWidth - insWidth);

    const navLines = navigatorSurface.lines.map(lineText);
    const priLines = (primarySurface?.lines ?? []).map(lineText);
    // Inspector sections render through the live renderInspector wiring
    // (T20 lens gate included).
    const focus = snapshot.focus;
    const insLines: string[] = [];
    if (focus !== null) {
      const sections = renderInspector({ snapshot, focus, observation });
      for (const section of sections.sections) {
        insLines.push(section.title);
        for (const line of section.lines) {
          insLines.push(`  ${lineText(line)}`);
        }
      }
    } else {
      insLines.push("No Focus selected");
    }

    const columns: { title: string; lines: string[]; width: number }[] = [
      {
        title: `${navigatorSurface.focused ? "▸" : " "} NAVIGATOR`,
        lines: navLines,
        width: navWidth,
      },
      {
        title: `${primarySurface?.focused ? "▸" : " "} PRIMARY`,
        lines: priLines,
        width: priWidth,
      },
      {
        title: `${inspectorSurface.focused ? "▸" : " "} INSPECTOR`,
        lines: insLines,
        width: insWidth,
      },
    ];

    rows.push(
      fit(columns.map((column) => fit(column.title, column.width).trimEnd()).join(" | "), width),
    );
    const bodyRows = Math.max(4, height - 5);
    for (let row = 0; row < bodyRows; row += 1) {
      rows.push(
        fit(columns.map((column) => fit(column.lines[row] ?? "", column.width)).join("|"), width),
      );
    }
    rows.push(fit(composerStrip, width));
    return rows.slice(0, height).join("\n");
  }

  // Compact: the one primary pane and the composer strip (DSN-007).
  rows.push("");
  if (primarySurface !== undefined) {
    rows.push(fit(`${primarySurface.focused ? "▸" : " "} ${primarySurface.title}`, width));
    for (const line of primarySurface.lines.slice(0, Math.max(1, height - 5))) {
      rows.push(fit(lineText(line), width));
    }
  }
  rows.push(fit(composerStrip, width));
  return rows.slice(0, height).join("\n");
}

// --- Keyboard decoding (raw bytes → shell keys) -------------------------------

const KEY_MAP: Readonly<Record<string, string>> = Object.freeze({
  "\t": "tab",
  "\r": "enter",
  "\n": "enter",
  "\x7f": "backspace",
  "\x08": "backspace",
  "\x1b[A": "up",
  "\x1b[B": "down",
  "\x1b[C": "right",
  "\x1b[D": "left",
  "\x1b": "escape",
});

function isPartialEscapeSuffix(text: string): boolean {
  // A chunk may end mid-sequence: ESC or ESC [ alone waits for the rest.
  return text.endsWith("\x1b") || text.endsWith("\x1b[");
}

// --- Entry -------------------------------------------------------------------

async function run(): Promise<number> {
  const config = resolveConfig({});
  // Transport classification is a support fact for the operator (stderr),
  // never canonical content.
  process.stderr.write(
    `gauntlet-workbench: transport=${config.transport} endpoint=${config.gauntlet_endpoint}\n`,
  );

  const model = new WorkbenchModel();
  model.startGeneration("gen-local-1");
  model.markReady("gen-local-1");

  let exitCode: number | null = null;
  const session = new ShellSession(
    { workspace: { workspace: "gauntlet", reference: "unbound" } },
    currentDimensions(),
    {
      quit: () => {
        exitCode = 0;
      },
    },
  );

  const draw = (): void => {
    process.stdout.write(`\x1b[2J\x1b[H${renderFrame(model.getSnapshot(), session)}\n`);
  };

  if (process.stdin.isTTY !== true) {
    // Non-TTY: one deterministic frame, then exit (release smoke path).
    draw();
    model.close();
    return 0;
  }

  enableRawMode();
  process.stdout.write("\x1b[?25l"); // hide cursor
  draw();

  process.on("SIGWINCH", () => {
    session.resize(currentDimensions());
    draw();
  });

  const onKey = (raw: string): void => {
    if (raw === "\x03") {
      exitCode = 0; // Ctrl-C is the visible exit chord (REQ-UX-006)
      return;
    }
    const key = KEY_MAP[raw] ?? raw;
    if (key.length > 0) {
      session.key(key, model.getSnapshot());
      draw();
    }
  };

  let buffer = "";
  process.stdin.setEncoding("utf8");
  process.stdin.on("data", (chunk: string) => {
    buffer += chunk;
    while (buffer.length > 0 && exitCode === null) {
      if (isPartialEscapeSuffix(buffer)) {
        return; // wait for the rest of the sequence
      }
      const three = buffer.slice(0, 3);
      const first = buffer[0] ?? "";
      const mapped = KEY_MAP[three] ?? KEY_MAP[first];
      const token = mapped ?? first;
      const consumed = mapped !== undefined ? (KEY_MAP[three] !== undefined ? 3 : 1) : 1;
      buffer = buffer.slice(consumed);
      if (token.length > 0) {
        onKey(token);
      }
    }
  });

  while (exitCode === null) {
    await Bun.sleep(20);
  }

  process.stdin.removeAllListeners("data");
  restoreTerminal();
  model.close();
  return exitCode;
}

process.exit(await run());
