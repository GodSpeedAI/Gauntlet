import { describe, expect, it } from "bun:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { DEFAULT_CONFIG } from "./config";

/**
 * T20 real-PTY operator journeys (plan T20; REQ-UX-005/007; the REQ-UX-005
 * real-rendering evidence T12 deferred; prereg A1/P2/D3).
 *
 * GATED behind GAUNTLET_T20_REAL_PTY=1 (T13's GAUNTLET_T13_REAL_PTY pattern):
 * the default suite stays deterministic; with the gate set, the REAL
 * workbench entry (`src/main.ts`, the release form's host loop) is driven in
 * a REAL Bun PTY at the three declared geometries — 120x40 (full
 * three-surface layout), 80x24 (one primary pane), and below-minimum
 * (recoverable prompt) — with KEYBOARD-ONLY journey steps and assertions on
 * the RENDERED output. Rendered frame captures are written to the task's
 * evidence directory when the gate is set.
 *
 * The terminal-restart/reattach leg runs in the default suite (see
 * ssh-target.test.ts); this file is the live-render leg.
 */

const REAL_PTY = process.env["GAUNTLET_T20_REAL_PTY"] === "1";
const WORKBENCH_DIR = join(import.meta.dir, "..");
const EVIDENCE_FRAMES = join(
  import.meta.dir,
  "..",
  "..",
  ".agents",
  "reports",
  "gauntlet-tui-prime-workbench",
  "evidence",
  "T20",
  "frames",
);

interface PtySession {
  readonly pid: number;
  output(): string;
  write(data: string): void;
  resize(cols: number, rows: number): void;
  /** Byte offset into output() at the moment of the most recent resize() call (0 if none yet). */
  resizeOffset(): number;
  exited(): Promise<number | "TIMEOUT">;
  kill(): void;
}

/**
 * Spawn the REAL workbench entry in a REAL Bun PTY.
 *
 * `env` overrides merge over two isolating defaults:
 *   - GAUNTLET_WORKBENCH_PROJECTION_STORE=":memory:" (workbench-host.ts's
 *     supported non-file value, already used by cold-rebuild.test.ts and
 *     others) — without it every spawn shares and mutates the checked-in
 *     workbench/.gauntlet/projections.db.
 *   - GAUNTLET_WORKBENCH_ENDPOINT="http://127.0.0.1:1" (the unreachable-port
 *     sentinel already used by secret-leakage.test.ts) — matches this
 *     release entry's own documented posture (tui-preflight.sh: "release
 *     entry is a cold-start host loop: no live gauntlet serve endpoint
 *     binding", a RECORDED-UNQUALIFIED T20 limitation, not a bug to route
 *     around). Without it, a spawn's default endpoint (127.0.0.1:4000) can
 *     reach whatever real `gauntlet serve` process happens to be running on
 *     the host — real generation syncs land mid-test and any "cold Case"
 *     assertion becomes ambient-state-dependent instead of deterministic.
 *     A test whose whole POINT is exercising a live-synced Case (e.g. the
 *     mouse-wheel journey scrolling real units) passes an explicit
 *     `{ GAUNTLET_WORKBENCH_ENDPOINT: DEFAULT_CONFIG.gauntlet_endpoint }`
 *     override to opt back into the real ambient endpoint — a documented,
 *     deliberate choice for that one test, not the default for this file.
 */
function spawnWorkbench(
  cols: number,
  rows: number,
  env: Record<string, string | undefined> = {},
): PtySession {
  let output = "";
  let resizeOffset = 0;
  const proc = Bun.spawn(["bun", "run", "src/main.ts"], {
    cwd: WORKBENCH_DIR,
    env: {
      ...process.env,
      TERM: "xterm-256color",
      GAUNTLET_WORKBENCH_PROJECTION_STORE: ":memory:",
      GAUNTLET_WORKBENCH_ENDPOINT: "http://127.0.0.1:1",
      ...env,
    },
    terminal: {
      cols,
      rows,
      name: "xterm-256color",
      data: (_terminal, data) => {
        output += new TextDecoder().decode(data);
      },
    },
  });
  const terminal = proc.terminal;
  if (terminal === undefined) {
    proc.kill();
    throw new Error("Bun.spawn returned no terminal for the workbench entry");
  }
  return {
    pid: proc.pid,
    output: () => output,
    write: (data) => terminal.write(data),
    resize: (nextCols, nextRows) => {
      // Mark the boundary BEFORE resizing: OpenTUI's processResize (verified
      // against @opentui/core's non-split-footer path) never emits a fresh
      // \x1b[2J or forces a full repaint on resize, so bytes written before
      // this point are addressed to columns/rows the new geometry may not
      // even have — they are stale from lastFrame()'s perspective even
      // though no clear-screen sequence marks them as such.
      resizeOffset = output.length;
      terminal.resize(nextCols, nextRows);
    },
    resizeOffset: () => resizeOffset,
    exited: () => Promise.race([proc.exited, Bun.sleep(10_000).then(() => "TIMEOUT" as const)]),
    kill: () => proc.kill(),
  };
}

/**
 * Wait until the FRESH last-drawn frame contains the marker (peer MINOR-1:
 * accumulated stream matching let a stale early frame satisfy a LATER
 * transition — the cold view is already [WORK], so the post-"1" wait matched
 * output from before the redraw and the capture caught the [TRACE] frame).
 * Every interaction-driven transition waits on the fresh frame; accumulated
 * output is only consulted where the fact genuinely lives in the stream
 * (exit-mode restoration after the final frame).
 */
async function untilFrame(session: PtySession, marker: string, timeoutMs = 10_000): Promise<void> {
  const deadline = Date.now() + timeoutMs;
  let last = "";
  while (Date.now() < deadline) {
    last = lastFrame(session);
    if (last.includes(marker)) {
      return;
    }
    await Bun.sleep(50);
  }
  throw new Error(
    `PTY last frame never contained '${marker}' within ${String(timeoutMs)}ms; last frame:\n${last.slice(0, 1600)}`,
  );
}

/** Strip ANSI CSI escape sequences by scan (no control characters in a regex). */
function stripAnsi(text: string): string {
  let result = "";
  for (let index = 0; index < text.length; ) {
    const char = text[index];
    if (char === "\x1b" && text[index + 1] === "[") {
      index += 2;
      while (index < text.length && !/[a-zA-Z]/.test(text[index] ?? "")) {
        index += 1;
      }
      index += 1; // consume the final letter
    } else {
      result += char;
      index += 1;
    }
  }
  return result;
}

/**
 * The LAST drawn frame with escape sequences stripped. Isolated from the raw
 * byte log (not a real terminal-grid emulation) by finding the later of: the
 * final clear-screen sequence, or the byte offset of the most recent
 * resize() call. The resize boundary matters because OpenTUI's resize path
 * does not itself emit a fresh clear-screen (verified against
 * @opentui/core's processResize) — without it, content written for the
 * PRE-resize geometry (e.g. a column that no longer exists at the new width)
 * stays in the raw log forever with nothing marking it stale.
 */
function lastFrame(session: PtySession): string {
  const raw = session.output();
  const lastClear = raw.lastIndexOf("\x1b[2J");
  const boundary = Math.max(lastClear >= 0 ? lastClear : 0, session.resizeOffset());
  return stripAnsi(raw.slice(boundary));
}

function captureFrame(name: string, session: PtySession): void {
  if (!REAL_PTY) {
    return;
  }
  mkdirSync(EVIDENCE_FRAMES, { recursive: true });
  writeFileSync(join(EVIDENCE_FRAMES, `${name}.txt`), lastFrame(session));
}

describe("T20 real-PTY operator journeys (gated: GAUNTLET_T20_REAL_PTY=1)", () => {
  it.skipIf(!REAL_PTY)(
    "120x40 FULL: cold start renders the three-surface layout; keyboard-only journey switches surfaces and views; resize to 80x24 and below-minimum behave; q quits with modes restored",
    async () => {
      const session = spawnWorkbench(120, 40);
      try {
        // Cold start: full three-surface layout with the honest empty states.
        await untilFrame(session, "NAVIGATOR");
        expect(lastFrame(session)).toContain("INSPECTOR");
        expect(lastFrame(session)).toContain("PRIMARY");
        expect(lastFrame(session)).toContain("COMPOSER");
        // "canonical surface" is main.ts's workspace reference literal since
        // increment 2a (516f5cb) — this gated real-PTY assertion had drifted
        // (found by the mouse-PTY qualification increment's real run).
        expect(lastFrame(session)).toContain("Gauntlet · gauntlet/canonical surface");
        // REQ-UX-001: a cold start launches no work and says what would move it.
        expect(lastFrame(session)).toContain("No authoritative source has");
        captureFrame("journey-120x40-full-cold", session);

        // Keyboard-only surface cycling: Tab cycles Primary → Inspector →
        // Composer → Navigator; the rendered focus marker follows.
        session.write("\t");
        await untilFrame(session, "▸ INSPECTOR");
        session.write("\t");
        await untilFrame(session, "▸ COMPOSER");
        session.write("\t");
        await untilFrame(session, "▸ NAVIGATOR");
        captureFrame("journey-120x40-navigator-focused", session);

        // Keyboard-only view switching on the Primary surface. Each transition
        // is pinned on the FRESH frame (MINOR-1): the accumulated stream still
        // carries [WORK] from the cold start, so only a fresh-frame wait
        // proves the "1" press genuinely redrew into the work view.
        session.write("2");
        await untilFrame(session, "▸[CHAT]");
        captureFrame("journey-120x40-chat-view", session);
        session.write("3");
        await untilFrame(session, "▸[TRACE]");
        session.write("1");
        await untilFrame(session, "▸[WORK]");
        captureFrame("journey-120x40-work-view", session);

        // Resize propagation: 80x24 → compact, one primary pane, Case and
        // attention retained (REQ-UX-005).
        session.resize(80, 24);
        await untilFrame(session, "▸[WORK]");
        const compactDeadline = Date.now() + 8000;
        let compact = lastFrame(session);
        while (Date.now() < compactDeadline && compact.includes("NAVIGATOR")) {
          await Bun.sleep(100);
          compact = lastFrame(session);
        }
        expect(compact).not.toContain("NAVIGATOR");
        // Case + health survive compaction (the cold Case has zero units).
        expect(compact).toContain("0 units · 0 blocked");
        captureFrame("journey-80x24-compact", session);

        // Below minimum: the recoverable prompt names current and required size.
        session.resize(60, 18);
        await untilFrame(session, "Gauntlet needs at least 80x24. This terminal is 60x18.");
        expect(lastFrame(session)).toContain("press q to quit");
        captureFrame("journey-60x18-below-minimum", session);

        // q quits from the prompt; terminal modes are restored on exit.
        session.write("q");
        const code = await session.exited();
        expect(code).toBe(0);
        expect(session.output()).toContain("\x1b[?25h"); // cursor restored
      } finally {
        session.kill();
      }
    },
    60_000,
  );

  it.skipIf(!REAL_PTY)(
    "80x24 COMPACT cold journey: one primary pane, honest empty state, composer strip, keyboard quit",
    async () => {
      const session = spawnWorkbench(80, 24);
      try {
        await untilFrame(session, "Gauntlet · gauntlet/canonical surface");
        const frame = lastFrame(session);
        expect(frame).not.toContain("NAVIGATOR");
        expect(frame).toContain("PRIMARY");
        expect(frame).toContain("COMPOSER");
        // Case + health survive compaction (REQ-UX-005; cold Case: zero units).
        expect(frame).toContain("0 units · 0 blocked");
        captureFrame("journey-80x24-cold-compact", session);

        session.write("q");
        const code = await session.exited();
        expect(code).toBe(0);
      } finally {
        session.kill();
      }
    },
    30_000,
  );

  it.skipIf(!REAL_PTY)(
    "below 80x24: launch straight into the recoverable prompt; only q acts; quit is reachable",
    async () => {
      const session = spawnWorkbench(60, 18);
      try {
        await untilFrame(session, "Gauntlet needs at least 80x24. This terminal is 60x18.");
        const promptFrame = lastFrame(session);
        expect(promptFrame).toContain("press q to quit");
        expect(promptFrame).not.toContain("▸[WORK]"); // no work surface below minimum
        captureFrame("journey-60x18-launch-prompt", session);

        // Only q acts below minimum: other keys leave the prompt up.
        session.write("p");
        session.write("1");
        await Bun.sleep(400);
        expect(lastFrame(session)).toContain("Gauntlet needs at least 80x24");

        session.write("q");
        const code = await session.exited();
        expect(code).toBe(0);
        expect(session.output()).toContain("\x1b[?25h");
      } finally {
        session.kill();
      }
    },
    30_000,
  );

  /**
   * Mouse journeys (T20 increment: byte-level mouse hop). Nothing above
   * exercises raw terminal mouse bytes → OpenTUI's SGR mouse parser →
   * MouseEvent → TerminalShellRenderer.rowMouseEvent → PointerGesture →
   * handlePointer end to end; handlePointer/handlePointer-equivalence is
   * pinned at the unit level (shell.test.ts), and this file's keyboard
   * journeys prove the semantic commands work through the real renderer,
   * but nothing before this proved a mouse byte ever reaches them.
   *
   * The wire format was verified directly against
   * `@opentui/core`'s bundled `MouseParser2.parseSgrSequence` /
   * `decodeSgrEvent` (chunk-bun-*.js): xterm SGR protocol,
   * `\x1b[<{Cb};{Cx};{Cy}M` for press (or a wheel notch — wheel events are
   * ALWAYS terminated "M", never "m"), `\x1b[<{Cb};{Cx};{Cy}m` for release;
   * Cb=0 is the left button; Cb=64/65 is wheel-up/wheel-down (the parser
   * takes `rawButtonCode & 3` for the scroll direction index and requires
   * the "M" terminator for the scroll branch); Cx/Cy are 1-based screen
   * columns/rows (the parser subtracts 1). OpenTUI's own mouse-mode-enable
   * sequences (`\x1b[?1000h` etc.) are OUTPUT the renderer sends to a real
   * terminal to ask for reports — they gate nothing on the parsing side, so
   * a raw PTY write of SGR bytes is interpreted exactly as if a terminal
   * had sent them, with no prior "enable" step needed from the test.
   *
   * Screen coordinates below were confirmed empirically against the real
   * 120x40 cold-start frame (cursor-position escapes dumped from a live
   * run): the Primary tab strip's "[CHAT]" label renders at row 3 (1-based)
   * starting column 47; the Navigator's outline rows start at row 4
   * (1-based, "Case outline"); the palette's "view chat" row renders at
   * row 5 (1-based) once opened. All all use 0-based (x, y) converted to
   * 1-based (Cx, Cy) for the wire format.
   */
  it.skipIf(!REAL_PTY)(
    "120x40 FULL: a real mouse click on the Primary tab strip's [CHAT] label converges on the same ▸[CHAT] marker the '2' keyboard journey produces",
    async () => {
      const session = spawnWorkbench(120, 40);
      try {
        await untilFrame(session, "NAVIGATOR");
        expect(lastFrame(session)).toContain("▸[WORK]"); // cold start: Work view selected

        // Real SGR left-click (press then release) inside the "[CHAT]"
        // glyph run: row 3, columns 47-52 (1-based). Column 49 lands
        // squarely inside the label.
        session.write("\x1b[<0;49;3M\x1b[<0;49;3m");
        await untilFrame(session, "▸[CHAT]");
        captureFrame("journey-120x40-mouse-click-chat", session);

        session.write("q");
        const code = await session.exited();
        expect(code).toBe(0);
      } finally {
        session.kill();
      }
    },
    30_000,
  );

  it.skipIf(!REAL_PTY)(
    "120x40 FULL: a real mouse wheel over the Navigator moves the selection the same way the down-arrow key does",
    async () => {
      // Deliberately opts back into the real ambient endpoint (see
      // spawnWorkbench's doc comment): this journey's whole point is
      // scrolling through REAL synced units, which only exist if this
      // spawn can actually reach a running `gauntlet serve`.
      const session = spawnWorkbench(120, 40, {
        GAUNTLET_WORKBENCH_ENDPOINT: DEFAULT_CONFIG.gauntlet_endpoint,
      });
      try {
        // Wait for the generation to load so the outline has real rows
        // beneath "Case outline" to move onto.
        await untilFrame(session, "AK-901");
        // Focus Navigator by keyboard first: a wheel over Navigator moves
        // navigatorIndex regardless of which surface is focused (the
        // gesture only borrows focus for the duration of the command, per
        // handlePointer's wheel branch), but the "◉" highlight marker only
        // renders while Navigator IS the focused surface — focusing it
        // first lets this test assert on that rendered marker.
        session.write("\t");
        session.write("\t");
        session.write("\t");
        await untilFrame(session, "▸ NAVIGATOR");

        // Real SGR wheel-down notches (button code 65 = scroll bit set,
        // direction index 1 = "down"; terminator is always "M" for wheel)
        // over the Navigator body (row 4, column 11 — inside the "Case
        // outline" row, but any row within Navigator's bounds resolves to
        // surfaceId "Navigator", which is all the wheel path needs). Two
        // notches move the highlight Case outline → AK-901 → AK-902, the
        // same nav.move.next effect the down-arrow key produces.
        session.write("\x1b[<65;11;4M");
        await untilFrame(session, "◉ AK-901");
        session.write("\x1b[<65;11;4M");
        await untilFrame(session, "◉ AK-902");
        captureFrame("journey-120x40-mouse-wheel-navigator", session);

        session.write("q");
        const code = await session.exited();
        expect(code).toBe(0);
      } finally {
        session.kill();
      }
    },
    30_000,
  );

  // Double-click sub-case (2c) DROPPED after genuine debugging effort: the
  // one concrete double-click-bound affordance reachable from cold start is
  // the command palette's row-activate branch (shell.ts's Overlay clause —
  // single click only moves paletteIndex, double_click also fires
  // ui.palette.activate). But it is UNREACHABLE by real mouse input: the
  // real renderer's terminal-renderer.ts resolveAt() (used by rowMouseEvent
  // to turn screen (x, y) into a surfaceId/lineId) does a forward,
  // first-match scan over `this.lastHitMap.regions`, and those regions are
  // pushed in a fixed order — header, then each surface column
  // (Navigator/Primary/Inspector, each already spanning from y=2 down
  // through its own row count), then bottom/attention/Composer/help/drawer,
  // and the "palette" region LAST, only once `shell.palette` is non-null.
  // The palette's z-index (1/2 vs. the background's 0, see the z-tier
  // comment near this file's "zIndex: region.key === "palette" ? 2 : 0")
  // governs PAINT order only — resolveAt never consults it. Every screen
  // cell the palette visually covers is therefore ALREADY claimed by the
  // Navigator/Primary/Inspector region drawn under it, so a click resolves
  // to that background surface, not "Overlay" — the palette is not
  // clickable through the real pointer path at all. Verified empirically
  // (`bun run` against a live PTY): clicking the rendered "view chat"
  // palette row at its exact screen coordinates produces no state change
  // (no "◉" selection marker, palette stays open) because the click lands
  // on the Navigator region beneath it. Fixing resolveAt's hit-test
  // precedence is a terminal-renderer.ts change — out of scope for this
  // test-only increment (no non-test files touched).
});
