# Increment 6 (remainder) — RealityTrace ledger lane projection (pure)

Status: COMPLETE (bounded slice). Files owned and delivered:
`workbench/src/reality-trace-lane.ts`, `workbench/src/reality-trace-lane.test.ts`.
New files only; nothing else touched; nothing committed.

## What this is

The pure projection from an SXR evidence lens (`projectSxrEvidenceLens`,
`workbench/src/sxr-adapter.ts`) to the Trace surface's RealityTrace lane rows,
with refresh-epoch staleness — the RealityTrace half of UESC §16.0's one
ledger / two labelled, never-merged sources. It renders nothing, spawns
nothing, and never imports the adapter: the caller maps `SxrEvidenceLens` onto
a narrow structural input type defined here, unwrapping the branded
`SxrOrdinal` / `SxrEntryHash` values at that boundary. The only import is a
TYPE from `./trace-stream`, so the stream hand-off is compile-checked against
the real `TraceStreamRecord` shape.

Adversarial teeth pinned: "staleness labels, never drops" (REQ-SXR-001
honesty) and "uninterpreted kinds are counted, not invented."

## API summary

Types: `RealityTraceLaneRow = { id, source: "realitytrace", ordinal,
entryHash, kind, label, pending: false }` (committed-only by construction —
the ledger export carries committed rows only; the type-level `pending:
false` documents that pending records arrive via a future durability channel
and are never fabricated here); `RealityTraceLane = { repoId, ledgerId,
refreshEpoch, rows, uninterpretedKinds, totalRecords }`.

Input types (narrow structural lens view, caller maps):
`RealityTraceLaneLensRecord = { ordinal, entryHash }`;
`RealityTraceLaneLensUninterpreted = { record_kind, count }`;
`RealityTraceLaneLens = { status: "projected", questions, thresholds,
snapshots, runs, evidence, settlements, uninterpreted, totalRecords }` — only
the `projected` variant of `SxrLensProjection` has a lane; absent/invalid
lenses are a shell source status (UESC §16.2 RealityTrace source machine),
and passing one throws `TypeError`.

- `projectRealityTraceLane(lens, meta)` — flattens the six interpreted
  partitions (fixed order: questions, thresholds, snapshots, runs, evidence,
  settlements), validates everything (untrusted model output), sorts by
  ordinal ascending (the authoritative per-source order), collapses duplicate
  ordinals to ONE row, mints ids `sxr:<ledgerId>#<ordinal>`, preserves
  uninterpreted kind identity verbatim (deduped, first-seen order), and
  returns a fully frozen lane. Throws `TypeError` on malformed input
  (non-projected status, empty ids, non-integer/negative epochs and
  ordinals, empty hashes/kinds, non-positive counts).
- `laneStale(lane, currentEpoch)` — `refreshEpoch !== currentEpoch` (any
  mismatch, older OR newer, is staleness). Staleness is a label; it never
  drops, hides, or replaces rows (UESC §16.2: a failed refresh keeps the last
  CURRENT lens visible and marks it stale).
- `laneRowsForStream(lane)` — `readonly TraceStreamRecord[]`-shaped output
  `{ id, source: "realitytrace", ordinal }`, exactly (no `timestampMs` — the
  ledger export omits `committed_at`, UESC §16.0). Works identically on stale
  lanes: rows are returned regardless of epoch.

## Invariant test list (36 tests, 182 assertions, all green)

Written RED first (module absent → "Cannot find module", 0 pass / 1 fail),
then implemented:

- (a) row id grammar: every id is exactly `sxr:<ledgerId>#<ordinal>`, unique
  within the lane, embeds ledger + ordinal, prefixed `sxr:` and therefore
  disjoint from gauntlet-style `trace-<source>-<cursor>` ids.
- (b) cross-lane identity: two lanes from different ledgers never share row
  ids (same ordinals, disjoint id sets); same ledger + ordinal is a stable
  function of the id.
- (c) staleness labels, never drops: epoch mismatch (older or newer) →
  `laneStale` true; a stale lane still returns all rows from
  `laneRowsForStream`; rows are byte-identical across epochs — only
  `refreshEpoch` differs.
- (d) uninterpreted kinds are counted, not invented: unknown kinds never
  become rows; identity preserved verbatim (no case folding, trimming, or
  normalization); repeated kinds deduped preserving first-seen order; the
  lens's word is final — a `question.declared` the lens diverted to
  `uninterpreted` stays uninterpreted (the lane never re-derives
  interpretation from kind names).
- (e) committed-only: runtime `pending === false` and `source:
  "realitytrace"` on every row; type-level teeth — `const proof: false =
  row.pending` compiles, and a `// @ts-expect-error` on `{ ...row, pending:
  true }` is a verified tsc error (an unused directive would itself fail
  `bun run check`).
- (f) ordering and collapse: rows sorted by ordinal ascending regardless of
  partition order; ONE ledger row → ONE lane row (an `evidence.created` row
  carrying two evidence entries yields a single row — identity is
  `ledger_id` + `ordinal` + `entry_hash`, UESC §16.8).
- (g) stream hand-off: `laneRowsForStream` maps to exactly
  `{ id, source: "realitytrace", ordinal }` (`toStrictEqual`), preserves
  order/ids, assigns to `readonly TraceStreamRecord[]` at compile time, and
  coexists with gauntlet records in one stream without id collision or
  source merging.
- (h) totals: `totalRecords` is the lens total (interpreted + uninterpreted
  + lens-accounting omissions), independent of `rows.length`; empty lens →
  empty lane with zero totals.
- (i) purity/freezing: lane, rows, row objects, `uninterpretedKinds`, and
  stream records all frozen; input copied on admission (later caller
  mutation cannot change the lane); caller's lens never mutated (sorting
  runs on the copy).

## Semantic decisions (flag for orchestrator/wiring review)

1. Input shape mirrors the lens partition (six record arrays +
   `{record_kind, count}` uninterpreted + `totalRecords`) rather than a flat
   kind-tagged list. Category ↔ record_kind is 1:1 in
   `projectSxrEvidenceLens`, so the lane derives `kind` and `label` from the
   partition via a frozen table (six kinds: question.declared,
   threshold.declared, semantic.snapshot, run.started, evidence.created,
   settlement.registered). Labels chosen: "Question declared", "Threshold
   declared", "Semantic snapshot", "Run started", "Evidence created",
   "Settlement registered" — presentation only, never identity.
2. Duplicate ordinals are COLLAPSED, not rejected: the lens legitimately
   produces multiple records for one `evidence.created` /
   `settlement.registered` row. One ledger row = one lane row; the finer
   records remain Inspector detail in the lens. If a caller maps the same
   ordinal into two partitions (a mapping bug), the fixed flatten order
   (questions → … → settlements) deterministically breaks the tie.
3. `uninterpretedKinds` carries kind identity only, not counts — the frozen
   output contract is `readonly string[]`; counts stay owned by the lens
   (`lens.uninterpreted[].count`), which the input still carries and
   validates (positive integers) so the caller mapping stays structural.
4. `rows.length + Σ uninterpreted counts` is NOT asserted to equal
   `totalRecords`: the adapter can interpret a known-kind row into zero
   records (e.g. an `evidence.created` row whose payload has no valid
   evidence entries) without counting it uninterpreted. Such rows remain in
   `lens.rows` upstream; the lane neither invents rows nor counts for them.
   `totalRecords` reports the honest lens row total.
5. `laneStale` is `!==`, not `<`: the brief pins "epoch mismatch → stale".
   A lane newer than the caller's current epoch (a completed refresh the
   shell has not yet admitted) is still a mismatch and is labelled stale.
6. Validation is admission-time `TypeError` (house style, cf.
   `trace-stream.ts`): model output is untrusted input. Entry-hash FORMAT is
   NOT re-verified here (no sha256 regex) — `sxrEntryHash` minting in the
   adapter is the format chokepoint; the lane requires non-empty strings and
   documents that authority explicitly.

## Integration notes for the Trace view / shell

- Map `SxrEvidenceLens` → `RealityTraceLaneLens` field-by-field
  (`requireSxrOrdinal(record.row.ordinal)` /
  `requireSxrEntryHash(record.row.entry_hash)`); pass repo id, ledger id
  (from `lens.rows[0].ledger_id` or the binding), and the shell's refresh
  epoch counter as `meta`. The refresh epoch must be minted by the shell
  around `executeSxrRefresh` (the refresh boundary in `sxr-adapter.ts`
  ~:1559-1609): increment on every accepted refresh completion; a discarded
  late refresh must NOT advance the admitted epoch.
- Render: `laneStale(lane, currentEpoch)` drives a STALE source label
  (UESC §16.2), never row removal; `uninterpretedKinds` feeds the "unknowns"
  Inspector line (UESC §16.1); `totalRecords` vs `rows.length` shows the
  honest ledger size.
- `laneRowsForStream(lane)` output is appendable to a TraceStream via
  `streamAppend` / `streamHydrated` unchanged; per-source ordering by
  ordinal is the view's responsibility (trace-stream treats cross-source
  order as arrival order).
- Pending (spooled / queued_for_commit) rows: NOT here by design. When the
  durability channel lands, it must project into a separate pending segment
  (UESC §16.0 "pending state renders apart from committed records"), not
  into this committed-only lane.

## Verification evidence

- RED: `bun test src/reality-trace-lane.test.ts` with the module absent →
  "Cannot find module './reality-trace-lane'" (0 pass / 1 fail).
- GREEN: `bun test src/reality-trace-lane.test.ts` → 36 pass / 0 fail,
  182 expect() calls.
- `bun run check` (tsc) run once at the end: ZERO errors in the two
  delivered files (`grep -c reality-trace-lane` → 0), including the
  `@ts-expect-error` negative type test being a genuine error. The command
  exits 1 overall: errors appear ONLY in concurrent agents' in-flight
  uncommitted files (`shell-gui.test.ts` / `shell.ts` — modified at 13:28,
  AFTER this slice's clean pre-work baseline; and a new
  `navigation-history.test.ts` referencing a not-yet-written module). Not
  caused by and not fixable from this slice; flagged to the orchestrator.
- `bun x biome check src/reality-trace-lane.ts src/reality-trace-lane.test.ts`
  → clean (after one `--write` formatting pass plus removal of a drafting
  leftover unused interface).
- Full suite intentionally not run (bounded-slice discipline); no commits;
  `sxr-adapter.ts` / `shell.ts` untouched.
- `.agents/CURRENT_STATUS.yml` intentionally NOT edited: the task pins "new
  files only", and that file already carries concurrent agents' uncommitted
  modifications; this report is the durable record for the slice.

## Next move (for the wiring increment)

Shell wiring: bind the lane projection at the `executeSxrRefresh` boundary
(mint/advance the refresh epoch there), render the lane beside the Gauntlet
stream with a STALE label from `laneStale`, and surface
`uninterpretedKinds` + `totalRecords` in the RealityTrace Inspector segment.
