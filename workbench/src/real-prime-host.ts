/**
 * Production Prime host binding REAL child processes via `Bun.spawn` (plan
 * Phase C2, workbench-intent-closure.md §4; REQ-GOAL-003, REQ-PRIME-001..010,
 * REQ-OPS-001, REQ-SEC-005).
 *
 * Composition, not reimplementation (D2): every session/mailbox/result-contract
 * BOOKKEEPING call here delegates to an internal `PrimeHostAdapter` (the now
 * explicitly-declared in-memory test double, `prime-host.ts`) — this file's
 * own code covers ONLY what the fake never had: actually launching
 * `prime-agent` and observing what it does.
 *
 * `spawnChild()` stays synchronous and nonblocking (REQ-PRIME-003): the
 * admission handle is minted and returned BEFORE the real process's stdout
 * is ever read. A background task per child parses the exact newline-JSON
 * event protocol T24's gated battery already proved (`prime-fresh-target.test.ts`:
 * `{type:"session", id}` names the real session, `{type:"message_end",
 * message:{role:"assistant", content}}` carries assistant text) and folds any
 * observed text into `receiveMailboxMessage` — mailbox text is communication,
 * NEVER completion (REQ-PRIME-005): only an explicit, schema-valid
 * `PrimeResultContract`, delivered after the process exits, calls
 * `deliverResultContract`. Because both calls route through the SAME
 * `PrimeHostAdapter` bookkeeping the fake uses, "mailbox != completion" is
 * enforced by the identical code path for a real process as for the fake —
 * not a second, parallel implementation of that invariant.
 *
 * PRIVILEGED BOUNDARY (REQ-SEC-005, D3): every real spawn is bound through
 * `terminal-session.ts`'s `validateSpawnRequest`/`anchorPath` (T13) for the
 * program/argv/cwd locus — `this.binary` is validated ONCE at construction
 * from `config.prime_binary` (an operator value; the plan explicitly forbids
 * the T24 gated-battery's hardcoded path migrating into production, so
 * `QUALIFIED_PRIME_BINARY` is never imported here) and is never re-resolved
 * per call. Env is constructed directly rather than through
 * `validateSpawnRequest`'s env validator: that validator refuses ANY
 * path-shaped env value outright (`env_value_path_separator`), which is
 * correct for terminal sessions (env can arrive from untrusted RPC input)
 * but would incorrectly reject this host's own self-constructed
 * `PRIME_AGENT_CODING_AGENT_DIR` value — nothing here is attacker-controlled.
 * `buildEnv()` still strips `LD_*`/`DYLD_*` unconditionally as defense in
 * depth, matching T13's loader-injection refusal in spirit.
 */

import { existsSync, mkdirSync, readdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import type {
  PrimeAdmissionHandle,
  PrimeContinuationRequest,
  PrimeContinuationResult,
  PrimeHostPort,
  PrimeMailboxMessage,
  PrimeResultContract,
  PrimeSessionState,
  PrimeSpawnRequest,
  PrimeStopOutcome,
} from "./prime-host";
import { PrimeHostAdapter } from "./prime-host";
import { type SpawnPathPolicy, validateSpawnRequest } from "./terminal-session";

// Re-exported for source compatibility: this type used to be declared here
// (plan Phase D); it now lives in prime-host.ts so PrimeHostAdapter (the
// in-memory test double) can report the same shape (plan P1).
export type { PrimeStopOutcome } from "./prime-host";

const QUALIFIED_MANIFEST_SUFFIX = ".qualified.json";

/**
 * The durable, self-written record of one admitted spawn (plan Phase D,
 * REQ-PRIME-010) — written synchronously at spawn time, BEFORE this file's
 * own process can die, so it survives this process's death. This is the
 * ENTIRE qualified-record vocabulary: a fresh `RealPrimeHost` reattaches
 * from these files and NOTHING else — never from directory presence, a
 * prior host's memory, or inferred liveness.
 */
interface QualifiedSpawnManifest {
  readonly session_id: string;
  readonly parent_session_id: string | null;
  readonly model: string;
  readonly contract_path: string;
  readonly pid: number;
  readonly spawned_at: string;
  /**
   * The REAL prime-agent session id (plan P1 bugfix): prime-agent mints its
   * OWN session identity, reported in the first `{"type":"session","id":...}`
   * stdout line — it is NOT this workbench's `session_id`. `--resume` only
   * accepts prime-agent's own id ("Error: No session found matching '<id>'"
   * otherwise, verified against the real binary). Additive/optional: absent
   * until the first turn's stdout has been parsed (or on a manifest written
   * before this field existed) — a fresh host honestly cannot resume such a
   * session until a turn completes and this is learned.
   */
  readonly prime_session_id?: string | undefined;
}

function isQualifiedSpawnManifest(value: unknown): value is QualifiedSpawnManifest {
  if (typeof value !== "object" || value === null) return false;
  const v = value as Record<string, unknown>;
  return (
    typeof v["session_id"] === "string" &&
    (v["parent_session_id"] === null || typeof v["parent_session_id"] === "string") &&
    typeof v["model"] === "string" &&
    typeof v["contract_path"] === "string" &&
    typeof v["pid"] === "number" &&
    typeof v["spawned_at"] === "string" &&
    (v["prime_session_id"] === undefined || typeof v["prime_session_id"] === "string")
  );
}

/**
 * Parse prime-agent's OWN session id from a child's stdout (plan P1
 * bugfix): the first `{"type":"session","id":"..."}` newline-JSON event.
 * Distinct from `parseAssistantText` — this walks the SAME protocol for a
 * different field, and is exported for direct unit testing.
 */
export function parsePrimeSessionId(stdout: string): string | null {
  for (const line of stdout.split("\n")) {
    try {
      const event = JSON.parse(line) as { type?: string; id?: string };
      if (event.type === "session" && typeof event.id === "string" && event.id.length > 0) {
        return event.id;
      }
    } catch {
      /* non-JSON line — never fatal, same discipline as parseAssistantText */
    }
  }
  return null;
}

/**
 * Whether a process is still alive, checked via signal 0 (no signal is
 * actually delivered — this only probes existence/permission). ESRCH means
 * the pid is gone; EPERM means it exists but we cannot signal it (still
 * alive); any other failure is treated as "not observably alive" rather than
 * risking a false "running" claim.
 */
export function isPidAlive(pid: number): boolean {
  try {
    process.kill(pid, 0);
    return true;
  } catch (error) {
    return (error as NodeJS.ErrnoException).code === "EPERM";
  }
}

/** One session this host found a qualified manifest for but could not honestly mark running or completed. */
export interface StaleQualifiedRecord {
  readonly session_id: string;
  readonly reason: "process_not_found_no_result_contract" | "contract_path_present_but_unreadable";
}

/**
 * Reconstruct the exact `PrimeResultContract` a completed session's evidence
 * file implies — the SAME derivation `RealPrimeHost.watch()` uses inline for
 * a process it is still directly observing, factored out so a cold reattach
 * (reading the file after the fact, with no live process) derives identically
 * (D2: one derivation, not two).
 */
function contractFromEvidenceRecord(
  sessionId: string,
  contractPath: string,
  evidence: unknown,
): PrimeResultContract {
  const record = evidence as { exit_code?: unknown; completed_at?: unknown };
  const exitCode = typeof record.exit_code === "number" ? record.exit_code : null;
  return {
    session_id: sessionId,
    schema_valid: exitCode !== null,
    outcome: exitCode === null ? "inconclusive" : exitCode === 0 ? "pass" : "fail",
    evidence_path: contractPath,
    completed_at:
      typeof record.completed_at === "string" ? record.completed_at : new Date().toISOString(),
  };
}

export class RealPrimeHostSpawnError extends Error {
  constructor(message: string) {
    super(message);
    this.name = "RealPrimeHostSpawnError";
  }
}

export interface RealPrimeHostOptions {
  /** Absolute path to the operator-qualified prime-agent binary (config.prime_binary). No default — never hardcoded here. */
  readonly binary: string;
  /** An isolated, workbench-owned agent directory (PRIME_AGENT_CODING_AGENT_DIR). Never the operator's real ~/.prime. */
  readonly agentDir: string;
  /** An isolated session directory (--session-dir). */
  readonly sessionDir: string;
  /** Working directory bound for every spawned child (--cwd). Defaults to sessionDir. */
  readonly workRoot?: string | undefined;
  /**
   * An additional allowed cwd root for WORK-MODE children (plan P1,
   * `PrimeSpawnRequest.workMode.cwd` — typically the operator's real project
   * root, `config.project_root`). Purely additive to the T13 boundary's
   * `allowedPathRoots`: omitted, a work-mode spawn whose `cwd` is not
   * `workRoot`/`sessionDir`/`agentDir` is refused exactly as it was before
   * this option existed (never bypassed, only extended).
   */
  readonly projectRoot?: string | undefined;
  /** Wall-clock ceiling for one child run before it is killed (ms). Default 10 minutes. */
  readonly timeoutMs?: number | undefined;
}

interface RealChildEvent {
  readonly type?: string;
  readonly id?: string;
  readonly message?: {
    readonly role?: string;
    readonly content?: ReadonlyArray<{ type: string; text?: string }>;
  };
}

function parseAssistantText(stdout: string): string | null {
  let finalText: string | null = null;
  for (const line of stdout.split("\n")) {
    try {
      const event = JSON.parse(line) as RealChildEvent;
      if (event.type === "message_end" && event.message?.role === "assistant") {
        const text = (event.message.content ?? [])
          .filter((part) => part.type === "text")
          .map((part) => part.text ?? "")
          .join("");
        if (text.length > 0) finalText = text;
      }
    } catch {
      /* non-JSON line — the protocol is newline-delimited JSON events interleaved with nothing else expected, but a stray line is never fatal */
    }
  }
  return finalText;
}

/**
 * Build a real child's environment from a base env map (D3: a directly
 * testable pure function, not buried inside a spawn call). Strips `LD_*`/
 * `DYLD_*` unconditionally (defense in depth mirroring T13's loader-injection
 * refusal, terminal-session.ts `env_loader_injection_family`) and overlays
 * `PRIME_AGENT_CODING_AGENT_DIR` — the isolated agent directory, never the
 * operator's real `~/.prime`.
 */
export function buildRealPrimeEnv(
  base: Readonly<Record<string, string | undefined>>,
  agentDir: string,
): Record<string, string> {
  const env: Record<string, string> = {};
  for (const [name, value] of Object.entries(base)) {
    if (value === undefined) continue;
    if (name.startsWith("LD_") || name.startsWith("DYLD_")) continue;
    env[name] = value;
  }
  env["PRIME_AGENT_CODING_AGENT_DIR"] = agentDir;
  env["TERM"] = env["TERM"] ?? "xterm";
  return env;
}

/**
 * The production `PrimeHostPort` implementation. One instance per
 * `WorkbenchHost` (constructed only when `config.prime_binary` is set — D6:
 * Prime absence never degrades Gauntlet, so nothing here runs unconfigured).
 */
export class RealPrimeHost implements PrimeHostPort {
  private readonly bookkeeping = new PrimeHostAdapter();
  private readonly binary: string;
  private readonly agentDir: string;
  private readonly sessionDir: string;
  private readonly workRoot: string;
  private readonly timeoutMs: number;
  private readonly pathPolicy: SpawnPathPolicy;
  private readonly liveProcesses = new Map<string, ReturnType<typeof Bun.spawn>>();
  /** pid of a REATTACHED (not directly Bun-owned) session still being polled. */
  private readonly reattachedPids = new Map<string, number>();
  private readonly reattachTimers = new Map<string, ReturnType<typeof setInterval>>();
  private readonly staleRecordsList: StaleQualifiedRecord[] = [];
  /** Work-mode settings recorded at spawn time, consulted by `continueChild` so a resumed turn matches the original launch's tools/cwd (plan P1). */
  private readonly workModeBySession = new Map<string, { toolsEnabled: boolean; cwd: string }>();
  /**
   * Workbench session id -> prime-agent's OWN session id (plan P1 bugfix):
   * `--resume` only accepts prime-agent's own id, learned from the
   * `{"type":"session","id":...}` stdout event once a turn's stdout has been
   * parsed. Populated by `watch()` after every turn (initial or resumed) and
   * by `reattachFromQualifiedRecords()` from a manifest that already recorded
   * it.
   */
  private readonly primeSessionIds = new Map<string, string>();

  constructor(options: RealPrimeHostOptions) {
    this.agentDir = options.agentDir;
    this.sessionDir = options.sessionDir;
    this.workRoot = options.workRoot ?? options.sessionDir;
    this.timeoutMs = options.timeoutMs ?? 600_000;
    // Fail closed at CONSTRUCTION, not at first spawn: an unbindable binary
    // path (relative, bare, metacharacter-bearing) refuses here. Routed
    // through the FULL validateSpawnRequest program-locus check (not
    // anchorPath alone — anchorPath does not check for shell
    // metacharacters, so a construction-time check that called it directly
    // would silently accept an injected `;`/`|`-bearing path; caught RED by
    // this file's own test suite the first time this used anchorPath alone).
    this.pathPolicy = Object.freeze({
      allowedProgramRoots: [dirname(options.binary)],
      allowedPathRoots:
        options.projectRoot === undefined
          ? [this.workRoot, this.sessionDir, this.agentDir]
          : [this.workRoot, this.sessionDir, this.agentDir, options.projectRoot],
    });
    const probe = validateSpawnRequest(
      { program: options.binary, cwd: this.workRoot },
      this.pathPolicy,
    );
    this.binary = probe.program;
    // Bun.spawn with a `cwd` that does not yet exist on disk fails —
    // confusingly, as an ENOENT blamed on the PROGRAM path, not the cwd
    // (caught RED the first time this constructor relied on the caller
    // having pre-created these directories). This class is self-sufficient:
    // it never depends on caller discipline for its own working directories.
    mkdirSync(this.agentDir, { recursive: true, mode: 0o700 });
    // 0700: sessionDir holds the qualified-record manifests stopChild()
    // trusts for pid-targeted SIGKILL (plan Phase D) — only this OS user may
    // write into it, narrowing the forged-manifest trust surface to
    // "another process running as the SAME user", which already carries
    // equivalent kill privilege on this host regardless of this directory.
    mkdirSync(this.sessionDir, { recursive: true, mode: 0o700 });
    mkdirSync(this.workRoot, { recursive: true });
    this.reattachFromQualifiedRecords();
  }

  /** Real, currently-running child session ids (inspection only, not part of PrimeHostPort). */
  liveSessionIds(): readonly string[] {
    return [...this.liveProcesses.keys(), ...this.reattachedPids.keys()];
  }

  /**
   * Qualified manifests this host found whose process is gone and whose
   * result never landed — an honest, typed-unavailable record (plan Phase D,
   * REQ-PRIME-010's explicit prohibition: this NEVER surfaces as "running").
   */
  staleRecords(): readonly StaleQualifiedRecord[] {
    return [...this.staleRecordsList];
  }

  /**
   * Explicit operator stop (plan Phase D, REQ-PRIME-009): the ONLY path in
   * this class that terminates a live child. Closing/detaching the host
   * never calls this (workbench-host.ts's `close()` never references
   * `primeDispatcher` at all) — this method exists so an explicit,
   * deliberate call CAN terminate work, while every other path (mailbox,
   * detach, host close, TUI death) genuinely cannot.
   *
   * For a session this SAME process still directly owns (a live
   * `Bun.Subprocess` handle), this only signals the process — the existing
   * `watch()` task records the (honest, non-fabricated) completion from the
   * process's own exit, so there is exactly one writer of that outcome, not
   * a race between two. For a REATTACHED session (pid recovered from a
   * qualified manifest, no live handle), this owns the only observer of it
   * (the reattach poll), so it cancels that poll and records termination
   * directly.
   */
  stopChild(sessionId: string): PrimeStopOutcome {
    const session = this.bookkeeping.getSession(sessionId);
    if (session === null) {
      return { session_id: sessionId, stopped: false, detail: "no known session with this id" };
    }
    if (session.handle.status === "completed" || session.handle.status === "failed") {
      return {
        session_id: sessionId,
        stopped: false,
        detail: `session already terminal: ${session.handle.status}`,
      };
    }
    const proc = this.liveProcesses.get(sessionId);
    if (proc !== undefined) {
      try {
        proc.kill("SIGKILL");
      } catch {
        /* already exited */
      }
      return {
        session_id: sessionId,
        stopped: true,
        detail:
          "explicit operator stop: SIGKILL sent to the live process; completion is recorded by its own exit",
      };
    }
    const pid = this.reattachedPids.get(sessionId);
    if (pid === undefined) {
      return {
        session_id: sessionId,
        stopped: false,
        detail: "no live process pid on record for this session",
      };
    }
    try {
      process.kill(pid, "SIGKILL");
    } catch {
      /* already gone */
    }
    const timer = this.reattachTimers.get(sessionId);
    if (timer !== undefined) {
      clearInterval(timer);
      this.reattachTimers.delete(sessionId);
    }
    this.reattachedPids.delete(sessionId);
    this.bookkeeping.deliverResultContract(sessionId, {
      session_id: sessionId,
      schema_valid: false,
      outcome: "inconclusive",
      evidence_path: session.handle.contract_path,
      completed_at: new Date().toISOString(),
    });
    return {
      session_id: sessionId,
      stopped: true,
      detail:
        "explicit operator stop: SIGKILL sent to the reattached process; no further evidence will be recorded for it",
    };
  }

  /**
   * Deliver an operator message to a child (plan P1 prime.message). The
   * operator's message is ALWAYS recorded as mailbox communication first,
   * regardless of delivery mechanism (REQ-PRIME-005: message content is
   * communication, never completion, on either side of the exchange).
   *
   * For a session still directly live (this host's own `Bun.Subprocess`, or
   * a reattached pid), delivery is attempted via the installed CLI's own
   * `prime-agent send --steer <session_id> <message>` (verified against
   * `prime-agent send --help`: `--steer` "Deliver as steering when the agent
   * is busy"). This host's own children are one-shot `-p` runs that exit
   * after their single turn, so — honestly — there is no daemon-registered
   * "agent" for `send` to address in the common case; the attempt is
   * best-effort and its exit code is recorded in `detail`, never presented as
   * a reply (the reply, if any, only ever arrives through `watch()`'s own
   * mailbox write).
   *
   * For a FINISHED session, this resumes the SAME session with a fresh
   * one-shot turn (`--resume <session_id> -p <message>`) using the same
   * work-mode (tools/cwd) the original launch used — never a new session id.
   */
  continueChild(sessionId: string, request: PrimeContinuationRequest): PrimeContinuationResult {
    const session = this.bookkeeping.getSession(sessionId);
    if (session === null) {
      return {
        session_id: sessionId,
        delivered: false,
        mode: "resumed_new_turn",
        detail: "no known session with this id",
      };
    }
    // Communication is recorded regardless of delivery mechanism.
    this.bookkeeping.receiveMailboxMessage(sessionId, {
      sender: request.sender,
      recipient: sessionId,
      content: request.message,
      kind: "communication",
    });

    const stillLive = this.liveProcesses.has(sessionId) || this.reattachedPids.has(sessionId);
    if (stillLive) {
      // `send` addresses prime-agent's own AGENT NAME (not necessarily this
      // workbench session id — reported honestly below regardless of exit
      // code). A 5s timeout keeps a hung/unaddressable target from ever
      // blocking the render loop.
      const sendResult = validateSpawnRequest(
        {
          program: this.binary,
          args: ["send", "--steer", "--from", "operator", sessionId, request.message],
          cwd: this.workRoot,
        },
        this.pathPolicy,
      );
      const proc = Bun.spawnSync([sendResult.program, ...sendResult.args], {
        cwd: sendResult.cwd,
        env: this.buildEnv(),
        timeout: 5000,
        killSignal: "SIGKILL",
      });
      const delivered = proc.exitCode === 0;
      return {
        session_id: sessionId,
        delivered,
        mode: "steer",
        detail: delivered
          ? "delivered via `prime-agent send --steer` (exit 0); message recorded in mailbox regardless — note `send` addresses an agent NAME, which may not be this session id, so delivery-to-the-right-target is not independently verified"
          : `message recorded in mailbox; \`prime-agent send --steer\` did NOT deliver (exit ${proc.exitCode ?? "timeout/killed"}) — one-shot children have no long-lived daemon to address in the common case`,
      };
    }

    // Finished: resume the SAME session with a new one-shot turn. `--resume`
    // accepts ONLY prime-agent's OWN session id (verified against the real
    // binary: `--resume <workbench id>` exits 1, "No session found matching
    // '<id>'") — never this workbench's `sessionId`.
    const primeSessionId = this.primeSessionIds.get(sessionId);
    if (primeSessionId === undefined) {
      return {
        session_id: sessionId,
        delivered: false,
        mode: "resumed_new_turn",
        detail:
          'prime-agent\'s own session id for this workbench session is not yet known (no {type:"session",id} event was parsed from its first turn) — refusing to guess a --resume target; nothing was spawned',
      };
    }
    const workMode = this.workModeBySession.get(sessionId);
    const cwd = workMode?.cwd ?? this.workRoot;
    const toolsArg: readonly string[] = workMode?.toolsEnabled === true ? [] : ["--no-tools"];
    let bound: ReturnType<typeof validateSpawnRequest>;
    try {
      bound = validateSpawnRequest(
        {
          program: this.binary,
          args: [
            "--mode",
            "json",
            "--resume",
            primeSessionId,
            "-p",
            request.message,
            "--offline",
            ...toolsArg,
            "-ne",
            "-ns",
            "-nc",
            "--session-dir",
            this.sessionDir,
            "--cwd",
            cwd,
          ],
          cwd,
        },
        this.pathPolicy,
      );
    } catch (error) {
      return {
        session_id: sessionId,
        delivered: false,
        mode: "resumed_new_turn",
        detail: `refused at the T13 spawn boundary: ${error instanceof Error ? error.message : String(error)}`,
      };
    }
    const proc = Bun.spawn([bound.program, ...bound.args], {
      cwd: bound.cwd,
      env: this.buildEnv(),
      stdout: "pipe",
      stderr: "pipe",
    });
    this.liveProcesses.set(sessionId, proc);
    void this.watch(sessionId, proc, session.handle.contract_path).catch(() => {
      // Same discipline as spawnChild(): a watch-loop crash never surfaces as
      // an unhandled rejection.
    });
    return {
      session_id: sessionId,
      delivered: true,
      mode: "resumed_new_turn",
      detail: `resumed prime-agent session '${primeSessionId}' with a new one-shot turn (--resume); reply lands via mailbox`,
    };
  }

  private manifestPath(sessionId: string): string {
    return join(this.sessionDir, `${sessionId}${QUALIFIED_MANIFEST_SUFFIX}`);
  }

  /**
   * Persist a learned prime-agent session id (plan P1 bugfix): updates the
   * in-memory map AND rewrites the durable qualified manifest (additive
   * field) so a restarted host can still resume. Best-effort on the file
   * write — the in-memory map is the map `continueChild` actually reads
   * within one process's lifetime; a write failure only affects RESTART
   * recovery, never this process's own ability to resume.
   */
  private persistPrimeSessionId(sessionId: string, primeSessionId: string): void {
    this.primeSessionIds.set(sessionId, primeSessionId);
    const path = this.manifestPath(sessionId);
    if (!existsSync(path)) return;
    try {
      const raw = JSON.parse(readFileSync(path, "utf8"));
      if (isQualifiedSpawnManifest(raw)) {
        writeFileSync(path, JSON.stringify({ ...raw, prime_session_id: primeSessionId }, null, 2));
      }
    } catch {
      /* best-effort: restart-recovery only, never this process's own resume ability */
    }
  }

  /**
   * Reconstruct topology from qualified records only (REQ-PRIME-010): scan
   * `sessionDir` for THIS host's own `*.qualified.json` manifests, written
   * synchronously at spawn time so they survive this process's death. A bare
   * directory with unrelated or unparseable files mints NOTHING (tooth:
   * directory presence alone never fabricates a "running" session) — only a
   * file that parses as a manifest is a qualified record.
   */
  private reattachFromQualifiedRecords(): void {
    let entries: string[];
    try {
      entries = readdirSync(this.sessionDir);
    } catch {
      return;
    }
    for (const name of entries) {
      if (!name.endsWith(QUALIFIED_MANIFEST_SUFFIX)) continue;
      let parsed: unknown;
      try {
        parsed = JSON.parse(readFileSync(join(this.sessionDir, name), "utf8"));
      } catch {
        continue;
      }
      if (!isQualifiedSpawnManifest(parsed)) continue;
      const manifest = parsed;
      // Seed the resume map regardless of which branch below applies: a
      // learned prime-agent session id survives this host's restart exactly
      // because it was persisted here (plan P1 bugfix).
      if (manifest.prime_session_id !== undefined) {
        this.primeSessionIds.set(manifest.session_id, manifest.prime_session_id);
      }
      if (this.bookkeeping.getSession(manifest.session_id) !== null) continue;

      const spawnOptions: PrimeSpawnRequest = {
        sessionId: manifest.session_id,
        parentSessionId: manifest.parent_session_id,
        model: manifest.model,
        contractPath: manifest.contract_path,
      };

      if (existsSync(manifest.contract_path)) {
        let evidence: unknown;
        try {
          evidence = JSON.parse(readFileSync(manifest.contract_path, "utf8"));
        } catch {
          this.staleRecordsList.push({
            session_id: manifest.session_id,
            reason: "contract_path_present_but_unreadable",
          });
          continue;
        }
        this.bookkeeping.spawnChild(spawnOptions);
        this.bookkeeping.deliverResultContract(
          manifest.session_id,
          contractFromEvidenceRecord(manifest.session_id, manifest.contract_path, evidence),
        );
        continue;
      }

      if (isPidAlive(manifest.pid)) {
        this.bookkeeping.spawnChild(spawnOptions);
        this.reattachedPids.set(manifest.session_id, manifest.pid);
        this.startReattachPoll(manifest.session_id, manifest.pid, manifest.contract_path);
        continue;
      }

      // A qualified manifest exists, but the process is gone AND no result
      // ever landed: honestly stale — never minted as "running" or folded
      // into bookkeeping at all (REQ-PRIME-010's explicit prohibition).
      this.staleRecordsList.push({
        session_id: manifest.session_id,
        reason: "process_not_found_no_result_contract",
      });
    }
  }

  /** Poll a reattached (non-Bun-owned) child for its contract landing or its pid dying. */
  private startReattachPoll(sessionId: string, pid: number, contractPath: string): void {
    const startedAt = Date.now();
    const finish = (contract: PrimeResultContract) => {
      const timer = this.reattachTimers.get(sessionId);
      if (timer !== undefined) clearInterval(timer);
      this.reattachTimers.delete(sessionId);
      this.reattachedPids.delete(sessionId);
      this.bookkeeping.deliverResultContract(sessionId, contract);
    };
    const timer = setInterval(() => {
      if (existsSync(contractPath)) {
        let evidence: unknown;
        try {
          evidence = JSON.parse(readFileSync(contractPath, "utf8"));
        } catch {
          finish({
            session_id: sessionId,
            schema_valid: false,
            outcome: "inconclusive",
            evidence_path: contractPath,
            completed_at: new Date().toISOString(),
          });
          return;
        }
        finish(contractFromEvidenceRecord(sessionId, contractPath, evidence));
        return;
      }
      if (!isPidAlive(pid) || Date.now() - startedAt > this.timeoutMs) {
        finish({
          session_id: sessionId,
          schema_valid: false,
          outcome: "inconclusive",
          evidence_path: contractPath,
          completed_at: new Date().toISOString(),
        });
      }
    }, 250);
    if (typeof timer.unref === "function") timer.unref();
    this.reattachTimers.set(sessionId, timer);
  }

  spawnChild(options: PrimeSpawnRequest): PrimeAdmissionHandle {
    if (options.task === undefined || options.task.trim().length === 0) {
      throw new RealPrimeHostSpawnError(
        `RealPrimeHost cannot spawn '${options.sessionId}': a real process requires a task/goal, and none was supplied`,
      );
    }
    // Work mode (plan P1): a composer-launched child needs tools to actually
    // edit code, and a cwd distinct from the isolated session/agent dirs — a
    // coding harness child with `--no-tools` cannot edit anything. Absent
    // `workMode`, behavior is EXACTLY what every pre-P1 caller/test already
    // gets: `--no-tools` and `this.workRoot` (D6-style additivity).
    const cwd = options.workMode?.cwd ?? this.workRoot;
    const toolsArg: readonly string[] =
      options.workMode?.toolsEnabled === true ? [] : ["--no-tools"];

    // Program/argv/cwd bound through the T13 privileged boundary BEFORE any
    // bookkeeping row exists — a refusal here mints nothing (clean fail).
    const bound = validateSpawnRequest(
      {
        program: this.binary,
        args: [
          "--mode",
          "json",
          "-p",
          options.task,
          "--offline",
          ...toolsArg,
          "-ne",
          "-ns",
          "-nc",
          "--session-dir",
          this.sessionDir,
          "--cwd",
          cwd,
        ],
        cwd,
      },
      this.pathPolicy,
    );

    // Admission is nonblocking (REQ-PRIME-003): bookkeeping mints the handle
    // and returns BEFORE the real process's output is ever read.
    const handle = this.bookkeeping.spawnChild(options);
    if (options.workMode !== undefined) {
      this.workModeBySession.set(options.sessionId, {
        toolsEnabled: options.workMode.toolsEnabled,
        cwd: options.workMode.cwd,
      });
    }

    // The project root is real, operator-owned state — never chmod'd or
    // otherwise narrowed the way the isolated agent/session dirs are, and
    // Bun.spawn fails (confusingly, blaming the program) if cwd is absent.
    if (options.workMode !== undefined) mkdirSync(options.workMode.cwd, { recursive: true });

    const proc = Bun.spawn([bound.program, ...bound.args], {
      cwd: bound.cwd,
      env: this.buildEnv(),
      stdout: "pipe",
      stderr: "pipe",
    });
    this.liveProcesses.set(options.sessionId, proc);
    // The qualified record (REQ-PRIME-010): written synchronously, BEFORE
    // this call returns, so it survives this process's own death. A fresh
    // RealPrimeHost reattaches from exactly this file and nothing else.
    writeFileSync(
      this.manifestPath(handle.session_id),
      JSON.stringify(
        {
          session_id: handle.session_id,
          parent_session_id: handle.parent_session_id,
          model: handle.model,
          contract_path: handle.contract_path,
          pid: proc.pid,
          spawned_at: handle.admitted_at,
        } satisfies QualifiedSpawnManifest,
        null,
        2,
      ),
    );
    void this.watch(options.sessionId, proc, options.contractPath).catch(() => {
      // A watch-loop crash must never surface as an unhandled rejection; the
      // session simply never receives a result contract and stays "running"
      // — honestly stalled, never silently marked complete.
    });

    return handle;
  }

  receiveMailboxMessage(
    sessionId: string,
    message: {
      sender: string;
      recipient?: string | undefined;
      content: string;
      kind?: "communication" | "progress" | "signal" | undefined;
    },
  ): PrimeMailboxMessage {
    return this.bookkeeping.receiveMailboxMessage(sessionId, message);
  }

  deliverResultContract(sessionId: string, contract: PrimeResultContract): boolean {
    return this.bookkeeping.deliverResultContract(sessionId, contract);
  }

  getSession(sessionId: string): PrimeSessionState | null {
    return this.bookkeeping.getSession(sessionId);
  }

  listSessions(): PrimeSessionState[] {
    return this.bookkeeping.listSessions();
  }

  private buildEnv(): Record<string, string> {
    return buildRealPrimeEnv(process.env as Record<string, string>, this.agentDir);
  }

  private async watch(
    sessionId: string,
    proc: ReturnType<typeof Bun.spawn>,
    contractPath: string,
  ): Promise<void> {
    const timer = setTimeout(() => {
      try {
        proc.kill();
      } catch {
        /* already exited */
      }
    }, this.timeoutMs);

    let stdout = "";
    let stderr = "";
    let exitCode: number;
    try {
      const [out, err, code] = await Promise.all([
        new Response(proc.stdout as ReadableStream<Uint8Array>).text(),
        new Response(proc.stderr as ReadableStream<Uint8Array>).text(),
        proc.exited,
      ]);
      stdout = out;
      stderr = err;
      exitCode = code;
    } finally {
      clearTimeout(timer);
      this.liveProcesses.delete(sessionId);
    }

    // Learn/refresh prime-agent's OWN session id from this turn's stdout
    // (plan P1 bugfix): `--resume` only ever accepts THIS id, never the
    // workbench's own `sessionId`. Every turn (initial or resumed) re-emits
    // a `session` event, so this unconditionally overwrites the stored id —
    // "the resumed turn's own session event must match the stored id;
    // update if not" is satisfied by simply always trusting the latest turn.
    const primeSessionId = parsePrimeSessionId(stdout);
    if (primeSessionId !== null) {
      this.persistPrimeSessionId(sessionId, primeSessionId);
    }

    // Mailbox text is communication, never completion (REQ-PRIME-005) — the
    // SAME invariant the fake enforces, because this calls the SAME method.
    const finalText = parseAssistantText(stdout);
    if (finalText !== null) {
      this.bookkeeping.receiveMailboxMessage(sessionId, {
        sender: sessionId,
        recipient: "orchestrator",
        content: finalText,
        kind: "communication",
      });
    }

    const completedAt = new Date().toISOString();
    const evidence = {
      session_id: sessionId,
      exit_code: exitCode,
      stdout_bytes: stdout.length,
      stderr_excerpt: stderr.slice(0, 4000),
      completed_at: completedAt,
    };
    try {
      await Bun.write(contractPath, JSON.stringify(evidence, null, 2));
    } catch {
      // Cannot write the evidence file at the declared locator: the contract
      // is NOT schema-valid by this host's own definition — never claim an
      // evidence_path that does not actually hold evidence (D1).
      this.bookkeeping.deliverResultContract(sessionId, {
        session_id: sessionId,
        schema_valid: false,
        outcome: "inconclusive",
        evidence_path: contractPath,
        completed_at: completedAt,
      });
      return;
    }
    this.bookkeeping.deliverResultContract(
      sessionId,
      contractFromEvidenceRecord(sessionId, contractPath, evidence),
    );
  }
}
