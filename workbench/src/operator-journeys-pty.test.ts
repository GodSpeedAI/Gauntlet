import { describe, expect, it } from "bun:test";
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";

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
  exited(): Promise<number | "TIMEOUT">;
  kill(): void;
}

/** Spawn the REAL workbench entry in a REAL Bun PTY. */
function spawnWorkbench(cols: number, rows: number): PtySession {
  let output = "";
  const proc = Bun.spawn(["bun", "run", "src/main.ts"], {
    cwd: WORKBENCH_DIR,
    env: { ...process.env, TERM: "xterm-256color" },
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
    resize: (nextCols, nextRows) => terminal.resize(nextCols, nextRows),
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

/** The LAST drawn frame (after the final clear-screen) with escape sequences stripped. */
function lastFrame(session: PtySession): string {
  const raw = session.output();
  const lastClear = raw.lastIndexOf("\x1b[2J");
  const frame = lastClear >= 0 ? raw.slice(lastClear) : raw;
  return stripAnsi(frame);
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
        expect(lastFrame(session)).toContain("Gauntlet · gauntlet/unbound");
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
        await untilFrame(session, "PRIMARY [CHAT]");
        captureFrame("journey-120x40-chat-view", session);
        session.write("3");
        await untilFrame(session, "PRIMARY [TRACE]");
        session.write("1");
        await untilFrame(session, "PRIMARY [WORK]");
        captureFrame("journey-120x40-work-view", session);

        // Resize propagation: 80x24 → compact, one primary pane, Case and
        // attention retained (REQ-UX-005).
        session.resize(80, 24);
        await untilFrame(session, "PRIMARY [WORK]");
        const compactDeadline = Date.now() + 8000;
        let compact = lastFrame(session);
        while (Date.now() < compactDeadline && compact.includes("NAVIGATOR")) {
          await Bun.sleep(100);
          compact = lastFrame(session);
        }
        expect(compact).not.toContain("NAVIGATOR");
        // Case + health survive compaction (the cold Case has zero units).
        expect(compact).toContain("0 units · ! 0 blocked");
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
        await untilFrame(session, "Gauntlet · gauntlet/unbound");
        const frame = lastFrame(session);
        expect(frame).not.toContain("NAVIGATOR");
        expect(frame).toContain("PRIMARY");
        expect(frame).toContain("COMPOSER");
        // Case + health survive compaction (REQ-UX-005; cold Case: zero units).
        expect(frame).toContain("0 units · ! 0 blocked");
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
        expect(promptFrame).not.toContain("PRIMARY [WORK]"); // no work surface below minimum
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
});
