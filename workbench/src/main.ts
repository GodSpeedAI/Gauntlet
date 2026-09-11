/** Production host: canonical observations projected through native OpenTUI. */
import { createCliRenderer } from "@opentui/core";
import { resolveConfig } from "./config";
import { lineText } from "./render-tree";
import { ShellSession } from "./shell";
import { TerminalShellRenderer, terminalKey } from "./terminal-renderer";
import { WorkbenchHost } from "./workbench-host";

async function run(): Promise<void> {
  const host = new WorkbenchHost(resolveConfig({}), {
    actor: process.env["GAUNTLET_SESSION_ACTOR"] ?? "operator",
  });
  let stopped = false;
  let finish: () => void = () => {};
  const done = new Promise<void>((resolve) => {
    finish = resolve;
  });
  const quit = () => {
    stopped = true;
    finish();
  };
  const session = new ShellSession(
    { workspace: { workspace: "gauntlet", reference: "canonical surface" } },
    { cols: process.stdout.columns ?? 80, rows: process.stdout.rows ?? 24 },
    host.interactions(quit),
  );
  const shell = () => {
    const rendered = session.render(host.model.getSnapshot(), {
      observation: host.observation,
      session: host.sessionContext,
    });
    return {
      ...rendered,
      header: {
        ...rendered.header,
        spans: [
          {
            text: `${host.status}${host.runId ? ` · run ${host.runId}` : ""} · `,
            emphasis: "dim" as const,
          },
          ...rendered.header.spans,
        ],
      },
      attention: [
        ...(host.notice
          ? [
              {
                id: "host.notice",
                spans: [{ text: host.notice, token: "attention.required" as const }],
              },
            ]
          : []),
        ...rendered.attention,
      ],
    };
  };
  if (process.stdin.isTTY !== true || process.stdout.isTTY !== true) {
    try {
      await host.refresh();
      const frame = shell();
      process.stdout.write(
        [
          lineText(frame.header),
          ...(frame.prompt?.lines ?? frame.surfaces.flatMap((surface) => surface.lines)).map(
            lineText,
          ),
          ...frame.attention.map(lineText),
          ...frame.help.map(lineText),
        ].join("\n") + "\n",
      );
    } finally {
      host.close();
    }
    return;
  }
  let renderer: Awaited<ReturnType<typeof createCliRenderer>> | undefined;
  let view: TerminalShellRenderer | undefined;
  const onSignal = () => quit();
  process.on("SIGINT", onSignal);
  process.on("SIGTERM", onSignal);
  try {
    renderer = await createCliRenderer({
      exitOnCtrlC: false,
      exitSignals: [],
      useMouse: false,
      screenMode: "alternate-screen",
      openConsoleOnError: false,
    });
    view = new TerminalShellRenderer(renderer);
    const draw = () => {
      if (stopped || !renderer || !view) return;
      session.resize({ cols: renderer.width, rows: renderer.height });
      view.render(shell());
    };
    renderer.keyInput.on("keypress", (event) => {
      const key = terminalKey(event);
      if (key === "ctrl+c") {
        quit();
        return;
      }
      if (key !== null)
        session.key(key, host.model.getSnapshot(), {
          observation: host.observation,
          session: host.sessionContext,
        });
      draw();
    });
    renderer.keyInput.on("paste", (event) => {
      session.paste(new TextDecoder().decode(event.bytes));
      draw();
    });
    renderer.on("resize", draw);
    draw();
    host.start(draw);
    await done;
  } finally {
    stopped = true;
    host.close();
    view?.destroy();
    renderer?.destroy();
    process.off("SIGINT", onSignal);
    process.off("SIGTERM", onSignal);
  }
}

try {
  await run();
} catch {
  process.stderr.write(
    "gauntlet-workbench: startup or terminal failure; check endpoint and terminal configuration.\n",
  );
  process.exitCode = 1;
}
