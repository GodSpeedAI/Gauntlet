/**
 * Composer surface: default composition plus temporary takeover states
 * (plan T12, REQ-TUI-004, REQ-UX-004, DESIGN.md Composer Surface).
 *
 * Core invariants:
 *   - Composer mode and destination are ALWAYS visible (REQ-UX-004).
 *   - A takeover (approval / question / recovery / kernel-input) owns the
 *     interaction TEMPORARILY. On completion the underlying default composer
 *     state (mode, destination, draft) returns unless the canonical
 *     interaction EXPLICITLY replaced it (REQ-TUI-004).
 *   - Takeovers queue by stable request identity; a request identity is
 *     admitted at most once — expired or already-answered requests cannot
 *     accept input (REQ-UX-004).
 *   - Pasted/typed text never executes or changes mode by itself; the only
 *     state transition out of composition is an explicit `submitted` result
 *     the host routes to canonical admission.
 *   - This module holds PRESENTATION state only; consequential submission is
 *     always an effect for the host's canonical path (REQ-TUI-001).
 */

import type { RenderedLine } from "./render-tree";

export type ComposerMode = "message" | "operation" | "prime" | "kernel" | "run";

/** Every composer variant names its destination (REQ-UX-004). */
export const COMPOSER_MODES: readonly ComposerMode[] = Object.freeze([
  "message",
  "operation",
  "prime",
  "kernel",
  "run",
]);

export type TakeoverKind = "approval" | "question" | "recovery" | "kernel-input";

/** One temporary takeover request. `requestId` is the stable request identity. */
export interface TakeoverRequest {
  readonly requestId: string;
  readonly kind: TakeoverKind;
  readonly prompt: string;
  readonly destination: string;
  readonly requestedAtMs: number;
  /** Wall-clock expiry; null means the request does not expire on its own. */
  readonly expiresAtMs: number | null;
}

/** Explicit canonical replacement handed back with a completed takeover. */
export interface ComposerReplacement {
  readonly mode?: ComposerMode | undefined;
  readonly destination?: string | undefined;
  readonly draft?: string | undefined;
}

/** The default-composer state set aside while a takeover owns the input. */
interface HeldComposerState {
  readonly mode: ComposerMode;
  readonly destination: string;
  readonly draft: string;
}

export interface ExpiredTakeover {
  readonly request: TakeoverRequest;
  readonly expiredAtMs: number;
}

export interface ComposerState {
  readonly mode: ComposerMode;
  readonly destination: string;
  readonly draft: string;
  readonly takeover: TakeoverRequest | null;
  readonly takeoverInput: string;
  /** Queued attention waiting for the active takeover to finish (stable identity). */
  readonly queued: readonly TakeoverRequest[];
  readonly held: HeldComposerState | null;
  /** Expired/answered requests stay inspectable and can never accept input. */
  readonly expired: readonly ExpiredTakeover[];
  readonly notice: string | null;
}

/** A completed composition the host must route to canonical admission. */
export interface ComposerSubmission {
  /** Non-null when the submission answers an active takeover request. */
  readonly requestId: string | null;
  readonly mode: ComposerMode;
  readonly destination: string;
  readonly draft: string;
}

/** Initial default composer: message mode, workbench destination, empty draft. */
export function createComposerState(): ComposerState {
  return Object.freeze({
    mode: "message",
    destination: "workbench",
    draft: "",
    takeover: null,
    takeoverInput: "",
    queued: Object.freeze([]),
    held: null,
    expired: Object.freeze([]),
    notice: null,
  });
}

/** Set the default composer draft (typing/paste never executes or changes mode). */
export function setComposerDraft(state: ComposerState, draft: string): ComposerState {
  return Object.freeze({ ...state, draft, notice: null });
}

/**
 * Cycle the default composer's mode (plan workbench-agent-interaction.md P1,
 * REQ-UX-004: mode/destination stay ALWAYS visible, and there must be an
 * operator-reachable way to change mode from inside the focused composer).
 * A no-op while a takeover owns the input (takeover mode is fixed by
 * `modeForTakeover`, not operator-cyclable). Destination is left untouched —
 * `prime`/`message` interpret it themselves (prime.launch ignores it in
 * favor of the current Focus; prime.message falls back to the last-launched
 * child when it is empty/default).
 */
export function cycleComposerMode(state: ComposerState): ComposerState {
  if (state.takeover !== null) {
    return state;
  }
  const index = COMPOSER_MODES.indexOf(state.mode);
  // Safe: `index` is always a valid COMPOSER_MODES position (state.mode is
  // typed ComposerMode), so the modulo index is always in range.
  const next = COMPOSER_MODES[(index + 1) % COMPOSER_MODES.length] as ComposerMode;
  return Object.freeze({ ...state, mode: next, notice: `composer mode: ${next}` });
}

/** Explicitly switch composer mode/destination (visible in every render). */
export function setComposerMode(
  state: ComposerState,
  mode: ComposerMode,
  destination?: string | undefined,
): ComposerState {
  return Object.freeze({
    ...state,
    mode,
    ...(destination !== undefined ? { destination } : {}),
    notice: null,
  });
}

function heldOf(state: ComposerState): HeldComposerState {
  return state.held ?? { mode: state.mode, destination: state.destination, draft: state.draft };
}

/**
 * Admit one takeover request.
 *   - a request whose identity already expired/answered is refused;
 *   - re-admitting the ACTIVE identity is idempotent;
 *   - while a takeover is active the request joins the queue (deduplicated by
 *     stable identity);
 *   - otherwise the takeover takes over and the default state is held. The
 *     admitted identity's stale queue entry (e.g. a dismissed-then-re-admitted
 *     request) is purged: an identity is admitted at most once, so it can
 *     never be promoted — and answered — a second time.
 */
export function beginTakeover(state: ComposerState, request: TakeoverRequest): ComposerState {
  if (state.expired.some((expired) => expired.request.requestId === request.requestId)) {
    return Object.freeze({
      ...state,
      notice: `refused: request '${request.requestId}' already answered or expired`,
    });
  }
  if (state.takeover?.requestId === request.requestId) {
    return state;
  }
  if (state.takeover !== null) {
    if (state.queued.some((queued) => queued.requestId === request.requestId)) {
      return Object.freeze({
        ...state,
        notice: `request '${request.requestId}' already queued`,
      });
    }
    return Object.freeze({
      ...state,
      queued: Object.freeze([...state.queued, Object.freeze(request)]),
      notice: `queued attention: '${request.requestId}' (${request.kind})`,
    });
  }
  return Object.freeze({
    ...state,
    // Purge any stale queue entry for the admitted identity (peer MAJOR,
    // correction round 1): a dismissed-then-re-admitted request must not
    // linger in the queue and later demand a second answer.
    queued: Object.freeze(state.queued.filter((queued) => queued.requestId !== request.requestId)),
    takeover: Object.freeze(request),
    takeoverInput: "",
    held: heldOf(state),
    notice: `takeover '${request.requestId}' (${request.kind}) owns the input`,
  });
}

function promoteNext(state: ComposerState, base: HeldComposerState): ComposerState {
  const [next, ...rest] = state.queued;
  if (next !== undefined) {
    return Object.freeze({
      ...state,
      mode: base.mode,
      destination: base.destination,
      draft: base.draft,
      takeover: next,
      takeoverInput: "",
      held: base,
      queued: Object.freeze(rest),
      notice: `takeover '${next.requestId}' (${next.kind}) owns the input`,
    });
  }
  return Object.freeze({
    ...state,
    mode: base.mode,
    destination: base.destination,
    draft: base.draft,
    takeover: null,
    takeoverInput: "",
    held: null,
    queued: Object.freeze([]),
    notice: `composer restored`,
  });
}

/**
 * Complete the ACTIVE takeover. Without `canonical` the held default composer
 * state returns unchanged (REQ-TUI-004); with `canonical` the canonical
 * interaction explicitly replaced mode/destination/draft. An unknown or
 * already-completed request identity changes nothing.
 */
export function completeTakeover(
  state: ComposerState,
  requestId: string,
  canonical?: ComposerReplacement | undefined,
): ComposerState {
  if (state.takeover === null || state.takeover.requestId !== requestId) {
    return Object.freeze({
      ...state,
      notice: `no active takeover '${requestId}'`,
    });
  }
  const held = heldOf(state);
  const base: HeldComposerState = canonical
    ? {
        mode: canonical.mode ?? held.mode,
        destination: canonical.destination ?? held.destination,
        draft: canonical.draft ?? held.draft,
      }
    : held;
  return promoteNext({ ...state }, base);
}

/**
 * Escape dismisses the takeover UI (REQ-UX-004): the default composer state
 * returns and the request RETURNS TO THE QUEUE — the attention stays pending;
 * it was dismissed, not answered.
 */
export function dismissTakeover(state: ComposerState): ComposerState {
  if (state.takeover === null) {
    return state;
  }
  const dismissed = state.takeover;
  const held = heldOf(state);
  return Object.freeze({
    ...state,
    mode: held.mode,
    destination: held.destination,
    draft: held.draft,
    takeover: null,
    takeoverInput: "",
    held: null,
    queued: Object.freeze([dismissed, ...state.queued]),
    notice: `takeover '${dismissed.requestId}' dismissed; request remains queued`,
  });
}

/**
 * Expire stale requests by wall-clock: expired requests cannot accept input;
 * an expired ACTIVE takeover returns the held composer state; remaining
 * expired queue entries are dropped from the queue. Expired identities stay
 * inspectable in `expired`.
 */
export function expireTakeovers(state: ComposerState, nowMs: number): ComposerState {
  const isStale = (request: TakeoverRequest): boolean =>
    request.expiresAtMs !== null && request.expiresAtMs < nowMs;

  const expiredIds: string[] = [];
  const expiredList: ExpiredTakeover[] = [...state.expired];

  // Expire queue entries first.
  const freshQueued: TakeoverRequest[] = [];
  for (const queued of state.queued) {
    if (isStale(queued)) {
      expiredIds.push(queued.requestId);
      expiredList.push({ request: queued, expiredAtMs: nowMs });
    } else {
      freshQueued.push(queued);
    }
  }

  let next: ComposerState = Object.freeze({ ...state, queued: Object.freeze(freshQueued) });

  // Expire the active takeover if stale: restore held state, then promote.
  if (next.takeover !== null && isStale(next.takeover)) {
    expiredIds.push(next.takeover.requestId);
    expiredList.push({ request: next.takeover, expiredAtMs: nowMs });
    const held = heldOf(next);
    next = Object.freeze({
      ...next,
      mode: held.mode,
      destination: held.destination,
      draft: held.draft,
      takeover: null,
      takeoverInput: "",
      held: null,
    });
    next = promoteNext(next, held);
  }

  if (expiredIds.length === 0) {
    return state;
  }
  return Object.freeze({
    ...next,
    expired: Object.freeze(expiredList),
    notice: `expired (cannot accept input): ${expiredIds.join(", ")}`,
  });
}

const graphemes = new Intl.Segmenter(undefined, { granularity: "grapheme" });

function isPrintable(key: string): boolean {
  const first = graphemes.segment(key)[Symbol.iterator]().next().value;
  return first?.segment === key && !/[\p{Cc}\p{Cs}]/u.test(key);
}

function eraseLastGrapheme(text: string): string {
  const last = graphemes.segment(text).containing(Math.max(0, text.length - 1));
  return text.slice(0, last?.index ?? 0);
}

function modeForTakeover(kind: TakeoverKind): ComposerMode {
  switch (kind) {
    case "kernel-input":
      return "kernel";
    case "question":
      return "message";
    case "approval":
    case "recovery":
      return "operation";
  }
}

/**
 * Route one keypress into the composer. Typing appends text; backspace
 * deletes; enter PRODUCES a submission for the host's canonical path and
 * never executes anything itself. With an active takeover the input owns the
 * interaction; completing it restores the held composer state.
 */
export function composeKeypress(
  state: ComposerState,
  key: string,
): { state: ComposerState; submitted: ComposerSubmission | null } {
  if (state.takeover !== null) {
    const takeover = state.takeover;
    if (isPrintable(key)) {
      return {
        state: Object.freeze({ ...state, takeoverInput: state.takeoverInput + key }),
        submitted: null,
      };
    }
    if (key === "backspace") {
      return {
        state: Object.freeze({
          ...state,
          takeoverInput: eraseLastGrapheme(state.takeoverInput),
        }),
        submitted: null,
      };
    }
    if (key === "enter") {
      const submitted: ComposerSubmission = Object.freeze({
        requestId: takeover.requestId,
        mode: modeForTakeover(takeover.kind),
        destination: takeover.destination,
        draft: state.takeoverInput,
      });
      return { state: completeTakeover(state, takeover.requestId), submitted };
    }
    return { state, submitted: null };
  }

  if (isPrintable(key)) {
    return {
      state: Object.freeze({ ...state, draft: state.draft + key }),
      submitted: null,
    };
  }
  if (key === "backspace") {
    return {
      state: Object.freeze({ ...state, draft: eraseLastGrapheme(state.draft) }),
      submitted: null,
    };
  }
  if (key === "enter") {
    const submitted: ComposerSubmission = Object.freeze({
      requestId: null,
      mode: state.mode,
      destination: state.destination,
      draft: state.draft,
    });
    return {
      state: Object.freeze({
        ...state,
        draft: "",
        notice: `submitted ${state.mode} → ${state.destination}`,
      }),
      submitted,
    };
  }
  return { state, submitted: null };
}

export interface RenderedComposer {
  readonly mode: ComposerMode;
  readonly destination: string;
  readonly lines: readonly RenderedLine[];
  readonly activeTakeover: TakeoverRequest | null;
  readonly queuedCount: number;
}

/** Render the composer surface. Mode + destination are ALWAYS visible. */
export function renderComposer(state: ComposerState): RenderedComposer {
  const lines: RenderedLine[] = [];

  // Mode and destination line — unconditional (REQ-UX-004).
  lines.push({
    id: "composer.mode",
    spans: [
      { text: "COMPOSER ", token: "structure.neutral" },
      { text: `[${state.mode}]`, emphasis: "bold" },
      { text: ` → ${state.destination}`, token: "focus.primary" },
      ...(state.takeover !== null
        ? [{ text: `  (takeover active)`, token: "attention.required" as const }]
        : []),
    ],
  });

  if (state.takeover !== null) {
    const takeover = state.takeover;
    lines.push({
      id: "composer.takeover",
      spans: [
        { text: `TAKEOVER ${takeover.kind} `, token: "attention.required", emphasis: "bold" },
        { text: `(${takeover.requestId})`, emphasis: "dim" },
      ],
    });
    lines.push({ id: "composer.takeover.prompt", spans: [{ text: takeover.prompt }] });
    lines.push({
      id: "composer.takeover.input",
      spans: [{ text: `> ${state.takeoverInput}`, token: "focus.ring" }],
    });
    lines.push({
      id: "composer.takeover.hint",
      spans: [{ text: "enter submits · escape dismisses (request stays queued)", emphasis: "dim" }],
    });
  } else {
    lines.push({
      id: "composer.draft",
      spans: [{ text: `> ${state.draft}`, token: "focus.ring" }],
    });
    lines.push({
      id: "composer.hint",
      spans: [
        { text: "message text does not execute or change mode · enter submits", emphasis: "dim" },
      ],
    });
  }

  if (state.queued.length > 0) {
    lines.push({
      id: "composer.queued",
      spans: [
        { text: `queued attention: ${state.queued.length} `, token: "attention.required" },
        { text: `[${state.queued.map((q) => q.requestId).join(", ")}]`, emphasis: "dim" },
      ],
    });
  }
  if (state.expired.length > 0) {
    lines.push({
      id: "composer.expired",
      spans: [
        {
          text: `expired, cannot accept input: ${state.expired.map((e) => e.request.requestId).join(", ")}`,
          token: "structure.neutral",
          emphasis: "dim",
        },
      ],
    });
  }
  if (state.notice !== null) {
    lines.push({ id: "composer.notice", spans: [{ text: state.notice, emphasis: "dim" }] });
  }

  return Object.freeze({
    mode: state.mode,
    destination: state.destination,
    lines: Object.freeze(lines),
    activeTakeover: state.takeover,
    queuedCount: state.queued.length,
  });
}
