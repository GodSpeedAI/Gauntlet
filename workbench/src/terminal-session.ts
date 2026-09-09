/**
 * Embedded terminal session over Bun's process/PTY facilities behind the
 * stable lower-surface contract (plan T13, REQ-BUN-004, REQ-SEC-005,
 * REQ-UX-005/006, DSN-014).
 *
 * Core invariants:
 *   - NONCANONICAL SESSION STATE (plan T13; prereg D2): terminal lifecycle
 *     (spawn, input, resize, exit, cancellation) lives in TerminalSession
 *     state that is PUBLICATION-only. It never enters a canonical
 *     FullWorkbenchSnapshot; the host MAY publish its projection as the
 *     snapshot's `terminal` provider field (shell.ts LOWER_VIEW_PROVIDERS:
 *     view "terminal" ← provider "terminal"), which only gates view
 *     availability. Resize/kill/restart churn therefore changes only the
 *     terminal-session projection (Attack 2).
 *   - DSN-014 VIEWPORT BOUNDARY: ANSI/OSC control sequences, hyperlinks
 *     (OSC 8) and clipboard requests (OSC 52) are the DELIBERATE PTY
 *     viewport's business inside it, and inert TEXT everywhere else. Every
 *     byte this module stores or exposes outside the raw PTY path passes
 *     through the control-sequence sanitizer first. Nothing here writes to
 *     the host terminal.
 *   - PRIVILEGED BOUNDARY (REQ-SEC-005): spawn requests are validated and
 *     BOUND before any process exists — argv is bound as an argv ARRAY
 *     (never interpolated into a shell string), path parameters are
 *     lexically anchored under declared roots (traversal refused), env is
 *     name-validated with keys-only provenance (values are never recorded —
 *     secret discipline), automation titles and RPC payload strings are
 *     control-neutralized. Refusals are typed, carrying the locus.
 *   - PROVIDER SEAM: Bun's process/PTY facilities are the QUALIFIED native
 *     path (REQ-BUN-004; T03 admission ledger — `Bun.spawn` qualified
 *     2026-09-08, `Bun.Terminal` PTY probed for T13). The session state
 *     machine consumes a TerminalProcessProvider port; tests inject fakes
 *     for determinism. An alternate stack would be admitted only behind this
 *     same port and only after a failing required case on the native path.
 *   - PROVENANCE: what was spawned, when, with what bound argv, and the full
 *     lifecycle event log are retained across exit and cancellation. Input
 *     BYTES are never recorded (they may carry secrets); only counts.
 */

// --- Typed boundary errors -----------------------------------------------------

/** Which privileged boundary produced a refusal (typed failure locus). */
export type BoundaryLocus =
  | "argv"
  | "path"
  | "env"
  | "automation"
  | "rpc-payload"
  | "dimensions"
  | "session-state"
  | "input";

/** Closed vocabulary of refusal codes (no bare strings). */
export type BoundaryRejectionCode =
  | "empty_program"
  | "shell_metacharacter_in_program"
  | "program_not_in_allowlist"
  | "relative_program_path"
  | "path_traversal"
  | "path_not_absolute"
  | "path_outside_allowed_roots"
  | "control_char_in_argv"
  | "env_name_invalid"
  | "env_value_control_char"
  | "env_not_object"
  | "payload_not_object"
  | "payload_key_not_allowed"
  | "payload_value_not_string"
  | "payload_too_large"
  | "dimension_out_of_range"
  | "not_running"
  | "session_not_active"
  | "unknown_automation_kind";

export class TerminalBoundaryError extends Error {
  /** The boundary that refused (typed failure locus — never inferred). */
  readonly locus: BoundaryLocus;
  /** Closed-vocabulary refusal code. */
  readonly code: BoundaryRejectionCode;

  constructor(locus: BoundaryLocus, code: BoundaryRejectionCode, detail: string) {
    super(`${locus}/${code}: ${detail}`);
    this.name = "TerminalBoundaryError";
    this.locus = locus;
    this.code = code;
  }
}

// --- ANSI/OSC sanitizer (DSN-014) ------------------------------------------------

/** Neutralized-sequence counts, per class (telemetry and test teeth). */
export interface NeutralizedSequences {
  /** CSI — ESC [ … final. */
  readonly csi: number;
  /** OSC — ESC ] … (BEL | ST), including title/clipboard/hyperlink forms. */
  readonly osc: number;
  /** OSC 8 hyperlink requests. */
  readonly oscHyperlink: number;
  /** OSC 52 clipboard requests. */
  readonly oscClipboard: number;
  /** DCS/SOS/PM/APC string sequences — ESC P/X/^/_ … ST. */
  readonly stringSequence: number;
  /** Other escapes and stray C0/C1 controls (excluding \n and \t). */
  readonly otherControl: number;
}

export interface SanitizedText {
  /** Inert text: no ESC, no C0/C1 controls except \n and \t. */
  readonly text: string;
  readonly neutralized: NeutralizedSequences;
}

export type SanitizeMode = "strip" | "caret";

const NONE: NeutralizedSequences = Object.freeze({
  csi: 0,
  osc: 0,
  oscHyperlink: 0,
  oscClipboard: 0,
  stringSequence: 0,
  otherControl: 0,
});

function blankNeutralized(): MutableNeutralized {
  return { csi: 0, osc: 0, oscHyperlink: 0, oscClipboard: 0, stringSequence: 0, otherControl: 0 };
}

/** Mutable accumulator shape for the sanitizer's internal counting. */
type MutableNeutralized = {
  -readonly [K in keyof NeutralizedSequences]: number;
};

function addNeutralized(
  into: NeutralizedSequences,
  from: NeutralizedSequences,
): NeutralizedSequences {
  return {
    csi: into.csi + from.csi,
    osc: into.osc + from.osc,
    oscHyperlink: into.oscHyperlink + from.oscHyperlink,
    oscClipboard: into.oscClipboard + from.oscClipboard,
    stringSequence: into.stringSequence + from.stringSequence,
    otherControl: into.otherControl + from.otherControl,
  };
}

function isC1(code: number): boolean {
  return code >= 0x80 && code <= 0x9f;
}

/**
 * Caret-render one consumed sequence as inert visible text: ESC → `^[`,
 * BEL → `^G`; every other control inside is dropped. The sequence becomes
 * literal text — legible forensics without any control function.
 */
function caretForm(raw: string): string {
  let out = "";
  for (const ch of raw) {
    const code = ch.charCodeAt(0);
    if (code === 0x1b) {
      out += "^[";
    } else if (code === 0x07) {
      out += "^G";
    } else if (code === 0x09 || (code >= 0x20 && !isC1(code))) {
      out += ch;
    }
    // remaining C0/C1 inside a consumed sequence are dropped
  }
  return out;
}

/** Strip the string terminator (BEL or ESC \) from a raw OSC/string body. */
function sequenceBody(raw: string): string {
  let body = raw.slice(2); // drop the two-character introducer (ESC ] / ESC P / …)
  if (body.endsWith("\x07")) {
    body = body.slice(0, -1);
  } else if (body.endsWith("\x1b\\")) {
    body = body.slice(0, -2);
  }
  return body;
}

/**
 * One-shot DSN-014 sanitizer for UNTRUSTED TEXT that will be rendered or
 * stored OUTSIDE the deliberate PTY viewport. In `strip` mode control
 * sequences vanish; in `caret` mode they render as visible caret text.
 * OSC 8 hyperlinks and OSC 52 clipboard requests are always neutralized.
 */
export function sanitizeTerminalOutput(
  input: string,
  options: { mode?: SanitizeMode } = {},
): SanitizedText {
  const mode = options.mode ?? "strip";
  const counts: MutableNeutralized = blankNeutralized();
  let out = "";
  let i = 0;
  const n = input.length;

  const findStringTerminator = (start: number): number => {
    // Index just past the ST (ESC \) or BEL, or -1 when unterminated.
    let j = start;
    while (j < n) {
      const code = input.charCodeAt(j);
      if (code === 0x07) {
        return j + 1;
      }
      if (code === 0x1b && input.charCodeAt(j + 1) === 0x5c) {
        return j + 2;
      }
      j += 1;
    }
    return -1;
  };

  while (i < n) {
    const code = input.charCodeAt(i);

    if (code === 0x1b) {
      const next = i + 1 < n ? input.charCodeAt(i + 1) : -1;
      if (next === -1) {
        counts.otherControl += 1;
        if (mode === "caret") {
          out += "^[";
        }
        i += 1;
        continue;
      }
      // CSI: ESC [ params(0x30-0x3F) intermediates(0x20-0x2F) final(0x40-0x7E)
      if (next === 0x5b) {
        let j = i + 2;
        while (j < n) {
          const c = input.charCodeAt(j);
          if ((c >= 0x40 && c <= 0x7e) || c === 0x1b) {
            break;
          }
          j += 1;
        }
        const complete = j < n && input.charCodeAt(j) !== 0x1b;
        const end = complete ? j + 1 : j;
        const raw = input.slice(i, end);
        counts.csi += 1;
        if (mode === "caret") {
          out += caretForm(raw);
        }
        i = end;
        continue;
      }
      // OSC: ESC ] … BEL | ST — classify hyperlink/clipboard requests.
      if (next === 0x5d) {
        const end = findStringTerminator(i + 2);
        const raw = end === -1 ? input.slice(i) : input.slice(i, end);
        counts.osc += 1;
        const body = sequenceBody(raw);
        if (body.startsWith("8;")) {
          counts.oscHyperlink += 1;
        }
        if (body.startsWith("52;")) {
          counts.oscClipboard += 1;
        }
        if (mode === "caret") {
          out += caretForm(raw);
        }
        i = end === -1 ? n : end;
        continue;
      }
      // DCS / SOS / PM / APC: ESC P, X, ^, _ … ST.
      if (next === 0x50 || next === 0x58 || next === 0x5e || next === 0x5f) {
        const end = findStringTerminator(i + 2);
        const raw = end === -1 ? input.slice(i) : input.slice(i, end);
        counts.stringSequence += 1;
        if (mode === "caret") {
          out += caretForm(raw);
        }
        i = end === -1 ? n : end;
        continue;
      }
      // nF escapes (ESC ( B …) and two-character escapes (ESC 7, ESC M, …).
      let j = i + 1;
      while (j < n && input.charCodeAt(j) >= 0x20 && input.charCodeAt(j) <= 0x2f) {
        j += 1;
      }
      const end = j < n ? j + 1 : j;
      counts.otherControl += 1;
      if (mode === "caret") {
        out += caretForm(input.slice(i, end));
      }
      i = end;
      continue;
    }

    if (code === 0x0a || code === 0x09) {
      out += input[i];
      i += 1;
      continue;
    }
    if (code === 0x0d) {
      // CR and CRLF normalize to LF (PTY line discipline output).
      if (input.charCodeAt(i + 1) === 0x0a) {
        i += 1;
      }
      out += "\n";
      i += 1;
      continue;
    }
    if (code < 0x20 || code === 0x7f || isC1(code)) {
      counts.otherControl += 1;
      i += 1;
      continue;
    }
    out += input[i];
    i += 1;
  }

  return { text: out, neutralized: Object.freeze(counts) };
}

/**
 * Stateful sanitizer for CHUNKED PTY output: a control sequence split across
 * two chunks is carried over instead of being half-stripped (a stateless
 * per-chunk strip could leak the tail of a split OSC into the projection).
 */
export function createChunkSanitizer(options: { mode?: SanitizeMode } = {}): {
  push(chunk: string): SanitizedText;
  flush(): SanitizedText;
} {
  const mode = options.mode ?? "strip";
  let carry = "";

  const findStringTerminator = (text: string, start: number): number => {
    // Index just past the ST (ESC \) or BEL, or -1 when unterminated.
    let j = start;
    while (j < text.length) {
      const code = text.charCodeAt(j);
      if (code === 0x07) {
        return j + 1;
      }
      if (code === 0x1b && text.charCodeAt(j + 1) === 0x5c) {
        return j + 2;
      }
      j += 1;
    }
    return -1;
  };

  return {
    push(chunk: string): SanitizedText {
      const joined = carry + chunk;
      let safe = joined.length;

      // Hold back a trailing bare ESC (or ESC + a byte that cannot complete
      // an ST): it may be the head of a sequence finished by the next chunk.
      const last = joined.charCodeAt(safe - 1);
      if (last === 0x1b) {
        safe -= 1;
      } else if (safe >= 2 && joined.charCodeAt(safe - 2) === 0x1b && last !== 0x5c) {
        safe -= 2;
      }

      // Hold back an UNTERMINATED string sequence (OSC/DCS/SOS/PM/APC): an
      // OSC split across chunks must not consume the chunk tail here and
      // leak its remainder next chunk as plain text.
      let j = 0;
      while (j < safe) {
        if (joined.charCodeAt(j) !== 0x1b) {
          j += 1;
          continue;
        }
        const next = joined.charCodeAt(j + 1);
        if (next === 0x5d || next === 0x50 || next === 0x58 || next === 0x5e || next === 0x5f) {
          const end = findStringTerminator(joined, j + 2);
          if (end === -1 || end > safe) {
            safe = j;
            break;
          }
          j = end;
          continue;
        }
        j += 2;
      }

      carry = joined.slice(safe);
      return sanitizeTerminalOutput(joined.slice(0, safe), { mode });
    },
    flush(): SanitizedText {
      const pending = carry;
      carry = "";
      return sanitizeTerminalOutput(pending, { mode });
    },
  };
}

// --- Privileged-boundary validation (REQ-SEC-005) --------------------------------

/** Untrusted spawn request shape (never executed as-is). */
export interface SpawnRequestInput {
  /** Program name (bare) or absolute path. Never a shell string. */
  readonly program: string;
  /** Arguments passed as argv ELEMENTS — data, never re-parsed. */
  readonly args?: readonly string[];
  /** Working directory; anchored under the policy's path roots. */
  readonly cwd?: string;
  /** Environment overrides; names validated, values never recorded. */
  readonly env?: Readonly<Record<string, string>>;
  /** Requested session title; control-neutralized before any projection. */
  readonly title?: string;
}

export interface SpawnPathPolicy {
  /** Roots an absolute program path may resolve under. Default /bin,/usr/bin. */
  readonly allowedProgramRoots?: readonly string[];
  /** Bare program names allowed without a root-anchored path. */
  readonly allowedPrograms?: readonly string[];
  /** Roots a cwd may resolve under (REQUIRED — no implicit roots). */
  readonly allowedPathRoots: readonly string[];
}

/** A bound spawn request: the ONLY shape a provider may be handed. */
export interface ValidatedSpawnRequest {
  /** Anchored absolute program path, or the bare allowlisted name. */
  readonly program: string;
  readonly args: readonly string[];
  /** Anchored absolute cwd (bound — never the caller's raw string). */
  readonly cwd: string;
  readonly env: Readonly<Record<string, string>>;
  /** Control-neutralized requested title, or null. */
  readonly title: string | null;
}

/** Shell metacharacters a program path may never contain. */
const PROGRAM_METACHARACTERS = new Set([
  ";",
  "|",
  "&",
  "$",
  "(",
  ")",
  "<",
  ">",
  "`",
  '"',
  "'",
  "*",
  "?",
  "~",
  "!",
  "{",
  "}",
  "[",
  "]",
  "\n",
  "\r",
  "\0",
  " ",
]);

/** True when the string carries a C0 control (except tab) or DEL. */
function hasControlChar(value: string): boolean {
  for (const ch of value) {
    const code = ch.charCodeAt(0);
    if ((code < 0x20 && code !== 0x09) || code === 0x7f) {
      return true;
    }
  }
  return false;
}

/**
 * Normalize + anchor a path lexically: `..` segments are resolved and the
 * result must land inside one of the allowed roots. Purely lexical — no
 * filesystem access, so validation is deterministic.
 */
export function anchorPath(input: string, allowedRoots: readonly string[]): string {
  if (input.length === 0) {
    throw new TerminalBoundaryError("path", "path_traversal", "empty path parameter");
  }
  if (input.includes("\0")) {
    throw new TerminalBoundaryError("path", "path_traversal", "NUL byte in path parameter");
  }
  if (!input.startsWith("/")) {
    throw new TerminalBoundaryError(
      "path",
      "path_not_absolute",
      `path parameter must be absolute to anchor: ${JSON.stringify(input.slice(0, 32))}`,
    );
  }
  const parts: string[] = [];
  for (const segment of input.split("/")) {
    if (segment === "" || segment === ".") {
      continue;
    }
    if (segment === "..") {
      if (parts.length === 0) {
        throw new TerminalBoundaryError(
          "path",
          "path_traversal",
          `'${input.slice(0, 64)}' escapes every allowed root`,
        );
      }
      parts.pop();
      continue;
    }
    parts.push(segment);
  }
  const resolved = `/${parts.join("/")}`;
  const inside = allowedRoots.some((root) => {
    const normalizedRoot = root.endsWith("/") ? root.slice(0, -1) : root;
    return resolved === normalizedRoot || resolved.startsWith(`${normalizedRoot}/`);
  });
  if (!inside) {
    throw new TerminalBoundaryError(
      "path",
      "path_outside_allowed_roots",
      `anchored path '${resolved}' is outside the declared roots [${allowedRoots.join(", ")}]`,
    );
  }
  return resolved;
}

/**
 * Validate + bind a spawn request at the privileged boundary. Either a
 * ValidatedSpawnRequest is produced or TerminalBoundaryError throws — no
 * partially-bound request ever reaches a provider.
 */
export function validateSpawnRequest(
  input: SpawnRequestInput,
  policy: SpawnPathPolicy,
): ValidatedSpawnRequest {
  // --- program (argv locus) ---
  const program = input.program;
  if (program.length === 0) {
    throw new TerminalBoundaryError("argv", "empty_program", "program is required");
  }
  for (const ch of program) {
    if (PROGRAM_METACHARACTERS.has(ch)) {
      throw new TerminalBoundaryError(
        "argv",
        "shell_metacharacter_in_program",
        `program contains metacharacter ${JSON.stringify(ch)} — spawn binds argv, never a shell string`,
      );
    }
  }
  let boundProgram: string;
  if (program.includes("/")) {
    if (!program.startsWith("/")) {
      throw new TerminalBoundaryError(
        "argv",
        "relative_program_path",
        `relative program path '${program}' is refused; use a bare allowlisted name or an absolute path`,
      );
    }
    boundProgram = anchorPath(program, policy.allowedProgramRoots ?? ["/bin", "/usr/bin"]);
  } else {
    if (policy.allowedPrograms !== undefined && !policy.allowedPrograms.includes(program)) {
      throw new TerminalBoundaryError(
        "argv",
        "program_not_in_allowlist",
        `bare program '${program}' is not in the declared allowlist`,
      );
    }
    boundProgram = program;
  }

  // --- args (bound as DATA: shell metacharacters are inert; control bytes refused) ---
  const args = input.args ?? [];
  for (const arg of args) {
    if (hasControlChar(arg)) {
      throw new TerminalBoundaryError(
        "argv",
        "control_char_in_argv",
        "argument carries a control byte (C0/DEL); it cannot be bound as terminal data",
      );
    }
  }

  // --- cwd (path locus, anchored) ---
  const cwd = anchorPath(input.cwd ?? "/", policy.allowedPathRoots);

  // --- env (name-validated; keys-only provenance downstream) ---
  const env: Record<string, string> = {};
  if (input.env !== undefined) {
    if (typeof input.env !== "object" || input.env === null) {
      throw new TerminalBoundaryError("env", "env_not_object", "env overrides must be an object");
    }
    for (const [name, value] of Object.entries(input.env)) {
      if (!/^[A-Za-z_][A-Za-z0-9_]*$/.test(name)) {
        throw new TerminalBoundaryError(
          "env",
          "env_name_invalid",
          `env name ${JSON.stringify(name.slice(0, 32))} is not a valid identifier`,
        );
      }
      if (hasControlChar(value) || value.includes("\n")) {
        throw new TerminalBoundaryError(
          "env",
          "env_value_control_char",
          `env value for '${name}' carries a control byte or newline`,
        );
      }
      env[name] = value;
    }
  }

  // --- title (automation locus: neutralized before it can be projected) ---
  const title =
    input.title === undefined ? null : sanitizeTerminalOutput(input.title, { mode: "strip" }).text;

  return Object.freeze({
    program: boundProgram,
    args: Object.freeze([...args]),
    cwd,
    env: Object.freeze({ ...env }),
    title,
  });
}

// --- RPC payload boundary (REQ-SEC-005: RPC payloads validated/bound) -------------

export interface RpcPayloadPolicy {
  /** Allowed top-level keys (allowlist — unknown keys refuse). */
  readonly allowedKeys: readonly string[];
  /** Per-string value byte cap. Default 4096. */
  readonly maxValueBytes?: number;
}

export interface BoundRpcPayload {
  readonly values: Readonly<Record<string, string>>;
  /** Sanitizer counts aggregated over every bound string value. */
  readonly neutralized: NeutralizedSequences;
}

/**
 * Validate + bind an RPC payload destined for terminal-adjacent surfaces.
 * Strings are control-neutralized at the boundary: an OSC/OSC 52/OSC 8
 * sequence riding an RPC value cannot reach any host-terminal output.
 */
export function validateRpcPayload(payload: unknown, policy: RpcPayloadPolicy): BoundRpcPayload {
  if (typeof payload !== "object" || payload === null || Array.isArray(payload)) {
    throw new TerminalBoundaryError(
      "rpc-payload",
      "payload_not_object",
      "RPC payload must be an object",
    );
  }
  const maxValueBytes = policy.maxValueBytes ?? 4096;
  const values: Record<string, string> = {};
  let totals = blankNeutralized();
  for (const [key, value] of Object.entries(payload as Record<string, unknown>)) {
    if (!policy.allowedKeys.includes(key)) {
      throw new TerminalBoundaryError(
        "rpc-payload",
        "payload_key_not_allowed",
        `payload key '${key}' is not in the declared allowlist`,
      );
    }
    if (typeof value !== "string") {
      throw new TerminalBoundaryError(
        "rpc-payload",
        "payload_value_not_string",
        `payload value for '${key}' must be a string`,
      );
    }
    if (new TextEncoder().encode(value).length > maxValueBytes) {
      throw new TerminalBoundaryError(
        "rpc-payload",
        "payload_too_large",
        `payload value for '${key}' exceeds ${maxValueBytes} bytes`,
      );
    }
    const sanitized = sanitizeTerminalOutput(value, { mode: "strip" });
    totals = addNeutralized(totals, sanitized.neutralized);
    values[key] = sanitized.text;
  }
  return Object.freeze({ values: Object.freeze(values), neutralized: Object.freeze(totals) });
}

// --- Process provider port ---------------------------------------------------------

export type TerminalProviderKind = "bun-pty" | "bun-pipe" | "fake" | "alternate";

/** Lifecycle facts delivered by the provider when the child terminates. */
export interface TerminalExitInfo {
  /** Process exit code; null when terminated by a signal. */
  readonly exitCode: number | null;
  /** Terminating signal name, or null. */
  readonly signal: string | null;
}

export interface TerminalProcessHandle {
  /** OS pid when the provider can observe one; null otherwise. */
  readonly pid: number | null;
  /** Raw bytes INTO the deliberate PTY (input ownership lives viewport-side). */
  write(data: string | Uint8Array): void;
  resize(cols: number, rows: number): void;
  kill(signal?: "SIGTERM" | "SIGKILL"): void;
}

export interface TerminalProcessHandlers {
  onData(chunk: Uint8Array): void;
  onExit(info: TerminalExitInfo): void;
}

/**
 * The adapter seam (REQ-BUN-004): the qualified native path is Bun's
 * process/PTY facility; an alternate stack would implement this same port.
 */
export interface TerminalProcessProvider {
  readonly kind: TerminalProviderKind;
  spawn(request: ValidatedSpawnRequest, handlers: TerminalProcessHandlers): TerminalProcessHandle;
}

// --- Native Bun providers (the QUALIFIED path, probed per T03 discipline) ----------

/**
 * Real Bun PTY provider: `Bun.spawn` with the `terminal` option — the child
 * sees a true terminal (isTTY), raw bytes flow both ways, resize reaches the
 * child (winsize + SIGWINCH, verified by the T13 probes). Exit detection
 * keys on `proc.exited` — NOT on the terminal `exit` callback, which reports
 * PTY-lifecycle status, not the subprocess exit. The PTY master is closed
 * once the child is reaped.
 */
export function bunPtyProcessProvider(
  options: { cols?: number; rows?: number; terminalName?: string } = {},
): TerminalProcessProvider {
  return {
    kind: "bun-pty",
    spawn(
      request: ValidatedSpawnRequest,
      handlers: TerminalProcessHandlers,
    ): TerminalProcessHandle {
      // Bun.spawn throws synchronously on refusal (ENOTSUP off POSIX, missing
      // program) — mapped to a failed session by the caller.
      const proc = Bun.spawn([request.program, ...request.args], {
        cwd: request.cwd,
        env: { ...request.env },
        terminal: {
          cols: options.cols ?? 80,
          rows: options.rows ?? 24,
          name: options.terminalName ?? "xterm-256color",
          data: (_terminal, data) => handlers.onData(data),
        },
      });
      const terminal = proc.terminal;
      if (terminal === undefined) {
        // Defensive: the `terminal` option guarantees an attached PTY.
        proc.kill("SIGTERM");
        throw new Error("bun-pty provider: Bun.spawn returned no terminal");
      }
      void proc.exited.then((code) => {
        handlers.onExit({
          exitCode: code,
          signal: proc.signalCode,
        });
        if (!terminal.closed) {
          terminal.close();
        }
      });
      return {
        pid: proc.pid,
        write: (data) => {
          terminal.write(data);
        },
        resize: (cols, rows) => {
          terminal.resize(cols, rows);
        },
        kill: (signal) => {
          proc.kill(signal ?? "SIGTERM");
        },
      };
    },
  };
}

/**
 * Real Bun pipe provider (no PTY): for non-interactive children whose
 * lifecycle/input/exit the session binds identically. `Bun.spawn` with
 * pipes is the T03-qualified spawn shape.
 */
export function bunPipeProcessProvider(): TerminalProcessProvider {
  return {
    kind: "bun-pipe",
    spawn(
      request: ValidatedSpawnRequest,
      handlers: TerminalProcessHandlers,
    ): TerminalProcessHandle {
      const proc = Bun.spawn([request.program, ...request.args], {
        cwd: request.cwd,
        env: { ...request.env },
        stdin: "pipe",
        stdout: "pipe",
        stderr: "pipe",
      });
      // Drain stderr (never ingested — children may write unbounded noise);
      // stdout feeds the session's sanitized projection; exit is delivered
      // AFTER the output so a scripted observe-then-exit ordering holds.
      void new Response(proc.stderr).text().catch(() => undefined);
      void new Response(proc.stdout)
        .text()
        .then((text) => {
          if (text.length > 0) {
            handlers.onData(new TextEncoder().encode(text));
          }
        })
        .catch(() => undefined)
        .then(() => proc.exited)
        .then((code) => {
          handlers.onExit({ exitCode: code, signal: proc.signalCode });
        })
        .catch(() => {
          handlers.onExit({ exitCode: null, signal: "SIGPIPE" });
        });
      return {
        pid: proc.pid,
        write: (data) => {
          proc.stdin.write(data);
        },
        resize: () => {
          /* pipes have no winsize; the session still records the request */
        },
        kill: (signal) => {
          proc.kill(signal ?? "SIGTERM");
        },
      };
    },
  };
}

// --- Deterministic fake provider (unit tests; never a production path) ------------

export interface FakeProviderScriptStep {
  /** Data chunk delivered to the session (utf-8 text, encoded here). */
  readonly emit?: string;
  /** Child exit delivered after every emit ran. */
  readonly exit?: TerminalExitInfo;
}

/** A scripted, deterministic provider with an inspectable call log. */
export class FakeProcessProvider implements TerminalProcessProvider {
  readonly kind = "fake" as const;
  readonly spawnedRequests: ValidatedSpawnRequest[] = [];
  readonly writes: string[] = [];
  readonly resizes: Array<{ cols: number; rows: number }> = [];
  readonly kills: string[] = [];
  spawnCount = 0;

  private handlers: TerminalProcessHandlers | null = null;
  private handle: TerminalProcessHandle | null = null;
  private nextPid = 42_000;

  spawn(request: ValidatedSpawnRequest, handlers: TerminalProcessHandlers): TerminalProcessHandle {
    this.spawnCount += 1;
    this.spawnedRequests.push(request);
    this.handlers = handlers;
    const pid = this.nextPid;
    this.nextPid += 1;
    this.handle = {
      pid,
      write: (data) => {
        this.writes.push(typeof data === "string" ? data : new TextDecoder().decode(data));
      },
      resize: (cols, rows) => {
        this.resizes.push({ cols, rows });
      },
      kill: (signal) => {
        this.kills.push(signal ?? "SIGTERM");
      },
    };
    return this.handle;
  }

  /** Deliver data/exit exactly as scripted (deterministic tests drive this). */
  runScript(steps: readonly FakeProviderScriptStep[]): void {
    if (this.handlers === null) {
      throw new Error("fake provider has no live spawn to script");
    }
    for (const step of steps) {
      if (step.emit !== undefined) {
        this.handlers.onData(new TextEncoder().encode(step.emit));
      }
      if (step.exit !== undefined) {
        this.handlers.onExit(step.exit);
      }
    }
  }

  /** A provider that refuses synchronously (e.g. ENOTSUP, missing program). */
  static refusing(error: Error): TerminalProcessProvider {
    return {
      kind: "fake",
      spawn: () => {
        throw error;
      },
    };
  }

  get lastHandle(): TerminalProcessHandle | null {
    return this.handle;
  }
}

// --- Session state (noncanonical) + provenance -------------------------------------

export type TerminalStatus = "running" | "exited" | "failed" | "cancelled";

export type TerminalLifecycleEventKind =
  | "spawn_requested"
  | "spawn_bound"
  | "spawn_refused"
  | "spawned"
  | "input"
  | "resize"
  | "output"
  | "exit"
  | "cancel_requested";

export interface TerminalLifecycleEvent {
  readonly atMs: number;
  readonly kind: TerminalLifecycleEventKind;
  /** Small typed detail record; input CONTENT is never recorded. */
  readonly detail: Readonly<Record<string, string | number>>;
}

export interface TerminalProvenanceRecord {
  readonly sessionId: string;
  readonly providerKind: TerminalProviderKind;
  /** Bound argv exactly as handed to the provider (program first). */
  readonly argv: readonly string[];
  /** Anchored cwd. */
  readonly cwd: string;
  /** Env NAMES only — values are never recorded (secret discipline). */
  readonly envKeys: readonly string[];
  readonly pid: number | null;
  readonly spawnedAtMs: number;
  readonly exitedAtMs: number | null;
  readonly exitCode: number | null;
  readonly signal: string | null;
  readonly cancelled: boolean;
  readonly cancelReason: string | null;
}

/**
 * The publication shape for the shell's terminal lower surface
 * (shell.ts LOWER_VIEW_PROVIDERS: view "terminal" is gated by a snapshot
 * provider field literally named "terminal"). Everything here is safe to
 * render OUTSIDE the PTY viewport: sanitized text only.
 */
export interface TerminalSessionProjection {
  readonly sessionId: string;
  readonly status: TerminalStatus;
  readonly title: string;
  readonly cols: number;
  readonly rows: number;
  /** Sanitized scrollback (oldest first), capped at the host's maxScrollback. */
  readonly scrollback: readonly string[];
  readonly provenance: TerminalProvenanceRecord;
  readonly neutralized: NeutralizedSequences;
  /** Monotonic projection generation — churn is observable without canonical effects. */
  readonly generation: number;
}

export interface TerminalSessionHostConfig {
  /** Path anchoring policy (REQUIRED — no implicit roots). */
  readonly pathPolicy: SpawnPathPolicy;
  /** Scrollback cap. Default 1000 lines. */
  readonly maxScrollback?: number;
  /** Initial viewport size. Default 80x24. */
  readonly dimensions?: { cols: number; rows: number };
  /** Deterministic clock seam. Default Date.now. */
  readonly nowMs?: () => number;
}

let sessionCounter = 0;

/**
 * One embedded terminal session: validated spawn → live child → exit/cancel,
 * tracked entirely in noncanonical session state with full provenance.
 * Exit, failure, and cancellation are DISTINCT terminal outcomes (never a
 * boolean); a refused spawn throws before any session exists.
 */
export class TerminalSession {
  readonly sessionId: string;
  private readonly config: TerminalSessionHostConfig;
  private readonly chunkSanitizer = createChunkSanitizer({ mode: "strip" });
  private readonly decoder = new TextDecoder("utf-8", { fatal: false });

  private statusValue: TerminalStatus;
  private titleValue: string;
  private colsValue: number;
  private rowsValue: number;
  private readonly scrollbackLines: string[] = [];
  private neutralizedTotal = blankNeutralized();
  private readonly lifecycleLog: TerminalLifecycleEvent[] = [];
  private provenanceValue: TerminalProvenanceRecord;
  private generationValue = 0;
  private cancelReasonValue: string | null = null;
  private handle: TerminalProcessHandle | null = null;

  constructor(
    input: SpawnRequestInput,
    provider: TerminalProcessProvider,
    config: TerminalSessionHostConfig,
  ) {
    this.sessionId = `terminal-session-${++sessionCounter}`;
    this.config = config;
    this.colsValue = config.dimensions?.cols ?? 80;
    this.rowsValue = config.dimensions?.rows ?? 24;
    const nowMs = config.nowMs ?? (() => Date.now());

    this.log("spawn_requested", { requestedProgram: input.program });

    // The privileged boundary: validation happens BEFORE any provider call.
    const bound = validateSpawnRequest(input, config.pathPolicy);

    this.statusValue = "running";
    this.titleValue = bound.title ?? "";
    this.log("spawn_bound", { boundProgram: bound.program, argCount: bound.args.length });

    this.provenanceValue = Object.freeze({
      sessionId: this.sessionId,
      providerKind: provider.kind,
      argv: Object.freeze([bound.program, ...bound.args]),
      cwd: bound.cwd,
      envKeys: Object.freeze(Object.keys(bound.env)),
      pid: null,
      spawnedAtMs: nowMs(),
      exitedAtMs: null,
      exitCode: null,
      signal: null,
      cancelled: false,
      cancelReason: null,
    });

    try {
      this.handle = provider.spawn(bound, {
        onData: (chunk) => this.ingest(chunk),
        onExit: (info) => this.finish(info),
      });
    } catch (error) {
      this.statusValue = "failed";
      this.log("spawn_refused", { reason: error instanceof Error ? error.name : "unknown" });
      this.bumpGeneration();
      return;
    }

    this.provenanceValue = Object.freeze({ ...this.provenanceValue, pid: this.handle.pid });
    this.log("spawned", { pid: this.handle.pid ?? -1 });
    this.bumpGeneration();
  }

  get status(): TerminalStatus {
    return this.statusValue;
  }

  get provenance(): TerminalProvenanceRecord {
    return this.provenanceValue;
  }

  get events(): readonly TerminalLifecycleEvent[] {
    return this.lifecycleLog;
  }

  get dimensions(): { cols: number; rows: number } {
    return { cols: this.colsValue, rows: this.rowsValue };
  }

  /** True while the session can accept input; NOT a success/failure bit. */
  isRunning(): boolean {
    return this.statusValue === "running";
  }

  /** Send raw bytes into the deliberate PTY (input ownership is explicit). */
  write(data: string | Uint8Array): void {
    if (!this.isRunning() || this.handle === null) {
      throw new TerminalBoundaryError(
        "input",
        "not_running",
        `session ${this.sessionId} is ${this.statusValue}`,
      );
    }
    const bytes = typeof data === "string" ? new TextEncoder().encode(data) : data;
    this.handle.write(data);
    this.log("input", { bytes: bytes.length });
    this.bumpGeneration();
  }

  /** Resize the PTY (SIGWINCH reaches the child); session projection only. */
  resize(cols: number, rows: number): void {
    if (
      !Number.isInteger(cols) ||
      !Number.isInteger(rows) ||
      cols < 2 ||
      cols > 500 ||
      rows < 1 ||
      rows > 500
    ) {
      throw new TerminalBoundaryError(
        "dimensions",
        "dimension_out_of_range",
        `resize ${cols}x${rows} outside the 2..500 x 1..500 bounds`,
      );
    }
    if (!this.isRunning() || this.handle === null) {
      throw new TerminalBoundaryError(
        "session-state",
        "session_not_active",
        `session ${this.sessionId} is ${this.statusValue}`,
      );
    }
    this.colsValue = cols;
    this.rowsValue = rows;
    this.handle.resize(cols, rows);
    this.log("resize", { cols, rows });
    this.bumpGeneration();
  }

  /**
   * Cancel the child (SIGTERM): a DISTINCT terminal outcome — never recorded
   * as a plain exit. The reason is retained in provenance. Idempotent.
   */
  cancel(reason = "operator requested"): void {
    if (!this.isRunning() || this.handle === null) {
      return;
    }
    this.cancelReasonValue = reason;
    this.provenanceValue = Object.freeze({
      ...this.provenanceValue,
      cancelled: true,
      cancelReason: reason,
    });
    this.log("cancel_requested", { reason });
    this.handle.kill("SIGTERM");
    this.bumpGeneration();
  }

  get cancelReason(): string | null {
    return this.cancelReasonValue;
  }

  /** The publication projection — sanitized text only, frozen per generation. */
  project(): TerminalSessionProjection {
    return Object.freeze({
      sessionId: this.sessionId,
      status: this.statusValue,
      title: this.titleValue,
      cols: this.colsValue,
      rows: this.rowsValue,
      scrollback: Object.freeze([...this.scrollbackLines]),
      provenance: this.provenanceValue,
      neutralized: Object.freeze({ ...this.neutralizedTotal }),
      generation: this.generationValue,
    });
  }

  private ingest(chunk: Uint8Array): void {
    const text = this.decoder.decode(chunk, { stream: true });
    this.accumulate(this.chunkSanitizer.push(text));
  }

  private finish(info: TerminalExitInfo): void {
    // Flush the streaming decoder and any carried partial sequence first.
    const decodedTail = this.decoder.decode();
    if (decodedTail.length > 0) {
      this.accumulate(sanitizeTerminalOutput(decodedTail, { mode: "strip" }));
    }
    this.accumulate(this.chunkSanitizer.flush());

    const status: TerminalStatus = this.provenanceValue.cancelled
      ? "cancelled"
      : info.signal !== null
        ? "failed"
        : "exited";
    this.statusValue = status;
    const nowMs = this.config.nowMs ?? (() => Date.now());
    this.provenanceValue = Object.freeze({
      ...this.provenanceValue,
      exitedAtMs: nowMs(),
      exitCode: info.exitCode,
      signal: info.signal,
    });
    this.log("exit", {
      exitCode: info.exitCode ?? -1,
      signal: info.signal ?? "none",
      status,
    });
    this.bumpGeneration();
  }

  private accumulate(sanitized: SanitizedText): void {
    if (sanitized.text.length > 0) {
      for (const line of sanitized.text.split("\n")) {
        this.scrollbackLines.push(line);
      }
      while (this.scrollbackLines.length > (this.config.maxScrollback ?? 1000)) {
        this.scrollbackLines.shift();
      }
      this.log("output", { chars: sanitized.text.length });
    }
    this.neutralizedTotal = addNeutralized(this.neutralizedTotal, sanitized.neutralized);
    this.bumpGeneration();
  }

  private log(kind: TerminalLifecycleEventKind, detail: Record<string, string | number>): void {
    this.lifecycleLog.push({
      atMs: (this.config.nowMs ?? (() => Date.now()))(),
      kind,
      detail: Object.freeze(detail),
    });
  }

  private bumpGeneration(): void {
    this.generationValue += 1;
  }
}

// --- Shell wiring seam (read-only counterpart to shell.ts LOWER_VIEW_PROVIDERS) ----

/**
 * Publish a terminal projection as a snapshot's `terminal` provider field so
 * shell.ts `availableViews` reports the terminal lower surface AVAILABLE
 * (provider-gated per the REQ-AFF-004 direction). This adds one OWN property
 * to the snapshot object without touching canonical fields — the orchestrator
 * wires the live projection here; without publication the view stays ABSENT
 * with its inspectable reason.
 */
export function publishTerminalProvider<S extends object>(
  snapshot: S,
  projection: TerminalSessionProjection,
): S & { terminal: TerminalSessionProjection } {
  return Object.freeze({ ...snapshot, terminal: projection });
}

// --- Terminal automation (REQ-SEC-005: terminal automation validated/bound) --------

export type TerminalAutomation =
  | { readonly kind: "title"; readonly value: string }
  | { readonly kind: "bell" };

export interface AppliedAutomation {
  readonly kind: "title" | "bell";
  /** Bound (control-neutralized) title, or null for bell. */
  readonly value: string | null;
  readonly neutralized: NeutralizedSequences;
}

/**
 * Validate + bind a terminal automation request. Titles are sanitized
 * through the DSN-014 boundary: an OSC payload in an automation value can
 * never ride into any host-visible surface.
 */
export function bindTerminalAutomation(automation: TerminalAutomation): AppliedAutomation {
  if (automation.kind === "bell") {
    return Object.freeze({ kind: "bell", value: null, neutralized: NONE });
  }
  if (automation.kind === "title") {
    const sanitized = sanitizeTerminalOutput(automation.value, { mode: "strip" });
    return Object.freeze({
      kind: "title",
      value: sanitized.text,
      neutralized: sanitized.neutralized,
    });
  }
  throw new TerminalBoundaryError(
    "automation",
    "unknown_automation_kind",
    "unknown terminal automation kind",
  );
}
