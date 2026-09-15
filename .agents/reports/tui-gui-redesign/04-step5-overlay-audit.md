# §8 step 5 overlay conformance audit (tui-gui-redesign)

Date: 2026-09-15. Scope: 03-hallmark-redesign.md §8 step 5 ("Overlays —
palette filter/ids, dialogs, context menus, tooltips, toasts (§3)") against
the §3 overlay table, read from `workbench/src` (shell.ts, command-registry.ts,
terminal-renderer.ts, views-chat-trace.ts). Read-only; no code changed.

Verdict: **step 5 is genuinely incomplete.** Palette, context menu, tooltip,
attention VIEW, and help VIEW exist; **modal Dialog and Toast DO NOT EXIST in
workbench/src at all** — no symbols, no commands, no renderer types
(`dialog`/`toast` match only `node_modules`/`dist`). The palette's staged
confirm renders as a reworded inline ROW, not the §3-required confirm dialog.
ANSI evidence (`baseline/after/` captures 28-33) covers only the five
chrome-remainder increments, never a dialog versus toast distinction.

## Per-overlay findings

| §3 overlay | Status | Evidence |
|---|---|---|
| Command palette | PRESENT | `paletteOpen`, `paletteQuery`, `ui.palette.*` commands, `FramePlan.overlay key:"palette"` (terminal-renderer.ts:65-95); rows carry title · command id · shortcut · status+reason (shell.ts ~2780+); covered live via SGR session 30 |
| Context menu | PRESENT | `right_click` PointerGesture + `ShellState.contextMenu` + `ui.context_menu.*` commands; clamped anchor + Esc/click-away dismiss; covered live via SGR session 30 |
| Help overlay | PRESENT (view) | keymap/pointer-equivalents content carried from prior recon; opened through the palette/view path, not a dedicated `ui.help` command |
| Attention overlay | PRESENT (view) | `renderAttentionView` (views-chat-trace.ts:554), wired into the Work/attention surface; no standalone `attention.open` command — the `! N`/takeover trigger routes through existing focus/composer paths |
| Dialog (modal) | **ABSENT** | No `dialog` symbol anywhere in workbench/src; no command, no `OverlayPlacement` kind, no renderer branch. §3 also requires the palette's staged double-Enter confirm to render "as a confirm dialog, not an inline row" — but `pendingConfirm` (shell.ts:1368) renders as a reworded inline row: `Invoke <op> on <kind>:<id>? Enter confirms, Esc backs out` (shell.ts:2782-2798), inside the same `palette.${index}` row. The staged-pair revalidation logic itself is sound; only the presentation form contradicts §3. |
| Popover/tooltip | PRESENT | `hover` PointerGesture from OpenTUI over/out + shared `rowMouseEvent` fall-through (d86acb7); delay honestly immediate-on-hover, recorded |
| Toast | **ABSENT** | No `toast` symbol anywhere in workbench/src; no `ui.notice`/`ui.toast` command; no auto-expiring non-focus-stealing surface. The §3 behavior it should carry (submission lifecycle per-control from ActionControl BUSY_SUBMISSION + gateway outcomes with the full reason grammar) exists upstream as state (c1c7bca) but has no toast presentation — outcomes surface inside Work rows/bar text instead |

## Why the gap went unrecorded

- §3's row for the confirm is explicit ("rendered as a confirm dialog, not an
  inline row", carried from the P9-earned staged confirm), yet increment 1's
  settlement (6e84eff — "semantic command layer") and every later gate
  validated the *logic* (staged pair revalidates, no stale confirm can invoke),
  never the *overlay form*.
- Toast has no settled label anywhere in the increments/follow-on lists, and
  its upstream content (BUSY_SUBMISSION from c1c7bca) landed a step after the
  overlay work, so nothing ever scheduled its presentation.
- The §8 step list itself compounds the issue by naming step 5's scope as
  "palette filter/ids, dialogs, context menus, tooltips, toasts" while the
  queue-close criteria verified the chrome-remainder five (breadcrumb, context
  menu, resize, MenuBar, tooltips) — a different five.

## Blast radius (bounded, not speculative)

- Adding a modal Dialog is a NEW overlay kind in the terminal-renderer
  `FramePlan` (palette and contextMenu are the two existing `overlay` kinds)
  plus a command family wired through the single intent reducer, with hit-map
  routing for pointer dismissal. The palette staged-confirm logic can keep its
  reducer behavior and re-route its RENDERING to the dialog kind.
- Adding a Toast is a NEW non-focus-stealing ephemeral surface with a timer
  path — which the d86acb7 record explicitly notes `main.ts` does NOT have
  (its render loop is gesture/key/resize-driven; the only timer is the ~2s
  host poll). That missing timer infrastructure is the honest cost driver: a
  spec-faithful Toast needs a general timer-redraw path first, the same work
  the tooltip delay skipped as disproportionate. Without it, a Toast can only
  be bolted onto the 2s host poll (jitter <=2s on expiry) or the poll cadence
  must be revisited.

## Alternatives, honestly stated

1. **Amend the design** (not the code): if modal Dialogs and Toasts are not
   wanted, §3 + step 5 should say so (e.g. "staged confirm stays an inline
   row; outcomes stay in Work rows/bar"), with the iconography constraints
   that substitute. Docs-only change to a committed design (fedbad1) — needs
   operator decision, since the design is the contract.
2. **Build them** as a scoped increment, priced with the timer caveat above —
   new primitives + hit-map routing + SGR-level qualification (the step-8
   discipline), captures 34+ under `baseline/after/`, then a commit. It does
   NOT fit inside the closed queue.
3. **Do nothing**: step 5 stays partially complete — but then CURRENT_STATUS
   must stop implying otherwise.

## Reproduce (the checks that produced this)

```bash
# absence by name (only node_modules/dist hit — never workbench/src)
grep -ril 'dialog' workbench/ | grep -v node_modules | grep -v /dist/
grep -ril 'toast' workbench/ | grep -v node_modules | grep -v /dist/
# the two overlay kinds the renderer knows
grep -n 'overlay: {' workbench/src/terminal-renderer.ts
# staged confirm: logic shell.ts:1368 + reducer ~2110 + ~2324, rendering as inline row shell.ts:2782-2798
grep -n 'pendingConfirm' workbench/src/shell.ts | tail -3
# full command inventory (41): no ui.dialog.*, no ui.notice/ui.toast, no attention.open
grep -rho '"[a-z][a-z0-9_.]*\.[a-z][a-z0-9_.]*"' workbench/src/command-registry.ts | tr -d '"' | sort -u
```
