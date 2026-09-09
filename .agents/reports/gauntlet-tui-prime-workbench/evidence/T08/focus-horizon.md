# T08 Evidence — Focus, Relation, Horizon, and Progressive Materialization Model

Date: 2026-09-08
Task: T08
Proof Level: P2 (confirmation: peer)
Scope: Stable FocusRef identity, multi-placement without forking, source-typed relation edges, hoist/de-hoist Horizons, saved noncanonical horizons, horizon-driven materialization (subscriptions, queries, prefetch, context candidates, visible affordances), case-wide attention independence, and inspectable capability reasons.
Preregistration: frozen at `.agents/research/preregistrations/task-gauntlet-tui-prime-workbench-T08.prereg.yaml` — verified byte-identical this session (sha256 `8456810e…b1958`), never modified.

## Settled Requirements

- **REQ-FOCUS-001** (identity stable across placements): `FocusRef.for(source, kind, sourceId)` interns one immutable identity per triple (`focus-identity.test.ts`: same object returned referentially; id `focus:gauntlet:WorkItem:u-root`; different source/kind → different identity; identity independent of labels and positions).
- **REQ-FOCUS-002** (multiple placements of one FocusRef without copying canonical state): `FocusPlacement` records reference the interned FocusRef and carry only occurrence state. Attack 1 places one WorkItem under Plan/Current Run/Agent/Attention and mutates one placement-local label; all placements still resolve to the same identity and the canonical store rows are unchanged.
- **REQ-FOCUS-003** (hoist/de-hoist alters only the active Horizon, constraining subscriptions/queries/prefetch/visible affordances/context): `hoistToFocus`/`dehoist` swap the active Horizon; counters, working-set membership, context candidates, and `affordance_ids` all narrow and restore with identical source rows and identical interned identities (Attack 2, `horizon-hoist.test.ts`). Affordance narrowing: hoisted snapshot contains `affordance-u-root` but not `affordance-u-other`.
- **REQ-FOCUS-004** (prefer horizon-local incremental materialization over eager whole-Case loading): `planMaterialization` produces the whole-Case plan only when no horizon is active; negative-control test asserts every counter (subscriptions/queries/prefetches/context_candidates) strictly shrinks on hoist and the hoisted working set equals the declared closure exactly; an event landing outside the hoisted horizon leaves the hoisted counters and working set byte-identical while the whole-Case world advances.
- **REQ-FOCUS-005** (saved Horizons are preferences, not domain truth): `SavedHorizonRegistry` is a plain JSON-serializable registry, deliberately separate from `SqliteProjectionStore`; save/restore writes no store rows, restores the SAME interned root identity across serialization into a fresh model, still narrows against the CURRENT world after the source advanced, and rejects malformed payloads (`MalformedSavedHorizonsError`).
- **REQ-GOAL-001** (local coherence without authority): one snapshot coherently carries units, sessions, evidence, trace history, relations, attention, and context candidates on one generation; every relation edge and projection row keeps `generation_id`/`source_id`/`source_cursor` provenance — the model owns no canonical truth.
- **REQ-GOAL-006** (Progressive Affordance Materialization shapes loading, subscriptions, local queries, agent context, capability presentation): all five dimensions are driven by the active Horizon in the model snapshot (see REQ-FOCUS-003/004 tests plus capability test). *Slice note:* the sixth listed dimension, "current actions", is the T09 canonical affordance resolver's surface; T08 shapes the data those actions will resolve against.
- **REQ-OPS-004** (T08 slice: mandatory closure + no horizon→critic leakage): with declared relation queries `[]`, the candidate set still contains `mandatory_reference`, `mandatory_governance`, and `mandatory_evidence` for the focused unit; every plan (hoisted or not) includes mandatory closure. No API on the model can turn Horizon relations into a formal-critic envelope — critic context assembly is simply not a model capability (the allowlist itself is enforced in the ops/critic layers, outside T08's surface).
- **REQ-AFF-004** (installed ≠ available): `resolveCapabilityStatus` marks an installed capability `available` only when all six current-path gates pass; installed with a failed gate renders `unavailable` with `payment_gate_not_passed` and is excluded from `affordance_ids` (negative control asserts absence, not just non-greying).
- **REQ-AFF-005** (unavailable affordances stay inspectable): unavailable entries (installed-with-failed-gate and `not_installed`) remain in `capability_reasons` with machine-readable reason codes, and statuses carry over unchanged across hoisting (no fake availability under narrowing).
- **REQ-UX-002** (T08 slice): selection retains one identity across views/surfaces; hoist exposes a breadcrumb (`["Case", "u-root", "s-1"]`) and one-step return (`dehoist` pops one horizon, not all); a persistent Case-wide attention summary (blocked-unit escalations, `horizon_independent: true`) survives hoisting without loading the far Unit into the horizon context. *Not proven here:* rendered-TUI behavior (later task) and budget/source-health attention classes (the projection store carries no such rows yet — only escalation-shaped blocked Units).
- **DSN-006** (co-owned): placements are projections of one FocusRef; no placement copies or forks canonical object state (Attack 1 proves referential identity across four placements plus store-row invariance).

## Implementation Details

1. **`workbench/src/focus.ts`** (new): `focusRefId` deterministic id derivation (`focus:${source}:${kind}:${sourceId}`); `FocusRef` class with a process-wide intern table — `FocusRef.for` returns the same frozen instance per triple, `FocusRef.fromJSON` validates untrusted documents (shape + id-consistency, typed `MalformedFocusRefError`) and re-resolves through the registry so deserialization can never fork identity; `FocusPlacement`/`createPlacement` with generation-scoped occurrence ids (`placement:…`) and placement-local `label_override`.
2. **`workbench/src/relations.ts`** (new): `ProjectionWorld` (the store slice), `RelationEdge` (source-typed, provenance, source version, endpoint validity), `deriveRelations` (unit→evidence `has_evidence`, session-parent `child_session` from authoritative rows), `traverseRelations` (cycle-safe BFS per declared `RelationQuerySpec`, visited-set bounded), and `focusForUnit/Evidence/Session` factories that intentionally mint without `human_label` (display text is placement/view concern; first-minter-would-win labels were rejected as an ordering hazard).
3. **`workbench/src/horizon.ts`** (new): `Horizon` entity + `createHorizon` (frozen); `DEFAULT_HOIST_RELATION_QUERIES`; `SavedHorizonRegistry` (plain serializable preferences, validating `fromJSON`/`toJSON`, re-resolving FocusRefs on load); materialization planning — `planMaterialization(world, horizon|null)` computing the working set, focus-scoped subscriptions, scoped queries (`focus_load`/`relation_traversal` vs whole-Case `case_scan`), prefetch keys, and context candidates with mandatory Reference/governance/evidence closure always included; `deriveAttention` (Case-wide, horizon-independent).
4. **`workbench/src/workbench-model.ts`** (upgraded): `selectFocus(FocusRef)`/`selectHorizon(Horizon|null)`; new `hoistToFocus`/`dehoist` (hoist stack = breadcrumb + one-step return); `placeFocus`/`relabelPlacement`/`removePlacement` (placement-local labels never touch the ref); `declareCapabilities` + `resolveCapabilityStatus` (honest gate evaluation); `getRelations`/`getMaterializationPlan`; snapshot gains `placements`, `active_horizon`, `horizon_breadcrumb`, `materialization` (counters), `attention`, `context_candidates`, `capability_reasons`, and horizon-scoped `affordance_ids`; `startGeneration` clears generation-scoped placements/horizons (focus identity survives, interned). Typed `WorkbenchModelError` carries the failing operation. Frozen arrays throughout.
5. **`workbench/src/index.ts`**: barrel exports for the three new modules.
6. **Tests (new)**: `focus-identity.test.ts` (4), `horizon-hoist.test.ts` (5), `horizon-materialization.test.ts` (6) — named for the `just tui-test <selector>` gates.
7. **`workbench/src/workbench-model.test.ts`** (minimal call-site adaptation, permitted by the task): `selectFocus("unit","u-42")` → `selectFocus(FocusRef.for("gauntlet","WorkItem","u-42"))` with the assertion strengthened from `toEqual({kind,id})` to `toBe(ref)`; `selectHorizon("horizon-milestone-1")` → `selectHorizon(createHorizon({root_focus: ref, generation: "gen-model-2"}))` with `horizon` identity plus `active_horizon` asserted. No T07 assertion was weakened; all 45 pre-existing tests pass unmodified otherwise.

## Teeth Falsification & Verification

### Attack 1 (plan falsifier; REQ-FOCUS-001/002, DSN-006): render one WorkItem under Plan, Current Run, Agent, and Attention and mutate only one placement-local label
- **Attack description**: place the same interned WorkItem ref in four surfaces, relabel one placement, and try to observe identity forking — either in other placements, the canonical FocusRef, or the SQLite projection rows.
- **Defense outcome**: all four placements resolve to the SAME object (`toBe` on every `placement.focus`), four distinct occurrence ids that are never Focus ids; the relabel is visible only on its placement; the ref's `human_label` stays undefined and re-minting returns the identical instance; the store still holds exactly one `u-root` row with the original title/cursor. A fork (copied state, re-minted id, or label leakage) would fail any of these assertions.
- **Test**: `src/focus-identity.test.ts` — "Attack 1 (REQ-FOCUS-001/002, DSN-006): four placements of one WorkItem resolve to one FocusRef; mutating one placement-local label never forks identity".
- **Result**: **PASS**.

### Attack 2 (plan falsifier; REQ-FOCUS-003/004): hoist to a leaf Focus and inspect queries/subscriptions/context candidates
- **Attack description**: hoist to leaf WorkItem `u-root` (declared `has_evidence` depth 1) and check whether the working world stayed whole-Case (eager) or failed to restore; also attack with declared queries `[]` to see whether mandatory closure gets dropped by narrowing.
- **Defense outcome**: hoisted working set is exactly `{u-root, ev-1, ev-2}` (whole Case is 11 foci) and every counter shrinks (subscriptions 11→3, queries 3→2, prefetches 11→3, candidates shrink); candidates exclude `u-other`/`u-far`/`ev-3`/sessions; with zero declared queries the mandatory evidence/reference/governance closure is still present; de-hoist restores all 11 foci as the SAME interned instances with source rows `toEqual` before. An eager implementation or broken narrowing fails the exact-membership or counter-diff assertions (negative control in `horizon-materialization.test.ts` test 1).
- **Tests**: `src/horizon-hoist.test.ts` — "Attack 2 (REQ-FOCUS-003/004)…"; `src/horizon-materialization.test.ts` — "Negative control: counters are real…".
- **Result**: **PASS**.

### Additional preregistered expectations covered
- Cycle handling (prereg blindspot): hoisting into the `s-a ↔ s-b` cycle at depth 50 terminates with a finite 2-focus working set (`horizon-hoist.test.ts`).
- Transfer/replay (prereg L1): three hoist/de-hoist cycles across two root kinds show zero identity churn; every observed ref is the interned instance (`horizon-materialization.test.ts`).
- Parametric variation (prereg L2): different leaf foci narrow to their own declared relations; saved horizons persist noncanonically (`horizon-hoist.test.ts`).

## Gate Results

| Gate Command | Result | Notes |
| :--- | :--- | :--- |
| `just prereg-check gauntlet-tui-prime-workbench-T08` | PASS (exit 0) | Hash matches `8456810e6601061d3060c3e8cce224c67a52c3174cb938cff7eb7df4002b1958`; prereg untouched |
| `just tui-test focus-identity` | PASS (exit 0) | 4 pass / 0 fail, 57 expect() calls |
| `just tui-test horizon-hoist` | PASS (exit 0) | 5 pass / 0 fail, 69 expect() calls |
| `just tui-test horizon-materialization` | PASS (exit 0) | 6 pass / 0 fail, 126 expect() calls |
| `just tui-check` | PASS (exit 0) | `tsc --noEmit --strict`, 0 errors (caught and fixed one real type weakness in a test helper during the run) |
| `just tui-lint` | PASS (exit 0) | `biome check` — T08 files: 0 errors, 0 warnings, 22 infos (same info class as the pre-existing tree). Baseline verified: clean HEAD also exits 0 |
| `just tui-preflight bootstrap` | PASS (exit 0) | Skeleton, pinned deps, bun.lock in sync, bun audit clean, spec/DESIGN digests match |
| `cd workbench && bun test` | PASS (exit 0) | **60 pass / 0 fail** across 15 files (45 pre-existing + 15 new), 630 expect() calls |

No test was disabled, weakened, or skipped to reach green. No runtime or dev dependency was added. No frozen document, status anchor, justfile recipe, or Rust code was touched; changed/added files are exactly: `src/focus.ts`, `src/relations.ts`, `src/horizon.ts`, `src/workbench-model.ts`, `src/index.ts`, `src/workbench-model.test.ts` (call-site adaptation only), and the three new test files.

## Known Limitations (recorded honestly)

1. `human_label` on FocusRef is set only at mint time by an owning source and is intentionally NOT derived from projection rows (first-minter ordering would make labels non-deterministic across suites); display labels in this model are placement-local. A future canonical-label source can call `FocusRef.for` with the label at first mint.
2. Attention currently covers escalation-shaped attention (blocked Units). Budget and source-health attention classes named in REQ-UX-002 have no rows in the projection store yet and are deferred to the task that adds those sources.
3. REQ-GOAL-006's "current actions" dimension and REQ-OPS-004's critic allowlist mechanics belong to T09/T12 surfaces; T08 proves the context/data shaping they will consume, not the resolver itself.
4. Saved Horizons persist root/filters/views-as-relation-queries but layout preferences do not exist yet (no layout state in the model); this is within REQ-FOCUS-005's MAY.
5. Counters are computed from real store rows over in-memory projections; they measure the plan the host would execute (subscription keys, scoped queries, prefetch keys), not wall-clock SQLite timings — an execution-layer benchmark is out of T08 scope.

Confirmation: BUILDER evidence pending peer confirmation — all declared gates exit 0 and both plan teeth behave as specified; settlement authority rests with the orchestrator's evidence review, not this report.

## Correction Round 1 (peer findings)

Peer verdict on round 0: CONFIRM with 2 MAJOR findings and 1 minor taken now. All fixes landed RED-first; original sections above are unchanged.

### MAJOR-1 — relations.ts: relations attributed link endpoints from the CLAIMING row's source (identity fork across sources)

- **Finding**: the `has_evidence` edge minted its unit endpoint with the EVIDENCE row's `source_id` (not the owning unit row's source) and `valid` compared `unit_id` only, ignoring source. In a multi-source world (the T06 store admits cross-source events in one generation via `applyEvent`), one underlying unit got two identities (`focus:<evidence-source>:WorkItem:<id>` alongside the true `focus:<unit-source>:WorkItem:<id>`) with `valid: true`. The same defect class existed one line away in the `child_session` parent endpoint (attributed from the child's source) and was fixed under this finding.
- **Fix** (`workbench/src/relations.ts`): new `attributedSource` rule — the endpoint is attributed from the OWNING row's source when exactly one row resolves the claimed id; the edge is `valid` only when that owner equals the claiming row's source (source disagreement ⇒ invalid, nothing silently picked). Dangling or multi-owner claims keep the claiming row's source as a claim with `valid: false` (inspectable, never trusted by traversal). Note: `unit_id`/`session_id` are the store's PRIMARY KEY, so an id can never belong to two sources at once in the current store; the multi-owner branch is defensive. Provenance still reflects the row that asserted the link.
- **New tests** (`src/horizon-materialization.test.ts`, "T08 Correction Round 1"): (1) unit owned by `gauntlet` + evidence row claiming `prime` on the same `unit_id` ⇒ edge endpoint `toBe(FocusRef.for("gauntlet","WorkItem","u-shared"))`, `valid === false`, and exactly one unit identity across all `has_evidence` endpoints; (2) session parent owned by `prime` claimed by a `gauntlet` child ⇒ parent endpoint attributed to `prime`, `valid === false`, plus a same-source control pair asserted `valid === true`.
- **RED-first verification**: failed before the fix with the final test text — verified twice: (a) in the initial RED run (60 pass / 5 fail) before any fix landed, and (b) after the tests were corrected (the first drafts accidentally used a helper that pinned `source_id: "gauntlet"`, which would have defeated the scenario — fixed in the tests), by temporarily reverting `attributedSource` to the pre-fix claim-based behavior (`TEMP-RED` marker), re-running (all 5 correction tests RED), then restoring the fix. Pre-fix failure mode: endpoint was `focus:prime:WorkItem:u-shared` and `valid` was `true`.

### MAJOR-2 — workbench-model.ts: focus fallback minted with `defaultSource` (focus_changed forked cross-source identity)

- **Finding**: `focusFromStoreProjection` minted `FocusRef.for(defaultSource, kind, id)` for provenance-free `focus_changed` rows, so a unit owned by `prime` plus a focus snapshot forked identity (`focus:gauntlet:WorkItem:u-y` vs the true `focus:prime:WorkItem:u-y`). The accompanying code comment was also false.
- **Fix** (`workbench/src/workbench-model.ts`, smallest sound option — in-model resolution, no store schema change): focus_changed frames are attributed at ADMISSION time via `attributeStoreFocus`, which maps the payload kind (`unit`/`session`/`evidence` → `WorkItem`/`Agent`/`Evidence`) and resolves the referenced id against current projected rows; exactly one row ⇒ the ref is minted with that row's `source_id` and cached in `storeFocusRef` (used as the snapshot focus fallback). Frames that cannot be attributed (dangling id, kind with no projected row source, malformed payload) are REFUSED before the store write with a typed result `{ applied: false, reason: "unattributable_focus_change: …" }` — never minted with a default. `storeWouldApply` mirrors the store's admission gates so only frames the store would apply reach attribution. `focusFromStoreProjection` and the `defaultSource` option were deleted (the option was T08-local and unused elsewhere); the false comment is gone, replaced by the attribution contract. The store itself (T06 surface) is untouched.
- **New tests** (`src/focus-identity.test.ts`, "T08 Correction Round 1"): (1) `prime`-owned unit + `prime` focus_changed ⇒ `snapshot.focus` is the SAME interned identity (`toBe(FocusRef.for("prime","WorkItem","u-y"))`) and equals the materialization working-set instance — no fork across selection and materialization; (2) refusal scenarios: dangling id, unsupported kind (`file`), and malformed payload each return `applied: false` with `unattributable_focus_change`, leave `snapshot.focus` null (no default mint), and do not poison the model (a well-attributed frame afterwards still applies and resolves to the `prime` identity).
- **RED-first verification**: same two-stage protocol as MAJOR-1. Pre-fix failure mode: the attribution test received `focus:gauntlet:WorkItem:u-y` (default mint), and the refusal test received `applied: true` for every frame. Scenario correction recorded honestly: the first draft's "ambiguous two-source unit" case is impossible in this store (`unit_id` is the primary key — a cross-source `unit_updated` REPLACES the row), so it was replaced with the real unattributable classes above.

### MINOR — selectHorizon lacked boundary validation (fabricated horizon starved subscribers via the blanket catch)

- **Finding**: `selectHorizon` accepted any object; a fabricated horizon crashed inside snapshot assembly (`planMaterialization` → `root_focus` undefined), where `notifySubscribers`' blanket catch swallowed the error — the API call "succeeded" while subscribers silently starved.
- **Fix**: new exported `isHorizon` structural guard in `workbench/src/horizon.ts` (id/generation strings, `root_focus instanceof FocusRef`, well-formed `relation_queries`); `selectHorizon` (and, same boundary class, `saveHorizon`) now reject non-Horizon values at the API with the typed `WorkbenchModelError`.
- **New test** (`src/horizon-hoist.test.ts`, "T08 Correction Round 1"): a fabricated horizon passed to `selectHorizon` throws `WorkbenchModelError`, and subscribers are still notified by the next valid interaction (`notifications >= 2`).
- **RED-first verification**: pre-fix, `selectHorizon(fabricated)` did not throw (failure surfaced only as starved listeners) — RED confirmed in both the initial run and the temporary-reversion run.

### Trigger-record only (not fixed in this round, per peer instruction)

- `horizon.ts` governanceScope whole-Case fallback lets far units' authority text enter hoisted context candidates for non-WorkItem roots — re-scope when a governance projection lands (T12).
- `focus.ts` id grammar joins the triple with unescaped `:` and could collide distinct triples if source/kind ever become free text — re-visit the id grammar if so.

### Correction Round 1 Gate Results

| Gate Command | Result | Notes |
| :--- | :--- | :--- |
| `just tui-test focus-identity` | PASS (exit 0) | 6 pass / 0 fail (4 original + 2 MAJOR-2) |
| `just tui-test horizon-hoist` | PASS (exit 0) | 6 pass / 0 fail (5 original + 1 MINOR) |
| `just tui-test horizon-materialization` | PASS (exit 0) | 8 pass / 0 fail (6 original + 2 MAJOR-1) |
| `just tui-check` | PASS (exit 0) | caught and fixed 2 test-only type errors during the round (undefined-vs-null `toBe` overload; `Horizon` imported from the wrong module) |
| `just tui-lint` | PASS (exit 0) | 10 warnings / 167 infos — warning count equals the clean-HEAD baseline; all infos are the pre-existing `useLiteralKeys` class; a new unused-parameter warning introduced mid-round was removed |
| `just tui-preflight bootstrap` | PASS (exit 0) | unchanged |
| `just prereg-check gauntlet-tui-prime-workbench-T08` | PASS (exit 0) | prereg still byte-identical (`8456810e…b1958`) |
| `cd workbench && bun test` | PASS (exit 0) | **65 pass / 0 fail** across 15 files (60 round-0 + 5 correction), 655 expect() calls |

No existing test was weakened or removed to reach green; the two MAJOR-1 tests and both MAJOR-2 tests were corrected only to fix defects in the tests themselves (a source-pinning helper; an impossible-by-PK ambiguity scenario), each re-verified RED against temporarily reverted fixes before going green. No dependency, frozen document, status anchor, justfile recipe, or Rust code was touched.

Confirmation: BUILDER evidence (Correction Round 1) pending peer confirmation — the two MAJOR findings and the minor are closed with RED-first regression tests; settlement authority remains with the orchestrator's evidence review.
