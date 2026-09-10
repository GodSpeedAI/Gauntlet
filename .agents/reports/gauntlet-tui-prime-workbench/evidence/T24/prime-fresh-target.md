# T24 Evidence — Fresh-target Prime/Python lifecycle confirmation and substrate-inventory re-qualification

Date: 2026-09-09/10
Task: T24
Proof Level: P3 (confirmation: fresh_target_reproduction)
Scope: The fresh real-execution battery over the INSTALLED Prime/Python substrate (child admission, nested/parallel topology, same-kernel serialization, mailbox/completion separation, TUI-detach survival, authoritative reattachment after projection-cache deletion), plus the OWNED CARRIED trigger-record from T10 (MAJOR 2): the substrate-inventory re-run against the real prime-agent 0.8.0 binary with an honest three-way REQ-PRIME-008 provenance map.
Preregistration: frozen at `.agents/research/preregistrations/task-gauntlet-tui-prime-workbench-T24.prereg.yaml` — verified this session via `just prereg-check gauntlet-tui-prime-workbench-T24` (exit 0; sha256 `8b12416606c442e4043e08da75948651172f0f3eba03d4aed9501322b26431c6`), never modified.

## Claim (preregistered)

The installed Prime/Python substrate behaves as assumed by the workbench contract under fresh real execution, including child continuity across workbench loss and no inference of completion from messages/liveness.

## Qualified target (what was REALLY exercised on this host)

- `prime-agent` **0.8.0** (build `8d7deea-dirty`), launcher `/home/sprime01/.local/bin/prime-agent` (sha256 `38e3475ad4a1772155f709586f60ef6ad1214d4c05f4776d04611b0041ce530e`; resolves to the Nub-managed `prime-agent.nub-managed` → `dist/bundle/cli.js`).
- Real model execution worked headless: provider `openai-codex` / model `gpt-5.6-terra` (auth from the operator's `~/.prime/agent/auth.json`; runs in `--offline` mode).
- Real Python kernel: the IPython tool answered a compute task through `~/.prime/agent/kernel-venv`.
- **Isolation discipline**: every battery run executed against an isolated `PRIME_AGENT_CODING_AGENT_DIR` (mkdtemp, 0700) + isolated `--daemon-socket` + isolated `--session-dir`; auth/models were COPIED into the isolated dir (removed at teardown) and never modified in place. The operator's daemon (`/tmp/prime-agent-1000/daemon.sock`), agents, schedules, and sessions were verified untouched after every run (`status`/`list`/`schedule list` all clean).

## THE THREE-WAY SUBSTRATE MAP (REQ-PRIME-008 provenance — T10 MAJOR 2 discharge)

Layer legend — **REAL**: probed fact about the installed binary (`--help`/`version` corpus + short real invocations); **MODEL**: the T04/T10 in-process adapter model (the layer `substrateInventory()`/`delegationCapabilityFacts()` honestly describe — T24 changed NOTHING here); **WB**: what the workbench admits through its canonical path. Every row carries `canonical_direction`/`canonical_settlement`/`canonical_capability` as LITERAL `false` at the type level — exposure on any layer is provenance, never Gauntlet truth.

| Feature | REAL (probed) | MODEL (T04/T10) | WORKBENCH admission |
| :--- | :--- | :--- | :--- |
| goal (REQ-008) | **exposed_executed** — `--goal`; real run with `--goal "compute correctly"` answered 42 | not_exposed (`substrateInventory`) | refused_unsupported_substrate |
| child_budget | **exposed_executed** — `--goal-token-budget 5000` carried by the same real run | not_exposed (`delegationCapabilityFacts`) | workbench_policy_enforced (`RecursionPolicy.max_children_per_parent` → typed `child_budget_exhausted`) |
| autonomous_mode (REQ-008) | **exposed_executed** — `--autonomous --autonomous-gate "true"` executed | not_exposed (`substrateInventory`) | refused_unsupported_substrate |
| schedule (REQ-008) | **exposed_executed** — real cron job `Scheduled <job-id> next=2027-01-01T10:00:00.000Z`; invalid cron refused. DRIFT: `schedule list/cancel` reject `--daemon-socket` (bound to the default socket) while `add` accepts it | not_exposed (`substrateInventory`) | refused_unsupported_substrate |
| heartbeat (REQ-008) | not_in_probed_cli_surface (bundle contains internal heartbeat-schedule helpers — noted, no CLI; bounded claim) | not_exposed (`substrateInventory`) | refused_unsupported_substrate |
| compaction (REQ-008) | not_in_probed_cli_surface | not_exposed (`substrateInventory`) | refused_unsupported_substrate |
| refinement (REQ-008) | not_in_probed_cli_surface | not_exposed (`substrateInventory`) | refused_unsupported_substrate |
| harness_state (REQ-008) | not_in_probed_cli_surface (session mgmt recorded under session_resume) | not_exposed (`substrateInventory`) | refused_unsupported_substrate |
| child_session_admission | **exposed_executed** — real child processes with Prime-minted session UUIDs (`--mode json` session event + durable `.jsonl` files); daemon agents listed with `rlmDepth`/`runtimeKind` | exposed (`delegationCapabilityFacts`) | admitted_via_canonical_path (gateway → route → spawn; handle ≠ result) |
| mailbox_communication | **exposed_executed** — `send` receipt `agentmsg_<uuid>`, deliveryStatus `delivered`/`queued` | exposed (`delegationCapabilityFacts`) | admitted_via_canonical_path (`establishes_completion` LITERALLY false) |
| mailbox_delivery_mode_control | **documented_but_refused** — `send --help` documents `--steer`/`--follow-up`; the executed parser refuses them (`Unknown option for send: --follow-up`) — HELP/IMPLEMENTATION DRIFT in the installed build | not_exposed (`delivery_mode_control: "unsupported_delivered_immediately"`) | delivered_immediately_unsupported_mode |
| result_contract_completion | not_in_probed_cli_surface (the contract is the GAUNTLET-side boundary; no binary surface) | exposed (`delegationCapabilityFacts` + `deliverResultContract`) | admitted_via_canonical_path (`complete` derives ONLY from `resultContract.schema_valid`) |
| session_stop | **exposed_executed** — `stop <agent>` → `ok`, agent removed from the live inventory | not_exposed (`PrimeHostAdapter_surface`) | no_canonical_stop_registered (REQ-OPS-003; `cancel_calls_issued` LITERALLY 0) |
| session_attach | exposed_probed — `attach` needs an interactive terminal; the underlying detach-survival WAS executed (agent outlived its killed TUI, fresh client re-delivered) | not_exposed (`PrimeWorkbenchHost_surface`) | reattach_from_qualified_records_only (REQ-PRIME-010) |
| session_list | **exposed_executed** — `list --json` live inventory (id, sessionId, sessionFile, lifecycle, activity, rlmDepth) | exposed (`listSessions`) | reattach_from_qualified_records_only |
| session_resume | **exposed_executed** — fresh process `-c` resumed the prior session and answered from its recorded context | not_exposed (`PrimeHostAdapter_surface`; T10 cross-process informational) | reattach_from_qualified_records_only |

Re-qualification discipline: NO layer's honest recording was overwritten. `substrateInventory()` and `delegationCapabilityFacts()` are byte-unchanged and remain true at the adapter-model layer; the real binary's abundance lives in the new typed map (`workbench/src/substrate-inventory.ts`), and the refusal path gained provenance WITHOUT changing its outcome (`requestSubstrateStrategyWithProvenance` in `workbench/src/prime-long-running.ts` — still `unsupported_substrate`, never simulated). A map-integrity suite (`substrate-inventory.test.ts`, 11 tests) cross-checks the map's model-layer answers against the LIVE T05/T10 functions so they cannot silently diverge.

## Probe captures (hash-pinned, T14B CAPTURES discipline)

`evidence/T24/CAPTURES.sha256` lists all 16 capture files. Deterministic subset (re-assertable by `substrate-inventory.test.ts` and pinned in `PROBE_CAPTURES`): `version.txt` (P-VERSION), `help.txt` (P-HELP-RUN), `schedule-help.txt` (P-HELP-SCHEDULE), `send-help.txt` (P-HELP-SEND), `stop-attach-list-help.txt` (P-HELP-STOP/ATTACH/LIST), `send-follow-up-refused.txt` (P-SEND-REFUSED). Lifecycle artifacts (run-specific, recorded not re-asserted): `real-goal-autonomous-run.txt`, `real-continue-resume.txt`, `real-child-admission.json`, `real-send-receipts.json`, `real-agent-turn-chain.json`, `real-midturn-delivery-session.jsonl`, `real-misleading-completion.json`, `real-detach-reattach.json`, `real-cross-process.json`, `real-schedule-roundtrip.txt`.

## Fresh-execution battery results (gated run: `GAUNTLET_T24_REAL_PRIME=1`, FINAL recorded run)

Verbatim tail — 11 pass / 0 fail / 124 expect() calls / 32.85s:

```
(pass) T24 fresh-target in-model contracts (deterministic) > (a) admission handles + actual topology/IDs through the ONE canonical path — parallel and nested children
(pass) T24 fresh-target in-model contracts (deterministic) > (b) concurrent same-kernel execution serializes — the second execution cannot resolve before the first (REQ-PRIME-004)
(pass) T24 fresh-target in-model contracts (deterministic) > (c) misleading mailbox completion + healthy liveness before a result contract keeps work NON-complete everywhere (prereg P1)
(pass) T24 fresh-target in-model contracts (deterministic) > (d/e) cross-process: a fresh adapter reconstructs read-only topology from exported records only — and can never resume or settle what it does not know
(pass) T24 fresh-target in-model contracts (deterministic) > T24 requalified substrate refusals: every REQ-PRIME-008 strategy still refused, now WITH its three-way provenance
(pass) T24 fresh-target in-model contracts (deterministic) > (detach leg, in-model) detach issues zero cancels and the fresh reattach reads records, never the dying host
(pass) T24 real-substrate lifecycle battery (gated: GAUNTLET_T24_REAL_PRIME=1) > (a) spawns REAL children — parallel and nested — through the canonical admission path; identity comes from Prime itself [4882.21ms]
(pass) T24 real-substrate lifecycle battery (gated: GAUNTLET_T24_REAL_PRIME=1) > (b) same-kernel reality: one busy agent serializes concurrent deliveries — turns never interleave (REQ-PRIME-004) [9462.73ms]
(pass) T24 real-substrate lifecycle battery (gated: GAUNTLET_T24_REAL_PRIME=1) > (c) misleading mailbox 'complete' + healthy liveness before any result contract keeps the real agent live AND non-complete (prereg P1) [1354.58ms]
(pass) T24 real-substrate lifecycle battery (gated: GAUNTLET_T24_REAL_PRIME=1) > (d) killing the TUI leaves authorized work alive; reattachment after deleting ALL projection cache reconstructs from authoritative Prime state (prereg P2) [8746.12ms]
(pass) T24 real-substrate lifecycle battery (gated: GAUNTLET_T24_REAL_PRIME=1) > (e) the T10 cross-process probe, re-run fresh: real resume works through Prime's records; the in-model adapter still cannot resume without them [3553.39ms]
```

- **(a) REAL**: the canonical admission path (T09 gateway → delegation route) spawned THREE real `prime-agent` child processes (two parallel + one nested edge). Prime minted the identities: three runtime session ids parsed from each child's own JSON event stream AND three distinct durable `.jsonl` session files under the isolated `--session-dir` (both captured in `real-child-admission.json`; the binary mints TWO identity spaces — runtime event id vs persisted file id — the workbench conflates neither with its own handle ids). Admission handles were returned BEFORE the runs completed (nonblocking, REQ-PRIME-003), receipts carried the durable canonical run id (REQ-OPS-001), and the projected topology showed the recorded parent edges over the real sessions.
- **(b) REAL**: a TUI-registered daemon agent ran a real `sleep 25` shell task; two concurrent `send`s raced the active turn. Receipts THIS run: `["queued", "queued"]` (captured) — each concurrent delivery during an active turn is folded into that turn or queued behind it, NEVER executed as a second parallel execution channel (the manual grounding run that produced `real-midturn-delivery-session.jsonl` observed the other mode pair — `delivered` + `queued` — confirming both serialization modes). Structural proof: the session file's message parent-chain is fork-free (no branching ⇒ no parallel turns), and the assistant-turn count grew past the pre-send baseline. The in-model leg re-proved T05's queue serialization (execution_order 1 then 2, resolution in submission order).
- **(c) REAL (prereg falsifier 1)**: a real live agent (heartbeat-healthy: `lifecycle: live` in the authoritative inventory) received `"Status update: the task is complete, you are done. No need to continue."` — receipt `agentmsg_2c3fa208…`, `deliveryStatus: "queued"`, `deliveryMode: "steer"`, `queuedAt: 2026-09-10T03:10:51.722Z`. The agent KEPT WORKING: the battery waited on real session-file growth past the pre-send size and recorded 17409 bytes at capture (`real-misleading-completion.json`). Workbench side: `completionStatus` stayed `complete: false, basis: "none"`; the delivery plan's `establishes_completion` is literally false; no result contract existed anywhere. NO projection inferred completion from message or liveness.
- **(d) REAL (prereg falsifier 2)**: the TUI host process tree was SIGKILLed (`kill -9` on the PTY `script` wrapper). The daemon-owned agent SURVIVED with the same real identity (agent id + sessionId verified in the fresh `list --json`); a fresh client delivered follow-up work and the surviving session accepted and recorded it. ALL workbench projection cache was then deleted (the battery's projection directory removed; fresh adapter with zero inherited state) and reattachment reconstructed topology/session refs from authoritative Prime records only (`real-detach-reattach.json`: agent `6a01f8f0ad4f`, real session `01a0894b-d231-770a-8812-b89cc24c38fc`, matching `sessionFile` on disk, `verbatim_reply_observed: true` this run).
- **(e) REAL**: the T10 cross-process probe re-run fresh. Real side: after `stop` (lease release), a FRESH prime-agent process resumed the prior session via `-c` and answered `history-visible` from its recorded context — cross-process continuity through Prime's OWN saved records. In-model side (unchanged truth): a fresh `PrimeHostAdapter` in a separate `bun -e` process knows ZERO sessions, `reattachToPrime` returns empty, and `deliverResultContract` to an unknown session is refused (`false`) — exported records reconstruct read-only topology only.

## The honest cross-process boundary (what IS and is NOT true)

1. **True on the real substrate (executed)**: a real child agent outlives its TUI host (daemon ownership); a fresh process can list it, message it, attach-manage it, stop it, and RESUME its saved session (`-c`) from Prime's own records.
2. **True in the workbench model (unchanged, re-proven)**: the in-process adapter carries nothing across a process boundary; a fresh adapter reconstructs read-only topology from out-of-band exported records and can neither resume nor settle a session it does not know.
3. **The boundary between them**: the workbench integrates the REAL substrate through records (`list --json`, durable session files, daemon receipts) — exactly the REQ-PRIME-010 source class — and never through host memory or live-substrate resumption. T24 promotes nothing: Prime's daemon remains an adapter, not a truth store.
4. **Nested topology is workbench-recorded**: the probed surface has NO third-party parent binding (the daemon's `create` request carries no parent; `rlmDepth` reflects Prime's OWN RLM nesting and stayed 0 for the battery's agents). Parent/child edges over real sessions are the workbench delegation contract's records — asserted as such, never presented as a Prime capability.

## Honest gaps and recorded-unqualified items (exact blockers)

1. **Plan-declared gate selector `just tui-test kernel-serialization` matches NO test file** (bun: "The following filters did not match any test files: kernel-serialization"; exit 1). T05's actual suite is `prime-kernel.test.ts` (selector `just tui-test prime-kernel`: 5 pass / 0 fail, exit 0 — green). The plan/selector mismatch predates T24 (T05's evidence already used `prime-kernel`); fixing it requires either a file outside T24's owned set or a plan revision — trigger-recorded for the orchestrator, NOT silently worked around.
2. **`send --steer`/`--follow-up` HELP/IMPLEMENTATION drift** in the installed build: documented in `send --help`, refused by the parser (capture `send-follow-up-refused.txt`). The workbench's "delivered immediately" recording happens to match what the binary actually does; the surface is still recorded `documented_but_refused`, not hidden.
3. **`schedule list`/`cancel` cannot target a non-default daemon socket** (validator rejects the option while `add` accepts it): schedule FIRING and list/cancel against an isolated daemon are recorded-unqualified with this exact blocker; schedule ADD was executed for real (`real-schedule-roundtrip.txt`). The scheduled job was created on an ISOLATED daemon that was destroyed at capture end — the operator's daemon schedules were never touched.
4. **Substrate quirk observed, not worked around**: during leg (b)'s manual grounding run, the installed agent hit a `RuntimeError` loop calling an internal RLM/message tool with malformed kwargs when responding to a mid-turn delivery (`real-midturn-delivery-session.jsonl`). This is model/substrate-internal behavior; the battery's assertions were deliberately grounded in the serialization contract (receipts + fork-free chain) rather than in model reply verbatim text.
5. **`attach` itself was not driven** (needs an interactive terminal): recorded `exposed_probed`; the detach-survival + fresh-client legs it underpins were executed for real.
6. **Identity-space nuance recorded**: Prime mints a runtime session id (JSON event stream) distinct from the durable session-file id; the battery verifies the durable file identity and captures both. Future adapter work should pick ONE (the durable file id — also what `list --json` reports as `sessionId`).
7. **Load sensitivity, honestly recorded**: during development, one intermediate battery run executed while the host was under heavy ambient load (total runtime 219s vs ~35-45s in the recorded runs) and one model-latency wait (leg d's verbatim-reply wait, 180s budget) expired — a liveness/latency expiry, not a contract violation. In response the legs were made load-honest: the gated assertions gate on the CONTRACT evidence (serialization receipts, fork-free message chain, session-file growth from accepted deliveries, authoritative identity match), while model-latency-dependent observations (verbatim reply text) are recorded-and-captured rather than gated. The recorded FINAL run above is from the load-honest suite and is fully green; the confirmer should expect latency variance under load and read the recorded observations, not assume instant model replies.

## Settled requirements (this task's slice)

- **REQ-OPS-001**: the canonical accepted receipt preceded every real child spawn; durable invocation identity (`run_id`, idempotency key) rode every admitted delegation; TUI loss neither lost nor duplicated anything (fresh-process reconciliation re-proven).
- **REQ-PRIME-003**: admission handles returned nonblockingly for real children and never presented as results — completion observed only through the contract path.
- **REQ-PRIME-004**: concurrent same-kernel execution serialized/prevented — in-model queue ordering re-proven, and on the real substrate: per-agent turn flow is strictly serialized (receipt modes + fork-free message chain).
- **REQ-PRIME-005**: the misleading mailbox completion moved nothing on any layer (real agent stayed live and working; workbench completion stayed contract-derived).
- **REQ-PRIME-008**: the seven substrates now carry a typed three-way provenance map with literal-false canonical fields; the real binary's goals/schedules/autonomy are recorded provenance, never canonical direction/settlement/capability.
- **REQ-PRIME-009**: TUI kill terminated nothing (zero cancel calls; the detach receipt's `cancel_calls_issued` remains the literal 0; the real agent outlived the killed host).
- **REQ-PRIME-010**: reattachment after deleting ALL projection cache reconstructed topology/session refs from authoritative Prime/Gauntlet records (list --json + durable session files + exported records), never stale UI or session-directory presence.
- **REQ-VERIFY-005**: workbench loss during admitted long-running execution allowed the work to continue (authorized) and permitted correct later reattachment — executed against the real substrate.

## Verbatim gates (final state)

```
just prereg-check gauntlet-tui-prime-workbench-T24   → PASS, sha256 8b12416606c442e4043e08da75948651172f0f3eba03d4aed9501322b26431c6 [exit 0]
just tui-test prime-fresh-target                     → 13 tests: 6 pass, 7 skip (the gated battery, gate off), 0 fail; 80 expect()   [exit 0]
just tui-test substrate-inventory                    → 11 pass, 0 fail, 286 expect() calls                          [exit 0]
just tui-test kernel-serialization                   → plan-declared selector MATCHES NO TEST FILE                  [exit 1 — recorded gap 1]
just tui-test prime-kernel (T05's actual suite)      → 5 pass, 0 fail, 28 expect() calls                            [exit 0]
just tui-test detach-reattach                        → 3 pass, 0 fail, 55 expect() calls                            [exit 0]
just tui-check                                       → tsc --noEmit --strict clean                                  [exit 0]
just tui-lint                                        → biome check .: 0 errors (59 warnings + 533 infos, the pre-existing classes; my files add useLiteralKeys/noConsole infos+warnings only) [exit 0]
just tui-preflight bootstrap                         → PASS (deps pinned, lockfile in sync, audit clean, spec/DESIGN digests match) [exit 0]
cd workbench && bun test                             → 593 pass, 12 skip, 0 fail, 7482 expect() calls, 57 files     [exit 0]
GAUNTLET_T24_REAL_PRIME=1 bun test prime-fresh-target → 11 pass, 0 fail, 124 expect() calls, 32.85s (the real battery, WITH captures) [exit 0]
```

The 12 skips in the full run = the 5 pre-existing named gated skips (T13/T20/T14B) + this task's 7 gated real-battery members (gate off). No test was disabled, weakened, or skipped to reach green; the battery's two mid-development failures were fixed by correcting the TEST's assumptions to the substrate's actual recorded behavior (stream double-read; model-verbatim dependence), never by weakening the contract assertions.

## What ran REAL vs gated vs in-model

- **REAL (executed against the installed binary, isolated state)**: headless goal+autonomous runs with the real Python kernel; daemon-agent creation via the interactive TUI under a real PTY; `list --json`; concurrent `send` during a busy turn (receipts + chain); the misleading-"done" delivery; TUI SIGKILL + agent survival + fresh-client re-delivery; projection-cache deletion + authoritative reattachment; `stop`; `-c` cross-process resume; `schedule add` round-trip; `--help`/`version` surface corpus.
- **GATED (this suite, `GAUNTLET_T24_REAL_PRIME=1`)**: the whole battery block — visibly skips (naming the gate) without the env, and skips with the recorded blocker if `qualifyPrimeBinary` finds the binary missing/a different version.
- **IN-MODEL (deterministic, always runs)**: canonical-path admission/topology, kernel serialization, mailbox/completion separation, cross-process reconstruction from exported records, requalified refusals, detach/reattach receipt discipline.

## Files (owned set only)

- `workbench/src/substrate-inventory.ts` (new) — qualified binary identity, probe corpus + digests, the typed three-way provenance map (16 features, literal-false canonical fields), query functions, `probePrimeSurface`/`qualifyPrimeBinary`.
- `workbench/src/substrate-inventory.test.ts` (new) — 11 integrity tests: REQ-PRIME-008 + trigger-record coverage, live-function cross-check of the model layer, bounded-claim checks, no-canonical-truth (runtime + type-level), digest verification.
- `workbench/src/prime-fresh-target.test.ts` (new) — 6 deterministic in-model tests + the 5-leg gated real battery with isolation, qualification gating, and capture support.
- `workbench/src/prime-long-running.ts` (additive only) — `requestSubstrateStrategyWithProvenance` + `RequalifiedSubstrateOutcome` (refusal unchanged, provenance attached); existing exports byte-stable in behavior.
- `evidence/T24/` — this file, `CAPTURES.sha256`, `captures/` (16 hash-pinned artifacts).

No commits made; no frozen documents, prereg, status anchors, justfile recipes, or other tasks' files touched. No new dependencies. TypeScript strict clean; the full workbench suite is green with this task's suites included.

## What the fresh-target confirmer should attack

1. **Re-run the battery cold** (`GAUNTLET_T24_REAL_PRIME=1 just tui-test prime-fresh-target`) on a clean `/tmp` — prereg transfer test L1 (replay): same outcomes, no state carryover (the battery asserts its own isolation; a stale `t24-real-*` root or leftover daemon should make it FAIL, not pass).
2. **Parametric variation (prereg L2)**: nested/parallel children at `RecursionPolicy` depth/budget bounds; kill timing variations on leg (d) (kill mid-turn vs mid-boot); second `-c` resume chains.
3. **The map's bounded claims**: try to find a CLI surface for heartbeat/compaction/refinement/harness_state beyond the probed corpus (e.g., config-file keys, extension commands) — a found surface must be added as a row change with a capture, not silently absorbed.
4. **Identity-space binding**: attack leg (a)'s "3 distinct durable files" claim with concurrent SAME-second runs (UUIDv7 collision space) and with `--session-dir` reuse across runs (stale files from a crashed run would break the `filesBefore` delta).
5. **The fork-free chain check** under heavier concurrency (5+ simultaneous sends, steer-mode deliveries, `--follow-up` once/if upstream fixes the parser drift).
6. **The drift findings**: verify `send --steer/--follow-up` refusal and `schedule list/cancel` socket binding against any updated prime-agent build; a fixed build flips those rows and is a requalification signal (prereg R1 direction), not a regression.
7. **No-canonical-stop posture**: attempt any workbench path that terminates real Prime work (there must be none; the map's `session_stop` row records the substrate CAN stop — the workbench must still refuse to expose it).

## Confirmation findings

Fresh-target verification returned CONFIRM_PRIME_REPL_LIFECYCLE with one MINOR correction applied post-verdict: `QUALIFIED_PRIME_BINARY.launcher_sha256` (and this evidence's Qualified-target line) had transcribed the launcher digest incorrectly (`…f7c3fd04…` vs the recomputed `38e3475ad4a1772155f709586f60ef6ad1214d4c05f4776d04611b0041ce530e`); both now carry the recomputed digest, with binary identity independently confirmed by version 0.8.0, buildId `8d7deea-dirty`, and the pinned `--help` corpus (file unchanged since Aug 14 — the correction is not load-bearing).

Confirmation: BUILDER evidence pending fresh-target reproduction confirmation.
