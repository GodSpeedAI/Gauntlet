# T09 Evidence — Canonical Affordance Resolver and Isomorphic Operation Gateway

Date: 2026-09-09
Task: T09
Proof Level: P3 (confirmation: independent_adversarial)
Scope: One canonical operation registry and gateway over the ONE Gauntlet operator-action admission path, AffordanceDescriptor resolution from current observations, invocation-time revalidation with fail-closed denial, actor/session/target/payload-digest/idempotency-key binding with key-reuse refusal and outcome_unknown reconciliation, and the deny-bypass matrix proving denied/stale/unaffordable paths non-executable through every currently implemented route.
Preregistration: frozen at `.agents/research/preregistrations/task-gauntlet-tui-prime-workbench-T09.prereg.yaml` — verified byte-identical this session (sha256 `0c07f855…0ca42` via `just prereg-check`), never modified.

**Recovery attribution:** this slice was recovered from an interrupted builder session — inventory, completion, and corrections by the recovery builder. The orphaned foundation (gateway, registry, resolver, two gate suites, the canonical-surface-stub harness, and the barrel exports) was kept and completed, not rewritten; the full recovery record is below.

## Settled Requirements

- **REQ-AFF-001** (every consequential action references one canonical operation identity; no presentation-specific side effects): `operation-registry.ts` is the ONE workbench place that knows the canonical operation vocabulary (`unit.resolve`, `unit.retry`, `state.refresh` — derived from `DurableSurface::actions()` and the surface-json capability handshake, never invented); the gateway routes only registry IDs, and the deny-bypass matrix asserts `operation === "unit.resolve"` on every record of every route (`operation-isomorphism.test.ts` test 1 pins the registry to exactly the canonical surface's vocabulary; `affordance-resolver.test.ts` renders unregistered operations `unsupported_operation`, never a fallback).
- **REQ-AFF-002** (same operation through every projection converges on one admission/validation/authority/idempotency/result semantics): the convergence control drives one allowed `unit.resolve` through all four implemented routes — gateway→HTTP surface, gateway→in-process shim, direct API-shape surface, direct CLI-shape surface — and deep-compares the normalized accepted shape `{run_id, unit_id, tickets_resolved, committed_now}` across all four (`deny-bypass.test.ts` control leg 1); the same idempotency key re-driven through a DIFFERENT route re-enters the durable settlement with `committed_now: false` and exactly one commit (control leg 2, §16.3 re-entry).
- **REQ-AFF-003** (a control displayed as available is revalidated at invocation time; stale projections fail safely): `OperationGateway.dispatch` resolves a FRESH descriptor from a FRESH canonical observation obtained through the SAME route, immediately before routing; a fresh `unavailable`/`degraded` resolution fails CLOSED with the typed `stale_affordance_denied` refusal before any command is sent (Tooth 1 test: authority revoked between render and invoke ⇒ typed refusal, zero admission contact).
- **REQ-AFF-004** (installed capability is inventory only, never implies available): capability observations ride the observation as declared inventory and are never consulted by the gate evaluation; `affordance-resolver.test.ts` proves a fully-installed, fully-passing capability still resolves `unavailable` with `no_open_ticket` (and stays unavailable under `surface_unreachable`).
- **REQ-AFF-005** (unavailable affordances stay inspectable with machine-readable reasons): `resolveAffordance` returns a frozen descriptor for EVERY input — including unknown operations and multi-gate failures — accumulating reason codes from the canonical refusal vocabulary (`authority_not_permitted`, `blocking_residual_open`, `no_open_ticket`, `readiness_not_earned`, `budget_exhausted`, `surface_unreachable`, `actor_authentication_required`, `target_not_visible`, `observation_degraded`), never throwing the descriptor away.
- **REQ-OPS-002** (recheck preconditions at the write boundary; request identity binds actor/operation/target/payload digest/precondition; key reuse with changed payload rejected; timeouts render outcome_unknown; no automatic retry under a new key): the gateway binds actor, session token, target, FNV-1a payload digest, and a deterministic idempotency key per invocation; key reuse with a changed payload is refused pre-routing on every route (`idempotency_key_payload_mismatch`, zero admission attempts); a lost transport response resolves the typed `outcome_unknown` under the SAME key (never silently retried) and `reconcile` re-drives that key with the SAME payload through canonical re-entry, refusing changed payloads and never-issued keys (`operation-isomorphism.test.ts` tests 4–5; deny-bypass control leg 3).
- **REQ-OPS-003** (versioned capability vocabulary; missing operations render unsupported, never a shell-command fallback): the registry's `surface_capability` tokens mirror the handshake (`action_resolve`/`action_retry`/`state_refresh`); operations the canonical surface does not serve (launch, steering, cancellation, evidence admission, session operations) are ABSENT from the catalog and resolve/refuse as `unsupported_operation`.
- **REQ-CONFIG-011** (revalidate the current affordance/admission before each privileged operation; never trust a stale UI control): the gateway's pre-admission revalidation runs on EVERY invocation (fresh observation, fresh descriptor) — there is no code path from a rendered control to admission that skips it; Tooth 1 and the negative control prove a bypassing route is refused before its first `perform` call.
- **REQ-OUT-004** (kernel and TUI expose the same admitted operation meanings; no privileged side paths): the direct-surface routes (API-shape, CLI-shape) are driven by the same matrix as the workbench routes and converge on the identical canonical denial/acceptance shapes; the negative control proves a forging side path is exposed by the durable ledger.
- **REQ-SAFE-002** (if the affordance cannot be revalidated, fail closed): an unreachable surface produces the typed `revalidation_unavailable` refusal (shim: observation throws; HTTP: 500 → honest unreachable observation resolving `surface_unreachable` + `target_not_visible`) with zero admission contact — never a throw, never an invocation (`deny-bypass.test.ts` REQ-SAFE-002 test).
- **REQ-SEC-002** (no TUI, kernel, plugin, terminal, Prime message, or local adapter path MAY bypass Gauntlet authority): the matrix's direct routes ARE gateway-skipping paths and every denial class is still refused at the ONE admission body with zero side effects; a deliberately forging bypass shim is caught by the matrix's discriminating assertions (divergent result shape + untouched durable ledger) and, when plugged in as a gateway route, is contained by pre-admission refusal with zero `perform` calls (negative control).
- **REQ-UX-003** (operator can inspect denial reason, required authority, and target before resolve/retry; denial states machine-distinguishable): descriptors carry `reason_codes` plus `required_attention_item` (the open escalation ticket an invocation responds to); the Tooth 1 test shows the projection refreshing from `requires_attention` to `unavailable` with the exact reason (`authority_not_permitted`) through the same route — the operator's next-step evidence.
- **REQ-VERIFY-003** (a representative canonical operation resolves through the same operation semantics and gates from every projection): the control matrix drives `unit.resolve` through all four implemented routes to one admission/idempotency/result shape.
- **REQ-VERIFY-004** (a denied operation remains denied through every supported projection and direct local adapter path): Tooth 2a/2b drive four representative denial classes through every implemented route — refusals before or at the single admission body, never past it — with a measurable zero-side-effect ledger on every route.

## Implementation Details

Recovered from an interrupted builder session — inventory, completion, and corrections by the recovery builder.

1. **`workbench/src/operation-registry.ts`** (orphaned foundation, kept): the frozen canonical operation catalog — `unit.resolve`/`unit.retry` (consequential, §16.3 idempotent re-entry) and `state.refresh` (read-shaped, not admitted through the action path) — with `isOperationId`/`lookupOperation`/`operationIds`. Recovery corrections: lint-only (`Object.hasOwn` for the `hasOwnProperty` builtin, import/format normalization).
2. **`workbench/src/affordance.ts`** (orphaned foundation, kept): the pure `AffordanceDescriptor` resolver — `resolveAffordance(operation, focus, observation, options)` evaluating visibility, reachability, settlement access (authenticated session + authority not revoked), governance, payment (estimate vs observed budget; absent constraint is honest absence), and operation-specific canonical preconditions (open escalation for resolve; failed verification for retry); `statusPermitsInvocation` permits only `available`/`requires_attention`. Frozen descriptors; optional fields attached only when defined (`exactOptionalPropertyTypes`). Recovery corrections: lint/format only.
3. **`workbench/src/operation-gateway.ts`** (orphaned foundation, kept): the ONE router, never a second authority — `invoke` (registry gate → binding/key-ledger → fresh revalidation fail-closed → route to canonical admission → typed canonical outcome), `invokeVia` (alternate route, same gateway semantics), `reconcile` (same key, same payload, canonical re-entry), typed `GatewayOutcome` (`accepted`/`refused` with `stale` flag + preserved `reason_codes` + fresh `descriptor` / `failed` / `outcome_unknown`), and `HttpOperatorActionRoute` (the production loopback transport mirroring the surface-json wire conventions; fetch rejection THROWS so the gateway resolves outcome_unknown). RE-DRIVES (keys this gateway already issued) enforce observability fail-closed but not the descriptor gate: a projection that changed because an earlier drive succeeded must never block reporting canonical durable truth; idempotent canonical re-entry decides. Recovery corrections: removed one unused import (`PaymentEstimate`), renamed one unused parameter (`_target`), import/format normalization — no behavioral change.
4. **`workbench/src/testing/canonical-surface-stub.ts`** (orphaned foundation, kept): the in-process canonical-surface test harness — formation/refusal codes from the canonical vocabulary, read-visibility knobs separating what READS show from what ADMISSION enforces (modeling stale projections vs denials racing past revalidation), side-effect counters (`settlements_committed`, `tickets_resolved`, `admission_attempts`) making "zero side effect" measurable, both surface entry shapes (`perform` API-shape, `performViaCli` CLI-shape) over the ONE decision body, a loopback HTTP server with the real wire conventions (session auth, operator binding, run anchor), and the in-process shim route. Recovery corrections: lint-only (`_`-prefixed unused parameters, `import type`).
5. **`workbench/src/index.ts`** (orphaned barrel additions, kept): exports `./affordance`, `./operation-gateway`, `./operation-registry`.
6. **`workbench/src/operation-isomorphism.test.ts`** (orphaned, COMPLETED by recovery): the builder died mid-refactor leaving 6 tsc errors — union narrowing never finished. Completed in the builder's own design language (typed narrowing guards alongside the existing expects): narrowed `outcomeA`/`outcomeC`/`outcomeB` to the accepted variants before `acceptedFacts` (whose parameter type was completed to the accepted variants of both outcome unions), narrowed `second` before `.committed_now`, hoisted `descriptors[0]` into a checked `reference` before deep-comparison. Removed the interrupted refactor's dead leftovers (unused `beforeAll` import, unused `refusedFacts` helper) that failed the lint gate. No assertion was weakened; all 6 tests pass unmodified in intent.
7. **`workbench/src/affordance-resolver.test.ts`** (orphaned, kept): 9 tests — lint/format normalization only.
8. **`workbench/src/deny-bypass.test.ts`** (NEW, recovery builder): the deny-bypass matrix suite — route inventory, stale-control tooth, deny-equivalence matrix (visible and racing variants, four representative denial classes), allowed-operation control with outcome_unknown reconciliation, and the bypassing-shim negative control. Detailed below.
9. **No Rust code was changed.** The design routes over the in-process canonical-surface-stub harness and the client seam; the canonical authority remains `crates/gauntlet-cli/src/durable_surface.rs` `perform(ActionCommand)` and `crates/gauntlet-adapter-surface-json` (T01's `TokenSessionAuthenticator` vocabulary: bearer session + operator binding + `?run=` precondition anchor).

## Recovery record

What the recovery builder found (verified against the interrupted tree):

- 7 untracked files (listed above) implementing the gateway/resolver/registry design, all tests green under `bun test` (80 pass / 0 fail), but `bun run check` FAILING with 6 tsc errors — the builder died mid-refactor.
- All 6 errors in `operation-isomorphism.test.ts`: `GatewayOutcome`/`CanonicalOutcome` union values used where only the accepted variant typechecks (4 sites), a property access on an un-narrowed union, and an `AffordanceDescriptor | undefined` from unguarded index access. Plus 2 latent lint-gate failures (unused import + unused helper from the same interrupted refactor) and biome format/organize-imports drift across the orphaned files.
- `workbench/src/index.ts` modified (tracked file) with the three new barrel exports — consistent, kept.
- No TODO/FIXME/stub markers anywhere in the orphaned files; the gateway/resolver/registry/stub logic was complete and coherent — the interruption was confined to test-file narrowing and lint hygiene.

What the recovery builder fixed:

- Completed the interrupted narrowing refactor in `operation-isomorphism.test.ts` (4 edit sites, same design language, no weakened assertion); removed the dead `beforeAll` import and `refusedFacts` helper.
- Applied biome's format/import-organization fixes across the orphaned files; removed the unused `PaymentEstimate` import and renamed one unused parameter in `operation-gateway.ts`; `_`-prefixed unused parameters in the stub; `Object.hasOwn` in the registry. One over-broad `biome --unsafe` pass that converted index-signature bracket access to dot access (conflicting with `noPropertyAccessFromIndexSignature`) was reverted byte-exactly from a pre-mutation backup before re-applying safe fixes only; the gateway file's final hash was verified identical to the original after that restore, then only the two documented lint edits were re-applied.

What the recovery builder added:

- `workbench/src/deny-bypass.test.ts` (7 tests, 251 expect() calls) — the matrix below.
- This evidence file.

## Teeth Falsification & Verification

### Tooth 1 (plan falsifier; REQ-AFF-003, REQ-CONFIG-011, REQ-SAFE-002): render an operation as available, revoke authority before activation, then invoke from the stale control
- **Attack description**: resolve `unit.resolve` on a blocked unit with an open escalation from a fresh observation (`requires_attention`, invocation permitted); revoke the unit's settlement authority (exposed by reads); invoke through the stale control; then attack the projection-refresh claim by re-resolving through the same route.
- **Defense outcome**: typed refusal (`stale_affordance_denied`, `stale: true`) carrying the canonical class verbatim in `reason_codes` (`authority_not_permitted`) and the fresh `unavailable` descriptor — BEFORE canonical admission (ledger: `admission_attempts: 0`, `settlements_committed: 0`, `tickets_resolved: 0`). The refresh is observable: the fresh descriptor resolves `unavailable` with the same reason, so the projection can degrade the control. A second variant proves defense in depth: a revocation INVISIBLE to reads races past revalidation and is still refused at the admission body (`authority_not_permitted`, `stale: false`) with zero side effects. A third leg proves fail-closed when the surface cannot be revalidated at all (typed `revalidation_unavailable` / `surface_unreachable` refusals, never a throw, zero admission contact).
- **Test**: `src/deny-bypass.test.ts` — "Tooth 1 — a control rendered available whose authority is revoked…", "Tooth 1b — a revocation racing past revalidation…", "REQ-SAFE-002 — an unobservable surface fails closed…".
- **Result**: **PASS**.

### Tooth 2 (plan falsifier; REQ-AFF-002, REQ-VERIFY-004, REQ-SEC-002): attempt the same denied operation through every implemented route
- **Attack description**: the SAME denied `unit.resolve` invocation driven through all four implemented routes — gateway→HTTP surface, gateway→in-process shim, direct API-shape surface call, direct CLI-shape surface call — for four representative denial classes (authority-revoked, governance-denied, unaffordable, stale-snapshot/retired-run anchor), in two variants: denials the fresh reads expose, and denials racing past revalidation (invisible to reads).
- **Defense outcome**: racing variant — all four routes converge on the IDENTICAL canonical denial shape (deep `toEqual` of `{class, code, message}` against the direct-API reference; only the CLI shape's `run_not_durable` message discloses its own anchor, same canonical class), every refusal decided AT the single admission body, zero side effects everywhere. Visible variant — gateway routes refuse pre-admission (`stale_affordance_denied`) while preserving the canonical class in `reason_codes`; direct routes earn the canonical code; both direct entries receive byte-identical canonical denials; zero settlements/tickets on every route. Allowed control — one accepted shape across all four routes; same-key cross-route re-drive commits once (`committed_now: false`); a lost response resolves `outcome_unknown` under the same key and reconciliation re-drives THAT key to the §16.3 re-entry result.
- **Tests**: `src/deny-bypass.test.ts` — "Tooth 2a — denials the fresh reads expose…", "Tooth 2b — denials racing past revalidation…", "control — one allowed operation converges…".
- **Result**: **PASS**.

### Negative control (the matrix has teeth)
- **Attack description**: a `ForgingBypassShim` — a deliberately bypassing route that skips the gateway and forges an `accepted` shape (`forged-run`, `forged-ticket`) without touching admission.
- **Defense outcome**: driven directly (projection skips the gateway), the forged result diverges from the converged canonical shape (`not.toEqual` fires — the matrix's discriminating assertion) AND the durable ledger exposes the forgery (`admission_attempts/settlements_committed/tickets_resolved` all 0; a fresh observation still shows the open escalation). Plugged IN as a gateway route against a denial-visible scenario, the shim is contained: the gateway's pre-admission revalidation refuses (`stale_affordance_denied`) with ZERO `perform` calls reaching the forged transport.
- **Mutation check (falsifiability of the matrix itself)**: temporarily disabling the gateway's pre-admission revalidation gate (`TEMP-RED` mutation, restored byte-identical afterwards, sha256-verified) turned exactly the stale-control/visible-denial tests RED (3 pass / 4 fail), confirming the matrix detects a revalidation-bypassing gateway; restored run is 7 pass / 0 fail.
- **Test**: `src/deny-bypass.test.ts` — "negative control — a deliberately bypassing shim that skips the gateway is caught by the matrix and contained by the authority".
- **Result**: **PASS**.

## Gate Results

| Gate Command | Result | Notes |
| :--- | :--- | :--- |
| `just prereg-check gauntlet-tui-prime-workbench-T09` | PASS (exit 0) | Hash matches `0c07f855529d82d62667ef1969ab680f7b9c49b72e36e4c987d145c97560ca42`; prereg untouched |
| `just tui-test affordance-resolver` | PASS (exit 0) | 9 pass / 0 fail, 48 expect() calls |
| `just tui-test operation-isomorphism` | PASS (exit 0) | 6 pass / 0 fail, 54 expect() calls |
| `just tui-test deny-bypass` | PASS (exit 0) | 7 pass / 0 fail, 251 expect() calls |
| `just tui-check` | PASS (exit 0) | `tsc --noEmit --strict` clean — recovered from the interrupted builder's 6 type errors |
| `just tui-lint` | PASS (exit 0) | `biome check .`: 0 errors; 10 warnings (pre-existing bench-file `noConsole` baseline) and 193 infos (`useLiteralKeys`, the same pre-existing info class T08 recorded — required by the tsconfig's `noPropertyAccessFromIndexSignature`) |
| `just tui-preflight bootstrap` | PASS (exit 0) | Skeleton, pinned deps, bun.lock in sync, bun audit clean, spec/DESIGN digests match |
| `just check-package gauntlet-app` | PASS (exit 0) | Finished dev profile, no errors (plan-declared Rust gate; TS slice changes cannot affect it — verified, not assumed) |
| `just test-package gauntlet-app` | PASS (exit 0) | 662 tests run: 662 passed, 0 skipped |
| `cd workbench && bun test` | PASS (exit 0) | **87 pass / 0 fail** across 18 files (80 pre-existing/orphaned + 7 new), 1008 expect() calls |

No test was disabled, weakened, or skipped to reach green. No dependency was added. No frozen document, status anchor, justfile recipe, prereg, or Rust code was touched; changed/added files are exactly the six orphaned T09 sources (lint/narrowing completions only), `src/index.ts` (orphaned barrel additions, kept), the new `src/deny-bypass.test.ts`, and this evidence file.

## Scope of the deny-equivalence matrix

The matrix is executed over every projection route CURRENTLY implemented by the recovered work, honoring the prereg's declared blindspot honestly:

1. **gateway-http** — workbench `OperationGateway` → `HttpOperatorActionRoute` over the loopback HTTP surface transport (the production route shape: `GET /api/v1/state` reads, `POST /api/v1/units/{unit}/{resolve|retry}?run=` admission, bearer session + `X-Gauntlet-Operator` binding).
2. **gateway-shim** — workbench `OperationGateway` → the in-process adapter shim route (`CanonicalSurfaceStub.asInProcessRoute()`), the internal-adapter projection shape.
3. **direct-api** — the surface's action-endpoint shape called directly with no workbench code (`CanonicalSurfaceStub.perform`), the CLI/API-projection bypass path that REQ-SEC-002 must still contain.
4. **direct-cli** — the surface's CLI shape called directly (operator names the ticket; the ticket's run anchors the action), the terminal-projection bypass path.

Not yet implemented and therefore NOT claimed: kernel-wrapper routes (T10/T11), plugin-callback routes (T15), and eligible Prime-context routes (T10) — those tasks re-run this matrix as they land, per the prereg's known_blindspots. Within the implemented set, "every route" is exhausted: the gateway has exactly two transports (HTTP, in-process shim), and the direct surface has exactly two entry shapes (API, CLI), all four terminating in the ONE decision body.

## Known Limitations (recorded honestly)

1. **The matrix runs over a stub, not the Rust binary.** The canonical-surface-stub is an in-process stand-in whose decision body, refusal vocabulary, wire conventions, and read/admission separation mirror `durable_surface.rs`/`operator_action.rs`, but the matrix does not drive a real `gauntlet serve` process. The router-not-authority CLAIM is about the workbench's structure (verified here); that the real Rust surface refuses identically is T01's settled surface behavior, not re-proven here.
2. **Gateway pre-admission refusals mint but do not surface the idempotency key** (the refusal carries `idempotency_key: ""` for minted keys): nothing was admitted, so no key has durable meaning yet; a projection that re-invokes legitimately mints the same deterministic key. If a future task needs pre-admission refusals to be key-bearing (e.g., for audit), that is a `CandidateRevision`-scale change to the outcome schema, not a silent edit.
3. **A compromised transport can forge an accepted outcome to its own gateway** (negative control part 1): the gateway trusts its configured route's response shape; the forgery is exposed by the durable ledger and fresh-observation reconciliation — the instruments the matrix relies on — not by the gateway alone. Transport authentication is the surface's concern (T01's session authenticator); the workbench-side hardening of route identity is future work.
4. **Concurrency is bounded by the prereg's blindspot**: revocation racing invocation at the exact admission boundary is tested sequentially (invisible-to-reads variant), not under true parallelism; production ordering is the owning write boundary's (REQ-OPS-002).
5. **`state.refresh` is registered but not routed through the gateway's action path** (it is read-shaped, `consequential: false`); the refresh read path itself is exercised via the observation route, not as a gateway operation. If a later task routes refresh through the gateway, the matrix re-runs.
6. **Idempotency keys and the key ledger are in-process state** (one gateway instance per host): durable key ledgering belongs to the canonical write boundary (§16.3), and cross-process reconciliation is REQ-OPS-001/T12 territory, claimed nowhere here.

Confirmation: BUILDER evidence pending independent adversarial confirmation.

## Correction Round 1 (adversarial verification findings)

Fresh independent adversarial verification of round 0 returned **NOT_CONFIRM** with 3 BLOCKING findings, 1 MAJOR, and 3 MINORS; its executed probes (reproduced byte-for-byte from `/tmp/t09-probes/`) are the RED repros. Every fix below landed RED-first against the current code, in the existing design language, with no Rust changes, no frozen-document edits, and no commits. The round's own regression tests are C1–C6 in `src/deny-bypass.test.ts` and C4(resolver)/C7 in `src/affordance-resolver.test.ts`; all six deny-bypass correction tests failed RED before their fixes and passed after.

### BLOCKING 1 — the key ledger armed a gate skip for ANY ledgered key (operation-gateway.ts dispatch)

- **Finding (executed probes A1/a1-strict/A4)**: the ledger armed at bind time and `issuedHere` (mere prior sighting of the key) authorized skipping the fresh descriptor gate. A REFUSED invocation (typed `stale_affordance_denied`/`budget_exhausted`, zero admission attempts) armed the skip: the byte-identical retry executed and committed (`a1-strict`: second = accepted, settlements_committed 1). A deterministic recompute of the minted key (A4) worked identically.
- **Fix** (`src/operation-gateway.ts`): the bind-time ledger is now mismatch-detection memory ONLY (REQ-OPS-002 key/payload binding); it confers no re-drive privilege. Skip authorization comes exclusively from the key's OWN durable canonical record: an outcome still `outcome_unknown` (reconciliation), or an ACCEPTED settlement (§16.3 re-entry reporting durable truth). REFUSED and FAILED outcomes arm nothing — re-invoking them is a fresh gate pass, whoever supplies the key and however deterministically the key could be recomputed.
- **Disclosed engineering call**: the verdict's literal "only outcome_unknown may skip" would have broken the settled §16.3 cross-route re-entry contract that the same verdict required to stay green (`operation-isomorphism.test.ts` test 3 and the deny-bypass control leg re-drive a caller-supplied key across routes after an accepted settlement; with a mandatory fresh gate the re-drive is refused `no_open_ticket` on the settled unit). An accepted-record re-entry is safe by construction: same key ⇒ same payload digest (mismatch refused pre-routing) ⇒ the idempotent §16.3 re-entry can never commit twice. The skip set is therefore `{outcome_unknown, accepted}`; canonical refusals and failed outcomes never enter it.
- **New tests**: C1 (both scenarios: byte-identical retry of an unaffordable-declaration refusal AND of an authority refusal stays `stale_affordance_denied` pre-admission, ledger 0/0/0), C2(b)/(c) (ledgered-key and recomputed-key re-drives under dead reads refuse), C3 (concurrency pair, below).

### BLOCKING 2 — the HTTP transport's non-throwing "unreachable" observation let re-drives route past observability (HttpOperatorActionRoute.observe)

- **Finding (executed probes A2/A4/A6)**: `observe` returned a non-throwing unreachable observation on non-OK reads; fresh invocations failed closed through the descriptor gate, but ANY ledgered-key re-drive or `reconcile` skipped that gate and POSTed into a dead-reads surface — accepted + settlement committed while a fresh-key invocation on the same state was refused (REQ-SAFE-002 violated on the re-drive path).
- **Fix**: `observe` now THROWS on every non-OK response (transport-style, like the in-process shim), AND `dispatch` independently treats an observation reporting `reachable: false` as `revalidation_unavailable`. Result: EVERY drive — `invoke`, `invokeVia`, §16.3 re-drive, `reconcile` — fails closed with the typed refusal and zero admission contact whenever the surface cannot be observed, regardless of key origin or outcome record.
- **Key non-secrecy, disclosed**: pre-admission refusals no longer hide the minted key as `""` — the key is deterministic (`operation:target:actor:digest`) and the verifier recomputed it, so secrecy was never the defense; the observability+fresh gates are. The gateway header documents this. Post-fix A4's recompute attack dies at `revalidation_unavailable` before admission.
- **New tests**: C2 (a) reconcile of an unknown-outcome key under dead HTTP reads → refused `revalidation_unavailable`, zero new admission; (b) caller-supplied ledgered-key re-drive under dead reads → refused; (c) recomputed-key re-drive under dead reads → refused, with the surfaced-key assertion.
- **Existing-test adaptation (disclosed, not a weakening)**: the round-0 REQ-SAFE-002 test's HTTP leg asserted the flagged orphaned behavior (`stale_affordance_denied` via the non-throwing unreachable observation). Its assertions were updated to the corrected typed class (`revalidation_unavailable` / `surface_unreachable`, `stale: true`) — equally strict, now consistent with the shim leg; intent (fail closed, never a throw, zero admission contact) unchanged.

### BLOCKING 3 — concurrent same-key invocations raced one refusal + one execution (dispatch interleaving)

- **Finding (executed probe A5)**: `Promise.all` of two identical invocations produced `accepted/refused` — the first decision's prefix armed the ledger before the second's gate check.
- **Fix**: a per-host decision queue (`enqueueDecision`) serializes each drive's bind → observe → gate → ledger decision; a rejected job never poisons the tail. Post-fix: denied pair ⇒ both refused, ledger 0/0/0 (A5 now prints `refused/refused`); allowed pair ⇒ exactly ONE canonical execution, both accepted with a single `committed_now: true` and a §16.3 `committed_now: false`, same key.
- **New test**: C3 asserts exactly the verdict's tooth — both denials or one canonical execution with a single admission, never refusal+execution.

### MAJOR 1 — the stub invented an admission-level budget refusal (`budget_exhausted` is not in the canonical vocabulary)

- **Finding**: `budget_exhausted` appears in `crates/gauntlet-cli/src/operator_action.rs` `refusal_code()` (lines 500–528) NOWHERE, and the operator-action admission path has no budget gate at all (`budget_exhausted` exists only in a settle/mod.rs:109 comment about a dishonest loop terminal). The stub's invented gate made the "unaffordable" matrix leg prove equivalence against a fiction.
- **Fix** (`src/testing/canonical-surface-stub.ts`): the decision body now mirrors ONLY the real vocabulary, pinned in the file header with Rust line citations (`refusal_code()` operator_action.rs:500–528; `no_open_ticket` :232 / `ambiguous_ticket` :242; :246–249 anchored-action proceeds; surface auth/formation codes). `exhaustBudget` is documented as a READ-side knob only. Affordability is a workbench RESOLVER policy: the gateway refuses oversized-estimate resolutions pre-admission (typed `stale_affordance_denied` preserving `budget_exhausted`, ZERO perform calls — admission never sees a gateway-denied request); the canonical surface itself admits them, because it must — the workbench cannot fabricate canonical denials. The deny-equivalence matrices (Tooth 2a/2b) now cover exactly the canonical classes (authority, governance, stale-anchor); the unaffordable leg moved to C6 asserting the resolver/admission split explicitly.

### MINOR 1 — a bare 2xx was fabricated into an acceptance

- **Finding (probe B5)**: `HttpOperatorActionRoute.perform` defaulted `committed_now: true` and empty `run_id` on a 2xx missing the canonical body facts.
- **Fix**: a 2xx WITHOUT non-empty `run_id`, `unit_id`, and a boolean `committed_now` is a FAILED outcome (`malformed_acceptance`) — nothing is fabricated, fail-closed. C5 asserts the lying-2xx now resolves `failed`; the verifier's B5 probe post-fix fails for exactly that reason.

### MINOR 2 — read-shaped registered operations lacked a distinct typed reason

- **Finding**: `state.refresh` through the gateway was lumped into `unsupported_operation`, and the resolver resolved it toward `available`.
- **Fix**: the gateway refuses it with the typed `not_action_admitted` (registered but read-shaped; served by the canonical read path, never the action admission path); `resolveAffordance` resolves it `unavailable` with `not_action_admitted`, never toward `available`. C4 (gateway) and C4(resolver) pin both; RED confirmed (was `unsupported_operation` / `available`).

### MINOR 3 — `no_open_ticket` strictness vs the real admission semantics

- **Resolution (disclosed stricter policy)**: `crates/gauntlet-cli/src/operator_action.rs:246–249` lets an EXPLICITLY-ANCHORED action proceed with zero open tickets; `no_open_ticket` (:232) is the UNANCHORED shape's refusal. The workbench resolve control is the unanchored shape, so the resolver keeps the strict prediction — now an explicitly DISCLOSED policy with the Rust lines cited in `affordance.ts` and pinned by C7 in `affordance-resolver.test.ts` so the strictness cannot silently drift. Operators wanting the anchored set-with-nothing-to-mark use the CLI/API shape directly.

### Superseded round-0 sentences

- Evidence line 20 ("there is no code path from a rendered control to admission that skips it") was FALSE as of round 0 and is contradicted by the same document's line 34 (the re-drive skip description): any ledgered key — including refused-outcome and caller-supplied/recomputed keys — skipped the fresh gate. The corrected claim is: **no code path routes to admission without EITHER a passed fresh descriptor gate and an observed surface, OR the key's own durable canonical record (outcome_unknown reconciliation or accepted-settlement §16.3 re-entry) — and the surface must be observable on every drive, without exception.**
- Known Limitation 2's rationale ("nothing was admitted, so no key has durable meaning yet… a projection that re-invokes legitimately mints the same deterministic key") is superseded: pre-admission refusals now surface the minted key, and the honest rationale is that the key was never secret — the verifier recomputed it — so the gate, not key obscurity, is the defense (round-0's hidden-key framing implied otherwise).

### Post-fix verifier-probe outcomes (closure evidence)

| Probe | Round-0 | Post-fix | Meaning |
| :--- | :--- | :--- | :--- |
| a1-strict (byte-identical refused-then-retry) | executed | **FAILS** (retry refused, ledger 0/0/0) | BLOCKING 1 closed |
| A2 (re-drive under dead HTTP reads) | executed | **FAILS** (refused `revalidation_unavailable`) | BLOCKING 2 closed |
| A4 (recomputed-key re-drive under dead reads) | executed | **FAILS** (refused; key surfaced) | BLOCKING 1+2 closed |
| A6 (reconcile under dead reads) | executed | **FAILS** (refused `revalidation_unavailable`) | BLOCKING 2 closed |
| A5 (concurrent pair) | accepted/refused | **refused/refused**, ledger 0/0/0 | BLOCKING 3 closed |
| A1 (retry DROPS the oversized estimate) | executed (skip) | executes via a FULL FRESH GATE PASS on a healthy world — the verdict's specified "fresh gate pass" for a caller-supplied key with no durable record; the skip is gone (C1/C2 pin the distinction: same-estimate retry and dead-reads retry refuse) | behavior now verdict-conformant |
| A3 (re-drive after authority refusal) | admission-level refusal via skip | refused PRE-admission (`stale_affordance_denied`, attempts 0) — probe's old-skip assertion superseded; defense strictly stronger | BLOCKING 1 closed |
| B5 (lying 2xx) | accepted (fabricated) | **FAILS** (resolves `failed`/`malformed_acceptance`) | MINOR 1 closed |
| B1–B4, B6 (forgery instruments, mismatch, unknown/reconcile guards, descriptor integrity) | pass | **still pass** | no regression |
| Falsifiers 1/1b/2 | pass | **still pass (3/3)** | preregistered teeth intact |

### Correction Round 1 Gate Results

| Gate Command | Result | Notes |
| :--- | :--- | :--- |
| `just tui-test affordance-resolver` | PASS (exit 0) | 11 pass / 0 fail (9 round-0 + C4-resolver + C7), 54 expect() calls |
| `just tui-test operation-isomorphism` | PASS (exit 0) | 6 pass / 0 fail — §16.3 cross-route re-entry intact under the corrected skip rule, 54 expect() calls |
| `just tui-test deny-bypass` | PASS (exit 0) | 13 pass / 0 fail (7 round-0 + C1–C6), 270 expect() calls |
| `just tui-check` | PASS (exit 0) | tsc --noEmit --strict clean |
| `just tui-lint` | PASS (exit 0) | 0 errors; 10 warnings (pre-existing bench noConsole baseline), 192 infos (pre-existing useLiteralKeys class) |
| `just tui-preflight bootstrap` | PASS (exit 0) | unchanged |
| `cd workbench && bun test` | PASS (exit 0) | **95 pass / 0 fail** across 18 files (all 87 round-0 tests green + 8 correction tests), 1033 expect() calls |
| `just prereg-check gauntlet-tui-prime-workbench-T09` | PASS (exit 0) | prereg still byte-identical (`0c07f855…0ca42`) |
| `just check-package gauntlet-app` | PASS (exit 0) | no errors |
| `just test-package gauntlet-app` | PASS (exit 0) | 662 tests run: 662 passed, 0 skipped |

RED-first record: C1–C6 each failed against the pre-fix code with the exact defect the verdict executed (verified before any fix landed: initial run 7 pass / 6 fail with every original test green), then passed after the fixes; the verifier's own probes in `/tmp/t09-probes/` were run pre-fix (all BLOCKING repros executed) and post-fix (closure table above). No test was disabled, weakened, or skipped; the single existing-test adaptation (REQ-SAFE-002 HTTP leg) is disclosed above. No dependency, frozen document, status anchor, justfile recipe, or Rust code was touched.

Confirmation: BUILDER evidence (Correction Round 1) pending independent adversarial confirmation — all three BLOCKING findings, the MAJOR, and all three MINORS are closed with RED-first regression tests; settlement authority remains with the orchestrator's evidence review.

## Correction Round 2 (adversarial verification, failed-arm finding)

Adversarial round 2 verified round-1's BLOCKING 2/3 and the MAJOR closed by its own probes, but returned **NOT_CONFIRM** on one new BLOCKING: the round-1 fix was INCOMPLETE.

### The finding

`OperationGateway.dispatch` armed `admittedKeys` for `accepted` **or `failed`** outcomes (`operation-gateway.ts`: `if (outcome.kind === "accepted" || outcome.kind === "failed")`). That contradicted this evidence's own round-1 sentence ("REFUSED and FAILED outcomes arm nothing") and the field's comment.

**Root cause (one line):** a failed drive committed nothing, so the accepted-arm safety argument (idempotent §16.3 re-entry can never commit twice) does not transfer — an armed retry after a failed outcome is a FRESH admission that CAN commit, and it reached that admission with the fresh descriptor gate skipped.

**Executed repro (verifier probes `/tmp/t09-r2-probes/attack.test.ts` F1a/F1b/F1c, all RED against the round-1 code):** drive 1 gets a typed `failed` (a shim returning `{kind:"failed"}` or a plain HTTP 503 on perform); the world then becomes unaffordable (budget exhausted, visible) or authority-revoked (visible); drive 2 with the same (deterministically identical) key skipped the fresh gate — F1a: an unaffordable operation ACCEPTED with a settlement committed, through the gateway.

### The fix (surgical)

`failed` arms NOTHING. `admittedKeys` records ACCEPTED settlements only; skip authorization is exactly `{outcome_unknown, accepted}` — matching the evidence sentence and the code comment, which now state the same thing (the accepted-arm comment explains why the idempotence argument does not extend to failed).

### New teeth (RED-first)

C8a/C8b/C8c in `src/deny-bypass.test.ts` (verifier F1a/F1b/F1c shapes, in-suite):

- **C8a** — failed drive (affordable estimate) → budget exhausted visibly → same-key retry with a now-oversized estimate (estimates are not part of the payload digest, so the key is identical) → refused `stale_affordance_denied` preserving `budget_exhausted`, `stale: true`, admission_attempts 0, settlements 0.
- **C8b** — failed drive → authority revoked visibly → same-key retry → refused `stale_affordance_denied` preserving `authority_not_permitted` at the fresh gate, admission never contacted.
- **C8c** — failed via a REAL HTTP 503 on perform (production transport shape) → unaffordable world → same-key retry refused pre-admission; the server's POST count stays at exactly the one 503.

**RED-first proof:** with the round-1 code and all 13 existing deny-bypass tests green, the three new teeth failed (13 pass / 3 fail, every failure on the gate-skip assertions); after the one-line fix, 16 pass / 0 fail. The verifier's full round-2 probe battery passes post-fix: 23 pass / 0 fail (F1a/F1b/F1c closed; F2–F12 and N1–N6 — round-1 closures and the new attack families — all still green).

### Trigger-records from the round-2 verdict (recorded, not fixed in this round)

1. **Accepted-record re-drive refusal-shape difference, anchored vs unanchored.** Scope: a same-key re-drive after an accepted settlement is descriptor-gate-skipped by design (§16.3 durable-truth reporting); the denial it can meet is admission-level, and its shape differs between the anchored (proceeds with zero open tickets, operator_action.rs:246–249) and unanchored (`no_open_ticket`, :232) entry shapes. No bypass — F5/F6/F7 pin mismatch refusal, no-new-commit re-entry, and fresh-gate for never-issued keys. Re-checked when the matrix re-runs in **T10/T15**.
2. **Reconcile unanchored committed-truth audit-UX residual.** Scope: reconciliation of an unanchored resolve reports the durable committed truth with (possibly) empty ticket bookkeeping — canonical per §16.3, but the projection's audit presentation of "committed with nothing to mark" is an operator-facing UX question, not a gateway-semantics one. Deferred to the projection task that renders operation states (**T10/T15** matrix re-run covers the semantics; the UX slice owns presentation).
3. **N1b — observe-never-settles head-of-line blocking is a bounded DoS, not a bypass.** Scope: the per-host decision queue (Correction Round 1 BLOCKING 3) means a route whose observe NEVER settles stalls later drives on that gateway instance — containment, not containment-failure (no drive routes past an unobserved surface; the stall is the fail-closed behavior). A host-level drive timeout/cancellation budget is the later hardening; the bounded-search discipline (AGENTS.md) forbids calling this exhaustion-proof within the declared scope.
4. **Non-crypto 64-bit payload digest.** Scope: the FNV-1a doubled digest is a BINDING/MISMATCH instrument for key/payload identity, not a cryptographic commitment; N3's bounded birthday search found collisions in principle. The mismatch detector is defense-in-depth behind the gates (admission re-validates the command regardless), so no security claim rests on collision resistance. If a later task ever elevates the digest into a security-relevant commitment, it must be replaced with a cryptographic hash — noted for **T18** (workbench-side operation hardening slice per the plan's dependency set).

### Correction Round 2 Gate Results

| Gate Command | Result | Notes |
| :--- | :--- | :--- |
| `just tui-test affordance-resolver` | PASS (exit 0) | 11 pass / 0 fail, 54 expect() calls |
| `just tui-test operation-isomorphism` | PASS (exit 0) | 6 pass / 0 fail, 54 expect() calls |
| `just tui-test deny-bypass` | PASS (exit 0) | 16 pass / 0 fail (13 round-1 + C8a/C8b/C8c), 293 expect() calls |
| `just tui-check` | PASS (exit 0) | tsc --noEmit --strict clean |
| `just tui-lint` | PASS (exit 0) | 0 errors; 10 warnings / 192 infos — both the pre-existing baseline classes |
| `just tui-preflight bootstrap` | PASS (exit 0) | unchanged |
| `cd workbench && bun test` | PASS (exit 0) | **98 pass / 0 fail** across 18 files (95 round-1 + 3 correction teeth), 1056 expect() calls |
| `just prereg-check gauntlet-tui-prime-workbench-T09` | PASS (exit 0) | prereg still byte-identical (`0c07f855…0ca42`) |

Verifier round-2 probes post-fix: **23 pass / 0 fail** (`/tmp/t09-r2-probes/attack.test.ts`), including the three executed F1 repros. No test was disabled, weakened, or skipped; no dependency, frozen document, status anchor, justfile recipe, or Rust code was touched; the only production change is the removal of the `failed` arm (plus its comment alignment) in `src/operation-gateway.ts`.

Confirmation: BUILDER evidence (Correction Round 2) pending independent adversarial confirmation — the failed-arm BLOCKING is closed with RED-first teeth matching the verifier's executed repros; the four trigger-records are logged with scope and re-check tasks; settlement authority remains with the orchestrator's evidence review.
