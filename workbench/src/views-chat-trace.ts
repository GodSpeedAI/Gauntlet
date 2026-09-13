/**
 * Chat and Trace primary projections plus the provider-backed lower views
 * (plan T12, REQ-TUI-003, REQ-DATA-005, REQ-OUT-001, DSN-010, DESIGN.md view
 * grammar).
 *
 * Core invariants:
 *   - Chat and Trace are SEPARATE projections over the SAME durable history
 *     snapshot; Chat is never canonical input for Trace (REQ-DATA-005,
 *     DSN-010). Every render consumes only `snapshot` fields, so removing the
 *     Chat projection cannot alter any Trace byte.
 *   - Chat is one View among several and is subordinate to the durable
 *     execution history (DSN-010): a disabled Chat projection renders an
 *     honest disabled state, never a fabricated conversation.
 *   - Lower surfaces (Evidence, Agents, Attention) render only the providers
 *     the snapshot actually publishes; each absent provider stays absent with
 *     an inspectable reason via the shell's view availability table.
 */

import type { FocusRef } from "./focus";
import { renderEvidenceRecord } from "./inspector";
import { laneStale } from "./reality-trace-lane";
import type { RenderedLine, RenderedRecord, ViewContent } from "./render-tree";
import type { SemanticToken } from "./theme";
import { SEMANTIC_TOKENS } from "./theme";
import type { ViewRenderArgs } from "./views-work";
import { renderWorkView } from "./views-work";
import type { FullWorkbenchSnapshot } from "./workbench-model";

/** Presentation-only token for a trace event type (display, never authority). */
export function traceEventToken(eventType: string): SemanticToken {
  switch (eventType) {
    case "evidence_added":
      return "evidence.complete";
    case "focus_changed":
      return "focus.primary";
    // unit_updated and session_updated are both live execution markers.
    default:
      return "execution.running";
  }
}

/** Presentation-only token for a Prime child session's status (never authority). */
function primeSessionToken(status: string): SemanticToken {
  switch (status) {
    case "completed":
      return "execution.succeeded";
    case "failed":
      return "execution.failed";
    default:
      return "execution.running";
  }
}

/**
 * Render one mailbox message's content compactly (plan P2 item 1): the
 * initial received-context message is a raw envelope JSON blob
 * (`prime-recursion.ts`'s `received_context`/`task`/`envelope` shape) — show
 * only the task, never the whole envelope. Anything else (assistant replies,
 * plain operator text) is not that shape and renders as-is.
 */
function summarizeMailboxContent(content: string): string {
  try {
    const parsed = JSON.parse(content) as { received_context?: unknown; task?: unknown };
    if (parsed.received_context === "prime-context-envelope/1" && typeof parsed.task === "string") {
      return `goal: ${parsed.task}`;
    }
  } catch {
    /* not JSON — plain communication text, rendered verbatim */
  }
  return content;
}

/**
 * Prime session list + selected transcript (plan workbench-agent-interaction.md
 * P2, UESC truth-ownership matrix "Prime messages | Prime mailbox | Project
 * communication, not completion"). `snapshot.prime_sessions`/`prime_configured`
 * are host-published lens data (see `inspector.ts` `publishLensData`,
 * `workbench-model.ts` `FullWorkbenchSnapshot`) — the model itself never
 * observes Prime. Selection is the ordinary Focus mechanism: a "PrimeSession"
 * kind FocusRef (minted by `relations.ts` `focusForPrimeChild`), set via the
 * SAME `select_focus` effect Navigator rows and this view's own clickable
 * `chat.session.<id>` rows both produce (mouse ≡ keyboard, ICM §35) — there is
 * no separate "current chat session" model.
 */
function renderPrimeChatSection(args: ViewRenderArgs, lines: RenderedLine[]): void {
  const { snapshot } = args;
  lines.push({
    id: "chat.prime.header",
    spans: [{ text: "PRIME SESSIONS", emphasis: "bold" }],
  });

  if (snapshot.prime_configured !== true) {
    // Item 5: absence is named, not blank (REQ-AFF-004 honesty precedent).
    lines.push({
      id: "chat.prime.not_configured",
      spans: [
        {
          text: "Prime is not configured. Set GAUNTLET_WORKBENCH_PRIME_BINARY to enable Prime sessions.",
          token: "structure.neutral",
        },
      ],
    });
    return;
  }

  const sessions = snapshot.prime_sessions ?? [];
  if (sessions.length === 0) {
    lines.push({
      id: "chat.prime.empty",
      spans: [
        {
          text: "No Prime sessions yet. Launch one from the composer's prime mode.",
          emphasis: "dim",
        },
      ],
    });
    return;
  }

  const selectedId = snapshot.focus?.kind === "PrimeSession" ? snapshot.focus.source_id : null;
  for (const session of sessions) {
    const selected = session.session_id === selectedId;
    const token = primeSessionToken(session.status);
    lines.push({
      id: `chat.session.${session.session_id}`,
      spans: [
        { text: selected ? "▸ " : "  ", token: "focus.primary" },
        { text: `${SEMANTIC_TOKENS[token].glyph} `, token },
        { text: `${session.status.toUpperCase()} `, token, emphasis: "bold" },
        { text: `${session.session_id} `, emphasis: selected ? "bold" : "normal" },
        { text: `unit ${session.target_unit ?? "unknown"} `, emphasis: "dim" },
        { text: `admitted ${session.admitted_at}`, emphasis: "dim" },
      ],
    });
  }
  lines.push({
    id: "chat.prime.hint",
    spans: [
      {
        text: "Prime messages are Prime-mailbox communication, not completion. Status comes from the admission/result contract only.",
        emphasis: "dim",
      },
    ],
  });

  const selectedSession = sessions.find((session) => session.session_id === selectedId);
  if (selectedSession === undefined) {
    lines.push({
      id: "chat.prime.select_hint",
      spans: [
        {
          text: "Select a session above (Enter or click) to view its transcript.",
          emphasis: "dim",
        },
      ],
    });
    return;
  }

  lines.push({
    id: `chat.prime.transcript.${selectedSession.session_id}`,
    spans: [{ text: `TRANSCRIPT — ${selectedSession.session_id}`, emphasis: "bold" }],
  });
  if (selectedSession.messages.length === 0) {
    lines.push({
      id: "chat.prime.transcript.empty",
      spans: [{ text: "No messages yet.", emphasis: "dim" }],
    });
    return;
  }
  for (const message of selectedSession.messages) {
    lines.push({
      id: `chat.prime.message.${message.message_id}`,
      spans: [
        { text: `${message.sender} → ${message.recipient}: `, emphasis: "bold" },
        { text: summarizeMailboxContent(message.content) },
        { text: `  (${message.kind} · ${message.timestamp})`, emphasis: "dim" },
      ],
    });
  }
}

/** Chat: "What was communicated?" (REQ-TUI-003). */
export function renderChatView(args: ViewRenderArgs): ViewContent {
  const { snapshot } = args;
  const lines: RenderedLine[] = [];
  const records: RenderedRecord[] = [];

  lines.push({
    id: "chat.question",
    spans: [{ text: "CHAT: what was communicated?", emphasis: "bold" }],
  });

  renderPrimeChatSection(args, lines);

  if (!snapshot.chat_enabled) {
    // Chat is subordinate to durable history (DSN-010): an unprojected Chat
    // says so honestly and never impersonates the execution history.
    lines.push({
      id: "chat.disabled",
      spans: [
        {
          text: "Chat projection disabled by the host; communication is not projected.",
          token: "structure.neutral",
        },
      ],
    });
    lines.push({
      id: "chat.disabled.trace",
      spans: [{ text: "Durable execution history remains available in Trace.", emphasis: "dim" }],
    });
    return { view: "chat", question: "What was communicated?", lines, records };
  }

  if (snapshot.chat_messages.length === 0) {
    lines.push({
      id: "chat.empty",
      spans: [
        {
          text: "No messages yet. Communication appears here as it is projected.",
          emphasis: "dim",
        },
      ],
    });
    return { view: "chat", question: "What was communicated?", lines, records };
  }

  for (const message of snapshot.chat_messages) {
    lines.push({
      id: `chat.${message.message_id}`,
      spans: [
        ...(message.role === "system"
          ? [
              {
                text: `${message.role}: `,
                token: "structure.neutral" as const,
                emphasis: "bold" as const,
              },
            ]
          : [{ text: `${message.role}: `, emphasis: "bold" as const }]),
        { text: message.content },
        { text: `  (${message.message_id} @ ${message.timestamp_ms})`, emphasis: "dim" },
      ],
    });
    records.push({
      id: `chat-message:${message.message_id}`,
      kind: "attention",
      fields: {
        role: message.role,
        content: message.content,
        message_id: message.message_id,
        timestamp_ms: String(message.timestamp_ms),
      },
    });
  }
  return { view: "chat", question: "What was communicated?", lines, records };
}

/**
 * Trace: "What actually happened?" over the durable execution history.
 *
 * Plan workbench-agent-interaction.md P2, item 4 (deliberately SKIPPED here,
 * documented rather than forced): Prime child lifecycle events (admitted,
 * turn started/resumed, reply received, completed/failed/stopped) are NOT
 * added to Trace. ICM's own "Trace terminology" preamble and UESC §16.0 fix
 * Trace as one ordered ledger over exactly two source-typed streams — Gauntlet
 * execution history and the RealityTrace consequence ledger — "the sources
 * are never merged"; REQ-DATA-005 separately requires "Chat and Trace actions
 * operate on distinct projections", and the truth-ownership matrix lists
 * "Prime messages | Prime mailbox | Project communication, not completion" as
 * its own row, distinct from both Trace sources. A Prime child is neither
 * Gauntlet execution history nor a RealityTrace ledger record, so folding its
 * lifecycle into this ledger would be a third, uninvited source and would
 * violate both the two-source invariant and REQ-DATA-005. Prime lifecycle
 * belongs in Chat (`renderPrimeChatSection`, above), which is exactly where
 * the spec already places Prime messages/mailbox truth.
 */
export function renderTraceView(args: ViewRenderArgs): ViewContent {
  const { snapshot } = args;
  const lines: RenderedLine[] = [];
  const records: RenderedRecord[] = [];

  lines.push({
    id: "trace.question",
    spans: [{ text: "TRACE: what actually happened?", emphasis: "bold" }],
  });

  if (snapshot.trace_records.length === 0) {
    lines.push({
      id: "trace.empty",
      spans: [{ text: "No execution history recorded yet for this projection.", emphasis: "dim" }],
    });
    appendRealityTraceSegment(lines, records, args);
    return { view: "trace", question: "What actually happened?", lines, records };
  }

  // GUI redesign (UESC §16.2, ICM §16.4-16.6): when the shell passes the
  // TraceStream window, the ledger renders the window with its follow state
  // and an explicit unseen-count affordance; it NEVER jumps to the live tail
  // while the operator holds on history.
  if (args.traceWindow !== undefined) {
    const { followState, unseenCount } = args.traceWindow;
    const stateLabel =
      followState === "LIVE_FOLLOW"
        ? "LIVE · following"
        : followState === "INSPECTION_HOLD"
          ? `INSPECTING history${unseenCount > 0 ? ` · ${unseenCount} new` : ""}`
          : followState === "HYDRATING"
            ? "HYDRATING"
            : "COLD";
    const stateToken =
      followState === "LIVE_FOLLOW"
        ? ("execution.running" as const)
        : unseenCount > 0
          ? ("attention.required" as const)
          : ("evidence.stale" as const);
    lines.push({
      id: "trace.follow",
      spans: [
        { text: `${SEMANTIC_TOKENS[stateToken].glyph} ${stateLabel}`, token: stateToken },
        ...(followState === "INSPECTION_HOLD"
          ? [
              {
                text: unseenCount > 0 ? " ▾ g return to live" : " · g return to live",
                emphasis: "dim" as const,
                ...(unseenCount > 0
                  ? { token: "attention.required" as const, emphasis: "bold" as const }
                  : {}),
              },
            ]
          : []),
      ],
    });
    for (const record of args.traceWindow.records) {
      pushTraceRow(lines, records, record);
    }
    if (args.traceWindow.records.length === 0) {
      lines.push({
        id: "trace.window.empty",
        spans: [{ text: "Window empty at this scroll position.", emphasis: "dim" }],
      });
    }
    appendRealityTraceSegment(lines, records, args);
    return { view: "trace", question: "What actually happened?", lines, records };
  }

  for (const record of snapshot.trace_records) {
    pushTraceRow(lines, records, record);
  }
  appendRealityTraceSegment(lines, records, args);
  return { view: "trace", question: "What actually happened?", lines, records };
}

/**
 * RealityTrace ledger segment (GUI-redesign increment 6, UESC §16.0/§16.2/
 * §16.11): appended after the gauntlet trace rendering, one ordered ledger
 * over two source-typed streams that are NEVER merged. Absent `args.sxrLane`
 * = unbound or never refreshed — renders nothing (unchanged current
 * behavior).
 */
function appendRealityTraceSegment(
  lines: RenderedLine[],
  records: RenderedRecord[],
  args: ViewRenderArgs,
): void {
  if (args.sxrLane === undefined) {
    return;
  }
  const lane = args.sxrLane;
  const stale = args.sxrRefreshEpoch !== undefined && laneStale(lane, args.sxrRefreshEpoch);
  lines.push({
    id: "trace.realitytrace.header",
    spans: [
      { text: "REALITYTRACE", emphasis: "bold" },
      ...(stale
        ? [
            {
              text: ` · stale (last refresh epoch ${lane.refreshEpoch})`,
              token: "evidence.stale" as const,
            },
          ]
        : []),
    ],
  });
  if (lane.rows.length === 0) {
    lines.push({
      id: "trace.realitytrace.empty",
      spans: [{ text: "Ledger bound; no interpreted rows yet.", emphasis: "dim" }],
    });
    return;
  }
  for (const row of lane.rows) {
    lines.push({
      id: `trace.realitytrace.${row.id}`,
      spans: [
        { text: `${row.kind} `, emphasis: "dim" },
        { text: `${row.label} `, emphasis: "bold" },
        { text: `ordinal ${row.ordinal} · ${row.entryHash}`, emphasis: "dim" },
      ],
    });
    records.push({
      id: `trace.realitytrace.${row.id}`,
      kind: "evidence",
      fields: {
        ordinal: String(row.ordinal),
        entryHash: row.entryHash,
        kind: row.kind,
        label: row.label,
      },
    });
  }
}

/** One ledger row: glyph + typed event + summary + machine provenance. */
function pushTraceRow(
  lines: RenderedLine[],
  records: RenderedRecord[],
  record: import("./workbench-model").TraceRecord,
): void {
  const token = traceEventToken(record.event_type);
  lines.push({
    id: `trace.${record.trace_id}`,
    spans: [
      { text: `${SEMANTIC_TOKENS[token].glyph} `, token },
      { text: `${record.event_type.toUpperCase()} `, token, emphasis: "bold" },
      { text: `${record.summary} `, emphasis: "normal" },
      { text: `unit ${record.unit_id} · ${record.trace_id}`, emphasis: "dim" },
    ],
  });
  records.push({
    id: `trace-record:${record.trace_id}`,
    kind: "relation",
    fields: {
      event_type: record.event_type,
      unit_id: record.unit_id,
      trace_id: record.trace_id,
      timestamp_ms: String(record.timestamp_ms),
      summary: record.summary,
    },
    token,
  });
}

/** Evidence lower surface: inspectable evidence record objects (DSN-008). */
export function renderEvidenceView(args: ViewRenderArgs): ViewContent {
  const { snapshot } = args;
  const lines: RenderedLine[] = [];
  const records: RenderedRecord[] = [];

  lines.push({
    id: "evidence.question",
    spans: [{ text: "EVIDENCE: what supports or contradicts settlement?", emphasis: "bold" }],
  });

  if (snapshot.evidence.length === 0) {
    lines.push({
      id: "evidence.empty",
      spans: [{ text: "No evidence records in the current projection.", emphasis: "dim" }],
    });
    return {
      view: "evidence",
      question: "What supports or contradicts settlement?",
      lines,
      records,
    };
  }

  for (const evidence of snapshot.evidence) {
    const record = renderEvidenceRecord(evidence);
    lines.push({
      id: record.id,
      spans: [
        { text: `${record.fields["result"]} `, token: record.token },
        { text: `${record.fields["identity"]} `, emphasis: "bold" },
        { text: `${record.fields["claim"]} `, emphasis: "normal" },
        { text: `(${record.fields["provenance"] ?? ""})`, emphasis: "dim" },
      ],
    });
    records.push(record);
  }
  return { view: "evidence", question: "What supports or contradicts settlement?", lines, records };
}

function resolveFocusLabel(snapshot: FullWorkbenchSnapshot, ref: FocusRef): string {
  const unit = snapshot.units.find((candidate) => candidate.unit_id === ref.source_id);
  if (unit !== undefined) {
    return unit.title || ref.source_id;
  }
  const session = snapshot.sessions.find((candidate) => candidate.session_id === ref.source_id);
  if (session !== undefined) {
    return `${session.model} (${session.status})`;
  }
  return ref.human_label ?? ref.source_id;
}

/** Agents lower surface: "Where is execution capacity allocated?" */
export function renderAgentsView(args: ViewRenderArgs): ViewContent {
  const { snapshot } = args;
  const lines: RenderedLine[] = [];
  const records: RenderedRecord[] = [];

  lines.push({
    id: "agents.question",
    spans: [{ text: "AGENTS: where is execution capacity allocated?", emphasis: "bold" }],
  });

  for (const session of snapshot.sessions) {
    const token: SemanticToken =
      session.status === "running" ? "execution.running" : "execution.waiting";
    lines.push({
      id: `agents.${session.session_id}`,
      spans: [
        { text: `${SEMANTIC_TOKENS[token].glyph} `, token },
        { text: `${session.status.toUpperCase()} `, token, emphasis: "bold" },
        { text: `${session.session_id} `, emphasis: "normal" },
        { text: session.model },
        {
          text: session.parent_id === null ? "  (root)" : `  (child of ${session.parent_id})`,
          emphasis: "dim",
        },
      ],
    });
  }

  // Declared agent-context placements of any Focus: alias rows resolved
  // through the snapshot — a placement never carries its own state copy
  // (DSN-006). The `≈` marker signals an alias, not a second object.
  const agentPlacements = snapshot.placements.filter((placement) => placement.view === "agents");
  for (const placement of agentPlacements) {
    lines.push({
      id: `agents.placement.${placement.id}`,
      spans: [
        { text: "≈ ", token: "structure.neutral" },
        { text: `${placement.focus.kind}:${placement.focus.source_id} `, emphasis: "bold" },
        { text: resolveFocusLabel(snapshot, placement.focus) },
        {
          text: `  (alias under agent context${placement.relation_role !== undefined ? ` · ${placement.relation_role}` : ""})`,
          emphasis: "dim",
        },
      ],
    });
  }

  if (snapshot.sessions.length === 0 && agentPlacements.length === 0) {
    lines.push({
      id: "agents.empty",
      spans: [{ text: "No agent sessions in the current projection.", emphasis: "dim" }],
    });
  }
  return { view: "agents", question: "Where is execution capacity allocated?", lines, records };
}

/** Attention summary view: the Case-wide urgent queue (REQ-UX-002). */
export function renderAttentionView(args: ViewRenderArgs): ViewContent {
  const { snapshot } = args;
  const lines: RenderedLine[] = [];
  const records: RenderedRecord[] = [];

  lines.push({
    id: "attention.question",
    spans: [{ text: "ATTENTION: what genuinely requires resolution?", emphasis: "bold" }],
  });

  if (snapshot.attention.length === 0) {
    lines.push({
      id: "attention.empty",
      spans: [{ text: "Nothing requires operator resolution right now.", emphasis: "dim" }],
    });
    return { view: "attention", question: "What genuinely requires resolution?", lines, records };
  }

  for (const entry of snapshot.attention) {
    lines.push({
      id: `attention.${entry.focus.id}`,
      spans: [
        { text: "! ", token: "attention.required", emphasis: "bold" },
        { text: `${entry.severity.toUpperCase()} `, token: "attention.required" },
        { text: `${entry.reason} `, emphasis: "bold" },
        { text: `${entry.focus.kind}:${entry.focus.source_id} `, emphasis: "normal" },
        { text: entry.horizon_independent ? "(Case-wide)" : "(horizon-scoped)", emphasis: "dim" },
      ],
    });
    records.push({
      id: `attention-record:${entry.focus.id}`,
      kind: "attention",
      token: "attention.required",
      fields: {
        reason: entry.reason,
        severity: entry.severity,
        target: entry.focus.id,
        horizon_independent: String(entry.horizon_independent),
      },
    });
  }
  return { view: "attention", question: "What genuinely requires resolution?", lines, records };
}

export type { ViewRenderArgs };

/**
 * The default shell view registry: the three required primary projections
 * plus only the lower surfaces whose providers this snapshot model actually
 * publishes. Roadmap surfaces (Harness) cannot be registered — see shell.ts.
 */
export function defaultViewRegistry(): {
  work: ViewContentRenderer;
  chat: ViewContentRenderer;
  trace: ViewContentRenderer;
  evidence: ViewContentRenderer;
  agents: ViewContentRenderer;
  attention: ViewContentRenderer;
} {
  return {
    work: renderWorkView,
    chat: renderChatView,
    trace: renderTraceView,
    evidence: renderEvidenceView,
    agents: renderAgentsView,
    attention: renderAttentionView,
  };
}

export type ViewContentRenderer = (args: ViewRenderArgs) => ViewContent;
