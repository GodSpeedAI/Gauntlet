# Task 16 — Closure: generic CLI agent adapter + P8 substitution proof

**SETTLED — CONFIRM.** Rounds: blind design critic (NOT_CONFIRM → 12 findings
all corrected pre-implementation) → builder B1 (+personal battery) →
EXECUTING falsifier round 1 (NOT_CONFIRM → COR-F1..F4, orchestrator-completed
after the dispatched agent died on the sub-agent usage window; degraded
blindness recorded) → fresh blind EXECUTING falsifier round 2 (**CONFIRM**,
0 blocking / 0 in-mechanism major) → COR-R2-1 + COR-R2-2 teeth landed
tests-only, teeth re-verified personally.

## 1. What landed (HEAD a1c15a2 + closure)

- `crates/adapters/gauntlet-adapter-agent-cli` — the second real AgentRunner:
  command template from `GAUNTLET_CLI_AGENT_CMD` parsed into shell-word argv
  (never shell grammar), `{prompt_file}/{workspace}/{result_path}/{model}`
  bound as single verbatim words; `ProcessChildTransport` seam (spawn/poll/
  stop with SIGTERM→bounded-grace→SIGKILL escalation and dedicated reap
  grace); runner state machine with the §10.3 classification total order —
  contract wins (identity + digest), exited-without-valid-result ⇒
  `Failed{missing_result}` decided BEFORE any deadline (shape 2), running-
  past-deadline ⇒ `TimedOut` + stop, else `Running`; terminal pins are
  stable and retire the child (no invocation outlives its verdict with a
  live process); honest capabilities (isolation none, delivery file, no
  native subagents); channels per §10.3 rule 1 (`GAUNTLET_RESULT_PATH` env,
  prompt RESULT-PATH, envelope body sibling).
- Ports revision: `InvocationFailure::MissingResult` + the shape-2 case as a
  SEPARATE exported sibling suite `exit_without_result_suite` (RES-D1) —
  mock and cli factories satisfy it; the settled prime adapter untouched.
- `crates/gauntlet-cli/tests/p8_adapter_substitution.rs` — the FULL shared
  suite over mock + cli factories in ONE loop body (zero per-adapter test
  bodies); selection `GAUNTLET_P8_ADAPTER=mock|cli|both` (default both),
  unknown value fails loudly (never a silently narrowed battery).
- Doctor: the cli runner is MEASURED through `probe_template` (parse AND
  render with synthetic values) via the typed RunnerDescriptor pattern — a
  parse-ok/render-impossible template fails check 3 honestly; config-only
  probes cannot claim a version probe. UNK-003 stays open (no
  `runners.default` selected — the operator's decision).
- Arch teeth: whole-crate vendor-identifier ban on the cli crate;
  delegation decode-locus exclusivity now enforced for BOTH real adapters
  (COR-R2-2); round-1 falsifier batteries retained in-tree as regression
  teeth (`tests/falsifier_round1_{template,live}_attacks.rs`); round-2
  teeth: `tests/probe_template_honesty.rs` (COR-R2-1) + the cli sibling
  scan with three negative controls (COR-R2-2).

## 2. Closure battery (orchestrator, personally, on the closure tree)

ruler PASS (start and end) · check PASS · **test 1189/1189** (1182 +
3 COR-R2-1 + 4 COR-R2-2) · lint clean (clippy -D warnings + fmt) ·
boundaries **228/228** (224 + 4) · deny ok · doctor exit OK ·
p8 gate line PASS · prereg-check 16 PASS (frozen hash untouched).

**Teeth re-verified personally (RED → byte-identical restore, sha256 →
GREEN):** (1) vendor plant in cli `lib.rs` ⇒ boundaries RED, 4 violations
naming lib.rs:102; (2) shape-2 pin removed from `runner.rs` poll ⇒
`the_cli_agent_runner_passes_the_exit_without_result_suite` RED with three
typed findings (verdict became Running + two second-terminal-transition
legs); (3) `probe_template` gutted to parse-only ⇒ EXACTLY
`parse_ok_render_impossible_template_is_refused` RED, other legs green —
the discriminating leg named its own regression in the panic message;
(4) `read_result_contract(` planted in `template.rs` ⇒ COR-R2-2 rule A RED
naming template.rs:294. Pristine digests matched the falsifier's and
builder's recorded values at every step
(794d0618…/421cb654…/1df6737d…).

## 3. Prereg adjudication (honest; frozen prereg f522dbd3…)

- **P1 (renderer interpolates into shell words) — DID NOT FIRE.** The
  grammar parses configuration into argv words; placeholders bind as
  single verbatim words; `argv.len()` assertions pin the shape. Round-1
  A1–A13 and round-2 L3a/L3b (quote-mode binding, metacharacter values)
  all repelled. The design phase took the risk seriously enough that the
  failure class never existed in code.
- **P2 (shape-1/shape-2 wrong in the first implementation) — DID NOT
  FIRE** as a mechanism defect: B1 implemented shape 2 correctly from the
  start (RES-D1 forced the distinction into the design). The distinction
  was proven the other way: the deliberate M2 mutation is caught by the
  shared suite — coverage, not breakage.
- **P3 (falsifier finds a process-lifecycle defect the suite misses) —
  MATCHED, twice.** Round-1 F1 (cancel escalation consumed the whole
  caller bound; a landed SIGKILL with pending reap reported CancelFailed,
  risking re-classification of a deliberate Cancelled into a retryable
  agent failure) and F4 (a terminal pin left the child ALIVE forever with
  cancel unreachable) are exactly the predicted cancel/reap/lifecycle
  family, both invisible to the green battery. Corrected COR-F1/COR-F4;
  round-2 re-attacked the class (C legs, L4/L6) and found nothing further.
- **P4 (suite extension breaks other adapters' factories) — MATCHED, in
  sharper form and EARLIER.** The blind design critic's blocking RES-D1:
  the settled prime adapter's Annex-C.4 one-input poll structurally cannot
  pass a universal process-exit case — "prime stays green" was
  unimplementable as briefed. The case landed as a separate exported
  sibling suite that mock+cli satisfy by design. Nothing ever went RED
  because the case never entered the universal suite — the prediction's
  core (a multi-adapter change discovered before green) fired at design
  level, one message instead of a builder round.
- **P5 (representational gap around selection/prompt-file) — MATCHED.**
  D-dev-1: the plan's literal `--adapter` test-harness flag does not fit
  nextest's arg-passing; selection is `GAUNTLET_P8_ADAPTER` (env,
  fail-closed). RES-D10 (C10): the p8 cli factory's template is a
  constructor-injected test literal, never the operator's process
  environment — the env var selects the battery only. Both recorded as
  decisions, never silent inventions.

### Blindspots

- Prompt-file format (index 0): did not materialize — the trivial probe
  child parsed the real format first try (live probe PASS 0.255 s through
  the REAL spawn path); format kept machine-parseable.
- Real agent CLIs (index 1): did not materialize; the explicit nonclaim
  stands — "the mechanism travels", never "every CLI works".
- WSL2 process semantics (index 2): PARTIAL — signal-escalation timing bit
  (round-1 C-sig ⇒ COR-F1) and hang-after-write retirement (C-hang ⇒
  COR-F4); daemonized-grandchild/process-group exotica remain un-attacked
  (round-2 not-attacked list) — recorded, not hidden.

### Transfer tests

- L1 replay: SUPPORTED — mock and prime suite legs green unchanged;
  shared-suite change additive-only (138 insertions, 0 deletions,
  verified at gate phase).
- L2 parametric: SUPPORTED — p8 over both factories, one loop body, no
  test body branches on the running adapter (L8a–L8e).
- L3 structural: SUPPORTED with one recorded caveat — round-2's novel
  payloads (directory poison, SIGKILL death, valid-impostor post-terminal
  tamper, grammar exotica) were refused by class-based validation, no
  payload blocklists; the directory-poison leg exposed a deliberate
  posture cost (R16-F2 below), recorded rather than special-cased.

### Explicit nonclaims (all restated open)

R-1b (no real builder executed — first composed builder execution still
hard-triggers task20), R-2 (this adapter consumes a command TEMPLATE, never
a wrapped SandboxedCommand — D3 fog decision; trigger stays on the first
wrap-consuming composition, task20), R-3 (wrap_builder_command gains no
production caller), UNK-003 (runners.default — doctor now MEASURES the cli
runner; the choice stays the operator's), no vendor CLI proven, no
containment claim (isolation none reported honestly).

## 4. Residuals opened (triggers recorded)

- **R16-F2** (minor, liveness posture): a hostile child can poison its own
  contract path (directory at `GAUNTLET_RESULT_PATH`) and stay alive —
  poll returns the typed Transport error forever, `TimedOut` unreachable,
  runner never self-stops; explicit cancel retires it (executed, worked).
  Deliberate §21.2 posture (uncertainty never becomes success). Trigger:
  next transport/lifecycle touch or task25's composed recovery E2E.
- **R16-F3** (minor, diagnostic label): `UnterminatedQuote` offset is a
  CHAR index rendered as "at byte N" (template.rs:88 vs 219/251; executed
  `ää'` ⇒ reports 3, true byte 4). Trigger: next template.rs touch.
- **R16-note-1**: SIGKILL-landed-but-unreapable `Ok(None)` reap-grace
  expiry path pinned only by code reading (unreachable with a normal
  child on this host). Trigger: next transport touch.
- **R16-note-2**: OS pid reuse against the pid-keyed child map — noted,
  not attacked; early-exit check in `stop` closes the practical window.
  Trigger: next transport touch.
- Round-2 not-attacked list (next round's benefit): full mutation suite
  (task24), daemonized grandchildren, real agent binary/model, dispatch-gate
  composition (R-1b), BudgetMeter/observer/comparator surfaces,
  concurrency >2 invocations.

## 5. Deviations (all recorded at the time)

D-dev-1 (env-var selection vs plan's `--adapter` flag), D-dev-2 (fixed-argv
`sh -c` SIGTERM helper — libc/nix ask-first), D-dev-3 (shape-2 as sibling
suite per RES-D1), D-dev-4 (controller disposition row for MissingResult),
D-dev-5 (scanner self-catch during integration), D-dev-6 (instrument
friction recorded not worked around), round-1 falsifier completion by the
orchestrator after the agent's usage-window death (degraded blindness,
attacks composed blind by the fallen agent, fresh floor attacks added),
round-2 falsifier's first restore via `git checkout --` (working-tree only,
digest-verified), 2 transient Agent-dispatch network failures before B2's
third dispatch (pre-launch; no state lost, no double-builder).

## 6. Answer-key bookkeeping

AK-013 (P8 substitution) and AK-014 (second real adapter) now carry test
evidence: provisional through Task 16. AK-013's frozen locator
(`tests/proofs/p8_adapter_substitution`) names a repo-root path where the
repository's proofs live under a crate — locator-class divergence recorded
in reference-candidates (2026-08-29 entry), never by editing the ruler.
R-1b/R-2/R-3 triggers re-recorded for task20; UNK-003 narrowed (doctor
measures the cli runner), still the operator's call.

## 7. Settlement statement

Settlement gates: independent blind confirmation (falsifier round 2,
CONFIRM, fresh context, firewalled, charter verbatim) ✓; proven teeth
(round-1 + round-2 batteries, four teeth re-verified personally) ✓;
prereg recorded with outcome file ✓; full battery green on the closure
tree ✓; ruler PASS before and after ✓. The orchestrator wrote no shipped
code this task: B1 (builder), B2 (builder), design critic, falsifier
rounds 1–2 were all delegated; the orchestrator's hands-on work was
adjudication, the round-1 completion under the standing heal+continue
exception (recorded, degraded), and personal teeth/battery verification.
