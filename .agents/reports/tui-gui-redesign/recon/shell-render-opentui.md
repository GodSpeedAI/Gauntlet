# Recon: shell-render-opentui

Area: production entry + mount path, shell layout/surfaces, theme/tokens, overlays, scrolling, pane sizing, width classes, below-minimum prompt, colour lowering, reduced motion; OpenTUI 0.5.11 inventory.
Mode: read-only. No source/test/spec edited. One scratchpad probe executed (in-memory model, cwd in scratchpad) — results in §7.
Draft-doc anchors: ICM md5 `7963c46d…`, UESC md5 `0b016d83…` — both files were being edited during recon (ICM §35 moved 1950→1960→2035). Cite by section number; line numbers below are from that snapshot.

---

## 1. Headline answers

| Question | Answer | Anchor |
|---|---|---|
| Is OpenTUI mounted? | YES — `createCliRenderer` (core, imperative). No `createRoot`. | workbench/src/main.ts:82-88 |
| String-built or native? | Hybrid: shell builds a renderer-neutral span tree (`RenderedLine[]`), then `TerminalShellRenderer` lowers each row to one absolutely-positioned `TextRenderable` inside one `BoxRenderable`. | render-tree.ts:17-27; terminal-renderer.ts:57-104 |
| Is React used? | NO. Zero `react`/`@opentui/react` imports, zero `.tsx`. Deps + `jsx: react-jsx` present but unused. No component boundaries exist. | package.json:23-26; tsconfig.json:9; graft grep `from "react"` → 0 prod hits |
| Mouse enabled/decoded? | NO. `useMouse: false` (OpenTUI default is `true`); no `onMouse*` handler anywhere; no hit-testing. | main.ts:85; node_modules/@opentui/core/chunk-bun-bwjmgnxw.js:7476 |
| What drives redraw? | Imperative `draw()` = `session.resize(renderer.w,h)` + `view.render(shell())`, called on: keypress, paste, renderer `resize` event, host `changed()` (poll every 2000 ms + every interaction). Render loop never `start()`ed; OpenTUI renders on demand because `add()` → `requestRender()`. | main.ts:90-114; workbench-host.ts:121-128,224,227-249; chunk-bun-bwjmgnxw.js:1174-1213,3973 |
| What drives resize? | `renderer.on("resize", draw)`; `ShellSession.resize` only updates dimensions (semantic location untouched). | main.ts:112; shell.ts:1241-1244 |
| Redraw granularity | Whole tree destroyed and rebuilt every draw (`destroyRecursively` then N new renderables, ids `cell-x-y`). | terminal-renderer.ts:52-58,60-67,71-72 |
| Keyboard path | `renderer.keyInput` global `keypress` → `terminalKey()` string → `ShellSession.key` precedence (below-min → takeover → composer → `handleKey`) → `ShellEffect[]` → host callbacks. No OpenTUI focus system used. | main.ts:95-107; terminal-renderer.ts:16-29; shell.ts:1247-1297,473-496 |
| Headless path | Non-TTY: one static text frame to stdout, then exit. | main.ts:57-75 |
| Terminal modes | alternate-screen; `exitOnCtrlC:false`, `exitSignals:[]`; SIGINT/SIGTERM → quit; `finally` destroys renderer. | main.ts:76-88,96-99,116-123 |

## 2. Mount / data flow (as built)

```
WorkbenchHost (poll 2s, gateway, model)  --changed()-->  draw()
   host.model.getSnapshot() + observation + sessionContext
        -> ShellSession.render()  (pure: ShellState + snapshot -> RenderedShell span tree)
        -> main.ts shell(): prepend host.status/runId span to header; prepend host.notice to attention[]
        -> TerminalShellRenderer.render(): destroy old Box; new Box(w,h,overflow hidden);
           header row0; prompt rows OR columns (x-advance by surface.width, 1-cell gutter) +
           attention(<=3) + composer + help(<=2) footer; palette = bordered Box + rows
keypress/paste -> terminalKey -> ShellSession.key/paste -> effects -> host.interactions -> model / gateway
```
Anchors: main.ts:27-56,90-114; shell.ts:873-1195; terminal-renderer.ts:57-164; workbench-host.ts:227-272.

## 3. Width classes / pane sizing (implemented vs contract)

| Class | Implemented rule | Columns | Contract | Status |
|---|---|---|---|---|
| Wide | cols>=140 AND rows>=40 | nav 32 / primary max(40,rest) / insp 44 | DESIGN Responsive/Wide (1473-1485) | layout OK; row gate wrong |
| Full | cols>=120 AND rows>=40 | nav 28 / primary max(36,rest) / insp 36 | DESIGN Responsive/Full: <40 rows keep 3 columns, collapse Bottom first (1496) | VIOLATES — any rows<40 → Compact |
| Compact | else if cols>=80 AND rows>=24 | primary = cols; Nav/Insp width 0, not rendered | DESIGN Compact: Nav/Insp as switchable side surfaces/overlay (1505); ICM §38 Narrow | VIOLATES — Inspector unreachable |
| BelowMinimum | cols<80 OR rows<24 | prompt only | DESIGN Below minimum (1515-1524) | OK (resize/q; no detach offered) |
Anchors: geometry.ts:20-41,48-83,90-96. Probe: 130x30, 139x39, 200x39 → `Compact` (§7).
Pane sizes are constants; no resize, collapse, maximize (UESC §22.11, ICM §27.3 absent).

## 4. Surfaces as rendered

- ApplicationChrome: a single header line: `host.status · run <id> · Gauntlet · <workspace>/<reference> · gen <8> · <breadcrumb> · ! attention N`. Workspace strings hardcoded `"gauntlet"/"canonical surface"`. No menus, back/forward. shell.ts:883-907; main.ts:23,32-43.
- Navigator (Full/Wide only): outline rows keyed by FocusRef; highlight `◉` + bold; alias rows `≈`; no viewport follow, no expand/collapse. shell.ts:325-395,926-979.
- Primary: title `PRIMARY [VIEW]`, per-view line offset scroll with `↑ N line(s) above` marker; views switched by `1/2/3` or palette; no tab strip. shell.ts:981-1034,505-509.
- Inspector (Full/Wide only): EMPTY text or `renderInspector` sections flattened into lines; no scroll, no pin. shell.ts:1036-1083.
- Bottom: `SurfaceId` member only; never rendered. shell.ts:69.
- Composer: full-width footer strip, `min(lines, max(3,h-8))` rows; mode+destination line always first. terminal-renderer.ts:112-114,137-139; composer.ts:409-423.
- Attention strip: <=3 rows above composer; includes `host.notice` as first row. terminal-renderer.ts:115,134-136; main.ts:44-53.
- Help: key legend 1 row (2 in Compact) + optional reduced-motion line. shell.ts:849-854,1108-1127.
- Overlay: palette only — bordered `BoxRenderable` title `Palette · ↑/↓ select · Enter open · Esc dismiss`, bg `RGBA.defaultBackground()`, z-order by insertion, windowed rows. No help/attention overlay, dialog, context menu, tooltip, floating window, toast. terminal-renderer.ts:143-163.
- Focus cue: focused surface first row prefixed `▸ `; titles bold (focused) / dim (unfocused); no borders; 1-cell negative gutter. terminal-renderer.ts:120-129; shell.ts:932,1009,1038.
- Below-minimum prompt: header row 0 + 3 prompt rows from row 2; only `q` (and ctrl+c) accepted. terminal-renderer.ts:105-111; shell.ts:609-613,909-924.

## 5. Theme / colour / motion

- Token table (36 tokens, 10 families) with fg hex, glyph, label, emphasis; gold reserved; `resolveToken` lowers truecolor / color256 / monochrome. theme.ts:45-89,109-355,386-422.
- No surface/background/border tokens (`surface.workspace/panel/elevated/overlay/code` of DESIGN Depth levels 1289-1293). TokenSpec has fg only. theme.ts:91-100.
- Missing UESC §23 families/members: `freshness.*`, `danger.destructive`, `*.quarantined`, `authority.degraded`, `proof.sanctioned_authority` naming. theme.ts:45-89.
- Production capability choice: `NO_COLOR` set → monochrome else truecolor. `color256` never selected; `renderer.capabilities` (`rgb`) never read; `ShellState.colorCapability` never consumed by renderer. terminal-renderer.ts:83-86; shell.ts:428,449.
- Only fg colour + BOLD/DIM applied; no bg, underline, inverse. terminal-renderer.ts:92-98.
- Motion: `MotionPreference` full|reduced; `runningIndicatorFrames` used only by test helper `containsMotionFrames`; production never sets `motion` (always `full`), no config/env path. theme.ts:436-444; shell.ts:448,1197-1209; main.ts:22-26; config.ts (only env source at :97).
- Legacy `formatStatus` glyphs `✗ ▶ ■` diverge from DESIGN glyph family `× ● ?`. theme.ts:450-496 (imported by preview.ts:16).

## 6. Scrolling

- Primary only: `scroll[view] += ±1` on up/down when Primary focused; lower bound 0, NO upper bound; offset preserved across view switch and stream updates. shell.ts:775-786,1008-1025.
- Navigator: selection index moves, viewport never follows; rows sliced from top by renderer. shell.ts:764-773; terminal-renderer.ts:125.
- Inspector/Composer: up/down no-op. shell.ts:788.
- Palette: renderer computes window so selected row stays visible. terminal-renderer.ts:145-147,160-162.
- Keys: `home`/`end` mapped by `terminalKey` but unhandled; `pageup`/`pagedown` unmapped (raw `ESC[5~` leaks as key string, ignored). terminal-renderer.ts:22-28.
- No TraceStream states, unseen count, return-to-live, load-older. views-chat-trace.ts:134-157 renders every record.

## 7. Probe evidence (scratchpad, harness `seededModel`, main.ts-style header prefix)

```
geo 130x30: Compact | geo 139x39: Compact | geo 200x39: Compact
80x24  header (105 cells) row0: "connected · run run-pty · Gauntlet · gauntlet/ref-1d457ec9 · run R17 · gen gen-t"   -> breadcrumb + "! attention 1" clipped
120x40 header with 26-char run id row0 ends "· Case · ! attenti"                                                    -> attention count clipped
Navigator: shift+tab -> Navigator; 55x down -> navigatorIndex 55 of 71 lines @120x40; frame contains "◉": false   -> focus invisible
Primary: 500x down -> scroll {"work":500}; lines ["PRIMARY [WORK]","↑ 500 line(s) above (scroll anchored)"]      -> blank pane
terminalKey(pageup) -> "\x1b[5~"; composer draft unchanged
```
Production header arithmetic (no harness run span): `connected · run run-pty · ` (26) + `Gauntlet ` (9) + `· gauntlet/canonical surface` (28) + ` · gen xxxxxxxx ` (16) = 79 cells before breadcrumb → same clipping at 80 cols.

## 8. Classification

### 8.1 PRESERVE

| # | Item | Anchor | Why |
|---|---|---|---|
| P1 | Native mount config: alternate-screen, workbench-owned ctrl+c/signals, renderer destroy in finally | main.ts:76-123 | Terminal-mode restoration and quit ownership already correct; close TUI never stops work |
| P2 | Non-TTY one-shot headless frame | main.ts:57-75 | Headless equivalence path; renderer-independent |
| P3 | `terminalKey` lowering (release filtered, ctrl+, shift+tab, return/linefeed→enter) | terminal-renderer.ts:16-29 | Native parser owns escape timing/Unicode; PTY-tested |
| P4 | Bracketed paste → text only, Composer-scoped | main.ts:108-111; shell.ts:1300-1314 | Paste never executes shortcuts (REQ-UX-005 tested) |
| P5 | Pure `handleKey` + `ShellEffect` single outbound edge | shell.ts:453-496,602-838 | Deterministic, testable intent layer to re-key into command IDs |
| P6 | Existing bindings 1/2/3, tab/shift+tab, up/down, enter, h, u, p, e, esc, q | shell.ts:505-509,725-837 | ICM §34.1 earned bindings |
| P7 | Input precedence below-min → takeover → composer → shell | shell.ts:1247-1297 | Takeover owns input (REQ-TUI-004); `q` typed in composer does not quit |
| P8 | Escape = dismissal only; backs out staged confirm then palette | shell.ts:617-626,741-750 | Never cancels admitted work |
| P9 | Palette staged double-Enter, revalidated against fresh entries → `invoke_affordance` → `OperationGateway` | shell.ts:674-702; workbench-host.ts:242-272 | Canonical gateway path; no one-Enter consequential invoke |
| P10 | Palette entries = view availability + `materializeAffordances` w/ reasons + Compact Focus rows | shell.ts:539-595 | Projection over registries (ICM §28 direction) |
| P11 | Span tree carries semantic token names; `renderShell` pure | render-tree.ts:1-27; shell.ts:873-878 | Renderer non-authority (REQ-TUI-001/005) |
| P12 | Geometry constants 80/24/120/40/140 and 3-col vs 1-pane allocation | geometry.ts:20-24,48-83 | Spec floors; tested at 120x40/80x24 |
| P13 | Below-minimum prompt text + input ownership + recovery | geometry.ts:90-96; shell.ts:609-613,909-924 | DSN-007 conformant, PTY-tested grow-back |
| P14 | Resize keeps semantic location | shell.ts:1241-1244; main.ts:112 | REQ-UX-005 resize teeth |
| P15 | Cell-width fit + control sanitation per span | geometry.ts:109-295; terminal-renderer.ts:34-44 | CJK/emoji/combining/ANSI safety tested |
| P16 | Semantic token table, DSN-001 raw-colour isolation, gold discipline, `resolveToken` lowering | theme.ts:102-422 | Tested token distinctness in monochrome |
| P17 | Per-view scroll offsets preserved across switches/stream updates | shell.ts:424-425,524-530 | Stream output never steals scroll (tested) |
| P18 | DSN-015 roadmap view guard | shell.ts:148-180 | Harness View cannot be registered |
| P19 | Provider-gated lower views with inspectable reasons | shell.ts:189-224 | REQ-AFF-004 absent-with-reason |
| P20 | `formatLocation` route-like SemanticLocation | shell.ts:108-141 | Basis for back/forward + deep links |
| P21 | Navigator outline as FocusRef placements + alias rows | shell.ts:325-395 | DSN-006 one identity many placements |
| P22 | Host redraw wiring: coalesced refresh, poll, `changed()` on interactions | workbench-host.ts:121-137,224,227-249 | Snapshot → render loop independent of renderer |
| P23 | Composer strip always shows mode + destination | composer.ts:409-423 | REQ-UX-004/005 action destination at 80x24 |
| P24 | Tests: native render, PTY release entry, geometry floors, keyboard a11y, cell fit, palette confirm, renderer boundary, gated PTY journeys | terminal-renderer.test.ts:7-64; production-entry.test.ts:56-182; opentui-shell.test.ts:44-149,283-370; keyboard-accessibility.test.ts:48-211; cell-fit.test.ts:21-45; palette-grouping-confirm.test.ts:27-199; renderer-boundary.test.ts:305-357; operator-journeys-pty.test.ts:134 | Behavioural oracle for any redesign |

### 8.2 REPLACE

| # | Item | Anchor | Why |
|---|---|---|---|
| R1 | Destroy-and-rebuild whole renderable tree per draw; one absolute `TextRenderable` per row | terminal-renderer.ts:52-104 | Defeats OpenTUI retained tree/diffing; no stable component identity for mouse/focus/scroll |
| R2 | Hand-computed layout (footerTop arithmetic, x-advance, row caps) | terminal-renderer.ts:112-142 | Should be Yoga flex boxes/SplitPane; caps (attention<=3, help<=2) hide content silently |
| R3 | Focus cue `▸ ` prefix + bold/dim titles; no surface borders; 1-cell gutter | terminal-renderer.ts:120-129; shell.ts:932,1009,1038 | DESIGN Depth levels/Borders want structural edge + surface tokens |
| R4 | Palette overlay: insertion-order z, manual windowing, key hints in title string | terminal-renderer.ts:143-163 | Needs zIndex overlay layer, ScrollBox, input field, command rows |
| R5 | Header = one concatenated text line, tail-clipped | shell.ts:883-907; main.ts:32-43 | Chrome needs structured regions (identity, health, attention, breadcrumb) that survive width |
| R6 | Key legend rendered as dim text rows | shell.ts:849-854,1108-1127 | Discoverable help should be overlay/menu + shortcut hints on controls |
| R7 | `PRIMARY [VIEW]` title instead of tab strip | shell.ts:1010-1016 | UESC §7.3 tabs; keep 1/2/3 as shortcuts |
| R8 | Inspector sections flattened to text lines with blank gap rows | shell.ts:1058-1073 | Needs scrollable sectioned region, pin control |
| R9 | fg-only colour + BOLD/DIM; no bg/surface/border tokens | terminal-renderer.ts:82-99; theme.ts:91-100 | DESIGN Depth levels (surface.*) and Overlay `surface.overlay` unimplementable |
| R10 | Primary scroll unbounded (blank pane with `↑ 500 line(s) above`) | shell.ts:775-786,1017-1025 | Corrupt viewport; replace with bounded ScrollRegion |
| R11 | Legacy T00A string-built ASCII preview (own state/reducer, box art, hardcoded 127.0.0.1:4000 hint) exported from index | preview.ts:18-29,151-197; index.ts:26; geometry.ts:306-326 | Parallel presentation path not used by production entry |
| R12 | Legacy `formatStatus`/`formatEvidenceItem` glyph vocabulary `✗ ▶ ■` | theme.ts:450-511 | Diverges from DESIGN glyph family (358-381); only caller is legacy preview.ts:248-356 |

### 8.3 VIOLATES

| # | Item | Anchor | Clause | Detail |
|---|---|---|---|---|
| V1 | rows<40 at >=120 cols collapses to single-pane Compact | geometry.ts:34-40 | DESIGN Responsive/Full (1496); Height constraints (1530) | Probe 130x30/200x39 → Compact; contract keeps 3 columns, collapses Bottom first, overlay Nav/Insp last |
| V2 | Compact renders no Navigator/Inspector at all | shell.ts:511-516,1074-1083 | DESIGN Responsive/Compact (1505); ICM §38 Narrow | Inspector detail unreachable 80-119 cols; only Focus selection via palette |
| V3 | Header tail-clip drops Horizon breadcrumb (and attention count) at 80 cols | shell.ts:893-907; main.ts:36-41 | DESIGN Responsive/Compact "Preserve breadcrumbs/Horizon at all times" (1511) | Probe row0 ends `gen gen-t`; code comment 893-895 claims opposite; attention survives only in strip |
| V4 | Navigator highlighted row can leave viewport | shell.ts:936-958; terminal-renderer.ts:125 | spec REQ-UX-005 "visible focus" (1381-1382) | Probe: index 55/71 lines at 120x40, `◉` absent from frame |
| V5 | Inspector overflow unreachable: no scroll, clip without indicator | shell.ts:788; terminal-renderer.ts:125; geometry.ts:166-170 | ICM §37 static surfaces; UESC §7.2 scroll; DESIGN Responsive (1526) | Content past footerTop-2 rows is silently lost |
| V6 | Colour capability not detected; truecolor assumed unless NO_COLOR; 256 never used | terminal-renderer.ts:83-86; shell.ts:428,449 | DESIGN Responsive/Color capability (1543-1545); DSN-002 (1598) | `renderer.capabilities.rgb` ignored; SSH without truecolor gets hex fg |
| V7 | `host.notice` (submission status) rendered as attention row with `attention.required` | main.ts:44-53; terminal-renderer.ts:115,134-136 | UESC §1 Cue↔AttentionItem; UESC §22.13 Toast | Status notice occupies one of 3 attention rows, can push canonical AttentionItems off-screen |
| V8 | Palette rows lack semantic command ID and shortcut | shell.ts:1162-1176 | ICM §28 "Each result MUST expose … semantic command ID … shortcut" | View rows show `view work`; affordance rows show op+status only |

### 8.4 MISSING

| # | Capability | Clause | Detail (anchor) |
|---|---|---|---|
| M1 | Mouse input (click/double-click/right-click/wheel/drag) | ICM §35.1-35.8; UESC §24 | `useMouse:false`, no `onMouse*`, no hit-test (main.ts:85) |
| M2 | ApplicationChrome menus File/View/Run/Agents/Help + connection cue | UESC §5.2; ICM §7 | Header is one text line (shell.ts:883-907) |
| M3 | Back/forward navigation history | ICM §6.6 | ShellState has no history (shell.ts:416-429) |
| M4 | Primary tabs (Tab machine, ui.tab.*) | UESC §7.3, §22.1; ICM §27.1 | Title text only (shell.ts:1010-1016) |
| M5 | Bottom Surface (tabs, draggable boundary, hide) | UESC §7.5; ICM §27.3; DESIGN Full layout (1490-1492) | `"Bottom"` only in type (shell.ts:69) |
| M6 | Inspector FOLLOW_SELECTION/PINNED + toggle | UESC §7.4; ICM §27.4 | Only empty vs focus (shell.ts:1042-1046) |
| M7 | Overlays: help, attention, ModalDialog, ContextMenu, Popover, Tooltip, FloatingWindow, Toast | UESC §7.7, §22.2-22.7, §22.13; ICM §29, §31 | Palette is the only overlay (terminal-renderer.ts:143-163) |
| M8 | Palette query/FILTERING, recent locations | UESC §22.8; ICM §28 | Keys limited to esc/p/up/down/enter; `p` closes (shell.ts:615-723) |
| M9 | Keymap resolver → semantic command-ID registry | ICM §34 binding layer, §4, §5.1; DESIGN Input modality (1564) | Literal keys switch directly to state/effects (shell.ts:725-837) |
| M10 | PageUp/PageDown/Home/End viewport movement | UESC §24 Scroll row | pageup leaks raw `ESC[5~`; home/end unhandled (terminal-renderer.ts:22-28; shell.ts:761-789) |
| M11 | TraceStream LIVE_FOLLOW/INSPECTION_HOLD, unseen_count, return-to-live, load older w/ anchor | UESC §16.2; ICM §16.4-16.6, §37; REQ-UX-005 follow-live | Bare line offset; all records rendered (shell.ts:1008-1025; views-chat-trace.ts:134-157) |
| M12 | Pane resize/collapse/maximize (SplitPane) | UESC §22.11; ICM §27.3, §36.1 | Fixed widths (geometry.ts:52-72) |
| M13 | Surface state vector (visibility/size/placement/interaction) | UESC §7.1 | Only `focusedSurface` tracked (shell.ts:416-429) |
| M14 | ActionControl HIDDEN/VISIBLE_DISABLED/VISIBLE_ENABLED/BUSY_SUBMISSION | ICM §30; UESC §22.9 | Submission feedback is free-text notice (workbench-host.ts:256-270) |
| M15 | Operator-reachable reduced-motion (and colour) setting | DESIGN Responsive/Reduced motion (1566-1568); DSN-013 (1609) | Host config never sets motion (main.ts:22-26; shell.ts:448) |
| M16 | Real Case identity in chrome | UESC §5.2 | Hardcoded `gauntlet`/`canonical surface` (main.ts:23) |
| M17 | Composer SUBMITTING draft state | UESC §7.6 Draft machine | Enter clears draft immediately with `submitted` notice (composer.ts:381-395) |
| M18 | Surface/background/border semantic tokens + missing UESC §23 members | DESIGN Depth levels (1289-1293); UESC §23 | TokenSpec fg-only; no freshness/danger/quarantined (theme.ts:45-100) |

## 9. OpenTUI 0.5.11 inventory

Paths relative to `workbench/node_modules/@opentui/`. "Used" = production source; test-only noted.

| Export | Declaration | Used today |
|---|---|---|
| `createCliRenderer` | core/renderer.d.ts:196 | yes main.ts:82 |
| `CliRendererConfig` | core/renderer.d.ts:24 | yes main.ts:82-88 |
| `.useMouse` | core/renderer.d.ts:46 (setter :439) | set `false` main.ts:85 (default true) |
| `.enableMouseMovement` | core/renderer.d.ts:45 | no |
| `.exitOnCtrlC` / `.exitSignals` | core/renderer.d.ts:32 / :33 | yes `false` / `[]` main.ts:83-84 |
| `.screenMode` / `ScreenMode` | core/renderer.d.ts:48 / :60 | yes `alternate-screen` main.ts:86 |
| `.useKittyKeyboard` / `KittyKeyboardOptions` | core/renderer.d.ts:52 / :135 | no |
| `.autoFocus` | core/renderer.d.ts:47 | no |
| `.openConsoleOnError` | core/renderer.d.ts:54 | yes `false` main.ts:87 |
| `.backgroundColor` / `.targetFps` / `.maxFps` | core/renderer.d.ts:53 / :37 / :38 | no |
| `CliRenderer` | core/renderer.d.ts:223 | yes (type) |
| `CliRenderer.root` | core/renderer.d.ts:246 | yes terminal-renderer.ts:67 |
| `CliRenderer.keyInput` | core/renderer.d.ts:429 | yes main.ts:95,108 |
| `CliRenderer.requestRender` | core/renderer.d.ts:418 | implicit via `add()` |
| `start/auto/pause/stop` | core/renderer.d.ts:584/585/587/591 | no (on-demand) |
| resize event `RESIZE="resize"` | core/renderer.d.ts:198 | yes main.ts:112 |
| `CliRenderer.capabilities` | core/renderer.d.ts:452 | no |
| `TerminalCapabilities` (kitty_keyboard :64, rgb :66, unicode :68, focus_tracking :74, bracketed_paste :76, hyperlinks :77, osc52 :78) | core/types.d.ts:63 | no |
| `CliRenderer.themeMode` / `ThemeMode` | core/renderer.d.ts:460 / core/types.d.ts:25 | no |
| `focusRenderable` / `currentFocusedRenderable` | core/renderer.d.ts:402 / :398 | no |
| `setMousePointer` / `MousePointerStyle` | core/renderer.d.ts:530 / core/types.d.ts:36 | no |
| `hitTest` | core/renderer.d.ts:531 | no |
| `getSelection` / `clearSelection` / `Selection` | core/renderer.d.ts:606 / :609 / core/lib/selection.d.ts:4 | no |
| `widthMethod` / `WidthMethod` | core/renderer.d.ts:409 / core/types.d.ts:54 | no (geometry.ts reimplements) |
| `MouseEvent` (type :155, button :156, x/y :157-158, modifiers :160, scroll :165, target :166, isDragging :168, stopPropagation :177, preventDefault :178) | core/renderer.d.ts:154 | no |
| `MouseEventType` = down/up/move/drag/drag-end/drop/over/out/scroll | core/lib/parse.mouse.d.ts:1 | no |
| `MouseParser` | core/lib/parse.mouse.d.ts:18 | no |
| `RenderableOptions.onMouse/onMouseDown/Up/Move/Drag/DragEnd/Drop/Over/Out/Scroll` | core/Renderable.d.ts:74-83 | no |
| `RenderableOptions.onKeyDown` / `onSizeChange` | core/Renderable.d.ts:85 / :86 | no |
| `RenderableOptions.zIndex` | core/Renderable.d.ts:67 | no |
| `LayoutOptions.position` | core/Renderable.d.ts:38 | yes `absolute` terminal-renderer.ts:73 |
| `LayoutOptions.overflow` | core/Renderable.d.ts:39 | yes `hidden` terminal-renderer.ts:64 |
| `Renderable.focusable/focus()/blur()` | core/Renderable.d.ts:164/176/178 | no |
| `RenderableEvents.FOCUSED/BLURRED` | core/Renderable.d.ts:16-17 | no |
| `Renderable.renderSelf` (custom renderables) | core/Renderable.d.ts:281 | no |
| `BoxRenderable` (border :9, title :13) | core/renderables/Box.d.ts:24 (options :6) | yes terminal-renderer.ts:60,148 |
| `TextRenderable` | core/renderables/Text.d.ts:11 | yes terminal-renderer.ts:71 |
| `StyledText` / `TextAttributes` | core/lib/styled-text.d.ts:17 / core/types.d.ts:7 | yes terminal-renderer.ts:80,93-98 |
| `RGBA.fromHex` / `fromIndex` / `defaultBackground` | core/lib/RGBA.d.ts:18 / :19 / :21 | fromHex :92, defaultBackground :155; fromIndex no |
| `ScrollBoxRenderable` (stickyScroll :26, stickyStart :27, scrollAcceleration :30, viewportCulling :31, scrollTop :69, scrollBy :80, scrollTo :85) | core/renderables/ScrollBox.d.ts:33 | no |
| `ScrollBarRenderable` | core/renderables/ScrollBar.d.ts:15 | no |
| `LinearScrollAccel` / `MacOSScrollAccel` | core/lib/scroll-acceleration.d.ts:5 / :29 | no |
| `CodeRenderable` | core/renderables/Code.d.ts:36 | no |
| `DiffRenderable` | core/renderables/Diff.d.ts:34 | no |
| `MarkdownRenderable` | core/renderables/Markdown.d.ts:134 | no |
| `LineNumberRenderable` | core/renderables/LineNumberRenderable.d.ts:28 | no |
| `InputRenderable` | core/renderables/Input.d.ts:32 | no |
| `TextareaRenderable` | core/renderables/Textarea.d.ts:26 | no |
| `SelectRenderable` / `SelectRenderableEvents` | core/renderables/Select.d.ts:41 / :37 | no |
| `TabSelectRenderable` | core/renderables/TabSelect.d.ts:37 | no |
| `TextTableRenderable` | core/renderables/TextTable.d.ts:35 | no |
| `EmbeddedTerminalRenderable` | core/renderables/EmbeddedTerminal.d.ts:27 | test-only production-entry.test.ts:2,8 |
| `FrameBufferRenderable` / `ImageRenderable` / `SliderRenderable` | core/renderables/FrameBuffer.d.ts:9 / Image.d.ts:15 / Slider.d.ts:15 | no |
| `VRenderable` / `Box()` / `ScrollBox()` constructs | core/renderables/composition/VRenderable.d.ts:12 / constructs.d.ts:6,16 | no |
| `KeyEvent` / `PasteEvent` / `KeyHandler` | core/lib/KeyHandler.d.ts:4 / :30 / :47 | KeyEvent terminal-renderer.ts:16; paste main.ts:108 |
| `createTimeline` / `Timeline` | core/animation/Timeline.d.ts:125 / :80 | no |
| `TerminalPalette` | core/lib/terminal-palette.d.ts:43 | no |
| `createTestRenderer` | core/testing/test-renderer.d.ts:54 | test-only terminal-renderer.test.ts:3, production-entry.test.ts:3, cell-fit.test.ts:2 |
| `createMockMouse` (click :29, doubleClick :30, drag :33, scroll :34) | core/testing/mock-mouse.d.ts:27 | no |
| `createMockKeys` | core/testing/mock-keys.d.ts:34 | no (renderer `mockInput` used) |
| `createRoot` | react/src/reconciler/renderer.d.ts:25 | no |
| `createPortal` / `flushSync` | react/src/reconciler/renderer.d.ts:26 | no |
| `baseComponents` (box,text,code,diff,markdown,input,select,textarea,scrollbox,tab-select,line-number,image,span…) | react/src/components/index.d.ts:4-26 | no |
| `extend` (custom renderable catalogue) | react/src/components/index.d.ts:41 | no |
| `useKeyboard` | react/src/hooks/use-keyboard.d.ts:22 | no |
| `useTerminalDimensions` | react/src/hooks/use-terminal-dimensions.d.ts:1 | no |
| `useRenderer` | react/src/hooks/use-renderer.d.ts:1 | no |
| `useOnResize` | react/src/hooks/use-resize.d.ts:1 | no |
| `useTimeline` | react/src/hooks/use-timeline.d.ts:2 | no |
| `useFocus` / `useBlur` | react/src/hooks/use-focus.d.ts:10 / use-blur.d.ts:10 | no |
| `usePaste` / `useSelectionHandler` | react/src/hooks/use-paste.d.ts:10 / use-selection.d.ts:12 | no |
| `useAppContext` | react/src/components/app.d.ts:7 | no |
| `focused` prop on Box/Select/ScrollBox | react/src/types/components.d.ts:39 / :59 / :64 | no |
| `testRender` | react/src/test-utils.d.ts:3 | no |

## 10. Notes / uncertainty

- React absence is not a contract violation: REQ-TUI-001 says "OpenTUI and any React binding"; DESIGN React frame is "a lowering pattern" (DESIGN.md:507). Deps admitted by bun-native-smoke.test.ts:106.
- On-demand rendering conclusion from bundle source (`add()` → `requestRender()`; renderer `requestRender` immediate path when not running) — not from docs.
- `useFocus`/`useBlur` semantics (component vs terminal focus-tracking) not verified; listed by name only.
- geometry.ts cell width claims parity with OpenTUI `unicode` width method (geometry.ts:106-107); not cross-checked against `renderer.widthMethod`.
- Probe used harness header strings (includes a harness `run R17` span); production clipping shown by arithmetic in §7, not by live capture.
- Enter on Primary is a no-op (shell.ts:790-805): Work rows are not selectable/openable from Primary — belongs to views/interaction recon, flagged here only.
- Alt/meta chords are dropped at lowering (terminal-renderer.ts:19).
- No baseline captures taken (AREA did not request them).
