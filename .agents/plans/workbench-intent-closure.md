# Workbench intent closure — binding what is already built

Status: proposed, 2026-09-10. Not dispatched.
Spec: `.agents/specs/gauntlet-tui-prime-workbench.spec.yaml` v0.3.0 (approved, frozen).
Supersedes the "Residuals (bounded, intended)" section of
`.agents/plans/tui-runtime-repair-followup.md` — see §1.
Prior analysis: `workbench/repair/GAP-VERDICT.md`.

---

## 1. The correction this plan rests on

The three residuals were recorded as *intended, bounded, fail-closed gaps*. Read against
the workbench spec rather than against Core-spec §20.2, that framing is wrong in one
specific way that changes the whole shape of the work:

**They are not intended gaps. They are the unclosed production remainder of MUST-level
requirements that were already discharged inside test harnesses.**

| Residual | The requirement it leaves open |
|---|---|
| No live Prime/kernel/message submission | REQ-GOAL-003 (Prime kernel, subagents, messaging, long-running control **MUST** be exploited as native substrates), REQ-VERIFY-005, REQ-PRIME-009/010 |
| Snapshot polling, no baseline+incremental reconciliation in production | REQ-OUT-003, REQ-DATA-002/003/004/007, REQ-SYNC-001, REQ-SAFE-005, REQ-VERIFY-006 |
| SSH leg skipped | CLAIM-003 (`partially_proven`; open gap is explicitly "End-to-end qualification in Gauntlet target terminals"), REQ-CONFIG-005 |

The preregistered desired outcome names the missing half directly: the operator should
"perform **or delegate** the next legitimate transformation" and "observe execution and
evidence **live**". Performing is bound. Delegating and live observation are not.

The good news, and the reason this plan is short: **almost nothing needs to be invented.**
Every mechanism these requirements need already exists in the tree, tested and settled. It
exists in test harnesses and unwired modules rather than in the production composition.
This plan is a binding plan, not a building plan. Its dominant risk is not "can we build
it" but "can we bind it without a single new honesty compromise".

### What is already built and merely unbound

| Requirement | Built artefact | Production status |
|---|---|---|
| REQ-SYNC-001 declarations, opaque cursors, epoch/retention validation, bounded buffering, resync | `live-streams.ts` — `AuthoritativeStreamPort:100`, `SourceDescriptor:73`, `mintOpaqueCursor:315`, `CursorValidation:464`, `SourceReconciliation:470`, `LiveStreamClient:602` | **unwired** — `WorkbenchHost` imports none of it |
| REQ-DATA-003 stale-frame rejection, REQ-DATA-002 generation reconciliation | `stale-frame.test.ts`, `reconnect-races.test.ts`, `projection-generation.test.ts` | proven against `InMemoryStreamPort` only |
| REQ-PRIME-003 receipt-before-spawn, serialized delegation decisions, typed zero-side-effect refusals | `PrimeDelegationDispatcher` (`prime-recursion.ts:337`, `enqueueDecision:390`, `checkPolicy:400`) | wired to the **in-model** `PrimeHostAdapter`, which spawns nothing |
| Real Prime substrate qualification (16 features, digest-pinned captures, real daemon/kernel/session lifecycle) | `substrate-inventory.ts`, `prime-fresh-target.test.ts`, T24 evidence | test-only; binary path hardcoded at `prime-fresh-target.test.ts:431` |
| Prime config slot and typed degradation | `WorkbenchConfig.prime_endpoint` (`config.ts:21`), `prime_integration_error` error class, preflight probe (`preflight.ts:94,184`) | present; nothing consumes it downstream |
| Kernel-side canonical callable + input mediation | `kernel-affordances.ts` — `KernelActionCallable:89`, `KernelInputBroker:446` | present; no live kernel behind it |
| Terminal/tmux qualification harness | `ssh-target.test.ts:129-190` | working, but the SSH leg is defective (§3) |
| Transport-mode distinction (REQ-CONFIG-005) | `transport-equivalence.test.ts`, `WorkbenchConfig.transport` | bound |

### Where my earlier verdict was wrong

`workbench/repair/GAP-VERDICT.md` argued `message` and `kernel` modes should stay
permanently open because they have "no canonical destination" and "no containment tier".
Against this spec both arguments fail:

- **`message` has a named destination and a named rule.** REQ-PRIME-005: mailbox delivery
  "MUST be treated as communication. Message content MAY update Chat/Trace/attention
  projections but MUST NOT alone establish Gauntlet completion." The spec does not forbid
  binding it; it specifies exactly how to bind it safely.
- **`kernel` is not a containment question.** `important_boundary.is_not` states the
  workbench "is not … a security sandbox", and NONGOAL-006 forbids *using* Prime/Python
  isolation as one. REQ-PRIME-006 requires kernel input/control to "return through the
  Prime-owned Python REPL mediation path" — a required path, not a forbidden one. The real
  constraint is REQ-OUT-004 and the false-completion condition "A kernel function or Prime
  extension can bypass a Gauntlet denial": the kernel may compute, and may never be an
  admission path.

The containment worry was imported from Core-spec §17.3, which governs agent-runner tier
selection, not an operator computing in their own trust domain. Corrected here.

---

## 2. What this plan does and does not change

**Does not change, at any phase:** the frozen spec, `.wayfinder/answer-key.md`,
`reference.lock`, any settled evidence record, the §20.2 canonical route table
(`crates/adapters/gauntlet-adapter-surface-json/src/http.rs:277-306`), any persisted core
schema, or `OPERATION_REGISTRY`'s canonical operation set. Any phase that turns out to
require one of these **stops and escalates** rather than proceeding.

**Does change:** the workbench's production composition (`workbench-host.ts` and its
adapters), the workbench config surface (additive, within REQ-CONFIG-003 which names
"socket" as a thing that MUST be configurable), the capability handshake's *declaration*
of planned operations, and one defective test leg.

---

## 3. Debt rules (binding on every phase)

These exist because the failure mode this plan is most likely to produce is not a bug — it
is a mechanism that *looks* more rigorous than the thing it replaced while being less
honest. Each rule is checkable.

- **D1 — Every declaration must be observable.** Nothing a `SourceDescriptor` declares
  (source identity, store epoch, cursor semantics, retention, baseline watermark) may be
  minted by the workbench. Each field derives from a canonical observation or is typed as
  unavailable. A client-invented epoch or an unenforceable retention floor is the exact
  "invented stream offset" the current residual forbids, wearing types.
- **D2 — Replace, don't accumulate.** No phase adds a mechanism beside an equivalent one.
  Phase B deletes the `poll-N` cursor. Phase C demotes `PrimeHostAdapter` to a declared
  test double with a tooth proving it cannot be the production path.
- **D3 — Every new privileged boundary carries an executed attack.** T18's
  `assertInventoryAttackCoverage` already fails the suite when a boundary lacks one; no
  phase may weaken that check to land.
- **D4 — RED first, failures preserved.** Every tooth below must be observed failing
  before it passes, and the failing round is recorded, not discarded.
- **D5 — No bench regression settles.** T17's `FROZEN_BUDGETS` and the T21 budget table
  are re-run at every phase gate. A phase that costs latency states the number and
  re-preregisters, or does not land.
- **D6 — Prime absence never degrades Gauntlet.** Per the spec's own
  `required_config_fields` note on `prime_endpoint` and the `prime_integration_error`
  blast radius: with Prime unconfigured or unreachable, every Gauntlet inspection and
  operation path must behave exactly as it does today. A tooth enforces this at every
  Prime-touching phase.
- **D7 — Unsupported stays unsupported until declared.** REQ-OPS-003: "Missing operations
  render unsupported, never a shell-command fallback." No mode may be bound before its
  operation contract exists (Phase C1).

---

## 4. Phases

### Phase A — Repair the false-green SSH leg *(do first; smallest; a defect in settled evidence)*

**Why first.** `ssh-target.test.ts:227` invokes
`ssh -o … localhost 'cd … && bun run src/main.ts'` with no `-t`/`-tt`, so sshd allocates no
remote pty; `main.ts:57` then takes the one-shot non-TTY dump path, which prints exactly
the markers the leg asserts (`Gauntlet`, `NAVIGATOR`, `INSPECTOR`) and exits before the
`send-keys q`. The leg cannot fail. Supplying an sshd fixture *first* would convert an
honest visible skip into a green result that qualifies nothing — strictly worse than today.
So the environment is the second half of this phase, not the first.

**Work.** Add `-tt`. Add assertions reachable only on the TTY path: alternate-screen entry,
a keystroke effect only raw mode can consume, and a geometry change from a `SIGWINCH`
delivered *through the ssh channel* (the three-state assertion the tmux leg already makes
at `ssh-target.test.ts:258-277`). Add the negative control. Then add an env-gated,
self-contained sshd fixture (own host keys, isolated `known_hosts`, never the operator's
ssh config, never on by default — same gating idiom as `GAUNTLET_T24_REAL_PRIME`).

**Teeth (RED first).**
1. With `-tt` withheld, the leg **fails**. This single tooth is what converts the leg from
   a tautology into a qualification; it is the load-bearing one.
2. SIGWINCH through the ssh channel changes rendered geometry.
3. Clean exit 0 with terminal restored, asserted *after* a raw-mode-only keystroke.

**Gates.** `just tui-test ssh-target` (RED, then GREEN) → `just tui-test release-manifest
transport-equivalence` → `just tui-build release` (regenerates `support_facts`) →
`just tui-test all` with the new skip count recorded **deliberately** (the 530/0/9 shape
changes; T25's `plan-traceability.ts` and unbacked-gate sweep must be re-baselined on
purpose, not by accident) → `just tui-preflight conformance` (the gate row moves GATED →
EXECUTED) → `just ruler`.

**Evidence.** The preserved RED round showing the leg passing without `-tt` — the tautology
recorded rather than quietly fixed — then failing under the negative control; SSH-hosted
pane captures at 120x40 and post-resize; the corrected `support_facts` line stating the
qualified shape truthfully (loopback sshd, same host, same surface — *not* a remote
deployment, per prereg D1); the re-baselined `tui-test all` table.

**Closes.** CLAIM-003's open gap for the SSH half. Re-level CLAIM-003 in Phase F.

---

### Phase B — Bind the projection integrity contract in production *(the foundation)*

**Why.** REQ-SYNC-001 requires each adapter to *declare* source identity, store epoch,
cursor semantics, retention, and baseline watermark. REQ-DATA-002 requires baseline reads
and incremental streams reconciled through a ProjectionGeneration and source cursors.
Production does neither: `workbench-host.ts:100-160` hand-rolls a `membership`/`previous`
string diff and a `poll-N` counter that addresses nothing, beside a fully-built,
fully-tested client that nothing imports. REQ-VERIFY-006 and REQ-SAFE-005 are proven only
against `InMemoryStreamPort`.

**The design decision that keeps this out of debt (D1).** Implement
`CanonicalSurfaceStreamPort` over the *existing* §20.2 routes, with every declaration
derived from canonical observation:

- **source identity** — the configured `gauntlet_endpoint` plus the run id from `GET /state`.
- **store epoch** — derived from `GET /capabilities` (`protocol_version`, `server_version`)
  and the current run id. Observed, never minted. A server restart or run change replaces
  the epoch and correctly forces a fresh baseline — which is precisely what the host's
  existing membership-change branch does today, now expressed in the contract's own terms
  instead of ad hoc.
- **cursor semantics** — an `observation_ordinal` over admitted observations, opaque, never
  lexically compared (REQ-SYNC-001 says so explicitly).
- **retention** — typed as **connect-scoped**. The port cannot serve increments from before
  the client connected, and says so; a cursor older than connect validates as
  `retention_expired` and takes a fresh baseline. That is the honest answer, not a
  workaround.
- **baseline watermark** — the ordinal of the observation the baseline was folded from.
- **granularity** — a new typed field on the descriptor declaring
  `snapshot_interval`: *events occurring between two observations are unobservable by
  construction*. This is the field that prevents the port from wearing a durable stream's
  costume. If it cannot be expressed cleanly in the port's types, **this phase stops** and
  the residual is sharpened instead (see §6).

Then wire `LiveStreamClient` into `WorkbenchHost`, deleting the hand-rolled diff and the
`poll-N` string (D2).

**Hard containment line.** The gateway's pre-admission observation
(`operation-gateway.ts:111-116`) must continue to read through `route.observe` and **never**
through the stream projection. If a drive ever pre-admits against streamed state, the
router-not-authority invariant is broken. This gets its own tooth.

**Teeth (RED first).**
1. Headless-replay equivalence: the streamed host produces a **byte-identical normalized
   projection digest** to the polled host over the same fixture (`headless-replay.ts` is the
   independent referee — the same role it played for T22).
2. The port cannot report a retention floor or epoch it cannot honour (D1 enforced in test).
3. No drive's pre-admission observation is served from the stream projection.
4. A cursor survives a real detach/reattach against the **derived** port, not the in-memory one.
5. A mid-poll backend restart produces a visible `resyncing` transition, never a silent
   cursor advance (REQ-SYNC-001, REQ-SAFE-005).
6. Focus survives a generation restart exactly as it does today (`focus-identity.test.ts` —
   the host currently clears focus on membership change at `workbench-host.ts:117-124`, and
   `LiveStreamClient`'s reconciliation has its own opinion; the observable behaviour must
   not change silently).

**Gates.** Port lands: `just tui-test live-streams`, `just tui-test cold-rebuild
detach-reattach reconnect-races stale-frame`. Host swap: `just tui-test workbench-host
production-entry`, `just tui-test headless-replay`, `just tui-test projection-generation
focus-identity`, `just tui-test cross-surface-equivalence operation-isomorphism`,
`just tui-test renderer-boundary` (a new state module must not import presentation).
Settlement: `just tui-check`, `just tui-lint`, `just tui-preflight conformance`,
`just tui-test all` (49 suites, 0 fail), `just tui-bench focus-latency` and
`just tui-bench projection-worker` against T17 `FROZEN_BUDGETS` (D5),
`just tui-build release` with a fresh PTY launch and byte-identical rebuild, `just ruler`,
`graft build`. `just verify` not required (no Rust change) — the decoupling recorded in the
repair followup plan.

**Evidence.** New evidence directory carrying: the polled-vs-streamed digest equality
record; the port's typed unobservability disclosure verbatim; the detach/reattach receipt
over the derived port; the bench table against frozen budgets; the preserved RED round
showing the digest tooth failing before the swap was correct.

**Closes in production.** REQ-DATA-002/003/004/007, REQ-SYNC-001, REQ-SAFE-005,
REQ-VERIFY-006, and the observation half of REQ-OUT-003.

**Explicitly not closed.** A durable backend-ordered stream. That needs a new route in the
§20.2 table and a persisted sequence/epoch in the core — a public-interface *and*
persisted-schema change requiring explicit approval, and it is **not required** by this
spec: REQ-DATA-002 asks for baseline-plus-incremental reconciled through generations and
cursors, which Phase B delivers. Residual text in §6.

---

### Phase C — Declare, then bind, the Prime operation contract

REQ-OPS-003 dictates the split: every exposed mutation must name its owner, actors, typed
inputs/results, preconditions, authority, containment, budget, idempotency, and evidence
contract **before implementation**. So declaration is not preamble — it is the spec's own
mandated first step, and it produces a durable artefact even if C2 never lands.

#### C1 — Declaration only, nothing executes *(can run in parallel with Phase B)*

Extend the versioned capability handshake to distinguish **existing** operations
(read / refresh / resolve / retry — today's `OPERATION_REGISTRY`) from **planned** ones
(launch, steering, cancellation, evidence admission, session operations). For each planned
operation write the full nine-field contract. Composer modes stay unsupported and keep
reporting so (D7) — but they now report *why*, naming the declared-but-unbound operation
rather than a bare "no adapter connected".

**Teeth.** A planned operation without a complete nine-field contract fails the suite. An
operation exposed as invocable while still classed `planned` fails the suite. The handshake
version increments and old clients see the distinction.

**Gates.** `just tui-test operation-registry`-scoped suites, `just tui-test
cross-surface-equivalence operation-isomorphism`, `just tui-check`, `just tui-lint`.

**Value if C2 never lands.** The residual becomes a *declared roadmap with a contract*
rather than an absence — which is what `claim_discipline.roadmap` is for.

#### C2 — Bind the real Prime substrate

**Config (additive; REQ-CONFIG-003 names "socket" as MUST-configurable).** Add
`prime_binary` and `prime_socket`. Note the mismatch this resolves: today `preflight.ts:184`
probes `${prime_endpoint}/health` over HTTP, while T24 proved the real substrate is a binary
plus a Unix daemon socket. REQ-BUN-005 prefers a local socket for same-host integration.
`prime_endpoint` stays for remote/HTTP-shaped Prime; the socket path is the same-host one.
Nothing hardcoded (the T24 battery's `/home/sprime01/.local/bin/prime-agent` must not
migrate into production).

**Adapter.** A production `RealPrimeHost` implementing the `PrimeHostAdapter` shape over
`Bun.spawn`, under the existing T13 spawn-env bound (PATH / `LD_*` / `DYLD_*` injection
family, program resolution provenance, anchored resolution of bare names). Driven **only**
through `PrimeDelegationDispatcher`, so the canonical accepted receipt precedes every spawn
(REQ-PRIME-003) and the handle is never the result. `PrimeHostAdapter` is demoted to a
declared test double (D2) with a tooth proving it cannot be the production path.

**Availability, not inventory.** `WorkbenchHost.read` currently reports `installed: true`
with every gate `false` (`workbench-host.ts:88-99`) — the honest shape, and it must not
loosen. A qualified-but-unreachable Prime is `installed: true, available: false` and
refuses submission. NONGOAL-004 in force.

**Mode bindings, each with its named rule:**

| Mode | Binds to | Rule that must hold |
|---|---|---|
| `prime` | `PrimeDelegationDispatcher` → real child session | REQ-PRIME-003: receipt before spawn; handle ≠ result. REQ-PRIME-001: project real sessions and topology, never synthetic records |
| `message` | Prime mailbox delivery | REQ-PRIME-005: communication only. May update Chat/Trace/attention; **may never alone establish completion**. REQ-DATA-005: Chat and Trace stay separate projections |
| `kernel` | Prime-owned Python REPL mediation path | REQ-PRIME-006: input/control surfaces through workbench Attention and returns through Prime's mediation. REQ-PRIME-004: never concurrent shell executions against one single-threaded REPL. REQ-PRIME-007: objects classed ephemeral/pinned/artifactized; promotion to evidence explicit and provenance-preserving. NONGOAL-006: this is **not** a sandbox and must never be described as one. REQ-OUT-004: never an admission path |

**Teeth (RED first).**
1. A composer `prime` submit against a **denial-class** canonical route spawns **zero**
   processes (the shape already sketched at `security-hardening.ts:183`).
2. Two concurrent submits with the same `child_session_id` spawn once; the second is
   refused with zero side effects.
3. A child that replies "done" in the mailbox settles nothing without a schema-valid result
   contract at the canonical locator (REQ-PRIME-005, REQ-SAFE-004, and the spec's own
   false-completion condition).
4. A forged or substituted binary path is refused, not resolved (T13 bound extended).
5. No auth material appears in any projection row, frame, log, or evidence artifact
   (`secret-leakage.test.ts` extended; REQ-CONFIG-002).
6. `installed: true, available: false` renders unavailable and refuses submission.
7. **D6:** with Prime unconfigured *and* with Prime configured-but-unreachable, every
   Gauntlet inspection and operation path behaves identically to today.
8. No kernel callable can perform an action Gauntlet would deny (`deny-bypass.test.ts`
   extended across the live kernel path — REQ-VERIFY-004, REQ-OUT-004).
9. Concurrent kernel submissions serialize against one REPL session (REQ-PRIME-004).
10. Every new privileged boundary appears in the T18 inventory with an executed attack (D3).

**Gates.** `just tui-test prime-host prime-recursion prime-kernel prime-topology` →
`just tui-test security-boundaries secret-leakage unsafe-input-boundaries deny-bypass` →
`just tui-test cross-surface-equivalence operation-isomorphism` (the
one-idempotency-identity-across-all-routes pin must survive a new route) →
`just tui-test substrate-inventory` → the gated real battery
(`GAUNTLET_T24_REAL_PRIME=1`, per-run isolation: own agent dir, daemon socket, session dir,
auth copied `0700` and shredded, operator state hash-verified untouched) →
`just tui-test all` → benches (D5) → `just tui-preflight conformance` →
`just tui-build release` → `just ruler` → `graft build`.

**Evidence.** The nine-field operation contracts from C1 as a settled artefact; the
production-adapter provenance rows added to `substrate-inventory.ts`'s three-way map
(`exposed-by-real-binary` for what the production path now genuinely uses); the gated real
battery transcript; the preserved RED rounds for teeth 1, 3, and 8 in particular.

**Closes.** REQ-GOAL-003 in production, REQ-PRIME-001..008, and the delegation half of the
preregistered desired outcome.

---

### Phase D — Long-running lifecycle and reattachment in production

**Why separate.** T24 proved cross-process survival and fresh-client reattachment against
the real substrate — in a gated battery. REQ-VERIFY-005, REQ-PRIME-009, and REQ-PRIME-010
are about the *product*: closing the TUI must not terminate admitted Prime work, and a
restarted workbench must recover topology and session references from qualified
Prime/Gauntlet records rather than infer status from stale UI or unvalidated
session-directory presence.

**Depends on** C2 (there must be real work to survive) and B (there must be an honest
observation path to reattach into).

**Work.** `WorkbenchHost.close()` must not signal admitted Prime work (today it closes only
the model and timer, which is already correct — this phase proves it and keeps it true once
C2 gives the host children). Restart reconstructs topology from qualified records only.
An explicit canonical stop/cancel is the *only* thing that terminates admitted work
(REQ-PRIME-009).

**Teeth (RED first).** TUI SIGKILL leaves the child live with identical ids, and a fresh
workbench process re-delivers to it. Deleting the entire projection store and restarting
reattaches from authoritative records alone (REQ-VERIFY-001 extended over Prime).
Session-directory presence *without* a qualified record does **not** produce a "running"
projection — it produces typed-unavailable (REQ-PRIME-010's explicit prohibition). An
explicit canonical stop does terminate.

**Gates.** `just tui-test prime-long-running detach-reattach recovery-matrix cold-rebuild`
→ gated real battery → `just tui-test all` → benches → `just tui-preflight conformance` →
`just ruler`.

**Closes.** REQ-VERIFY-005, REQ-PRIME-009/010, and the remaining half of REQ-OUT-003.

---

### Phase E *(optional, explicitly deferred)* — Unix-socket local transport

REQ-BUN-005 is a **SHOULD**: same-host integration should prefer a local transport such as a
Unix socket while preserving the same application protocol for remote modes. Phase C2 brings
a Prime socket in; extending the *Gauntlet* surface to a Unix socket would additionally
satisfy REQ-BUN-005 for the canonical path and give REQ-CONFIG-005's local/remote
distinction a second real mode to prove equivalence across.

**Deferred, not folded in,** because it touches the canonical surface (a new listener shape
in the Rust adapter) and this plan's §2 boundary forbids that without escalation. Recorded
here so it is a decision rather than an omission.

---

### Phase F — Re-level the claim ledger and settle

No code. With A–D landed: re-level CLAIM-003 (`partially_proven` → `evidence_backed` for
the SSH/tmux target matrix), CLAIM-006 (`assumption` → `evidence_backed`: its
required_evidence is exactly "integration test of session lifecycle, messaging, kernel
events, steering, extension prompts, and reattachment", which C2+D deliver against the
installed version), and CLAIM-001 (`partially_proven` → `evidence_backed`). Rewrite the
residual section of `.agents/plans/tui-runtime-repair-followup.md` to point here and to
carry only §6's surviving residual. Update `.agents/CURRENT_STATUS.yml`.

**Gate.** `just tui-preflight conformance` must show every touched requirement EXECUTED
with an evidence-backed row, and the unbacked-gate sweep must be clean.

---

## 5. Ordering and why

**A → B → (C1 ∥ B) → C2 → D → F.** E is out-of-band.

- **A first** because it is a correctness defect inside *settled* evidence, it is the
  smallest item on the list, and it is confined to one test file. A false-green tooth in a
  settled campaign outranks any absent capability.
- **B second** because every later phase reads through the host's projection path. Rewiring
  that path after Prime children, mailbox messages, and kernel objects are feeding into it
  multiplies the regression surface by everything C2 adds. B also converts the observation
  contract from tested-in-isolation to load-bearing, which is what makes D's reattachment
  claims mean anything.
- **C1 in parallel with B** — it is declaration work with no dependency on B, and REQ-OPS-003
  requires it to precede C2 regardless.
- **C2 after B and C1** because delegating work you cannot then observe in order is worse
  than not delegating it, and because D7 forbids binding a mode whose contract does not exist.
- **D after C2** because there must be real admitted work before "survives TUI loss" is a
  claim rather than a simulation.
- **F last** because claim levels follow evidence, never precede it.

---

## 6. The residual that survives this plan

Only one, and it should be stated in these terms:

> The canonical Gauntlet surface exposes snapshots, not an ordered durable event stream:
> its route table is the nine §20.2 endpoints and contains no event or offset route. The
> workbench therefore reconciles baseline reads with increments derived from successive
> canonical observations, through a port that **declares** its granularity as
> snapshot-interval: events occurring between two observations are unobservable by
> construction, the retention floor is connect-scoped, and the store epoch is derived from
> observed canonical facts (protocol version, server version, run identity) rather than
> minted. No backend offset is invented or implied. A durable resumable stream would
> require a new public route and a persisted sequence and epoch in the core — a
> public-interface and persisted-schema change — and is not attempted without explicit
> approval. It is not required by REQ-DATA-002, which asks for baseline-plus-incremental
> reconciled through generations and cursors.

**Tripwires that reopen it:** any proposal to persist a workbench cursor as though it
addressed backend state; any core change introducing a monotonic event sequence; any
operator-visible symptom traced to between-observation invisibility (a missed transient
state, an attention cue that never surfaced).

**Tripwire for Phase B itself:** if the `granularity: snapshot_interval` declaration cannot
be expressed cleanly in the port's types — if honouring D1 requires a caveat that lives in
prose rather than in the contract — Phase B **stops**, `WorkbenchHost` keeps its `poll-N`
loop, and the residual above is published as-is with the `LiveStreamClient` recorded as
built-and-unwired-pending-a-real-backend-stream. A legible honest poll loop beats a
1285-line client whose vocabulary overstates its source.

---

## 7. Size and blast radius

| Phase | Rough size | Blast radius | Regression surface |
|---|---|---|---|
| A | ~80–150 lines, 1 test file + fixture | `ssh-target.test.ts`, `support_facts` string | `release-manifest`, `transport-equivalence`, T25 traceability baseline |
| B | ~300 new, ~80 deleted, 1 production module rewired | `WorkbenchHost` → `main.ts` only (+2 test files); `GauntletReadClient` stays for its six other importers incl. `preview.ts` | projection generation, focus identity, renderer boundary, frozen budgets |
| C1 | ~150 lines + contract documents | handshake + registry declarations; nothing executes | isomorphism pins |
| C2 | ~600–900 lines + gated battery | contained in the call graph (`WorkbenchHost` has 3 importers) but **unbounded in kind**: the workbench becomes a process parent holding credentials | T18 boundary inventory (21 entries, attack-coverage-enforced), secret leakage, deny-bypass, isomorphism |
| D | ~200 lines + battery legs | lifecycle only | recovery matrix, cold rebuild, detach/reattach |
| F | 0 | records only | preflight conformance sweep |

C2 is the phase that changes the system's character, and it is the one to stop at if
appetite runs out: A+B+C1 leave the workbench strictly more honest and more capable with no
new process, no new credential handling, and no new authority, and they leave C2 as a
fully-specified declared roadmap rather than an absence.
