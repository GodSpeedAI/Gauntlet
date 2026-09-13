/**
 * Prime Agent structured headless host adapter, process lifecycle, and mailbox protocol.
 * Conforms to spec §20.2, REQ-GOAL-003, REQ-OPS-001, REQ-PRIME-001..005, and plan T04.
 *
 * Core Invariant:
 *   - Child admission is nonblocking (returns admission handle immediately).
 *   - Mailbox communication != completion (a mailbox reply saying "done" is communication,
 *     never settlement; completion requires a schema-valid result contract at the contract path).
 */

/** Inputs to admit one Prime child session (`PrimeHostPort.spawnChild`). */
export interface PrimeSpawnRequest {
  readonly sessionId: string;
  readonly parentSessionId?: string | null | undefined;
  readonly model?: string | undefined;
  readonly contractPath: string;
  /**
   * The child's task/goal text. Optional because the in-memory
   * `PrimeHostAdapter` test double never needed it (the fake models "task
   * delivery" purely as an inspectable `receiveMailboxMessage` call the
   * caller makes right after spawning). `RealPrimeHost` (plan Phase C2)
   * REQUIRES it — a real process cannot be launched without a goal — and
   * refuses to spawn when it is absent, honestly, rather than inventing one.
   */
  readonly task?: string | undefined;
  /**
   * Work-mode override for a composer-launched child (plan
   * workbench-agent-interaction.md P1): tools enabled and a `cwd` distinct
   * from the host's isolated session/agent directories — a coding harness
   * child with `--no-tools` cannot edit code. Absent (the default for every
   * pre-P1 caller) means the existing `--no-tools`/isolated-cwd behavior,
   * completely unchanged (D6-style additivity). `PrimeHostAdapter` (the fake)
   * ignores this field; only `RealPrimeHost` acts on it.
   */
  readonly workMode?: { readonly toolsEnabled: boolean; readonly cwd: string } | undefined;
}

/**
 * The 5-method surface `PrimeDelegationDispatcher`/`PrimeDelegationRoute`
 * drive a Prime host through (plan Phase C2, D2 "replace don't accumulate").
 * `PrimeHostAdapter` below is the declared in-memory TEST DOUBLE
 * implementation; `RealPrimeHost` (`real-prime-host.ts`) is the production
 * implementation over `Bun.spawn`. Both satisfy this port identically, so
 * `PrimeDelegationDispatcher` (prime-recursion.ts) needed zero changes beyond
 * widening its stored type from the concrete class to this interface.
 */
export interface PrimeHostPort {
  spawnChild(options: PrimeSpawnRequest): PrimeAdmissionHandle;
  receiveMailboxMessage(
    sessionId: string,
    message: {
      sender: string;
      recipient?: string | undefined;
      content: string;
      kind?: "communication" | "progress" | "signal" | undefined;
    },
  ): PrimeMailboxMessage;
  deliverResultContract(sessionId: string, contract: PrimeResultContract): boolean;
  getSession(sessionId: string): PrimeSessionState | null;
  listSessions(): PrimeSessionState[];
}

export interface PrimeAdmissionHandle {
  readonly session_id: string;
  readonly parent_session_id: string | null;
  readonly model: string;
  readonly admitted_at: string;
  readonly contract_path: string;
  status: "admitted" | "running" | "completed" | "failed";
}

export interface PrimeMailboxMessage {
  readonly message_id: string;
  readonly session_id: string;
  readonly sender: string;
  readonly recipient: string;
  readonly content: string;
  readonly timestamp: string;
  readonly kind: "communication" | "progress" | "signal";
}

export interface PrimeResultContract {
  readonly session_id: string;
  readonly schema_valid: boolean;
  readonly outcome: "pass" | "fail" | "inconclusive";
  readonly evidence_path: string;
  readonly completed_at: string;
}

export interface PrimeSessionState {
  readonly handle: PrimeAdmissionHandle;
  readonly messages: PrimeMailboxMessage[];
  resultContract: PrimeResultContract | null;
}

/**
 * Outcome of an explicit operator stop (plan Phase D / workbench-agent-interaction
 * P1, REQ-PRIME-009). Declared here (not only on `RealPrimeHost`) so the
 * in-memory `PrimeHostAdapter` test double can expose the SAME shape for
 * dispatcher-level unit tests — `real-prime-host.ts` re-exports this type for
 * source compatibility with its existing callers/tests.
 */
export interface PrimeStopOutcome {
  readonly session_id: string;
  readonly stopped: boolean;
  readonly detail: string;
}

/** One operator message delivered into a live or finished child's turn (P1 prime.message). */
export interface PrimeContinuationRequest {
  readonly sender: string;
  readonly message: string;
}

/**
 * Outcome of delivering an operator message to a child (P1 prime.message).
 * `mode` records HOW delivery happened — never a completion claim by itself
 * (REQ-PRIME-005): "steer"/"follow_up" address a still-running child through
 * the installed CLI's `send` command; "resumed_new_turn" continues the SAME
 * session with a fresh one-shot turn once the prior turn finished.
 */
export interface PrimeContinuationResult {
  readonly session_id: string;
  readonly delivered: boolean;
  readonly mode: "steer" | "follow_up" | "resumed_new_turn";
  readonly detail: string;
}

/**
 * The wider surface `RealPrimeHost` and `PrimeHostAdapter` both implement,
 * beyond the base `PrimeHostPort` five methods `PrimeDelegationDispatcher`
 * drives. Kept SEPARATE from `PrimeHostPort` deliberately: widening that
 * interface would ripple into every existing implementer/mock across the
 * suite for a capability only the workbench host's prime.message/prime.cancel
 * paths need. `isExtendedPrimeHost` narrows a plain `PrimeHostPort` to this
 * shape at the one call site that needs it (workbench-host.ts).
 */
export interface ExtendedPrimeHostPort extends PrimeHostPort {
  stopChild(sessionId: string): PrimeStopOutcome;
  continueChild(sessionId: string, request: PrimeContinuationRequest): PrimeContinuationResult;
}

export function isExtendedPrimeHost(host: PrimeHostPort): host is ExtendedPrimeHostPort {
  const candidate = host as Partial<ExtendedPrimeHostPort>;
  return typeof candidate.stopChild === "function" && typeof candidate.continueChild === "function";
}

/**
 * Optional "cheap change signal" surface (plan workbench-agent-interaction.md
 * P2): a host implementing this can push a single notification whenever its
 * bookkeeping changes (mailbox message received, result contract delivered,
 * a session stopped) so `WorkbenchHost` can redraw the Chat/Trace surfaces
 * WITHOUT waiting for the next poll tick and WITHOUT polling the Prime host
 * itself (no busy loop: this is a push from the SAME async work — a child's
 * background watch task, or a synchronous bookkeeping call — that already
 * runs). Declared separately from `PrimeHostPort`/`ExtendedPrimeHostPort` for
 * the same reason those are separate: widening either would ripple into
 * every existing implementer for a capability only the live-redraw path
 * needs. `hasChangeListener` narrows at the one call site that wires it.
 */
export interface PrimeHostChangeSource {
  setChangeListener(listener: () => void): void;
}

export function hasChangeListener(
  host: PrimeHostPort,
): host is PrimeHostPort & PrimeHostChangeSource {
  return typeof (host as Partial<PrimeHostChangeSource>).setChangeListener === "function";
}

/**
 * Chat-surface projection of one Prime child session (plan P2): the SAME
 * `PrimeSessionState` bookkeeping every dispatcher drive already produces,
 * plus the target unit the dispatcher's own `DelegationRecord` recorded at
 * admission time — never invented, never parsed back out of the session id.
 * `WorkbenchHost.listPrimeSessions()` is the only place this is assembled.
 */
export interface PrimeChatSession {
  readonly session_id: string;
  readonly target_unit: string | null;
  readonly status: PrimeAdmissionHandle["status"];
  readonly admitted_at: string;
  readonly model: string;
  readonly messages: readonly PrimeMailboxMessage[];
}

/**
 * The in-memory Prime host TEST DOUBLE (plan Phase C2, D2). This was the
 * only `PrimeHostPort` implementation before Phase C2; it is now explicitly
 * declared as such — production composition (`workbench-host.ts`) binds
 * `RealPrimeHost` (`real-prime-host.ts`) instead. Every test that constructs
 * `new PrimeHostAdapter()` directly (prime-recursion.test.ts and siblings)
 * keeps working unchanged: this class's public shape did not change, it only
 * gained an explicit `implements PrimeHostPort`.
 */
export class PrimeHostAdapter implements PrimeHostPort, PrimeHostChangeSource {
  private readonly sessions = new Map<string, PrimeSessionState>();
  private messageCounter = 0;
  private changeListener: (() => void) | undefined;

  /** Plan P2: push a redraw signal on every bookkeeping change (see `PrimeHostChangeSource`). */
  setChangeListener(listener: () => void): void {
    this.changeListener = listener;
  }

  private notifyChanged(): void {
    this.changeListener?.();
  }

  /**
   * Spawn or admit a Prime child session (REQ-PRIME-001).
   * Nonblocking: returns an admission handle immediately without awaiting execution.
   */
  spawnChild(options: PrimeSpawnRequest): PrimeAdmissionHandle {
    const handle: PrimeAdmissionHandle = {
      session_id: options.sessionId,
      parent_session_id: options.parentSessionId ?? null,
      model: options.model ?? "claude-3-5-sonnet",
      admitted_at: new Date().toISOString(),
      contract_path: options.contractPath,
      status: "admitted",
    };

    this.sessions.set(options.sessionId, {
      handle,
      messages: [],
      resultContract: null,
    });

    // Mark as running as execution begins asynchronously
    handle.status = "running";
    return { ...handle };
  }

  /**
   * Record a mailbox communication message in session history (REQ-PRIME-003, D1).
   *
   * ATTACK 1 DEFENSE (AGENTS.md invariant):
   * A mailbox reply is a signal/communication, NOT a contract.
   * Even if message content contains "done", "complete", or "finished",
   * the child's status MUST REMAIN RUNNING. It CANNOT settle or complete
   * from mailbox text.
   */
  receiveMailboxMessage(
    sessionId: string,
    message: {
      sender: string;
      recipient?: string | undefined;
      content: string;
      kind?: "communication" | "progress" | "signal" | undefined;
    },
  ): PrimeMailboxMessage {
    const session = this.sessions.get(sessionId);
    if (!session) {
      throw new Error(`Session '${sessionId}' not found in Prime host adapter`);
    }

    this.messageCounter += 1;
    const mailboxMsg: PrimeMailboxMessage = {
      message_id: `msg-${this.messageCounter}`,
      session_id: sessionId,
      sender: message.sender,
      recipient: message.recipient ?? "orchestrator",
      content: message.content,
      timestamp: new Date().toISOString(),
      kind: message.kind ?? "communication",
    };

    session.messages.push(mailboxMsg);

    // CRITICAL: We explicitly DO NOT change session.handle.status to 'completed'!
    // A mailbox message cannot claim task completion.

    this.notifyChanged();
    return mailboxMsg;
  }

  /**
   * Deliver a schema-valid result contract to complete a session (AGENTS.md).
   * Completion requires a schema-valid result contract at the contract path.
   */
  deliverResultContract(sessionId: string, contract: PrimeResultContract): boolean {
    const session = this.sessions.get(sessionId);
    if (!session) {
      return false;
    }

    if (!contract.schema_valid) {
      session.handle.status = "failed";
      this.notifyChanged();
      return false;
    }

    session.resultContract = contract;
    session.handle.status = "completed";
    this.notifyChanged();
    return true;
  }

  /**
   * Get current session status.
   */
  getSession(sessionId: string): PrimeSessionState | null {
    const s = this.sessions.get(sessionId);
    if (!s) return null;
    return {
      handle: { ...s.handle },
      messages: [...s.messages],
      resultContract: s.resultContract ? { ...s.resultContract } : null,
    };
  }

  /**
   * List all known sessions.
   */
  listSessions(): PrimeSessionState[] {
    return Array.from(this.sessions.values()).map((s) => ({
      handle: { ...s.handle },
      messages: [...s.messages],
      resultContract: s.resultContract ? { ...s.resultContract } : null,
    }));
  }

  /**
   * Test-double stop (plan P1): idempotent on an unknown or already-terminal
   * session, zero side effects on refusal. A running/admitted session is
   * marked terminal WITHOUT a schema-valid result contract (mirrors
   * `RealPrimeHost.stopChild`'s "no further evidence will be recorded"
   * outcome) so dispatcher-level unit tests can exercise cancel against the
   * fake without a real process.
   */
  stopChild(sessionId: string): PrimeStopOutcome {
    const session = this.sessions.get(sessionId);
    if (session === undefined) {
      return { session_id: sessionId, stopped: false, detail: "no known session with this id" };
    }
    if (session.handle.status === "completed" || session.handle.status === "failed") {
      return {
        session_id: sessionId,
        stopped: false,
        detail: `session already terminal: ${session.handle.status}`,
      };
    }
    session.handle.status = "failed";
    this.notifyChanged();
    return {
      session_id: sessionId,
      stopped: true,
      detail: "test-double stop: session marked terminal, no further evidence will be recorded",
    };
  }

  /**
   * Test-double message continuation (plan P1): records the operator's
   * message as mailbox communication and reports whether the session was
   * still live ("steer") or already finished ("resumed_new_turn") — the SAME
   * session id throughout; this fake never mints a second session for a
   * follow-up message.
   */
  continueChild(sessionId: string, request: PrimeContinuationRequest): PrimeContinuationResult {
    const session = this.sessions.get(sessionId);
    if (session === undefined) {
      return {
        session_id: sessionId,
        delivered: false,
        mode: "resumed_new_turn",
        detail: "no known session with this id",
      };
    }
    this.receiveMailboxMessage(sessionId, {
      sender: request.sender,
      recipient: sessionId,
      content: request.message,
      kind: "communication",
    });
    const stillLive = session.handle.status === "admitted" || session.handle.status === "running";
    return {
      session_id: sessionId,
      delivered: true,
      mode: stillLive ? "steer" : "resumed_new_turn",
      detail: stillLive
        ? "test-double: message recorded against the live session (no real process to steer)"
        : "test-double: message recorded as a follow-up turn on the SAME finished session",
    };
  }
}
