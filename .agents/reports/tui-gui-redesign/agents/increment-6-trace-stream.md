# Increment 6 (core) — TraceStream presentation state machine

Status: COMPLETE (bounded slice). Files owned and delivered:
`workbench/src/trace-stream.ts`, `workbench/src/trace-stream.test.ts`. Nothing else touched.

## What this is

The pure, frozen-data follow/hold/unseen/load-older machine the redesigned Trace
view (UESC §16.2 `TraceStream`, ICM §16.4–16.6) will consume. It does not
render, does not import anything, and never touches the model, the terminal,
or the RealityTrace adapter. Today's Trace view is a flat unbounded dump
(`views-chat-trace.ts:116-159`); this module is the presentation core that
replaces that behavior at the view layer in a later wiring increment.

Adversarial tooth pinned: "Trace does not jump to live tail while the user
inspects history."

## API summary

Types: `TraceFollowState = "COLD" | "HYDRATING" | "LIVE_FOLLOW" | "INSPECTION_HOLD"`;
`TraceSource = "gauntlet" | "realitytrace"` (two labelled sources, never merged);
`TraceStreamRecord = { id, source, ordinal?, timestampMs? }` (minimal shape; the
view maps real `trace_records` / SXR ledger rows onto it);
`TraceStream = { followState, unseenCount, windowSize, anchorTopId, selectedId, records }`.

All functions are pure and return frozen results (state, records array, and each
record copy are `Object.freeze`d; caller input arrays/records are copied on
admission, never aliased). Documented no-ops return the same reference so tests
can pin them with `toBe`:

- `createTraceStream(windowSize)` — COLD, empty. Throws `RangeError` on
  non-integer or `< 1` window sizes.
- `streamHydrated(s, records)` — explicit hydration/re-hydration: replaces the
  baseline, → `LIVE_FOLLOW`, unseen reset, anchor cleared, selection survives
  only if its id exists in the new baseline. The ONLY exit from COLD/HYDRATING.
- `streamAppend(s, records)` — live: window stays on tail, unseen stays 0;
  hold: visible window unchanged, `unseenCount += records.length`, selection
  preserved even outside the window; COLD/HYDRATING: identity no-op.
- `streamScrollUp(s)` — suspends follow, moves the window up one row (clamped
  at head), anchors `anchorTopId` to the resulting top row.
- `streamScrollDown(s)` — hold only: window moves one row toward the tail;
  reaching the tail → `LIVE_FOLLOW`, unseen 0, anchor cleared.
- `streamReturnToLive(s)` — explicit resume (ICM §16.5 `↓ N new` / End /
  palette): window jumps to tail, live, unseen 0, selection kept by identity.
- `streamSelect(s, id)` — identity-based selection; never moves the window or
  the follow state; unknown id deselects (null).
- `streamLoadOlder(s, older)` — prepends history; anchor row stays the visible
  window top (anchor stability); follow state and unseen unchanged.
- `streamVisibleWindow(s)` — derived rows: tail in live, anchored range in
  hold, empty in COLD/HYDRATING. Frozen.

## Invariant test list (46 tests, 151 assertions, all green)

Written RED first (module absent), then implemented:

- (a) append during INSPECTION_HOLD never changes the visible window and never
  steals attention — only `unseenCount` grows (multiple appends, both sources).
- (b) `unseenCount` exact across batches, empty batches, and resets (return to
  live, scroll-to-tail, re-hydration).
- (c) selection survives appends and window moves — identity, not viewport
  (selected row leaves the window in both hold and live-tail turnover).
- (d) `loadOlder` preserves the anchor row as the visible window top during
  hold; during live the tail window is unchanged (no yank from prepends).
- (e) reaching the tail via `scrollDown` returns to `LIVE_FOLLOW` with unseen
  reset and anchor cleared; short-stream holds resume immediately.
- (f) sources never merge: same id from `gauntlet` and `realitytrace` are two
  records, both survive, sources/order preserved; ids are source-qualified by
  the caller (documented in module + tests).
- (g) COLD/HYDRATING transitions are explicit only: every non-hydration
  operation is an identity no-op in both states; nothing in the API produces
  COLD/HYDRATING except construction; `streamHydrated` is the only exit.
- Purity/freezing: frozen state/records/window, input mutation after admission
  has no effect, input state never mutated, malformed records rejected with
  `TypeError` (untrusted-input admission), `RangeError` on bad window sizes.

## Semantic decisions (flag for orchestrator/wiring review)

1. `streamScrollUp` moves the window up one row in addition to suspending
   follow. The task text says only "sets anchorTopId to the current top row";
   a literal no-movement reading leaves NO API able to move the window upward,
   making INSPECTION_HOLD unable to display history (incomplete machine). The
   chosen semantics anchor the top row of the resulting window. If the intent
   was freeze-at-current-top, the diff is one line in `streamScrollUp`.
2. Scroll granularity is one row per call; the view maps its scroll physics
   onto repeated calls.
3. Partial-window edge: while held on a stream shorter than `windowSize`, new
   arrivals can extend the window downward inside the fixed frame (anchor and
   all previously visible rows keep their positions). The frame never moves
   toward live. Full windows are byte-identical across appends.
4. `unseenCount` is a single total across both sources (state shape is frozen
   by contract); a per-source `G: n / R: m` breakdown is view-derived.
5. `HYDRATING` is in the type union and fully guarded, but no API enters it —
   the reconnect machine (UESC §16.2, frames from older generations rejected)
   is a future slice; its wiring will construct it (tests hand-build it today
   to pin the guards).
6. `streamSelect` on an id not present deselects; there is no separate deselect
   API (smallest complete API).

## Integration notes for the Trace view

- Map Gauntlet `trace_records` rows → `{ id: trace_id, source: "gauntlet",
  timestampMs }` and RealityTrace lens rows → `{ id: ledger-qualified id,
  source: "realitytrace", ordinal }`. Ids MUST be source-qualified by the
  caller (e.g. the existing `sxr:` prefix discipline) — the module matches
  anchor/selection by bare id and never dedupes.
- Arrival order is the cross-source fold (per-source order stays the caller's
  authority; interleave by time only when both rows carry timestamps —
  UESC/ICM §16.0). `streamAppend` batches preserve given order.
- Wire view scroll events: meaningful upward scroll → `streamScrollUp`
  (possibly repeated for granularity), downward → `streamScrollDown`;
  `↓ N new` badge reads `unseenCount`; End/live shortcut and the badge →
  `streamReturnToLive`; head paging → `streamLoadOlder` (QUERY_ONLY, per
  source, ICM §16.6); render exactly `streamVisibleWindow(s)`.
- Commands `trace.follow.suspend` / `trace.follow.resume` /
  `trace.history.load_older` (ICM §16.4–16.6) map 1:1 onto these functions;
  `trace.event.select` (§16.1) maps onto `streamSelect`.
- Selection must drive the Inspector by (source, id) against the record found
  in `records`; `selectedId` alone is ambiguous only if the caller violates
  the source-qualification rule.

## Verification evidence

- `bun test src/trace-stream.test.ts` → 46 pass / 0 fail, 151 expect() calls.
- `bun x biome check --write src/trace-stream.ts src/trace-stream.test.ts` → clean.
- `bun run check` (tsc) → zero errors in the two delivered files. The command
  exits 1 with 35 errors, ALL in sibling in-flight increment files owned by
  other agents (`theme-capability.test.ts`, `theme.ts`, `wire-shape.test.ts`,
  `shell.ts` area — increments 2–5 currently uncommitted in the worktree).
  Not caused by and not fixable from this slice; flagged to the orchestrator.
- Full suite intentionally not run (bounded-slice discipline); no commits made.

## Next move (for the wiring increment)

Trace view consumes `streamVisibleWindow` + a per-source unseen breakdown;
reconnect slice adds the HYDRATING entry path with generation-guarded
re-hydration (reject older-generation frames per UESC §16.2).
