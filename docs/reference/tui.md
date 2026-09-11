# Reference: Operator Workbench (`workbench/`)

This document provides technical reference specifications for the Gauntlet Operator Workbench TUI, including keybindings, surfaces, views, semantic theme tokens, geometry classes, configuration variables, verification gates, and design conformance clauses.

---

## 1. Keybinding Catalog

All keys operate directly via host terminal raw mode decoding ([`shell.ts`](../../workbench/src/shell.ts), [`main.ts`](../../workbench/src/main.ts)).

### Global Navigation & Views

| Key | Context | Action |
|---|---|---|
| `1` | Global | Switch Primary surface to **Work View** (`views-work.ts`). |
| `2` | Global | Switch Primary surface to **Chat View** (`views-chat-trace.ts`). |
| `3` | Global | Switch Primary surface to **Trace View** (`views-chat-trace.ts`). |
| `Tab` | Global | Cycle active focus clockwise through surfaces: `Navigator` $\rightarrow$ `Primary` $\rightarrow$ `Inspector` $\rightarrow$ `Composer`. |
| `Esc` | Global | Dismiss overlays / palette, restore focus to `Primary`. Never cancels running work. |
| `p` | Global | Open / close the **Command Palette** overlay. |
| `e` | Global | Focus the **Composer** input surface. |
| `h` | Focused Entity | **Hoist**: Narrow active horizon to selected entity (e.g. Unit `AK-014`). |
| `u` | Hoisted State | **Unhoist**: Return active horizon to parent scope. |
| `q` | Global | Gracefully restore terminal modes and quit the workbench process. |

### Surface Navigation

| Key | Surface | Action |
|---|---|---|
| `Up` / `Down` | Navigator | Move cursor up or down through the Case $\rightarrow$ Reference $\rightarrow$ Unit $\rightarrow$ Run hierarchy. |
| `Enter` | Navigator | Select highlighted row as active `Focus` (updates Inspector). |
| `Up` / `Down` | Primary View | Scroll view contents vertically. |
| `Up` / `Down` | Command Palette | Navigate palette entries. |
| `Enter` | Command Palette | Execute selected command, switch to selected view, or jump to focus. |
| `Enter` | Composer | Submit message or command to active destination mode. |

---

## 2. Surface & View Inventory

### Surfaces ([`shell.ts`](../../workbench/src/shell.ts))

| Surface ID | Role | Layout Behavior |
|---|---|---|
| `Navigator` | Hierarchical outline of the Case | Left column in Wide/Full; hidden/overlay in Compact. |
| `Primary` | Question-oriented data projection | Center column in Wide/Full; full width in Compact. |
| `Inspector` | Contextual details for active `Focus` | Right column in Wide/Full; overlay in Compact. |
| `Bottom` | Secondary dock for live terminal/repl | Optional bottom region beneath Primary. |
| `Composer` | Input surface for text & commands | Persistent strip at the bottom of the screen. |
| `Overlay` | Modal dialogs, palette, takeovers | Centered floating modal capturing active input. |

### Views ([`views-work.ts`](../../workbench/src/views-work.ts), [`views-chat-trace.ts`](../../workbench/src/views-chat-trace.ts))

| View ID | Primary? | Core Question Answered | Implementation Locus |
|---|---|---|---|
| `work` | **Yes** | "What are we trying to move?" (Escalations, active/settled units) | [`views-work.ts`](../../workbench/src/views-work.ts) |
| `chat` | **Yes** | "What was communicated?" (Conversational turns; subordinate to Trace) | [`views-chat-trace.ts`](../../workbench/src/views-chat-trace.ts) |
| `trace` | **Yes** | "What actually happened?" (Immutable execution event stream) | [`views-chat-trace.ts`](../../workbench/src/views-chat-trace.ts) |
| `evidence` | No | "What has been proven?" (Inspectable evidence record catalog) | [`views-chat-trace.ts`](../../workbench/src/views-chat-trace.ts) |
| `agents` | No | "What execution capacity is running?" (Agent pool status) | [`views-chat-trace.ts`](../../workbench/src/views-chat-trace.ts) |
| `attention` | No | "What requires operator intervention?" (Open human escalations) | [`views-chat-trace.ts`](../../workbench/src/views-chat-trace.ts) |
| `files` | No | "What files were touched in this workspace?" | [`shell.ts`](../../workbench/src/shell.ts) |
| `kernel` | No | "What is the local computational state?" | [`prime-kernel.ts`](../../workbench/src/prime-kernel.ts) |
| `terminal` | No | "What is running in the embedded PTY?" | [`terminal-session.ts`](../../workbench/src/terminal-session.ts) |
| `domain` | No | "What domain concepts exist?" | [`domainforge-adapter.ts`](../../workbench/src/domainforge-adapter.ts) |

> **Roadmap Boundary**: The view registry explicitly forbids `harness` (the Harness View is roadmap-only per `DSN-015`). Registering it raises a runtime exception.

---

## 3. Semantic Token Catalog & Degradation

Semantic tokens defined in [`theme.ts`](../../workbench/src/theme.ts) map abstract system states to visual styling across three color capability levels:

| Token | Family | Glyph | Truecolor Hex | ANSI 256 | Monochrome Differentiation |
|---|---|---|---|---|---|
| `settlement.accepted` | Settlement | `✓` | `#4ade80` (Green) | 48 | `✓ SETTLED` (bold) |
| `evidence.complete` | Evidence | `●` | `#60a5fa` (Blue) | 75 | `● VERIFIED` (normal) |
| `evidence.failed` | Evidence | `✗` | `#f87171` (Red) | 203 | `✗ FAILED` (bold) |
| `evidence.missing` | Evidence | `○` | `#94a3b8` (Slate) | 246 | `○ INCONCLUSIVE` (dim) |
| `execution.running` | Execution | `▸` | `#38bdf8` (Sky) | 81 | `▸ RUNNING` (bold) |
| `readiness.blocked` | Readiness | `!` | `#facc15` (Amber) | 220 | `! BLOCKED` (bold) |
| `attention.required` | Attention | `!` | `#fb923c` (Orange) | 208 | `! ATTENTION` (bold) |
| `brand.gold` | Proof | `★` | `#eab308` (Gold) | 220 | `★ PROVEN` (bold) |
| `structure.workspace` | Structure | `·` | `#0f172a` (Navy) | 234 | Normal background |
| `surface.panel` | Structure | `·` | `#1e293b` (Slate) | 236 | Thin box-drawing line |

### The Gold Discipline (`DSN-003`)
`brand.gold` is never used for ordinary selection, hover states, UI accents, or decorative icons. It is strictly reserved for:
- Formally certified proof obligations (`P1`–`P12`).
- Sanctioned operator authority signatures.
- Certified settlement records.

---

## 4. Geometry Classes & Thresholds

Defined in [`geometry.ts`](../../workbench/src/geometry.ts):

| Class | Column Floor | Row Floor | Active Column Budget |
|---|---|---|---|
| **Wide** | 140 | 40 | Navigator: 32 cols · Primary: $\ge 64$ cols · Inspector: 44 cols |
| **Full** | 120 | 40 | Navigator: 28 cols · Primary: $\ge 52$ cols · Inspector: 40 cols |
| **Compact** | 80 | 24 | Single primary pane with switchable tabs and composer strip |
| **BelowMinimum** | $< 80$ | $< 24$ | Recoverable prompt; normal rendering halted |

---

## 5. Configuration & Environment Variables

Resolved via [`config.ts`](../../workbench/src/config.ts):

| Variable | Default Value | Description |
|---|---|---|
| `GAUNTLET_ENDPOINT` | `http://127.0.0.1:4000` | Target loopback HTTP URL for `gauntlet serve` (bind itself is set separately via `GAUNTLET_API_BIND`; there is no `--port` flag). |
| `GAUNTLET_SESSION_TOKEN` | *None* | Bearer authentication token for loopback API. |
| `GAUNTLET_ACTOR` | `operator` | Authenticated actor identifier bound to command idempotency keys. |
| `GAUNTLET_TUI_COLOR` | `auto` | Color capability override: `truecolor`, `color256`, or `monochrome`. |
| `GAUNTLET_TUI_MOTION` | `normal` | Motion preference: `normal` or `reduced`. |
| `GAUNTLET_LOG_PATH` | `.gauntlet/tui.log` | Path for TUI diagnostic logs (always separate from stdout/stderr). |

---

## 6. Verification Gates

All workbench verification is orchestrated via repository `just` recipes:

| Recipe | Underlying Command | What It Verifies |
|---|---|---|
| `just tui-check` | `cd workbench && bun run check` | TypeScript strict type-check (`tsc --noEmit`). |
| `just tui-test [selector]` | `cd workbench && bun test [filter]` | Unit, integration, and property test suites. `selector=all` runs full normative conformance. |
| `just tui-lint` | `cd workbench && bun run lint` | Biome linting and formatting check (`biome check .`). |
| `just tui-preflight bootstrap` | `./scripts/tui-preflight.sh bootstrap` | Verifies pinned dependencies, `bun.lock`, audit, and `DESIGN.md` / spec SHA-256 digests. |
| `just tui-preflight conformance` | `./scripts/tui-preflight.sh conformance` | Full runtime qualification across all integration boundaries. |
| `just tui-bench <selector>` | `cd workbench && bun run src/<sel>.bench.ts` | Performance benchmarks (`bun-substrate`, `focus-latency`, `projection-worker`, `sqlite-replay`, `terminal-path`, `large-case`, `long-horizon`, `all-required`). |
| `just tui-build release` | `cd workbench && bun run src/release-manifest.ts` | Builds compiled binary (`dist/gauntlet-workbench`) and release manifest. |

---

## 7. Design Conformance Index (DSN-001..DSN-015)

From `DESIGN.md`:

| ID | Design Clause | Spec Basis | Owning Task |
|---|---|---|---|
| **DSN-001** | Components resolve semantic theme tokens; raw colour literals appear only in theme adapters. | `REQ-TUI-001` | T12 |
| **DSN-002** | Consequential state stays legible through glyph + label + emphasis + ordering in monochrome. | `REQ-UX-005` | T12, T20 |
| **DSN-003** | `brand.gold` carries only proof, independent confirmation, or sanctioned authority. | `REQ-UX-005` | T12 |
| **DSN-004** | Authority, execution, evidence, settlement, readiness stay distinct token families. | `REQ-UX-003` | T12 |
| **DSN-005** | No component-local authority: enablement comes strictly from canonical affordance resolver. | `REQ-AFF-001` | T09, T12 |
| **DSN-006** | Every rendered occurrence of a Focus is a placement of one `FocusRef`; no copying of state. | `REQ-FOCUS-001` | T08, T12 |
| **DSN-007** | Geometry classes honour spec floors: full layout at 120x40, compact at 80x24, prompt below 80x24. | `REQ-UX-005` | T12, T20 |
| **DSN-008** | Evidence and authority render as inspectable record objects with provenance, not badges. | `REQ-UX-003` | T12 |
| **DSN-009** | Blocked affordances show blocking gate and next step; impossible actions stay absent. | `REQ-AFF-005` | T09, T12 |
| **DSN-010** | Chat is subordinate to Trace; Trace never takes Chat as input. | `REQ-DATA-005` | T07, T12 |
| **DSN-011** | No component holds canonical truth; views are replaceable projections. | `REQ-DATA-001` | T07, T19 |
| **DSN-012** | Plugins register and unregister cleanly with zero leaked subscriptions. | `REQ-PLUGIN-003` | T15 |
| **DSN-013** | Motion is optional: reduced-motion mode retains full operational clarity. | `REQ-UX-005` | T12 |
| **DSN-014** | Untrusted text renders as text; ANSI/OSC escape sequences do not leak into host terminal. | `REQ-UX-006` | T13, T18 |
| **DSN-015** | No roadmap surfaces built: Harness View is forbidden. | `roadmap_boundaries` | T25 |

---

## 8. Source Trail

- [`workbench/src/shell.ts`](../../workbench/src/shell.ts) — Keybinding definitions, surface registry, view list.
- [`workbench/src/theme.ts`](../../workbench/src/theme.ts) — Semantic tokens, status descriptors, palette lowering.
- [`workbench/src/geometry.ts`](../../workbench/src/geometry.ts) — Responsive layout math and column allocation.
- [`workbench/src/config.ts`](../../workbench/src/config.ts) — Environment variable resolution and endpoint defaults.
- [`workbench/src/tui-test-all.ts`](../../workbench/src/tui-test-all.ts) — Normative test harness runner.
- [`DESIGN.md`](../../DESIGN.md) — Root design system and visual authority contract.
