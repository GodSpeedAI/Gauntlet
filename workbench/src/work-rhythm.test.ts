/**
 * Work-view rhythm pass + Inspector field-value wrap (DESIGN.md "Grid and
 * spacing" space.2/space.3, "Row density" standard class, text.machine
 * "tabular alignment", "Do not visually flatten labels, prose, hashes, and
 * statuses into one undifferentiated stream"; PRODUCT.md "quiet until
 * judgment is required").
 *
 * These suites close two recorded residuals:
 *   - tui_layout_pass: "Primary view internals (views-work.ts) left flat; a
 *     follow-up rhythm pass there is deferred, not started."
 *   - tui_critique_actions: "Inspector keeps 12-cell label padding ...;
 *     provenance values can still clip in the 36-col column."
 *
 * Discipline held: no new semantic token, no colour meaning added or removed
 * (DSN-001..004), glyph+label+emphasis legibility unchanged in monochrome
 * (DSN-002), escalation-first ordering untouched (REQ-UX-001), resolver-driven
 * affordances untouched (DSN-005/009), geometry floors untouched (DSN-007).
 */

import { describe, expect, it } from "bun:test";
import { FocusRef } from "./focus";
import { cellWidth, padCellText, wrapCellText } from "./geometry";
import { renderInspector } from "./inspector";
import { lineText, renderToText } from "./render-tree";
import { createShellState, renderShell } from "./shell";
import { coldModel, FULL_DIMS, GEN, observationFor, seededModel } from "./testing/shell-harness";
import { defaultViewRegistry } from "./views-chat-trace";
import { renderWorkView } from "./views-work";
import type { FullWorkbenchSnapshot } from "./workbench-model";
import { WorkbenchModel } from "./workbench-model";

const LONG_GEN = "5d1e8324-c4c6-4a6b-a95b-fad8c38c7596";

function workLines(model: WorkbenchModel) {
  return renderWorkView({
    snapshot: model.getSnapshot(),
    observation: observationFor(model),
  }).lines;
}

/** A model whose generation id is a 36-char unbreakable token. */
function modelWithLongGeneration(): WorkbenchModel {
  const model = new WorkbenchModel();
  model.startGeneration(LONG_GEN);
  model.applyEvent({
    generation_id: LONG_GEN,
    source_id: "gauntlet",
    cursor: "c-1",
    event_type: "unit_updated",
    payload: {
      unit_id: "u-root",
      title: "Root Unit",
      status: "active",
      authority: "builder",
      blast_radius: "isolated",
    },
    timestamp_ms: 1000,
  });
  model.markReady(LONG_GEN);
  return model;
}

describe("Geometry: cell padding and wrapping primitives", () => {
  it("padCellText pads to an exact cell width and never truncates", () => {
    expect(padCellText("ACTIVE", 7)).toBe("ACTIVE ");
    expect(padCellText("BLOCKED", 7)).toBe("BLOCKED");
    expect(padCellText("u-other", 5)).toBe("u-other");
    expect(padCellText("", 3)).toBe("   ");
  });

  it("wrapCellText wraps at word boundaries and hard-splits unbreakable tokens", () => {
    expect(wrapCellText("short", 22)).toEqual(["short"]);
    expect(wrapCellText("alpha beta gamma", 10)).toEqual(["alpha beta", "gamma"]);
    // A word longer than the width is hard-split at the cell boundary.
    expect(wrapCellText("abcdefghijklmnopqrstuvwx", 10)).toEqual([
      "abcdefghij",
      "klmnopqrst",
      "uvwx",
    ]);
    // Machine tokens break at their own delimiters (ids, paths, uuids) before
    // ever hard-splitting mid-token.
    expect(wrapCellText("focus:gauntlet:WorkItem:u-far", 22)).toEqual([
      "focus:gauntlet:",
      "WorkItem:u-far",
    ]);
    // Non-positive widths are a no-op (caller has no column budget).
    expect(wrapCellText("anything", 0)).toEqual(["anything"]);
    // Nothing is lost: chunks re-join to the original content.
    const wrapped = wrapCellText(
      "generation 5d1e8324-c4c6-4a6b-a95b-fad8c38c7596 source gauntlet",
      22,
    );
    expect(wrapped.every((part) => cellWidth(part) <= 22)).toBe(true);
    expect(wrapped.join(" ").replace(/\s+/g, "")).toContain("5d1e8324-c4c6-4a6b-a95b-fad8c38c7596");
  });
});

describe("Work view rhythm: groups separate, rows align (DESIGN.md spacing scale)", () => {
  it("separates adjacent non-empty groups with blank rhythm lines; empty groups leave no gap", () => {
    const lines = workLines(seededModel());
    const ids = lines.map((line) => line.id ?? "");

    // Seeded fixture: capability group empty; attention (u-far blocked),
    // units, and affordances populated.
    expect(ids).toContain("work.gap.attention");
    expect(ids).toContain("work.gap.units");
    expect(ids).toContain("work.gap.affordances");
    expect(ids).not.toContain("work.gap.capability");

    // Gap lines are the same blank rhythm-line idiom the Inspector uses.
    for (const line of lines) {
      if ((line.id ?? "").startsWith("work.gap.")) {
        expect(lineText(line)).toBe(" ");
      }
    }

    // Reading order: header block, gap, attention, gap, units, gap, actions.
    const at = (id: string) => ids.indexOf(id);
    expect(at("work.gap.attention")).toBeGreaterThan(at("work.summary"));
    expect(at("work.gap.attention")).toBeLessThan(at("work.gap.units"));
    expect(at("work.gap.units")).toBeLessThan(at("work.gap.affordances"));
    const firstAttention = ids.find((id) => id.startsWith("work.attention."));
    expect(firstAttention).toBeDefined();
    expect(at("work.gap.attention")).toBeLessThan(at(firstAttention ?? ""));
    expect(at("work.gap.affordances")).toBeLessThan(at("work.affordances.none"));

    // No gap opens or closes the view: first line is the question, last is content.
    expect(ids[0]).toBe("work.question");
    expect((ids.at(-1) ?? "").startsWith("work.gap.")).toBe(false);
  });

  it("aligns the status and machine-id columns of unit rows (tabular alignment)", () => {
    const lines = workLines(seededModel());
    const unitLines = lines.filter((line) => (line.id ?? "").startsWith("work.unit."));
    expect(unitLines.length).toBe(3);

    // glyph | status | id — every row identical width in each column, so the
    // title column starts at one cell across rows.
    const statusWidths = new Set(unitLines.map((line) => cellWidth(line.spans[1]?.text ?? "")));
    const idWidths = new Set(unitLines.map((line) => cellWidth(line.spans[2]?.text ?? "")));
    const glyphWidths = new Set(unitLines.map((line) => cellWidth(line.spans[0]?.text ?? "")));
    expect(statusWidths.size).toBe(1);
    expect(idWidths.size).toBe(1);
    expect(glyphWidths.size).toBe(1);
    // The status column is at least the widest status vocabulary word (7).
    expect([...statusWidths][0]).toBeGreaterThanOrEqual(7);
    // Escalation-first ordering survives the pass (REQ-UX-001).
    expect((unitLines[0]?.spans[1]?.text ?? "").trim()).toBe("BLOCKED");
    // The machine id and title survive verbatim.
    const flat = renderToText(unitLines);
    expect(flat).toContain("u-far");
    expect(flat).toContain("u-root");
    expect(flat).toContain("Root Unit");
  });

  it("carries the attention glyph only when something is blocked, and splits the budget reason into a trailing span", () => {
    const seeded = workLines(seededModel()).find((line) => line.id === "work.summary");
    expect(seeded).toBeDefined();
    const attentionSpans = (seeded?.spans ?? []).filter(
      (span) => span.token === "attention.required",
    );
    expect(attentionSpans.length).toBe(1);
    expect(attentionSpans[0]?.text).toBe("! 1 blocked ");

    const cold = workLines(coldModel()).find((line) => line.id === "work.summary");
    expect(cold).toBeDefined();
    const coldText = lineText(cold ?? { spans: [] });
    expect((cold?.spans ?? []).some((span) => span.token === "attention.required")).toBe(false);
    expect(coldText).toContain("0 blocked");
    // The decision ("budget: unavailable") is its own span; the long reason is
    // the LAST span so narrow terminals clip the explanation, never the state.
    const spans = cold?.spans ?? [];
    const budget = spans.find((span) => span.text.includes("budget: unavailable"));
    expect(budget).toBeDefined();
    expect(spans.at(-1)?.text).toContain("host budget read not bound");
  });

  it("splits empty states into a statement line and a next-step line with the guidance intact", () => {
    // Warm empty: a source cursor exists but no units were admitted.
    const warm = new WorkbenchModel();
    warm.startGeneration(GEN);
    warm.applyEvent({
      generation_id: GEN,
      source_id: "gauntlet",
      cursor: "c-1",
      event_type: "session_updated",
      payload: { session_id: "s-1", parent_id: null, model: "model-a", status: "running" },
      timestamp_ms: 1000,
    });
    warm.markReady(GEN);
    const warmLines = workLines(warm);
    const statement = warmLines.find((line) => line.id === "work.empty");
    const next = warmLines.find((line) => line.id === "work.empty.next");
    expect(statement).toBeDefined();
    expect(next).toBeDefined();
    expect(lineText(statement ?? { spans: [] })).toContain("No units reported yet.");
    expect(lineText(next ?? { spans: [] })).toContain("run a Gauntlet run");
    expect(warmLines.indexOf(statement ?? { spans: [] })).toBeLessThan(
      warmLines.indexOf(next ?? { spans: [] }),
    );

    // Cold empty: no authoritative source at all.
    const coldLines = workLines(coldModel());
    const coldStatement = coldLines.find((line) => line.id === "work.empty");
    const coldNext = coldLines.find((line) => line.id === "work.empty.next");
    expect(lineText(coldStatement ?? { spans: [] })).toContain(
      "No authoritative source has reported.",
    );
    expect(lineText(coldNext ?? { spans: [] })).toContain("gauntlet serve");
  });
});

describe("Inspector field-value wrap (closes the 36-col provenance clip residual)", () => {
  it("wraps long values under the value column, losing nothing, leaving short fields alone", () => {
    const model = modelWithLongGeneration();
    const focus = FocusRef.for("gauntlet", "WorkItem", "u-root");
    const { sections } = renderInspector({
      snapshot: model.getSnapshot(),
      focus,
      observation: observationFor(model),
      valueWidth: 22,
    });
    const provenance = sections.find((section) => section.id === "provenance");
    expect(provenance).toBeDefined();
    const lines = (provenance?.lines ?? []).map(lineText);

    // Label column (13) + value column (22) = the real 35-cell Full budget.
    expect(lines.every((text) => cellWidth(text) <= 35)).toBe(true);

    // The 36-char generation renders as a label line plus continuation lines
    // indented to the value column; hard-split chunks re-join to the exact id.
    const labelIndex = lines.findIndex((text) => text.startsWith("generation"));
    expect(labelIndex).toBeGreaterThanOrEqual(0);
    const continuations: string[] = [];
    for (const text of lines.slice(labelIndex + 1)) {
      if (!text.startsWith(" ".repeat(13))) break;
      continuations.push(text);
    }
    expect(continuations.length).toBeGreaterThan(0);
    // First chunk rides the label line; continuations carry the rest.
    const firstChunk = (provenance?.lines ?? [])[labelIndex]?.spans[1]?.text ?? "";
    expect(`${firstChunk}${continuations.join("")}`.replace(/ /g, "")).toBe(LONG_GEN);

    // A short field stays a single un-wrapped label line (12-cell label + gap).
    const sourceLine = lines.find((text) => text.startsWith("source"));
    expect(sourceLine).toBe("source       gauntlet");
  });

  it("the shell passes the Inspector column width so the production path wraps", () => {
    const model = modelWithLongGeneration();
    model.selectFocus(FocusRef.for("gauntlet", "WorkItem", "u-root"));
    const state = createShellState(
      { workspace: { workspace: "gauntlet", reference: "ref-1d457ec9" } },
      FULL_DIMS,
    );
    const rendered = renderShell(state, model.getSnapshot(), defaultViewRegistry(), {
      observation: observationFor(model),
    });
    const inspector = rendered.surfaces.find((surface) => surface.id === "Inspector");
    expect(inspector).toBeDefined();
    const lines = (inspector?.lines ?? []).map(lineText);
    const text = lines.join("\n");

    // Continuation lines exist in the rendered Inspector surface...
    expect(lines.some((l) => l.startsWith(" ".repeat(13)) && l.trim().length > 0)).toBe(true);
    // ...and the full generation id remains fully visible there (not clipped,
    // not hidden): whitespace-stripped join still carries every character.
    expect(text.replace(/\s/g, "")).toContain(LONG_GEN);
  });
});

describe("Shell header: location outlives metadata in clip order", () => {
  it("shortens the header generation to an 8-char prefix while the Inspector keeps the full id", () => {
    const model = modelWithLongGeneration();
    const state = createShellState(
      { workspace: { workspace: "gauntlet", reference: "ref-1d457ec9", run: "R17" } },
      FULL_DIMS,
    );
    const rendered = renderShell(
      state,
      model.getSnapshot() as FullWorkbenchSnapshot,
      defaultViewRegistry(),
      {
        observation: observationFor(model),
      },
    );
    const headerText = lineText(rendered.header);
    expect(headerText).toContain("gen 5d1e8324");
    expect(headerText).not.toContain(LONG_GEN);
    // Identity and location survive ahead of the metadata.
    expect(headerText).toContain("Gauntlet");
    expect(headerText).toContain("ref-1d457ec9");
    expect(headerText).toContain("R17");
  });
});
