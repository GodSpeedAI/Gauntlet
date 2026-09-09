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

/** Chat: "What was communicated?" (REQ-TUI-003). */
export function renderChatView(args: ViewRenderArgs): ViewContent {
  const { snapshot } = args;
  const lines: RenderedLine[] = [];
  const records: RenderedRecord[] = [];

  lines.push({
    id: "chat.question",
    spans: [{ text: "CHAT — what was communicated?", emphasis: "bold" }],
  });

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

/** Trace: "What actually happened?" over the durable execution history. */
export function renderTraceView(args: ViewRenderArgs): ViewContent {
  const { snapshot } = args;
  const lines: RenderedLine[] = [];
  const records: RenderedRecord[] = [];

  lines.push({
    id: "trace.question",
    spans: [{ text: "TRACE — what actually happened?", emphasis: "bold" }],
  });

  if (snapshot.trace_records.length === 0) {
    lines.push({
      id: "trace.empty",
      spans: [{ text: "No execution history recorded yet for this projection.", emphasis: "dim" }],
    });
    return { view: "trace", question: "What actually happened?", lines, records };
  }

  for (const record of snapshot.trace_records) {
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
  return { view: "trace", question: "What actually happened?", lines, records };
}

/** Evidence lower surface: inspectable evidence record objects (DSN-008). */
export function renderEvidenceView(args: ViewRenderArgs): ViewContent {
  const { snapshot } = args;
  const lines: RenderedLine[] = [];
  const records: RenderedRecord[] = [];

  lines.push({
    id: "evidence.question",
    spans: [{ text: "EVIDENCE — what supports or contradicts settlement?", emphasis: "bold" }],
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
    spans: [{ text: "AGENTS — where is execution capacity allocated?", emphasis: "bold" }],
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
    spans: [{ text: "ATTENTION — what genuinely requires resolution?", emphasis: "bold" }],
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
