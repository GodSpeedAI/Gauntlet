/**
 * Process-level tests for the codemode host. Each test spawns `bun src/codemode/cli.ts` and talks
 * to it over real stdio with a scripted fake parent, so the wire protocol is what is exercised.
 */
import { afterEach, describe, expect, test } from "bun:test";
import { fileURLToPath } from "node:url";

const CLI = fileURLToPath(new URL("./cli.ts", import.meta.url));
const WORKBENCH = fileURLToPath(new URL("../..", import.meta.url));
const TEST_TIMEOUT_MS = 20_000;

const DEFAULT_LIMITS = {
  timeout_ms: 5_000,
  max_tool_calls: 20,
  max_parallel_calls: 4,
  max_result_bytes: 100_000,
  max_source_bytes: 100_000,
};

type Json = null | boolean | number | string | Json[] | { [key: string]: Json };
type Outcome =
  | { kind: "ok"; value: Json }
  | { kind: "refused" | "invalid" | "error"; message?: string };
interface CallMsg {
  v: 1;
  t: "call";
  id: number;
  tool: string;
  args: Json;
}
interface DoneMsg {
  v: 1;
  t: "done";
  status: string;
  value: Json;
  message?: string;
  limit?: string;
}

/** One host child process with line-buffered stdout. */
class HostProcess {
  private readonly proc: ReturnType<typeof Bun.spawn<"pipe", "pipe", "pipe">>;
  private buffer = "";
  private readonly lines: string[] = [];
  private readonly waiters: Array<(line: string) => void> = [];
  private stderrText = "";

  constructor() {
    this.proc = Bun.spawn(["bun", CLI], {
      stdin: "pipe",
      stdout: "pipe",
      stderr: "pipe",
      cwd: WORKBENCH,
    });
    void this.pumpStdout();
    void this.pumpStderr();
  }

  private async pumpStdout(): Promise<void> {
    const decoder = new TextDecoder();
    for await (const chunk of this.proc.stdout) {
      this.buffer += decoder.decode(chunk, { stream: true });
      let newline = this.buffer.indexOf("\n");
      while (newline !== -1) {
        const line = this.buffer.slice(0, newline);
        this.buffer = this.buffer.slice(newline + 1);
        const waiter = this.waiters.shift();
        if (waiter) waiter(line);
        else this.lines.push(line);
        newline = this.buffer.indexOf("\n");
      }
    }
  }

  private async pumpStderr(): Promise<void> {
    const decoder = new TextDecoder();
    for await (const chunk of this.proc.stderr) {
      this.stderrText += decoder.decode(chunk, { stream: true });
    }
  }

  send(message: unknown): void {
    const text = typeof message === "string" ? message : JSON.stringify(message);
    this.proc.stdin.write(`${text}\n`);
    this.proc.stdin.flush();
  }

  closeStdin(): void {
    this.proc.stdin.end();
  }

  /** The next stdout line, parsed. Fails the test if the host stays silent. */
  async next(): Promise<unknown> {
    const queued = this.lines.shift();
    if (queued !== undefined) return JSON.parse(queued);
    const line = await new Promise<string>((resolve, reject) => {
      const timer = setTimeout(
        () => reject(new Error("timed out waiting for a host line")),
        15_000,
      );
      this.waiters.push((text) => {
        clearTimeout(timer);
        resolve(text);
      });
    });
    return JSON.parse(line);
  }

  exit(): Promise<number> {
    return this.proc.exited;
  }

  stderr(): string {
    return this.stderrText;
  }

  kill(): void {
    this.proc.kill();
  }
}

const hosts: HostProcess[] = [];

function openHost(): HostProcess {
  const host = new HostProcess();
  hosts.push(host);
  return host;
}

afterEach(() => {
  for (const host of hosts.splice(0)) host.kill();
});

function runMessage(
  source: string,
  options: { admitted?: string[]; limits?: Partial<typeof DEFAULT_LIMITS> } = {},
) {
  return {
    v: 1,
    t: "run",
    source,
    admitted: options.admitted ?? ["agents.list"],
    limits: { ...DEFAULT_LIMITS, ...options.limits },
  };
}

interface Exchange {
  readonly done: DoneMsg;
  readonly calls: CallMsg[];
  readonly exitCode: number;
}

/**
 * Drive one program. `respond` answers each call; returning null leaves it unanswered, so a test
 * can observe what the host does while a call is outstanding.
 */
async function runProgram(
  source: string,
  options: {
    admitted?: string[];
    limits?: Partial<typeof DEFAULT_LIMITS>;
    respond?: (call: CallMsg) => Outcome | null;
  } = {},
): Promise<Exchange> {
  const host = openHost();
  host.send(runMessage(source, options));
  const calls: CallMsg[] = [];
  for (;;) {
    const message = (await host.next()) as CallMsg | DoneMsg;
    if (message.t === "call") {
      calls.push(message);
      const outcome = options.respond?.(message) ?? null;
      if (outcome) host.send({ v: 1, t: "result", id: message.id, outcome });
      continue;
    }
    return { done: message, calls, exitCode: await host.exit() };
  }
}

const ok = (value: Json): Outcome => ({ kind: "ok", value });

describe("codemode host: program outcomes", () => {
  test(
    "an admitted tool call gets the parent's answer",
    async () => {
      const exchange = await runProgram('return await tools.agents.list({ scope: "all" });', {
        respond: () => ok([{ id: "a1" }]),
      });
      expect(exchange.done).toMatchObject({
        t: "done",
        status: "completed",
        value: [{ id: "a1" }],
      });
      expect(exchange.calls).toEqual([
        { v: 1, t: "call", id: 1, tool: "agents.list", args: { scope: "all" } },
      ] as CallMsg[]);
      expect(exchange.exitCode).toBe(0);
    },
    TEST_TIMEOUT_MS,
  );

  test(
    "a top-level admitted tool is callable with undefined arguments as {}",
    async () => {
      const exchange = await runProgram("return await tools.ping();", {
        admitted: ["ping"],
        respond: () => ok("pong"),
      });
      expect(exchange.done.value).toBe("pong");
      expect(exchange.calls[0]?.args).toEqual({});
    },
    TEST_TIMEOUT_MS,
  );

  test(
    "a non-admitted tool is not callable and never reaches the parent",
    async () => {
      const source = `
				const probes = {
					sibling: typeof tools.agents.delete,
					top: typeof tools.secret,
					admitted: typeof tools.agents.list,
				};
				let threw = "";
				try { await tools.agents.delete({ id: 1 }); } catch (e) { threw = e.name; }
				return { ...probes, threw };
			`;
      const exchange = await runProgram(source);
      expect(exchange.done.status).toBe("completed");
      expect(exchange.done.value).toEqual({
        sibling: "undefined",
        top: "undefined",
        admitted: "function",
        threw: "TypeError",
      });
      expect(exchange.calls).toEqual([]);
      expect(exchange.exitCode).toBe(0);
    },
    TEST_TIMEOUT_MS,
  );

  test(
    "ambient capabilities and common escape routes are absent",
    async () => {
      const source = `
				const names = ["fetch", "process", "require", "Bun", "Deno", "console", "host", "setTimeout", "XMLHttpRequest"];
				const globals = Object.fromEntries(names.map((n) => [n, typeof globalThis[n]]));
				const viaFunction = Function("return typeof process")();
				const viaCtor = (() => {}).constructor("return typeof require")();
				const realm = (() => {}).constructor("return this")();
				const viaRealm = { fetch: typeof realm.fetch, process: typeof realm.process, bun: typeof realm.Bun };
				const viaToolCtor = tools.agents.list.constructor("return typeof process")();
				return {
					globals,
					viaFunction,
					viaCtor,
					viaRealm,
					viaToolCtor,
					toolsProto: Object.getPrototypeOf(tools) === null,
					toolKeys: Object.keys(tools).length,
					toolsCtor: typeof tools.constructor,
				};
			`;
      const exchange = await runProgram(source, { limits: { max_tool_calls: 100 } });
      expect(exchange.done.status).toBe("completed");
      const value = exchange.done.value as Record<string, unknown>;
      expect(value["globals"]).toEqual({
        fetch: "undefined",
        process: "undefined",
        require: "undefined",
        Bun: "undefined",
        Deno: "undefined",
        console: "undefined",
        host: "undefined",
        setTimeout: "undefined",
        XMLHttpRequest: "undefined",
      });
      expect(value["viaFunction"]).toBe("undefined");
      expect(value["viaCtor"]).toBe("undefined");
      expect(value["viaRealm"]).toEqual({
        fetch: "undefined",
        process: "undefined",
        bun: "undefined",
      });
      expect(value["viaToolCtor"]).toBe("undefined");
      expect(value["toolsProto"]).toBe(true);
      expect(value["toolKeys"]).toBe(0);
      expect(value["toolsCtor"]).toBe("undefined");
      expect(exchange.calls).toEqual([]);
    },
    TEST_TIMEOUT_MS,
  );

  test(
    "the tools tree cannot be mutated",
    async () => {
      const exchange = await runProgram(`
				"use strict";
				try { tools.agents.list = () => "forged"; return "assigned"; }
				catch (e) { return e.name; }
			`);
      expect(exchange.done.value).toBe("TypeError");
    },
    TEST_TIMEOUT_MS,
  );

  test(
    "an infinite loop is stopped as timed_out",
    async () => {
      const started = Date.now();
      const exchange = await runProgram("while (true) {}", { limits: { timeout_ms: 300 } });
      expect(exchange.done).toMatchObject({ status: "timed_out", value: null });
      expect(Date.now() - started).toBeLessThan(10_000);
      expect(exchange.exitCode).toBe(0);
    },
    TEST_TIMEOUT_MS,
  );

  test(
    "exceeding max_tool_calls is limit_exceeded",
    async () => {
      const exchange = await runProgram(
        "await tools.agents.list({}); await tools.agents.list({}); return 1;",
        { limits: { max_tool_calls: 1 }, respond: () => ok([]) },
      );
      expect(exchange.done).toMatchObject({ status: "limit_exceeded", limit: "tool_calls" });
      expect(exchange.calls.length).toBe(1);
    },
    TEST_TIMEOUT_MS,
  );

  test(
    "non-admitted attempts count against max_tool_calls",
    async () => {
      const exchange = await runProgram(
        "tools.agents.a; tools.agents.b; tools.agents.c; return 1;",
        {
          limits: { max_tool_calls: 2 },
        },
      );
      expect(exchange.done).toMatchObject({ status: "limit_exceeded", limit: "tool_calls" });
      expect(exchange.calls).toEqual([]);
    },
    TEST_TIMEOUT_MS,
  );

  test(
    "more calls in flight than max_parallel_calls is limit_exceeded (parallel)",
    async () => {
      const exchange = await runProgram(
        "await Promise.all([tools.agents.list({ n: 1 }), tools.agents.list({ n: 2 })]);",
        { limits: { max_parallel_calls: 1 }, respond: () => null },
      );
      expect(exchange.done).toMatchObject({ status: "limit_exceeded", limit: "parallel" });
      expect(exchange.calls.length).toBe(1);
    },
    TEST_TIMEOUT_MS,
  );

  test(
    "a result larger than max_result_bytes is limit_exceeded",
    async () => {
      const exchange = await runProgram('return "x".repeat(100);', {
        limits: { max_result_bytes: 20 },
      });
      expect(exchange.done).toMatchObject({ status: "limit_exceeded", limit: "result_bytes" });
    },
    TEST_TIMEOUT_MS,
  );

  test(
    "a source larger than max_source_bytes is limit_exceeded before running",
    async () => {
      const exchange = await runProgram('return "this source is long";', {
        limits: { max_source_bytes: 10 },
      });
      expect(exchange.done).toMatchObject({ status: "limit_exceeded", limit: "source_bytes" });
      expect(exchange.calls).toEqual([]);
    },
    TEST_TIMEOUT_MS,
  );

  test(
    "a non-JSON result is failed",
    async () => {
      const sources = [
        "return () => 1;",
        "return { a: undefined };",
        "return 1n;",
        "const o = {}; o.self = o; return o;",
        "return { get x() { return 1; } };",
      ];
      for (const source of sources) {
        const exchange = await runProgram(source);
        expect(exchange.done.status).toBe("failed");
        expect(exchange.done.message).toContain("result is not JSON");
      }
    },
    TEST_TIMEOUT_MS * 2,
  );

  test(
    "a program that returns nothing completes with null",
    async () => {
      const exchange = await runProgram("const x = 1;");
      expect(exchange.done).toMatchObject({ status: "completed", value: null });
    },
    TEST_TIMEOUT_MS,
  );

  test(
    "a thrown error is failed with its name and message",
    async () => {
      const exchange = await runProgram('throw new RangeError("boom");');
      expect(exchange.done).toMatchObject({ status: "failed", message: "RangeError: boom" });
    },
    TEST_TIMEOUT_MS,
  );

  test(
    "a syntax error in the program is failed",
    async () => {
      const exchange = await runProgram("return (;");
      expect(exchange.done.status).toBe("failed");
    },
    TEST_TIMEOUT_MS,
  );

  test(
    "a parent refusal surfaces to the program as a ToolError with its kind",
    async () => {
      const source = `
				try { await tools.agents.list({}); return "no error"; }
				catch (e) { return { name: e.name, kind: e.kind, message: e.message }; }
			`;
      const exchange = await runProgram(source, {
        respond: () => ({ kind: "refused", message: "not allowed" }),
      });
      expect(exchange.done.value).toEqual({
        name: "ToolError",
        kind: "refused",
        message: "not allowed",
      });
    },
    TEST_TIMEOUT_MS,
  );

  test(
    "non-JSON tool arguments are rejected inside the program",
    async () => {
      const source = `
				const attempts = [() => tools.agents.list(() => 1), () => tools.agents.list({ d: new Date(0) }) ];
				const results = [];
				for (const attempt of attempts) {
					try { await attempt(); results.push("sent"); } catch (e) { results.push(e.name); }
				}
				return results;
			`;
      const exchange = await runProgram(source);
      expect(exchange.done.value).toEqual(["TypeError", "TypeError"]);
      expect(exchange.calls).toEqual([]);
    },
    TEST_TIMEOUT_MS,
  );
});

describe("codemode host: cancellation", () => {
  test(
    "cancel while a call is outstanding is cancelled",
    async () => {
      const host = openHost();
      host.send(runMessage("return await tools.agents.list({});"));
      const call = (await host.next()) as CallMsg;
      expect(call.t).toBe("call");
      host.send({ v: 1, t: "cancel" });
      const done = (await host.next()) as DoneMsg;
      expect(done).toMatchObject({ t: "done", status: "cancelled", value: null });
      expect(await host.exit()).toBe(0);
    },
    TEST_TIMEOUT_MS,
  );

  test(
    "cancel reaches a program that is spinning",
    async () => {
      const host = openHost();
      host.send(runMessage("while (true) {}", { limits: { timeout_ms: 60_000 } }));
      await Bun.sleep(300);
      host.send({ v: 1, t: "cancel" });
      const done = (await host.next()) as DoneMsg;
      expect(done).toMatchObject({ status: "cancelled" });
      expect(await host.exit()).toBe(0);
    },
    TEST_TIMEOUT_MS,
  );
});

describe("codemode host: protocol faults fail closed", () => {
  const faults: Array<{ name: string; lines: string[] }> = [
    { name: "malformed JSON", lines: ["this is not json"] },
    { name: "an empty line", lines: [""] },
    { name: "a non-object message", lines: ["[1,2,3]"] },
    { name: "a bad protocol version", lines: [JSON.stringify({ v: 2, t: "cancel" })] },
    { name: "an unknown message type", lines: [JSON.stringify({ v: 1, t: "explode" })] },
    {
      name: "a result before run",
      lines: [JSON.stringify({ v: 1, t: "result", id: 1, outcome: { kind: "ok", value: 1 } })],
    },
    {
      name: "a run with an unknown key",
      lines: [JSON.stringify({ ...runMessage("return 1;"), extra: true })],
    },
    {
      name: "a run with a missing limit",
      lines: [
        JSON.stringify({
          v: 1,
          t: "run",
          source: "return 1;",
          admitted: [],
          limits: { timeout_ms: 1000 },
        }),
      ],
    },
    {
      name: "a run with a reserved tool segment",
      lines: [JSON.stringify(runMessage("return 1;", { admitted: ["agents.then"] }))],
    },
    {
      name: "a run where a tool is also a namespace",
      lines: [JSON.stringify(runMessage("return 1;", { admitted: ["agents", "agents.list"] }))],
    },
    {
      name: "an outcome with an unknown kind",
      lines: [
        JSON.stringify(runMessage("return await tools.agents.list({});")),
        JSON.stringify({ v: 1, t: "result", id: 1, outcome: { kind: "maybe", value: 1 } }),
      ],
    },
    {
      name: "a result for an unknown call id",
      lines: [
        JSON.stringify(runMessage("return await tools.agents.list({});")),
        JSON.stringify({ v: 1, t: "result", id: 99, outcome: { kind: "ok", value: 1 } }),
      ],
    },
    {
      name: "a second run",
      lines: [
        JSON.stringify(runMessage("while (true) {}", { limits: { timeout_ms: 60_000 } })),
        JSON.stringify(runMessage("return 1;")),
      ],
    },
  ];

  for (const fault of faults) {
    test(
      `${fault.name} is failed and exits non-zero`,
      async () => {
        const host = openHost();
        for (const line of fault.lines) host.send(line);
        const exchange: DoneMsg[] = [];
        for (;;) {
          const message = (await host.next()) as CallMsg | DoneMsg;
          if (message.t === "call") continue;
          exchange.push(message);
          break;
        }
        expect(exchange[0]).toMatchObject({ t: "done", status: "failed" });
        expect(exchange[0]?.message).toContain("protocol fault");
        expect(await host.exit()).not.toBe(0);
      },
      TEST_TIMEOUT_MS,
    );
  }

  test(
    "an oversize line (over 1 MiB) is failed and exits non-zero",
    async () => {
      const host = openHost();
      host.send("x".repeat(1024 * 1024 + 16));
      const done = (await host.next()) as DoneMsg;
      expect(done).toMatchObject({ status: "failed" });
      expect(done.message).toContain("1 MiB");
      expect(await host.exit()).not.toBe(0);
    },
    TEST_TIMEOUT_MS,
  );

  test(
    "a protocol fault stops a spinning program and exits",
    async () => {
      const host = openHost();
      host.send(runMessage("while (true) {}", { limits: { timeout_ms: 60_000 } }));
      await Bun.sleep(200);
      host.send("garbage");
      const done = (await host.next()) as DoneMsg;
      expect(done.status).toBe("failed");
      expect(await host.exit()).toBe(1);
    },
    TEST_TIMEOUT_MS,
  );

  test(
    "stdin closed while a call is outstanding is failed and exits non-zero",
    async () => {
      const host = openHost();
      host.send(runMessage("return await tools.agents.list({});"));
      const call = (await host.next()) as CallMsg;
      expect(call.t).toBe("call");
      host.closeStdin();
      const done = (await host.next()) as DoneMsg;
      expect(done.status).toBe("failed");
      expect(done.message).toContain("stdin closed");
      expect(await host.exit()).toBe(1);
    },
    TEST_TIMEOUT_MS,
  );
});
