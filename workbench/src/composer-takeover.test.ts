import { describe, expect, it } from "bun:test";
import type { TakeoverRequest } from "./composer";
import {
  beginTakeover,
  completeTakeover,
  composeKeypress,
  createComposerState,
  dismissTakeover,
  expireTakeovers,
  renderComposer,
  setComposerDraft,
  setComposerMode,
} from "./composer";
import { FULL_DIMS, modelWiredSession, seededModel } from "./testing/shell-harness";

/**
 * T12 composer-takeover suites (REQ-TUI-004, REQ-UX-004, prereg P2): a
 * temporary takeover owns the interaction and then the original composer
 * state returns UNLESS canonical state explicitly replaced it; takeovers
 * queue by stable request identity; expired/answered requests cannot accept
 * input; mode+destination are always visible.
 */

const NOW = 1_000_000;

function kernelTakeover(overrides: Partial<TakeoverRequest> = {}): TakeoverRequest {
  return {
    requestId: "req-kernel-1",
    kind: "kernel-input",
    prompt: "prime kernel requests input",
    destination: "prime-kernel",
    requestedAtMs: NOW,
    expiresAtMs: null,
    ...overrides,
  };
}

function draftComposer(): ReturnType<typeof createComposerState> {
  return setComposerDraft(createComposerState(), "refine the retry path");
}

describe("T12 composer takeover restores the underlying state (REQ-TUI-004, prereg P2)", () => {
  it("kernel-input takeover with draft text present: draft and mode return after completion", () => {
    let state = draftComposer();
    expect(state.mode).toBe("message");
    expect(state.draft).toBe("refine the retry path");

    // The takeover takes over.
    state = beginTakeover(state, kernelTakeover());
    expect(state.takeover?.requestId).toBe("req-kernel-1");
    expect(state.held?.draft).toBe("refine the retry path");

    // The takeover owns the input; typed text goes to the takeover buffer.
    for (const key of "print(state)".split("")) {
      state = composeKeypress(state, key).state;
    }
    expect(state.takeoverInput).toBe("print(state)");
    // The underlying draft is untouched while the takeover owns the input.
    expect(state.held?.draft).toBe("refine the retry path");

    // Enter submits the takeover answer to the host's canonical path.
    const { state: afterSubmit, submitted } = composeKeypress(state, "enter");
    expect(submitted).not.toBeNull();
    expect(submitted?.requestId).toBe("req-kernel-1");
    expect(submitted?.mode).toBe("kernel");
    expect(submitted?.destination).toBe("prime-kernel");
    expect(submitted?.draft).toBe("print(state)");

    // Completion without canonical replacement restores the held state.
    expect(afterSubmit.takeover).toBeNull();
    expect(afterSubmit.draft).toBe("refine the retry path");
    expect(afterSubmit.mode).toBe("message");
    expect(afterSubmit.destination).toBe("workbench");
  });

  it("canonical state explicitly replacing the takeover wins over the held draft", () => {
    let state = draftComposer();
    state = beginTakeover(state, kernelTakeover({ requestId: "req-recovery", kind: "recovery" }));
    state = completeTakeover(state, "req-recovery", {
      mode: "operation",
      draft: "resolve u-far",
    });
    expect(state.mode).toBe("operation");
    expect(state.draft).toBe("resolve u-far");
  });

  it("re-admitting the active identity is idempotent; completing an unknown identity is a no-op", () => {
    let state = draftComposer();
    state = beginTakeover(state, kernelTakeover());
    const frozen = state;
    state = beginTakeover(state, kernelTakeover());
    expect(state).toBe(frozen);

    const unchanged = completeTakeover(state, "req-unknown");
    expect(unchanged.takeover?.requestId).toBe("req-kernel-1");
    expect(unchanged.notice).toContain("no active takeover");
  });
});

describe("T12 takeover queueing by stable request identity (REQ-UX-004)", () => {
  it("queued attention activates in order; the original draft survives the whole chain", () => {
    let state = draftComposer();
    state = beginTakeover(state, kernelTakeover({ requestId: "req-a" }));
    state = beginTakeover(
      state,
      kernelTakeover({ requestId: "req-b", kind: "approval", prompt: "approve resolve" }),
    );
    state = beginTakeover(
      state,
      kernelTakeover({ requestId: "req-c", kind: "question", prompt: "clarify scope" }),
    );
    expect(state.takeover?.requestId).toBe("req-a");
    expect(state.queued.map((queued) => queued.requestId)).toEqual(["req-b", "req-c"]);

    // Duplicate queue entries by the same identity are refused.
    const deduped = beginTakeover(state, kernelTakeover({ requestId: "req-b" }));
    expect(deduped.queued.length).toBe(2);
    expect(deduped.notice).toContain("already queued");

    state = completeTakeover(state, "req-a");
    expect(state.takeover?.requestId).toBe("req-b");
    state = completeTakeover(state, "req-b");
    expect(state.takeover?.requestId).toBe("req-c");
    state = completeTakeover(state, "req-c");

    expect(state.takeover).toBeNull();
    expect(state.draft).toBe("refine the retry path");
    expect(state.mode).toBe("message");
  });
});

describe("T12 expired takeovers cannot accept input (REQ-UX-004)", () => {
  it("expiry restores the composer, keeps the request inspectable, and refuses re-admission", () => {
    let state = draftComposer();
    state = beginTakeover(
      state,
      kernelTakeover({ requestId: "req-stale", expiresAtMs: NOW + 100 }),
    );
    expect(state.takeover?.requestId).toBe("req-stale");

    state = expireTakeovers(state, NOW + 200);
    expect(state.takeover).toBeNull();
    expect(state.draft).toBe("refine the retry path");
    expect(state.expired.map((expired) => expired.request.requestId)).toEqual(["req-stale"]);
    expect(state.notice).toContain("expired");

    // An expired/already-answered request cannot accept input again.
    const refused = beginTakeover(state, kernelTakeover({ requestId: "req-stale" }));
    expect(refused.takeover).toBeNull();
    expect(refused.notice).toContain("already answered or expired");
  });

  it("expires stale queue entries too; fresh queue entries still activate", () => {
    let state = draftComposer();
    state = beginTakeover(state, kernelTakeover({ requestId: "req-a", expiresAtMs: NOW + 100 }));
    state = beginTakeover(state, kernelTakeover({ requestId: "req-b" }));
    state = expireTakeovers(state, NOW + 200);

    expect(state.takeover?.requestId).toBe("req-b");
    expect(state.expired.map((expired) => expired.request.requestId)).toEqual(["req-a"]);
    state = completeTakeover(state, "req-b");
    expect(state.draft).toBe("refine the retry path");
  });
});

describe("T12 escape dismisses the takeover UI without answering it (REQ-UX-004)", () => {
  it("dismissal returns the draft and keeps the request queued for later attention", () => {
    let state = draftComposer();
    state = beginTakeover(state, kernelTakeover({ requestId: "req-dismiss" }));
    state = dismissTakeover(state);
    expect(state.takeover).toBeNull();
    expect(state.draft).toBe("refine the retry path");
    expect(state.queued.map((queued) => queued.requestId)).toEqual(["req-dismiss"]);

    // A dismissed (not answered) request can be re-admitted.
    state = beginTakeover(state, kernelTakeover({ requestId: "req-dismiss" }));
    expect(state.takeover?.requestId).toBe("req-dismiss");
  });

  it("REGRESSION (peer MAJOR): re-admitting a dismissed request removes its stale queue entry — it cannot be answered twice", () => {
    // Peer repro ordering: begin(a) → dismiss(a) → begin(a) → begin(b) → complete(a).
    let state = draftComposer();

    state = beginTakeover(state, kernelTakeover({ requestId: "req-a" }));
    expect(state.takeover?.requestId).toBe("req-a");

    state = dismissTakeover(state);
    expect(state.takeover).toBeNull();
    expect(state.queued.map((queued) => queued.requestId)).toEqual(["req-a"]);

    // Re-admit the dismissed identity: it becomes ACTIVE again. Its stale
    // queue entry must be purged — an identity is admitted at most once.
    state = beginTakeover(state, kernelTakeover({ requestId: "req-a" }));
    expect(state.takeover?.requestId).toBe("req-a");
    expect(state.queued.map((queued) => queued.requestId)).not.toContain("req-a");
    expect(state.queued.map((queued) => queued.requestId)).toEqual([]);

    // A fresh request queues behind the re-admitted one.
    state = beginTakeover(
      state,
      kernelTakeover({ requestId: "req-b", kind: "approval", prompt: "approve resolve" }),
    );
    expect(state.queued.map((queued) => queued.requestId)).toEqual(["req-b"]);

    // Completing the re-admitted request must NOT re-activate a itself.
    state = completeTakeover(state, "req-a");
    expect(state.takeover?.requestId).toBe("req-b");

    // The chain ends with the composer restored — each identity answered once.
    state = completeTakeover(state, "req-b");
    expect(state.takeover).toBeNull();
    expect(state.draft).toBe("refine the retry path");
    expect(state.mode).toBe("message");
  });
});

describe("T12 composer rendering (REQ-UX-004)", () => {
  it("mode and destination are ALWAYS visible; takeover, queue and expiry are named", () => {
    let state = setComposerMode(draftComposer(), "prime", "prime-agent");
    let rendered = renderComposer(state);
    const modeLine = rendered.lines.find((line) => line.id === "composer.mode");
    expect(modeLine?.spans.map((span) => span.text).join("")).toContain("[prime]");
    expect(modeLine?.spans.map((span) => span.text).join("")).toContain("prime-agent");

    state = beginTakeover(state, kernelTakeover({ requestId: "req-k", expiresAtMs: NOW + 100 }));
    state = beginTakeover(state, kernelTakeover({ requestId: "req-q" }));
    rendered = renderComposer(state);
    const text = rendered.lines
      .map((line) => line.spans.map((span) => span.text).join(""))
      .join("\n");
    expect(text).toContain("TAKEOVER kernel-input");
    expect(text).toContain("(req-k)");
    expect(text).toContain("queued attention: 1");
    expect(text).toContain("[prime]"); // still visible under takeover

    state = expireTakeovers(state, NOW + 10_000);
    rendered = renderComposer(state);
    const expiredText = rendered.lines
      .map((line) => line.spans.map((span) => span.text).join(""))
      .join("\n");
    expect(expiredText).toContain("expired, cannot accept input: req-k");
  });

  it("pasted text never executes or changes mode by itself", () => {
    const pasted = "look like a kernel command: import os\nrm -rf /";
    const state = setComposerDraft(createComposerState(), pasted);
    expect(state.mode).toBe("message");
    expect(state.notice).toBeNull();
    const rendered = renderComposer(state);
    expect(rendered.mode).toBe("message");
    const text = rendered.lines
      .map((line) => line.spans.map((span) => span.text).join(""))
      .join("\n");
    expect(text).toContain(pasted); // rendered as inert text
  });
});

describe("T12 session-level takeover owns the interaction (REQ-TUI-004)", () => {
  it("keys route to the takeover, then the composer state returns and focus follows", () => {
    const model = seededModel();
    const session = modelWiredSession(model, FULL_DIMS);
    const snapshot = model.getSnapshot();

    // Draft in the default composer.
    session.key("e", snapshot);
    expect(session.state.focusedSurface).toBe("Composer");
    for (const key of "draft text".split("")) {
      session.key(key, model.getSnapshot());
    }
    expect(session.composerState.draft).toBe("draft text");

    // Host admits a kernel-input takeover: it owns the interaction.
    session.beginTakeover(kernelTakeover({ requestId: "req-session" }));
    expect(session.state.focusedSurface).toBe("Composer");
    expect(session.composerState.takeover?.requestId).toBe("req-session");

    for (const key of "1+1".split("")) {
      session.key(key, model.getSnapshot());
    }
    expect(session.composerState.takeoverInput).toBe("1+1");
    // Tab does not steal focus from an owning takeover.
    session.key("tab", model.getSnapshot());
    expect(session.state.focusedSurface).toBe("Composer");

    // Enter submits and completes: draft returns, focus returns to Primary.
    session.key("enter", model.getSnapshot());
    expect(session.composerState.takeover).toBeNull();
    expect(session.composerState.draft).toBe("draft text");
    expect(session.state.focusedSurface).toBe("Primary");

    // Escape dismisses a takeover back into the queue with the draft intact.
    session.beginTakeover(kernelTakeover({ requestId: "req-esc", kind: "approval" }));
    session.key("escape", model.getSnapshot());
    expect(session.composerState.takeover).toBeNull();
    expect(session.composerState.draft).toBe("draft text");
    expect(session.composerState.queued.map((queued) => queued.requestId)).toEqual(["req-esc"]);
    expect(session.state.focusedSurface).toBe("Primary");
  });
});
