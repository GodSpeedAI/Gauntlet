# Recon: canonical-ops-prime

Read-only recon of the Gauntlet TUI workbench, 2026-09-11. It covers which consequential operations exist end to end, how the production TUI reaches them, and what Prime integration is actually live.

Anchor caveat: both `.agents/specs/tui/*.md` drafts were modified at 12:00 today while this recon was running. Draft anchors below were re-captured after that edit, but they may drift again.

## 1. Production composition

- **Entry point.** `main.ts:10-12` builds `WorkbenchHost(resolveConfig({}))`. The shell identity is hardcoded at `main.ts:23` as `{workspace:"gauntlet", reference:"canonical surface"}`.
- **Renderer.** The renderer runs with `useMouse:false` (`main.ts:84`). Keypresses go to `ShellSession.key` (`main.ts:95-107`), then `host.start(draw)` begins polling (`main.ts:114`).
- **Host.** `WorkbenchHost` (`workbench-host.ts:84-115`) wires the following:
  - `GauntletReadClient` (read), `HttpOperatorActionRoute`, and `OperationGateway`, all bound to the same endpoint (`:95-97`).
  - `CanonicalSurfaceStreamPort` feeding `LiveStreamClient` (`:98-99`).
  - `WorkbenchModel{enableChat:false}` (`:90`).
  - A `PrimeDelegationDispatcher` over `RealPrimeHost`, created only when `prime_binary` is set (`:100-110`). The default is null (`config.ts:60`); the env var is `GAUNTLET_WORKBENCH_PRIME_BINARY` (`config.ts:103`).
- **Read loop.** `read()` (`workbench-host.ts:139-225`) polls every 2 s. It calls `port.observe()` (GET `/state` + `/capabilities` + per-unit `/inspection`) in parallel with `route.observe()` (GET `/state`).
- **Capability facts.** Every capability gate is set to false. Prime is reported as `installed:true` but never reachable (`:175-217`).
- **Outbound edges.** `interactions()` (`:227-249`) exposes only these:
  - selectFocus
  - hoist
  - dehoist
  - invokeAffordance → `invoke` (`:251-272`)
  - submitComposer → `submit` (`:274-290`)
- **Close.** `close()` (`:292-296`) never touches `primeDispatcher`.

## 2. Core loopback surface (`gauntlet serve`)

- **Availability.** The command exists only under the `surfaces` feature (`serve_cmd.rs:1-13`).
- **Bind and authentication.**
  - The bind comes from `GAUNTLET_API_BIND` and is loopback-only (`serve_cmd.rs:34-61`; `http.rs:123-156`).
  - One `DurableSurface` backs all four seams (`serve_cmd.rs:69-84`).
  - Token auth is enabled only when `GAUNTLET_API_TOKEN` or `GAUNTLET_SESSION_TOKEN` is set, with the actor taken from `GAUNTLET_SESSION_ACTOR` (`serve_cmd.rs:85-99`).
- **Routes** (`http.rs:277-292`):

  | Method | Path | Anchor |
  |---|---|---|
  | GET | `/capabilities` | :278 |
  | GET | `/state` | :279 |
  | POST | `/refresh` (a read) | :280 |
  | GET | `/escalated` | :281 |
  | GET | `/runs/{id}` | :282 |
  | GET | `/units/{id}` | :283 |
  | GET | `/units/{id}/inspection` | :284 |
  | POST | `/units/{id}/resolve` | :287 |
  | POST | `/units/{id}/retry` | :290 |

  Nothing else is served.
- **Handshake.** `unsupported_operations` lists `launch`, `steering`, `cancellation` and `evidence_admission` (`http.rs:336-342`).
- **Action path.**
  - `action_response` (`http.rs:438-589`) requires the `X-Gauntlet-Operator` header (`:445-454`) and a session token when auth is on (`:458-493`).
  - The body must be empty or a JSON object carrying `note` (`:494-541`); `?run=` is optional (`:542-553`).
  - It calls `DurableSurface::perform` (`durable_surface.rs:550-553`), which maps resolve/retry to escalation-fs actions and `operator_action::execute_operator_action`.
  - Outcomes: accepted → 200 with `{action, run_id, unit_id, committed_now, tickets_resolved}` (`http.rs:565-583`); refused → 409; failed → 500 (`:584-588`).
- **Error body.** Always nested: `{"error":{"code","message"}}` (`lib.rs:205-213`; `http.rs:904-911`).
- **`/state` body.** `run, escalated, in_flight, units{id,status,tier}, budget, sandbox_denials, open_residuals, run_derivation` (`lib.rs:94-150`). It has no `open_escalations`, `budget_remaining`, `governance_denied_units` or `authority_revoked_units`.
- **CLI-only operations** (no HTTP route):
  - `gauntlet run` (`gauntlet-cli/src/lib.rs:65`)
  - `verify` (`:68`)
  - `reference digest|validate|form|freeze|show|diff|verify` (`reference_cmd.rs:13-19`)
  - `escalations list|resolve|retry` (`escalations_cmd.rs:25-30`)
- **Domain types.** A grep for `pub (struct|enum) (Case|CaseId|Thread|ThreadId|Wayfinder|Conversation|Message)` over `crates/` returns nothing.

## 3. Operation reachability

"Prod TUI" means `main.ts` → `WorkbenchHost`. Keyboard only, because mouse input is disabled at `main.ts:84`.

| Operation | Core surface route | Workbench client fn | Reachable from prod TUI | Notes |
|---|---|---|---|---|
| case.create | none (http.rs:277-307) | none | no | ICM:583 says BIND_EXISTING, but there is nothing to bind; crates define no Case type |
| case.open | none | none | no | Chrome identity is a literal (main.ts:23); active run is derived server-side (`run_derivation`, lib.rs:145-148) |
| case.list | none | none | no | — |
| resource.bind | none | none | no | ICM:640 |
| thread.create / list | none | none | no | ICM:659; crates define no Thread/Message types |
| thread.message.submit (to Orchestrator) | none | none | no. Composer Enter → shell.ts:1286-1290 → host.submit shows "unsupported…Message was not sent" (workbench-host.ts:283-287) | Default Composer is mode `message`, destination `workbench` (composer.ts:88-93); `prime.message` is planned only (planned-operations.ts:117-136) |
| orchestrator conversation read | none | none | no | Chat is disabled (workbench-host.ts:90 → views-chat-trace.ts:52-68) |
| wayfinder.continue / answer / freeze | none | none | no | ICM:801/817/842 |
| reference.candidate.* / revision.begin | none (no CLI verb either) | none | no | ICM:866/886/933 |
| reference.validate / freeze | CLI only (reference_cmd.rs:14,16) | none | no | ICM:897/908 |
| run start | CLI only (gauntlet-cli lib.rs:65); HTTP lists `launch` unsupported (http.rs:338) | none | no | `prime.launch` launches a Prime child, not a Gauntlet Run (planned-operations.ts:95-116) |
| run.admission.inspect | partial read: GET /runs/{id} (http.rs:282) | fetchRun (client.ts:186), not on the prod path | no | The port uses only fetchState, fetchCapabilities and fetchInspection (canonical-surface-stream-port.ts:125-156) |
| run.request_cancel | none; `cancellation` unsupported (http.rs:340) | none | no | ICM:1014 |
| run.reattach | none | GET /state re-poll + epoch resync (workbench-host.ts:139-171; canonical-surface-stream-port.ts:137-152) | partial: reconnect only | Not an operation; no Run lifecycle reattach |
| verification.run | CLI only (lib.rs:68) | none | no | ICM:1080 |
| verification.retry (≈ unit.retry) | POST /units/{id}/retry (http.rs:290) → durable_surface.rs:550-553 | unit.retry (operation-registry.ts:70-78) → OperationGateway.invoke (operation-gateway.ts:250) → HttpOperatorActionRoute.perform (:623-686) | **yes**, two ways: (a) palette Enter twice (shell.ts:674-693) → applyShellEffects (:491) → host.invoke (workbench-host.ts:242-243, 251-271); (b) Composer `operation` mode with destination = op id (workbench-host.ts:275-279) | This is an escalation retry, not a verification re-run. Wire defects W1/W2 apply. Palette rows come from `materializeAffordances` over `operationIds()` (views-work.ts:117; shell.ts:558-570) |
| evidence.offer / admit / reject | none; `evidence_admission` unsupported (http.rs:341) | none | no | `evidence.admit` is planned (planned-operations.ts:177-196) |
| settlement.evaluate / readiness | none | none | no | ICM:1476 |
| settlement decision (operator resolve) | POST /units/{id}/resolve (http.rs:287), the one operator-action path (lib.rs:16-22; action.rs:130-136) | unit.resolve (operation-registry.ts:61-69) → gateway → perform | **yes**, same two paths as retry | W1, W2 and W3 apply. W3: tickets with a null unit match every unit (affordance.ts:265-267) |
| state refresh | POST /refresh (http.rs:280) | state.refresh short-circuits to a re-GET (workbench-host.ts:252-255) | yes | Registered but not action-admitted (operation-gateway.ts:291-298) |
| attention.acknowledge | none | none | no | ICM:1542; only escalated ticket ids are read (/state `escalated`) |
| attention.respond.* (approval / question) | none (only resolve/retry on tickets) | none | no | ICM:771. `beginTakeover` (shell.ts:1317) has no non-test caller, so Composer takeover never happens in prod |
| agent.spawn | none | PrimeDelegationDispatcher.delegate (prime-recursion.ts:442) | no. Constructed only (workbench-host.ts:100-107); only caller is testing/cross-surface-matrix.ts:710 | Spawn is gated on an accepted unit.resolve/retry (prime-recursion.ts:272-304) |
| agent.request_stop | none; canonicalStopAvailability returns `registered:false` (prime-long-running.ts:478-487) | RealPrimeHost.stopChild, a direct SIGKILL (real-prime-host.ts:298-356) | no | Not gateway-routed; no prod caller |
| agent.message | none | deliverOperatorMessage (prime-long-running.ts:139-169) | no (Composer `message` → unsupported notice) | RealPrimeHost mailbox is in-memory bookkeeping only (real-prime-host.ts:543-553); it never reaches the process |
| kernel.execute | none | PrimeReplKernel.execute (prime-kernel.ts:57-115) | no (Composer `kernel` → unsupported notice; `prime.steer` has composer_mode kernel, planned-operations.ts:137-157) | Simulated: regex variable capture plus a 5 ms sleep (prime-kernel.ts:79-104). Not constructed by the host |
| kernel.interrupt | none | PrimeReplKernel.interrupt (prime-kernel.ts:120-122) | no | Sets a flag only |
| kernel.restart | none | none (only terminate, prime-kernel.ts:196-199) | no | ICM:1631 |
| kernel.object.pin / artifactize | none | transitionLifecycle (prime-kernel.ts:127-140) | no | Artifactize sets a path string and writes nothing |

Bottom line: the only consequential operations that exist end to end are **unit.resolve** and **unit.retry**. The Rust surface serves nothing else, and the workbench catalog mirrors that exactly (`operation-registry.ts:24-28`).

## 4. Wire-shape defects: workbench versus the real Rust surface

- **W1: refusal codes are lost.**
  - The workbench reads a flat `body.code` / `body.message` (`operation-gateway.ts:680-681`).
  - Rust sends `{"error":{code,message}}` (`lib.rs:205-213`; `http.rs:904-911`).
  - Effect: every real 4xx/409/5xx outcome becomes code `surface_error` with an empty message. That breaks the gateway's own "refusal code preserved" claim (`operation-gateway.ts:31-34`).
- **W2: free-text notes are rejected.**
  - The workbench POSTs the note as raw `text/plain` (`operation-gateway.ts:626, 642`).
  - Rust parses a non-empty body as a JSON object and returns 400 `body_unparsable` otherwise (`http.rs:494-541`).
  - Effect: Composer `operation` mode sends the draft as the note (`workbench-host.ts:278`), so any free-text draft is refused. Palette invocations send `note=""` (`workbench-host.ts:251`) and pass.
- **W3: `/state` fields the workbench expects are not served.**
  - The workbench observe reads `open_escalations`, `budget_remaining`, `governance_denied_units` and `authority_revoked_units` (`operation-gateway.ts:578-619`). Rust `/state` serves none of them (`lib.rs:94-150`).
  - Tickets fall back to `unit_id:null` (`operation-gateway.ts:588-590`). The resolver matches null against any unit (`affordance.ts:265-267, 295-300`), so resolve predicts `requires_attention` on every unit whenever any ticket is open.
  - The budget and governance gates can never fire from real data.
- **W4: the test stubs encode the workbench's assumed wire, not the Rust wire.** They send a flat `{code,message}` (`testing/canonical-surface-stub.ts:419`; `deny-bypass.test.ts:1219-1222`) and treat the raw text body as the note (`canonical-surface-stub.ts:466`). W1 and W2 are therefore invisible to the suite. No workbench test launches the real `gauntlet serve`.

## 5. Prime integration

| Module | Live in prod entry? | What it actually is | Anchors |
|---|---|---|---|
| prime-host.ts `PrimeHostAdapter` | Only as RealPrimeHost's internal bookkeeping | Declared in-memory test double. Spawn returns the handle immediately; the model default is the literal `claude-3-5-sonnet` | prime-host.ts:86-122, :107 |
| real-prime-host.ts `RealPrimeHost` | Constructed iff `prime_binary` is set; the constructor runs mkdir (0700) and a reattach scan at TUI start. `spawnChild` is never called in prod | `Bun.spawn prime-agent --mode json -p <task> --offline --no-tools -ne -ns -nc --session-dir --cwd`, one-shot. Parses newline-JSON `message_end` assistant text after exit | real-prime-host.ts:230-265, 482-514, 169-186, 588-611 |
| prime-recursion.ts `PrimeDelegationDispatcher` | Constructed, never invoked | Canonical perform first, then the duplicate guard, then spawn, then the envelope mailbox message. Policy limits: depth 3, 4 children per parent | prime-recursion.ts:64-67, 272-332, 442-524 |
| prime-long-running.ts | Not used by the host | Mailbox delivery plans, completionStatus, PrimeWorkbenchHost detach/close, reattachToPrime, substrate refusals | :54-97, 139-169, 375-434, 454-487 |
| prime-kernel.ts `PrimeReplKernel` | Not used by the host. Exported (index.ts:29) and imported by kernel-affordances.ts and evidence-promotion.ts | Simulated Python execution | :79-104 |
| prime-topology.ts | Only via dispatcher.topology / reattachToPrime, neither called in prod | Pure parent/child tree reconstruction | :36-69 |
| prime-context-envelope.ts | Not used by the host | Bounded envelope by default; whole-Case envelope only by explicit call | :226-314, 335-369 |
| substrate-inventory.ts | preflight.ts uses probePrimeSurface; main.ts does not | Three-way provenance with literal-false canonical fields. Real binary has `session_stop` = exposed_executed, yet the workbench admits no stop | :214-236, 627-655, 657-690, 808-839 |

**Transport.**
- The workbench implements no client for Prime's host JSON-lines protocol (`execute` / `interrupt` / `host_request` / `host_reply`). The spec requires it (spec:100-111; DESIGN.md:885); a grep for `host_request` in non-test `workbench/src` is empty.
- RealPrimeHost uses one-shot `-p` print mode with `--no-tools`, so the child has no Python REPL tool at all.
- The only code in the repo that drives Prime's REPL is the Rust core adapter, which tells Prime to "Execute … as a single IPython cell" (`crates/adapters/gauntlet-adapter-agent-prime/src/transport.rs:323`). The workbench does not use it.

**Spawn admission versus completion.**
- The handle is returned before any output is read (`real-prime-host.ts:505-507`).
- Completion requires a result contract (`prime-long-running.ts:86-97`).
- **But the host writes the contract itself:** `watch` records `{exit_code,…}` to `contractPath` (`real-prime-host.ts:613-622`), and `contractFromEvidenceRecord` maps exit code 0 to `schema_valid:true, outcome:"pass"` (`:123-138`). Process exit therefore stands in for a result contract.

**Message as communication.**
- Messages are recorded with kind `communication` (`real-prime-host.ts:605-610`), and the type literally carries `establishes_completion:false` (`prime-long-running.ts:54-66`).
- Operator messages land only in in-memory bookkeeping (`real-prime-host.ts:543-553`). The spawn configures no stdin (`:509-514`), so nothing reaches the real child.

**Lifecycle independence (REQ-PRIME-009, spec:1518-1521).**
- `WorkbenchHost.close` does not stop Prime (`workbench-host.ts:292-296`). Detach issues literally 0 cancel calls (`prime-long-running.ts:411-423`). The qualified manifest is written before spawn returns (`real-prime-host.ts:519-533`).
- Caveats:
  - While the TUI is alive, the host kills the child after 600 s (`:234, 576-582`).
  - Child stdout/stderr are pipes owned by the TUI (`:512-513`).
  - Only the TUI-side watcher writes the contract, so a child that outlives the TUI is later recorded `inconclusive` (`:460-467`).

**Reattach (REQ-PRIME-010).**
- `reattachFromQualifiedRecords` (`real-prime-host.ts:370-430`) rebuilds sessions only from `*.qualified.json` manifests and reports stale records honestly.
- It runs in prod at construction, but nothing surfaces the result.
- The Agents view reads `snapshot.sessions` (`views-chat-trace.ts:224-266`), which is fed only by `session_updated` events. Those are produced only by fixtures (`worker-folds.ts:1071`; `payment-budgets.ts:284`); the canonical port emits only `unit_updated` / `evidence_added` (`canonical-surface-stream-port.ts:171, 185`).

**Jupyter versus Prime REPL.**
- No code in the workbench implements Jupyter; `workbench/src` contains no Jupyter token.
- Its kernel is a simulated "Prime Python REPL" (`prime-kernel.ts:1-10, 79-104`).
- The normative documents say Prime REPL over the host protocol, not Jupyter:
  - spec protocol_boundary (spec:108-111)
  - REQ-PRIME-006, "Prime-owned Python REPL mediation path" (spec:1505-1508)
  - the key renamed from `prime_and_jupyter_integration` (spec:40)
  - DESIGN.md:885, "not a Jupyter kernel"
- The drafts contradict this:

| Draft | Anchor | Wording |
|---|---|---|
| UESC | :7 | "Prime/Jupyter projections" |
| UESC | :142 | "Jupyter kernel lifecycle \| Prime/Jupyter host" |
| UESC | :209 | "Gauntlet + Prime + Jupyter + repository…" |
| UESC §19 | :1650, :1655, :1678 | "Jupyter/kernel elements"; "persistent IPython/Jupyter kernel"; "Jupyter/Prime host" |
| UESC | :2392 | "Prime/Jupyter lifecycle semantics" |
| ICM | :7, :38 | "Prime/Jupyter controls", "(Prime/Jupyter/filesystem/etc.)" |
| ICM | :97 | "REQ-PRIME-006 \| … returns through Prime/Jupyter mediation" (misquotes spec:1505-1508) |
| ICM | :188 | "Jupyter interrupt/restart/input response" |
| ICM | :231, :275 | "Prime/Jupyter/filesystem/etc. adapter"; owner "Prime/Jupyter adapter" |
| ICM | :794 | "Must return through Prime/Jupyter mediation." |
| ICM §23 | :1603, :1626 | "Kernel/Jupyter interactions"; "Prime-owned Jupyter control mediation" |
| ICM | :2561 | diagram "Prime/Jupyter/tools" |

## 6. Operator Chat / Orchestrator today

The Orchestrator conversation does not exist: it is not a Prime session, not a canonical thread, and nothing else stands in for it.
- **Composer.** It defaults to mode `message` with destination `workbench` (`composer.ts:88-93`). Submitting ends in the notice "unsupported; 'prime.message' is declared (planned…). Message was not sent." (`workbench-host.ts:283-287`).
- **Chat view.** It shows "Chat projection disabled by the host" (`views-chat-trace.ts:52-68`), because `enableChat:false` is set at `workbench-host.ts:90`.
- **Chat model when enabled.** The model defaults Chat on (`workbench-model.ts:386`). It then invents `system` chat rows from `unit_updated` events (`workbench-model.ts:531-539`).
- **Backend.** No Rust Thread/Message/Orchestrator types or routes exist.

## 7. Classification

### preserve
1. OperationGateway (`operation-gateway.ts:191-422`):
   - deterministic idempotency key
   - rejects a reused key with a different payload
   - fails closed when revalidation is impossible, on every drive
   - `outcome_unknown` is never retried under a new key
   - serialized decisions
2. HttpOperatorActionRoute (`operation-gateway.ts:623-679`): operator and bearer headers, `?run=` anchor, and it refuses to fabricate an acceptance.
3. Operation registry mirroring Rust (`operation-registry.ts:24-88`), plus planned nine-field contracts and handshake v2 (`planned-operations.ts:92-289`).
4. CanonicalSurfaceStreamPort derives epoch and cursor from observed facts, with declared snapshot granularity (`canonical-surface-stream-port.ts:124-198`).
5. WorkbenchHost single-flight refresh, poll loop, and Focus cleared only on resync (`workbench-host.ts:121-171`).
6. Palette two-step confirm, delivered as a ShellEffect edge (`shell.ts:674-693, 473-496`).
7. Honest unsupported submission for message/prime/kernel (`workbench-host.ts:274-290`).
8. Rust surface: loopback bind, token auth, typed accepted/refused/failed through one operator-action path (`http.rs:438-589`; `serve_cmd.rs:26-113`; `durable_surface.rs:550-600`).
9. PrimeDelegationRoute: canonical admission first, spawn-time duplicate guard, receipt before spawn (`prime-recursion.ts:272-332`).
10. RealPrimeHost qualified manifest, reattach from manifests only, and stale records (`real-prime-host.ts:370-430, 515-533`).
11. Communication is not completion, enforced at the type level (`prime-long-running.ts:54-97`).
12. Host close/detach issues no cancel (`workbench-host.ts:292-296`; `prime-long-running.ts:411-433`).
13. Substrate three-way provenance with literal-false canonical fields (`substrate-inventory.ts:214-236`).
14. Bounded context envelope by default (`prime-context-envelope.ts:226-369`).
15. Spawn isolation: 0700 directories, `LD_*` stripped, validated program path (`real-prime-host.ts:196-209, 242-263`).

### replace
1. Connection status and run id are string-prefixed into the header (`main.ts:34-43`).
2. The global `host.notice` string is the only operation feedback (`workbench-host.ts:256-270`). There is no per-control BUSY_SUBMISSION (ICM §30, :1887).
3. Composer `operation` mode takes a typed destination as the operation id (`workbench-host.ts:275-281`).
4. Chat "disabled by host" placeholder (`views-chat-trace.ts:52-68`).
5. Agents view rows come from fixture `session_updated` events only (`views-chat-trace.ts:224-266`).
6. Literal workspace/reference identity in chrome (`main.ts:22-26`).

### violates
1. W1, flat refusal-code read: `operation-gateway.ts:680-681`. Breaks ICM §33 (:1988) and REQ-AFF-002.
2. W2, raw text note body: `operation-gateway.ts:636-643`. Breaks REQ-AFF-002.
3. W3, null-unit ticket fallback makes resolve over-available: `operation-gateway.ts:588-590` with `affordance.ts:265-267`. Breaks ICM §30.
4. Host-authored result contract where exit 0 means pass: `real-prime-host.ts:123-138, 613-639`. Breaks REQ-PRIME-003 and UESC §1 (spawn admission is not child completion).
5. Delegation spawn rides an accepted unit.resolve/retry commit: `prime-recursion.ts:272-304, 499-503`. Breaks UESC §1 and ICM agent.spawn (:1134).
6. Chat rows synthesized from `unit_updated` events: `workbench-model.ts:531-539`. Breaks REQ-DATA-005 (spec:1543-1546) and UESC §1, Message is not Event (:46).
7. `host.notice` injected as an attention row with the `attention.required` token: `main.ts:44-53`. Breaks UESC §1, Cue is not AttentionItem (:56).
8. Simulated Python kernel exported as a production module: `prime-kernel.ts:79-104`; `index.ts:29`. Breaks DESIGN.md:885 and REQ-PRIME-006.
9. Draft Jupyter wording: UESC:1650-1678; ICM:97, 1603-1626. Breaks spec:108-111 and DESIGN.md:885.

### missing
1. Case create/open/list and resource.bind: no route or types (ICM:583, 625, 640; UESC §10, :649).
2. Thread create/list, thread.message.submit, and the Orchestrator conversation (ICM:659, 702; UESC §10.3, :693 and §10.5, :738; REQ-TUI-003).
3. Wayfinder continue/answer/freeze (ICM:801-842).
4. Reference candidate/validate/freeze/revision on the surface; today CLI-only or absent (ICM:866-933).
5. Run start/cancel/admission-inspect/reattach operations (ICM:976-1027; http.rs:338-340).
6. verification.run, evidence offer/admit/reject, settlement.evaluate (ICM:1080, 1413, 1476).
7. attention.acknowledge, attention.respond.*, and a production takeover source (ICM:771, 1542; shell.ts:1317).
8. agent spawn/stop/message as canonical operations with prod wiring (ICM:1134-1160; REQ-PRIME-001).
9. A real Prime host JSON-lines kernel client for execute/interrupt/restart/input (ICM:1606-1651; spec:100-111).
10. Prime session topology projected into Agents (REQ-PRIME-001; UESC:558).
11. `/state` gate facts: open_escalations, budget_remaining, governance/authority unit lists (lib.rs:94-150).
12. Ordered durable event stream for Trace; the port polls snapshots only (canonical-surface-stream-port.ts:26-31; UESC §16, :1251).

## 8. Notes
- The action path works only against `gauntlet serve` built with `surfaces`. The workbench session is null unless both `GAUNTLET_SESSION_TOKEN`/`GAUNTLET_API_TOKEN` and an actor are present (`workbench-host.ts:111-114`; `config.ts:106`).
- `client.ts:411` `mutateUnit` duplicates `HttpOperatorActionRoute.perform`; the prod host uses only the latter.
- `unit.retry` is escalation retry (`durable_surface.rs:552-553`). Mapping it to ICM `verification.retry` is approximate.
- The gated real-Prime batteries (prime-fresh-target, prime-delegation-real-battery, prime-cross-process-lifecycle-real-battery) were not re-run. Their daemon-survival claims are unverified here.
- What happens to a child when the TUI dies while its stdout pipe is open (SIGPIPE) was not verified.
- Mouse is disabled in prod (`main.ts:84`). The mouse contract belongs to another area.
