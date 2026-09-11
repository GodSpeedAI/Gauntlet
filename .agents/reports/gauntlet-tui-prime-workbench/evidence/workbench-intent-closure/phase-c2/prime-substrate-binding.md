# Phase C2 — bind the real Prime substrate

Plan: `.agents/plans/workbench-intent-closure.md` §4 Phase C2.
Date: 2026-09-10. Prior phases: A (SSH leg repair), B (projection integrity), C1
(planned-operation declarations).

## What changed

- **`prime-host.ts`**: extracted `PrimeHostPort` — the 5-method interface (`spawnChild`,
  `receiveMailboxMessage`, `deliverResultContract`, `getSession`, `listSessions`)
  `PrimeDelegationDispatcher` actually drives. `PrimeHostAdapter` now `implements
  PrimeHostPort` and is explicitly documented as the in-memory TEST DOUBLE (D2) — its
  public shape and behavior are byte-for-byte unchanged; every existing test that
  constructs `new PrimeHostAdapter()` directly still passes unmodified.
  `PrimeSpawnRequest` gained an optional `task` field (real hosts need the goal text to
  launch a process; the fake ignores it) — additive, threaded from
  `PrimeDelegationRoute.perform()`'s existing mailbox-message call.
- **`real-prime-host.ts`** (new): `RealPrimeHost implements PrimeHostPort` — the production
  adapter over `Bun.spawn`. Composition, not reimplementation (D2): all session/mailbox/
  contract bookkeeping delegates to an internal `PrimeHostAdapter`; this file's own code is
  only what the fake never had — launching `prime-agent` (bound through the T13
  `validateSpawnRequest`/`anchorPath` program/argv/cwd boundary, `terminal-session.ts`) and
  a background task per child that parses the real newline-JSON event protocol (proven by
  T24's own gated battery: `{type:"session",...}` / `{type:"message_end",
  message:{role:"assistant",...}}`) into `receiveMailboxMessage`/`deliverResultContract`
  calls on that same bookkeeping. `spawnChild()` stays synchronous and nonblocking
  (REQ-PRIME-003) — the real process's stdout is never read before the handle returns.
- **`config.ts`**: additive `prime_binary`/`prime_socket` fields (REQ-CONFIG-003,
  REQ-BUN-005), same CLI→env→file→default precedence idiom as `prime_endpoint`.
  `prime_socket` is declared but not yet consumed by `RealPrimeHost` (D1: an honest
  declaration is not itself a claim of use) — `RealPrimeHost`'s first production binding
  uses one-shot per-child `Bun.spawn`, not a persistent daemon; the socket-based path is
  future work this field's presence documents without over-claiming.
- **`preflight.ts`**: the Prime "configured" check and the runtime probe now honor
  `prime_binary` (probed via `substrate-inventory.ts`'s `probePrimeSurface`, a real
  `--version`/`--help` spawn) in addition to the existing `prime_endpoint`/`/health` HTTP
  path — closing the mismatch the plan named (`preflight.ts` previously only knew how to
  probe an HTTP endpoint, while T24 qualified the real substrate as a binary).
- **`workbench-host.ts`**: `primeDispatcher: PrimeDelegationDispatcher | null` — constructed
  with a real `RealPrimeHost` only when `config.prime_binary !== null` (D6), sharing the
  SAME `HttpOperatorActionRoute` the read/write paths already use. The observation's
  `capabilities` array gains an honest `{capability: "prime", installed: true, gates: {...
  all false}}` row when configured — never claiming reachability the poll loop never
  probed (tooth 6). Composer-level `prime`/`message`/`kernel` submission ROUTING through
  this dispatcher (full envelope construction from a live composer submission) is
  explicitly **not** built this pass — see "Explicitly not closed" below.
- **`substrate-inventory.ts`**: the `child_session_admission` row's `provenance_note`
  records that `RealPrimeHost` now performs canonical-dispatch-then-real-spawn in
  PRODUCTION code, not only in T24's standalone test battery.
- **`security-hardening.ts`**: new `PRIVILEGED_BOUNDARY_INVENTORY` entry
  `real.prime.host.spawn` (D3) — the real `Bun.spawn` boundary, distinct from
  `prime.delegation.spawn` (canonical-dispatch ordering, unchanged) and
  `terminal.spawn.request` (TerminalSession's own spawn boundary).
- **`scripts/tui-preflight.sh`**: a new gated row for `GAUNTLET_C2_REAL_PRIME`, and the
  env-gate sweep's regex widened from `GAUNTLET_T[0-9]+_` to `GAUNTLET_[A-Z][0-9]+_` — see
  "The unbacked-gate sweep gap" below.

## The mechanism decision (confirmed with the operator before implementation)

Two open design questions were resolved before writing `RealPrimeHost`, since the plan text
underspecified them and this is the plan's highest-risk phase (real process spawning, real
credential handling):

1. **How does `RealPrimeHost` learn about mailbox/result activity after `spawnChild()`
   returns synchronously?** → a background poll/watch task per child (not a synchronous
   drain, not deferred to a later pass) — `spawnChild()` mints the handle and returns
   immediately; a `void this.watch(...)` task awaits the real process's stdout/exit and
   folds observed events into the SAME bookkeeping the fake uses.
2. **Build and run the gated real-substrate battery this pass, including real credential
   handling?** → yes, following T24's exact isolation pattern.

## The nine field-worth of D1 discipline carried into `RealPrimeHost`

- **program identity** — `this.binary`, validated ONCE at construction from
  `config.prime_binary` via the FULL `validateSpawnRequest` program-locus check (never
  `anchorPath` alone — see the first RED round below); never re-resolved per call, so no
  caller can ever substitute a different binary through `PrimeSpawnRequest` (it names no
  program field at all).
- **argv** — fixed, self-constructed (`--mode json -p <task> --offline --no-tools -ne -ns
  -nc --session-dir ... --cwd ...`), the exact invocation T24's own battery already proved
  against the real binary; task text is bound as an argv DATA element, never shell-parsed.
- **env** — constructed directly (not through `validateSpawnRequest`'s env validator, which
  refuses ANY path-shaped value outright — correct for terminal sessions whose env can
  arrive from untrusted RPC input, wrong for this host's self-constructed, non-adversarial
  `PRIME_AGENT_CODING_AGENT_DIR`); `LD_*`/`DYLD_*` stripped unconditionally as defense in
  depth, exported as the pure, directly-testable `buildRealPrimeEnv`.

## The RED rounds (preserved, D4)

1. **Constructor validated the binary path with `anchorPath` alone.** `anchorPath` does not
   check for shell metacharacters (that check lives inside `validateSpawnRequest`'s
   program-locus validation) — a construction-time check calling it directly would have
   silently accepted an injected `;`/`|`-bearing path. Caught by
   `real-prime-host.test.ts`'s own "refuses a binary path carrying a shell metacharacter"
   tooth failing to throw. Fixed by routing construction through the FULL
   `validateSpawnRequest` check instead.
2. **`Bun.spawn` with a `cwd` that does not yet exist on disk fails — confusingly, as an
   ENOENT blamed on the PROGRAM path, not the cwd.** `RealPrimeHost`'s constructor validated
   paths but never created `agentDir`/`sessionDir`/`workRoot` on disk, relying on caller
   discipline (only true in production, where `WorkbenchHost` pre-created them). Debugged
   directly (`existsSync` true immediately before `Bun.spawn`, manual `execSync` of the
   identical script succeeded, yet `Bun.spawn` itself ENOENT'd) down to the missing `cwd`.
   Fixed: the constructor now `mkdirSync`s all three directories itself — the class is
   self-sufficient, never dependent on caller discipline for its own working directories.
   (The now-redundant `mkdirSync` calls in `WorkbenchHost`'s constructor were removed.)
3. **My own test wrote real evidence files into the tracked repo tree.** The first version
   of `prime-delegation-real.test.ts` didn't supply an explicit `contract_path`, so
   `PrimeDelegationDispatcher`'s default (`.agents/reports/.../T10/contracts/`) fired, and
   `RealPrimeHost.watch()` genuinely `Bun.write`s to it — three stray JSON files landed
   under `workbench/.agents/reports/...` and were caught by the NEXT `bun run lint`'s
   formatter check. Confirmed untracked (`git status`), deleted, and every delegation call
   in that test file now supplies an explicit `contract_path` under its own temp root.
4. **The env-gate sweep silently missed the new gate.** `scripts/tui-preflight.sh`'s sweep
   matched only `GAUNTLET_T[0-9]+_...` names; `GAUNTLET_C2_REAL_PRIME` (this plan's own
   Phase-lettered vocabulary, not task-numbered) doesn't match, so `just tui-preflight
   conformance` passed silently without a row for it — see the dedicated section below.

## The unbacked-gate sweep gap (a pre-existing MINOR trigger-record, closed here)

T25's settlement (prior session) recorded a MINOR trigger: "unbacked-gate sweep idiom scope
(widen at next preflight touch)." This phase is that touch. The sweep's regex was
`GAUNTLET_T[0-9]+_[A-Z_]+` — built when every env-gated battery in the tree was
task-numbered (T13/T20/T24). This plan uses Phase letters (A/B/C1/C2/D), so
`GAUNTLET_C2_REAL_PRIME` doesn't match. Confirmed by running `just tui-preflight
conformance` with the new gate declared but the regex unwidened: it passed with **no row for
the new gate at all** — a real unbacked-gate escape, not hypothetical. Widened to
`GAUNTLET_[A-Z][0-9]+_[A-Z_]+`, `GAUNTLET_C2_REAL_PRIME` added to `declared_env_gates`, and
a full GATED row + `check_backing` call added (mirroring T13/T20/T24's exact idiom). Re-run
with the row present and this evidence file (this file) absent: the sweep correctly FAILED
closed (`UNBACKED — evidence file ... missing`), proving the widened check is load-bearing,
not decorative.

## Teeth (plan §4 Phase C2)

| # | Tooth | Where | Result |
|---|---|---|---|
| 1 | Denial-class canonical route spawns ZERO processes | `prime-delegation-real.test.ts` | PASS — `host.getSession()` stays null, no bookkeeping row, canonical dispatch ran first |
| 2 | Concurrent same-key drives spawn once | `prime-delegation-real.test.ts` | PASS — one admitted, one `duplicate_session_id`, one real process |
| 3 | Mailbox "done" text never establishes completion | `prime-delegation-real.test.ts` (2 cases: pass exit, fail exit) | PASS — status stays "running" until the explicit contract; a nonzero-exit child with "done!" mailbox text still records outcome "fail" |
| 4 | Forged/substituted binary path refused, not resolved | `real-prime-host.test.ts`, `security-boundaries.test.ts` | PASS — relative/bare/metacharacter paths refused at construction (`TerminalBoundaryError`); no `PrimeSpawnRequest` field can substitute a program per call |
| 5 | No auth material in any projection/frame/log/evidence | `secret-leakage.test.ts` | PASS — a secret-shaped ambient env value reaches the real child's env but never appears in `session.messages`/`resultContract`/thrown-error text/the written evidence file |
| 6 | `installed:true, available:false` renders unavailable, refuses submission | `workbench-host.test.ts` | PASS — the `prime` capability row resolves `unavailable` via `resolveCapabilityStatus` even when configured; absent entirely (not a fabricated row) when unconfigured |
| 7 | D6: Prime unconfigured AND configured-but-unreachable → identical Gauntlet behavior | `workbench-host.test.ts` | PASS — full read+invoke cycle against a nonexistent `prime_binary` path is byte-identical to the unconfigured case |
| 8 | No kernel callable can perform a Gauntlet-denied action | *(not built this pass)* | **not closed** — see below |
| 9 | Concurrent kernel submissions serialize against one REPL | *(not built this pass)* | **not closed** — see below |
| 10 | Every new privileged boundary carries an executed attack (D3) | `security-hardening.ts` + `security-boundaries.test.ts` | PASS — `real.prime.host.spawn` entry added; `assertInventoryAttackCoverage` (26 entries) passes with a real attack recorded (forged path + denial-class zero-spawn over a real `Bun.spawn`-backed host) |

**Teeth 8 and 9 (the kernel path) are explicitly not built this pass.** They require a real
Prime-owned Python REPL mediation binding (`KernelActionCallable`/`KernelInputBroker`
extended to drive the real substrate's kernel/REPL surface, not just child-session
admission) — a subsystem of comparable size to everything else in this phase, and the plan
text's ten teeth don't gate on composer-level wiring existing, only on the dispatcher/host/
security layer, which this phase fully delivers. `kernel` composer mode continues to report
"declared (planned, not yet bound)" from Phase C1, honestly unchanged.

## Gate results

| Gate | Result |
|---|---|
| `bun test src/prime-host.test.ts src/prime-recursion.test.ts src/prime-kernel.test.ts src/prime-topology.test.ts` | 20/0 |
| `bun test src/security-boundaries.test.ts src/secret-leakage.test.ts src/unsafe-input-boundaries.test.ts src/deny-bypass.test.ts` | 101/0 |
| `bun test src/cross-surface-equivalence.test.ts src/operation-isomorphism.test.ts src/substrate-inventory.test.ts` | 33/0 |
| Gated real battery: `GAUNTLET_C2_REAL_PRIME=1 bun test src/prime-delegation-real-battery.test.ts` | 2/0 — the real, T24-qualified `prime-agent` 0.8.0 binary launched via `RealPrimeHost`, ran to completion, delivered a schema-valid result contract at the declared `contract_path`; per-run isolation (mktemp root, 0700 agent dir, copy-never-mutate operator `~/.prime/agent/{auth,models}.json`, pkill sweep before/after, recursive shred on teardown); operator's real `~/.prime` directory confirmed untouched |
| `just tui-test all` (49 registered suites) | 540 pass / 0 fail / 10 skip (was 537/0/10 after C1; +3 within existing registered suites — C2 binds real production code, it adds no new T-numbered gate) |
| `just tui-check` / `bun run lint` | clean (only pre-existing `bun-substrate.bench.ts` `noConsole` warnings) |
| `just tui-bench focus-latency` | ALL BUDGETS PASS |
| `just tui-bench projection-worker` | ALL BUDGETS PASS |
| `just tui-preflight conformance` | PASS — new `GAUNTLET_C2_REAL_PRIME` row present and backed by this file |
| `just tui-build release` (×2) | OK both times, `sha256=bfc56fdcaf3a9b0c…` byte-identical, clean 120x40 PTY launch exit 0 |
| `just ruler` | PASS |

## Closes / Does not close

**Closes**: REQ-GOAL-003 in production (Prime is now genuinely exploited as a native
substrate, not only inside test harnesses), REQ-PRIME-001 (real topology, no synthetic
identity), REQ-PRIME-002 (headless/structured — no terminal-UI scraping), REQ-PRIME-003
(receipt before spawn, handle never the result — proven against a real process, teeth 1–2),
REQ-PRIME-005 (mailbox ≠ completion, tooth 3), REQ-SEC-005 (T13 spawn boundary now protects
a real Prime child spawn, not only `TerminalSession`), and D3/D6 as declared.

**Explicitly not closed**: REQ-PRIME-004/006/007 and REQ-OUT-004's kernel-path half (teeth
8–9 — the real Python REPL mediation binding); composer-level `prime`/`message` submission
routing through the dispatcher (envelope construction from a live composer submission);
`prime_socket`/daemon-based session listing (declared config surface, not yet consumed —
`RealPrimeHost`'s first production binding is one-shot-per-child, matching what T24 already
proved works, not a persistent daemon). REQ-PRIME-009/010 (long-running lifecycle,
reattachment) is Phase D's explicit scope, unchanged.

**Gated real battery reproduction**: `GAUNTLET_C2_REAL_PRIME=1 bun test
src/prime-delegation-real-battery.test.ts` from `workbench/`.
