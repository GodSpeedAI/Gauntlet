# T13 Evidence — Bun PTY/process plus OpenTUI embedded-terminal path

Date: 2026-09-09
Task: T13 (proof level P2, peer confirmation)
Claim: The workbench can host a real interactive terminal with correct lifecycle, input, resize, exit, and provenance on required local targets while untrusted inputs cannot become unbound process or terminal control data.
Prereg: `.agents/research/preregistrations/task-gauntlet-tui-prime-workbench-T13.prereg.yaml` — `just prereg-check` PASS, frozen hash `ca3f05e4…` (unmodified).
Artifacts:
- `workbench/src/terminal-session.ts` — session state machine, DSN-014 sanitizer, privileged-boundary validators, provider port, native Bun providers, shell-wiring seam
- `workbench/src/embedded-terminal.test.ts` — lifecycle, Attack 2, DSN-014 projection, gated real-PTY probe
- `workbench/src/unsafe-input-boundaries.test.ts` — Attack 1 injection matrix + negative control
- `workbench/src/terminal-path.bench.ts` — `just tui-bench terminal-path`
- `justfile` — mechanical `terminal-path` selector (the only justfile change)

## Settled Requirements

- **REQ-BUN-004** — Bun PTY/process facilities qualified as the primary embedded-terminal path BEFORE any alternate stack (T03 admission-ledger discipline). No alternate stack was admitted: the native path passed every required case.
- **REQ-SEC-005** — process argv, paths, terminal automation, and RPC payloads are validated/bound at the privileged boundary; nothing is interpolated into a shell string; path parameters are lexically anchored.
- **REQ-UX-005/006** — terminal lifecycle binds to NONCANONICAL terminal-session state; untrusted text is inert outside the deliberate PTY viewport; input ownership and exit/cancel are explicit and distinct; provenance survives exit and cancellation.
- **DSN-014** — ANSI/OSC control sequences, hyperlinks (OSC 8), and clipboard requests (OSC 52) never escape into the host terminal outside the deliberate PTY viewport; a caret mode renders them as visible TEXT.

## Native-path qualification (REAL probes, T03 ledger discipline)

Ran 2026-09-09, Bun 1.4.0, Linux WSL2. Ad hoc probes plus a gated in-suite probe (`GAUNTLET_T13_REAL_PTY=1 bun test embedded-terminal` — 15/15 pass; the default suite skips it to stay deterministic).

| Capability | Instrument | Result |
| :--- | :--- | :--- |
| PTY spawn + read + write | `Bun.spawn(cmd, { terminal: {…} })`, `/bin/sh -c "echo pty-alive-$((41+1))"` | PTY data callback received `pty-alive-42\r\n`; write via `terminal.write` |
| Kill / exit | `proc.kill()` after first output | exit observed via `proc.exited` = 143 (SIGTERM) in ~2.5 ms |
| Standalone Terminal reuse | `new Bun.Terminal({...})` across two `Bun.spawn({ terminal })` children | both children's output delivered through the one terminal |
| Pipe child lifecycle | `Bun.spawn` with pipe stdio; stdin write + `stdin.end()` | transformed output received; exit code 7 preserved |
| Resize DELIVERY (not just acceptance) | `terminal.resize(90, 40)`; child runs `stty size` | child reported `40 90` — winsize + SIGWINCH reach the child |
| Exit semantics | clean-exit child on a dedicated `Bun.Terminal` | terminal `exit` callback did NOT fire promptly at child exit → the provider keys exit on `proc.exited` and closes the PTY master after reaping |
| In-suite real lifecycle | gated test: spawn → typed input → resize 100x30 → cancel | status `cancelled` (distinct from `exited`), provenance retained, sanitized scrollback free of ESC |

**Honest disclosure — what is real vs faked:**
- REAL (probed above): Bun PTY spawn/read/write/resize/kill, terminal reuse, pipe-child lifecycle, exit-code and signal observation.
- FAKE (deterministic unit tests): all lifecycle/projection/security assertions except the gated probe run on `FakeProcessProvider` behind the `TerminalProcessProvider` port; the port is the only thing an alternate stack would ever implement.
- NOT EXERCISED HERE: `@opentui/core` `EmbeddedTerminalRenderable` instantiation (it requires a live render context/GlyphCache). The session's side of that seam matches its typed API (`write`, `onData`/`onTerminalResize`, `screen()`); visual fidelity inside the deliberate viewport is T18/T20 territory, as are SSH/tmux targets and tmux key conflicts (REQ-UX-005's remote clause).
- NOT CLAIMED: hostile-process sandboxing — a PTY child runs with the user's authority; the boundary binds WHAT is spawned, not what the child may do (T18 carries that disclosure).

## What was built

**`TerminalSession`** (`terminal-session.ts`): validated spawn → live child → exit/failed/cancelled, entirely in noncanonical publication-only state. `project()` returns a frozen `TerminalSessionProjection` (status, title, dims, sanitized scrollback, provenance, neutralized-sequence counts, monotonic generation). Exit (`exited`), signal death (`failed`), and cancellation (`cancelled`) are three distinct outcomes — no boolean conversion exists. Input bytes are forwarded raw into the PTY (deliberate viewport input ownership) but only BYTE COUNTS are recorded; env values are never recorded (keys only).

**DSN-014 sanitizer**: one-shot `sanitizeTerminalOutput` (strip | caret) + stateful `createChunkSanitizer` for chunked PTY output. CSI, OSC (classified OSC 8/OSC 52), DCS/SOS/PM/APC, nF escapes, stray C0/C1 are neutralized with per-class counts; CRLF normalizes to LF. A sequence SPLIT across chunks is carried over — a stateless per-chunk strip leaks the tail of a split OSC (found by test, fixed, regression-pinned).

**Privileged boundary**: `validateSpawnRequest` binds program (metacharacter wall; absolute paths anchored under declared roots; bare names allowlist-able), args (C0/DEL refused; shell metacharacters BOUND AS DATA — argv is never joined into a shell string), cwd (anchored), env (name pattern; control bytes refused), title (neutralized). `anchorPath` is purely lexical (deterministic). `validateRpcPayload` enforces key allowlist, string types, byte caps, and control-neutralization. `bindTerminalAutomation` neutralizes titles. All refusals are typed `TerminalBoundaryError` with closed `locus`/`code` vocabularies.

**Provider port**: `TerminalProcessProvider` with `bunPtyProcessProvider` (qualified native path) and `bunPipeProcessProvider` (T03-qualified pipe shape, stdout feeds the projection). An alternate stack would implement this port only after a failing required native case (none occurred).

**Shell wiring seam**: shell.ts `LOWER_VIEW_PROVIDERS` gates the `terminal` view on a snapshot provider field literally named `terminal`. `publishTerminalProvider(snapshot, projection)` adds that own-property without touching canonical fields; without publication the view stays ABSENT with its inspectable reason (asserted both ways). **No shell.ts edit was needed or made.**

## Teeth (plan-declared attacks)

**Attack 1 — inject shell metacharacters/path traversal through an untrusted parameter** (`unsafe-input-boundaries.test.ts`, 29 tests): 7 program-injection shapes and 4 argv-injection shapes typed-refused or bound-as-data; cwd traversal (`../..`, relative, NUL, outside-roots) refused at the path locus; OSC 52/OSC 8 riding titles, automation, RPC payloads, and child OUTPUT neutralized with counts; env smuggling refused. Negative control live-fires the FORBIDDEN shape (`/bin/sh -c "echo benign; touch <file>"` built by interpolation) proving the attack is real on this platform (marker file IS created), then proves the same untrusted content through the module's bound path executes nothing extra (marker file absent, exit 0, injection visible only as echo's inert data). A mutation-style tooth asserts the metacharacter wall rejects each of `; | & $ ` > < ( )` individually.

**Attack 2 — resize/kill/restart churn while the TUI stays active** (`embedded-terminal.test.ts`): a seeded canonical snapshot is captured, JSON-frozen, then a full churn cycle runs (input, hostile output with OSC 52/8/CSI/DCS, double resize, cancel→SIGTERM, full restart with clean exit). The canonical snapshot stays byte-identical (`JSON.stringify` equal) while the terminal projection's generation advances and its status/scrollback change. Publisher test asserts availability flips only via the provider field and every canonical field is preserved.

## Gate results (verbatim, 2026-09-09)

| Gate | Result |
| :--- | :--- |
| `just tui-test embedded-terminal` | exit 0 — 14 pass, 1 skip (gated real-PTY probe), 0 fail |
| `just tui-test unsafe-input-boundaries` | exit 0 — 29 pass, 0 fail |
| `just tui-bench terminal-path` | exit 0 (below) |
| `just tui-check` | exit 0 |
| `just tui-lint` | exit 0 — 17 warnings, 308 infos (warn-level only; same classes as pre-baseline files: `noConsole` on benches, `useLiteralKeys` where tsc's `noPropertyAccessFromIndexSignature` forces bracket access) |
| `just tui-preflight bootstrap` | PASS |
| `cd workbench && bun test` | 322 pass, 1 skip, 0 fail across 38 files (187 pre-existing + 44 T13 + parallel-wave tasks' suites) |
| `GAUNTLET_T13_REAL_PTY=1 bun test embedded-terminal` | 15 pass, 0 fail |
| `just prereg-check gauntlet-tui-prime-workbench-T13` | PASS (hash ca3f05e4…, unmodified) |

Note: during this builder's run the tree-wide `tui-check`/`bun test` gates transiently failed inside OTHER wave builders' in-flight files (`plugins.ts`, `plugin-lifecycle.test.ts`, `domainforge-adapter.test.ts`, `semantic-identity.test.ts`, `kernel-affordances.test.ts`, `detach-reattach.test.ts`); per the wave protocol they were retried after ~30–120 s and all cleared above. At no point did any of those failures originate in a T13 file (verified per-file each time).

## Bench (`just tui-bench terminal-path`, frozen fixtures, deterministic LCG buffers)

```
[sanitize:strip] 8.00MiB hostile text: median 73.20ms 109 MiB/s
[sanitize:caret] 8.00MiB hostile text: median 980.72ms 8 MiB/s
[session-ingest] 2000×4096B PTY stream (7.8MiB): median 112.90ms 69 MiB/s
[projection] full 1000-line project(): p50 0.313ms p95 1.040ms
[resize-churn] 100 resize→project cycles: p50 0.024ms p95 0.073ms
[real-child] Bun.spawn echo round trip: p50 4.32ms
```

Read: the default ingest path (sanitize→scrollback→projection) sustains ~69 MiB/s — orders of magnitude above any real PTY stream; resize churn and projection rebuild are sub-0.1 ms / sub-1 ms — the render path is never the bottleneck. Caret mode (~8 MiB/s) is the opt-in forensic renderer, not the ingest path.

## Needed wiring (for the orchestrator; not owned by T13)

1. `workbench/src/index.ts`: add `export * from "./terminal-session";` (index.ts was not a T13-owned file).
2. Host loop wiring: hold the live `TerminalSession` beside the `ShellSession` and call `publishTerminalProvider(snapshot, session.project())` where snapshots are handed to `renderShell` — this alone makes the terminal lower surface AVAILABLE through shell.ts's existing provider gate. Unpublishing (or child absence) returns it to ABSENT with reason.
3. The OpenTUI viewport itself: route `bunPtyProcessProvider`'s raw chunks to an `EmbeddedTerminalRenderable` instance (its `write`/`onTerminalResize` map 1:1 onto the session handle) when T18/T20 build the live viewport view; the session keeps the safe projection for every non-viewport surface in the meantime.

## Honest gaps / limits

- Fidelity inside the deliberate viewport (VT parsing, mouse, alt-screen) is OpenTUI's, unexercised here — no rendered-viewport test exists (needs T18/T20 render context).
- Scrollback is a TEXT projection: lines are `\n`-split; no cell-width/CJK/combining folding at this layer (REQ-UX-005's Unicode cell-width clause remains T18's viewport concern).
- `exited` covers any self-declared exit code (including nonzero) by design; only signal deaths are `failed`.
- Session ids are process-unique counters, not content-derived; projections are in-memory only (nothing persists — terminal scrollback deliberately never becomes durable evidence).
- Single active child per session; multi-session fan-out/pooling is unexercised.
- Caret-mode sanitizer throughput is unmetered beyond ~8 MiB/s; fine for forensic display, would need work for bulk forensic export.

## What the peer should attack

- The chunk-boundary sanitizer: try adversarial splits of OSC/DCS payloads, `ESC` at chunk edges, `BEL`-terminated vs `ST`-terminated mixes, and C1 bytes inside sequence bodies — the regression suite covers the found cases, not all edges.
- `publishTerminalProvider` snapshot immutability and the Attack-2 byte-identity under heavier churn (mid-churn snapshot reads, concurrent sessions).
- The real-PTY cancel/exit race: cancel during output bursts, kill before first read, double cancel.
- The bound-as-data posture for argv metacharacters: attempt to construct a provider call that re-joins argv (the type surface makes it awkward; try anyway), and try injection through `title`/RPC values that survive sanitization as text.
- The negative-control live-fire test writes fixed `/tmp` paths (`gauntlet-t13-attack1-marker`, `/tmp/gauntlet-t13/`); confirm no cross-run interference.

Confirmation: BUILDER evidence pending peer confirmation.
