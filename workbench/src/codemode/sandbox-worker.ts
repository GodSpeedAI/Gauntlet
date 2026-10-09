/**
 * Worker entry. One worker runs one program in one fresh QuickJS VM. Running in a worker keeps
 * the host thread free to read stdin (cancel) and to time out a spinning program; the VM's
 * interrupt handler polls a shared flag the host sets.
 *
 * The worker never touches the filesystem, network or process state itself: the only host
 * capability inside the VM is the `host` bridge, which posts to the host thread.
 */
import { readFileSync } from "node:fs";
import { createRequire } from "node:module";
import { parentPort, workerData } from "node:worker_threads";
import { JSException, type JSValueHandle, MAX_STACK_SIZE, QuickJS } from "quickjs-wasi";
import { isHostToWorker, type SandboxStart, type WorkerToHost } from "./sandbox-messages";
import { SANDBOX_PRELUDE } from "./sandbox-prelude";

const WASM_PATH = createRequire(import.meta.url).resolve("quickjs-wasi/quickjs.wasm");

function post(message: WorkerToHost): void {
  parentPort?.postMessage(message);
}

function crash(error: unknown): void {
  post({
    type: "crash",
    message: error instanceof Error ? `${error.name}: ${error.message}` : String(error),
  });
}

/**
 * QuickJS writes engine diagnostics to fd 1 and 2. That output is not part of the protocol, so
 * it is discarded. Reporting every byte as written stops libc from retrying.
 */
function discardOutput(memory: { readonly buffer: ArrayBufferLike }) {
  return {
    fd_write(_fd: number, iovsPtr: number, iovsLen: number, nwrittenPtr: number): number {
      const view = new DataView(memory.buffer);
      let written = 0;
      for (let i = 0; i < iovsLen; i++) {
        written += view.getUint32(iovsPtr + i * 8 + 4, true);
      }
      view.setUint32(nwrittenPtr, written, true);
      return 0;
    },
  };
}

function describeException(error: JSException): string {
  return JSON.stringify({ name: error.name, message: error.message });
}

async function main(data: SandboxStart): Promise<void> {
  const interrupt = new Int32Array(data.interrupt);
  const vm = await QuickJS.create({
    wasm: readFileSync(WASM_PATH),
    memoryLimit: data.memoryLimitBytes,
    // Without a guard, deep recursion traps the wasm stack instead of throwing a RangeError.
    maxStackSize: MAX_STACK_SIZE,
    interruptHandler: () => Atomics.load(interrupt, 0) !== 0,
    wasi: discardOutput,
  });

  // The only host capability in the guest. Arguments are primitives: a kind, then up to three.
  const bridge = vm.newFunction("host", (kindHandle, a, b, c) => {
    const kind = kindHandle.toString();
    if (kind === "call") {
      post({ type: "call", id: a.toNumber(), tool: b.toString(), args: c.toString() });
    } else if (kind === "attempt") {
      post({ type: "attempt", path: a.toString() });
    } else if (kind === "done") {
      post({ type: "done", ok: a.toBoolean(), payload: b.toString() });
    } else {
      post({ type: "crash", message: `unknown bridge kind ${kind}` });
    }
    return vm.undefined;
  });

  const api = vm.withScope((scope) =>
    scope.escape(
      vm.callFunction(
        vm.evalCode(SANDBOX_PRELUDE, "codemode-prelude.js"),
        vm.undefined,
        bridge,
        vm.newString(JSON.stringify(data.admitted)),
      ),
    ),
  );
  const start = api.getProp("start");
  const settle = api.getProp("settle");
  const drain = () => {
    vm.executePendingJobs();
  };

  parentPort?.on("message", (message: unknown) => {
    if (!isHostToWorker(message)) return;
    try {
      vm.withScope(() => {
        vm.callFunction(
          settle,
          api,
          vm.newNumber(message.id),
          message.ok ? vm.true : vm.false,
          vm.newString(message.payload),
        );
      });
      drain();
    } catch (error) {
      crash(error);
    }
  });

  // The program is the body of an async function whose only parameter is `tools`.
  let body: JSValueHandle;
  try {
    body = vm.evalCode(`(async (tools) => {${data.source}\n})`, "codemode.js");
  } catch (error) {
    if (!(error instanceof JSException)) throw error;
    post({ type: "done", ok: false, payload: describeException(error) });
    return;
  }
  vm.callFunction(start, api, body).dispose();
  body.dispose();
  drain();
}

if (parentPort) {
  main(workerData as SandboxStart).catch(crash);
}
