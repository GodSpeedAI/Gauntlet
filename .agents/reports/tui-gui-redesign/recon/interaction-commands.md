# Recon: interaction-commands

Scope: key decoding, keymap, surface/row focus, select/open, hoist, history, palette, help, composer/takeover, semantic dispatch (registry, planned ops, resolver, gateway, kernel wrappers, evidence promotion). Read-only. Every anchor below was opened and read.

Runtime fact: production uses imperative `@opentui/core` only (`main.ts:2`, `terminal-renderer.ts:1-9`). No `@opentui/react` import exists in `src/`; it appears only in the admitted-deps list at `bun-native-smoke.test.ts:106`. Mouse is off: `createCliRenderer({ useMouse: false })` at `main.ts:82-88`.

## Direct answers

**Q1. Is there one semantic command identity layer that keys, palette and visible actions all route through?**
No. There are three disjoint identity spaces:
1. **Raw key strings.** `handleKey` switches on literals (`shell.ts:602-838`) and applies the state change directly. It emits at most five `ShellEffect` types (`shell.ts:454-459`). No command ids, no command envelope (ICM §4), no keymap resolver (ICM §34).
2. **Canonical `OperationId`s.** Three ids: `unit.resolve`, `unit.retry`, `state.refresh` (`operation-registry.ts:24-28`). They are used by the palette, the Work ACTIONS bar and the Inspector AFFORDANCES section (all via `resolveAffordance`), and by kernel and Prime routes via the gateway.
3. **Planned ids.** `prime.launch`, `prime.message`, `prime.steer`, `prime.cancel`, `evidence.admit`, `prime.session` (`planned-operations.ts:40-47`). These are declarations only and not invocable.

Only the consequential edge converges: palette row → `ShellEffect invoke_affordance` → `WorkbenchHost.invoke` → `OperationGateway.invoke`. The Work ACTIONS bar and Inspector rows are display-only; no key activates them.

**Q2. Which paths invoke operations directly?**
- No presentation module calls `gateway.invoke` or `.perform`. A static pin enforces this (`cross-surface-equivalence.test.ts:717-720`), and the host is the single production gateway caller (`:645-649`).
- Direct model mutation from shell interactions is navigation-local and legitimate: `WorkbenchHost.interactions` calls `model.selectFocus`, `hoistToFocus` and `dehoist` (`workbench-host.ts:230-241`).
- `state.refresh` short-circuits to `host.refresh()` without the gateway (`workbench-host.ts:252-255`). It is a read, but the palette shows it as `unavailable`.
- Composer `operation` mode sends the composer destination string to `host.invoke` as the operation id, with the draft as the note (`workbench-host.ts:275-279`).
- The shell writes Focus and Horizon into its own `ShellState.location` before or alongside effects (`shell.ts:710,797,812,824`). This is a shadow copy (see V2).
- Kernel route: `KernelAffordanceSet.invoke` → `gateway.invoke` (`kernel-affordances.ts:292-299`).
- Prime route: `PrimeDelegationDispatcher.drive` → `gateway.invokeVia` (`prime-recursion.ts:503`). The dispatcher builds its **own** gateway (`prime-recursion.ts:363`), separate from the host's (`workbench-host.ts:97`).
- Neither the kernel nor the Prime route is reachable from any key in production.

**Q3. How do blocked affordances carry reasons?**
1. `resolveAffordance` (`affordance.ts:168-317`) returns a frozen descriptor: a four-way `status` plus flat string `reason_codes` using the canonical refusal vocabulary.
2. `materializeAffordances` (`views-work.ts:109-145`) sorts each descriptor into `materialized`, `blocked_with_reason` or `palette_only`. It attaches `nextStep`, which is prose from `reasonNextStep` (`inspector.ts:81-108`) for the first code only.
3. Where reasons render:
   - Work: `× OP BLOCKED: codes · next: …` (`views-work.ts:318-348`)
   - Palette: `action op: status (codes)` (`shell.ts:1162-1169`)
   - Inspector: AFFORDANCES records (`inspector.ts:213-289`)
4. Invocation-time refusals carry `code`, `message`, `reason_codes`, `stale` and `descriptor` (`operation-gateway.ts:122-157,511-534`). The host flattens them to the notice `${op}: ${class} — ${code}` (`workbench-host.ts:265-270`), which drops `reason_codes` and the descriptor.
5. None of this matches ICM §33: no reason `class`, `summary`, `related_focus` or `next_affordance` id.

**Q4. What would a mouse gesture need to call to converge on the keyboard path?**
1. Enable pointer input: `useMouse: true` (`main.ts:85`).
2. Add a hit map from rendered cells back to line and element ids. The renderer draws absolute `cell-x-y` TextRenderables and keeps no mapping (`terminal-renderer.ts:68-104,119-133`). Stable line ids already exist:
   - Navigator `row.id` (`shell.ts:939-940`)
   - `palette.${index}` (`shell.ts:1151,1171`)
   - `work.affordance.${op}` (`views-work.ts:325`)
   - `trace.${trace_id}` (`views-chat-trace.ts:137`)
3. Extract an intent reducer from `handleKey`. Local state (`focusedSurface`, `navigatorIndex`, `paletteIndex`, `pendingConfirm`, `scroll`) has no setter other than `handleKey(key)`. Without the extraction a click would have to fake N arrow keys.
4. For consequential or navigation effects, reuse the existing edge unchanged:
   - `applyShellEffects(effects, interactions)` (`shell.ts:473-496`)
   - selection: `{type:"select_focus", ref}` → `interactions.selectFocus` → `WorkbenchModel.selectFocus` (`workbench-model.ts:550-556`)
   - actions: `{type:"invoke_affordance", operation, ref}` → `WorkbenchHost.invoke` (`workbench-host.ts:251-272`) → `OperationGateway.invoke` (`operation-gateway.ts:250-252`), whose `dispatch` runs fresh observe, gate and ledger (`:273-422`)

## Table 1 — Key bindings (decoder → effect → handler)

| Key (normalized) | Context | Effect | Handler |
|---|---|---|---|
| release events | any | dropped | terminal-renderer.ts:17 |
| `ctrl+<name>` | any | string `ctrl+x`; only `ctrl+c` consumed | terminal-renderer.ts:18; main.ts:97-100 |
| `ctrl+c` | any | quit (host.close; no stop) | main.ts:97-100 → main.ts:18-21 |
| meta/option chords | any | dropped (null) | terminal-renderer.ts:19 |
| Tab / Shift-Tab (CSI Z) | any | `tab` / `shift+tab` | terminal-renderer.ts:20; production-entry.test.ts:112-116 |
| Return/linefeed | any | `enter` | terminal-renderer.ts:21 |
| escape, backspace, delete, arrows, home, end | any | named; delete/left/right/home/end have no consumer | terminal-renderer.ts:22-27 |
| other | any | `key.sequence ‖ key.name` (Unicode by OpenTUI) | terminal-renderer.ts:28 |
| bracketed paste | Composer focused | append to draft/takeoverInput; never command | main.ts:108-111; shell.ts:1300-1314 |
| `q` | BelowMinimum | quit; all else ignored | shell.ts:611-613 |
| `escape` | palette open | clear staged confirm, else close palette | shell.ts:617-626 |
| `p` | palette open | close palette | shell.ts:627-636 |
| `up`/`down` | palette open | move paletteIndex (clamped), clear confirm | shell.ts:637-653 |
| `enter` | palette: header | no-op | shell.ts:657-659 |
| `enter` | palette: available view | switchView, close | shell.ts:660-673 |
| `enter` ×1 / ×2 | palette: affordance (any status) | stage confirm / emit `invoke_affordance` | shell.ts:674-702 |
| `enter` | palette: focus row (Compact only) | `select_focus`, close | shell.ts:703-714 |
| `enter` | palette: absent view | clear confirm only | shell.ts:715-718 |
| other printable | palette open | ignored (no filter) | shell.ts:720-721 |
| `1`/`2`/`3` | shell | view work/chat/trace | shell.ts:505-509,725-728 |
| `shift+tab` / `tab` | shell | prev/next in visibleSurfaces | shell.ts:731-740,511-522 |
| `escape` | shell | focusedSurface→Primary | shell.ts:741-750 |
| `p` | shell | open palette (Overlay) | shell.ts:751-760 |
| `up`/`down` | Navigator | navigatorIndex ± (clamped) | shell.ts:764-774 |
| `up`/`down` | Primary | per-view line scroll (no upper clamp) | shell.ts:775-787 |
| `up`/`down` | Inspector/other | no-op | shell.ts:788 |
| `enter` | Navigator | `select_focus` + location.focus | shell.ts:791-802 |
| `enter` | Primary/other | no-op | shell.ts:804 |
| `h` | shell | hoist `snapshot.focus ?? location.focus` | shell.ts:806-818 |
| `u` | shell | dehoist (one step) | shell.ts:819-830 |
| `e` | shell | focus Composer | shell.ts:831-832 |
| `q` | shell | quit | shell.ts:833-834 |
| `escape` | takeover active | dismiss → request re-queued, draft restored | shell.ts:1259-1263; composer.ts:234-251 |
| `tab`/`shift+tab` | takeover active | swallowed | shell.ts:1264-1266 |
| printable / `backspace` | takeover active | edit takeoverInput (grapheme erase) | composer.ts:342-356 |
| `enter` | takeover active | submit + local completeTakeover | composer.ts:357-365; shell.ts:1267-1276 |
| `escape`/`tab`/`shift+tab` | Composer focused | delegated to handleKey | shell.ts:1280-1285 |
| printable / `backspace` | Composer focused | edit draft (grapheme erase) | composer.ts:369-380 |
| `enter` | Composer focused | submit; draft cleared immediately | composer.ts:381-395; shell.ts:1286-1290 |

## Table 2 — ICM semantic id → existing code identity

| ICM id | Existing identity | Anchor |
|---|---|---|
| nav.focus.select | ShellEffect `select_focus` → host.selectFocus → WorkbenchModel.selectFocus | shell.ts:455,791-802,703-714; workbench-host.ts:230-233; workbench-model.ts:550-556 |
| nav.focus.open | none (Enter = select) | shell.ts:790-805 |
| nav.focus.inspect | implicit: Inspector renders `snapshot.focus ?? location.focus` | shell.ts:1036-1073 |
| nav.horizon.hoist | key `h` → ShellEffect `hoist` → WorkbenchModel.hoistToFocus | shell.ts:806-818; workbench-model.ts:587-605 |
| nav.horizon.dehoist | key `u` → ShellEffect `dehoist` → WorkbenchModel.dehoist (stack pop) | shell.ts:819-830; workbench-model.ts:611-618 |
| nav.history.back / forward | none | — |
| ui.context_menu.open | none (mouse off) | main.ts:85 |
| ui.presentation.close | partial: Esc closes palette / blurs surface | shell.ts:617-626,741-750 |
| view.activate.work/chat/trace | switchView via 1/2/3 or palette | shell.ts:505-509,524-530,660-673 |
| view.activate.evidence/agents/attention | palette view rows only | shell.ts:544-553; views-chat-trace.ts:322-338 |
| view.activate.files/kernel/domain | ViewId + availability only; no renderer | shell.ts:79-88,206-224 |
| view.activate.reference/harness | none (Harness forbidden, DSN-015) | shell.ts:174-180 |
| ui.inspector.pin / unpin / toggle | none | — |
| ui.navigator.toggle | none | — |
| ui.bottom.toggle | none ("Bottom" only in SurfaceId) | shell.ts:69 |
| ui.pane.resize | none | — |
| ui.tab.* | none | — |
| trace.event.select | none (rows not selectable) | views-chat-trace.ts:134-157 |
| trace.follow.suspend / resume | none | — |
| trace.history.load_older | none | — |
| trace.filter.set / trace.search | none | — |
| composer.draft.edit | composeKeypress + ShellSession.paste | composer.ts:336-398; shell.ts:1300-1314 |
| composer.takeover.enter | ShellSession.beginTakeover / composer.beginTakeover (no production caller) | shell.ts:1317-1323; composer.ts:137-171 |
| attention.respond.* | takeover Enter → submitComposer → host.submit (operation mode → invoke(destination)) | composer.ts:318-328,357-365; workbench-host.ts:274-290 |
| attention.open / acknowledge | none (attention lines inert) | shell.ts:1099-1106 |
| run.start | none; planned `prime.launch` (declared) | planned-operations.ts:95-116 |
| run.admission.inspect | partial: Inspector AFFORDANCES (unit ops only) | inspector.ts:213-289 |
| run.request_cancel | none; planned `prime.cancel` | planned-operations.ts:158-176 |
| run.reattach | none in UI | — |
| verification.run | none | — |
| evidence.offer | KernelObjectLedger.offerAsEvidence (tests/recovery only); planned `evidence.admit` | evidence-promotion.ts:205-251; planned-operations.ts:177-196 |
| agent.inspect | partial: Agents view via palette | views-chat-trace.ts:214-268 |
| agent.request_stop | none; planned `prime.cancel` | planned-operations.ts:158-176 |
| agent.message | none; planned `prime.message`; composer message → "unsupported" notice | planned-operations.ts:117-136; workbench-host.ts:282-287 |
| kernel.execute | PrimeReplKernel.execute only via KernelInputBroker.respond (tests) | kernel-affordances.ts:528 |
| kernel.interrupt / restart | none in UI (adapter not inspected) | — |
| kernel.object.pin | KernelObjectLedger.pin (no UI) | evidence-promotion.ts:157-159 |
| kernel.object.artifactize | KernelObjectLedger.artifactize (no UI) | evidence-promotion.ts:167-175 |
| case.create / case.open / resource.bind | none | — |
| thread.create / open / message.submit | none | — |
| reference.freeze / validate | none | — |
| wayfinder.* | none | — |
| help.* | none (persistent legend string only) | shell.ts:849-854,1108-1127 |
| app.quit | ShellEffect `quit` (q, BelowMinimum q, ctrl+c) → main quit; no work stopped | shell.ts:458,612,833-834; main.ts:18-21,97-100 |
| search.global | none | — |
| (existing, not in ICM list) unit.resolve / unit.retry / state.refresh | OPERATION_REGISTRY → resolveAffordance → OperationGateway | operation-registry.ts:24-88; affordance.ts:168-317; operation-gateway.ts:273-422 |

## preserve

- P1 `terminalKey` normalizes OpenTUI native `KeyEvent` (escape timeout, modifiers, Unicode). The real PTY test covers standalone ESC, CSI Z, arrows+CR and `界🙂`. `terminal-renderer.ts:15-29`; `production-entry.test.ts:108-134`.
- P2 `handleKey` is a pure `(state, snapshot, key) → {state, effects}` reducer, and its bindings are earned (ICM §34.1). `shell.ts:597-838`; `opentui-shell.test.ts:283-371`.
- P3 `ShellEffect` + `applyShellEffects` form the shell's only outbound edge, statically pinned. `shell.ts:453-496`; `cross-surface-equivalence.test.ts:717-720`.
- P4 Input precedence runs takeover → composer → shell, and BelowMinimum owns input. `shell.ts:1246-1297`.
- P5 Palette two-step confirm revalidates against the fresh entry list, and moving or Esc clears it. `shell.ts:674-702`; `palette-grouping-confirm.test.ts:135-207`.
- P6 Paste is text only and never a command or submission. `shell.ts:1299-1314`; `workbench-host.test.ts:267-291`.
- P7 The composer takeover machine queues by request id, dismisses back to the queue, expires, holds and restores the draft, and honors canonical replacement. `composer.ts:137-304`; `composer-takeover.test.ts:42-275`.
- P8 Grapheme-aware printable check and erase. `composer.ts:306-316`; `composer-unicode.test.ts:4`.
- P9 `resolveAffordance` is pure and deterministic and uses the canonical refusal vocabulary. `affordance.ts:163-326`.
- P10 `materializeAffordances` implements DSN-009 placement (materialized / blocked_with_reason / palette_only). `views-work.ts:99-145`.
- P11 `OperationGateway` serializes decisions and does a fresh fail-closed observe on every drive. It keeps a key/payload mismatch ledger, typed `outcome_unknown`, and `reconcile`. `operation-gateway.ts:191-535`; `deny-bypass.test.ts:245-1178`.
- P12 The registry keeps existing and planned ops separate, with a versioned handshake. `operation-registry.ts:24-103`; `planned-operations.ts:40-289`.
- P13 The kernel affordance set is generated from the registry, invalidates on stale denial and has a single gateway edge. `KernelInputBroker` never prompts directly. `kernel-affordances.ts:210-300,446-535`.
- P14 `KernelObjectLedger` makes pin, artifactize and offer explicit, and offers carry literal-false `establishes_*` fields. `evidence-promotion.ts:110-251`.
- P15 Model focus selection and the hoist stack with one-step return. `workbench-model.ts:550-618`; `horizon-hoist.test.ts:284-301`.
- P16 Geometry classes, per-geometry surface cycle, and per-view scroll preserved across switches. `geometry.ts:30-41`; `shell.ts:511-522,775-787`; `keyboard-accessibility.test.ts:168-191`.
- P17 The Prime dispatcher does canonical `invokeVia` before any spawn. `prime-recursion.ts:472-524`.
- P18 The cross-surface route matrix includes a `shell-effect` route. `testing/cross-surface-matrix.ts:85-93,732-812`.

## replace

- R1 The literal key switch is the command identity. Replace it with a keymap → semantic-id resolver and keep the bindings. `shell.ts:725-837`.
- R2 Help is only a persistent legend string. `shell.ts:849-854,1108-1127`.
- R3 Palette rows are flat description strings, and the overlay is a bordered box with a fixed title. `shell.ts:1129-1180`; `terminal-renderer.ts:143-163`.
- R4 The Work ACTIONS bar is text lines, not ActionControls. `views-work.ts:303-384`.
- R5 The Inspector AFFORDANCES section is a text list. `inspector.ts:213-289`.
- R6 The composer strip is text (mode line, `>` draft, hint, queue). `composer.ts:409-488`.
- R7 The renderer destroys and rebuilds the whole frame each draw with absolute cells and no hit regions. `terminal-renderer.ts:57-104`.
- R8 Submission lifecycle is one global notice string. `workbench-host.ts:256-270`.
- R9 The staged confirm is an inline palette row, not a dialog. `shell.ts:1149-1160`.

## violates

- V1 The palette stages and invokes affordances of **any** status, with no `statusPermitsInvocation` check (`shell.ts:674-702`). Example: `state.refresh` shows `unavailable (not_action_admitted)`, yet two Enters run `host.refresh()` (`workbench-host.ts:252-255`). Clause: ICM §30 (VISIBLE_DISABLED must not activate), §50.3.
- V2 The shell shadows Focus and Horizon in `ShellState.location` (`shell.ts:710,797,812,824`), and consumers fall back to it (`shell.ts:556,807,1037`). A resync calls `model.clearFocus()` (`workbench-host.ts:163-171`), but the shell copy survives. The Inspector, palette actions and `h` then target a vanished unit. Clause: ICM §0 (snapshot→renderer), UESC §6.3.
- V3 Composer Enter empties the draft before any outcome (`composer.ts:381-395`). For message, prime and kernel modes the host then reports "Message was not sent" (`workbench-host.ts:282-289`), so the draft is lost. Clause: UESC §7.6 (DRAFT→SUBMITTING), ICM §30.
- V4 Enter on a takeover completes it locally before any canonical result (`composer.ts:357-365`; `shell.ts:1267-1276`). A refused answer cannot be re-answered because the request is neither re-queued nor expired. Clause: REQ-UX-004 ("revalidate on reply"), ICM §10.3, §5.2.
- V5 The host notice drops gateway `reason_codes` and descriptor. A pre-admission refusal shows only the class code (`PRE_ADMISSION_CLASS`), not the exact gate. `workbench-host.ts:265-270`; `operation-gateway.ts:377-383`. Clause: ICM §32.3.
- V6 Palette results do not expose a semantic command id for views, shortcut or target, and have no title/id split. `shell.ts:238-261,1162-1169`. Clause: ICM §28.
- V7 Shortcuts map straight to state transitions, not to semantic ids. `shell.ts:725-837`. Clause: ICM §34.3.
- V8 The Inspector surface is focusable, but `up`/`down` does nothing (`shell.ts:788`) and the renderer clips its lines (`terminal-renderer.ts:125`). Clipped detail is unreachable. Clause: ICM §37 (static viewport movement), REQ-UX-005.
- V9 In Compact, Navigator and Inspector are not reachable as switchable side surfaces or overlays. The Inspector is never rendered (`shell.ts:511-516,1074-1083`); only focus rows leak into the palette (`:576-582`). Clause: DESIGN Responsive/Compact, ICM §38 Narrow.

## missing

- M1 Semantic command registry, envelope and startup validation (ICM §4, §48, §53). Only the three-op `OPERATION_REGISTRY` exists (`operation-registry.ts:60-88`).
- M2 Mouse input and hit-testing (ICM §35, UESC §0). `main.ts:85`; `terminal-renderer.ts:68-104`.
- M3 `nav.focus.open` as distinct from select (ICM §6.2). `shell.ts:790-805`.
- M4 `nav.history.back`/`forward` with a location history (ICM §6.6, UESC §6.3). Only the hoist stack exists (`workbench-model.ts:611-618`).
- M5 Context menu (ICM §6.4, §29).
- M6 Inspector EMPTY / FOLLOW_SELECTION / PINNED and pin/unpin/toggle (UESC §7.4, ICM §27.4). The Inspector always follows (`shell.ts:1036-1073`).
- M7 Bottom surface, tabs and pane resize (UESC §7.5, ICM §27.1-27.3). `shell.ts:69`.
- M8 ApplicationChrome menus and back/forward (UESC §5.2, ICM §7). The header is one line (`shell.ts:883-907`).
- M9 Trace select, follow/hold with unseen count, load older, filter and search (ICM §16.1-16.8, UESC §16.2, REQ-UX-005). `views-chat-trace.ts:116-159`; `shell.ts:1008-1025`.
- M10 Per-control BUSY_SUBMISSION (ICM §30). `workbench-host.ts:256`.
- M11 Structured reason grammar with class, summary, related_focus and next_affordance (ICM §33). `affordance.ts:52-53`; `inspector.ts:81-108`.
- M12 Composer mode/destination switching in production (REQ-UX-004). `setComposerMode` has only a test caller (`composer-takeover.test.ts:219`); the default is fixed (`composer.ts:89-101`).
- M13 Production trigger for takeovers from Attention or the kernel broker (ICM §10.2). `ShellSession.beginTakeover` has no production caller (`shell.ts:1317-1323`).
- M14 Palette text query and search domains (ICM §28). Typed keys are ignored and `p` closes the palette (`shell.ts:627-636,720-721`).
- M15 `help.open` and `help.shortcuts` overlay (ICM §7.5).
- M16 Canonical ops beyond resolve/retry: run.*, agent.*, verification.run, evidence.offer, case.*, thread.*, reference.*, wayfinder.*. They are planned or absent (`planned-operations.ts:40-47`).
- M17 Kernel UI commands (execute, pin, artifactize, offer). The adapters exist but are not constructed in production; `KernelAffordanceDirectory` is referenced only in tests and the matrix (`testing/cross-surface-matrix.ts:450-492`).
- M18 `attention.open` and `acknowledge` (ICM §21). Attention rows are inert (`shell.ts:1099-1106`).
- M19 `search.global` (ICM §41).

## notes / uncertainty

- There are two gateway instances per host. `PrimeDelegationDispatcher` builds its own (`prime-recursion.ts:363`), while the host builds another (`workbench-host.ts:97`). That means separate idempotency ledgers, which contradicts "one instance per workbench host" (`operation-gateway.ts:187-190`). It is latent, because the dispatcher is not driven from any production input path.
- V2 was derived by reading the code, not by executing it. `opentui-shell.test.ts:326` comments that the fallback ("shell focus falls back to location focus") is intentional.
- The Primary scroll has no upper clamp (`shell.ts:782`), so the view can scroll into blank space.
- Composer Enter on an empty draft still emits a submission (`composer.ts:381-395`).
- Letters are case-sensitive: `P`, `Q`, `H` are unbound.
- `production-entry.test.ts:129` shows a multi-character single write (`界🙂q draft`) landing in the draft. Whether OpenTUI splits it into per-character events or something else handles it was not traced.
- `prime-kernel.ts` interrupt/restart surfaces were not inspected. The `PRE_ADMISSION_CLASS` literal value was not read.
