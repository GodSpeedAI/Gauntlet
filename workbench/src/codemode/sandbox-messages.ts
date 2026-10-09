/**
 * Messages between the host thread (`run.ts`) and the sandbox worker (`sandbox-worker.ts`).
 * Tool arguments and results cross as JSON text; the worker never builds structured guest values.
 */

export interface SandboxStart {
  readonly source: string;
  readonly admitted: readonly string[];
  readonly memoryLimitBytes: number;
  /** One Int32. The VM interrupt handler polls it; the host sets it to stop a spinning program. */
  readonly interrupt: SharedArrayBuffer;
}

export type WorkerToHost =
  /**
   * `pending` is the number of calls the guest had issued and not yet settled when this call was
   * issued. The host enforces max_parallel_calls against it, so a group's count is set by the
   * guest's synchronous issue order rather than by the timing of the parent's answers.
   */
  | {
      readonly type: "call";
      readonly id: number;
      readonly tool: string;
      readonly args: string;
      readonly pending: number;
    }
  | { readonly type: "attempt"; readonly path: string }
  /** `payload` is the JSON text of the result when `ok`, otherwise JSON `{name, message}`. */
  | { readonly type: "done"; readonly ok: boolean; readonly payload: string }
  | { readonly type: "crash"; readonly message: string };

export interface HostToWorker {
  readonly type: "result";
  readonly id: number;
  readonly ok: boolean;
  /** JSON text of the value when `ok`; JSON `{kind, message}` otherwise. */
  readonly payload: string;
}

export function isHostToWorker(value: unknown): value is HostToWorker {
  return (
    typeof value === "object" &&
    value !== null &&
    (value as { type?: unknown }).type === "result" &&
    typeof (value as { id?: unknown }).id === "number" &&
    typeof (value as { ok?: unknown }).ok === "boolean" &&
    typeof (value as { payload?: unknown }).payload === "string"
  );
}
