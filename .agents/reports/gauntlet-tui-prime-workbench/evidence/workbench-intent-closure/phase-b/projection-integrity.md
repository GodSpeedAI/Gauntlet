# Phase B — bind the projection integrity contract in production

Plan: `.agents/plans/workbench-intent-closure.md` §4 Phase B.
Date: 2026-09-10. Prior phase: Phase A (SSH leg repair; addendum in
`evidence/T20/release-qualification.md`).

## What changed

`workbench-host.ts` no longer hand-rolls a `membership`/`previous`/`poll-N` string diff.
Production read projection is now `CanonicalSurfaceStreamPort` (new,
`workbench/src/canonical-surface-stream-port.ts`) driving `LiveStreamClient` (T16,
`live-streams.ts`) over `WorkbenchModel` — the same generation/cursor/stale-frame
reconciliation machinery the T16 suites already proved against `InMemoryStreamPort`, now
bound to the real §20.2 HTTP routes (`GauntletReadClient`) instead of an in-process fake.

`SourceDescriptor` gained a `granularity: "event_stream" | "snapshot_interval"` field
(threaded into `SourceReconciliation` so it is visible on every connect handshake, not
just buried in the port). `InMemoryStreamSource` declares `"event_stream"` (it retains
every event individually — the durable-log reference fake T16's suites are written
against). `CanonicalSurfaceStreamPort` declares `"snapshot_interval"` on every descriptor
and window it serves: this surface is periodic polling, not a durable per-event log, and
says so in its own types rather than wearing a durable stream's costume (D1).

## The design decision (D1): every declaration is derived from an observed fact

- **source identity** — the configured endpoint's run id from `GET /api/v1/state`.
- **store epoch** — `JSON.stringify([protocol_version, server_version, run,
  sorted_unit_ids])` from `GET /api/v1/capabilities` + `GET /api/v1/state`. A server
  restart, a run change, OR a unit-membership change (unit added or removed) replaces the
  epoch — reproducing exactly what the deleted `membership !== this.membership` string
  diff did, now expressed in the port's own typed contract. A content-only change (a
  unit's status/title/evidence changing, membership unchanged) does NOT replace the
  epoch — it is an ordinary incremental poll.
- **cursor semantics** — delegated to `InMemoryStreamSource`'s opaque per-source sequence
  (D2: reuse the retention/epoch/cursor bookkeeping T16 already proved, rather than
  reimplementing it). `CanonicalSurfaceStreamPort` owns one `InMemoryStreamSource` as an
  internal buffer and repopulates it from real HTTP responses on `observe()`.
- **retention** — connect-scoped: the buffer only ever holds what has been observed since
  construction or since the last epoch replacement.
- **granularity** — `"snapshot_interval"`, disclosed on every descriptor/window/
  reconciliation record (see above).

The ONLY async step is `CanonicalSurfaceStreamPort.observe()` — one real round trip
(`GET /api/v1/state`, `GET /api/v1/capabilities`, `GET /api/v1/units/{id}/inspection` per
unit) that diffs against the port's own last observation and folds any difference into the
internal buffer. `describeSources`/`baselineWindow`/`incrementsSince` stay fully
synchronous, serving strictly from that buffer — `LiveStreamClient` needed zero changes to
consume a real surface instead of `InMemoryStreamPort`.

`WorkbenchHost.read()` now: calls `port.observe()` + `route.observe()` in parallel, calls
`stream.connect()` on the first successful observation or `stream.poll()` thereafter, and
clears operator Focus only when `poll()` reports a resync (or on first connect) AND the
focused unit's source has genuinely disappeared or the run changed — the same rule the
deleted diff enforced, now driven off the stream's own resync signal instead of a string
comparison.

## Hard containment line (unchanged, verified)

`OperationGateway`'s pre-admission observation (`operation-gateway.ts:331`,
`route.observe(...)`) was not touched. It remains a wholly separate HTTP round trip from
`port`/`stream`; `operation-gateway.ts` imports neither `live-streams.ts` nor
`canonical-surface-stream-port.ts` (asserted in
`operation-isomorphism.test.ts`, new describe block "Phase B — hard containment").

## The RED round (preserved, D4)

Tooth 1 (headless-replay equivalence) was written against a hand-constructed reference
fold (independently retyped `unit_updated` payloads with invented cursors `ref-1`/`ref-2`)
and FAILED on first run:

```
Expected: "5e12fece5eb2023106663089e295f369c86baa75f0a0239991cc5eedd0ab09f9"
Received: "6b96238673567c83ae3a5a9cc0c7b0a9c0c8b625f9895f3173301c9b4c4378f3"
```

Root cause: `normalizeLiveFold`'s canonical sections include per-source cursor state, and
the hand-invented `ref-1`/`ref-2` cursors legitimately differed from the port's real
minted cursors — not a defect in the port. Fixed by folding the reference model directly
from the port's own observed `StreamEvent`s (`port.describeSources().flatMap(d =>
port.baselineWindow(d.source_id).events)`, `canonicalStreamOrder`-sorted) — the same
`referenceModel()` idiom `detach-reattach.test.ts`/`reconnect-races.test.ts` already use
against `InMemoryStreamPort`, here run against the real derived port. Re-run: GREEN, with
a negative control (one unit's status tampered before folding) proving the digest
comparison is load-bearing, not vacuously equal.

A second RED round: the same file's first "detach/reattach against the derived port" tooth
initially asserted `cursor_validation: "valid"` after a unit was ADDED while detached —
wrong, because adding a unit is a membership change and correctly forces `epoch_changed`
under this design. Fixed by testing the "valid" case with a content-only change instead,
and adding a second, explicit test for the membership-change case (asserting
`epoch_changed`/`fresh_baseline`, which passed as designed).

## Teeth (plan §4 Phase B, all landed and green)

1. **Headless-replay equivalence** — `headless-replay.test.ts`, "Phase B — headless-replay
   equivalence" describe block. Byte-identical `projectionDigest(canonicalSections(...))`
   between the `CanonicalSurfaceStreamPort`+`LiveStreamClient` path and a direct fold of
   the port's own events, plus a tampered negative control.
2. **D1 enforcement (retention/epoch honesty)** — `live-streams.test.ts` (new file), six
   tests: granularity disclosure, retention-floor honesty, run-change epoch replacement,
   membership-change epoch replacement, no-op-observe idempotence, honest unreachable
   reporting.
3. **Pre-admission never served from the stream projection** — `operation-isomorphism.test.ts`,
   "Phase B — hard containment" describe block: a structural import check plus a
   behavioral proof (a governance denial that appears on the surface AFTER the last
   `WorkbenchHost.refresh()` still refuses the very next `invoke()` with no intervening
   refresh, proving admission re-fetches rather than reading the port's stale buffer).
4. **Detach/reattach against the derived port** — `detach-reattach.test.ts`, "Phase B —
   detach/reattach against the DERIVED port" describe block: a persisted cursor is honoured
   across a real content-only change (`cursor_validation: "valid"`, incremental catch-up),
   and a membership change while detached correctly forces `epoch_changed`/`fresh_baseline`
   on reattach.
5. **Mid-poll backend restart** — `reconnect-races.test.ts`, "Phase B — mid-poll backend
   restart against the DERIVED port" describe block: a real `server_version` change
   produces a recorded `resyncing` transition (`client.stateLog`), a fresh generation
   (`generationId` changes), and `resyncCount === 1` — never a silent cursor advance. An
   ordinary content-only poll in the same test is confirmed NOT to trigger a resync.
6. **Focus-survival regression** — `focus-identity.test.ts`, "Phase B — focus survives
   LiveStreamClient reconciliation" describe block: `connect()`/`poll()` never touch
   `currentFocus` themselves (only `WorkbenchHost` decides to clear it); end-to-end proof
   that Focus persists across an ordinary poll and clears on unit disappearance already
   existed in `workbench-host.test.ts` (unchanged, still green against the new host).

## Gate results

| Gate | Result |
|---|---|
| `bun test` (full, unfiltered) | 624 pass / 0 fail / 13 skip (was 611/0/13 before Phase B; +13 new tests) |
| Phase B file group (13 files) | 84 pass / 0 fail |
| `just tui-test all` (49 registered suites) | 537 pass / 0 fail / 10 skip (was 530/0/10 after Phase A; +7 within existing registered suites — no suite count change, Phase B binds existing declared gates, it does not add new T-numbered tasks) |
| `just tui-check` | clean |
| `bun run lint` (`biome check --write .` then re-run) | exit 0; only pre-existing `bun-substrate.bench.ts` `noConsole` warnings remain (predate this phase, out of scope) |
| `just tui-preflight conformance` | PASS, all rows unchanged (SSH row still `GATED-UNAVAILABLE`, honest) |
| `just tui-bench focus-latency` | ALL BUDGETS PASS (interactive p50 0.0026ms / p99 0.0402ms vs 16.6/50ms; hoisted subscriptions 2 ≤ 122; RSS 219MB ≤ 1536MB) |
| `just tui-bench projection-worker` | ALL BUDGETS PASS (install p99 46.03ms ≤ 500ms; whole-Case fold p99 159.35ms ≤ 1500ms; apply p99 44.14ms ≤ 100ms; RSS 171MB ≤ 1536MB) |
| `just tui-build release` (×2) | OK both times, sha256 `f184b18725831f10…` byte-identical, clean 120x40 PTY launch/exit 0 |
| `just ruler` | PASS — `.wayfinder/answer-key.md`/`reference.lock` untouched |
| `graft build` | clean, 435 files (6 re-parsed, 429 cache-replayed), no errors |

## Closes / Does not close / Tripwire respected

**Closes in production** (per plan §4 Phase B): REQ-DATA-002/003/004/007, REQ-SYNC-001,
REQ-SAFE-005, REQ-VERIFY-006, and the observation half of REQ-OUT-003 — the production
read path now declares source identity/epoch/cursor/retention/granularity and reconciles
baseline-plus-incremental through a `ProjectionGeneration` and source cursors, exactly as
these requirements demand, proven against a real HTTP surface rather than only against
`InMemoryStreamPort`.

**Explicitly not closed**: a durable backend-ordered stream. `granularity:
"snapshot_interval"` says so in the port's own types — two real changes to the same unit
between two `observe()` calls fold into one delivered event. This is the plan's §6
residual (needs a new §20.2 route and a persisted sequence/epoch in the core; not attempted
here; requires explicit approval).

**Tripwire respected**: the plan required Phase B to *stop* if `granularity` could not be
expressed cleanly in the port's types (D1). It could — a two-member string union threaded
through `SourceDescriptor`/`SourceReconciliation` with zero `any`/casts — so Phase B
proceeded to completion.

**Frozen references**: `.wayfinder/answer-key.md` and `.gauntlet/reference.lock`
untouched — confirmed by `just ruler` PASS.

Re-level CLAIM-001/006 (and CLAIM-003's non-SSH half, if applicable) in Phase F, per plan
§4.
