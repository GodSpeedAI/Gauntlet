# T23 — Independent cross-surface affordance and no-bypass confirmation (builder evidence)

Task: gauntlet-tui-prime-workbench-T23 (P3, `independent_adversarial`).
Status: **BUILDER evidence pending independent adversarial confirmation.**
Prereg: `.agents/research/preregistrations/task-gauntlet-tui-prime-workbench-T23.prereg.yaml`
(verified `just prereg-check gauntlet-tui-prime-workbench-T23` → PASS,
sha256 bc9f693e85070086ae31e590d871e60f86a0b69e63a9447c6e3d03c3c4480c7c; never modified).

---

## 1. FROZEN representative matrix (written BEFORE inspection of builder evidence)

This section was written before this builder inspected any T09/T10/T11/T12/T15/T18/T20
implementation evidence or deep source, satisfying prereg distinction **D1**. The only
sources consulted before this freeze were the rulers: the source spec
(`.agents/specs/gauntlet-tui-prime-workbench.spec.yaml`), the plan T23 block, and the
prereg itself.

### 1.1 Allowed representative — canonical UNIT RETRY under a genuinely holding precondition

* **Operation (frozen semantically):** the canonical *retry of a blocked Unit* — one of
  the spec's existing operation set "read/refresh/resolve/retry" (REQ-OPS-003), chosen
  because it is consequential (state transition) and precondition-bearing.
* **Precondition (frozen):** at invocation time the actor currently holds retry
  authority; the target Unit is in the blocked state that a retry addresses; the
  canonical precondition version bound into the request identity is current (not stale)
  at the write boundary.
* **Expected canonical identity (frozen, per REQ-OPS-002):** request identity binds
  `{actor, operation, target unit, payload digest, canonical precondition version}`.
  Every route presents the SAME idempotency identity for this representative.
* **Expected result (frozen):** exactly **one** canonical admission; a typed success
  result carrying the canonical `operation_id`; **the side effect happens once in
  total**, not once per route — the first route to present the identity performs it,
  and every other route presenting the same identity receives the identical canonical
  result as an idempotent replay with zero additional side effect. Side-effect counters
  across all routes sum to exactly 1 for this representative. Projection choice does
  not multiply side effects.

### 1.2 Denied representative — canonical UNIT RESOLVE under post-render authority revocation

* **Operation (frozen semantically):** the canonical *resolve of an escalation/Unit* —
  the second precondition-bearing member of the spec's existing operation set; chosen
  because the workbench affords it as an operator control and because "resolving an
  escalation is not a generic approve button" (REQ-UX-003).
* **Denial setup (frozen):** the affordance is rendered/obtained first, then the actor's
  authority for resolve is **revoked after render** (post-render authority change,
  prereg D3); the invocation is then attempted. Revalidation MUST occur at the write
  boundary (REQ-OPS-002, REQ-AFF-003).
* **Expected result (frozen):** the **same typed canonical denial on every route** —
  deep-equal on denial class, `error.code`, and canonical identity — with **zero side
  effects on every route** (REQ-VERIFY-004, REQ-SEC-002). The refusal reason codes
  survive every projection unchanged.

### 1.3 Stale-control representative

* **Setup (frozen):** the allowed representative's affordance/identity is captured,
  then canonical state is advanced so the bound canonical precondition version is no
  longer current; the captured (stale) invocation is then presented on every route.
* **Expected result (frozen):** denial **before side effect on every route**; reason
  class is the stale/precondition class, distinct from the authority class of §1.2.

### 1.4 Denial-reason variation (prereg parametric variation, L2)

Four frozen reason classes, each expected preserved verbatim through every route's
refusal (no projection may rewrite a specific denial into a generic error):

1. **governance** — operation refused by governance/authority policy;
2. **payment** — budget/payment exhausted or insufficient;
3. **settlement-access** — operation requires settlement access the actor lacks;
4. **stale** — canonical precondition version no longer current.

### 1.5 Binding note (frozen)

Operation identity spellings on the wire are bound to whatever the versioned capability
handshake actually declares for these two operations (REQ-OPS-003 handshake); the
frozen representatives are the *retry of a blocked Unit* and the *resolve of an
escalation under revoked authority* by semantic role, and the observed wire spellings
are recorded verbatim in §4 (results). Any mismatch between wire spelling and these
semantic roles is a finding, not a silent re-freeze. If the implementation lacks one of
the two semantic roles entirely, that is recorded as a gap against this frozen section.

---

## 2. Route registry as it stands (post T09/T10/T11/T12/T15/T20)

Consolidated in `workbench/src/testing/cross-surface-matrix.ts` (`ROUTE_REGISTRY`) —
the matrix every future change re-runs. Wire spellings bound to the capability
registry (`OPERATION_IDS`): `unit.retry` / `unit.resolve` — exactly the frozen
semantic representatives of §1; no re-freeze was needed.

| # | route id | seam (owning task) | drive path | side-effect source | key | est. | sess. |
|---|----------|--------------------|-----------|--------------------|-----|------|-------|
| 1 | `gateway-http` | `operation-gateway.ts` (T09) | `OperationGateway.invoke` → `HttpOperatorActionRoute` (loopback serve wire) | surface settlement/ticket/admission counters | yes | yes | yes |
| 2 | `gateway-shim` | `testing/canonical-surface-stub.ts` (T09) | `OperationGateway.invokeVia(asInProcessRoute)` (shim re-validates session) | same | yes | yes | yes |
| 3 | `direct-api` | stub `perform` (T09 mirror) | raw API shape, run-anchored, no workbench code | same | no | no | no |
| 4 | `direct-cli` | stub `performViaCli` (T09 mirror) | raw CLI shape, ticket-anchored | same | no | no | no |
| 5 | `kernel-callable` | `kernel-affordances.ts` (T11) | `KernelActionCallable.invoke` → `KernelAffordanceSet.invoke` → `OperationGateway.invoke` | same | yes (minted) | **no** | yes |
| 6 | `plugin-projection` | `plugins.ts` (T15) | `affordanceProjections()` → `projectionDescriptor` (T09 resolver predicts) → HOST `gateway.invoke(projection.operation)`; plugins hold no gateway | same | yes | yes (projection's own estimate) | yes |
| 7 | `prime-delegation` | `prime-recursion.ts` (T10) | `PrimeDelegationDispatcher.delegate` → its gateway `invokeVia(PrimeDelegationRoute)` → canonical dispatch FIRST, child spawn behind acceptance | surface counters **plus** Prime adapter session ledger | yes | yes | yes |
| 8 | `shell-effect` | `shell.ts` (T12) | palette Enter → `ShellEffect invoke_affordance` → `applyShellEffects` → `interactions.invokeAffordance` → host `gateway.invoke`; shell holds no gateway | same | yes | yes | yes |

Honest non-invocation surfaces (`NON_INVOCATION_SURFACES`, recorded — not forced):

* **`release-host-loop` (T20)** — the released standalone artifact is a cold,
  read-only host loop (launch qualification renders and quits); it exposes no
  invocation entry, so there is no consequential route to exercise yet.
* **`read-client` (T00A)** — finding of this build: `client.ts` is a read
  client whose ONLY POST is the T18 security probe `mutateUnit`, aimed at the
  SAME canonical action endpoint the gateway route drives. Recorded as a test
  instrument over an existing route, not an independent invocation route
  (transport auth can never authenticate an unauthorized operation,
  REQ-SEC-002). The earlier "emits zero mutations" header claim is true of the
  read surface; the probe is disclosed, pinned by the static suite.

## 3. Method

`workbench/src/cross-surface-equivalence.test.ts` (13 tests, 465 assertions)
executes the frozen matrix of §1 through every registered route over ONE
canonical admission body per scenario (`CanonicalSurfaceStub` — the suite's
stand-in for the ONE Rust write path), with:

* **per-route fresh scenarios** for (a) allowed, (b) denied (visible + racing
  revocation), (c) stale control — no cross-scenario state;
* **one shared scenario** for the prereg P2 identity test: the same
  idempotency identity presented through all 8 routes;
* **parametric variation** (prereg L2): governance / payment /
  settlement-access / stale-run through every applicable route;
* **static routing inspection** in the same suite: importer allowlist of the
  ONE gateway, the exactly-two `OperatorActionRoute` implementors, single
  `gateway.invoke`/`invokeVia` call sites, the single `decide` body with the
  only settlement increment, plugins/shell holding no gateway, and the T19
  renderer-boundary import ban re-checked over the admission-side modules.

## 4. Results

Gates (all exit 0):

```
just tui-test cross-surface-equivalence   → 13 pass, 0 fail (466 expect calls)
just tui-test deny-bypass                 → 16 pass, 0 fail (293 expect calls)
just tui-check                            → see §7 (parallel-wave status)
just tui-lint                             → see §7 (parallel-wave status)
just tui-preflight bootstrap              → PASS
cd workbench && bun test                  → see §7 (parallel-wave status)
```

Executed-matrix findings:

* **(a) Allowed `unit.retry` (anchored, precondition genuinely holding)** —
  every route returns the IDENTICAL canonical accepted facts
  `{run_id, unit_id: AK-923, tickets_resolved: [ESC-923], committed_now: true}`;
  `settlements_committed == 1` per scenario, total — never per-route. The Prime
  route additionally spawns exactly one nonblocking child behind the ONE
  accepted drive.
* **(P2) One idempotency identity across all projections** — first drive
  accepts (`committed_now: true`); the shim, kernel, plugin, and shell routes
  re-enter the SAME settlement under the SAME key (deterministic
  `operation:target:actor:digest` identity) and report `committed_now: false`
  with zero additional effect; the Prime dispatcher's own gateway mints the
  SAME key, passes its fresh gate, re-entries canonically (`committed_now:
  false`) and spawns one child; the raw API shape re-enters identically. THE
  assertion: `settlements_committed == 1` and `tickets_resolved == 1` after
  eight routes. Key reuse with a changed payload is refused before routing
  (`idempotency_key_payload_mismatch`, zero admission attempts). Disclosed
  pinned truth: the ticket-anchored CLI shape's re-drive names a ticket that no
  longer exists → formation refusal `action_failed`, zero effects.
* **(b) Denied `unit.resolve` after post-render authority revocation** —
  *visible in reads:* gateway-family routes deny PRE-admission (typed
  `stale_affordance_denied` with reason `authority_not_permitted` preserved,
  zero admission attempts); the raw shapes deny AT the write boundary
  (`authority_not_permitted`); zero settlements and zero ticket resolutions on
  every route; no Prime child spawns; the kernel wrapper is invalidated
  (projection consequence only). *racing (hidden from reads):* the denial is
  caught ONLY at canonical admission — and is the IDENTICAL normalized denial
  on ALL EIGHT routes (`refused` / `authority_not_permitted` /
  `[authority_not_permitted]` / not-pre-admission), one admission attempt each,
  zero effects.
* **(c) Stale control** — retry rendered while the unit was `fail`, canonical
  status advanced to `pass`, invocation on every route: refused
  `readiness_not_earned`, zero effects everywhere; workbench routes deny before
  admission, raw shapes at admission.
* **(L2) Denial-reason variation** — `blocking_residual_open` (governance)
  preserved on all 8 routes; `budget_exhausted` (payment) preserved on the
  five estimate-carrying routes; `actor_authentication_required`
  (settlement-access) preserved through the gateway family, the Prime route and
  the raw HTTP transport's wire auth gate, with the CLI shape preserving its
  own `operator_identity_required`; `run_not_durable` (stale anchor) preserved
  IDENTICALLY on all 8 routes. No route rewrote a specific denial into a
  generic error.
* **Static inspection** — the gateway-importer pin is a WHOLE-TREE scan:
  every `.ts`/`.tsx` under `src/` is enumerated and classified, and ANY file
  outside the allowlist that imports the gateway module (by any import form,
  or by binding the `OperationGateway` symbol from any module — closing the
  barrel bypass) fails the suite, as does an allowlisted file that vanished
  (§8, Correction Round 1 MAJOR-1); exactly two transports implement
  `OperatorActionRoute` (`HttpOperatorActionRoute`, `PrimeDelegationRoute`);
  the kernel callable and the Prime dispatcher route through the gateway
  exactly once; plugin projections and the shell hold no gateway and no
  `.perform(` path; both direct surface shapes terminate in the ONE `decide`
  body holding the only settlement increment; the T19 renderer-boundary import
  ban holds for every admission-side module.

## 5. Honest gaps and disclosures

1. **Scope is representative, not exhaustive** (prereg blindspot, honored):
   this proves the architecture with the two frozen representative operations
   plus structural checks that every supported route shares one gateway — not
   every product operation individually.
2. **The eligible-Prime-context route is bounded by the T10/T24 adapter-model
   scope**: in-process contracts only; cross-process Prime is T24 work. The
   dispatcher's Prime side effect measured here is an in-process adapter child
   session, not a cross-process Prime lifecycle.
3. **Payment denial is honestly ABSENT on the kernel callable and the raw
   surface shapes**: `KernelInvokeRequest` structurally carries no resolution
   field (pinned statically), and the canonical admission body has no
   affordability gate (Correction Round 1 MAJOR 1). Payment is a workbench
   resolver policy. Not a rewritten refusal — an absent one.
4. **The ticket-anchored CLI shape cannot re-drive a settled escalation** (its
   anchor is consumed by the settlement): formation refusal `action_failed`,
   zero effects. Recorded, not smoothed.
5. **The canonical admission body under test is the suite's stand-in**
   (`CanonicalSurfaceStub.decide`), mirroring the Rust operator-action path's
   vocabulary; the same matrix re-runs against the real surface whenever the
   transport points at `gauntlet serve`.
6. **client.ts's `mutateUnit`** is a mutation-capable probe on the read client
   (T18 attack instrument). It shares the canonical endpoint and wire auth
   gates; recorded in the registry rather than hidden.

## 6. What the independent adversarial verifier should attack

* Drive the denied operation through **`plugin-projection` with a forged
  contribution**: a plugin declaring a non-canonical `operation` must hit the
  registry gate (`plugin_operation_not_canonical`) — try to register a
  projection that carries its own invocation closure instead of
  `projection.operation`.
* Attack the **Prime spawn boundary**: `delegate()` with a reused
  `child_session_id` behind an accepted drive from a SECOND dispatcher over the
  same adapter (the spawn-time duplicate guard), and with a stale envelope
  schema; a denial must never leave a child session behind.
* Attack the **shell edge**: change canonical authority between render and
  Enter and try to make the PALETTE (not the gateway) the enforcement point in
  either direction — the effect must always fire and the gateway must always
  decide.
* Attack **kernel wrapper state**: hand-construct a `KernelAffordanceSet` with
  a permissive route, or invoke from an invalidated set — wrapper currency must
  contribute NOTHING to the decision.
* Attack **idempotency across hosts**: present the same logical invocation
  through two gateway instances and one raw shape concurrently; the canonical
  §16.3 re-entry must keep settlements at one.
* Re-run the matrix with the harness pointed at the **real `gauntlet serve`**
  surface and confirm the refusal vocabulary survives the wire unchanged.
* Add a new gateway importer anywhere in the tree without registering it in
  the allowlist — the WHOLE-TREE static pin must fail the suite (§8: this
  guarantee was corrected in Correction Round 1; the original pin scanned a
  fixed candidate list and was blind to new files).

## 7. Parallel-wave gate status (recorded honestly)

T21 (`payment-budgets.*`, `large-case.bench.ts`, `long-horizon.bench.ts`) and
T22 (`recovery-matrix.*`) had in-flight files in the shared tree while this
builder ran. Timeline of the tree-wide gates, recorded without smoothing:

* First pass: `tui-check` failed ONLY in `src/recovery-matrix*.ts` (T22,
  mid-flight); `tui-lint` failed with 11 errors located only in T21/T22
  in-flight files (verified by per-file scoped runs).
* Retries (~1–5 min apart, per the parallel-wave protocol): T22 settled —
  `tui-check` observed clean; T21's four files continued moving (new errors
  appeared and moved inside `large-case.bench.ts` between runs). During the
  Correction Round 1 re-verification, one full `bun test` run again showed 5
  transient failures and `tui-lint` formatting errors located only in
  `src/recovery-matrix*.ts` (T22 editing again); a capture minutes later and
  every subsequent run were fully green.
* Final state, all gates green (this builder's two files were biome-clean and
  tsc-clean at every scoped check; no tree-gate failure at any point was
  attributable to a T23 file):

```
just tui-test cross-surface-equivalence   13 pass, 0 fail (466 expect calls)  exit 0
just tui-test deny-bypass                 16 pass, 0 fail (293 expect calls)  exit 0
just tui-check                            tsc --noEmit clean                  exit 0
just tui-lint                             biome check: 0 errors               exit 0
just tui-preflight bootstrap              PASS                                exit 0
cd workbench && bun test                  576 pass, 5 skip, 0 fail (581 tests, 55 files)
```

## 8. Correction Round 1 (adversarial verification findings)

**MAJOR-1 (CONFIRMED, fixed): the gateway-importer allowlist pin's
future-change protection was hollow.** The original pin checked the allowlist
against a FIXED candidate list of 22 pre-existing filenames, so a NEW file
importing `operation-gateway` was invisible to every gate. The verifier
demonstrated `src/rogue-projection.ts` (constructs an `OperationGateway` on a
throwaway route) passing the cross-surface suite 13/0, the full bun suite,
`tsc`, and `biome` — while the same import in listed `shell.ts` correctly
failed. This falsified §6's last bullet as originally written.

**Fix (landed):** the pin is now a WHOLE-TREE scan — `readdirSync` recursively
enumerates every `.ts`/`.tsx` under `src/` (the `renderer-boundary.test.ts`
enumeration pattern), classifies each file, and fails when ANY non-test file
outside `GATEWAY_IMPORT_ALLOWLIST` reaches the gateway module — by relative
import in any form (static, dynamic, side-effect, type-only, star re-export)
OR by binding the `OperationGateway` symbol from any module (closing the
barrel `index.ts` re-export bypass) — and also fails if an allowlisted file
vanished. Test/bench files are the declared exempt driver class (they
legitimately drive the gateway without being seams).

**RED-first proof (on a throwaway copy of `workbench/` at `/tmp/t23-red`, with
`src/rogue-projection.ts` added; copy deleted after the demonstration):**

1. OLD pin + rogue: cross-surface suite **13 pass / 0 fail — the rogue was
   invisible** (bug reproduced against the verifier's finding).
2. NEW pin + rogue: the whole-tree test **FAILS** with
   `offenders == ["rogue-projection.ts"]`.
3. NEW pin, rogue removed: **13 pass / 0 fail** on the clean tree.
4. Control (verifier's case, new pin): the same import injected into
   unregistered `shell.ts` **fails** the whole-tree test.

**Verifier informational notes, carried as recorded state:**

* The `OperatorActionRoute` implementor pin is TEXTUAL: route values also
  arise without `implements` — the stub's object-literal shim
  (`asInProcessRoute`) and security-hardening's observe-deadline route
  decorator both yield `OperatorActionRoute` values. Neither is a separate
  authority (both dispatch into the ONE canonical route); the structural pins
  (single gateway call sites, single `decide` body) cover them. The test now
  discloses this next to the pin.
* The prereg's `frozen_at` wall-clock stamp is cosmetically inaccurate.
  Immutability is intact (hash verified); the frozen prereg is NOT touched.
* The verifier's independent registry-completeness scan found NO implemented
  invocation route missing from `ROUTE_REGISTRY` (registry completeness
  holds as of this round).

Confirmation: BUILDER evidence pending independent adversarial confirmation.
