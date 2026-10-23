/**
 * Headless codemode host. Reads one `run` message from stdin, executes the program in a QuickJS
 * sandbox, speaks JSONL on stdio, emits one `done`, and exits. Diagnostics go to stderr.
 *
 * Exit codes: 0 when a program outcome (any status) was reported; 1 when the parent broke the
 * protocol or closed the channel early (a `done` with status "failed" is still written first).
 */
import {
  type DoneMessage,
  encodeHostMessage,
  type HostMessage,
  MAX_LINE_BYTES,
  type ParentMessage,
  ProtocolError,
  parseParentMessage,
} from "./protocol";
import { RunSession } from "./run";

let session: RunSession | undefined;
let exiting = false;
let pending: Buffer = Buffer.alloc(0);

function writeAndExit(message: HostMessage, code: number): void {
  exiting = true;
  process.stdout.write(encodeHostMessage(message), () => process.exit(code));
}

/** Fail closed: a `done` with status "failed", then a non-zero exit. The first fault wins. */
function fault(reason: string): void {
  if (exiting) return;
  process.stderr.write(`codemode-host: protocol fault: ${reason}\n`);
  const done: DoneMessage = {
    v: 1,
    t: "done",
    status: "failed",
    value: null,
    message: `protocol fault: ${reason}`,
  };
  writeAndExit(done, 1);
}

function startRun(message: Extract<ParentMessage, { t: "run" }>): void {
  session = new RunSession(
    message,
    (call) => {
      process.stdout.write(encodeHostMessage({ v: 1, t: "call", ...call }));
    },
    (done) => {
      writeAndExit(done, 0);
    },
  );
  session.start();
}

function handleLine(line: string): void {
  let message: ParentMessage;
  try {
    message = parseParentMessage(line);
  } catch (error) {
    fault(error instanceof ProtocolError ? error.message : String(error));
    return;
  }
  if (!session) {
    if (message.t !== "run") {
      fault("the first message must be run");
      return;
    }
    startRun(message);
    return;
  }
  switch (message.t) {
    case "result":
      try {
        session.answer(message.id, message.outcome);
      } catch (error) {
        fault(error instanceof Error ? error.message : String(error));
      }
      return;
    case "cancel":
      session.cancel();
      return;
    case "run":
      fault("run may be sent only once");
      return;
  }
}

function onData(chunk: Buffer): void {
  if (exiting) return;
  pending = pending.length === 0 ? chunk : Buffer.concat([pending, chunk]);
  let newline = pending.indexOf(0x0a);
  while (newline !== -1) {
    if (newline > MAX_LINE_BYTES) {
      fault("line exceeds 1 MiB");
      return;
    }
    const line = pending.subarray(0, newline).toString("utf8");
    pending = pending.subarray(newline + 1);
    handleLine(line);
    if (exiting) return;
    newline = pending.indexOf(0x0a);
  }
  if (pending.length > MAX_LINE_BYTES) fault("line exceeds 1 MiB");
}

export function main(): void {
  process.stdin.on("data", onData);
  process.stdin.on("end", () => {
    if (!exiting) fault("stdin closed before the program finished");
  });
}

if (import.meta.main) {
  main();
}
