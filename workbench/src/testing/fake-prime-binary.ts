import { chmodSync, mkdirSync, writeFileSync } from "node:fs";
import { dirname } from "node:path";

/**
 * Write a tiny, deterministic stand-in for the real `prime-agent` binary
 * (plan Phase C2 non-gated teeth). It ignores whatever argv `RealPrimeHost`
 * passes (a POSIX shell script that never reads `$@`) and prints fixed
 * newline-JSON lines in the exact protocol shape T24's gated battery proved
 * against the real binary (`{type:"session",...}` / `{type:"message_end",
 * message:{role:"assistant",...}}`) — this exercises RealPrimeHost's REAL
 * `Bun.spawn` → stdout-parse → mailbox/contract pipeline end to end without
 * requiring the qualified installed binary, for the teeth that test
 * dispatcher/gateway-level invariants rather than the real binary's own
 * surface (which the GATEWAY real battery separately qualifies).
 */
export function writeFakePrimeAgent(
  path: string,
  options: {
    readonly assistantText?: string | undefined;
    readonly exitCode?: number | undefined;
    readonly sleepMs?: number | undefined;
  } = {},
): void {
  const assistantText = options.assistantText ?? null;
  const exitCode = options.exitCode ?? 0;
  const sleepMs = options.sleepMs ?? 0;
  const lines: string[] = [`{"type":"session","id":"fake-session"}`];
  if (assistantText !== null) {
    const escaped = assistantText.replace(/\\/g, "\\\\").replace(/"/g, '\\"');
    lines.push(
      `{"type":"message_end","message":{"role":"assistant","content":[{"type":"text","text":"${escaped}"}]}}`,
    );
  }
  const sleepCmd = sleepMs > 0 ? `sleep ${(sleepMs / 1000).toFixed(3)}\n` : "";
  const script = `#!/bin/sh\n${sleepCmd}${lines.map((line) => `printf '%s\\n' '${line}'`).join("\n")}\nexit ${exitCode}\n`;
  mkdirSync(dirname(path), { recursive: true });
  writeFileSync(path, script, { mode: 0o755 });
  chmodSync(path, 0o755);
}
