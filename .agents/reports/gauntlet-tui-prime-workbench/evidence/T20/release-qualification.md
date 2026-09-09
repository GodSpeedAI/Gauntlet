# T20 Evidence — Linux/WSL same-host, SSH/tmux terminal, and release qualification

Date: 2026-09-09
Task: T20 (proof level P2, confirmation: peer)
Claim: Local and remote/SSH modes preserve operation meaning, Focus/resize/input/reconnect behavior, and long-running work continuity; the release form runs on actual target hardware with documented dependencies.
Prereg: `.agents/research/preregistrations/task-gauntlet-tui-prime-workbench-T20.prereg.yaml` — `just prereg-check gauntlet-tui-prime-workbench-T20` PASS, frozen hash `4eabe7b870b7463f8670aa50bb5264cd0177d77ec8d01d661565cc40dde000e9` (byte-identical this session; never modified).
Environment: Bun 1.4.0, Linux WSL2 (6.18.35.2-microsoft-standard, Ubuntu-26.04), tmux 3.6, ssh client present / sshd NOT reachable.
Artifacts:
- `workbench/src/workbench-model.ts` — T17 planner-swap wiring (obligation 1): MaterializationFoldCoordinator into the production snapshot path
- `workbench/src/inspector.ts` — T14/T14B lens wiring (obligation 2): `renderSemanticSection` + `renderSxrEvidenceSection` appended to the live section list, data-source-gated, plus `publishLensData` (read-only publication seam mirroring T13's `publishTerminalProvider`)
- `workbench/src/planner-swap.test.ts` — 9 regression tests for the wired plan path
- `workbench/src/inspector-lens-wiring.test.ts` — 6 wiring tests (gate, publication, live shell path)
- `workbench/src/transport-equivalence.test.ts` — Attack 1: config/operation/projection axes across local and remote-representative modes
- `workbench/src/ssh-target.test.ts` — SSH probe + honest gated leg; real tmux leg; tmux client-death/reattach; T16-shaped terminal-restart-mid-run
- `workbench/src/operator-journeys-pty.test.ts` — env-gated (`GAUNTLET_T20_REAL_PTY=1`, T13 pattern) real-PTY operator journeys at 120x40 / 80x24 / below-minimum
- `workbench/src/main.ts` — the workbench entry (release form's host loop): real `WorkbenchModel` + `ShellSession` driven from the host terminal; cold start is honest (REQ-UX-001); non-TTY renders one frame and exits 0
- `workbench/src/release-manifest.ts` + `release-manifest.test.ts` — release form: pinned-target build + real-PTY launch qualification + manifest write/verify
- `justfile` — `tui-build release` implementation (replaces the NOT_IMPLEMENTED stub)
- Evidence frames: `evidence/T20/frames/*.txt` (8 rendered captures, incl. the release artifact's own launch frame)

## Both tracked obligations discharged

### 1. T17 planner swap (WorkbenchModel × MaterializationFoldCoordinator)

`WorkbenchModel` accepts an optional `foldExecutor` (workbench-model.ts). With one configured, `getSnapshot()` routes materialization planning through `currentPlan`:

- **Snapshot served from cache on interaction**: the last worker-accepted plan is served only while it is valid for the EXACT current context — same active generation, same horizon (`plan.horizon_id`), same world version (`planVersion`, bumped by `applyEvent`/`startGeneration`/horizon changes via `invalidatePlan()`). Focus selection and repeated reads in a stable context re-dispatch nothing.
- **Refreshed plan lands on completion**: the first read on a stale context folds INLINE (immediate fallback — the model never blocks rendering on the worker) and schedules an asynchronous `coordinator.submit(...)`; the outcome handler re-admits against the LIVE generation (coordinator's own invalidation contract) and re-publishes to subscribers.
- **Never regresses**: stale/superseded results are discarded (generation, task, horizon all checked); a wedged executor can only freeze the refresh, never the render path (in-flight guard clears on context change; inline keeps serving fresh folds).
- Default model (no executor) is byte-identical to pre-T20 behavior (pinned by test).

Regression suites (`planner-swap.test.ts`, 9 pass): byte-identical default; first-read inline + exactly-one scheduled refresh; cache-served interactions with zero re-dispatch; world-change invalidation + re-refresh; listener re-publication on admission; stale-result discard; never-answering worker never blocks; hoist invalidates whole-Case plan (horizon_id pins context); real `BunWorkerFoldExecutor` end-to-end (first read synchronous, worker plan lands and equals the reference inline fold with interned identities). T17's `worker-folds.test.ts` stays 18/18 green — the coordinator's semantics were not modified.

### 2. Inspector lens wiring (T14/T14B sections into the live renderInspector list)

`renderInspector` now appends `renderSemanticSection(snapshot.semantic)` and `renderSxrEvidenceSection(snapshot.sxr_lens, snapshot.sxr_paths ?? [])` between PROVENANCE and AFFORDANCES — strictly DATA-SOURCE-GATED: sections appear only when the snapshot CARRIES the projected lens data (published via `publishLensData`, an own-property publication that leaves canonical fields untouched); no lens carried → no section, never a fabricated one. Because the gate reads the snapshot itself, the live shell path (`shell.ts` renderInspector call) picks the sections up with NO shell.ts edit.

Wiring suites (`inspector-lens-wiring.test.ts`, 6 pass): bare snapshot keeps the exact pre-existing section list; published ABSENT lenses render typed absent sections at the wired position (empty records); a projected lens reaches the live list with its repo identity; the REAL pinned DomainForge capture (same bytes T14B pins) renders model identity + concept records; publication preserves canonical fields; the shell's Inspector surface renders `SEMANTIC`/`REALITY TRACE` only when carried. DSN-008 record shapes and T14B's `semantic-evidence-composition` suite stay green (10/10).

## Transport equivalence (Attack 1; prereg P1/D1)

`transport-equivalence.test.ts` (8 pass). Deployment shape: SSH/tmux operates the SAME host — the same loopback HTTP surface (`gauntlet serve`, T02's config authority) driven from an SSH-session terminal. The transport difference is represented at the config/client layer only:

- Config axis (REQ-CONFIG-005): loopback endpoints classify `local`, other hosts `remote`, explicit env override wins; the SAME loopback surface re-declared as remote yields a config byte-identical in every field except `transport`; operation vocabulary and snapshot JSON carry no transport concept.
- Operation axis: one `unit.resolve` (allowed), one `unit.retry` (refused), one `state.refresh` (refused `not_action_admitted`) each driven through the in-process local route AND the real HTTP-loopback route (`CanonicalSurfaceStub.serveHttp` + `HttpOperatorActionRoute` — a distinct client process boundary, the surface an SSH-session terminal drives): identical operation identity, admission decision, refusal codes, result facts, and deterministic idempotency key. A refused key replayed through the other mode's gateway neither arms nor poisons.
- Projection axis: one authoritative log folded locally (arrival order) vs through the canonical orderer (the fresh-client reconstruction path) yields byte-identical snapshots.

Result: no semantic divergence found between modes — prereg P1 supported within the declared scope (same host, loopback surface; see honest gaps for the non-executed real-`sshd` leg).

## SSH/tmux reality (what ran vs what could not)

Probe results (recorded in-suite, `ssh-target.test.ts`):

| Target | Reality this session |
| :--- | :--- |
| `ssh localhost` (BatchMode, ConnectTimeout=3) | FAILS — exit 255, `ssh: connect to host localhost port 22: Connection timed out` (no reachable sshd; `systemctl is-active ssh` = inactive, nothing listening on :22) |
| tmux 3.6 | AVAILABLE — fully exercised |

What RAN for real:

1. **Real tmux leg**: the workbench entry (`src/main.ts`) under a real tmux session at 120x40 — the shell renders (NAVIGATOR/PRIMARY/INSPECTOR/COMPOSER, `Gauntlet` header); `tmux resize-window` to 80x24 → compact layout (SIGWINCH reached the process; Navigator gone, Primary retained); to 60x18 → the recoverable prompt (`Gauntlet needs at least 80x24`, `press q to quit`); `q` exits with recorded exit code 0.
2. **Client death / fresh-client reattach through the tmux server**: a second client session grouped with the first is KILLED — the workbench process keeps running (same `pane_pid` before/after); a fresh grouped client attaches and observes the same live pane state from the tmux server (authoritative, not client memory).
3. **Terminal restart mid-run with long-running Prime work (prereg D2, T16 shapes)**: the client detaches (receipt `cancel_calls_issued` typed 0), authoritative events and a new Prime child arrive while no TUI exists; the fresh client reattaches — Prime topology from qualified records (exact ids/parent links, no liveness inference), streams reconciled since the persisted cursors, adapter call log READS ONLY, fresh projection equals the authoritative reference fold (`projectionDifferences` = []).

What could NOT be exercised (visible gated skip, never faked): the real `ssh localhost` leg — driving the workbench under tmux THROUGH an actual SSH session — because sshd is unreachable on this host. The skip names the exact probe failure in the test name; enabling sshd (`sudo systemctl start ssh`) and re-running `just tui-test ssh-target` executes it unchanged.

## Real-PTY operator journeys (REQ-UX-005/007 — the T12 deferral)

`GAUNTLET_T20_REAL_PTY=1 bun test src/operator-journeys-pty.test.ts` — 3 pass, 0 fail (default suite: 3 visible skips). The REAL workbench entry in a REAL Bun PTY, keyboard-only:

- **120x40 FULL**: cold start renders the three-surface layout with the honest empty state (`No authoritative source has …`, REQ-UX-001 — cold launches no work); Tab cycles Primary → Inspector → Composer → Navigator with the rendered focus marker following; `2`/`3`/`1` switch PRIMARY [CHAT] → [TRACE] → [WORK]; resize to 80x24 → compact (Navigator gone, Case+health retained); to 60x18 → prompt; `q` exits 0 with terminal modes restored (show-cursor sequence in output).
- **80x24 COMPACT**: one primary pane, honest health line (`0 units · ! 0 blocked`), composer strip; `q` exits 0.
- **Below minimum (60x18)**: launch lands directly on the recoverable prompt naming current and required size; only `q` acts; exit 0, modes restored.

Frame captures (gate-set runs): `frames/journey-120x40-{full-cold,navigator-focused,work-view}.txt`, `frames/journey-80x24-{cold-compact,compact}.txt`, `frames/journey-60x18-{launch-prompt,below-minimum}.txt`.

## Release form (REQ-BUN-005): `just tui-build release`

The recipe builds the workbench entry as a standalone executable (`bun build --compile --target=bun-linux-x64 src/main.ts` — the form actually qualified on this host; `--compile` IS available in Bun 1.4.0), LAUNCHES the artifact in a real PTY at 120x40, asserts the rendered workbench shell, quits it via the keyboard, writes the launch frame to `evidence/T20/frames/release-artifact-120x40.txt`, and writes + verifies `workbench/dist/release-manifest.json`. Exit 0 requires artifact + valid manifest + clean launch.

Recorded build (final closure run, 2026-09-09T21:20:08Z):

| Field | Value |
| :--- | :--- |
| Artifact | `workbench/dist/gauntlet-workbench` (82,670,792 bytes, executable) |
| sha256 | `ec110d785fc5b96f2f425a6e2bf79b40e58c39628616c998e0bd958eaf5d994c` |
| Entry / target / form | `src/main.ts` / `bun-linux-x64` / `bun-compile-standalone` |
| Bun | 1.4.0 |
| Lockfile | `bun.lock` sha256 `90268e15dc32f9f18a6c743035b3549f26970300685113e2dc4981344e100c95`; all deps pinned exact (bootstrap preflight PASS) |
| Dependencies (documented) | `@opentui/core` 0.5.11, `@opentui/react` 0.5.11, `react` 19.2.8, `react-reconciler` 0.33.0 (+4 pinned devDeps: biome 2.5.12, @types/bun 1.4.2, @types/react 19.2.8, typescript 7.0.2) |
| Launch qualification | real PTY 120x40; rendered markers Gauntlet/NAVIGATOR/PRIMARY/INSPECTOR/COMPOSER; exited cleanly, code 0 |

`release-manifest.test.ts` (5 pass) checks the manifest contract at the ts level (schema, digest formats, launch-record requirements, support facts, on-disk artifact digest verification — drift fails).

Target-specific limitations are recorded as configuration/support facts in the manifest (`support_facts`) — compile form choice, structural-shell renderer scope, WSL distribution, tmux availability, sshd unavailability — never as domain behavior.

## Gate results (verbatim, 2026-09-09)

| Gate | Result |
| :--- | :--- |
| `just tui-test transport-equivalence` | exit 0 — 8 pass, 0 fail |
| `just tui-test ssh-target` | exit 0 — 5 pass, 1 skip (real-`sshd` leg; probe failure named in the skip), 0 fail |
| `just tui-test operator-journeys` | exit 0 — 10 pass (T12) + 3 gated skips (T20 PTY legs), 0 fail |
| `just tui-test semantic-evidence-composition` (plan T20 gate) | exit 0 — 10 pass, 0 fail |
| `just tui-build release` | exit 0 — artifact + manifest + real-PTY launch qualification (details above) |
| `just tui-check` | exit 0 (tsc --noEmit clean) |
| `just tui-lint` | exit 0 — 34 warnings, 528 infos (warn-level only; `useLiteralKeys` where tsc's `noPropertyAccessFromIndexSignature` forces bracket access, `noConsole` on benches — same classes as the pre-existing baseline) |
| `just tui-preflight bootstrap` | exit 0 PASS (pins, lockfile sync, audit, spec/DESIGN hashes) |
| `cd workbench && bun test` | exit 0 — 526 pass, 5 skip, 0 fail across 51 files, 5821 expect() calls |
| `cd workbench && GAUNTLET_T20_REAL_PTY=1 bun test` | exit 0 — 529 pass, 2 skip (T13's real-PTY probe under its own gate + the T20 SSH leg), 0 fail |
| `GAUNTLET_T20_REAL_PTY=1 bun test src/operator-journeys-pty.test.ts` | exit 0 — 3 pass, 0 fail |
| `bun test src/planner-swap.test.ts src/release-manifest.test.ts src/inspector-lens-wiring.test.ts` | 20 pass, 0 fail |
| `just prereg-check gauntlet-tui-prime-workbench-T20` | PASS — hash 4eabe7b8… unmodified |

During this builder's run the tree-wide gates transiently failed inside the parallel T19 builder's in-flight files (`headless-replay.test.ts` tsc error; `headless-replay.ts`/`renderer-boundary.test.ts` lint diagnostics); per the wave protocol they were retried after intervals and all cleared above. No T20-owned file caused a cross-file failure.

## Honest gaps / limits

- **The real-`sshd` SSH leg did not run**: sshd is unreachable on this host. Everything SSH-mode-shaped ran under real tmux on the same host (the deployment shape the prereg declares: SSH/tmux operates the SAME host and the same loopback surface), but an actual SSH transport (latency, PTY passthrough, tmux key-conflict behavior across an ssh channel, TERM negotiation) is UNQUALIFIED here. One gated test executes it unchanged once sshd is reachable.
- **The release artifact renders the structural ShellSession tree, not an OpenTUI React viewport** (T13's recorded deferral; the embedded `@opentui/core` viewport remains later-task wiring). The declared OpenTUI dependencies are pinned in the manifest's dependency list but are not imported by the release entry — recorded as a support fact, not hidden.
- **The release entry is a cold-start host loop**: it does not bind a live `gauntlet serve` endpoint (REQ-UX-001 honesty: it launches no work and reports the not-yet-observed surface). Affordance invocation is not routed to the operation gateway from the entry; the read-only inspection slice holds.
- **Worker-fold placement is opt-in per host decision** (`foldExecutor`); the release entry runs the cold model where the T17-measured decision is inline. The production wiring and its regression proof live in `planner-swap.test.ts`; no LARGE-fixture interactive benchmark was re-run in T20 (T17's frozen budgets remain the measurement authority).
- **Bracketed IPv6 endpoints classify `remote`** on this Bun (URL.hostname keeps brackets) — recorded as a T02 config support fact; not pinned by a test and not "fixed" (T02's file).
- **tmux resize delivery was qualified at the layout level** (rendered compact/below-minimum transitions), not by reading `stty size` inside the child as T13 did for the raw PTY.
- `bun build --compile` output is ~83 MB (embedded runtime) — recorded as a packaging fact; no size budget was frozen for this task.

## What the peer should attack

- The planner-swap cache: try to make `getSnapshot` serve a plan for a superseded world (event during fold flight, horizon swap between submit and accept, generation advance mid-refresh) — the version/horizon/task triple is the invariant to break; also try to wedge the refresh loop via an executor that errors after install.
- The lens gate: publish a lens shaped to crash a renderer (wrong-kind enrichment), publish then retract between render passes, verify canonical snapshots stay byte-identical under publication, and check the shell's DSN-008 record output when a lens section shares ids with base sections.
- Transport equivalence: attempt per-transport semantic drift through `HttpOperatorActionRoute` (header smuggling, partial reads) and try to make the idempotency key diverge across modes.
- The tmux orchestration in `ssh-target.test.ts`: session-name collisions across parallel runs, the session-group reattach semantics under tmux versions < 3.2 (`resize-window -x/-y`), leftover servers if a test is interrupted mid-leg.
- The release artifact: run `dist/gauntlet-workbench` under non-x64 or musl targets (expect an honest failure — single-target build), SIGWINCH storms mid-frame, and the manifest digest after ANY source touch (verify it fails).
- The PTY journeys write frames into the evidence tree when gated — confirm a gate-set CI run cannot be used to overwrite unrelated evidence paths.

Confirmation: BUILDER evidence pending peer confirmation.

## Confirmation findings (peer verdict: CONFIRM — 0 blocking / 0 major / 4 minors, 2026-09-09)

### MINOR-1 — evidence honesty: mislabeled work-view journey frame — FIXED this round

**Finding (peer):** `evidence/T20/frames/journey-120x40-work-view.txt` actually rendered `PRIMARY [TRACE]`:
`untilOutput(session, "PRIMARY [WORK]")` matched STALE ACCUMULATED output — the cold default view is
already `[WORK]`, so the marker was present in the byte stream from the first frame and the wait
returned before the "1" press redrew; the capture then took `lastFrame` (still the [TRACE] frame).
The [CHAT]/[TRACE] transitions were genuine (first-occurrence markers) and were kept.

**Fix** (`workbench/src/operator-journeys-pty.test.ts`): added `untilFrame`, which waits on the
STRIPPED FRESH last-drawn frame rather than the accumulated stream, and moved EVERY
interaction-driven wait (surface cycling, all three view transitions, resize, prompt, cold-start
markers) onto it. Accumulated `session.output()` is now consulted only where the fact genuinely
lives in the stream — the exit-mode restoration check (`\x1b[?25h` arrives after the final frame).
The stale-matching helper `untilOutput` was removed entirely, so this failure class cannot silently
recur in this file. A new `journey-120x40-chat-view.txt` capture pins the [CHAT] transition explicitly.

**Verification (re-captured 2026-09-09):** `GAUNTLET_T20_REAL_PTY=1 just tui-test operator-journeys-pty`
— 3 pass / 0 fail; `journey-120x40-work-view.txt` now contains `PRIMARY [WORK]` and ZERO occurrences
of `[TRACE]`/`[CHAT]`; `journey-120x40-chat-view.txt` contains `PRIMARY [CHAT]`; the compact capture
contains no `NAVIGATOR`. The [WORK] transition is genuinely pinned by a fresh-frame assertion.

### MINOR-2 — TRIGGER-RECORD (not fixed): lens render gate is presence-only

The T20 lens wiring gates on the PRESENCE of published lens data on the snapshot; it does not
re-validate the SHAPE of a published PROJECTED value. A malformed PROJECTED shape would throw a
TypeError inside `renderInspector` (fail-fast, in-process, at the render boundary — no silent
corruption), and every producer path that builds these values (`parseSemanticEnvelopeDocument` /
`projectSemanticEnrichment`, `parseSxrLedgerExport` / `projectSxrEvidenceLens`) is adapter-validated
upstream. Before any future trust boundary publishes lens data from unvalidated input, a typed shape
guard is required at the publication seam (`publishLensData`) so the failure locus is a typed
refusal rather than a render-time throw. Recorded for that future task; T20's delivered gate scope
(host-published, adapter-projected values) is unchanged.

### MINOR-3 — TRIGGER-RECORD (not fixable in T20): REQ-UX-007 final-acceptance clause is partially future

REQ-UX-007's final-acceptance clause adds full requirement/evidence-content/current-denial inspection
plus resolve/retry, delegation, detach/reconnect and optional semantic/evidence navigation in a real
PTY against the canonical boundary. T20's delivered real-PTY slice is the honestly cold read-only
host loop: geometry floors, keyboard navigation, view/surface transitions, prompt behavior and clean
exit are REAL-PTY-qualified; live resolve/retry/delegation journeys against a BOUND endpoint are not
(the release entry deliberately launches no work and contacts no authoritative source — REQ-UX-001
honesty, prereg D3's declared scope). State-level coverage for those journeys exists (T09 gateway
suites, T12 operator-journeys, T16 detach/reattach); the missing piece is only their real-PTY
execution against a live `gauntlet serve`. **Note for T25's settle-list**: bind the entry to a live
endpoint and replay the operator journey set in a real PTY.

### MINOR-4 — TRIGGER-RECORD (not fixed): planner-swap refresh guard is not outcome-precise

`WorkbenchModel`'s fold outcome handler unconditionally clears `refreshInFlight` on ANY outcome. A
STALE DISCARDED outcome (superseded generation/task arriving while a newer context is already
in-flight) therefore releases the guard one round early: the next snapshot read can schedule a
duplicate fold of the same context. The coordinator's invalidation contract discards the loser, so
the consequence is bounded to one wasted fold — no stale plan can ever be served, no state corruption.
Additionally, `notifySubscribers` fires for APPLIED-but-version-invalid plans (accepted, then
invalidated by a concurrent context change before the next read); listeners re-read and the version
check serves the inline fold, so the extra notification is harmless. A precise fix would clear the
guard only when the outcome's task id is the latest dispatched task for the current context; recorded
as a bounded-efficiency follow-up, not a correctness defect.

### Confirmation-round housekeeping note (recorded for transparency)

Regenerating the manifest for the MINOR-1 re-run surfaced a second-order interaction: biome's VCS
integration (`files.useIgnoreFile`) scans `workbench/dist/`, so the freshly written
`dist/release-manifest.json` failed `just tui-lint`'s format check. Fix: `workbench/.gitignore`
(tracked since T00, content `node_modules/`) gained one additive entry — `dist/` — the repo's own
designed mechanism for excluding generated output. During this edit the tracked file was initially
overwritten instead of appended; the original content was recovered byte-exactly from HEAD (the file
carried no prior uncommitted work — the overwrite was the only modification), and `git diff` now
shows exactly the three added lines. AGENTS.md's check-before-overwrite rule now has a fresh
instance for the failure log.

Post-fix confirmation gates (verbatim, 2026-09-09):

| Gate | Result |
| :--- | :--- |
| `GAUNTLET_T20_REAL_PTY=1 just tui-test operator-journeys-pty` | exit 0 — 3 pass, 0 fail (fresh-frame assertions; frames re-captured) |
| `just tui-lint` | exit 0 — 34 warnings, 528 infos (warn-level only), 102 files (dist/ ignored) |
| `just tui-check` | exit 0 |
| `cd workbench && bun test` | exit 0 — 526 pass, 5 skip, 0 fail across 51 files, 5821 expect() calls |
