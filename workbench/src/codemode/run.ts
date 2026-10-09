/**
 * Host-side state machine for one program run. It owns the sandbox worker, the wall-clock timer,
 * the tool budget and the in-flight cap, and it reports exactly one `done` message through
 * `onFinish`. The caller (`cli.ts`) writes that line and exits.
 *
 * Authority: the host counts tool attempts and in-flight calls; the VM-side prelude only reports
 * them. A protocol fault from the parent is not a program outcome; `cli.ts` handles it.
 */
import { fileURLToPath } from "node:url";
import { Worker } from "node:worker_threads";
import type {
  DoneMessage,
  DoneStatus,
  JsonValue,
  LimitName,
  ResultOutcome,
  RunMessage,
} from "./protocol";
import type { SandboxStart, WorkerToHost } from "./sandbox-messages";

/** Guest heap cap. Exhausting it surfaces as "out of memory" and is reported as the memory limit. */
export const SANDBOX_MEMORY_LIMIT_BYTES = 64 * 1024 * 1024;

export type CallEmitter = (message: { id: number; tool: string; args: JsonValue }) => void;

interface Outcome {
  readonly status: DoneStatus;
  readonly value: JsonValue;
  readonly message?: string;
  readonly limit?: LimitName;
}

export class RunSession {
  private readonly interrupt = new SharedArrayBuffer(4);
  private readonly admitted: ReadonlySet<string>;
  private readonly inFlight = new Map<number, string>();
  private worker: Worker | undefined;
  private timer: ReturnType<typeof setTimeout> | undefined;
  private finished = false;
  private attempts = 0;

  constructor(
    private readonly run: RunMessage,
    private readonly emitCall: CallEmitter,
    private readonly onFinish: (done: DoneMessage) => void,
  ) {
    this.admitted = new Set(run.admitted);
  }

  get isFinished(): boolean {
    return this.finished;
  }

  start(): void {
    const sourceBytes = Buffer.byteLength(this.run.source, "utf8");
    if (sourceBytes > this.run.limits.max_source_bytes) {
      this.finish({
        status: "limit_exceeded",
        value: null,
        message: `source is ${sourceBytes} bytes; the limit is ${this.run.limits.max_source_bytes}`,
        limit: "source_bytes",
      });
      return;
    }
    this.timer = setTimeout(() => {
      this.finish({
        status: "timed_out",
        value: null,
        message: `program exceeded ${this.run.limits.timeout_ms} ms`,
      });
    }, this.run.limits.timeout_ms);

    const data: SandboxStart = {
      source: this.run.source,
      admitted: this.run.admitted,
      memoryLimitBytes: SANDBOX_MEMORY_LIMIT_BYTES,
      interrupt: this.interrupt,
    };
    const worker = new Worker(fileURLToPath(new URL("./sandbox-worker.ts", import.meta.url)), {
      workerData: data,
    });
    worker.on("message", (message: WorkerToHost) => {
      this.onWorkerMessage(message);
    });
    worker.on("error", (error: Error) => {
      this.finish({ status: "failed", value: null, message: `sandbox error: ${error.message}` });
    });
    this.worker = worker;
  }

  /** The parent answered a call. Throws when the id is not outstanding (a protocol fault). */
  answer(id: number, outcome: ResultOutcome): void {
    if (this.finished) return;
    if (!this.inFlight.has(id)) throw new Error(`result for unknown call id ${id}`);
    this.inFlight.delete(id);
    if (outcome.kind === "ok") {
      this.worker?.postMessage({
        type: "result",
        id,
        ok: true,
        payload: JSON.stringify(outcome.value),
      });
    } else {
      const payload = JSON.stringify({ kind: outcome.kind, message: outcome.message ?? "" });
      this.worker?.postMessage({ type: "result", id, ok: false, payload });
    }
  }

  cancel(): void {
    this.finish({ status: "cancelled", value: null, message: "cancelled by parent" });
  }

  private onWorkerMessage(message: WorkerToHost): void {
    if (this.finished) return;
    switch (message.type) {
      case "call":
        this.onCall(message.id, message.tool, message.args, message.pending);
        return;
      case "attempt":
        // A probe of a name that is not admitted: it counts, and never reaches the parent.
        this.spendAttempt(message.path);
        return;
      case "done":
        this.onProgramDone(message.ok, message.payload);
        return;
      case "crash":
        this.finish(this.classifyFailure(`sandbox crashed: ${message.message}`));
        return;
    }
  }

  /**
   * `pending` counts the guest's unsettled calls before this one. Using the guest's count, not
   * `inFlight`, makes the bound independent of when the parent's answers arrive: an answer can
   * land between two synchronously issued calls of a group, and must not admit the over-bound one.
   */
  private onCall(id: number, tool: string, argsText: string, pending: number): void {
    if (!this.admitted.has(tool)) {
      // The prelude never forwards a non-admitted name; reaching here means the VM misbehaved.
      this.finish({
        status: "failed",
        value: null,
        message: `internal: non-admitted call ${tool}`,
      });
      return;
    }
    if (this.spendAttempt(tool)) return;
    if (pending >= this.run.limits.max_parallel_calls) {
      this.finish({
        status: "limit_exceeded",
        value: null,
        message: `more than ${this.run.limits.max_parallel_calls} calls in flight`,
        limit: "parallel",
      });
      return;
    }
    this.inFlight.set(id, tool);
    this.emitCall({ id, tool, args: JSON.parse(argsText) as JsonValue });
  }

  /** Counts one attempt against the budget. Returns true when this finished the run. */
  private spendAttempt(name: string): boolean {
    this.attempts += 1;
    if (this.attempts <= this.run.limits.max_tool_calls) return false;
    this.finish({
      status: "limit_exceeded",
      value: null,
      message: `more than ${this.run.limits.max_tool_calls} tool attempts (last: ${name})`,
      limit: "tool_calls",
    });
    return true;
  }

  private onProgramDone(ok: boolean, payload: string): void {
    if (!ok) {
      this.finish(this.classifyFailure(describeError(payload)));
      return;
    }
    const bytes = Buffer.byteLength(payload, "utf8");
    if (bytes > this.run.limits.max_result_bytes) {
      this.finish({
        status: "limit_exceeded",
        value: null,
        message: `result is ${bytes} bytes; the limit is ${this.run.limits.max_result_bytes}`,
        limit: "result_bytes",
      });
      return;
    }
    this.finish({ status: "completed", value: JSON.parse(payload) as JsonValue });
  }

  /** Engine out-of-memory is reported as the memory limit, not as a generic failure. */
  private classifyFailure(message: string): Outcome {
    if (/out of memory/i.test(message)) {
      return { status: "limit_exceeded", value: null, message, limit: "memory" };
    }
    return { status: "failed", value: null, message };
  }

  private finish(outcome: Outcome): void {
    if (this.finished) return;
    this.finished = true;
    if (this.timer !== undefined) clearTimeout(this.timer);
    // A spinning guest cannot be stopped by terminating the worker from here; the VM interrupt
    // handler polls this flag. The caller exits the process, which ends the worker.
    Atomics.store(new Int32Array(this.interrupt), 0, 1);
    void this.worker?.terminate();
    this.onFinish({
      v: 1,
      t: "done",
      status: outcome.status,
      value: outcome.value,
      ...(outcome.message !== undefined ? { message: outcome.message } : {}),
      ...(outcome.limit !== undefined ? { limit: outcome.limit } : {}),
    });
  }
}

/** Format the worker's `{name, message}` JSON as "Name: message". */
function describeError(payload: string): string {
  try {
    const parsed = JSON.parse(payload) as { name?: unknown; message?: unknown };
    const name = typeof parsed.name === "string" ? parsed.name : "Error";
    const message = typeof parsed.message === "string" ? parsed.message : "";
    return message ? `${name}: ${message}` : name;
  } catch {
    return payload;
  }
}
