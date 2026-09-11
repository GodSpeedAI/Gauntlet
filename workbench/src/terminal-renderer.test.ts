import { expect, test } from "bun:test";
import { TextAttributes } from "@opentui/core";
import { createTestRenderer } from "@opentui/core/testing";
import { TerminalShellRenderer, terminalKey } from "./terminal-renderer";
import { FULL_DIMS, modelWiredSession, renderOptions, seededModel } from "./testing/shell-harness";

test("native screen preserves column allocation, palette, composer, attention and Unicode", async () => {
  const terminal = await createTestRenderer({ width: 120, height: 40 });
  const view = new TerminalShellRenderer(terminal.renderer);
  const model = seededModel();
  const session = modelWiredSession(model, FULL_DIMS);
  const draw = async () => {
    view.render(session.render(model.getSnapshot(), renderOptions(model)));
    await terminal.renderOnce();
    return terminal.captureCharFrame();
  };
  try {
    const screen = await draw();
    const title = screen.split("\n")[2] ?? "";
    expect(title.slice(0, 28)).toContain("NAVIGATOR");
    expect(title.slice(28, 84)).toContain("PRIMARY");
    expect(title.slice(84)).toContain("INSPECTOR");
    expect(screen).toContain("keys:");
    expect(screen).toContain("unit_blocked");
    expect(
      terminal
        .captureSpans()
        .lines.flatMap((row) => row.spans)
        .some(
          (span) => span.text.includes("Gauntlet") && (span.attributes & TextAttributes.BOLD) !== 0,
        ),
    ).toBe(true);
    session.key("p", model.getSnapshot());
    expect(await draw()).toContain("view work");
    terminal.renderer.keyInput.on("keypress", (event) => {
      const key = terminalKey(event);
      if (key) session.key(key, model.getSnapshot());
    });
    terminal.mockInput.pressEscape();
    await Bun.sleep(100);
    expect(session.state.paletteOpen).toBe(false);
    session.key("e", model.getSnapshot());
    await terminal.mockInput.typeText("wide 界 café");
    expect(await draw()).toContain("wide 界 café");
    terminal.mockInput.pressTab({ shift: true });
    await Bun.sleep(30);
    expect(session.state.focusedSurface).not.toBe("Composer");
    terminal.resize(80, 24);
    session.resize({ cols: 80, rows: 24 });
    expect(await draw()).toContain("wide 界 café");
    terminal.resize(70, 20);
    session.resize({ cols: 70, rows: 20 });
    expect(await draw()).toContain("at least 80x24");
    terminal.resize(60, 18);
    session.resize({ cols: 60, rows: 18 });
    const prompt = await draw();
    expect(prompt).toContain("press q to quit");
    expect(prompt).toContain("admitted work keeps running");
  } finally {
    view.destroy();
    terminal.renderer.destroy();
    model.close();
  }
});
