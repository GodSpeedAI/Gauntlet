# Task 15 — blind DESIGN review (pre-implementation)

Critic: fresh-context design review under `comparison_record@v1`.
Reviewed artifact: `.agents/reports/task15/recon.md` (the design), judged
against the frozen contracts only. No builder history received; none exists
in this context. Read-only round: no cargo/just executed; no file modified
except this report.

## Envelope actually used

- Design: `.agents/reports/task15/recon.md` (all sections).
- Plan: `.agents/plans/gauntlet-impl.md` lines 498–521 (Task 15), 525–546
  (Task 16 context), 549–571 (Task 17 context).
- Spec: `.agents/specs/Core-spec.md` §6.1 (408–438), §10.3 (738–755),
  §10.5 (779–794), §8.5 (584–597), §21.2 (1221–1227), Annex C.1–C.4
  (1991–2037).
- Frozen code (read, not run): `crates/gauntlet-ports/src/contract/agent_runner.rs`,
  `crates/gauntlet-ports/src/agent_runner.rs`,
  `crates/gauntlet-ports/src/role_result.rs` (API + `read_result_contract`),
  `crates/adapters/gauntlet-adapter-agent-mock/tests/contract_suite.rs`,
  `crates/gauntlet-app/src/doctor/checks.rs` (checks 3/4/5 regions),
  `crates/gauntlet-arch-tests/src/denylist.rs` and `src/lib.rs`/`src/literals.rs`
  (rule scopes).
- `.agents/reports/task14/closure.md` §4 residual tables;
  `.agents/research/preregistrations/task-15.prereg.yaml` (frozen);
  `.agents/CURRENT_STATUS.yml` R-1b lines (grep-level).

---

## Residuals

### RES-D1 — invoke's return point is the parent print-run, not handle-existence (D1's second end)

- dimension: functional
- severity: **blocking**
- locus: recon §4 transport shape (lines 96–103) and §8 risk bounding
  (lines 188–190) vs plan `.agents/plans/gauntlet-impl.md:506` ("capture …
  into the handle, and **return immediately**"), spec §6.1:436 ("`invoke`
  MUST return promptly after admission"), prereg D1 second end
  (task-15.prereg.yaml:62–69: "never wait for the parent print-run to
  outlive admission").
- defect: The design's production transport reads, in order: submit headless
  parent run → parent model executes the cell → handle JSON lands at
  HANDLE_PATH → adapter parses and returns. The only completion events the
  design names are parent-run events, and §8 equates "admission latency"
  with "the `-p` parent round-trip" to be bounded by a timeout — i.e. invoke
  waits for the parent run (bounded), and admission is defined as the
  round-trip's end. The parent print-run outlives the cell (admission) by
  definition; a parent model that rambles after the cell — or chooses to
  check on / wait for the child, which the harness permits — makes invoke's
  latency unbounded and, in the waiting-parent case, makes invoke block on
  child completion in production. That is the exact AK-009/D1 shape the task
  exists to forbid, and the prereg itself predicts the temptation (P1, R1).
  The suite over doubles cannot catch it: doubles resolve instantly, so the
  production wait-for-parent shape passes every suite case while violating
  the contract on the real harness.
- reproduction/consequence: Implement §4 as literally written; dispatch a
  spec whose parent model executes the cell then polls
  `rlm.list_subagents()` until the child replies before exiting its print
  run. `invoke` now returns only after the child finished — admission and
  completion collapse into one blocking call; fan-in polling (§10.4) is
  dead code on the happy path, and the deadline in the spec never governs
  admission.
- correction demanded (design-level, cheap): pin invoke's return condition
  to **parsed handle existence** (bounded watch of HANDLE_PATH), re-home the
  timeout to that watch (not the parent process lifetime), and specify
  parent-run disposition after admission (leave detached vs reap — see
  RES-D4). Require the env-gated live probe to evidence the D1 split:
  HANDLE_PATH observed (or invoke returned) while the parent run is still in
  flight, and a poll reaching `Completed` only after the parent run has
  exited — which also verifies the daemon-survival fact (§3) the early-return
  shape depends on, before the transport shape is trusted at the gate.

### RES-D2 — poll is described stateless; a late-writing child resurrects a TimedOut invocation

- dimension: functional
- severity: major
- locus: recon §4 poll classification (lines 104–111) vs frozen port
  semantics `crates/gauntlet-ports/src/agent_runner.rs:415–421` ("once
  terminal … every later `poll` returns the SAME terminal status") and suite
  case `terminal_state_reported_exactly_once`
  (`contract/agent_runner.rs:291–377`); prereg L3 transfer test
  (task-15.prereg.yaml:294–302) predicts a post-deadline writer is refused.
- defect: The design's classification is a pure function of (file state,
  deadline). That satisfies first-transition semantics but not post-terminal
  stability: after `TimedOut` + cancel, a child that lost the cancel race and
  writes a valid, matching document makes the next poll report `Completed` —
  a second terminal transition (resurrection). No suite case scripts
  late-write-after-timeout (the suite's scripts are enumerated at
  `contract/agent_runner.rs:87–110`; none covers it), so the suite-over-
  doubles run is silent on exactly the prereg's L3 attack.
- reproduction/consequence: Double (or live child) writes a schema-valid,
  identity-matching document one tick after the deadline expiry verdict;
  poll sequence `Running → TimedOut → Completed`. Settlement machinery fed
  from a resurrected invocation contradicts §16 and the port's pinned
  semantics.
- correction demanded: add one design sentence: the adapter persists the
  first terminal verdict per admitted `invocation_id` (adapter-local control
  state — permitted; I11 forbids only session_dir as control state) and every
  later poll re-reports it verbatim.

### RES-D3 — filesystem anchor seams unspecified: opaque ResultContractPath, ENVELOPE_PATH/HANDLE_PATH ownership, parent cwd from WorkspaceRef

- dimension: architecture
- severity: major
- locus: recon §4 (lines 92–103, 104–115) vs §10.3 rule 1
  (`Core-spec.md:748`: "The runtime creates the invocation directory and
  passes the path in the prompt and the environment"), prereg P5 second half
  (task-15.prereg.yaml:215–223).
- defect: The suite dispatches `ResultContractPath::new("suite-result-document")`
  (`contract/agent_runner.rs:156–157`) — an opaque token, not a path. The
  design never states the rule that turns the token into a real filesystem
  location, who creates the invocation directory, where ENVELOPE_PATH and
  HANDLE_PATH live, or how `workspace: WorkspaceRef` ("ws-suite-01" in the
  suite) becomes the parent print-run's cwd. These are exactly the decisions
  the prereg says must surface as recorded decisions, and the workspace
  resolution half is R-1b's own subject — an adapter that silently invents
  workspace→path resolution reaches past the recorded R-1b trigger. The
  suite-over-doubles only works if the double and the adapter share ONE
  resolution rule; leaving it unpinned invites a suite that passes on a
  resolution production never uses.
- reproduction/consequence: Two implementers reading §4 produce adapters
  that resolve the contract path differently (state-dir join vs env var vs
  cwd-relative); both pass the unmodified suite (the double matches whichever
  rule its own factory encodes) while only one matches §10.3 rule 1.
- correction demanded: pin one resolution rule before implementation — e.g.
  real path = configured state-dir root ⊕ contract token (canonical §10.3
  shape), invoke creates the invocation directory, envelope/handle files live
  in the same invocation directory, and the parent cwd / probe roots are
  injected typed inputs from the composition root (production resolution
  stays unreachable this task; suite and probe inject temp roots). Record it.

### RES-D4 — crash window between child spawn and handle write yields an admitted-but-untracked daemon child

- dimension: functional
- severity: major
- locus: recon §3 (children are daemon-backed, line 77–78) + §4 invoke shape
  vs Annex C.2 (list_subagents is liveness/forensics, `Core-spec.md:2027`),
  §17.4 budget governance, §21.2 error discipline (`Core-spec.md:1221–1227`).
- defect: The handle file is written by model-executed code as the cell's
  last statement; a parent run killed/refused/interrupted between child
  spawn and handle write leaves a live, daemon-backed child that the runtime
  can never poll, cancel, or budget (invoke has no handle to return; the
  caller got an error). The child runs outside `BudgetMeter` accounting
  until daemon idle-unload, and its eventual result document is orphaned.
  The design names no recovery sweep, and does not classify invoke-time
  failures (the port offers `AdmissionRefused` vs `Transport`,
  `agent_runner.rs:39–73`; the design assigns neither, and the §8
  "honest probe failure" language covers only the probe, not production
  invoke).
- reproduction/consequence: SIGKILL the parent print-run mid-cell (or the
  model refuses after starting the cell); `prime-agent list` shows a live
  child; no handle exists; spend accrues unaccounted; retry re-dispatches a
  second child for the same slice — doubled model cost, no ledger entry.
- correction demanded: specify (a) invoke failure taxonomy
  (handle absent/invalid ⇒ `AdmissionRefused`; transport/binary failure ⇒
  `Transport`); (b) a best-effort orphan sweep on admission failure — consult
  `list_subagents()` by the deterministic invocation name and `stop` any
  discovered child before returning the error, recording the sweep outcome in
  the error detail.

### RES-D5 — the kernel-transport outcome type is unpinned; D1's unrepresentability is currently conventional, not type-level

- dimension: architecture
- severity: major
- locus: recon §4 seams (lines 117–127: "submit a rendered program, observe
  the admission outcome") vs prereg D1/R1 (task-15.prereg.yaml:60–69,
  226–239) and plan done-when:519 ("no code path awaits a value from the
  delegation call").
- defect: The design names the seam but not its success payload type. If the
  trait returns raw program output, an unstructured value, or a String, then
  "the child answer cannot flow through the delegation call" is enforced only
  by HANDLE_PATH content discipline plus the A5 grep — textual, not
  structural. The prereg's claim (A2: strict-admission of harness output) is
  also unstated for the handle decode specifically: strict decode of the
  four-field handle JSON is what stops an answer being smuggled through
  HANDLE_PATH itself. Related hazard: the rendered kernel program legitimately
  contains `await rlm(...)` (Annex C.1 — awaiting *admission*, upstream API
  shape); a naive await-ban grep over adapter sources self-flags the
  template, tempting a weakened grep at implementation time.
- reproduction/consequence: Implement the transport as
  `fn submit(&self, program: &str) -> Result<String, String>`; the grep passes,
  the suite passes, and nothing in the type system prevents a later
  "convenience" from parsing a child reply out of the transport outcome.
- correction demanded: pin the kernel-transport trait's outcome to a struct
  carrying exactly `rlm_child_id`/`name`/`session_dir`/`model`, strict-decoded
  from HANDLE_PATH (unknown fields rejected), with typed transport errors;
  specify the arch grep's subject (Rust-side awaits on delegation outcomes /
  answer-shaped channels), its negative control (A5), and its exemption for
  the kernel template's admission-await.

### RES-D6 — capabilities: `supports_streaming_events` / `supports_model_selection` values and `model_hint` disposition are undecided

- dimension: functional
- severity: major
- locus: recon §1 step 3 (lines 15–18) vs spec §6.1 `RunnerCapabilities`
  (`Core-spec.md:425–432`, "capabilities() drives scheduling decisions",
  :438) and prereg D5 (task-15.prereg.yaml:99–107).
- defect: The design states isolation/result_delivery/native_subagents/
  max_depth but is silent on the two boolean fields the port requires
  (`agent_runner.rs:192–200`). D5 (frozen) pins `supports_streaming_events:
  false` with the print-mode rationale — the design document itself never
  says it. `supports_model_selection` is genuinely open: Annex C.1's program
  passes no model to `rlm(...)`, so `model_hint` would be silently dropped —
  an adapter reporting `true` while dropping hints is a dishonest capability
  that misdirects scheduling, and dropping without recording contradicts
  §21.2. The suite can only check stability and token parse-back
  (`contract/agent_runner.rs:748–800`), never honesty of values, so this
  must be decided at design time.
- reproduction/consequence: Implementer sees `handle.model` exists upstream
  and reports `supports_model_selection: true`; scheduler assigns models via
  hints; hints never reach `rlm(...)`; runs silently execute on the default
  model while evidence claims otherwise.
- correction demanded: declare all six capability values in the design
  (streaming false per D5; model_selection true only if the rendered program
  forwards the hint to `rlm`, else false) and state the `model_hint`
  disposition (forwarded, or dropped-and-recorded).

### RES-D7 — doctor runner descriptor: token-match half and fail-closed semantics not carried over from the BackendDescriptor precedent

- dimension: architecture
- severity: minor
- locus: recon §5 (lines 137–149) vs `crates/gauntlet-app/src/doctor/checks.rs:724–736`
  (descriptor `token` "must equal the resolved … or the descriptor describes
  a different backend") and `:770–802` (None / token-mismatch / unmeasured ⇒
  Fail, never Unavailable).
- defect: The design says check 3 reports the measured probe result and
  check 4's runner half is "fed from the measured capabilities", but omits
  (a) the token-match requirement — a composition-root bug could feed prime's
  measurement while config resolves mock/cli, and check 3 would pass on the
  wrong measurement; (b) the explicit rule that a failed probe ⇒ Fail on both
  the check 3 resolution half and the check 4 runner half (the current code
  marks both halves `Unavailable` inside a Pass combine — `checks.rs:697–706`,
  `:849–857` — so "measured failure ⇒ non-zero exit" is a semantic change
  that must be stated, not implied); (c) labeling the measurement's scope
  (harness API-surface version compatibility only) so a passing check does
  not visually promise daemon/provider readiness (prereg D4).
- reproduction/consequence: swap the descriptor wiring in the composition
  root; check 3 stays green while describing the wrong adapter.
- correction demanded: mirror the precedent verbatim for the runner
  descriptor (token, measured, note), state the Fail-not-Unavailable rule for
  a failed probe, and name the measured dimension in the evidence text.

### RES-D8 — home of the probe's supported-range constant (and marker/bound constants) undecided; marker probe adds an unspecified failure mode

- dimension: architecture
- severity: minor
- locus: recon §5 (lines 129–136) and §7 defaults note (line 183) vs
  §8.1 literal policy / `crates/gauntlet-arch-tests/src/literals.rs` (scan
  scope: the four core crates) and prereg P5 (task-15.prereg.yaml:215–223).
- defect: The design says "a supported-range constant" without saying where
  it lives. Scanner scope makes adapter-crate literals legal (adapters are
  outside the literals/denylist scan per `arch-tests/src/lib.rs:62–63`), but
  the repo's own convention routes configuration defaults through
  `gauntlet-app/src/config/defaults*` and the composition root — and the
  prereg demands the decision be recorded, not invented mid-implementation.
  The "bounded presence probe of Annex-C surface markers" additionally (a)
  adds a fourth probe failure mode absent from the design's typed list
  (binary absent / unparseable / outside range), with unspecified pass/fail
  semantics, and (b) couples the adapter to package-internal install layout
  the design itself declares non-durable ("the durable contract is the
  versioned range").
- reproduction/consequence: implementer hardcodes the range in the adapter
  and a marker list of internal paths; a packaging change upstream flips the
  marker probe to failure with no decision record explaining its authority.
- correction demanded: record the constant home (recommended: typed probe
  configuration injected by the composition root from the defaults module)
  and either specify the marker probe's failure semantics or drop the marker
  probe in favor of the versioned range the design already calls durable.

### RES-D9 — plan gate line cannot run as written; the substitution must be recorded at settlement

- dimension: evidence
- severity: minor
- locus: recon §1 gate note (lines 27–33) vs plan:515–517
  (`cargo nextest run -p gauntlet-ports contract::agent_runner -- --adapter prime`).
- defect: The design correctly recognizes the `--adapter` selection flag is
  Task 16's step 3 and substitutes the established factory pattern (verified:
  `crates/adapters/gauntlet-adapter-agent-mock/tests/contract_suite.rs:18–24`
  calls `agent_runner_suite(&MockRunnerFactory, ProbeBudget::INSTANT)` and
  asserts empty findings). Correct reading — but the divergence from the
  plan's literal gate command must be carried into the closure record as an
  auditable plan-defect reading, otherwise the gate evidence and the plan
  text disagree without a recorded reason (and the failure mode the design
  must avoid — editing the frozen suite to add the flag early — stays only
  implicitly forbidden).
- reproduction/consequence: settlement reviewer runs the plan's literal gate
  line; it fails; no record explains why the actual evidence is stronger.
- correction demanded: the closure record states the substitution verbatim
  with the mock-pattern precedent, and the gate evidence shows the
  adapter-crate factory test plus the unmodified suite source untouched
  (git diff empty on `crates/gauntlet-ports/src/contract/agent_runner.rs`).

### RES-D10 — fact-table locus error: mock contract-suite path is wrong in §7

- dimension: evidence
- severity: minor
- locus: recon §7 table line 179
  (`crates/adapters/adapter-agent-mock/tests/contract_suite.rs:18`) vs the
  actual `crates/adapters/gauntlet-adapter-agent-mock/tests/contract_suite.rs:18`
  (the crate prefix `gauntlet-adapter-` is dropped; §1 line 31 spells it
  correctly, so the document is internally inconsistent).
- defect: a builder sent to the cited path finds no such directory.
- reproduction/consequence: `ls crates/adapters/adapter-agent-mock` →
  nonexistent.
- correction demanded: fix the table row.

### RES-D11 — design claims Annex C "adapter rules" but the role-installation/verification rows are Task 17's; the deferral is unrecorded

- dimension: evidence
- severity: minor
- locus: recon §1 line 36–37 ("Annex C (kernel program shape, adapter rules,
  caveats)") vs Annex C.2 rows 5 (`Core-spec.md:2025`) and C.3
  (`:2029–2031`) vs plan Task 17 (`:549–571`, roles install/verify + the
  advisory inline-prompt fallback at `:571`).
- defect: Task 15's kernel program inlines ROLE_PROMPT per Annex C.1 — which
  is exactly Task 17's redesign-trigger fallback ("injecting the role body
  into each invocation's prompt … `enforcement: advisory`"). Legitimate, but
  the design claims the C.2 "adapter rules" surface while deferring the
  install/verify rows without recording the deferral (UNK-003 and AK-013 get
  recorded in §2; this one does not). An unrecorded deferral is a leak by the
  closure's own standard (`task14/closure.md:269`).
- reproduction/consequence: a later reader of the Task-15 closure believes
  harness role installation and drift verification are handled; they arrive
  only with Task 17.
- correction demanded: narrow the §1 claim to the C.2 rows this task
  implements (never-await, poll-contract-not-mailbox, session_ref-only,
  isolation-none, list_subagents-as-forensics) and record the
  install/verify row as Task-17-owned with advisory inline-prompt semantics
  in the meantime.

### RES-D12 — `Running{last_event_at, payment_so_far}` sourcing unspecified for a file-only surface

- dimension: functional
- severity: minor
- locus: recon §4 poll classification vs port `agent_runner.rs:151–156`
  (Running carries `last_event_at`, `payment_so_far`) and spec §6.1:423.
- defect: The design's poll reads only the result contract; it never says
  what fills the two Running payload fields. The child's spend is not
  visible to the adapter mid-flight, so an honest value must be defined
  (e.g. last_event_at = last observed contract-path state change;
  payment_so_far = the reserved slice boundary or zero with an explicit
  "not observable through this surface" note) — otherwise implementation
  invents values that later evidence treats as measurements.
- reproduction/consequence: implementer reports accumulated wall-clock or
  fabricated zero spend; BudgetMeter reconciliation (§17.4) trusts it.
- correction demanded: one recorded decision naming the honest sources for
  both fields.

---

## Verified clean (design elements checked against contract, no residual)

1. **Poll classification map — first-transition semantics.** Mailbox reply
   without a valid result file ⇒ Running, never Completed (§4:107–109) =
   §10.3 rule 2 + §10.5 result-delivery row (`Core-spec.md:787`). Malformed ⇒
   `Failed{malformed_result}`, never collected; identity mismatch ⇒
   `Failed{contract_mismatch}`; Completed only on strict-decode +
   `validate_against` via the ports-canonical `read_result_contract`
   (role_result.rs:983–996) = §10.3 rules 3/4/6. Deadline expiry ⇒ TimedOut +
   cancel preserving `session_ref` = rule 5. Annex C.4 no-process caveat:
   liveness is forensics, "never a state machine input" (§4:109–111) =
   `Core-spec.md:2035`. (Late-writer stability is RES-D2; first-transition
   coverage itself is complete.)
2. **R-1b boundary held.** No builder executed; shared suite over transport
   doubles spawns nothing by default; the env-gated live probe spawns only a
   trivial file-writing child, dispatched for no Unit, through no
   AuthorityGate/GovernDeps, exercising no workspace composition; the
   controller dispatch path stays mock-fed (§6:151–169). Matches the
   CURRENT_STATUS hard trigger (CURRENT_STATUS.yml:83–91), closure §4
   R-1/R-2/R-3 rows (closure.md:275–277), prereg D3/R4, and the assignment's
   boundary verbatim.
3. **Fenced dispositions respected.** AK-013/P8 left to Task 16 (§2:47–48);
   UNK-003 not forced (§2:49–50; doctor check 3 still Skipped-reasons
   UNK-003, checks.rs:641–651); R-1b/R-2/R-3 restated open with recorded
   triggers; the harness's presence treated as observed recon fact with the
   §4 transport shape as design, not as an existence claim (§3:54–56).
4. **Suite consumed unmodified.** Factory + `RunnerScript` + `ProbeBudget`
   parameterization mirrors the frozen suite's own design
   (`contract/agent_runner.rs:12–17, 114–120, 809–825`) and the mock's
   established integration pattern; the design explicitly forbids editing the
   suite and defers `--adapter` to Task 16 (§1:27–33, §4:126–127). No design
   element forces a port, domain, or suite modification. (Gate-line recording
   gap is RES-D9.)
5. **Honest isolation and delivery.** `isolation: none` unless externally
   wrapped, `result_delivery: file` (§1:15–18) = Annex C.2 trust row
   (`Core-spec.md:792, 2024`) and plan step 3; `max_depth` from
   configuration = C.2 depth row. Prereg D5's streaming=false is consistent
   with the design (though unstated in it — RES-D6).
6. **session_ref discipline.** `session_dir` → handle `session_ref` only;
   cancel/timeout preserves it for forensics (§1:14–15, §4:107–108) = I11,
   C.2 row 3, §10.5 persistence row; matches `InvocationHandle`
   (agent_runner.rs:276–301).
7. **Doctor surfacing direction.** Composition root measures; `gauntlet-app`
   receives typed descriptors; only the CLI composition root constructs the
   adapter; no adapter identifiers inward — consistent with the denylist's
   derived adapter-identifier rule over domain/ports/application
   (denylist.rs:199–225, 291–301; anticipated crate names at :619–621) and
   the BackendDescriptor precedent (checks.rs:716–736). Probe mismatch ⇒
   check failure, not runtime surprise = plan step 4. (Pattern-completeness
   gap is RES-D7.)
8. **Literal policy compliance.** The binary is configured, never a literal
   (§4:119–120, §5:131); no model ids appear in any code-facing surface;
   recon's absolute paths are report prose, not design constants; the
   adapter crate sits outside the literals/denylist scan scope
   (arch-tests/src/lib.rs:62–63) while gauntlet-app receives only typed
   values. (Constant-home decision owed is RES-D8.)
9. **Untrusted-input discipline at both seams.** Handle JSON parsed,
   validated, admitted (prereg A2, reflected in §4:100–102); child result
   admitted only through the ports strict codec = §10.3 rule 4 + §21.2.
10. **Prereg conformance otherwise.** No contradiction found with D2 (no new
    suite cases owed or proposed), D4 (probe names the API surface only;
    provider nonclaim held in §8's honest carve-out language "recorded,
    never faked" = plan:521), the explicit nonclaims (isolation none, no
    composition, doubles-vs-live evidence separation), or the design's own
    blindspots (model-backed admission, daemon/socket unknown, exact child
    bytes unverified — all carried, none silently resolved).

---

## Not examined

- `.agents/reports/task15/b1-progress.md` or any builder working notes —
  excluded by contract; none were read.
- Any implementation code beyond the frozen port/suite files, the mock
  integration test, the doctor check 3/4/5 regions, and the arch-test rule
  scopes named in the envelope. Absence of implementation is per contract
  not a finding.
- The upstream prime-agent package itself (`~/.cache/nub/...`) — outside the
  envelope; recon's upstream facts (rlm handle fields, mailbox semantics,
  daemon survival, CLI control family) were assessed only as design
  commitments, not re-verified against the vendor tree. Daemon survival of
  children after parent exit — load-bearing for RES-D1's fix — is registered
  in the design's own blindspots and must be proven by the live probe.
- `.agents/CURRENT_STATUS.yml` beyond the R-1b lines; task14 closure outside
  §4's residual tables; the full constant tables of `literals.rs`.
- No cargo/just/rust-analyzer execution (builder holds the compile lock);
  every conclusion above is from static reading of the enumerated sources.
