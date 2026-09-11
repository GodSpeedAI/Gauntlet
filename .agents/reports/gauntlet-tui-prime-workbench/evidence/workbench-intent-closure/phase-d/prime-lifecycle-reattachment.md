# Phase D — long-running lifecycle and reattachment in production

Plan: `.agents/plans/workbench-intent-closure.md` §4 Phase D. Depends on Phase B (honest
observation path) and Phase C2 (real admitted work to survive) — both settled before this
phase started. `GAUNTLET_D_REAL_PRIME_LIFECYCLE=1` is the gate for this phase's real-substrate
battery.

## What changed

- **`workbench/src/prime-host.ts`** — no change; `PrimeHostPort`/`PrimeSpawnRequest` from
  Phase C2 already carried everything this phase needed.
- **`workbench/src/prime-long-running.ts`** — every function/class typed against the concrete
  `PrimeHostAdapter` (`deliverOperatorMessage`, `loggingPrimeAdapter`,
  `PrimeWorkbenchHostOptions.adapter`, `PrimeWorkbenchHost.adapter`, `PrimeWorkbenchHost.detach`,
  `reattachToPrime`) is now typed against `PrimeHostPort`. Zero behavior change — this is the
  SAME D2 extraction Phase C2 did for `prime-recursion.ts`, letting T10's entire
  detach/reattach/messaging vocabulary drive a REAL `RealPrimeHost` with no new code (proven in
  `prime-long-running.test.ts`'s new cross-substrate describe block).
- **`workbench/src/live-streams.ts`** — `LiveStreamClient.reattach`'s `prime_adapter` option
  widened from `PrimeHostAdapter` to `PrimeHostPort` for the same reason (it calls
  `reattachToPrime` directly).
- **`workbench/src/recovery-matrix.ts`** — `MatrixWorld.observedAdapter` (the call-logging
  proxy T22 uses to prove zero cancel calls) widened to `PrimeHostPort`; `MatrixWorld.adapter`
  itself stays the concrete `PrimeHostAdapter` (T22's own fixture still constructs the fake
  directly — untouched).
- **`workbench/src/real-prime-host.ts`** (Phase C2 file, extended) — the qualified-record
  reattachment and explicit-stop machinery this phase is actually about:
  - `spawnChild()` now writes a `*.qualified.json` manifest synchronously (`session_id`,
    `parent_session_id`, `model`, `contract_path`, `pid`, `spawned_at`) into `sessionDir`
    BEFORE returning — the durable record a future instance reattaches from.
  - The constructor now calls a new private `reattachFromQualifiedRecords()`: scans
    `sessionDir` for `*.qualified.json` files; a manifest whose `contract_path` already exists
    is folded in directly (completed/failed, synchronous, no polling); a manifest whose `pid`
    `isPidAlive()` is reattached as `"running"` and polled every 250ms for either the contract
    landing or the pid dying; a manifest whose pid is gone with no contract is recorded via the
    new `staleRecords()` accessor (`process_not_found_no_result_contract`) — never minted as
    running. Anything in `sessionDir` that isn't a well-formed manifest (wrong shape, invalid
    JSON, unrelated files) is silently skipped — presence alone mints nothing.
  - New `isPidAlive(pid)` (exported, pure — signal-0 probe) and `contractFromEvidenceRecord()`
    (exported-internal — the ONE derivation `watch()`'s live path and the cold-reattach path
    both call, so "exit code → outcome" has a single source of truth, D2).
  - New `stopChild(sessionId): PrimeStopOutcome` — the only method in this class that ever
    sends a signal. For a session this instance still directly owns (live `Bun.Subprocess`
    handle), it kills the handle and lets `watch()`'s own natural completion path record the
    (honest, never-fabricated) outcome. For a reattached session (pid-only, no handle), it
    kills by pid and records termination itself (no other observer exists for it). Refuses
    cleanly — zero signal sent — for an unknown or already-terminal session id.
  - `sessionDir` is now created `0700` (previously unrestricted) — the manifests `stopChild`
    trusts for pid-targeted `SIGKILL` are now only writable by this OS user.
- **`workbench/src/workbench-host.ts`** — `primeDirs()` exported (previously file-private) so
  tests can compute the exact production dirs without duplicating the derivation.
- **`workbench/src/security-hardening.ts`** — new `PRIVILEGED_BOUNDARY_INVENTORY` entry
  `real.prime.host.stop` (D3: a new privileged boundary — signal delivery — needs its own
  inventory row and executed attack).
- **`workbench/src/security-boundaries.test.ts`** — new attack test proving `stopChild`
  refuses on unknown/terminal sessions with zero signal sent, `sessionDir` is `0700`, and a
  forged manifest's blast radius is bounded to exactly its own recorded pid (a live decoy and
  bystander process, and this test process itself, are provably untouched).
- **`workbench/src/prime-long-running.test.ts`** — new describe block reproducing T10's
  detach/reattach/no-cancel proof against a REAL `RealPrimeHost` (fake-script binary): zero
  cancel calls, work outlives the host object, a fresh `reattachToPrime()` sees the real
  completed state.
- **`workbench/src/prime-cross-process-lifecycle.test.ts`** (NEW, unconditional) — the core
  teeth: mid-flight reattach across two `RealPrimeHost` instances: reattach after completion
  (synchronous fold); directory presence without a qualified record never fabricates running;
  a dead-pid manifest with no result lands as a typed stale record; explicit stop terminates a
  real (fake-script) process both for a same-instance-owned and a reattached session; stop
  refuses cleanly on unknown/terminal sessions.
- **`workbench/src/prime-cross-process-lifecycle-real-battery.test.ts`** (NEW, gated:
  `GAUNTLET_D_REAL_PRIME_LIFECYCLE=1`) — the same shapes proven against the REAL, T24-qualified
  `prime-agent` binary: qualification, real mid-flight cross-instance reattach + real
  completion fold, real explicit-stop `SIGKILL` termination.
- **`workbench/src/workbench-host.test.ts`** — new describe block proving REQ-VERIFY-001
  extended over Prime: deleting `config.projection_store` (+ WAL/SHM sidecars) and
  constructing a fresh `WorkbenchHost` against the same config recovers the Prime session from
  `sessionDir` — a sibling directory the deleted file never touched — with zero effect.
- **`scripts/tui-preflight.sh`** — new `G3c` gated row for `GAUNTLET_D_REAL_PRIME_LIFECYCLE`;
  the env-gate sweep regex widened again (see RED round 2 below).

## The mechanism decision (this phase's own judgment call, disclosed)

The plan text states the invariant ("an explicit canonical stop/cancel is the only thing that
terminates admitted work") without specifying a mechanism, and REQ-PRIME-009's actual text
allows for one ("...unless the operator invokes an explicit canonical stop/cancel operation").
The existing `canonicalStopAvailability()` (T10) correctly reports that the **Gauntlet
canonical surface** serves no such operation — that remains true and untouched; this plan's §2
boundary forbids touching the canonical/Rust surface without escalation, and nothing here does.
`RealPrimeHost.stopChild()` is a **local, non-canonical** capability instead: the underlying
mechanism a future canonical-bound `prime.cancel` (declared in Phase C1, still unbound) would
eventually call, built now so REQ-PRIME-009's "unless an explicit stop..." clause has an actual,
tested implementation rather than staying permanently vacuous. This was my own call, not a
user-confirmed design fork like Phase C2's background-poll-loop question — the plan's dependency
chain and REQ text left only one coherent interpretation once "an explicit stop is the only thing
that terminates work" needed something real to test against.

The qualified-record manifest design (`*.qualified.json`, written synchronously at spawn,
scanned at construction) was necessary because T24 established that the REAL `prime-agent`
binary writes its own `.jsonl` session files under `--session-dir`, named by **Prime's own
runtime session id** — not the workbench's session id — so they cannot serve as the
workbench-side reattachment key on their own. The manifest is the workbench's own durable
record of "I admitted this session, here is its pid and contract locator," entirely additive to
Prime's own on-disk state and never mutating or depending on Prime's `.jsonl` format.

## The RED rounds (preserved, D4)

1. **My own test's wrong assumption about `stopChild`'s outcome.** The first draft of
   "stopChild on a session this SAME instance directly owns" asserted
   `expect(status).not.toBe("completed")` after a `SIGKILL`. It failed — genuinely, on the
   first run — because `PrimeHostAdapter.deliverResultContract` sets `status: "completed"`
   whenever `schema_valid` is `true`, regardless of `outcome`; a killed process's exit code is
   still captured and written as valid evidence (`schema_valid: true`, `outcome: "fail"`).
   `"completed"` here means "a schema-valid contract landed," not "succeeded" — the production
   code was correct throughout; the test's own expectation was wrong. Fixed by asserting
   `resultContract?.outcome` is never `"pass"` instead — the actual invariant (a `SIGKILL`
   must never launder into a fabricated success), not a stronger, incorrect one about status.
2. **The unbacked-gate sweep gap, again — same class as the Phase C2 fix, wider this time.**
   Phase C2 widened `scripts/tui-preflight.sh`'s env-gate regex from `GAUNTLET_T[0-9]+_` to
   `GAUNTLET_[A-Z][0-9]+_` to catch phase-lettered gates like `GAUNTLET_C2_REAL_PRIME`. That
   fix still silently missed `GAUNTLET_D_REAL_PRIME_LIFECYCLE` — Phase D, E, and F carry **no
   digit at all** (unlike C1/C2), so `[0-9]+` (one-or-more) still didn't match a bare-lettered
   phase gate. Caught the same way as before: `just tui-preflight conformance` ran green with
   zero row for the new gate before the fix. Fixed by making the digit suffix optional
   (`[0-9]*`), and verified load-bearing the same way as Phase C2: re-ran preflight with the
   new gate declared but its evidence file not yet written — it correctly failed closed
   (`FAIL — GATED/UNQUALIFIED row 'D real-substrate lifecycle battery' is UNBACKED — evidence
   file ... missing`) — before this evidence file was written to resolve it.

## Teeth (plan-specified, all four)

| # | Tooth | Where proven | Result |
|---|-------|---------------|--------|
| 1 | TUI SIGKILL leaves the child live with identical ids; a fresh workbench process re-delivers to it | `prime-cross-process-lifecycle.test.ts` (fake binary, unconditional) + `prime-cross-process-lifecycle-real-battery.test.ts` (real binary, gated) — a second `RealPrimeHost` instance, never referencing the first, sees the still-running child as `"running"` purely from the qualified manifest | PASS |
| 2 | Deleting the entire projection store and restarting reattaches from authoritative records alone (REQ-VERIFY-001 extended over Prime) | `workbench-host.test.ts` "Phase D — Prime reattachment survives projection-store deletion" — `config.projection_store` (+ sidecars) deleted; a fresh `WorkbenchHost` against the same config recovers the Prime session via `primeDispatcher.topology()` | PASS |
| 3 | Session-directory presence WITHOUT a qualified record does NOT produce a "running" projection — it produces typed-unavailable | `prime-cross-process-lifecycle.test.ts` — garbage files (unrelated, unparseable, wrong-shape) mint nothing; a well-formed manifest with a dead pid and no result lands in `staleRecords()`, never `listSessions()` | PASS |
| 4 | An explicit canonical stop does terminate | `prime-cross-process-lifecycle.test.ts` (fake) + `prime-cross-process-lifecycle-real-battery.test.ts` (real) + `security-boundaries.test.ts`'s `real.prime.host.stop` attack — `stopChild()` kills a live or reattached process; detach/close (proven in `prime-long-running.test.ts`'s cross-substrate block) never do | PASS |

## Gate results

| Gate | Result |
|---|---|
| `just tui-test prime-long-running` | 9/0 |
| `just tui-test detach-reattach` | 5/0 |
| `just tui-test recovery-matrix` | 20/0 |
| `just tui-test cold-rebuild` | 3/0 |
| `bun test src/prime-cross-process-lifecycle.test.ts` (new, unconditional) | 7/0 |
| `bun test src/security-boundaries.test.ts` | 28/0 (incl. new `real.prime.host.stop` attack) |
| `bun test src/workbench-host.test.ts` | 8/0 (incl. new projection-store-deletion tooth) |
| Gated real-substrate battery (`GAUNTLET_D_REAL_PRIME_LIFECYCLE=1`, live) | 3/0 — real qualification, real mid-flight cross-instance reattach, real `SIGKILL` termination, all against the real `prime-agent` 0.8.0 binary |
| `just tui-test all` | 542 pass / 0 fail / 10 skip across 49 suites |
| `just tui-bench focus-latency` | ALL BUDGETS PASS |
| `just tui-bench projection-worker` | ALL BUDGETS PASS |
| `just tui-preflight conformance` | PASS (new `G3c` row backed; widened sweep verified fail-closed before this file existed) |
| `just tui-build release` ×2 | byte-identical binary, sha256 `0934499d32c4dcc9…` both runs |
| `just ruler` | PASS |
| `bun run check` (tsc) | clean |
| `bun run lint` (biome) | exit 0 |
| Full `bun test` | 670 pass / 22 skip / 0 fail across 692 tests / 69 files |

## Gated-battery reproduction

```
GAUNTLET_D_REAL_PRIME_LIFECYCLE=1 bun test src/prime-cross-process-lifecycle-real-battery.test.ts
```

Isolation follows T24/C2 exactly: unique mktemp root, `0700` agent+session dirs, operator's
real `~/.prime/agent/{auth,models}.json` COPIED (never mutated), a defensive `pkill` sweep
before/after keyed to the unique temp-dir substring, recursive removal on teardown. Live-run
result: 3/0 in 8.32s. Post-run verification: no stray processes matching the run's temp-dir
substring, no stray temp directories under `/tmp`, operator's real `~/.prime/agent/` untouched
(unrelated, pre-existing long-running `prime-agent` processes were observed on the host —
~18 hours old, unconnected to this battery's isolated temp root or pkill pattern, left alone).

## Closes

REQ-VERIFY-005 (closing the workbench during admitted long-running Prime execution allows the
work to continue and permits correct later reattachment — proven both for host-object closure,
which was already correct and is now proven against a REAL host, and for actual TUI-process
death, via qualified-manifest reattachment), REQ-PRIME-009 (an explicit stop — now built,
local, non-canonical — is the only thing that terminates admitted work; TUI close/detach still
never do), REQ-PRIME-010 (reattachment recovers topology from qualified records alone — never
directory presence, never a prior host's memory, never inferred liveness), and the remaining
half of REQ-OUT-003 (long-running Prime work now demonstrably survives TUI detachment against
both the fake and the real substrate).

## Does not close / explicitly deferred

- **Composer-level routing to `stopChild`.** `prime.cancel` (declared in Phase C1) is still
  unbound at the composer submission layer — this phase built and proved the underlying
  termination mechanism, not its UI wiring, consistent with Phase C2's own disclosed residual
  (composer-level `prime`/`message`/`kernel` routing) which this phase does not additionally
  close.
- **REQ-PRIME-004, REQ-PRIME-006, REQ-PRIME-007, and REQ-OUT-004's kernel-path half** — carried
  forward unchanged from Phase C2's own "explicitly not closed" list; Phase D did not touch the
  kernel/REPL mediation path.
- **A daemon-socket-based session listing** (`config.prime_socket`, declared in Phase C2, still
  unconsumed) — `RealPrimeHost` remains one-shot-per-child; this phase's reattachment mechanism
  is manifest-based specifically because there is no persistent daemon to query.
