# Recon — AREA=readmodel-views-trace

Read-only characterization of state ownership, read models, views, Inspector lens wiring, and RealityTrace (SXR) versus the Trace View. Every anchor below was verified by reading the cited range. Repo root: `/home/sprime01/projects/gauntlet`. All paths are under `workbench/src/` unless marked otherwise.

Contracts: spec `.agents/specs/gauntlet-tui-prime-workbench.spec.yaml`, then `DESIGN.md`, then ICM `.agents/specs/tui/INTERACTION_COMMAND_MAP.md` and UESC `.agents/specs/tui/UI_ELEMENT_STATE_CATALOG.md`.

---

## 0. Production read path (verified)

`main.ts:10` builds a `WorkbenchHost`. From there:

1. `workbench-host.ts:90` calls `new WorkbenchModel({dbPath, enableChat:false})`. It passes no `foldExecutor`, so the plan is folded inline.
2. `workbench-host.ts:94` starts a placeholder UUID generation.
3. `workbench-host.ts:98-99` creates the `CanonicalSurfaceStreamPort` and `LiveStreamClient`.
4. The poll loop runs at `:121-128`. `read()` at `:139-225` calls `port.observe()` (HTTP), then `stream.connect()` or `stream.poll()`.
5. `LiveStreamClient.applyToGeneration` (`live-streams.ts:859-872`) calls `WorkbenchModel.applyEvent` (`workbench-model.ts:494-544`).
6. `main.ts:28` renders with `session.render(host.model.getSnapshot())`.
7. `renderShell` (`shell.ts:873`) calls the registry renderer (`:982-990`) and `renderInspector` (`:1048`).
8. `TerminalShellRenderer` draws the result (`main.ts:89-93`). The mouse is off (`main.ts:85`, `useMouse:false`).

**SXR in production: none.** A grep of `preflight.ts`, `config.ts`, `workbench-host.ts`, `main.ts`, `shell.ts` and `terminal-renderer.ts` for `sxr` or `reality` finds nothing. `sxr-adapter` reaches production code only through:

- the `renderSxrEvidenceSection` import at `inspector.ts:36`;
- `export * from "./sxr-adapter"` at `index.ts:37`.

---

## 1. FullWorkbenchSnapshot field inventory

Types: `workbench-model.ts:244-263` extends `WorkbenchSnapshot` (`projection-store.ts:72-82`). Assembly: `getSnapshot` at `workbench-model.ts:756-795`.

| Field | Type | Source / writer | Anchor | Production status |
|---|---|---|---|---|
| id | string | store `snap-${gen}-${cursors}` | projection-store.ts:428; spread workbench-model.ts:776 | live |
| generation | string | store active generation | projection-store.ts:432 | live; id minted by LiveStreamClient live-streams.ts:742-748 |
| horizon | string\|null | model override `currentHorizon.id` | workbench-model.ts:757,777 | null unless hoisted |
| focus | FocusRef\|null | `currentFocus ?? storeFocusRef`; shadows store `{kind,id}` (projection-store.ts:421-425) | workbench-model.ts:778, 1044-1080 | UI selection only (port emits no focus_changed) |
| source_cursors | Record<src,cursor> | `source_cursors` table | projection-store.ts:368-377 | live (`gauntlet` only) |
| affordance_ids | string[] | store `affordance-<unit>`, rescoped to plan working set, plus `affordance:capability:*` | projection-store.ts:436; workbench-model.ts:761-768, 874-909 | capability part always empty |
| units | UnitProjection[] | `unit_updated` → units_projection | projection-store.ts:274-293, 380-382 | live |
| sessions | SessionProjection[] | `session_updated` → sessions_projection | projection-store.ts:317-335, 416-418 | always [] (port never emits session_updated: canonical-surface-stream-port.ts:168-195) |
| evidence | EvidenceProjection[] | `evidence_added` → evidence_projection | projection-store.ts:296-315, 397-413 | live (from `verification.evidence` only) |
| trace_records | TraceRecord[] | model in-memory push per applied frame; length-bounded view | workbench-model.ts:379, 522-528, 833-841 | in-memory only; reset per generation |
| chat_messages | ChatMessage[] | model push on `unit_updated` if chatEnabled | workbench-model.ts:531-539, 847-855 | always [] (enableChat:false) |
| chat_enabled | boolean | model flag | workbench-model.ts:314, 386, 781 | false |
| placements | FocusPlacement[] | model Map via `placeFocus` | workbench-model.ts:311, 624-641, 783 | always [] (placeFocus callers: tests only) |
| active_horizon | Horizon\|null | model | workbench-model.ts:784 | live |
| horizon_breadcrumb | string[] | `"Case"` + hoist-stack root labels | workbench-model.ts:770-773 | live |
| materialization | {horizon_id, working_set_size, counters} | worker plan if valid, else inline `planMaterialization` | workbench-model.ts:786-790, 925-942 | inline |
| attention | AttentionEntry[] | `deriveAttention(world)`: blocked units only | workbench-model.ts:822; horizon.ts:325-337 | live |
| context_candidates | ContextCandidate[] | `plan.context_candidates` | workbench-model.ts:792 | live |
| capability_reasons | CapabilityStatus[] | `resolveCapabilityStatus` over `declareCapabilities` | workbench-model.ts:206-230, 677-683, 858-866 | always []: host writes `observation.capabilities` but never calls declareCapabilities (workbench-host.ts:175-218) |
| semantic / sxr_lens / sxr_paths (not in type) | optional own props | `publishLensData` | inspector.ts:440-458 | never (test-only caller) |

Caches (T21): `baseCache` is keyed by (worldVersion, generationId) at `workbench-model.ts:805-826`. The trace, chat and capability caches use version counters (`:833-866`). Publication is O(1) through `immutableHistoryView` (`:94-154`).

---

## 2. Generations and stale-frame rejection

- **Store** (`projection-store.ts`):
  - `startGeneration` closes the previous generation and sets status syncing (`:178-198`). `markGenerationReady` is at `:203-208`, `closeGeneration` at `:213-224`.
  - `applyEvent` refuses `no_active_generation`, `superseded_generation` and `generation_closed` (`:247-261`). It commits the cursor and rows in one transaction (`:264-349`).
- **Model:**
  - `startGeneration` resets trace, chat, placements, the horizon stack and storeFocusRef, and bumps versions (`workbench-model.ts:420-435`).
  - `storeWouldApply` mirrors the store gate for focus attribution (`:1087-1090`). An unattributable `focus_changed` is refused (`:496-504`).
- **Stream** (`live-streams.ts`):
  - `reconcileAll` mints a generation, calls `model.startGeneration`, replays the baseline in canonical order, then calls `markReady` (`:737-771`).
  - `ingestLive` handles the stale cases: a superseded generation is rejected (`:898-904`); a frame arriving during resync is rejected as generation_closed (`:905-907`); an epoch mismatch triggers resync (`:912-921`); an old sequence is already_applied (`:922-928`).
  - A sequence gap is buffered, bounded, and triggers resync on overflow (`:929-931`, `:952-986`). A store refusal triggers a visible resync (`:990-997`). `poll` detects epoch or retention changes (`:1051-1112`).
- **Fold coordinator:** discards results for a superseded generation or task (`worker-folds.ts:621-632`). The model guards against a closed generation (`workbench-model.ts:396-411`).
- **Production epoch:** built from protocol, server version, run id and the sorted unit ids. Any change calls `replaceStorage`, which clears retained events (`canonical-surface-stream-port.ts:137-152`; `live-streams.ts:406-412`), so a fresh baseline follows.

This satisfies UESC §16.2: frames from an older generation are rejected.

---

## 3. How trace_records and chat_messages are built

**TraceRecord** has exactly 5 fields (`workbench-model.ts:156-162`):

- `trace_id` = `trace-${source_id}-${cursor}` (`:523`);
- `unit_id` = `payload.unit_id ?? "unknown"` (`:524`);
- `event_type`;
- `summary` = `[source] type (cursor)` (`:526`);
- `timestamp_ms` = frame time (`:527`).

It has no sequence, generation, actor, payload, FocusRef or parent. One record is pushed for every applied frame of any kind, including `focus_changed`.

**ChatMessage** (`:164-169`) is pushed only when `chatEnabled` and the frame is `unit_updated` (`:531-538`). It is a `role:"system"` message with text `"Unit X updated: status = Y"`. Chat is therefore a restatement of Event data, not communication.

**Production event content** (`canonical-surface-stream-port.ts:162-196`):

- On any content-digest change, one `unit_updated` is appended for every unit, changed or not, all with the same `Date.now()` timestamp.
- One `evidence_added` is appended per verification that has evidence.
- Granularity is declared `snapshot_interval` (`:26-31`). Retention is connect-scoped (`:23-25`).

**Headless parity:**

- `normalizedTrace` (`headless-replay.ts:325-334`) mirrors the model trace and adds `source_id` and `cursor` (`:205-212`).
- `parseTraceSummary` (`:622-645`) parses the model's `summary` string back, so the summary format is load-bearing for headless equivalence.
- `getTrace()` copies O(n) (`workbench-model.ts:999-1001`). Its callers are tests and benches only.

---

## 4. Focus identity and placements

- **FocusRef** is interned per (source, kind, sourceId). The id is `focus:${source}:${kind}:${sourceId}` (`focus.ts:18-20, 35-68`). `fromJSON` re-admits through the registry and rejects an id mismatch (`:75-103`).
- **Row factories:** `focusForUnit`, `focusForEvidence` and `focusForSession` produce kinds WorkItem, Evidence and Agent (`relations.ts:54-64`).
- **FocusPlacement** is scoped to a generation, surface and view, and carries no state (`focus.ts:132-172`). The model API is `placeFocus`, `relabelPlacement` and `removePlacement` (`workbench-model.ts:624-671`). It is used only by tests (`focus-identity.test.ts`, `headless-replay.test.ts:248-279`).
- **Relations:** `deriveRelations` builds `has_evidence` and `child_session` edges through a linear owner index. Each edge carries provenance and a valid flag (`relations.ts:108-178`). `traverseRelations` is cycle-safe (`:191-246`).
- **Navigator rows** are keyed by FocusRef: units, evidence closure, sessions, placement aliases, attention (`shell.ts:325-395`). Enter selects the focus (`:790-801`). Up/down on Primary only scrolls lines (`:775-786`).
- **Trace rows are not Focus.** A TraceRecord has no FocusRef, and no Event kind exists.

---

## 5. Views, Inspector, Domain, plugins

- **Registry.** `defaultViewRegistry` registers work, chat, trace, evidence, agents and attention (`views-chat-trace.ts:322-338`).
  - `availableViews` treats primaries as always available and lower surfaces as gated on an own snapshot property (`shell.ts:188-224`).
  - DSN-015 is enforced: `ROADMAP_VIEW_IDS=["harness"]` plus guards (`shell.ts:147-180`).
- **Work** (`views-work.ts:198-410`). Escalation-first ordering (`:72-82`). The affordance bar comes from the resolver through `materializeAffordances` (`:109-145`). Capability lines read `capability_reasons`, which is always empty in production (`:231-248`).
- **Chat** (`views-chat-trace.ts:42-113`). Disabled in production, so it shows "Chat projection disabled by the host" (`:52-68`). Message records use `kind:"attention"` (`:103`).
- **Trace** (`views-chat-trace.ts:116-159`):
  - A flat loop over every `trace_records` item on every render (`:134`).
  - Rows show glyph, `EVENT_TYPE`, summary, unit and trace_id. Records use `kind:"relation"` (`:147`).
  - Empty state only (`:126-132`).
  - `traceEventToken` maps anything except evidence and focus to `execution.running` (`:29-39`).
- **Evidence, Agents, Attention** (`:162-313`) are text rows over `evidence`, `sessions`+placements, and `attention`.
- **Primary scroll** is `lines.slice(offset)` with an "↑ N line(s) above" marker (`shell.ts:1008-1025`). The offset has no upper bound (`:782`). ShellState holds only per-view scroll (`:416-429`).
- **Inspector** (`inspector.ts:292-435`):
  - Sections: IDENTITY; STATE (WorkItem only, `:316-342`); RELATIONS (`:345-358`); AUTHORITY (`:361-371`); EVIDENCE (WorkItem focus only, `:374-405`); PROVENANCE (`:408-418`); SEMANTIC and REALITY TRACE, gated on published data (`:424-429`); AFFORDANCES (`:432`).
  - The focus comes from `snapshot.focus ?? state.location.focus` (`shell.ts:1037`). There is no pin state.
  - `renderShell` pushes only `section.lines` (`shell.ts:1058-1072`). `RenderedSurface` has no `records` (`shell.ts:228-236`).
- **Lens wiring.** `publishLensData` does `Object.freeze({...snapshot, ...lenses})` (`inspector.ts:453-458`). Its only callers are in `inspector-lens-wiring.test.ts:105-211`.
- **Domain.** `LOWER_VIEW_PROVIDERS.domain = "domain_model"` (`shell.ts:197`). No snapshot field and no registry renderer exist, so Domain is always ABSENT. DomainForge appears only as the Inspector SEMANTIC section through `renderSemanticSection` (`domainforge-adapter.ts:1406-1501`), and only in tests.
- **Plugins.** `PLUGIN_SLOTS` includes `trace.row` and `inspector.section` (`plugins.ts:86-96`). `PluginTraceRowContribution.render(record, snapshot)` is at `:150-155`. `traceRows()` (`:1684-1696`) is called only from `plugin-lifecycle.test.ts`. `PluginCompositionManager` is never created in `main.ts` or `workbench-host.ts`.

---

## 6. RealityTrace (SXR) — `sxr-adapter.ts`

### 6.1 Data flow

| Stage | Function / type | Anchor | Production-wired |
|---|---|---|---|
| Pinned contract | PINNED_SXR_OUTPUT_VERSION, PINNED_SXR_RECORD_KINDS (26) | sxr-adapter.ts:89, 164-191 | no (constants) |
| Typed identity | SxrEntityRef / SxrOrdinal / SxrEntryHash brands + WeakSet domains | :197-288 | no |
| Closed read-only vocabulary | SxrRefreshCommand: ledger_export, ledger_verify, query_evidence | :1129-1132 | no |
| Side-effect class (pre-invocation) | SXR_SIDE_EFFECT_CLASSES (appends/launches literal false) | :1135-1167 | no |
| Bind request | validateSxrRefreshRequest (absolute program, metachar refusal, XDG anchoring) | :1262-1359 | no |
| Spawn | SxrProcessRunner / bunSxrProcessRunner (Bun.spawn, cwd `/`, deadline+cap, kill) | :1395-1398, 1405-1508 | no (sxr-evidence-adapter.test.ts:1356) |
| Refresh | executeSxrRefresh (async; partial output never parsed) | :1559-1609 | no (tests: sxr-evidence-adapter.test.ts:807-1393; semantic-evidence-composition.test.ts:385; recovery-matrix.test.ts:94,111) |
| Parse cmd output | parseSxrCommandOutput (durability typed, never defaulted; ledger_position) | :405-530 (461-510) | no |
| Parse raw export | parseSxrLedgerExport (strict rows, kind_known, omissions, density) | :597-681 | no (tests; large-case.bench.ts:510) |
| Parse Slice0B envelope | parseSxrSlice0bEnvelopeOutput (+CAS) | :878-1090 | no |
| Project lens | projectSxrEvidenceLens → SxrLensProjection (absent \| projected) | :1764-1950 | no |
| Explicit Unit binding | admitSxrUnitBindings (refuses names) | :1982-2030 | no |
| Verification path | sxrVerificationPath (declaration→binding→question→threshold→evidence→sxr_settlement, missing[]) | :2035-2187 | no |
| Offer candidate evidence | offerSxrEvidenceAsGauntletEvidence (establishes_* literal false) | :2213-2334 | no |
| Admission gate | SxrEvidenceAdmissionGate.admit (decision is input) | :2384-2431 | no |
| Authoritative frame | authoritativeSxrEvidenceFrame → `evidence_added` | :2438-2474 | no (semantic-evidence-composition.test.ts:260-268 applies it to a model) |
| Publish to snapshot | publishLensData | inspector.ts:453-458 | no (inspector-lens-wiring.test.ts only) |
| Inspector gate | `if (snapshot.sxr_lens !== undefined)` | inspector.ts:427-429 | code path live, data never present → dormant |
| Render section | renderSxrEvidenceSection "REALITY TRACE" | sxr-adapter.ts:2487-2580 | dormant |
| Shell emission | renderShell pushes section.lines only; records dropped | shell.ts:1069-1071, 228-236 | live, but drops path steps |

### 6.2 Owns, receives, stores, derives, renders

- **Owns:** nothing canonical. The SXR ledger (`$XDG_DATA_HOME/sxr/repos/<id>/ledger.db`, `:1138`) is SXR-owned. The adapter holds only transient parse and lens values.
- **Receives:** stdout of the `sxr` CLI through `executeSxrRefresh`, which no production code calls.
- **Stores:** nothing. No SQLite table, no model field, no snapshot field. `SxrEvidenceAdmissionGate` keeps an in-memory `admitted` Map (`:2385`). `offerCounter` is module-global (`:2234, 2311`).
- **Derives:**
  - The lens partitions rows by `record_kind` into questions, thresholds, snapshots, run.started, evidence and settlements. All other kinds, including `run.finished`, are counted in `uninterpreted` (`:1815-1933`).
  - The path, bindings and offer are derived from the lens.
- **Renders:** Inspector section only.
  - ABSENT form (`:2491-2514`).
  - Projected form: lines show repo, the row count labelled "committed ledger record(s)", per-kind counts and unparsed kinds (`:2515-2542`).
  - Per-step ledger ordinal and entry_hash exist only in `records` (`:2543-2578`), which the shell never emits.
  - The Evidence and Trace views never read SXR.

### 6.3 Identity, ordering, committed vs pending, record kinds, admission

- **Identity:** each row carries `ledger_id`, `ordinal: SxrOrdinal` (dense, 1-based) and `entry_hash: SxrEntryHash` (`sha256:<64hex>`) (`:536-549, 633-649`). Path steps carry `{ordinal, entry_hash}` (`:2045`).
- **Ordering:** export order. `ordinal_density_ok` checks per-ledger density (`:667-674`). Chain linkage cannot be checked because the export omits `previous_entry_hash`, `payload_hash`, `committed_at` and `writer_identity_ref` (`:577-587`).
  - `ordinal_density_ok` lives on the parse (`:562`) and is not carried into `SxrEvidenceLens` (`:1764-1777`).
  - `ledger_verify` output is never projected.
- **Committed vs pending:** the export is committed-only by construction.
  - Pending exists only as the command-output `durability` channel (`:302-306, 461-473`).
  - The offer receives pending state as a caller-supplied boolean `pending_durability_observed` (`:2250-2259`).
  - The lens has no pending records and no pending rendering.
- **Record kinds:** 26 pinned kinds (`:164-191`). Unknown kinds are kept with `kind_known:false` (`:648`). The lens interprets 6 kinds (`:1818, 1839, 1862, 1878, 1894, 1914`).
- **Admission gate** (REQ-SXR-002):
  1. The offer is refused for pending evidence, a lens that is not projected, a repo or ledger mismatch, or evidence missing from the export (`:2252-2310`).
  2. The offer's `admission_contract` locator is `sxr-ledger:<ledger>#<ordinal>:<hash>` and its id is `sxr:<repo>:<eid>` (`:2320-2326`).
  3. The gate refuses `refused_canonically`, `decision_mismatch` and `already_admitted` (`:2391-2412`).
  4. The frame is built only from an admitted decision, and `verified` is copied verbatim (`:2450-2473`).
  5. SXR settlement has no import channel. `SxrSettlementStatus` is source-typed (`:1672`).

### 6.4 Tests covering SXR

Counts are grep counts of `it(`/`test(` lines.

- `sxr-evidence-adapter.test.ts` (57): 11 describes covering output parse, export parse, Slice0B, CAS, bounded refresh, lens, path, admission, rendering and qualification. The live-binary blocks are `describe.skipIf` on binary sha (`:1336, 1410`).
- `semantic-evidence-composition.test.ts` (7): binding-mode matrix; admission composition into a WorkbenchModel evidence row (`:217-291`); source loss; deny matrix.
- `inspector-lens-wiring.test.ts` (6): the presence gate, plus the live shell path using `publishLensData` (`:193-220`).
- `recovery-matrix.test.ts:86-146` `realSxrProbes`, feeding `recovery-matrix.ts:2756` `runSxrAbsence`.

---

## 7. Trace View data vs RealityTrace

### 7.1 Trace event kinds by source

| Source | Kinds | Anchor | Reaches trace_records / Trace view |
|---|---|---|---|
| AuthoritativeEventFrame union | unit_updated, evidence_added, session_updated, focus_changed | projection-store.ts:31 | yes, every applied frame (workbench-model.ts:522-528) |
| Production CanonicalSurfaceStreamPort | unit_updated (all units per change), evidence_added | canonical-surface-stream-port.ts:168-195 | yes |
| InMemoryStreamSource (tests/benches) | any of the 4 | live-streams.ts:373-390 | yes |
| Chat projection (model) | synthesized from unit_updated | workbench-model.ts:531-538 | Chat only, disabled in production |
| Headless normalized trace | the same 4, plus source_id and cursor | headless-replay.ts:205-212, 325-334 | parity check only |
| SXR ledger | 26 pinned kinds plus preserved unknowns | sxr-adapter.ts:164-191, 648 | no |
| SXR lens (interpreted) | question.declared, threshold.declared, semantic.snapshot, run.started, evidence.created, settlement.registered | sxr-adapter.ts:1815-1933 | no (Inspector section only) |
| SXR admission | evidence_added (sxr:-prefixed id) | sxr-adapter.ts:2438-2474 | only in tests |
| Plugin trace.row | renderer over TraceRecord | plugins.ts:150-155, 1684-1696 | no (manager not instantiated in production) |
| Prime / kernel | none | — | no |

### 7.2 Aspect comparison

| Aspect | Trace View (`trace_records`) | RealityTrace (SXR lens) |
|---|---|---|
| Receipt | HTTP poll → port diff → LiveStreamClient → model.applyEvent | `sxr` CLI export via executeSxrRefresh (never called in production) |
| Storage | model in-memory array (workbench-model.ts:379); not in SQLite; cleared per generation (:422) | none (transient value) |
| Event identity | `trace-${source}-${cursor}`; opaque cursor from `mintOpaqueCursor(seq)` (live-streams.ts:331-334) | ledger_id + dense ordinal + sha256 entry hash |
| Ordering | per-source sequence; cross-source sorted fold (live-streams.ts:9-14); time = observation `Date.now()` | export order; density only; no committed_at |
| Turns / steps | none | none |
| Tool calls | none | none (observation/claim uninterpreted) |
| Agents | session_updated shape exists; none in production | none |
| Messages | none (chat synthesized from events) | none |
| Runs | run id only in host header (main.ts:38) | run.started; run.finished uninterpreted |
| Evidence / verification / settlement | evidence_added (verified bool) | typed question/threshold/evidence/SXR settlement + path + missing |
| Committed vs pending | n/a | committed-only export; pending only via durability channel or caller flag |
| Paging / bounds | unbounded in-memory; no load-older; connect-scoped retention | whole export per refresh; 32 MiB cap, 10 s deadline (:1192-1193) |
| Live-follow / hold / unseen | none (no symbol in workbench/src) | none (no incremental vocabulary) |
| Staleness guard | generation + epoch + sequence | none: lens carries no generation or refresh epoch (:1779-1785) |
| Rendering | flat rows (views-chat-trace.ts:116-159) | Inspector summary lines; path records dropped |

### 7.3 Can RealityTrace back a turn-aware execution ledger?

**No, not on its own.**

**What it offers:** strong committed-record identity (ledger_id, ordinal, entry_hash) and a typed verification chain (question → threshold → evidence → SXR settlement).

**What it lacks for an execution ledger:**

- turns, messages, tool calls (input/output/duration), agents or subagent topology;
- Gauntlet Run and Unit identity (only operator-declared bindings);
- timestamps (`committed_at` is omitted) and actor (`writer_identity_ref` is omitted);
- checkable hash linkage;
- pending records, incremental or cursor refresh, paging;
- any generation or staleness tag on refresh results;
- interpretation of `run.finished`, `observation.recorded` and `claim.made`.

It can back a verification and evidence lane that attaches to execution rows through explicit bindings.

### 7.4 What a composed projection needs (REQ-SXR-001/002, without merging authorities)

1. **Two typed row identities, never merged.** Execution rows are keyed by (source_id, source_epoch, sequence, cursor, generation). SXR rows are keyed by (ledger_id, ordinal, entry_hash). The composite row is a derived placement referencing exactly one of them.
2. **FocusRef kinds for both.** Today no Event kind exists, and SXR lens records mint no FocusRef even though `SXR_SOURCE_ID="sxr"` (`:291`).
3. **Ordering.** Keep per-authority canonical order. Interleave across sources only as a declared presentation fold (`live-streams.ts:9-14`). SXR has no `committed_at`, so attach SXR rows by explicit binding (Run ↔ `run:<ulid>`, Unit ↔ entity ref via `admitSxrUnitBindings`), never by time or label.
4. **Separate committed, pending and unverified lanes.**
   - committed: export rows;
   - pending: command-output `durability` or `ledger_position`;
   - verification: `ledger_verify` output plus `ordinal_density_ok`.
5. **Source-typed settlement.** `sxr_settlement` stays source-typed. Gauntlet evidence appears only after offer → canonical decision → `evidence_added`. That row then belongs to the execution stream with its `sxr:` id and locator visible.
6. **Refresh off the render loop.** Refresh runs through `executeSxrRefresh`. Results are tagged with a refresh epoch or generation, and superseded results are rejected. ABSENT and loss are labelled (REQ-SYNC-003).
7. **Durable, pageable execution history.** Required for turn/step/tool/agent/message grouping (marked derived, UESC §16.3-16.4), load-older, and live-follow/hold/unseen. The production port declares that a durable backend-ordered stream is unbuilt (`canonical-surface-stream-port.ts:26-31`).

---

## 8. Classification

### preserve

- `WorkbenchModel.applyEvent` fan-out, store gated first (workbench-model.ts:494-544). REQ-DATA-006.
- Store generation gates plus atomic cursor/row transaction (projection-store.ts:246-355).
- LiveStreamClient reconcile/ingest/buffer/resync/poll (live-streams.ts:737-1112). Satisfies UESC §16.2 reconnect and rejection.
- CanonicalSurfaceStreamPort's honest epoch and `snapshot_interval` declaration (canonical-surface-stream-port.ts:124-198). The production read path.
- `immutableHistoryView` plus version caches (workbench-model.ts:94-154, 805-866).
- Fold coordinator stale discard plus model guard (worker-folds.ts:610-636; workbench-model.ts:396-411).
- FocusRef interning, fromJSON re-admission, generation-scoped placements (focus.ts:35-172; workbench-model.ts:624-671).
- `focus_changed` attribution refusal (workbench-model.ts:1044-1080).
- `deriveRelations` / `traverseRelations` with provenance and valid flag (relations.ts:108-246).
- Headless log-vs-store equality (headless-replay.ts:584-613). The summary-parse coupling is at :622-645.
- SXR typed identities, strict export and output parsers (sxr-adapter.ts:197-288, 405-681).
- Bounded refresh boundary (sxr-adapter.ts:1129-1609).
- Lens, bindings, path with explicit missing links (sxr-adapter.ts:1787-2187).
- Offer, admission gate and frame builder (sxr-adapter.ts:2213-2474).
- Inspector lens gate plus ABSENT rendering (inspector.ts:420-429; sxr-adapter.ts:2491-2514).
- `availableViews` provider gating plus DSN-015 guard (shell.ts:147-224).
- `materializeAffordances` resolver-driven policy (views-work.ts:109-145).
- `renderEvidenceRecord` / `renderAuthorityRecord` DSN-008 record shapes (inspector.ts:130-166).
- Plugin slot catalog and trace.row contract (plugins.ts:86-155).
- Test suites: sxr-evidence-adapter, semantic-evidence-composition, inspector-lens-wiring, stale-frame, projection-fanout, headless-replay, work-chat-trace.

### replace

- `renderTraceView` flat dump of all records (views-chat-trace.ts:116-159).
- `traceEventToken` gives every non-evidence update the running cue (views-chat-trace.ts:29-39).
- `TraceRecord.summary` string-encoded provenance (workbench-model.ts:526). Replacing it must keep headless parity (headless-replay.ts:622-645).
- Record kind taxonomy misuse: chat uses `attention`, trace uses `relation` (views-chat-trace.ts:103, 147).
- Line-offset Primary scroll with no upper bound (shell.ts:775-786, 1008-1025).
- Inspector flattened to text with records discarded (shell.ts:1058-1072, 228-236).
- REALITY TRACE section lines show summary counts only (sxr-adapter.ts:2515-2542).
- Evidence/Agents/Attention text rows (views-chat-trace.ts:162-313).
- Work view text composition (views-work.ts:198-410).

### violates

- Chat synthesized from `unit_updated` events (workbench-model.ts:531-538). UESC §1 Message↔Event; REQ-DATA-005.
- Trace rows are polled snapshot diffs, not durable occurrences (canonical-surface-stream-port.ts:162-196). UESC §16.6; DESIGN "Trace rows".
- Trace history silently lost on resync or epoch change, with no unavailability label (workbench-model.ts:420-423; canonical-surface-stream-port.ts:144-152; views-chat-trace.ts:126-132). REQ-SYNC-003.
- Inspector PROVENANCE shows the source's latest cursor, not the row's producing cursor (inspector.ts:407-418). REQ-DATA-007.
- SXR ordinal/entry_hash path steps never reach the screen (shell.ts:1069-1071; sxr-adapter.ts:2543-2578). REQ-SXR-001. Latent.
- The lens drops ledger verification and density status (sxr-adapter.ts:562 vs 1764-1777). REQ-SXR-001. Latent.
- The Trace pane has no loading, stale, partial or disconnected state (views-chat-trace.ts:126-132). REQ-UX-006.

### missing

- TraceStream COLD/HYDRATING/LIVE_FOLLOW/INSPECTION_HOLD plus unseen_count. UESC §16.2; ICM §16.4, §16.5, §50.8.
- Load older history, preserving anchor. ICM §16.6; DESIGN Trace "backward paging".
- Trace row select/open with Inspector following the Event; TraceRow as FocusPlacement. ICM §16.1-16.2; UESC §16.5.
- Turn, Step, ToolCall, Agent and Message grouping, marked derived. UESC §16.3, 16.4, 16.7.
- Trace search, filter and related-open. ICM §16.7-16.9.
- Gauntlet vs Prime/kernel provenance in Trace. DESIGN Trace rows.
- Inspector EMPTY/FOLLOW_SELECTION/PINNED states. UESC §7.4.
- Production SXR binding and exposure in the Evidence and Trace surfaces. REQ-SXR-001.
- SXR pending and unverified records shown separately. REQ-SXR-001.
- Domain view renderer and `domain_model` provider when bound. DESIGN Views table (DESIGN.md:549); REQ-DOM-001 (SHOULD).
- The `trace.row` plugin slot is not composed into the Trace view. DSN-012, per plugins.ts:83.
- `capability_reasons` is never populated in production. REQ-AFF-005 (SHOULD).
- Virtualization and history bound for trace rows. UESC §16.5 VIRTUALIZED.

---

## 9. Notes and uncertainty

- **trace_id uniqueness.** `mintOpaqueCursor` is `(seq*7919)%104729` (live-streams.ts:331-334). Cursors, and therefore `trace_id`, repeat after 104,729 sequences in one epoch. Inferred from the arithmetic; not tested.
- **Snapshot focus mixes two things.** `snapshot.focus` combines UI selection (`selectFocus`) with authoritative `focus_changed` (workbench-model.ts:778). Whether this collapses Selected↔primary attention (UESC §1) is unclear.
- **Doc drift.** The sxr-adapter header names `admitSxrEvidenceOffer` and `CanonicalEvidenceAdmissionDecision` (sxr-adapter.ts:65-67). The actual symbols are `SxrEvidenceAdmissionGate.admit` and `CanonicalSxrEvidenceDecision`.
- **Offer ids are process-order dependent.** `offerCounter` is module-global (sxr-adapter.ts:2234), which affects headless determinism if offers are ever replayed.
- **Attention coverage.** `deriveAttention` covers only blocked units (horizon.ts:325-337). Other attention kinds were not assessed against the UESC attention catalogue.
- **Mouse is off in production** (main.ts:85). ICM §35 belongs to another area; recorded here as context only.
- **Evidence and Agent focus in Inspector.** Selecting an Evidence Navigator row gives "no projected state" and an empty EVIDENCE section (inspector.ts:316-342, 374-376). Detail appears only through RELATIONS.
- **Test counts** are grep counts of `it(`/`test(` lines, not runtime results. No tests were run for this recon.
