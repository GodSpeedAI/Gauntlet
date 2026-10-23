/**
 * Wire protocol for the codemode host: newline-delimited JSON, one object per line, both
 * directions, over stdin/stdout. Normative description: `PROTOCOL.md` in this directory.
 *
 * Parsing is strict. Unknown message types, unknown keys, a wrong version, a missing or
 * mistyped field, or a line above `MAX_LINE_BYTES` raise `ProtocolError`; the caller fails closed.
 */

export const PROTOCOL_VERSION = 1;
export const MAX_LINE_BYTES = 1024 * 1024;

export const DONE_STATUSES = [
  "completed",
  "failed",
  "timed_out",
  "limit_exceeded",
  "cancelled",
] as const;
export type DoneStatus = (typeof DONE_STATUSES)[number];

/** Which budget was exceeded. Reported as `limit` on a `limit_exceeded` done message. */
export type LimitName = "tool_calls" | "parallel" | "result_bytes" | "source_bytes" | "memory";

export type JsonValue =
  | null
  | boolean
  | number
  | string
  | JsonValue[]
  | { [key: string]: JsonValue };

export interface RunLimits {
  readonly timeout_ms: number;
  readonly max_tool_calls: number;
  readonly max_parallel_calls: number;
  readonly max_result_bytes: number;
  readonly max_source_bytes: number;
}

export interface RunMessage {
  readonly v: typeof PROTOCOL_VERSION;
  readonly t: "run";
  readonly source: string;
  readonly admitted: readonly string[];
  readonly limits: RunLimits;
}

export type ResultOutcome =
  | { readonly kind: "ok"; readonly value: JsonValue }
  | {
      readonly kind: "refused" | "invalid" | "error";
      readonly value?: JsonValue;
      readonly message?: string;
    };

export interface ResultMessage {
  readonly v: typeof PROTOCOL_VERSION;
  readonly t: "result";
  readonly id: number;
  readonly outcome: ResultOutcome;
}

export interface CancelMessage {
  readonly v: typeof PROTOCOL_VERSION;
  readonly t: "cancel";
}

export type ParentMessage = RunMessage | ResultMessage | CancelMessage;

export interface CallMessage {
  readonly v: typeof PROTOCOL_VERSION;
  readonly t: "call";
  readonly id: number;
  readonly tool: string;
  readonly args: JsonValue;
}

export interface DoneMessage {
  readonly v: typeof PROTOCOL_VERSION;
  readonly t: "done";
  readonly status: DoneStatus;
  readonly value: JsonValue;
  readonly message?: string;
  readonly limit?: LimitName;
}

export type HostMessage = CallMessage | DoneMessage;

export class ProtocolError extends Error {
  override readonly name = "ProtocolError";
}

const SEGMENT_PATTERN = /^[A-Za-z][A-Za-z0-9_-]{0,63}$/;
const FORBIDDEN_SEGMENTS: ReadonlySet<string> = new Set([
  "then",
  "constructor",
  "prototype",
  "__proto__",
]);
const MAX_TOOL_NAME_CHARS = 256;

/**
 * A tool name is dot-separated segments; each segment is an identifier-like word. Segments
 * that could reach prototype or thenable machinery are refused. Returns an error message, or
 * `undefined` when the name is valid.
 */
export function toolNameProblem(name: string): string | undefined {
  if (name.length === 0 || name.length > MAX_TOOL_NAME_CHARS)
    return "tool name length out of range";
  for (const segment of name.split(".")) {
    if (!SEGMENT_PATTERN.test(segment)) return `invalid tool name segment in "${name}"`;
    if (FORBIDDEN_SEGMENTS.has(segment)) return `forbidden tool name segment in "${name}"`;
  }
  return undefined;
}

/** Validate the admitted list: valid names, no duplicates, and no name that is also a namespace. */
export function validateAdmitted(admitted: readonly string[]): string | undefined {
  const seen = new Set<string>();
  for (const name of admitted) {
    const problem = toolNameProblem(name);
    if (problem) return problem;
    if (seen.has(name)) return `duplicate admitted tool "${name}"`;
    seen.add(name);
  }
  for (const name of admitted) {
    for (const other of admitted) {
      if (other.startsWith(`${name}.`)) return `"${name}" is both a tool and a namespace`;
    }
  }
  return undefined;
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}

function expectKeys(
  record: Record<string, unknown>,
  allowed: readonly string[],
  where: string,
): void {
  for (const key of Object.keys(record)) {
    if (!allowed.includes(key)) throw new ProtocolError(`${where}: unknown key "${key}"`);
  }
  for (const key of allowed) {
    if (!(key in record)) throw new ProtocolError(`${where}: missing key "${key}"`);
  }
}

function expectNonNegativeInteger(value: unknown, where: string): number {
  if (typeof value !== "number" || !Number.isSafeInteger(value) || value < 0) {
    throw new ProtocolError(`${where}: expected a non-negative safe integer`);
  }
  return value;
}

function parseLimits(value: unknown): RunLimits {
  if (!isRecord(value)) throw new ProtocolError("run.limits: expected an object");
  const keys = [
    "timeout_ms",
    "max_tool_calls",
    "max_parallel_calls",
    "max_result_bytes",
    "max_source_bytes",
  ] as const;
  expectKeys(value, keys, "run.limits");
  const limits = {
    timeout_ms: expectNonNegativeInteger(value["timeout_ms"], "run.limits.timeout_ms"),
    max_tool_calls: expectNonNegativeInteger(value["max_tool_calls"], "run.limits.max_tool_calls"),
    max_parallel_calls: expectNonNegativeInteger(
      value["max_parallel_calls"],
      "run.limits.max_parallel_calls",
    ),
    max_result_bytes: expectNonNegativeInteger(
      value["max_result_bytes"],
      "run.limits.max_result_bytes",
    ),
    max_source_bytes: expectNonNegativeInteger(
      value["max_source_bytes"],
      "run.limits.max_source_bytes",
    ),
  };
  if (limits.timeout_ms < 1) throw new ProtocolError("run.limits.timeout_ms must be at least 1");
  if (limits.max_parallel_calls < 1) {
    throw new ProtocolError("run.limits.max_parallel_calls must be at least 1");
  }
  return limits;
}

/** Parse one inbound line. Throws `ProtocolError` on anything that is not a valid parent message. */
export function parseParentMessage(line: string): ParentMessage {
  if (Buffer.byteLength(line, "utf8") > MAX_LINE_BYTES)
    throw new ProtocolError("line exceeds 1 MiB");
  let parsed: unknown;
  try {
    parsed = JSON.parse(line);
  } catch {
    throw new ProtocolError("line is not valid JSON");
  }
  if (!isRecord(parsed)) throw new ProtocolError("message must be a JSON object");
  if (parsed["v"] !== PROTOCOL_VERSION) throw new ProtocolError(`unsupported protocol version`);
  switch (parsed["t"]) {
    case "run": {
      expectKeys(parsed, ["v", "t", "source", "admitted", "limits"], "run");
      const source = parsed["source"];
      if (typeof source !== "string") throw new ProtocolError("run.source: expected a string");
      const admitted = parsed["admitted"];
      if (!Array.isArray(admitted) || !admitted.every((name) => typeof name === "string")) {
        throw new ProtocolError("run.admitted: expected an array of strings");
      }
      const problem = validateAdmitted(admitted as string[]);
      if (problem) throw new ProtocolError(`run.admitted: ${problem}`);
      return {
        v: PROTOCOL_VERSION,
        t: "run",
        source,
        admitted: admitted as string[],
        limits: parseLimits(parsed["limits"]),
      };
    }
    case "result": {
      expectKeys(parsed, ["v", "t", "id", "outcome"], "result");
      const id = expectNonNegativeInteger(parsed["id"], "result.id");
      const outcome = parsed["outcome"];
      if (!isRecord(outcome)) throw new ProtocolError("result.outcome: expected an object");
      for (const key of Object.keys(outcome)) {
        if (!["kind", "value", "message"].includes(key)) {
          throw new ProtocolError(`result.outcome: unknown key "${key}"`);
        }
      }
      const kind = outcome["kind"];
      if (kind === "ok") {
        if (!("value" in outcome)) throw new ProtocolError("result.outcome: ok requires value");
        return {
          v: PROTOCOL_VERSION,
          t: "result",
          id,
          outcome: { kind, value: outcome["value"] as JsonValue },
        };
      }
      if (kind !== "refused" && kind !== "invalid" && kind !== "error") {
        throw new ProtocolError("result.outcome: unknown kind");
      }
      const message = outcome["message"];
      if (message !== undefined && typeof message !== "string") {
        throw new ProtocolError("result.outcome.message: expected a string");
      }
      const built: { kind: "refused" | "invalid" | "error"; value?: JsonValue; message?: string } =
        { kind };
      if ("value" in outcome) built.value = outcome["value"] as JsonValue;
      if (message !== undefined) built.message = message;
      return { v: PROTOCOL_VERSION, t: "result", id, outcome: built };
    }
    case "cancel":
      expectKeys(parsed, ["v", "t"], "cancel");
      return { v: PROTOCOL_VERSION, t: "cancel" };
    default:
      throw new ProtocolError("unknown message type");
  }
}

/** Encode one outbound message as a single JSON line (with trailing newline). */
export function encodeHostMessage(message: HostMessage): string {
  return `${JSON.stringify(message)}\n`;
}
