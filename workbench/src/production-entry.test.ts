import { expect, it } from "bun:test";
import { EmbeddedTerminalRenderable } from "@opentui/core";
import { createTestRenderer } from "@opentui/core/testing";

/** Exercise the released process, including its real terminal escape sequences. */
async function launch(endpoint: string) {
  const display = await createTestRenderer({ width: 120, height: 40 });
  const screen = new EmbeddedTerminalRenderable(display.renderer, {
    id: "production-terminal",
    width: 120,
    height: 40,
    cols: 120,
    rows: 40,
    onData: (data) => processHandle.terminal?.write(data),
  });
  display.renderer.root.add(screen);
  const processHandle = Bun.spawn([process.execPath, "src/main.ts"], {
    cwd: `${import.meta.dir}/..`,
    env: {
      ...process.env,
      TERM: "xterm-256color",
      GAUNTLET_WORKBENCH_ENDPOINT: endpoint,
      GAUNTLET_WORKBENCH_PROJECTION_STORE: ":memory:",
    },
    terminal: { cols: 120, rows: 40, data: (_terminal, data) => screen.write(data) },
  });
  async function waitFor(predicate: (text: string) => boolean, label: string, timeout = 5000) {
    const deadline = Date.now() + timeout;
    while (Date.now() < deadline) {
      if (predicate(screen.screen().text)) return;
      await Bun.sleep(20);
    }
    throw new Error(`${label}\n${screen.screen().text}`);
  }
  return {
    screen: () => screen.screen(),
    waitFor,
    key: (key: string) => processHandle.terminal?.write(key),
    resize: async (cols: number, rows: number) => {
      screen.width = cols;
      screen.height = rows;
      display.resize(cols, rows);
      await display.renderOnce();
      processHandle.terminal?.resize(cols, rows);
    },
    exited: processHandle.exited,
    close: async () => {
      processHandle.kill();
      await processHandle.exited;
      processHandle.terminal?.close();
      display.renderer.destroy();
    },
  };
}

it("release entry contacts the canonical surface and paints its data in terminal cells", async () => {
  const reads: string[] = [];
  const mutations: string[] = [];
  let connected = true;
  let claim = "Terminal界 Unit";
  const backend = Bun.serve({
    hostname: "127.0.0.1",
    port: 0,
    fetch(request) {
      const path = new URL(request.url).pathname;
      (request.method === "GET" ? reads : mutations).push(path);
      if (!connected) return Response.json({ code: "unavailable" }, { status: 503 });
      if (path === "/api/v1/state")
        return Response.json({
          run: "run-pty",
          escalated: [],
          units: [{ id: "PTY-001", status: "open", tier: "contained" }],
        });
      if (path === "/api/v1/units/PTY-001/inspection")
        return Response.json({
          unit_id: "PTY-001",
          claim,
          authority: "operator",
          blast_radius: "local",
          depends_on: [],
          verifications: [],
          settled: false,
        });
      return Response.json({ code: "not_found" }, { status: 404 });
    },
  });
  const terminal = await launch(backend.url.href);
  try {
    await terminal.waitFor(() => reads.includes("/api/v1/state"), "entry never contacted backend");
    await terminal.waitFor((text) => text.includes("PTY-001"), "canonical unit never rendered");
    await terminal.waitFor((text) => text.includes(claim), "canonical claim never rendered");
    expect(mutations).toEqual([]);
    expect(terminal.screen().columns).toBe(120);
    expect(terminal.screen().rows).toBe(40);
    const headings = terminal
      .screen()
      .lines.find((line) => line.includes("NAVIGATOR") && line.includes("INSPECTOR"));
    expect(headings?.slice(0, 28)).toContain("NAVIGATOR");
    expect(headings?.slice(28, 84)).toContain("PRIMARY");
    expect(headings?.slice(84)).toContain("INSPECTOR");
    expect(
      terminal
        .screen()
        .lines.slice(15, 25)
        .every((line) => !line.includes("…")),
    ).toBe(true);

    terminal.key("p");
    await terminal.waitFor((text) => text.includes("Palette ·"), "palette invisible");
    terminal.key("\x1b");
    await terminal.waitFor((text) => !text.includes("Palette ·"), "standalone Escape failed", 1000);
    terminal.key("\x1b[Z");
    await terminal.waitFor(
      (text) => text.includes("▸ NAVIGATOR"),
      "Shift-Tab did not reverse focus",
    );
    terminal.key("\x1b[B\x1b[B\r");
    await terminal.waitFor(
      (text) => text.includes("IDENTITY"),
      "navigator selection never reached Inspector",
    );
    terminal.key("\x1b");
    await terminal.waitFor(
      (text) => text.includes("▸ PRIMARY"),
      "Escape did not restore Primary",
      1000,
    );
    terminal.key("e");
    terminal.key("界🙂q draft");
    await terminal.waitFor(
      (text) => text.includes("界🙂q draft"),
      "Unicode composer draft invisible",
    );
    expect(terminal.screen().text).not.toContain("�");
    terminal.key("\x1b[200~ pasted🙂q\x1b[201~");
    await terminal.waitFor(
      (text) => text.includes("界🙂q draft pasted🙂q"),
      "bracketed paste lost Unicode or executed a shortcut",
    );
    terminal.key("\x1b");
    await terminal.waitFor((text) => text.includes("▸ PRIMARY"), "composer Escape failed", 1000);

    claim = "Updated界 Unit";
    await terminal.waitFor(
      (text) => text.includes(claim) && !text.includes("Terminal界 Unit"),
      "source update not projected",
    );
    connected = false;
    await terminal.waitFor((text) => /disconnected/i.test(text), "source loss not visible");
    expect(mutations).toEqual([]);
    connected = true;
    await terminal.waitFor(
      (text) => !/disconnected/i.test(text) && text.includes(claim),
      "source reconnect not visible",
    );

    await terminal.resize(80, 24);
    await terminal.waitFor(
      (text) => !text.includes("INSPECTOR") && text.includes("COMPOSER"),
      "compact layout failed",
    );
    expect(terminal.screen().columns).toBe(80);
    expect(terminal.screen().rows).toBe(24);
    expect(terminal.screen().text).toContain("界🙂q draft");
    await terminal.resize(70, 20);
    await terminal.waitFor(
      (text) => /resize|too small|minimum/i.test(text),
      "below-minimum prompt missing",
    );
    await terminal.resize(120, 40);
    await terminal.waitFor(
      (text) => text.includes("INSPECTOR") && text.includes(claim),
      "resize recovery lost data",
    );
    expect(mutations).toEqual([]);
    terminal.key("q");
    expect(await Promise.race([terminal.exited, Bun.sleep(3000).then(() => "timeout")])).toBe(0);
  } finally {
    await terminal.close();
    backend.stop(true);
  }
}, 30000);
