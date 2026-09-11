# Increment 2a: theme capability — UESC §23 members, surface/fg tokens, V6/M15 fixes

Agent slice: `workbench/src/theme.ts`, `workbench/src/theme-capability.test.ts` (pre-written RED contract, unchanged), `workbench/src/config.ts`, NEW `workbench/src/config.test.ts`, `workbench/src/main.ts`.
Frozen contract: `DESIGN.md` "Color Palette & Roles" (dark-surface projection, glyph family, gold discipline) + "Responsive Behavior > Color capability / Reduced motion"; report `03-hallmark-redesign.md` §6.
Violations repaired: V6 (truecolor assumed; now detected, default color256) and M15 (reduced-motion unreachable; now operator-reachable via env/config).
Branch `task26-dcritic-apparatus-repair`; NOT committed (per slice contract).

## Changes

### `workbench/src/theme.ts`

1. **UESC §23 semantic tokens (9 new, additive)** — `authority.degraded`,
   `settlement.quarantined`, `evidence.quarantined`, `capability.quarantined`,
   `freshness.current`, `freshness.stale`, `danger.destructive`,
   `proof.independently_confirmed`, `proof.sanctioned_authority`. Colors from
   DESIGN's utility palette (amber `#C58A2A` degraded/quarantined/freshness.stale,
   green `#2F8F5B` freshness.current, red `#9E2F2F` danger.destructive, GOLD
   `#D7A84A` for both proof tokens); glyphs from DESIGN's family
   (`?` quarantined, `✓` current, `◷` stale, `×` destructive, `◆` proof).
   Distinct quarantined labels (DSN-004): `QUARANTINED` / `EV-QUARANTINED` /
   `CAP-QUARANTINED`. `TokenFamily` extended with `freshness` and `danger`
   (prefix-aligned; the seven DSN-004-required families unchanged).
2. **`SURFACE_TOKENS` + `resolveSurfaceToken`** — DESIGN dark-surface projection
   (workspace `#05070A`, panel `#0B1220`, elevated `#111827`, overlay `#0F172A`,
   code `#08101D`, muted `#334155`; borders `#334155` panel/code/workspace,
   `#566978` elevated/overlay; muted border `#334155` — DESIGN assigns no distinct
   border for muted, its own value is the separator color). Lowering:
   truecolor `{background,border}` / color256 `{background256,border256}` /
   monochrome all-null.
3. **`TEXT_ROLE_TOKENS` + `resolveTextRoleToken`** — `fg.primary #FFFFFF`,
   `fg.secondary #94A3B8`, `fg.tertiary #64748B`, same three-way lowering.
4. **`detectColorCapability(env, terminalRgb?)`** — precedence exactly as pinned:
   non-empty `NO_COLOR` → monochrome (even with rgb); valid
   `GAUNTLET_WORKBENCH_COLOR` override; `terminalRgb === true` → truecolor;
   `COLORTERM` truecolor/24bit → truecolor; else color256 (both `*-256color`
   TERM and unknown terminals — never assume truecolor; V6).
5. **`resolveMotionPreference(env)`** — valid `GAUNTLET_WORKBENCH_MOTION`
   (`reduced|full`) wins; invalid ignored; non-empty `NO_MOTION` → reduced;
   unset → full (DSN-013, M15).
6. **`COLOR_256_ANCHORS` extended** with nearest xterm-256 indexes for the nine
   new hex values (exhaustive nearest-RGB over the 256-color palette:
   `#05070A`→232, `#0B1220`→233, `#111827`→234, `#0F172A`→234, `#08101D`→233,
   `#334155`→238, `#FFFFFF`→15, `#94A3B8`→109, `#64748B`→66).

### `workbench/src/config.ts`

`WorkbenchConfig` gained optional `motion?: "full"|"reduced"` and
`color?: "truecolor"|"color256"|"monochrome"`, env-mirrored from
`GAUNTLET_WORKBENCH_MOTION` / `GAUNTLET_WORKBENCH_COLOR` (documented-fields-only).
Invalid/unset → `undefined` (ignored, not guessed); defaults live in the theme
layer, not config. **The unions are deliberately inlined literals**: `config.ts`
is pinned state-layer in `renderer-boundary.test.ts` and `theme.ts` is a
presentation seed — even a type-only import would reclassify it and fail the
DSN-011 boundary gate.

### `workbench/src/config.test.ts` (NEW)

Pins the env surface: valid mappings both directions, unset/invalid → undefined.

### `workbench/src/main.ts`

`motion` resolved once from env; the `shell()` closure now takes the session.
Session construction moved per-path so colour capability is detected **after**
the renderer exists: TTY path passes
`detectColorCapability(process.env, renderer.capabilities?.rgb === true)`; the
headless path passes `detectColorCapability(process.env)` (no terminal report).
Behavior when env is unset is identical except the session's default capability
is now color256 (V6 fix). Initial dims, quit/signal handling, and the
refresh/close lifecycle are unchanged; `production-entry.test.ts` passes against
the real spawned process.

## Test/check results

- `bun test src/theme-capability.test.ts` — 18/18 pass (RED → GREEN).
- `bun test src/config.test.ts src/config-preflight.test.ts` — 10/10 pass.
- `bun test src/index.test.ts` (barrel re-exports) — pass.
- `bun test src/renderer-boundary.test.ts src/keyboard-accessibility.test.ts` — pass.
- `bun test src/work-chat-trace.test.ts` — 1 EXPECTED fail, see conflict 1.
- `bun test src/production-entry.test.ts` — pass (6.5 s, real PTY loop).
- `bun run check` — zero errors in slice files; one pre-existing foreign error
  in the untracked `src/renderer-hittest.test.ts` (concurrent increment's RED
  file, see conflict 3). It failed before this slice touched anything.
- `bun x biome check --write` on all five touched files, then clean (exit 0).

## Integration notes for the orchestrator

1. **`work-chat-trace.test.ts` gold pin is now stale (out of my ownership).**
   "T12 DSN-003 ... only the sanctioned authority and independent proof tokens
   carry brand gold" asserts
   `goldReservedTokens()` EQUALS `["authority.sanctioned","proof.independent"]`
   and that every other token's color `!== GOLD`. That exact pin is
   unsatisfiable with the RED contract's four gold tokens
   (`proof.independently_confirmed`, `proof.sanctioned_authority` included).
   The increment-2a contract explicitly authorizes the four-token list; the
   stale assertion needs a one-line update to the four-token list (its
   family∈{authority,evidence} sub-check already passes: the new proof tokens
   use families `evidence` and `authority` respectively).
2. **`terminal-renderer.ts` still assumes truecolor (V6 residue, out of
   ownership).** `TerminalShellRenderer.render` lowers spans with
   `resolveToken(token, process.env["NO_COLOR"] === undefined ? "truecolor" : "monochrome")`
   — it ignores `ShellState.colorCapability` entirely. The session state is now
   correctly detected, but the renderer's lowering should consume
   `shell.motion`-style state (the resolved capability) rather than re-sniffing
   env. Recommend a small follow-up slice: thread `state.colorCapability`
   (or the `RenderedShell`) into the renderer's `resolveToken` calls. Rendered
   TEXT is unaffected today (glyphs/labels identical), which is why all shell
   tests stay green.
3. **Concurrent-slice interference (not mine):** untracked
   `src/renderer-hittest.test.ts` (mouse/hit-map increment) currently fails
   `bun run check` with TS2339 (`hitMap`/`createdLastRender` not on
   `TerminalShellRenderer`) — it was failing before my first edit and its error
   set is shrinking as that agent works. `just tui-check` cannot go green until
   their slice lands.
4. **`createShellState` default remains `"truecolor"`** (`shell.ts`, out of
   ownership). Production `main.ts` now always passes an explicit detected
   value, so the V6 fix holds for the production path; test harnesses keep the
   old default. If you want the default itself lowered to color256, that is a
   one-line change in `shell.ts` plus a harness expectation check — left
   untouched to keep this slice additive.
5. Config's `motion`/`color` are env-mirrored only (no cli/file surface), per
   the slice contract; main.ts consumes the theme-layer resolvers directly, and
   the config fields are the typed, documented mirror for other consumers.

## Files changed

- `workbench/src/theme.ts` (extended, +~290 lines)
- `workbench/src/config.ts` (+27 lines)
- `workbench/src/config.test.ts` (NEW, 43 lines)
- `workbench/src/main.ts` (rewired session construction, +36/-10)
- `workbench/src/theme-capability.test.ts` (unchanged RED contract)
