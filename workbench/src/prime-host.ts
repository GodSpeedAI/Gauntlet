/**
 * Prime Agent structured headless host adapter, process lifecycle, and mailbox protocol.
 * Conforms to spec §20.2, REQ-GOAL-003, REQ-OPS-001, REQ-PRIME-001..005, and plan T04.
 *
 * Core Invariant:
 *   - Child admission is nonblocking (returns admission handle immediately).
 *   - Mailbox communication != completion (a mailbox reply saying "done" is communication,
 *     never settlement; completion requires a schema-valid result contract at the contract path).
 */

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

export class PrimeHostAdapter {
  private readonly sessions = new Map<string, PrimeSessionState>();
  private messageCounter = 0;

  /**
   * Spawn or admit a Prime child session (REQ-PRIME-001).
   * Nonblocking: returns an admission handle immediately without awaiting execution.
   */
  spawnChild(options: {
    sessionId: string;
    parentSessionId?: string | null | undefined;
    model?: string | undefined;
    contractPath: string;
  }): PrimeAdmissionHandle {
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
      return false;
    }

    session.resultContract = contract;
    session.handle.status = "completed";
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
}
