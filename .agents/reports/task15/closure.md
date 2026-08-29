# Task 15 — closure record

**Task 15 — Prime-Agent adapter** *(phase 4)*. First real (non-mock)
`AgentRunner`: delegation through the prime-agent harness's native recursive
sub-agent mechanism, fan-in through the result contract.

**Status: SETTLED (CONFIRM).** One pre-implementation blind design review
(NOT_CONFIRM, all 12 findings accepted and corrected before B1 completed) and
one fresh blind EXECUTING falsifier round (CONFIRM: 0 blocking; 2 major
missing-teeth findings corrected by COR-D3 and their teeth re-verified
personally; 1 minor, corrected in the same round). Every prediction-bearing
loop is closed; every residual below has an explicit trigger.

Provisional, not settled: the ruler's owner has not settled any
locator-materialization candidates. No new reference-candidate entry is owed
by this task — every frozen locator in play (`tests/contract/agent_runner::*`)
materialized and passes for a second, real adapter.

---

## 1. What Task 15 delivered, per plan step

| Plan step | Delivery |
|---|---|
| 1. Kernel-program adapter | `crates/adapters/gauntlet-adapter-agent-prime/` (~3.4k lines with tests): Annex C.1 renderer byte-pinned by round-trip tests (model kwarg forwarded when hinted; atomic tmp+rename result-contract instruction embedded in the child prompt; hostile-input escaping tested); `KernelTransport` seam whose success payload `AdmissionHandle` carries EXACTLY the four admission fields (arch-scanned) strict-decoded from the handle FILE — a child answer is unrepresentable; `HarnessControl` seam (`stop`/`list`, forensics-only). Production impls: std-only process spawning. |
| 2. Poll via result contract | `PrimeAgentRunner::poll` is a ONE-input state machine: the contract file via the ports strict codec. Mailbox never read; liveness never consulted (Annex C.4); deadline owned by the spec field; TimedOut ⇒ best-effort stop preserving `session_ref`; post-terminal verdicts pinned and re-reported verbatim (late writers cannot resurrect); malformed ⇒ `Failed{malformed_result}`; identity mismatch ⇒ `Failed{contract_mismatch}`; unreadable file ⇒ typed `Transport`, uncertainty never becomes a verdict. |
| 3. Honest capabilities | native_subagents true, streaming_events false (print-mode surface), model_selection true (hint forwarded into the `rlm(...)` call), isolation **None** (Annex C.2 trust row — the scheduler can refuse this adapter), delivery File, max_depth from config. |
| 4. Version probe | `probe.rs`: bounded `--version`, strict semver parse (v-prefix tolerated), range match against a typed `VersionRange` INJECTED from the app defaults module (RES-D8 — no literal in the adapter, no install-layout probe). Typed configuration-class failures; doctor surfaces them via the measured-descriptor pattern (check 3 resolution half + check 4 runner half; failed probe ⇒ Fail, never Unavailable-inside-Pass; mistokened descriptor ⇒ `runner_resolution_mismatch`). |
| 5. Unmodified contract suite | `tests/contract_suite.rs` binds a script-following factory over in-memory doubles; `agent_runner_suite(...)` returns ZERO findings. The frozen suite is byte-identical to git HEAD (verified independently by orchestrator AND falsifier). AK-009..AK-012 re-evidenced on a second, real adapter; AK-008 evidenced on a REAL process by the live probe's D1 forensics. |

**Supporting surface:** arch await-ban scan (`gauntlet-arch-tests/src/delegation.rs`,
rules A/B/C with planted negative controls; documented exemption for the kernel
template's upstream admission-await); doctor check-3/4 runner descriptor wiring
(`RunnerDescriptor`, BackendDescriptor-mirror; composition root measures,
app receives typed input); env-gated live probe (`GAUNTLET_PRIME_LIVE_PROBE=1`)
asserting the full real-harness leg.

## 2. Gates — and who ran them

**Final tree, run personally by the orchestrator, 2026-08-29:**

| Gate | Result |
|---|---|
| `just test` (workspace) | **1123 run, 1123 passed, 0 skipped** |
| `just check` | PASS |
| `just lint` | PASS (clippy `-D warnings` + fmt) |
| `just boundaries` | **220/220** |
| `just deny` | advisories ok, bans ok, licenses ok, sources ok |
| `just doctor` | exit OK |
| `just ruler` | PASS — digests match the frozen lock |
| `just prereg-check 15` | PASS — frozen hash `61a0841d…` |
| `git diff` on `crates/gauntlet-ports/` | EMPTY (suite byte-identity, RES-D9 evidence) |
| live probe (`GAUNTLET_PRIME_LIVE_PROBE=1`) | **PASS** (35.4 s orchestrator; 40.4 s falsifier — one real admission each, D1-split forensics asserted) |

**Teeth, re-verified personally (orchestrator, not on any report's say-so):**
the falsifier's mutations M2 (handle unknown-field refusal → silent ignore)
and M7 (cancel terminal no-op disabled → second stop) were re-applied by hand:
M2 → `handle_decode_teeth` 2 RED; M7 → `cancel_issues_exactly_one_stop_ever`
RED; both restored byte-identically (sha256-diff empty against pre-mutation
baseline); 34/34 GREEN after restore.

## 3. Confirmation tracks and corrections

| Track | Verdict | Substance |
|---|---|---|
| Blind design critic (pre-implementation) | **NOT_CONFIRM** (1 blocking / 5 major / 6 minor) | RES-D1 blocking: the recon's invoke waited on the parent print-run — on a rambling/waiting parent model, admission collapses into child completion, invisible to the suite (doubles resolve instantly). Plus: post-terminal resurrection, unpinned filesystem anchors, orphan-child crash window, unpinned transport payload type, undecided capability values, doctor pattern gaps, probe-constant home, gate-line recording, recon typo, unrecorded Task-17 deferral, Running-payload sourcing. ALL 12 accepted; corrections routed to the builder MID-FLIGHT; recon corrected in place. |
| Blind executing falsifier (round 1) | **CONFIRM** (0 blocking / 2 major / 1 minor) | 13 executable attacks, ALL REPELLED (late-writer resurrection, deadline ownership, mailbox-ignored, liveness-ignored, 16 hostile handle documents, answer-shaped field end-to-end, leaf-anchor traversal, duplicate admission, cancel semantics + effect counting, capabilities honesty scan, classification whole-document, unreadable contract, collect locus). 7 hand-targeted mechanism mutations, all reverted byte-identically. Live probe re-run independently: PASS. Hygiene: 191-file sha256 baseline diff EMPTY; doctor OK; ruler PASS; zero strays. Findings: F1/F2 missing in-tree teeth, F3 coverage nuance. |

**Corrections at gate phase (all executed against the real harness,
never faked):**

| Id | Substance |
|---|---|
| COR-D1 | `--version` prints to **stderr** on this harness (recon's `2>&1` probes masked it). Probe now parses combined streams; typed errors carry both. Found by live-probe round 1 failing honestly, `VersionUnparseable{output: ""}` — fail-closed held. |
| COR-D2 | Three divergences found by executing diagnosis (6 bounded parent probes + 1 real child): (a) `RLMSpawnHandle.session_dir` is a `PosixPath` — `json.dumps` of raw fields raised INSIDE the cell after a successful admission, so the handle never landed; renderer now coerces all four fields with `str()`. (b) A child of a print-mode parent materializes ONLY while the parent lives past admission (no linger → child never appears; 60 s in-cell linger → child completed its work at +15 s): the rendered cell now carries a bounded `admission_linger_seconds` (default 30, evidence-cited); `invoke` STILL returns at handle parse (D1 untouched — the linger is the parent's cell); the success-path disposition is a detached bounded reaper, not an immediate SIGTERM. (c) `list --json` returns `{"sessions": [...]}` (object wrapper), not a bare array; `parse_listing` strictly accepts exactly that shape. |
| COR-D3 | Tests only (zero production changes): F1 teeth — 16 hostile handle documents through `AdmissionHandle::parse`/`read_admission` incl. an answer-shaped unknown field, plus end-to-end invoke refusal; F2 teeth — control-counting double proving exactly ONE stop across cancel + two typed no-ops; F3 teeth — the exact late-writer resurrection shape against the prime adapter. |

**Pre-implementation design corrections (RES-D1..D12):** all recorded with
dispositions in `design-adjudication.md`; the blocking RES-D1 restructure
(handle-watch return, never parent-run end) is load-bearing in the shipped
transport and is what the live probe's D1 forensics evidence.

## 4. Residuals carried forward

Every entry names a trigger.

| Id | Residual | Trigger |
|---|---|---|
| **R-1b (restated, HARD)** | Per-unit workspace resolution in `GovernDeps` — unchanged by this task: task15 shipped a runner and executed NO builder (suite over doubles; the live probe's children are trivial, dispatched for no Unit, through no gate). | **Before any real runner executes a builder** (Task 16's real-process legs, Task 20 `run`, or Task 25 — whichever first composes one). |
| R-2 / R-3 (restated) | Port-level forgeability (`PolicyValidation`/`SandboxedCommand`/`SandboxPolicy`); `wrap_builder_command`'s missing production caller. Neither touched by this task. | Unchanged: ports revision at Tasks 15-16 (R-2 now due at Task 16), composed wrap at Task 20. |
| T15-R1 | The 30 s admission linger is calibrated on ONE host observation (child completed at +15 s under a live parent). A child-creation longer than the linger orphans the child; the orphan sweep is the mitigation, the sweep's real-binary path was exercised only on the failure shape. | Any production dispatch wiring (Task 16/20): re-measure child-creation latency once, move the default only with recorded evidence. |
| T15-R2 | `supports_model_selection: true` is code-verified and suite-evidenced but the live probe ran UNHINTED — model-hint forwarding has never been exercised against the real harness. | First live hinted dispatch (Task 16/25). |
| T15-R3 | Concurrency posture asserted, not proven: doubles are single-threaded, transport forensics are Mutex-guarded, but parallel `invoke`/`poll` against one runner instance is unexercised. | `max_parallel > 1` wiring (Task 20). |
| T15-R4 | `ProcessKernelTransport` race windows (partial handle writes under SIGTERM, reaper vs. natural exit) are code-read + fixture-tested + live-evidenced ONCE each, not systematically attacked. | The task that composes real dispatch (Task 16/20) or any transport touch. |
| T15-R5 | `last_event_at` mtime correctness: only the fallback path (no file yet) was exercised; mtime-derived values unexercised. | First consumer of `Running{last_event_at}` (Task 20 surfaces). |

**Not attacked / scoped out (falsifier §7, recorded for the next round's
benefit):** recovery/resume and `orphaned_by_restart` (Task 16+), workspace
resolution (R-1b), AK-013/P8 (Task 16), BudgetMeter side, observer/comparator
paths, full `cargo-mutants` (Task 24's instrument).

## 5. Bounded claims — what is NOT claimed

1. **AK-008's real-process evidence rests on the live probe's D1 forensics** —
   three successful real admissions (orchestrator ×1, falsifier ×1, plus the
   t6 diagnosis probe), all on ONE host, ONE harness version (0.8.0), ONE
   provider. Not a portability claim.
2. **The suite-over-doubles run is not live-harness evidence**; the live probe
   is not deterministic-battery evidence. They are separate legs, both recorded.
3. **"All attacks repelled" is scoped to the falsifier's declared surface**
   (§7). Four blind rounds on task14 found fresh defects in green code; this
   surface should be assumed to yield to further angles too.
4. **No claim about provider/model quality, child correctness beyond the
   trivial contract-write, or daemon behavior beyond this host's version.**
5. **Unit status is provisional** — the ruler's owner settles locators, not
   this loop.

## 6. Deviations from the plan's Task-15 steps

1. **Gate-line substitution (RES-D9, recorded):** the plan's
   `cargo nextest run -p gauntlet-ports contract::agent_runner -- --adapter
   prime` does not exist as such — the `--adapter` selection flag is Task 16
   step 3. The delivered evidence is the established factory pattern (the
   mock's own precedent): `tests/contract_suite.rs` over the UNMODIFIED
   shared suite, byte-identity proven. Stronger than the plan's line, not
   weaker.
2. **The env-gated live probe is an addition** the plan does not name. It
   emerged from the standing rule that confirmation rounds must include an
   executing role and from the prereg's own L3/transfer commitments; it is
   opt-in, bounded, and costed (one real admission per run), and it is the
   only leg that touches the real harness.
3. **`session_ref` encoding:** the domain `SessionRef` refuses path-rooted
   tokens while a real harness `session_dir` IS a path; the adapter records it
   losslessly as `session-dir:<session_dir>` (documented, reversible,
   forensics-only — I11 compliant).
4. **No sub-agent interruptions this task** (vs 25+ across tasks 3-14): the
   builder ran to completion three times (B1+B2, COR-D2, COR-D3) plus one
   mid-flight design-correction routing. The checkpoint discipline was
   maintained but never needed for recovery.

## 7. Sources

`.agents/reports/task15/`: `recon.md`, `IN-FLIGHT.md`, `critic-design.md`,
`design-adjudication.md`, `b1-progress.md`, `builder-b1.md`,
`falsifier-round1.md`. Also
`.agents/research/preregistrations/task-15.{prereg.yaml,prereg.sha256,outcome.yaml}`
and the installed harness docs read during recon
(`~/.cache/nub/node/24.19.0/lib/node_modules/prime-agent/docs/rlm*.md`).
