# Bounded-gap feasibility verdict — 2026-09-10

Advisory only. No code changed, no evidence altered, no frozen reference touched.
Scope: the three residuals recorded in `.agents/plans/tui-runtime-repair-followup.md`
("Residuals (bounded, intended)") and `workbench/repair/HANDOFF-PLAN.md` item 4.

---

## 0. Corrections to the gap descriptions as posed

Four things in the framing are wrong or incomplete, and two of them change the verdicts.

**C1 — `prime-host.ts` / `prime-kernel.ts` are not substrate.** They are in-model
adapters. `PrimeHostAdapter.spawnChild` (`prime-host.ts:49`) mints a handle in a `Map`;
`PrimeReplKernel.execute` (`prime-kernel.ts:57`) serializes onto a promise tail and
fabricates no process. Neither ever spawns anything. The only code in the tree that
drives the *real* substrate is the T24 battery `prime-fresh-target.test.ts`, and it
hardcodes an absolute operator-specific path (`prime-fresh-target.test.ts:431,497,513,596`
— `/home/sprime01/.local/bin/prime-agent`) behind `GAUNTLET_T24_REAL_PRIME`. So the answer
to "does a real submission path exist in the substrate" is **yes, but not in any production
module** — it exists as a qualified *fact* (`substrate-inventory.ts`,
`QUALIFIED_PRIME_BINARY`, `SUBSTRATE_PROVENANCE_MAP`) and as *test-local process driving*.
Binding the composer to it means writing a production real-process adapter that does not
exist today.

**C2 — Gap 2 is two gaps, not one, and the workbench half is already built.**
`live-streams.ts` (1285 lines) already implements the entire resumable-stream *client*:
`AuthoritativeStreamPort` (`live-streams.ts:100`), opaque cursors (`mintOpaqueCursor:315`),
epoch/retention cursor validation (`CursorValidation:464`, `SourceReconciliation:470`),
baseline refusal accounting (`BaselineRefusal:502`), visible degraded/resync state
(`StreamConnectionState:521`), detach/reattach receipts (`:555`, `:569`), and
`LiveStreamClient` (`:602`). What is missing is (a) any non-in-memory implementation of the
port — only `InMemoryStreamPort` (`:428`) exists — and (b) **any production wiring at all**:
`WorkbenchHost` imports `GauntletReadClient` and nothing from `live-streams`
(`workbench-host.ts:1-14`; importers of `live-streams` are eight test/aggregate modules
only). The `poll-N` cursor is not a fallback for a missing stream; it is a second,
hand-rolled cursor mechanism running beside an unused, fully-tested one.

**C3 — the SSH leg does not test a remote transport, and as written cannot fail.**
Prereg D1 (`ssh-target.test.ts:17-37`) states SSH/tmux operate the *same* host against the
*same* loopback surface. The leg runs
`ssh -o … localhost 'cd … && bun run src/main.ts'` (`ssh-target.test.ts:227`) with **no
`-t`/`-tt`**, so sshd allocates no remote pty. `main.ts:57` branches on
`process.stdin.isTTY !== true || process.stdout.isTTY !== true` and takes the one-shot
non-TTY dump path — which prints exactly the markers the leg asserts (`Gauntlet`,
`NAVIGATOR`, `INSPECTOR`) and then returns. The subsequent `send-keys q` and
`__T20_EXIT_0__` assertion are satisfied by a process that already exited. **The leg would
go green without ever entering the OpenTUI/raw-mode path it exists to qualify.** This
inverts gap 3: the residual is not an environment limitation being honestly disclosed, it is
a weak tooth whose skip is currently *hiding* its weakness.

**C4 — "operation" mode is narrower than implied.** `OPERATION_REGISTRY`
(`operation-registry.ts:60`) holds exactly two canonical operations, `unit.resolve` and
`unit.retry`. The canonical surface's whole route table is a nine-arm `match` at
`crates/adapters/gauntlet-adapter-surface-json/src/http.rs:277-306`. There is no
message/prime/kernel-shaped canonical endpoint to bind to, and no "irreversible unit"
concept in the workbench registry to auto-dispatch through — the irreversibility guard the
brief cites lives upstream of this surface, not in it.

---

## 1. Gap 1 — no live Prime/kernel/message submission transports

### (a) Verdict: **KEEP-OPEN** for `message` and `kernel`. **CLOSE-CONDITIONALLY, deferred** for `prime`.

`message` has no canonical destination: nothing in the §20.2 route table
(`http.rs:277-306`) or `OPERATION_REGISTRY` accepts an operator-authored message, and the
spec's own §20.2 TUI priority order (escalation queue · in-flight invocations · answer-key
completion · budget burn · sandbox denials · residuals) never asks for one. Building a
destination for it *is* inventing authority. `kernel` is worse than merely unbuilt: the
real Python kernel is reachable only inside the real `prime-agent` process, so binding the
composer to it puts arbitrary code execution as the operator uid behind a text box, with no
containment tier — precisely what §17.3 (`isolation: none` must not serve a tier requiring
containment) and §21.3 ("Sandbox failure → escalate; **never** fall back to unsandboxed
execution") forbid. `prime` is the one legitimate candidate, because the gate already
exists and is already proven: `PrimeDelegationDispatcher` (`prime-recursion.ts:337`) takes
a canonical accepted receipt *before* any child spawn, serializes decisions so a concurrent
duplicate cannot reset a live child (`enqueueDecision:390`), and enforces typed
zero-side-effect policy refusals (`checkPolicy:400`). T24 recorded that composition as
`exposed-by-adapter-model` against a real binary that genuinely serves it
(`substrate-inventory.ts:518,539` — real delivery receipt `agentmsg_<uuid>`,
`deliveryStatus: "delivered"`). But it is deferred, not closed now, because it needs Gap 2
first (see §4) and because it needs an inventory→availability gate that does not exist.

### (b) Feasibility

Exists: the dispatcher and its policy gates; the gateway and `OperatorActionRoute`
(`operation-gateway.ts:110-119`) with fail-closed pre-admission observation; the qualified
binary identity and 16-feature three-way provenance map (`substrate-inventory.ts:245`,
`QUALIFIED_PRIME_BINARY:49`); a `probePrimeSurface` helper that already spawns
(`substrate-inventory.ts:807`) but is deliberately read-only over a closed argv vocabulary
(`:796`); real-substrate proof of `send`, daemon-socket agent creation, `--mode json`
session records, and cross-process survival (`prime-fresh-target.test.ts`, T24 evidence
`.agents/reports/gauntlet-tui-prime-workbench/evidence/T24/`).

Would have to be built: a production `RealPrimeHostAdapter` implementing the
`PrimeHostAdapter` shape over `Bun.spawn` against a *configured* (never hardcoded) binary
path and a per-host daemon socket; capability→availability gating so a merely-*installed*
binary is not treated as available (the standing "installed capability is inventory, never
availability" invariant — today `WorkbenchHost.read` sets `installed: true` with every gate
`false`, `workbench-host.ts:88-99`, which is the honest shape and must not be loosened);
argv construction under the existing T13 spawn-env bound (PATH/`LD_*`/`DYLD_*` injection
family, program resolution provenance); and secret handling for the auth material T24 had
to copy `0700` and shred.

Implications: no frozen-spec change (§6.1 `AgentRunner` already anticipates a real runner
with `RunnerCapabilities`); no persisted-schema change; no public-interface change to the
canonical surface. But a **large containment and blast-radius change**: today the workbench
spawns nothing in production, and this makes it a process parent holding credentials.

### (c) Cost / risk

Rough size: ~600–900 lines of new production code plus a gated real battery. Blast radius
from the gateway outward is contained on paper — `WorkbenchHost` is imported only by
`main.ts`, `workbench-host.test.ts`, `cross-surface-equivalence.test.ts` — but the *process*
blast radius is unbounded in kind: a delegated child is a real agent with the operator's
credentials. Regression surface: `security-boundaries.test.ts`, `secret-leakage.test.ts`,
`unsafe-input-boundaries.test.ts`, `deny-bypass.test.ts`, the 21-entry privileged-boundary
inventory (T18's `assertInventoryAttackCoverage`, which fails the suite when a new boundary
lacks an executed attack — so this closure *cannot* land without new attacks), plus
`operation-isomorphism.test.ts` and `cross-surface-equivalence.test.ts` (a new route must
not break the one-idempotency-identity-across-all-routes pin).

RED-first teeth required: a composer `prime` submit with a *denial-class* canonical route
must leave zero processes spawned (already the shape of `security-hardening.ts:183`);
duplicate `child_session_id` submitted concurrently must spawn once; a child that replies
"done" in the mailbox must not settle anything without a schema-valid result contract; a
forged binary path in config must be refused, not resolved; auth material must never appear
in any projection, frame, or log; `installed: true, available: false` must render as
unavailable and refuse submission.

### (d) Residual disclosure to stand (KEEP-OPEN portion)

> The composer's `message` and `kernel` modes have no canonical destination and no
> contained execution tier. `WorkbenchHost.submit` reports them unsupported and sends
> nothing; no delivery is fabricated. `prime` mode is deferred, not unsupported in
> principle: the gated composition (`PrimeDelegationDispatcher`) exists and the real
> substrate is qualified (T24), but no production real-process adapter is built and no
> installed→available gate exists. Binding any of the three today would create authority
> the canonical surface does not grant.

Tripwires that should force revisiting: (i) the canonical surface gains a message- or
delegation-shaped route in `http.rs`; (ii) `OPERATION_REGISTRY` grows past
`unit.resolve`/`unit.retry`; (iii) any production module (not test) acquires a `Bun.spawn`
of `prime-agent`; (iv) `substrate-inventory.ts` reclassifies any REQ-PRIME-008 feature from
`exposed-by-adapter-model` to `exposed-by-real-binary` in a production path.

---

## 2. Gap 2 — no resumable event stream, snapshot polling only

### (a) Verdict: **CLOSE-CONDITIONALLY**, split in two.

**2a — CLOSE now (no approval needed).** Implement a `SnapshotDerivedStreamPort`
satisfying the existing `AuthoritativeStreamPort` over the *existing* §20.2 GET routes,
and wire `LiveStreamClient` into `WorkbenchHost` in place of the hand-rolled
`membership`/`previous`/`cursor` diffing at `workbench-host.ts:100-160`. This requires no
core change, no route change, and no persisted-schema change, and it retires a duplicate
cursor mechanism rather than adding one.

**2b — KEEP-OPEN pending explicit approval.** A genuinely durable, backend-ordered,
resumable stream requires a new route in the nine-arm `match` at `http.rs:277-306` plus a
persisted monotonic sequence and store epoch in the core. That is simultaneously a
public-interface change (§20.2 enumerates its routes normatively) and a persisted-schema
change. It should not be attempted without the operator's explicit approval, and it is not
required to make the workbench honest.

The condition on 2a is a hard one: the derived port must **not** wear a durable stream's
costume. Sequences derived from snapshot diffs mean the retention floor is "since this
client connected", the epoch is client-minted, and events between two polls are
unobservable by construction. Those three facts must be typed into the port and surfaced in
the client's degraded reasons, not papered over. If that honesty cannot be preserved, 2a
becomes KEEP-OPEN too.

### (b) Feasibility

Exists (and is tested): the entire client half named in C2 above, plus its suites —
`cold-rebuild.test.ts`, `detach-reattach.test.ts`, `reconnect-races.test.ts`,
`stale-frame.test.ts`, `transport-equivalence.test.ts`, `secret-leakage.test.ts`; the T16
evidence at `.agents/reports/gauntlet-tui-prime-workbench/evidence/T16/live-streams.md`;
the T19 headless replay and normalized-projection digest referee (`headless-replay.ts`),
which is the natural equivalence oracle for proving the swap changes no projection.

Would have to be built for 2a: the derived port (~200–300 lines), the host swap
(~80 lines removed, ~60 added), and typed unobservability disclosure. For 2b: a core
sequence/epoch store, a route, wire-format, retention policy, and the whole
approval-gated change.

Implications: 2a — none on frozen spec, persisted schema, or public interface; it consumes
only routes that already exist. 2b — both a public-interface and a persisted-schema change.
Neither touches authority: the stream is observation only, and the gateway's
re-validate-against-fresh-canonical-observation rule (`operation-gateway.ts:111-116`) must
keep reading through `route.observe`, **never** through the stream projection. That is the
single most important containment line in this closure: if a drive ever pre-admits against
a streamed projection instead of a fresh observation, the router-not-authority invariant is
broken.

### (c) Cost / risk

Size: 2a is medium (~400 lines net, one production module rewired). Blast radius:
`WorkbenchHost` → `main.ts` only, plus `workbench-host.test.ts` and
`cross-surface-equivalence.test.ts`. `GauntletReadClient` stays (six other importers,
including `preview.ts`). Regression surface is real but well-fenced: the T06 projection
generation contract (`projection-generation.test.ts`), focus identity across generation
restarts (`focus-identity.test.ts` — the host currently clears focus on membership change
at `workbench-host.ts:117-124`, and `LiveStreamClient`'s reconciliation has its own opinion
about that), `renderer-boundary.test.ts`'s state/presentation classification (a new state
module must not import presentation), and T21's frozen payment budgets — the client is
heavier than the poll loop and `focus-latency.bench.ts` / `projection-worker.bench.ts` must
still pass unchanged.

RED-first teeth: a headless-replay equivalence proving the streamed host produces a
byte-identical normalized projection digest to the polled host over the same fixture (this
tooth must fail if the swap changes any projection); a tooth that fails if the derived port
reports a retention floor or epoch it cannot honour; a tooth that fails if any drive's
pre-admission observation is served from the stream projection rather than
`route.observe`; a cursor persisted across a real detach/reattach against the *derived*
port, not the in-memory one; a mid-poll backend restart producing a visible `resyncing`
transition rather than a silent advance.

### (d) Closure plan for 2a

Acceptance: `WorkbenchHost` holds exactly one cursor mechanism; the `poll-N` string is gone
from production code; every unobservable-by-construction property of the derived port is
typed and surfaced; the normalized projection digest is unchanged; no route, schema, or
authority change.

Per-step gates: after the port lands — `just tui-test live-streams`, `just tui-test
cold-rebuild detach-reattach reconnect-races stale-frame`. After the host swap —
`just tui-test workbench-host production-entry`, `just tui-test headless-replay`,
`just tui-test projection-generation focus-identity`, `just tui-test
cross-surface-equivalence operation-isomorphism`, `just tui-test renderer-boundary`.
Before settlement — `just tui-check`, `just tui-lint`, `just tui-preflight conformance`,
`just tui-test all` (must stay at 49 suites, 0 fail), the focus-latency and
projection-worker benches against T17's `FROZEN_BUDGETS`, `just tui-build release` with a
fresh PTY launch and a byte-identical rebuild, and `just ruler`. `just verify` is not
required (no Rust source changes) — the same decoupling recorded in the followup plan.
`graft build` after code changes.

Evidence artifacts: a new evidence directory under
`.agents/reports/gauntlet-tui-prime-workbench/evidence/` carrying the polled-vs-streamed
digest equality record, the derived port's typed unobservability disclosure, the
detach/reattach receipt over the derived port, the bench table against the frozen budgets,
and the preserved RED round showing the digest tooth failing before the swap was correct.

### (d′) Residual disclosure to stand for 2b (KEEP-OPEN portion)

> The canonical surface exposes snapshots, not an ordered durable stream: its route table
> is the nine §20.2 endpoints and contains no event/offset route. The workbench's stream
> client therefore runs over a snapshot-derived port whose sequence, epoch, and retention
> floor are client-local: events occurring between two observations are unobservable by
> construction, and no backend offset is invented or implied. A durable resumable stream
> requires a new public route and a persisted sequence/epoch in the core, and is not
> attempted without explicit approval.

Tripwire: any proposal to persist a workbench cursor as though it addressed backend state,
or any core change that introduces a monotonic event sequence — either should reopen 2b.

---

## 3. Gap 3 — real-sshd leg permanently skipped

### (a) Verdict: **CLOSE-CONDITIONALLY** — and the condition is repairing the leg, not supplying the environment.

Per C3, the leg passes an SSH invocation with no remote pty into an entry point that
branches to a one-shot non-TTY dump (`main.ts:57`) emitting exactly the markers the leg
asserts. Standing up a container/CI sshd today would turn a visible, honest skip into a
green result that qualifies nothing — strictly worse than the current disclosure, because
the skip at least advertises that the leg did not run. So: fix the leg first, then supply
the environment. Once both are done the gap genuinely closes, because the transport shape
under test is narrow and well-understood. It is *mostly* an environment limitation — but
"mostly" is doing real work, because the residual it currently masks is a verification
defect, not a transport defect.

### (b) Feasibility

Exists: the whole tmux harness (`spawnWorkbenchInTmux:129`, `capturePane:155`,
`panePid:159`, `untilPane:173`), a real sshd reachability probe that records its actual
failure (`probeSshLoopback:68`), the release manifest's `support_facts` channel
(`release-manifest.ts:58,80`), the tmux leg's SIGWINCH/resize/below-minimum/quit
assertions (`ssh-target.test.ts:241-283`) as the template for what the SSH leg should
assert, and the T20 real-PTY journeys at 120x40 / 80x24 / below-minimum.

Would have to be built: `-tt` (or `-t`) on the ssh invocation; assertions that can only
pass on the TTY path — alternate-screen entry, a raw-mode-only keystroke effect, and a
`SIGWINCH` delivered *through the ssh channel* rather than only through the local tmux
pane; a negative control proving the leg fails if the entry takes the non-TTY branch; and a
sshd fixture (container or CI service) with its own host keys and an isolated
`known_hosts`, never touching the operator's ssh config.

Implications: none on frozen spec, persisted schema, public interface, or authority. The
only containment note is the fixture itself — an sshd accepting loopback connections must
be scoped to the test container and must not be brought up on a developer host by default,
which argues for an env-gated fixture (the same gating idiom as `GAUNTLET_T24_REAL_PRIME`)
rather than an always-on service. Blast radius is confined to one test file plus the
manifest's `support_facts` string.

### (c) Cost / risk

Small: ~80–150 lines in `ssh-target.test.ts`, plus fixture config. Regression surface is
`ssh-target.test.ts`, `release-manifest.test.ts` (the `support_facts` line changes),
`transport-equivalence.test.ts`, and T25's plan-traceability runner and unbacked-gate sweep
(`plan-traceability.ts`, `tui-test-all.ts`) — a leg that stops being a named skip changes
the 530/0/9 shape, so `just tui-test all` must be re-baselined deliberately, not
accidentally. Risk of the fix: the leg becomes slow and flaky if the sshd fixture is not
isolated; and any residual reliance on the operator's real sshd would make the suite
environment-dependent in the way it was correct to avoid.

RED-first teeth: the negative control above is the load-bearing one — with `-tt` removed,
the leg must FAIL. That single tooth is what converts this from a tautology into a
qualification. Second: SIGWINCH through the ssh channel must change the rendered geometry
(the same three-state assertion the tmux leg already makes). Third: a clean exit 0 with the
terminal restored, asserted *after* a keystroke that only the raw-mode path can consume.

### (d) Closure plan

Acceptance: the SSH leg asserts at least what the tmux leg asserts; it cannot pass on the
non-TTY branch (proven by a preserved RED round); the sshd fixture is env-gated and
self-contained; `support_facts` states the qualified shape truthfully (loopback sshd, same
host, same surface — not a remote deployment).

Per-step gates: `just tui-test ssh-target` (RED first, with `-tt` withheld), then GREEN;
`just tui-test release-manifest transport-equivalence`; `just tui-test all` with the new
skip count recorded deliberately; `just tui-build release` to regenerate the manifest and
its `support_facts`; `just tui-preflight conformance` so the leg's gate row moves from
GATED to EXECUTED with evidence; `just ruler`.

Evidence artifacts: the preserved RED round showing the leg passing without `-tt` (the
tautology, recorded rather than hidden) and then failing once the negative control is in;
the captured pane frames from the SSH-hosted session at 120x40 and after resize; the
updated `support_facts` line; the re-baselined `tui-test all` table.

---

## 4. Ordering

**3 → 2a → (2b, on approval) → 1.**

3 first because it is the only one of the three that is a defect in *existing settled
evidence* rather than an absent capability, it is the smallest, and it is confined to one
test file — a false-green tooth in a settled campaign is the highest-priority item on this
list regardless of the other two.

2a second because it is a pure internal consolidation with no interface change, it retires
a duplicated cursor mechanism, and it must land before anything else consumes observation:
every later feature reads through the host's projection path, so rewiring it after adding
consumers multiplies the regression surface.

2b next only if approved, because a real backend stream is the honest foundation for 1 —
"in-flight invocations" (the §20.2 TUI priority order's second item) is exactly the kind of
state that snapshot polling observes badly, and delegating work you cannot then observe in
order is worse than not delegating it.

1 last because it depends on 2 for honest observation of what it dispatches, it carries by
far the largest containment change, and it is the only one of the three whose absence costs
the operator nothing the spec asks for.

---

## 5. Recommendation

**Close gap 3's leg defect now; close gap 2a next; keep gap 1 and gap 2b open with
sharpened residuals.**

The leg defect is a correctness problem in settled evidence and should not wait. 2a is the
one closure that increases honesty without increasing surface: it deletes a hand-rolled
mechanism in favour of a tested one and costs no new authority, no new route, and no new
process. 1 and 2b both require something the operator has not granted — respectively a
process-spawning production adapter holding credentials, and a public-interface plus
persisted-schema change — and neither buys anything the spec's own §20.2 priority order
asks for. The residuals as currently written are close to right; they need only the
corrections in §0 folded in (C2 especially: "no resumable stream" understates what is
already built and overstates what is missing).

### Strongest counterargument to this recommendation

2a is the weak link. It rewires the single production module that was just settled and
independently confirmed, and it buys the operator **no new capability whatsoever** — the
same data, from the same nine routes, at the same poll interval. In exchange it replaces a
legible ~60-line loop with a 1285-line client whose central concepts — epoch, retention
floor, opaque cursor, resync — are *partly fictional* over a snapshot source. A derived
port that mints its own epoch and floors its own retention at connect time is, read
uncharitably, exactly the "invented stream offset" the residual currently forbids, dressed
in types. It is entirely defensible to argue that a polled projection honestly labelled
`poll-N` is *more* truthful than a polled projection wearing a durable-stream vocabulary,
and that the right move is to leave `WorkbenchHost` alone, sharpen the residual to say
plainly that a fully-built stream client sits unwired pending a real backend stream, and
spend nothing until 2b is approved. If the honesty condition in §2(a) cannot be met
cleanly in the port's types, that counterargument wins and 2a should become KEEP-OPEN.
