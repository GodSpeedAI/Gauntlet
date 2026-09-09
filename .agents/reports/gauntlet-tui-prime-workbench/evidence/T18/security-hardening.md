# T18 — Security, trust, secret, and privileged-input hardening (builder evidence)

Task: gauntlet-tui-prime-workbench-T18 · P3 · independent_adversarial
Owned files: `workbench/src/security-hardening.ts`, `workbench/src/security-boundaries.test.ts`, `workbench/src/secret-leakage.test.ts`, and this report.
No other task's file was edited; no git commits; prereg untouched (`just prereg-check gauntlet-tui-prime-workbench-T18` PASS, frozen hash `4bd5c124a3fef1b113375eb79da32b9257cbd2394ea45643d6e4116ccd0583d3`).

## DECLARED ENFORCEMENT SCOPE

Stated first, as the plan requires and as `ENFORCEMENT_SCOPE` (typed data in `security-hardening.ts`) carries in code:

- **Trusted same-user arbitrary code is NOT sandboxed by a TUI.** This workbench hardens authority routing and input binding for hostile/untrusted INPUT crossing privileged boundaries of a same-user tool. Kernel/worker/plugin process containment organizes failure, NOT security enforcement (REQ-SEC-001); hostile execution requires an external enforcement boundary appropriate to the risk.
- **Liveness is provenance, never success** (REQ-SAFE-004): heartbeats, process aliveness, handle status, and mailbox volume are recorded signals; completion/settlement derives only from canonical evidence.
- **Fallback is visible or absent** (REQ-SAFE-003): there is no third attestation.
- **Excluded threat classes — recorded, never implied covered:**
  1. trusted same-user arbitrary code (above);
  2. remote network authentication and transport qualification — **T20**;
  3. host OS privilege escalation, kernel-level containment, memory-safety attacks;
  4. cryptographic commitment strength of the gateway's non-crypto 64-bit payload digest (a binding instrument; canonical admission re-validates every command regardless);
  5. secrets leaked by UPSTREAM authoritative stores inside their own event payloads (the scanner detects and redacts views; upstream hygiene is the source's).
- **Authority:** the ONE canonical Gauntlet admission path decides every consequential operation. Nothing in this task substitutes for it; this task adds instruments and tests, never a second authority.

## Privileged boundary inventory (as data, in `security-hardening.ts`)

`PRIVILEGED_BOUNDARY_INVENTORY` — 21 entries over 8 privileged side-effect classes. Each entry names its owning module, task, NAMED enforcement mechanism, exported enforcement symbol, and typed denial shape. `assertInventoryAttackCoverage` makes the enumeration load-bearing: any inventory boundary without an executed attack record in `security-boundaries.test.ts` fails the suite (a new boundary cannot silently escape).

| Boundary id | Class (area) | Module (task) | Enforcement | Attack route | Denial before side effect |
| --- | --- | --- | --- | --- | --- |
| `gauntlet.action.gateway.http` | gauntlet_command | operation-gateway.ts (T09) | gateway_prevalidation | gateway over `HttpOperatorActionRoute` | `stale_affordance_denied` / `revalidation_unavailable`, admission_attempts 0 |
| `gauntlet.action.gateway.shim` | gauntlet_command | operation-gateway.ts (T09) | canonical_admission | gateway over in-process shim | same, admission_attempts 0 |
| `gauntlet.action.direct.api` | gauntlet_command | canonical-surface-stub.ts (T09 mirror) | canonical_admission | `stub.perform` | admission-level refusal, zero settlements |
| `gauntlet.action.direct.cli` | gauntlet_command | canonical-surface-stub.ts (T09 mirror) | canonical_admission | `stub.performViaCli` | admission-level refusal, zero settlements |
| `prime.delegation.spawn` | prime_execution | prime-recursion.ts (T10) | canonical_dispatch_before_side_effect | denied `delegate()` | `refused_by_gateway`, adapter sessions 0 |
| `prime.mailbox.completion` | prime_execution | prime-host.ts (T04) | no_status_transition_from_communication | "done" mailbox text | handle stays `running`; completion only via schema-valid contract |
| `prime.steer.message` | prime_execution | prime-long-running.ts (T10) | no_status_transition_from_communication | steer delivery | `establishes_completion` literal false; completion unchanged |
| `kernel.action.callable` | kernel_operation | kernel-affordances.ts (T11) | gateway_prevalidation | stale-set callable invoke | gateway refusal; wrapper invalidated, gate untouched |
| `kernel.read.helper` | kernel_operation | kernel-affordances.ts (T11) | read_only_projection | read helper (no `invoke` exists) | `read_unavailable` fail-closed; `affects_canonical_state: false` |
| `kernel.input.broker` | kernel_operation | kernel-affordances.ts (T11) | attention-mediated only | request with no sink | typed `unroutable`; answers return only via REPL mediation; replay refused |
| `kernel.object.promotion` | kernel_operation | evidence-promotion.ts (T11) | canonical_decision_gate | offer live value / locator-less artifactize | `not_artifactized` / `missing_artifact_ref` |
| `terminal.spawn.request` | terminal_process | terminal-session.ts (T13) | typed_boundary_validator | traversal/metachar/allowlist bypass | `TerminalBoundaryError`, provider spawnCount 0 |
| `terminal.automation.title` | terminal_process | terminal-session.ts (T13) | control_sequence_sanitizer | OSC 52 + OSC 8 in title | inert text, neutralized counts recorded |
| `terminal.rpc.payload` | terminal_process | terminal-session.ts (T13) | typed_boundary_validator | allowlist escape + OSC payload | typed refusal / inert bound strings |
| `plugin.load.trust` | plugin_loading | plugins.ts (T15) | trust_policy | undeclared/untrusted/prototype-inherited trust | `PluginTrustRefusalError` / manifest error; module never activates |
| `plugin.capability.boundary` | plugin_loading | plugins.ts (T15) | capability_allowlist + serialized boundary | undeclared capability; function-carrying payload; malformed response | `capability_not_declared` / nothing unvalidated admitted |
| `plugin.operation.registry.gate` | plugin_loading | plugins.ts (T15) | registry_gate | non-canonical plugin operation | `plugin_operation_not_canonical` |
| `projection.store.sql` | projection_store | projection-store.ts (T06) | parameterized_sql | SQL-metacharacter payloads | content binds as DATA; schema intact on independent connection |
| `transport.client.rpc` | transport_rpc | client.ts (T00A) + surface | surface_authentication | right transport, wrong/missing credentials | 401/403 surface refusal; zero settlements; no credential echo |
| `evidence.admission.gate` | evidence_admission | evidence-promotion.ts (T11) | canonical_decision_gate | forged/mismatched decision | `decision_mismatch` / `refused_canonically`; ledger empty |
| `evidence.frame.fabrication` | evidence_admission | evidence-promotion.ts (T11) | canonical_decision_gate | frame from refused decision | typed throw — no frame without an admitted decision |

## What was implemented (owned files only)

1. **`security-hardening.ts`** — the hardening instruments layer; owns NO authority:
   - `ENFORCEMENT_SCOPE` (typed scope declaration, mirrored above);
   - `PRIVILEGED_BOUNDARY_INVENTORY` + `assertInventoryAttackCoverage` + `inventoryByArea`;
   - the secret-redaction scanner: `detectSecretPatterns` (5 calibrated pattern classes), `SecretRedactor` (registered secrets → STABLE reference markers `[REDACTED:<kind>:<ref>]`, marker refs derived from caller labels or a counter — never from secret content; `redactText`/`redactDeep`/`leaksAny`/`markers`), and `SECRET_LOOKALIKE_NEGATIVE_CONTROLS` (the 15-line corpus the detector must NOT mangle);
   - `FALLBACK_VISIBILITY_LEDGER` (7 attested paths) + `assertNoSilentFallback` — every provider/model/transport fallback is `policy_permitted_visible` (with a named visibility signal) or `absent`;
   - `withObserveDeadline` — composition-level closure of the T09 head-of-line trigger-record (typed `ObserveDeadlineExceededError` → the gateway's typed fail-closed refusal), without editing the gateway;
   - `auditIsolationClaims` + `REQUIRED_ISOLATION_DISCLOSURES` — the absence-of-claims instrument (every "sandbox" mention must carry same-line negation; seven pinned disclosures must exist).
2. **`security-boundaries.test.ts`** — Attack 1 over the enumerated matrix (below), the isolation/liveness absence-of-claims legs, and the three carried trigger-record re-attacks. 25 tests, 208 expect() calls.
3. **`secret-leakage.test.ts`** — Attack 2 over every forbidden channel (below), the scanner's negative control, and the fallback-ledger attestation. 14 tests, 137 expect() calls.

## Attack 1 — denied operation through every supported projection + direct helper paths

Scenario: one blocked unit, one open escalation, authority revoked (visible to reads for workbench-mediated routes; an additional racing-revocation variant lives in T09's matrix and is re-proved here at the admission level via the trigger-record leg). Results — every route denied BEFORE consequential side effect:

- **gateway-http** → refused `stale_affordance_denied`, reason `authority_not_permitted`, admission_attempts 0, settlements 0.
- **gateway-shim** → identical typed refusal, admission_attempts 0.
- **direct-api / direct-cli** → refused AT admission with the identical canonical class (`authority_not_permitted`), zero settlements/tickets.
- **kernel callable** (stale-capable wrapper, currency "current") → same gateway refusal; the SET is invalidated (projection bookkeeping) and the gate is untouched; admission_attempts 0.
- **plugin affordance-projection route** → `projectionDescriptor` only PREDICTS (`unavailable` + `authority_not_permitted`); the host gateway denies the invocation; admission_attempts 0. A plugin-DEFINED operation (`plugin.launch.missile`) is refused at registration (`plugin_operation_not_canonical`) — plugin-defined operations do not exist.
- **Prime delegation** → `refused_by_gateway` with the same canonical reason; the adapter has ZERO sessions, the ledger ZERO records — no child is ever spawned behind a denial.

Uniform semantics: every workbench-mediated route converges on one canonical operation identity and preserves the canonical refusal class in machine-readable reason codes; the direct surface routes earn the identical canonical denial. The inventory-coverage contract then asserts all 21 boundary ids were executed (verified RED-capable: the assertion's error lists any unattacked id — demonstrated during development when the two mailbox legs had not yet recorded their ids).

**Isolation/liveness absence-of-claims** (REQ-SEC-001, REQ-SAFE-004):
- Source audit over 15 modules: zero unhedged "sandbox" mentions; disclosures pinned exactly as follows — SIX module disclosures in `REQUIRED_ISOLATION_DISCLOSURES` (`prime-kernel` liveness-cannot-settle; `prime-host` mailbox-NOT-a-contract; `prime-long-running` liveness-never-implies-success; `prime-recursion` policy-not-Prime-capabilities; `plugins` trust-never-inferred; `terminal-session` env-values-never-recorded) PLUS the seventh honesty anchor, the `ENFORCEMENT_SCOPE` statement in `security-hardening.ts`, pinned verbatim by the suite's dedicated scope assertion (see Correction Round 1 for this count being made exact).
- Runtime honesty: `delegationCapabilityFacts`/`substrateInventory` record every native control (`native_nested_delegation_depth_limit`, `native_child_budget_accounting`, all 7 REQ-PRIME-008 substrates) `supported: false` with reason `not_exposed_by_installed_prime_adapter_model`; `requestSubstrateStrategy` returns `unsupported_substrate` (never simulated); `canonicalStopAvailability().registered === false`.
- Liveness is not success, behaviorally: a live kernel refuses `canSettleUnit`; "done complete finished" mailbox text leaves the handle `running` with `completionStatus.complete === false, basis: "none"`; a steer delivery changes nothing; the host exposes NO stop/cancel/kill/terminate method, `detach` issues `cancel_calls_issued: 0` (literal type), and the adapter call log shows no lifecycle call after detach/close — the child keeps living.

## Attack 2 — seeded secrets vs every forbidden channel

Seeds (synthetic, well-marked, approved sources ONLY): env `GAUNTLET_SESSION_TOKEN=t18-FAKE-ENV-TOKEN-7f3a9c2b4d`, CLI `session_token=t18-FAKE-CLI-TOKEN-9c2e7a1f58`. Marker stability verified: same secret → same `[REDACTED:registered_secret:env-token|cli-token]` marker across scans; markers carry no secret content.

| Forbidden channel | Result |
| --- | --- |
| Config file source | a token seeded in a FILE propagates NOWHERE (`session_token: null` per REQ-CONFIG-002); env/cli resolve in memory only; `redactConfig` view is `[REDACTED]`; redacted deep view clean |
| Error messages | a transport whose observe error text embeds the env token produces the gateway's typed `revalidation_unavailable` — the RAW message carries the transport's text (DISCLOSED BOUND, see below); the redacted view a projection must store is clean and carries the stable marker; the stub's side-effect ledger holds no echo |
| Snapshots | seeded secrets appear nowhere in `WorkbenchModel` snapshots; an upstream-leaked `sk-…` token riding a payload IS the source's leak (disclosed) — the detector FLAGS it in the snapshot text and the redacted view is clean |
| Evidence frames | the authoritative frame and the admission ledger contain no seeded secret (`redactor.leaksAny === false`) |
| Plugin manifests | an extra own property `session_token` on a manifest document is DROPPED by parsing (own-fields only) — the smuggled secret never survives admission |
| Plugin state / logs | private state values never appear in the fingerprint or record views (KEYS only); an activation crash whose cause text embeds the secret shows the raw-text disclosed bound; the redacted view is clean and the post-unwind manager ledger holds nothing |
| SQLite writes | full sweep of EVERY row of EVERY table over an INDEPENDENT connection to a real temp-file store: no approved-source secret in any row; the upstream-carried leak is DETECTED by the scanner (audit instrument works on real rows) |
| Terminal | spawn env records the NAME only (`envKeys: ["GAUNTLET_SESSION_TOKEN"]`); written input BYTES are never recorded (counts only); projection/scrollback/provenance/events clean of both seeds; OSC 52 in an automation title is neutralized and the render-safe view redacts the secret |
| Stream frames | client surfaces (handshake, state log, degraded reasons, rejections, refusals) clean under approved-source seeding; an upstream frame leak is detected and the redacted live-snapshot view is render-safe |
| Read client | auth refusals from a real HTTP surface never echo the credential (`text(refusal)` clean); zero settlements |

**Negative control:** the 15-line look-alike corpus (`password_policy=strict`, `token_bucket_fill_rate=1000/s`, `session_token_ttl=3600`, `Bearer <token>`, plain loopback URLs, run ids, digests, fingerprints, `password hashing uses argon2id`) produces ZERO detections and byte-identical redaction output — the scanner does not mangle non-secrets.

## Carried trigger-records re-attacked (T09 round-2 evidence)

1. **Compromised-transport forged acceptance — tested, exposure's bound recorded honestly (owning task T09).** A route forges `accepted {run_id: "forged-run", committed_now: true}`. Instruments: (a) the durable ledger — admission_attempts 0, settlements 0, tickets 0; (b) a FRESH observation — the escalation is still open, so the forgery is exposed against canonical truth. THE BOUND, demonstrated end-to-end: the forged acceptance ARMS the gateway's re-entry record for that key, so a later same-key drive SKIPS the workbench fresh-descriptor gate even after the world turns visibly denied — it reaches the transport — where an honest canonical admission STILL refuses (`authority_not_permitted`, `stale: false`, zero side effects). The exposure is therefore bounded to (a) misreporting outcomes to the projection and (b) skipping the WORKBENCH-side gate on one gateway instance; canonical authority is never bypassed. Closing (b) requires the gateway to require transport-identity binding on the re-entry record — **not possible without editing operation-gateway.ts; recorded for a T09 correction round.**
2. **Non-crypto 64-bit payload digest — bound verified and pinned (owning task T09).** Determinism: two gateways mint the identical key; an INDEPENDENT reimplementation of the public doubled-FNV digest reproduces the real minted key exactly. Mismatch detection: key reuse with a changed payload is refused `idempotency_key_payload_mismatch` before any routing (zero admissions). Disclosure pinned in source ("A binding/mismatch digest, not a cryptographic instrument"). A bounded 8192-payload collision search with the validated model found no collision — a BOUNDED absence, never a claim of collision resistance; per T09's record, collisions exist in principle, and no security claim rests on the digest because canonical admission re-validates every command regardless (demonstrated by the trigger-record 1 leg, where the gate-skipping drive is still denied by admission).
3. **Observe head-of-line bounded stall — demonstrated, then CLOSED at composition level (this task's module).** Raw bound (demonstrated, not fixed in the gateway): with a never-settling observe, the queued drive is stalled so deeply it never even reaches its own observe call (30 ms observation: settled=false, exactly 1 observe call, 0 performs) — fail-closed containment, the DoS shape T09 recorded. Closure: `withObserveDeadline(route, 15)` wraps the route; the stalled drive becomes the gateway's typed `revalidation_unavailable`/`surface_unreachable` refusal and the NEXT drive proceeds against the now-healthy surface and is accepted. Hosts should construct gateways over the wrapped route; a host-level default deadline remains a T09/T20 policy decision.

## Fallback visibility (REQ-SAFE-003)

`FALLBACK_VISIBILITY_LEDGER`: 4 paths `policy_permitted_visible` (documented default endpoint; transport auto-classification; Prime-absent → Gauntlet-only mode with warning + blocked capabilities; Gauntlet-unreachable → disconnected status + fail-closed capabilities) and 3 `absent` (model/provider fallback — no such config field exists; alternate-endpoint retry — the client holds ONE baseUrl and exposes no retry/fallback method; automatic route switching — `invokeVia` is an explicit caller action). `assertNoSilentFallback` fails the ledger if a permitted path ever loses its visibility signal. Surface checks prove the absence claims true: `DEFAULT_CONFIG` has no model/provider key; `GauntletReadClient` exposes no retry/fallback/alternate method; `resolveConfig` is synchronous (it cannot have probed or substituted) and returns an unreachable endpoint verbatim rather than swapping it.

## Defects / findings in OTHER tasks' modules (for correction rounds; NOT edited by this builder)

1. **[T09 — MINOR/security-relevant] Gateway re-entry record is armed by any `accepted` transport response without transport-identity binding.** `operation-gateway.ts` `dispatch()` adds the key to `admittedKeys` on any accepted-shaped response. A compromised transport can therefore arm the §16.3 gate-skip for a key it never truthfully settled (repro: `security-boundaries.test.ts`, "trigger-record 1"). Canonical admission contains it (verified); the durable ledger and fresh observation expose it (verified). Suggested root fix (T09): bind the re-entry record to the route identity that earned it, or require a fresh-gate pass for any drive whose armed record was written by a different route instance. Until then the bound above is the honest limit of the workbench gate.
2. **[T09 — recorded, unchanged] Raw transport error text passes verbatim into `revalidation_unavailable` / `outcome_unknown` messages** (`describeError(error)`). Transports are same-user trusted code (inside the declared scope), so this is a presentation-hygiene requirement, not a boundary breach: projections and logs rendering these messages MUST pass them through the secret-redaction instrument (proved clean in `secret-leakage.test.ts`). If the orchestrator prefers defense-in-depth at the source, a T09 correction could redact at message construction; this builder did not edit the gateway.
3. **[T06/T16 — informational, disclosed] Upstream-carried secrets in event payloads persist in projection rows and stream-driven snapshots.** The workbench cannot refuse authoritative upstream data without forking canonical truth (REQ-SAFE-005 direction); the scanner is the detection instrument and the redacted view is the render-safe form. Recorded as an excluded/upstream-hygiene threat class, with the detector legs proving auditability.
4. **[T05/T11 — informational] `PrimeReplKernel.execute` simulates Python execution (T05's adapter model); `canSettleUnit`'s artifactized check reads that model.** No security claim rests on the simulation; the kernel/REPL is disclosed as NOT a sandbox wherever it is represented (verified by the claim audit). No action required.

## What the independent adversarial verifier should attack hardest

1. **The forged-acceptance arming path (finding 1)** — try to convert the workbench-gate skip into a consequential commit by combining a forged acceptance with an admission race (the C3 concurrency shape) or `invokeVia` route alternation. The suite's containment claim rests on canonical admission re-validating; breaking THAT would be a real bypass.
2. **The redaction scanner's calibration** — craft strings that are secrets my patterns miss (e.g. custom header schemes, base64-wrapped secrets without key hints, unicode separators) or look-alikes my patterns wrongly mangle beyond the exported negative-control corpus; the corpus is the declared contract, anything outside it is fair game.
3. **The inventory coverage contract** — add a privileged path in a scratch copy WITHOUT a test record and confirm the suite fails; or argue a privileged side-effect class the inventory omits entirely (candidate: shell/composer state, evidence-promotion `attachToFocus`, `publishTerminalProvider` snapshot field).
4. **The head-of-line closure** — try routes that settle observe but never settle perform (the wrapper deliberately covers observe only; perform stalls await their own caller, matching T09's per-call await semantics — same shape as T15's `callCapability` record).
5. **Fallback attestation completeness** — find a fallback path in any landed module that is neither in the ledger's permitted-visible nor absent set (silent fallback = redesign trigger R1).

## Gate results (verbatim, final capture)

| Gate command | Result | Notes |
| :--- | :--- | :--- |
| `just tui-test security-boundaries` | PASS (exit 0) | 25 pass / 0 fail, 208 expect() calls |
| `just tui-test secret-leakage` | PASS (exit 0) | 14 pass / 0 fail, 137 expect() calls |
| `just tui-test deny-bypass` | PASS (exit 0) | 16 pass / 0 fail, 293 expect() calls (T09 matrix unchanged and green) |
| `just tui-check` | PASS (exit 0) | tsc --noEmit --strict clean across the whole tree |
| `just tui-lint` | PASS (exit 0) | 0 errors; 34 warnings / 465 infos — the pre-existing baseline classes (`noConsole` bench class, `useLiteralKeys` infos); T18's three files lint-clean |
| `just tui-preflight bootstrap` | PASS (exit 0) | unchanged (pins, lockfile, audit, spec/design hashes) |
| `cd workbench && bun test` | PASS (exit 0) | 449 pass / 0 fail / 2 gated skips across 43 files, 5382 expect() calls — includes T18's 39 tests (25 + 14) and the parallel wave's landed suites (the 330-pass baseline plus T14B/T17/T18 additions) |
| `just prereg-check gauntlet-tui-prime-workbench-T18` | PASS (exit 0) | prereg still byte-identical (`4bd5c124…0583d3`) |

During the capture window the tree-wide gates (`tui-check`, `tui-lint`, full `bun test`) transiently failed ONLY in the parallel builders' in-flight files (`sxr-adapter.ts`, `worker-folds*.ts`, `focus-latency.bench.ts`, `projection-worker.bench.ts`, `semantic-evidence-composition.test.ts` — each failure identified per-file at the moment it was observed, with retries per the dispatch note). T18's three files were lint- and type-clean at every check, and all eight gates above were captured green in a single final pass after the wave settled.

No test was disabled, weakened, or skipped; no golden artifact was updated to match changed output; no frozen document, status anchor, other task's file, dependency, or justfile recipe was touched.

Confirmation: BUILDER evidence pending independent adversarial confirmation.

## Correction Round 1 (adversarial verification findings)

The independent adversarial verifier CONFIRMED the preregistered claims (P1/P2) and the arming-bound adjudication, and issued two MAJORs. MAJOR-2 (inventory precision vs `evidence.frame.fabrication`) is this builder's and is closed below; MAJOR-1 belongs to another owner and is not recorded as fixed here. No frozen document, status anchor, other task's file, dependency, or justfile recipe was touched; `workbench-model.ts` and `operation-gateway.ts` were NOT edited.

### MAJOR-2 — CLOSED: the inventory now names the REAL enforcement locus for evidence-row fabrication

**The finding (adjudicated accurate):** the round-0 inventory's `evidence.frame.fabrication` entry presented the frame-construction helper's canonical-decision gate as the locus, while the store write path is ungated: the verifier demonstrated `WorkbenchModel.applyEvent({event_type: "evidence_added", source_id: "not-a-canonical-source", payload: {verified: true, ...}})` writes a `verified: true` row with NO canonical decision.

**Fix (owned files only):**
1. `security-hardening.ts`: `PrivilegedBoundary` gains an optional `disclosed_bound` field ("honest enforcement-limit disclosure... the true state must stay visible in the inventory itself, never smoothed over"). The `evidence.frame.fabrication` entry is rewritten honestly: module now names `evidence-promotion.ts + projection-store.ts` (owning tasks `T11 + T06`); `attack_route` names BOTH routes (the frame helper with a refused decision AND the direct `applyEvent` forged-frame route); `denial_shape` states the split plainly — frame helper: typed throw; STORE applyEvent path: **UNGATED**; and the new `disclosed_bound` names the three mitigations with their in-suite demonstrations.
2. `security-boundaries.test.ts`: a new attack drives the direct store write route end-to-end and pins the TRUE state — the forged frame lands (`applied: true`), the snapshot carries the `verified: true` row attributed to `not-a-canonical-source`, and NO admission exists anywhere (`EvidenceAdmissionGate.admittedRecords()` empty). It then demonstrates each mitigation in force: (a) the REBUILDABLE layer — a fresh generation drops the forged row; (b) nothing consequential reads the projection as authority — with the forged row live, a denied operation through the gateway is STILL refused (`authority_not_permitted`, zero settlements); (c) trusted same-user host code only — asserted against `ENFORCEMENT_SCOPE`. Finally it pins the amended attestation verbatim-shaped (`attack_route` contains `applyEvent`; `denial_shape` contains `UNGATED`; `disclosed_bound` contains `trusted-host-writable`, `REBUILDABLE layer`, `nothing consequential reads it as authority`) — a future "fix" that reverts the inventory to claiming the frame-helper gate covers the store write path FAILS the suite.

**RED-first proof:** the test was written and executed BEFORE the module amendment. Observed RED: `TS2339: Property 'disclosed_bound' does not exist on type 'PrivilegedBoundary'` (3 sites) and the boundary test failing on the missing attestation. After the amendment: GREEN. The store-write demonstration itself is a true-state pin (it never "passed falsely" — what went RED was the suite-level assertion that the inventory DESCRIBES the true state, which is exactly the defect the verifier found).

**Why this stayed non-blocking (recorded, agreed):** the projection is the noncanonical rebuildable layer; nothing consequential reads it as authority; only trusted same-user host code reaches `applyEvent` (inside the declared enforcement scope). The store path is BY DESIGN not an admission boundary; the defect was the inventory's overclaim, and that is what was fixed.

### MINOR — CLOSED: disclosure-count wording made exact

Round 0 said "all seven required disclosures present" while `REQUIRED_ISOLATION_DISCLOSURES` pins SIX module patterns. The round-0 sentence is amended in place to name exactly what is pinned where: SIX module disclosures in `REQUIRED_ISOLATION_DISCLOSURES` (prime-kernel, prime-host, prime-long-running, prime-recursion, plugins, terminal-session) PLUS the seventh honesty anchor — the `ENFORCEMENT_SCOPE` statement — pinned verbatim by the suite's dedicated scope assertion. The count is now true in both the evidence and the code (the suite asserts both lists).

### Scanner calibration — the verifier's candidates added, each with a positive AND a corpus negative

`detectSecretPatterns` additions:
- UPPERCASE env-style keys: the `key_value_secret` rule is now case-insensitive (`PASSWORD=xK9mQ2vL8pR4tZ7w` detects). Positive asserted; negatives `PASSWORD_HASH_ALGORITHM=argon2id` and `PASSWORDS_FILE=/etc/checked` added to the corpus.
- `AWS_SECRET_ACCESS_KEY=`: the alternation now names `aws_secret_access_key` IN FULL. Calibration finding worth recording: `\b` before a mid-word key never fires ("SECRET" inside `AWS_SECRET_ACCESS_KEY` is preceded by `_`), so compound cloud-key names must be enumerated whole — the same `\b` that admits the compound is what keeps the look-alike `AWS_SECRET_ACCESS_KEY_POLICY=managed` out (corpus negative asserted, detects nothing).
- `ghp_` PATs: added to the `known_token_shape` rule (`ghp_` + 20+ alphanumerics). Positive asserted; negative `GitHub PATs look like ghp_ (elided in docs)` in the corpus.
- JWTs without key hints: NEW rule and NEW `SecretKind` `"jwt"` — a three-segment base64url shape anchored on the always-`eyJ` encoded header, so detection needs no key hint. Positive asserted (kind `"jwt"`); negative `the JWT header segment always starts eyJ (plain base64 of a brace)` in the corpus.
- Single-quoted values: already carried by the `key_value_secret` quoted alternative; now asserted positively (`token: 'yK8mQ2wL9pR5tZ3x'`) with the corpus negative `token: 'short'`.

The full corpus (now 21 lines) is asserted twice: zero detections over the whole corpus, and byte-identical redaction output per line.

### New trigger-records (recorded, NOT fixed — this builder's files, owning tasks named)

1. **perform-hang instance-wide stall (T09-owned).** `withObserveDeadline` covers the OBSERVE leg only (its module doc now says so verbatim). A route whose PERFORM never settles still stalls its whole gateway instance — the serialized decision queue holds the drive open — the same instance-wide DoS shape, one leg over. Fix shape: a composition-level perform-deadline and/or a host-level drive budget; it is T09-owned because the queue is the gateway's. (Same shape family as T15's `callCapability` no-timeout record.)
2. **`redactDeep` does not descend Map/Set (this module; recorded).** Descent covers strings, arrays, and plain objects; Map/Set instances pass through UNREDACTED as opaque leaves (module doc now says so verbatim). The guarded channels (snapshots, frames, manifests, outcome objects) are plain JSON-shaped by construction; callers holding Map/Set payloads must convert first.
3. **Real-PTY echoed paste in scrollback (T13 surface; recorded).** A real PTY echoes operator input into output, so a pasted secret appears in the session scrollback of the DELIBERATE PTY viewport. That surface is the DSN-014 deliberate viewport, not a forbidden channel; the forbidden-channel assertions in `secret-leakage.test.ts` run against provenance/events/projection bookkeeping (input BYTES are never recorded there) and against the FakeProvider's non-echoing path, which is what they always claimed.

### Parallel-wave contract change absorbed (recorded for the audit trail)

Mid-round, a parallel correction round landed a stricter `terminal-session.ts` spawn contract (bare program names now resolve through a declared `programResolution` table to anchored ABSOLUTE paths before any provider call — new typed refusals `bare_program_unresolved` / relocated `program_not_in_allowlist`). This builder edited nothing in that module; the owned suites attack the CURRENT contract from outside: the terminal.spawn.request attack matrix now drives both new refusal shapes (bare name with no resolution table; bare name absent from the table) alongside the unchanged traversal/metacharacter/outside-roots shapes, and the leakage terminal leg declares `{cat: "/bin/cat"}`.

### Correction Round 1 gate results (verbatim)

| Gate command | Result | Notes |
| :--- | :--- | :--- |
| `just tui-test security-boundaries` | PASS (exit 0) | 26 pass / 0 fail (+1 MAJOR-2 store-write/attestation test), 226 expect() calls |
| `just tui-test secret-leakage` | PASS (exit 0) | 15 pass / 0 fail (+1 calibration test), 169 expect() calls |
| `just tui-test deny-bypass` | PASS (exit 0) | 16 pass / 0 fail, 293 expect() calls (unchanged and green) |
| `just tui-check` | PASS (exit 0) | tsc --noEmit --strict clean across the whole tree |
| `just tui-lint` | PASS (exit 0) | 0 errors; 34 warnings / 469 infos — the pre-existing baseline classes; T18's three files lint-clean |
| `just tui-preflight bootstrap` | PASS (exit 0) | unchanged |
| `cd workbench && bun test` | PASS (exit 0) | 467 pass / 0 fail / 2 gated skips across 43 files, 5510 expect() calls (parallel wave suites included) |
| `just prereg-check gauntlet-tui-prime-workbench-T18` | PASS (exit 0) | prereg still byte-identical (`4bd5c124…0583d3`) |

RED-first was observed and recorded before each GREEN: the MAJOR-2 attestation assertions failed on the unamended module (`disclosed_bound` absent; the "jwt" kind not yet in the vocabulary), and the calibration positives failed before the patterns landed. No test was disabled, weakened, or skipped; the round-0 sections above stand as written except the single disclosure-count sentence amended in place (noted above).

The round-0 inventory table's `evidence.frame.fabrication` row is superseded by the amended entry in `security-hardening.ts` (this section); the table is not the normative inventory — the frozen-in-suite data is.

Confirmation: BUILDER evidence (Correction Round 1) pending re-confirmation — MAJOR-2 is closed RED-first with the true state pinned in the suite and the inventory attestation made honest; the wording MINOR and the scanner calibration are closed; three trigger-records are recorded with owning tasks; settlement authority remains with the orchestrator's evidence review.
