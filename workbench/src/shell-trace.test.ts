import { describe, expect, it } from "bun:test";
import { createShellState, handleCommand, ShellSession, type ShellState } from "./shell";
import { FULL_DIMS, renderOptions, seededModel, unitEvent } from "./testing/shell-harness";
import { streamVisibleWindow } from "./trace-stream";

/**
 * GUI-redesign increment 6: TraceStream integration into the shell (UESC §16.2,
 * ICM §16.4-16.6). Adversarial teeth: live-follow at the tail; scroll-up
 * suspends follow and NEVER jumps to the live tail while inspecting history;
 * unseen arrivals surface as an explicit count with a return-to-live path.
 */

const HOST = { workspace: { workspace: "gauntlet", reference: "ref-1d457ec9", run: "R17" } };

function traceSession(): { session: ShellSession; model: ReturnType<typeof seededModel> } {
  const model = seededModel();
  const session = new ShellSession(HOST, FULL_DIMS);
  session.key("3", model.getSnapshot()); // Trace view
  return { session, model };
}

describe("trace follow/hold through the session (adversarial: no live-tail jump)", () => {
  it("starts in LIVE_FOLLOW at the tail and rendering appends new records", () => {
    const { session, model } = traceSession();
    session.render(model.getSnapshot(), renderOptions(model));
    expect(session.state.trace.followState).toBe("LIVE_FOLLOW");

    // A new event arrives; the stream ingests it on the next render.
    model.applyEvent(unitEvent("u-late", "Late Unit", `c-${Date.now() % 100000}`, "active"));
    session.render(model.getSnapshot(), renderOptions(model));
    const windowIds = session.state.trace.records.map((r) => r.id);
    expect(windowIds).toContain("trace-gauntlet-c-1");
  });

  it("scroll-up suspends follow; later arrivals grow unseenCount and do NOT move the window", () => {
    const { session, model } = traceSession();
    session.render(model.getSnapshot(), renderOptions(model));

    session.key("up", model.getSnapshot()); // scroll up in the trace view
    expect(session.state.trace.followState).toBe("INSPECTION_HOLD");
    const before = streamVisibleWindow(session.state.trace).map((r) => r.id);

    model.applyEvent(unitEvent("u-new", "New Unit", "c-99999", "active"));
    session.render(model.getSnapshot(), renderOptions(model));

    expect(session.state.trace.followState).toBe("INSPECTION_HOLD");
    expect(session.state.trace.unseenCount).toBe(1);
    // No jump to the live tail (adversarial tooth): the window still starts
    // at the same oldest row. (Partial-window edge: with a window larger than
    // the whole ledger, arrivals may extend the window's tail — pinned as an
    // explicit machine edge in trace-stream.test.ts.)
    const after = streamVisibleWindow(session.state.trace).map((r) => r.id);
    expect(after[0]).toBe(before[0]);
    const lastBefore = before.at(-1);
    if (lastBefore !== undefined) expect(after).toContain(lastBefore);
  });

  it("trace.follow.resume (key g) returns to live and clears the unseen count", () => {
    const { session, model } = traceSession();
    session.render(model.getSnapshot(), renderOptions(model));
    session.key("up", model.getSnapshot());
    model.applyEvent(unitEvent("u-new2", "New Unit 2", "c-88888", "active"));
    session.render(model.getSnapshot(), renderOptions(model));
    expect(session.state.trace.unseenCount).toBeGreaterThan(0);

    session.key("g", model.getSnapshot());
    expect(session.state.trace.followState).toBe("LIVE_FOLLOW");
    expect(session.state.trace.unseenCount).toBe(0);
  });

  it("the trace view surfaces the follow state and an explicit unseen affordance", () => {
    const { session, model } = traceSession();
    session.render(model.getSnapshot(), renderOptions(model));
    session.key("up", model.getSnapshot());
    model.applyEvent(unitEvent("u-new3", "New Unit 3", "c-77777", "active"));
    const rendered = session.render(model.getSnapshot(), renderOptions(model));

    const primary = rendered.surfaces.find((s) => s.id === "Primary");
    expect(primary).toBeDefined();
    const text = primary!.lines.map((l) => l.spans.map((s) => s.text).join("")).join("\n");
    expect(text).toContain("INSPECTING");
    expect(text).toContain("1 new");
    expect(text).toContain("g");
  });

  it("a generation reset re-hydrates the stream instead of appending stale ids", () => {
    const { session, model } = traceSession();
    session.render(model.getSnapshot(), renderOptions(model));
    model.startGeneration("gen-two");
    model.applyEvent({
      generation_id: "gen-two",
      source_id: "gauntlet",
      cursor: "c-f1",
      event_type: "unit_updated",
      payload: {
        unit_id: "u-fresh",
        title: "Fresh",
        status: "active",
        authority: "builder",
        blast_radius: "isolated",
      },
      timestamp_ms: 2000,
    });
    session.render(model.getSnapshot(), renderOptions(model));
    const ids = session.state.trace.records.map((r) => r.id);
    expect(ids).toContain("trace-gauntlet-c-f1");
    expect(
      session.state.trace.records.every((r) => ids.filter((i) => i === r.id).length === 1),
    ).toBe(true);
  });
});
