# TUI GUI-grade redesign — substrate characterization (consolidated)

Date: 2026-09-11. Branch: `task26-dcritic-apparatus-repair`.
Inputs: `recon/shell-render-opentui.md`, `recon/interaction-commands.md`, `recon/canonical-ops-prime.md`, `recon/readmodel-views-trace.md` (each anchor verified by its recon pass), plus `baseline/` captures and gate outputs.
Note: the fifth recon area (tests-run-baseline) was cut off by the session limit after writing its artifacts but before writing its report; its evidence lives in `baseline/gates-*.txt` and the 34 `baseline/*.ansi` captures (populated + cold paths at 140x45/120x40/100x30/80x24/72x20, view switching, palette, hoist, composer, SGR click/wheel no-ops, resize recovery, quit). No `workbench/src` code has changed since those gates ran, so the baseline remains current for this report.

## 1. Substrate in one page

- **Entry/renderer.** `main.ts` mounts OpenTUI core imperatively (`createCliRenderer`, alternate-screen, `useMouse:false`, on-demand draw). No React anywhere in production (`react`/`@opentui/react` installed but unimported). The shell builds a renderer-neutral `RenderedLine[]` span tree (`renderShell`, pure); `TerminalShellRenderer` then destroys and rebuilds one absolutely-positioned `TextRenderable` per row inside one clipped `BoxRenderable` every draw.
- **Shell/surfaces.** Single header line (status · run · identity · breadcrumb · attention count, tail-clipped), Navigator outline (Full/Wide only), Primary view surface with line-offset scroll, Inspector (Full/Wide only, no scroll), Composer footer strip, ≤3 attention rows, 1–2 help rows, palette overlay (the only overlay). `Bottom` exists only as a `SurfaceId` member. Fixed pane widths per width class (Wide/Full/Compact/BelowMinimum); no resize/collapse/tabs.
- **Focus/keyboard.** Input precedence: below-minimum → takeover → composer → `handleKey` (pure reducer → `ShellEffect[]` → host callbacks). Bindings: `1/2/3` views, tab/shift+tab surfaces, up/down (navigator move / primary scroll), enter (select), `h/u` hoist/dehoist, `e` composer, `p` palette, esc dismissal, `q` quit. Alt chords dropped; PageUp/PageDown unmapped; Home/End unhandled.
- **Palette.** Entries = view availability + `materializeAffordances` (with reasons) + Compact focus rows. Staged double-Enter confirm revalidated against fresh entries → `invoke_affordance` → `OperationGateway`. No text filtering; `p` closes it.
- **Command dispatch.** Three disjoint identity spaces today: raw key strings (state changes applied directly), canonical `OperationId`s (`unit.resolve`, `unit.retry`, `state.refresh` — the only consequential ops that exist end to end), and planned declarations (`prime.launch/message/steer/cancel`, `evidence.admit`, `prime.session`) that are not invocable. No semantic command registry, no keymap resolver, no command envelope.
- **State ownership.** `WorkbenchModel` over `projection-store` (SQLite) with generation/epoch/sequence stale-frame rejection; `LiveStreamClient` reconcile/ingest/resync; `CanonicalSurfaceStreamPort` polls `/state` + `/capabilities` + `/inspection` every 2 s. Snapshot → `ShellSession.render()` → renderer; renderer is non-authority (pinned by test). Shell keeps a shadow `location` (Focus/Horizon) that can outlive a model resync.
- **Mouse.** Disabled (`useMouse:false`); no `onMouse*`, no hit-testing, no hover, no wheel. OpenTUI 0.5.11 ships the full mouse/scroll/focus/overlay apparatus unused (inventory in `recon/shell-render-opentui.md` §9, incl. `ScrollBoxRenderable`, `ScrollBarRenderable`, zIndex, `hitTest`, focusable renderables, Input/Select/TabSelect, Markdown/Diff/Code, embedded terminal, React binding).
- **Trace.** `trace_records` = in-memory, one row per applied frame (4 event kinds), reset per generation; rendered as a flat unbounded dump. RealityTrace (SXR, `sxr-adapter.ts`) is a complete typed consequence-ledger adapter (identity, lens, bindings, verification path, offer/admission gate, frame builder) that production never calls — its only live seam is a dormant Inspector section. The two sources stay distinct by design (conflict C1, resolved in `1d4d46e`: one ledger, two labelled sources, never merged).
- **Prime.** `RealPrimeHost` + `PrimeDelegationDispatcher` constructed only when `prime_binary` is set; never invoked from any production input path. No client for Prime's host JSON-lines protocol; the one-shot `-p --no-tools` child has no REPL. Lifecycle independence (close ≠ stop, qualified manifests, reattach-from-records) is real and tested; but the TUI-side watcher authors the result contract itself (exit 0 ⇒ pass) — a violation to fix, not preserve.
- **Chat.** Does not exist: `enableChat:false`; when enabled the model synthesizes `system` chat rows from `unit_updated` events (Message↔Event collapse, forbidden); composer submit → honest "unsupported / not sent" notice. No Orchestrator conversation, no Thread entity anywhere in `crates/`.
- **Rust surface.** `gauntlet serve` (surfaces feature): loopback bind, optional token auth, `/state` `/capabilities` `/refresh` `/escalated` `/runs/{id}` `/units/{id}`(+`/inspection`,`/resolve`,`/retry`). Only resolve/retry are consequential. Wire defects W1–W4 (see §4 V-items).
- **Tests.** 697 pass / 22 skip / 0 fail / 8044 assertions across 73 files (baseline `gates-tui-test.txt`); `tui-check` clean; `tui-lint` exits 1 on 6 pre-existing findings in untouched files. Renderer-boundary, PTY journey, keyboard-a11y, cell-fit, palette-confirm, deny-bypass suites are the behavioral oracle for the redesign.

## 2. EARNED SUBSTRATE — preserve (consolidated)

Full per-area tables: `shell-render-opentui.md` §8.1 (P1–P24), `interaction-commands.md` (P1–P18), `canonical-ops-prime.md` §7 (1–15), `readmodel-views-trace.md` §8 (preserve list). Load-bearing set:

1. **Terminal/mount discipline** — alternate-screen, workbench-owned ctrl+c/signals, destroy-in-finally, non-TTY headless frame, resize keeps semantic location, below-minimum prompt with recovery.
2. **Input precedence** — below-min → takeover → composer → shell; takeover owns input; paste is text-only and never a command; escape never cancels admitted work.
3. **Pure interaction core** — `handleKey(state,snapshot,key) → {state,effects}`; `ShellEffect` + `applyShellEffects` as the shell's single outbound edge (statically pinned); palette staged double-Enter with fresh revalidation; grapheme-aware composer editing; takeover queue machine.
4. **Canonical operation path** — `OperationGateway` (fresh fail-closed observe every drive, idempotency ledger, serialized decisions, `outcome_unknown` never re-keyed); `HttpOperatorActionRoute` (operator/bearer headers, refuses fabricated acceptance); registry mirrors the Rust surface exactly; planned ops kept separate with versioned handshake.
5. **Read model** — generation/epoch/sequence stale-frame rejection (store + model + stream + fold coordinator); atomic cursor/row commits; `immutableHistoryView` + version caches; FocusRef interning + generation-scoped placements; `deriveRelations` with provenance; headless log-vs-store parity.
6. **Projection honesty** — provider-gated views with inspectable reasons; DSN-015 roadmap guard; `materializeAffordances` DSN-009 placement; canonical refusal vocabulary in `resolveAffordance`; capability facts literal-false until proven.
7. **RealityTrace adapter** — typed identities, strict parsers, bounded refresh, lens/bindings/path with explicit missing links, offer → admission gate → authoritative `evidence_added` frame, `establishes_*` literal false, source-typed settlement.
8. **Prime substrate honesty** — spawn admission ≠ child completion (type level); communication ≠ completion; close/detach issues no cancel; qualified manifests; reattach from records only; spawn isolation (0700 dirs, env strip); bounded context envelope; three-way substrate provenance.
9. **Presentation tokens** — semantic token table with monochrome/256/truecolor lowering, DSN-002 discipline, gold reserved; geometry floors 80/24/120/40; cell-width fit + control sanitation (CJK/emoji/ANSI-safe).
10. **Test oracle** — the suites listed in §1 Tests; every preserved behavior above has a pin.

## 3. PRESENTATION — replace/redesign (consolidated)

1. **Render pipeline** — destroy-and-rebuild absolute TextRenderables per draw → retained component tree (OpenTUI boxes/ScrollBox/zIndex), stable identities for mouse/focus/scroll hit-testing.
2. **Chrome** — one concatenated header line → structured regions (identity, health, attention, breadcrumb, menus, back/forward) that survive width; real Case identity instead of hardcoded literals.
3. **Tabs/panes** — `PRIMARY [VIEW]` title → tab strip (keep `1/2/3` shortcuts); fixed panes → draggable boundaries, collapse, maximize; a real Bottom surface (tabs + hide) instead of the phantom `SurfaceId`.
4. **Inspector** — flattened text → scrollable sectioned region with EMPTY/FOLLOW_SELECTION/PINNED machine and records (not just lines) so SXR path steps reach the screen.
5. **Overlays** — palette-only → overlay layer: palette with query/filter + command ids + shortcuts, help, attention, dialogs (staged confirm becomes a dialog), context menus, popovers, tooltips, toasts (notice leaves the attention strip — Cue ≠ AttentionItem).
6. **Scroll** — unbounded line-offset → bounded ScrollRegions per surface; Navigator viewport follows selection; wheel scrolls region under pointer; TraceStream LIVE_FOLLOW/INSPECTION_HOLD/unseen-count/load-older.
7. **Trace presentation** — flat dump → source-typed ledger (Gauntlet history always; RealityTrace when bound), compact primary rows, visually legible kinds, selection-driven Inspector, stable identity/order from the model, plugin `trace.row` slot composed.
8. **Views' text composition** — Work/Evidence/Agents/Attention rows → record-like objects where useful; ActionControls with HIDDEN/VISIBLE_DISABLED(+reason)/VISIBLE_ENABLED/BUSY_SUBMISSION instead of text bars; composer strip → SUBMITTING draft state (Enter no longer clears before outcome).
9. **Colour/motion** — fg-only spans → surface/background/border tokens per DESIGN depth levels; capability detection (`renderer.capabilities`, 256-color path); operator-reachable reduced-motion setting; legacy T00A preview (`preview.ts`) and legacy glyph vocabulary retired from the production path.
10. **Chat presentation** — "disabled by host" placeholder → communication projection surface (records only; no fabricated threads), with composer destination/mode switching and honest unsupported states.

## 4. VIOLATIONS — must change regardless of presentation (consolidated)

From `shell-render-opentui.md` §8.3 (V1–V8), `interaction-commands.md` (V1–V9), `canonical-ops-prime.md` §7.3 (1–9), `readmodel-views-trace.md` §8 (violates):

- **Responsive contract:** rows<40 at ≥120 cols wrongly collapses to Compact (V1); Compact drops Navigator/Inspector entirely (V2); 80-col header clips breadcrumb + attention count (V3); Navigator focus ring can leave the viewport (V4); Inspector overflow unreachable (V5).
- **Interaction contract:** palette invokes affordances of any status, including disabled ones (no `statusPermitsInvocation` check); shell shadow `location` outlives model resync; composer Enter clears the draft before any outcome; takeover Enter completes locally before canonical result (refused answers cannot be re-answered); host notice drops `reason_codes`/descriptor; palette rows lack command id/shortcut; shortcuts map straight to state transitions; PageUp/PageDown leak raw sequences.
- **Colour/capability:** no terminal capability detection; 256-color never selected (DSN-002/DSN-007 adjacent).
- **Wire defects (workbench ↔ Rust):** W1 flat refusal-code read against nested `{error:{code,message}}` (every real refusal becomes `surface_error`); W2 raw `text/plain` note body against Rust's JSON-object-only parser; W3 `/state` fields expected but never served (null-unit tickets make resolve over-available; budget/governance gates can never fire); W4 test stubs encode the assumed wire, not the Rust wire — the suite cannot see W1–W3.
- **Semantic identity:** chat rows synthesized from `unit_updated` (Message≠Event); `host.notice` rendered as an `attention.required` row (Cue≠AttentionItem); trace rows are polled snapshot diffs presented as occurrences; trace history silently lost on resync with no unavailability label (REQ-SYNC-003); Inspector PROVENANCE shows latest cursor, not the row's producing cursor; `trace`/`chat` record-kind misuse.
- **Prime honesty:** host-authored result contract (exit 0 ⇒ pass) breaks spawn-admission≠completion; delegation spawn rides an accepted unit.resolve/retry commit; simulated Python kernel exported as a production module; draft "Jupyter" wording (fixed in specs, C2).

## 5. MISSING — required by the new contracts (consolidated)

Priority-ordered for implementation planning (full lists in recon §8.4 / missing sections):

1. **Semantic command layer** — registry + envelope + keymap resolver so keys, palette, menus, context menus, and mouse converge on one command identity (ICM §4/§34/§48). This unblocks everything else.
2. **Mouse** — enable `useMouse`, hit-map from stable element ids (Navigator rows, palette rows, work affordances, trace rows already have ids), gestures → intent → same reducer edge as keys.
3. **Chrome apparatus** — menus, back/forward over `formatLocation` routes, tabs, Bottom surface, pane resize, Inspector pin, dialogs/context menus/popovers/tooltips/toasts.
4. **ActionControl states + structured reason grammar** (class, summary, related_focus, next_affordance) — also fixes the V-item notice flattening.
5. **TraceStream** — follow/hold/unseen/load-older/selection; durable pageable history (port declares it unbuilt — presentation builds against the model's bounded view first, honestly labelled).
6. **RealityTrace production binding** — refresh off the render loop, refresh-epoch staleness tags, pending/unverified lanes, evidence/trace surface exposure (adapters exist; wiring does not).
7. **Chat/Orchestrator** — communication transport absent (host reports unsupported); keep honest-disabled until a canonical path exists; composer mode/destination switching in production; production takeover sources.
8. **Canonical ops beyond resolve/retry** — case.*/thread.*/wayfinder.*/reference.*/run.*/verification.*/evidence.*/attention.acknowledge respond — all absent-with-reason, never fabricated (conflicts C3–C6, C10 resolved).
9. **Small-but-required** — PageUp/PageDown/Home/End; colour capability detection; reduced-motion setting; capability_reasons population; `search.global`.

## 6. What already satisfies the new contracts

Provider-gated views with reasons; DSN-015 guard; palette two-step confirm through the gateway; renderer non-authority with pinned single outbound edge; generation/epoch stale-frame rejection; geometry floors; below-minimum prompt; monochrome token discipline; honest unsupported submissions; close-TUI-never-stops-work; RealityTrace admission chain (when wired); headless parity. These are the seams the redesign extends rather than replaces.

## 7. Redesign entry conditions (next step)

Conflicts C1–C8, C10 resolved (C9 standing: DESIGN.md + spec frozen). Characterization complete. Next per the operator's task: Hallmark `redesign` pass over presentation/interaction projection (visual hierarchy, shell, panes, tabs, menus, objects, spacing, mouse discoverability, trace presentation, evidence objects, overlays, responsive layout) with the semantic architecture fixed by the governing artifacts — then incremental implementation in the order recommended by the task (command layer → shell primitives → mouse convergence → chrome → surfaces → responsive → equivalence + adversarial verification), committing after every meaningful change.
