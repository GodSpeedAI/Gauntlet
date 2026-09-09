import { describe, expect, it } from "bun:test";
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import type { AffordanceObservation } from "./affordance";
import { resolveAffordance } from "./affordance";
import { FocusRef } from "./focus";
import type { ShellState, ViewId } from "./shell";
import {
  availableViews,
  createShellState,
  paletteEntries,
  ROADMAP_VIEW_IDS,
  renderShell,
} from "./shell";
import { OPERATOR_SESSION, observationFor, seededModel } from "./testing/shell-harness";
import type { ColorCapability, SemanticToken } from "./theme";
import { goldReservedTokens, resolveToken, SEMANTIC_TOKENS } from "./theme";
import { defaultViewRegistry } from "./views-chat-trace";
import { materializeAffordances, renderWorkView } from "./views-work";

/**
 * T12 work-chat-trace suites: the three primary projections over ONE history
 * (REQ-TUI-003, REQ-DATA-005, DSN-010), the DSN-015 no-roadmap-surface
 * assertion, the DSN-001 raw-colour-literal ban, DSN-002 monochrome
 * distinguishability, DSN-003 gold discipline, DSN-004 distinct token
 * families, DSN-005 resolver-driven enablement, and DSN-009 blocked vs
 * palette-discoverable separation.
 */

const SHELL_SOURCES = [
  "shell.ts",
  "views-work.ts",
  "views-chat-trace.ts",
  "inspector.ts",
  "composer.ts",
  "render-tree.ts",
];

function sourceOf(file: string): string {
  return readFileSync(resolve(import.meta.dir, file), "utf-8");
}

function stateAt(): ShellState {
  return createShellState(
    { workspace: { workspace: "gauntlet", reference: "ref-1d457ec9" } },
    { cols: 120, rows: 40 },
  );
}

describe("T12 Work/Chat/Trace: three questions over the same history (REQ-TUI-003)", () => {
  it("switches views while selection identity is retained across all three", () => {
    const model = seededModel();
    const unitRef = FocusRef.for("gauntlet", "WorkItem", "u-root");
    model.selectFocus(unitRef);
    const snapshot = model.getSnapshot();
    const registry = defaultViewRegistry();

    for (const view of ["work", "chat", "trace"] as const) {
      const content = registry[view]({
        snapshot,
        observation: observationFor(model),
      });
      expect(content.view).toBe(view);
      // The same snapshot, the same selection identity — every view.
      expect(snapshot.focus).toBe(unitRef);
      expect(content.lines.length).toBeGreaterThan(0);
    }
  });

  it("renders chat from chat_messages and trace from trace_records of the same snapshot", () => {
    const model = seededModel();
    const snapshot = model.getSnapshot();
    const registry = defaultViewRegistry();

    const chatContent = registry.chat({ snapshot, observation: observationFor(model) });
    const trace = registry.trace({ snapshot, observation: observationFor(model) });
    const chatText = chatContent.lines
      .map((line) => line.spans.map((s) => s.text).join(""))
      .join("\n");
    const traceText = trace.lines.map((line) => line.spans.map((s) => s.text).join("")).join("\n");

    expect(snapshot.chat_messages.length).toBeGreaterThan(0);
    expect(snapshot.trace_records.length).toBeGreaterThan(0);
    const firstChatMessage = snapshot.chat_messages[0];
    expect(firstChatMessage).toBeDefined();
    if (firstChatMessage !== undefined) {
      expect(chatText).toContain(firstChatMessage.content);
    }
    const firstTraceRecord = snapshot.trace_records[0];
    expect(firstTraceRecord).toBeDefined();
    if (firstTraceRecord !== undefined) {
      expect(traceText).toContain(firstTraceRecord.event_type.toUpperCase());
    }
    expect(traceText).toContain("UNIT_UPDATED");
  });
});

describe("T12 DSN-010: Chat is never canonical input for Trace", () => {
  it("removing the Chat projection leaves Trace byte-identical at the view layer", () => {
    const model = seededModel();
    const registry = defaultViewRegistry();

    const before = model.getSnapshot();
    const traceBefore = registry.trace({ snapshot: before, observation: observationFor(model) });
    expect(before.chat_messages.length).toBeGreaterThan(0);

    // Remove Chat entirely.
    model.setChatEnabled(false);
    const after = model.getSnapshot();
    expect(after.chat_messages.length).toBe(0);
    expect(after.chat_enabled).toBe(false);

    const traceAfter = registry.trace({ snapshot: after, observation: observationFor(model) });
    expect(traceAfter.lines).toEqual(traceBefore.lines);
    expect(traceAfter.records).toEqual(traceBefore.records);

    // Trace content still covers the same durable history.
    const traceText = traceAfter.lines
      .map((line) => line.spans.map((s) => s.text).join(""))
      .join("\n");
    expect(traceText).toContain("TRACE");
    expect(after.trace_records.length).toBe(before.trace_records.length);

    // Chat renders an honest disabled state and never impersonates history.
    const chat = registry.chat({ snapshot: after, observation: observationFor(model) });
    const chatText = chat.lines.map((line) => line.spans.map((s) => s.text).join("")).join("\n");
    expect(chatText).toContain("Chat projection disabled");
    expect(chatText).not.toContain("UNIT_UPDATED");
  });
});

describe("T12 DSN-015: no roadmap surface exists anywhere in the shell", () => {
  it("registry, availability table and view ids contain no Harness View", () => {
    const registry = defaultViewRegistry();
    const model = seededModel();
    const availability = availableViews(model.getSnapshot());

    const harnessIsRoadmap = (ROADMAP_VIEW_IDS as readonly string[]).includes("harness");
    expect(harnessIsRoadmap).toBe(true);
    for (const key of Object.keys(registry)) {
      expect((ROADMAP_VIEW_IDS as readonly string[]).includes(key)).toBe(false);
    }
    for (const entry of availability) {
      expect(entry.view).not.toBe("harness");
    }
    // No roadmap id can even be expressed as a ViewId; the absence of any
    // harness renderer anywhere is asserted by the registry key scan above.
  });
});

describe("T12 DSN-001: no raw colour literals outside the theme adapter", () => {
  it("shell source files contain no hex colours, rgb() calls or ANSI escapes", () => {
    const hex = /#[0-9a-fA-F]{3,8}\b/;
    const rgb = /\brgb\(/;
    const ansiEscapeSource = /\\x1b|\\u001b/;
    const literalEscape = String.fromCharCode(27);
    for (const file of SHELL_SOURCES) {
      const source = sourceOf(file);
      expect(source.match(hex)).toBeNull();
      expect(source.match(rgb)).toBeNull();
      expect(source.match(ansiEscapeSource)).toBeNull();
      expect(source.includes(literalEscape)).toBe(false);
    }
  });

  it("the theme adapter is the one place raw colour values live (sanity: the regex works)", () => {
    const themeSource = sourceOf("theme.ts");
    expect(themeSource.match(/#[0-9a-fA-F]{6}\b/)).not.toBeNull();
  });
});

describe("T12 DSN-002: monochrome lowering keeps every consequential state distinguishable", () => {
  it("lowers every token to colourless presentation with glyph+label+emphasis intact", () => {
    const capabilities: ColorCapability[] = ["truecolor", "color256", "monochrome"];
    for (const token of Object.keys(SEMANTIC_TOKENS) as SemanticToken[]) {
      const spec = SEMANTIC_TOKENS[token];
      for (const capability of capabilities) {
        const resolved = resolveToken(token, capability);
        expect(resolved.glyph).toBe(spec.glyph);
        expect(resolved.label).toBe(spec.label);
        expect(resolved.emphasis).toBe(spec.emphasis);
        expect(resolved.family).toBe(spec.family);
        if (capability === "monochrome") {
          expect(resolved.color).toBeNull();
          expect(resolved.color256).toBeNull();
        } else if (capability === "color256") {
          expect(resolved.color256).not.toBeNull();
        } else {
          expect(resolved.color).toBe(spec.color);
        }
      }
    }
  });

  it("token identities stay distinct in monochrome: family|glyph|label|emphasis is unique per token", () => {
    const tokens = Object.keys(SEMANTIC_TOKENS) as SemanticToken[];
    const identities = new Set(
      tokens.map((token) => {
        const resolved = resolveToken(token, "monochrome");
        return [resolved.family, resolved.glyph, resolved.label, resolved.emphasis].join("|");
      }),
    );
    expect(identities.size).toBe(tokens.length);
  });

  it("renders Work rows whose states stay distinguishable as plain text", () => {
    const model = seededModel();
    const content = renderWorkView({
      snapshot: model.getSnapshot(),
      observation: observationFor(model),
    });
    const text = content.lines.map((line) => line.spans.map((s) => s.text).join("")).join("\n");
    expect(text).toContain("BLOCKED");
    expect(text).toContain("ACTIVE");
    expect(text).toContain("×"); // blocked glyph
    expect(text).toContain("●"); // running glyph
  });
});

describe("T12 DSN-003: gold is reserved for proof and sanctioned authority", () => {
  it("only the sanctioned authority and independent proof tokens carry brand gold", () => {
    const GOLD = "#D7A84A";
    expect([...goldReservedTokens()].sort()).toEqual(["authority.sanctioned", "proof.independent"]);
    for (const token of Object.keys(SEMANTIC_TOKENS) as SemanticToken[]) {
      const spec = SEMANTIC_TOKENS[token];
      if (goldReservedTokens().includes(token)) {
        expect(spec.color).toBe(GOLD);
        expect(["authority", "evidence"]).toContain(spec.family);
      } else {
        expect(spec.color).not.toBe(GOLD);
      }
    }
    // Selection, hover, ordinary success and decoration never resolve gold.
    for (const token of [
      "focus.primary",
      "focus.ring",
      "execution.succeeded",
      "readiness.ready",
    ] as const) {
      expect(SEMANTIC_TOKENS[token].color).not.toBe(GOLD);
    }
  });
});

describe("T12 DSN-004: token families stay distinct even where colours coincide", () => {
  it("all seven required families exist as separate semantic identities", () => {
    const families = new Set(Object.values(SEMANTIC_TOKENS).map((spec) => spec.family));
    for (const family of [
      "authority",
      "execution",
      "evidence",
      "settlement",
      "readiness",
      "integrity",
      "capability",
    ]) {
      expect(families.has(family as never)).toBe(true);
    }
  });

  it("same default colour never means same state: amber tokens keep distinct family+label", () => {
    const amber = "#C58A2A";
    const amberTokens = (Object.keys(SEMANTIC_TOKENS) as SemanticToken[]).filter(
      (token) => SEMANTIC_TOKENS[token].color === amber,
    );
    // authority.escalated, settlement.escalated, evidence.missing, evidence.stale,
    // readiness.degraded, capability.degraded, attention.required ...
    expect(amberTokens.length).toBeGreaterThanOrEqual(4);
    const identities = new Set(
      amberTokens.map(
        (token) => `${SEMANTIC_TOKENS[token].family}|${SEMANTIC_TOKENS[token].label}`,
      ),
    );
    expect(identities.size).toBe(amberTokens.length);
  });
});

describe("T12 DSN-005: enablement comes from the resolver, never a component predicate", () => {
  it("rendered affordance states deep-equal fresh resolveAffordance results", () => {
    const model = seededModel();
    const focus = FocusRef.for("gauntlet", "WorkItem", "u-far");
    model.selectFocus(focus);
    const snapshot = model.getSnapshot();
    const observation = observationFor(model);
    const content = renderWorkView({ snapshot, observation, session: OPERATOR_SESSION });
    const affordanceRecords = content.records.filter((record) => record.kind === "affordance");

    expect(affordanceRecords.length).toBe(2); // bar items only: resolve + retry (refresh is palette-only)
    for (const record of affordanceRecords) {
      const descriptor = resolveAffordance(record.fields["operation"] ?? "", focus, observation, {
        session: OPERATOR_SESSION,
      });
      expect(record.fields["status"]).toBe(descriptor.status);
      expect(record.fields["reasons"]).toBe(descriptor.reason_codes.join(",") || "none");
    }
  });

  it("flipping ONLY the observation flips the rendered state — no view-local pathway exists", () => {
    const model = seededModel();
    const focus = FocusRef.for("gauntlet", "WorkItem", "u-root");

    // With an open escalation and authority, resolve responds to the ticket.
    const allowed: AffordanceObservation = observationFor(model, {
      open_escalations: [{ ticket_id: "ESC-9", unit_id: "u-root" }],
    });
    // Revoke ONLY the authority fact: the same control must flip.
    const revoked: AffordanceObservation = observationFor(model, {
      open_escalations: [{ ticket_id: "ESC-9", unit_id: "u-root" }],
      authority_revoked_units: ["u-root"],
    });

    const before = materializeAffordances(focus, allowed, { session: OPERATOR_SESSION }).find(
      (item) => item.descriptor.operation === "unit.resolve",
    );
    const after = materializeAffordances(focus, revoked, { session: OPERATOR_SESSION }).find(
      (item) => item.descriptor.operation === "unit.resolve",
    );
    expect(before?.descriptor.status).toBe("requires_attention");
    expect(after?.descriptor.status).toBe("unavailable");
    expect(after?.descriptor.reason_codes).toContain("authority_not_permitted");
    expect(after?.nextStep).toContain("authority");
  });
});

describe("T12 DSN-009: blocked-with-reason vs absent-but-palette-discoverable", () => {
  it("materialises a blocked affordance with its gate and next step; hides illegitimate ones from the bar but keeps them in the palette", () => {
    const model = seededModel();
    const focus = FocusRef.for("gauntlet", "WorkItem", "u-far");
    model.selectFocus(focus);
    const snapshot = model.getSnapshot();
    const observation = observationFor(model);
    const content = renderWorkView({ snapshot, observation, session: OPERATOR_SESSION });

    const materialized = materializeAffordances(focus, observation, {
      session: OPERATOR_SESSION,
    });
    const resolve = materialized.find((item) => item.descriptor.operation === "unit.resolve");
    const refresh = materialized.find((item) => item.descriptor.operation === "state.refresh");

    // unit.resolve on a visible unit: important + blocked (no open ticket)
    // → materialised WITH the gate and a valid next step.
    expect(resolve?.outcome).toBe("blocked_with_reason");
    expect(resolve?.descriptor.reason_codes).toContain("no_open_ticket");
    expect(resolve?.nextStep).toContain("escalation ticket");

    // state.refresh is not an action: absent from the action bar...
    expect(refresh?.outcome).toBe("palette_only");
    const barLineIds = content.lines.map((line) => line.id ?? "");
    expect(barLineIds.some((id) => id === "work.affordance.state.refresh")).toBe(false);
    // ...but discoverable in the palette footer WITH its reason...
    const text = content.lines.map((line) => line.spans.map((s) => s.text).join("")).join("\n");
    expect(text).toContain("more in command palette");
    expect(text).toContain("state.refresh: not_action_admitted");
    // ...and fully inspectable in the palette entries with the reason.
    const state = stateAt();
    const entries = paletteEntries({ ...state, location: { ...state.location, focus } }, snapshot, {
      observation,
      session: OPERATOR_SESSION,
    });
    const refreshEntry = entries.find(
      (entry) => entry.type === "affordance" && entry.operation === "state.refresh",
    );
    expect(refreshEntry).toBeDefined();
    if (refreshEntry?.type === "affordance") {
      expect(refreshEntry.reasons).toContain("not_action_admitted");
    }
  });

  it("escalation observations upgrade the same affordance to requires_attention", () => {
    const model = seededModel();
    const focus = FocusRef.for("gauntlet", "WorkItem", "u-far");
    const withTicket = observationFor(model, {
      open_escalations: [{ ticket_id: "ESC-7", unit_id: "u-far" }],
    });
    const items = materializeAffordances(focus, withTicket, { session: OPERATOR_SESSION });
    const resolve = items.find((item) => item.descriptor.operation === "unit.resolve");
    expect(resolve?.descriptor.status).toBe("requires_attention");
    expect(resolve?.descriptor.required_attention_item).toBe("ESC-7");
  });
});

describe("T12 empty and stale projections render honest next steps (REQ-UX-001/006)", () => {
  it("a source-backed Case without units names the wait step; a bare generation names the bind step", () => {
    const model = seededModel();
    // A cursor exists (source reported) but no units are admitted yet.
    model.startGeneration("gen-empty");
    model.applyEvent({
      generation_id: "gen-empty",
      source_id: "gauntlet",
      cursor: "c-0",
      event_type: "session_updated",
      payload: { session_id: "s-0", parent_id: null, model: "model-a", status: "running" },
      timestamp_ms: 100,
    });
    model.markReady("gen-empty");
    const content = renderWorkView({
      snapshot: model.getSnapshot(),
      observation: observationFor(model),
    });
    const text = content.lines.map((line) => line.spans.map((s) => s.text).join("")).join("\n");
    expect(text).toContain("No units reported yet");
    expect(text).toContain("run a Gauntlet run");

    // A cold generation with NO source report names the bind step.
    const cold = seededModel();
    cold.startGeneration("gen-cold");
    cold.markReady("gen-cold");
    const coldContent = renderWorkView({
      snapshot: cold.getSnapshot(),
      observation: observationFor(cold),
    });
    const coldText = coldContent.lines
      .map((line) => line.spans.map((s) => s.text).join(""))
      .join("\n");
    expect(coldText).toContain("No authoritative source has reported");
    expect(coldText).toContain("gauntlet serve");
  });

  it("an unreachable observation renders surface_unreachable with a recovery next step", () => {
    const model = seededModel();
    const focus = FocusRef.for("gauntlet", "WorkItem", "u-root");
    const unreachable = observationFor(model, { reachable: false });
    const items = materializeAffordances(focus, unreachable);
    for (const item of items) {
      if (item.descriptor.operation === "state.refresh") {
        continue;
      }
      expect(item.descriptor.reason_codes).toContain("surface_unreachable");
      expect(item.nextStep).toContain("restore the canonical surface connection");
    }
  });
});

describe("T12 full-shell render over the registry", () => {
  it("renders every registered view through the shell without error", () => {
    const model = seededModel();
    const registry = defaultViewRegistry();
    const snapshot = model.getSnapshot();
    const state = stateAt();
    for (const view of availableViews(snapshot)) {
      if (view.status !== "available") {
        continue;
      }
      const rendered = renderShell(
        { ...state, location: { ...state.location, view: view.view as ViewId } },
        snapshot,
        registry,
        { observation: observationFor(model) },
      );
      expect(rendered.surfaces.length).toBeGreaterThan(0);
    }
  });
});
