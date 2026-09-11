# Hallmark redesign — GUI-grade presentation & interaction design

Date: 2026-09-11. Verb: `hallmark redesign` over the whole TUI app shell.
System-managed project: root `DESIGN.md` is the locked design system (sha256-frozen; C9 standing). This document **designs within** DESIGN.md + ICM + UESC; it invents no View, Surface, entity, threshold, or capability. Where DESIGN.md already decides (tokens, gold discipline, record shapes, floors), it is referenced, never overridden. This file governs implementation choices DESIGN.md leaves open: chrome anatomy, tab/menu/dialog composition, mouse gesture map, density, and the presentation order of the convergence work.

Hallmark stamp: `/* Hallmark · genre: system-managed institutional control plane (DESIGN.md) · macrostructure: Workbench · theme: DESIGN.md locked (no rotation) · designed-as-app */`
Pre-emit critique: P5 H5 E4 S5 R5 V4 (Specificity held to anchors; Variety marked 4 because system-managed consistency intentionally caps divergence — the variety budget lives in component archetypes, not themes).

Inputs: DESIGN.md (full), ICM/UESC drafts (post-`16e90a9`), `02-substrate-characterization.md`, 34 baseline captures.

---

## 1. What is being redesigned, in one sentence

Replace the single-header + three-text-columns terminal chrome with a **retained-tree application shell** — chrome bar, tab strip, three columns, collapsible bottom, composer, overlay layer — in which every visible control is a projection of a semantic command, and mouse and keyboard are two representations of one intent path.

The invariant (task contract, ICM §0/§4): `gesture → intent → semantic command → affordance revalidation → owning operation → authoritative/projection update → snapshot → renderer`. No `button.onClick() → mutation` anywhere.

## 2. Shell anatomy (Wide ≥140, Full 120–139 × ≥40 rows)

Row budget at 120x40 (normative floor) — every region earns its rows:

```text
┌─ chrome ─────────────────────────────────────────────────────────────────────┐ 1 row
│ ◆ Gauntlet │ journey-integrity · ref 1d457ec9 │ Case › R17 › T23 │ ‹ › │ ● run │ ! 2 │ File View Run Agents Help │
├─ columns ────────────────────────────────────────────────────────────────────┤
│ NAVIGATOR 28 │ PRIMARY ╭─[Work][Chat][Trace]─╮              │ INSPECTOR 36  │
│ Case        │         │ view content rows    │              │ IDENTITY      │
│ └ Run R17   │         │ (ScrollBox)          │              │ STATE         │
│   └ T23 ●   │         │                      │              │ RELATIONS     │
│     ├ ≈agents│        │                      │              │ EVIDENCE      │
│ ▾ Evidence  │         │                      │              │ PROVENANCE    │
│   └ EV-991 ◆│         │                      │              │ AFFORDANCES   │
├─ bottom (collapsed → 1 tab row; open → 6–10 rows, drag boundary) ────────────┤
│ [Terminal][Kernel][Agents][Evidence][Problems][Changes]        ⌃ hide        │
├─ composer ───────────────────────────────────────────────────────────────────┤
│ @T23 message › draft…                                           mode · dest │
└──────────────────────────────────────────────────────────────────────────────┘
```

Composition rules:

- **Chrome bar (1 row, always).** Structured regions with fixed priority so nothing load-bearing clips: identity (`◆ Gauntlet`) → Case identity (workspace·ref) → breadcrumb (Horizon path, clickable segments) → back/forward (`‹ ›`, disabled-with-reason at history bounds) → health (`● run R17` / connection state) → attention (`! N`, opens Attention overlay) → menu bar (right-aligned). At 80 cols the order degrades **from the right**: menus fold into palette (`:` hint), health shortens, breadcrumb never disappears (DESIGN Compact: "Preserve breadcrumbs/Horizon at all times").
- **Columns.** Navigator 28 / Primary ≥ rest / Inspector 36 at Full; 32/≥40/44 at Wide. Single-line slate borders between columns (DSN: borders only where they clarify containment — three columns, one hairline each). Focused surface gets a `focus.ring` structural edge on its border (not a glow, not a `▸` prefix — the prefix dies).
- **Primary tab strip.** `[Work][Chat][Trace]` always (required primaries); promotable views appear as tabs **only when their provider exists** (REQ-AFF-004: absent-with-reason, never a dead tab). Keep `1/2/3` shortcuts. Tabs have close controls only for promotable views; closing a tab is presentation-only (never stops a Run/Agent — adversarial tooth).
- **Inspector.** Sectioned, scrollable, `FOLLOW_SELECTION` default with explicit pin toggle; records (not flattened lines) so RealityTrace path steps (ordinal+entry_hash) render. Sections per DESIGN: Identity/State/Relations/Authority/Evidence/Provenance/Affordances.
- **Bottom surface.** Tab row always visible at Full/Wide when ≥44 rows; below that it collapses to a single entry row (tab row hidden, `b` or click toggles). Draggable top boundary (mouse) with `ui.pane.resize` command equivalence (keyboard: `Ctrl+arrow` growth step, recorded in ICM §36.1 terms).
- **Composer.** Mode + destination always visible (existing earned behavior P23), draft, SUBMITTING state (Enter no longer clears before outcome — fixes V3), takeover rendering per existing machine.
- **Attention strip → toast + overlay.** `host.notice` becomes a toast (bottom-right, auto-expire, Cue ≠ AttentionItem); canonical AttentionItems get the `! N` chrome affordance + overlay list. The 3-row strip over the composer is retired.

## 3. Overlay layer (z-ordered, `surface.overlay` + border)

| Overlay | Trigger | Behavior |
|---|---|---|
| Command palette | `p` / chrome | Query filter (typed text), grouped results (Views / Actions / Focus / Recent locations), each row: title · semantic command id · shortcut · status(+reason when blocked). Staged double-Enter confirm kept (earned P9) but rendered as a confirm dialog, not an inline row. |
| Context menu | right-click / `m` | Items generated from the clicked object's affordance set (same resolver as palette/Actions). Disabled entries show reason inline. Position: clamp to viewport; Esc/click-away dismiss. |
| Help overlay | `?` / Help menu | Keyboard map (from the keymap registry — single source), pointer-equivalents column, current keymap name. |
| Attention overlay | `! N` click / `A` | Ranked AttentionItems, `attention.open` → focus + inspector, respond flows route into composer takeover. |
| Dialog (modal) | confirm/destructive/approval | Consequence framing: operation, target, irreversibility, authority basis. Destructive uses `danger` token + typed confirmation. |
| Popover/tooltip | hover (800ms) / focus (0ms) | Lightweight secondary info: machine ids, cursors, gate reasons. Never interactive content that can't be reached another way. |
| Toast | notices | Submission lifecycle per-control (ActionControl BUSY_SUBMISSION) + gateway outcomes with full reason grammar. Auto-expire, never steal focus. |

All overlays: explicit border, dismissible (Esc = dismissal only, never cancels admitted work), input-capturing only when required (palette/dialog; menus capture arrow nav).

## 4. Mouse gesture map (ICM §35) — every gesture converges on a command

| Gesture | Region | Resolves to |
|---|---|---|
| single click | row/object/tab/menu item | `nav.focus.select` / `ui.tab.select` / menu command |
| double click / Enter | Navigator row, trace row, evidence record | `nav.focus.open` (new — distinct from select) |
| right click | any object | `ui.context_menu.open` for that object's placements |
| wheel | any scroll region | that region's scroll (region under pointer, never the focused one) |
| drag | pane boundary / tab | `ui.pane.resize` / tab reorder |
| hover | rows, controls | tooltip popover (read-only) |
| click `‹ ›` | chrome | `nav.history.back/forward` |
| click breadcrumb segment | chrome | location resolution (hoist to that horizon) |
| click `! N` | chrome | Attention overlay |

Implementation: `useMouse: true`; hit-map from **stable element ids** (already exist: Navigator `row.id`, `palette.${i}`, `work.affordance.${op}`, `trace.${trace_id}`; tabs/menus mint ids in the same grammar); gestures lower to the same `handleKey`-equivalent intent reducer — there is exactly one reducer, keyed by command id, not two input pipelines.

## 5. Object presentation

- **Work rows** (Operational/Speed posture): compact tabular rows, glyph cue column (`● ○ ! × ? ◷ ◆` per DESIGN family), label, machine id (dim, tabular), status label. Not cards.
- **Evidence/Authority records** (Review posture): DESIGN's exact record shape (bordered `┌─ VERIFICATION ─…` object, claim/verifier/result/identity/provenance/freshness), negative space around them, gold **only** for `◆ independently confirmed` / sanctioned authority. No card grids.
- **Agent rows**: capacity framing (`builder-03 RUNNING / T23 retry implementation / parent: orchestrator`), never chat presence.
- **Trace ledger**: source-typed rows — `G` lane for Gauntlet execution history (always), `R` lane for RealityTrace records (when bound), visually distinct provenance labels, never merged. Compact primary row: `ordinal/kind/summary/target`; Inspector carries relations + payload detail. Follow/hold: live-follow while at tail; scroll-up suspends (INSPECTION_HOLD) with `N new ▾` return-to-live affordance; selection preserved across arrivals; load-older at the head with anchor stability.
- **Run formation**: readiness panel in Work view grounded in real gate facts (`readiness.ready/degraded/blocked` tokens + reasons) — no invented percentages (Honest-copy discipline).

## 6. Tokens & capability (extends theme.ts; DSN-001/002)

Add the missing families as named tokens (no component literals): `surface.workspace/panel/elevated/overlay/code/muted` (bg + border pairing), `focus.ring`, `freshness.*`, `danger`, `*.quarantined`, `authority.degraded`, `proof.sanctioned_authority`. Lowering: detect `renderer.capabilities.rgb` → truecolor/256/monochrome (fixes V6); monochrome keeps glyph+label+emphasis. Reduced-motion setting operator-reachable (config + palette `ui.motion.reduced`), defaulting to full only when unset. Retire legacy `formatStatus` glyphs (`✗ ▶ ■`) and the T00A `preview.ts` path from production (kept for its own tests or deleted with approval).

## 7. Responsive behavior (DSN-007 fixes V1/V2/V3)

| Class | Rule |
|---|---|
| Wide ≥140 | Full layout, wider columns, richer inline metadata |
| Full 120–139 | 3 columns + bottom + composer. **≥40 rows only** gates bottom visibility, never column count — 120x30 keeps 3 columns + collapsed bottom (fixes V1) |
| Compact 80–119 | Primary + tab strip; Navigator/Inspector become switchable drawers (overlay at ≤99 cols) reachable by `Ctrl+←/→`, tab-cycle, and edge click-tabs; breadcrumb + Case identity + attention + composer destination always present (fixes V2/V3) |
| Below <80x24 | Recoverable prompt (unchanged, earned P13) |

Height ladder 40→24 rows: bottom collapses first, then chrome menus fold (palette remains), then Inspector drawer-izes; Focus, View, Horizon, attention, composer destination, consequential status survive every step.

## 8. Implementation order (binds characterization §5 to steps)

1. **Semantic command layer** — registry + envelope + keymap resolver; re-key `handleKey` literals to command ids; palette rows gain id+shortcut; startup validation (no orphan bindings). *(Unblocks everything; also where W1–W3 wire fixes land: nested error parse, JSON note body, /state gate-field honesty.)*
2. **GUI shell primitives** — retained-tree renderer (boxes/ScrollBox/zIndex per OpenTUI inventory §9), surface/border tokens, capability detection.
3. **Mouse convergence** — useMouse, hit-map, gesture→command (§4).
4. **Chrome + tabs + bottom + pane resize** (§2).
5. **Overlays** — palette filter/ids, dialogs, context menus, tooltips, toasts (§3).
6. **Surfaces redesign** — Navigator (viewport-follow, expand/collapse), Chat (honest projection), Trace ledger (§5), Inspector (sections/pin/records), Work/Attention.
7. **Responsive ladder** (§7).
8. **Equivalence + adversarial verification** — mouse≡keyboard≡palette≡context-menu on representative consequential actions; stale-enabled-button, shortcut-bypass, palette-bypass, close≠stop (tab/TUI), spawn≠completion, "done"≠settlement, trace follow-hold teeth; W1–W4 regression suite against a real `gauntlet serve` wire stub.

Every step ends with `just tui-check && just tui-test` (+ targeted suites), captured ANSI evidence under `baseline/after/`, and a commit.

## 9. Anti-slop gate (run against DESIGN.md Do/Don't + task visual language)

No SaaS card grids ✓ · no fake metrics ✓ · no neon/cyberpunk ✓ · no AI-chat cliché (Chat is one tab, subordinate) ✓ · gold scarce (proof/sanction only) ✓ · status never color-alone (glyph+label always) ✓ · borders only at containment boundaries ✓ · motion restrained to state change ✓ · chrome is structural (Prussian/slate), not decorative ✓ · no re-drawn fake windows — overlays are real OpenTUI z-layers ✓. Standing check at every step: DESIGN.md's 14-question visual acceptance list.
