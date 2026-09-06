# Task 25 — BLIND design-as-built critic adjudication

Critic: blind design-as-built critic, fresh context, 2026-09-05.
Inputs: `.agents/reports/task25/charter.md`, `.agents/reports/task25/critic-brief.md`,
spec `.agents/specs/Core-spec.md` (§5.3.2, §6.1/6.2, §8.5, §11, §16, §17, §21),
ruler units AK-066/AK-044/AK-047/AK-048 (`.wayfinder/answer-key.md`), the delivered
tree (fixture `tests/fixtures/target-real/`, targets `e2e_full_loop_real_runner`,
`e2e_recovery`, `e2e_budget`, diffs on `compose.rs` / agent-cli `runner.rs`+`lib.rs` /
`improve_cmd.rs` / `compose_teeth.rs`, report `.agents/reports/e2e-real.md`), and the
named prior art. Builder reports were NOT opened (blindness rule held).
Read-only on tracked files: nothing mutated at any point; every cargo/just invocation
ran under its own top-level `flock .agents/reports/task25/compile.lock -c '...'` with
`sccache --stop-server` inside the same flock'd command. No scratch file was planted
anywhere under the repo.

---

## Q1 — AK-066 fidelity (the settleability question)

**Verdict: honest resolution, not a dodged one — with two findings (F1 minor, F2 MAJOR).**

What the happy leg actually proves, unit by unit (all assertions record-derived from
the durable stores/export/escalation surface):

- **AK-951** (contained/critic): dispatches (wrapped — the fixture's live backend
  policy set applies to every tier), completes, verifies PASS via the real test
  instrument, and SETTLES via `ClosingActor::Automatic` — the only closing actor the
  unattended controller ever uses (`controller.rs:3295`, sole call site). The §16.1
  five conditions are enforced inside `SettlementService::settle` (settle/mod.rs
  19–51), not asserted into existence by the test. `status` shows `settled=1`; the
  report classification for AK-951 is `implemented-and-proven`.
- **AK-952** (cross_cutting/lead_agent): ADMITTED and DISPATCHED WRAPPED (see Q2),
  completes, and its closure is REFUSED for the in-loop actor and routed to the
  operator — asserted from the escalation listing (`AK-952` + `lead_agent`,
  e2e_full_loop_real_runner.rs:424-429). It never settles unattended. This is
  §5.3.2's coupling (cross_cutting authority can never be critic), §16.1 condition 3
  as implemented (lead_agent requires the orchestrator route with a recorded
  rationale; the unattended loop does not fabricate one — settle/mod.rs:77-83), and
  the §11.5 row-6 disposition. The bounded re-dispatches exhaust `max_rounds: 4`, so
  the run's terminal is the honest `budget_exhausted` (asserted non-zero exit,
  e2e_full_loop_real_runner.rs:309-317).
- **AK-953** (irreversible/human): never dispatched — asserted from the durable
  export (zero `role_invocation` records naming AK-953, lines 396-402), and its
  escalation ticket is asserted in leg 2 (lines 521-528). AK-044 holds.

The settleability question is answered the honest way: the frozen authority semantics
are pinned as observed (a cross_cutting unit in a seeded reference cannot settle
unattended), the run never claims `settled` for AK-952 or AK-953, and the divergence
from AK-066's literal wording is recorded in the test module docs
(e2e_full_loop_real_runner.rs:286-297), the fixture's own reference
(reference.yaml `desired_state`: "…or the runtime honestly reports the non-settlement"),
and `.agents/reports/e2e-real.md:165-179` for the ruler's owner. The alternatives
were both worse: seeding no cross_cutting unit (dodges the charter's own fixture
requirement) or fabricating an unattended orchestrator-rationale closure
(self-authorizing). Done-when coverage is necessarily thinner than the plan's
Done-when sentence ("every blocking Unit settles" is unattainable here) — disclosed,
not hidden.

**F2 (MAJOR — the proof does not bind the wrap to the cross_cutting unit).**
The wrapped-execution evidence is unit-agnostic:
`store_holds_text_containing(&world.evidence_dir(), "confinement=confined")` and
`…"tmp-write=denied"` (e2e_full_loop_real_runner.rs:360-367) scan the WHOLE
content-addressed store. The AK-951 (contained) child runs under the same live
backend and the same template, so its locator artifact carries the same tokens.
Mutation that keeps leg 1 (and leg 2) green while violating AK-048 for exactly the
tier the real-world leg exists for: make `LiveContainment::wrap`
(crates/gauntlet-cli/src/compose.rs, the `CommandContainment` impl) wrap only the
`Contained` tier and return `Ok(command)` verbatim for `CrossCutting` — AK-952 then
spawns unconfined, its artifact records `confinement=uncontained` (unasserted), the
confined scan still passes on AK-951's artifact, the AK-952 gate-admission record
(lines 411-420) still names tier/policy/`process` because the seam is installed and
the admission still validates, and no assertion fails. The gate-admission assertion
pins the gate's DECISION, not the spawned PROCESS. Remediation (one assertion):
find the stored artifact containing `"unit":"AK-952"` and assert it also carries
`confinement=confined` and `tmp-write=denied` (the artifact production is durable
pre-settlement — the produce-window machinery — so the object exists in this leg).
The run report's AK-066 bullet (e2e-real.md:117-127, "the wrapped child OBSERVED its
own confinement") reads as the cross_cutting child; until F2 is fixed the test only
proves it of some child.

**F1 (minor — test name overclaims).**
`the_composed_loop_settles_every_dispatchable_unit_wrapped_and_escalates_the_irreversible`
(e2e_full_loop_real_runner.rs:301): AK-952 IS dispatchable and does NOT settle; the
test's own body and the durable record (`settled=1`, AK-952 escalated) say otherwise.
Rename (e.g. "…settles the auto-closable unit, wraps the cross_cutting dispatch, and
escalates the rest…"). Flagged because the brief names exactly this class: a test
name claiming "settles" for a unit whose durable record says otherwise.

## Q2 — AK-048 reality (wrap from process start)

**Verdict: the wrap is real, per-dispatch, after admission and before process start;
no unwrapped-under-live-posture path found. Residual binding gap recorded as F2.**

- Seam installed exactly when the posture is live:
  `compose_run` composes `govern_posture` FIRST and installs the SAME
  sandbox+policy instances as the gate into the cli-agent runner for BOTH builder and
  critic (compose.rs:893-916, `LivePosture` shared — "one posture, one set of facts,
  no bypass copy").
- Wrap decided after admission, before any process start and before any filesystem
  effect: in `CliAgentRunner::invoke`, the containment match runs immediately after
  template render and BEFORE the invocation directory/prompt-file writes and
  `transport.spawn` (agent-cli runner.rs, "THE CONTAINMENT WRAP" block); a refusal
  leaves nothing behind.
- No seam-bypass on retry: `invoke` is the only spawn point; every attempt re-enters
  the same match; the runner holds the seam for its lifetime (installed at
  construction, `with_containment`).
- Mock/other runners under a live posture cannot spawn unwrapped above contained:
  prime/recursive reports `Isolation::None` (adapter-agent-prime runner.rs:592), mock
  likewise, and a seam-less cli-agent reports `Isolation::None` — the gate's
  capability-vs-tier conjunct refuses all of them for any tier above contained
  (AK-050's mechanism). Only the seam-equipped cli-agent reports
  `Isolation::Process`, and it physically cannot spawn a policy-carrying dispatch
  unwrapped: `(None, Some(policy))` is a typed `AdmissionRefused`
  (fail-closed arm, agent-cli runner.rs), and `(_, None)` is the historical
  no-policy world spawning verbatim (nothing to claim containment about).
- Resume path: `gauntlet resume` composes through the same `compose_run`
  (run_cmd.rs:147) — the seam is installed on resumed runs identically.
- Improve surface: passes `None` (see Q12) — and a policy-carrying dispatch there
  would be REFUSED by the same fail-closed arm, never unwrapped.
- The "child observes its own confinement" leg is a genuine inside-sandbox
  observation, not a tautology: the bwrap backend mandatorily passes
  `--unshare-pid` ("WITHOUT IT THE SANDBOX IS NOT A PROCESS SANDBOX",
  adapter-sandbox-bwrap backend flags) plus `--ro-bind` baseline and
  `--unshare-net/user/ipc/uts/cgroup`; the child computes
  `confinement=confined` only from pid1==`bwrap` AND an out-of-root write being
  denied (child.sh:130-153). Unconfined on this host, both facts invert (host init
  as pid1; writable /tmp); the report records the discriminator was verified
  empirically unconfined before pinning (e2e-real.md:125-127). The only weakness is
  the unit binding — F2 above.

## Q3 — AK-047 (validate before execute, no unsandboxed fallback)

**Verdict: proven fail-closed at BOTH layers; per-dispatch re-validation is present
in the mechanism, though the E2E leg cannot discriminate it from the
composition-time check (informational).**

- Composition-time: `wrap_checked_posture` still admits + wraps the FIRST planned
  dispatch before any run starts; any refusal is `CompositionError::WrapRefused`
  (compose.rs:1080-1135). The `Sandboxed` verdict now routes instead of refusing —
  the ordered retirement — and only a typed refusal stops the composition.
- Per-dispatch: `LiveContainment::wrap` re-RESOLVES the admitted policy by ref over
  the dispatchable tiers and RE-VALIDATES (`sandbox.validate`) at the spawn boundary
  before wrapping (compose.rs:549-576) — "validation precedes use — AGAIN"; any
  resolution/validation/wrap failure is the typed `ContainmentRefusal`, mapped to
  `AgentError::AdmissionRefused` — the dispatch does not run; there is no arm that
  means "run it unwrapped".
- The real-world leg (leg 2): the cross_cutting policy's backend is rewritten to the
  `none` token IN THE SCRATCH WORLD ONLY (probe hygiene held — repo
  `.gauntlet/policies/` untouched, verified by ls/mtimes), the policy still
  RESOLVES and FAILS backend validation, and the run closes `blocked` with the typed
  escalation ("failed validation" + "never an unsandboxed fallback") and EXACTLY ONE
  durable dispatch (the contained unit's) — a fallback-to-unsandboxed mutation would
  create a second `role_invocation` and fail the count (e2e_full_loop_real_runner.rs:530-543).
- Informational: because the break exists BEFORE composition, leg 2 cannot
  distinguish per-dispatch re-validation from composition-time-only validation (a
  mid-run policy mutation leg through the real binary would be inherently racy).
  The per-dispatch re-validation is pinned by code and by the Sandbox port contract
  ("validation precedes every use", §6.2 — tests/contract/sandbox is AK-047's own
  frozen locator); the real-world leg proves the escalation arm end to end. No
  finding beyond this note.

## Q4 — AK-044 (irreversible never dispatched)

**Verdict: proven from durable records, plus the queue presence. One coverage note
(folded into F9).**

- Absence: every leg scans the durable export for `role_invocation` records naming
  AK-953 and fails if any exists (happy leg e2e_full_loop_real_runner.rs:396-402;
  recovery leg 1 post-resume e2e_recovery.rs:483-490; budget leg
  e2e_budget.rs:430-437 which also asserts no AK-952 dispatch under exhaustion).
  This is record-derived absence, not report wording.
- Queue presence: leg 2 asserts AK-953's ticket names `irreversible` and "never an
  automatic dispatch" (e2e_full_loop_real_runner.rs:521-528); the budget leg
  additionally asserts AK-951's escalation under exhaustion. Coverage note: in the
  HAPPY leg itself the escalation-queue assertion covers AK-952 only; AK-953's
  ticket is proven in leg 2's governance close. The plan Done-when sentence ties
  queue presence to the same run as never-dispatched; delivered across two legs of
  the same target rather than one. Minor.

## Q5 — the production correction (compose.rs + agent-cli)

**Verdict: a genuinely small, typed, ports-clean seam; the recorded authority-ceiling
raise is disclosed and gate-neutral; one exposed real defect shipped unfixed (F3,
MAJOR, disclosed).**

- `LiveContainmentDeferred` is fully retired (enum variant, Display, and firing site
  all removed); the `Sandboxed` arm routes with a comment naming the retirement.
- The seam is a semantic capability, not a leak: `CommandContainment` lives in the
  adapter crate (gauntlet-adapter-agent-cli), is implemented by the composition over
  the Sandbox+PolicyProvider PORTS, references only domain types
  (`WorkspaceRef`, `SandboxPolicyRef`), and core (gauntlet-app) is untouched (git
  status: gate.rs/settle/controller unmodified). `just boundaries` green (Q11).
- Previously-refusing routes still fire: bad template → `AdmissionRefused`
  (unchanged path); capability-vs-tier → the gate conjunct (p5 regression green);
  composition wrap refusal → `CompositionError::WrapRefused` retained. No
  previously-refusing route silently degrades: the ONLY behavioral changes are (a)
  the ordered `Sandboxed`→route retirement and (b) the live-posture exercisable
  ceiling Critic→`Authority::LeadAgent` (compose.rs:478-487) — required for the gate
  to ADMIT a cross_cutting dispatch at all (gate conjunct 2: unit authority must be
  within the exercisable ceiling), recorded in code and in e2e-real.md:90-95. The
  conservative `none` posture keeps its Critic ceiling and behavior byte-for-byte;
  closure authority is unchanged (the unattended loop still cannot close lead_agent
  work — Q1). This is a posture-level governance change the operator should ratify
  at settlement; it is disclosed, not silent.
- §16.2/§17.5 interaction is clean: authority (gate admission, recorded per dispatch)
  and containment (the seam's wrap at the spawn boundary, carrying the gate-admitted
  policy ref) are separate mechanisms; both precede process start; §17.5.4 holds for
  every policy-carrying dispatch.
- The `sh -c` spawn of the wrapped ONE-string command is mechanically sound
  (POSIX single-quote joining with `'\''` escaping; the sandbox binary is the first
  process — and the bwrap flags make it the pid-1 of the namespace).
- **F3 (MAJOR — real defect exposed, disclosed, shipped unfixed).**
  `RecordingAgentRunner::admitted`
  (crates/gauntlet-app/src/compare/recording.rs) hardcodes
  `tier: BlastRadius::Contained` into the DURABLE role_invocation record. Latent
  until this task made live cross_cutting dispatches reachable; now the durable
  record misstates the tier for every such dispatch. The builder disclosed it
  (test comment e2e_full_loop_real_runner.rs:381-385; e2e-real.md:189-196) and
  routed the assertions to the accurate facts (policy ref + gate admission
  evidence). Disclosed-and-deferred is the right call for a task whose contract is
  the wrap routing (the fix is a tier channel through the recording runner — a
  settled-surface change the orchestrator should schedule), but per the severity
  scale an exposed real defect is MAJOR.

## Q6 — the settled-surface edit (compose_teeth.rs leg 4)

**Verdict: honest re-pin, required by the retirement; one overstated claim (F4,
minor) and the binding gap is F2's.**

- The old leg pinned the deferral this task retires; keeping it was not an option.
  The replacement asserts: the old refusal text is GONE, the run reaches a bounded
  honest non-settled terminal without crashing, and the durable dispatch record for
  AK-901 carries the admitted policy ref (`tier-contained-compose-teeth`) — the
  routed world. The fail-closed `(None, Some(policy))` arm is covered here too: a
  seam-less composition under a policy-carrying dispatch produces NO invocation and
  the export find panics — the never-unwrap refusal arm is still teething.
- Mutation analysis the brief asks for: "drop the actual wrap but keep recording the
  policy" (e.g. `LiveContainment::wrap` returns `Ok(command)` verbatim) keeps leg 4
  green — the policy ref rides the spec/admission regardless of what the seam
  returned, and this leg's child program does not exist so nothing observes
  confinement. The OLD leg would ALSO have been green under old code + that same
  mutation (its refusal came from the composition arm, not the seam), so this is NOT
  a weakening relative to the old leg's teeth — but see F2 for the leg that MUST
  carry the execution-binding (the real-runner E2E), where the unit-agnostic scan
  leaves exactly this mutation partially open (tier-conditional variant).
- **F4 (minor — overstated disclosure wording).** The re-pin comment claims the
  never-unwrap invariant is pinned "in its stronger, routed form"
  (compose_teeth.rs:355-366, echoed at e2e-real.md:184-188 "pinned STRONGER"). The
  routed pin is stronger in coverage (the wrap now actually executes somewhere) but
  WEAKER at this leg's locus: a record-level pin cannot distinguish wrapped from
  unwrapped spawn, whereas the old refusal made any execution impossible. The
  disclosure is otherwise thorough and adequate. Say "differently", not "stronger",
  at this leg; the strength lives in the E2E once F2 is fixed.

## Q7 — the recovery target

**Verdict: the kill boundaries are observed durable facts and the exactly-once /
run-identity assertions are airtight for both kill sides; the produce-window leg is
real with a disclosed race; the R16-F2 leg reaches its bounded honest terminal AND
exposes a real orphan defect (F6, MAJOR, disclosed and pinned).**

- Leg 1 (mid-settlement): the boundary is the DURABLE settlement event count read
  from the state DB via the settled test_support reader (20 ms tick, 120 s bounded
  deadline, typed panic — no sleep-and-hope). The kill always lands AFTER §16.3
  step 1 (the event is the trigger), either side of step 2 — and the post-resume
  assertions hold for BOTH: `settlement_committed_events == 1` AND exactly one
  AK-951 settlement row (e2e_recovery.rs:459-468) — a re-minted settlement on
  resume fails the count; an unrepaired half-commit fails the row. Run identity is
  derived from the durable state-dir layout (exactly one run) and proven by the
  export answering for the same id; AK-953 never-dispatched re-asserted
  post-resume; prior evidence (the wrapped child's artifact) asserted intact. The
  documented stale-lock refusal + operator lock removal (COR-R3-3) is itself
  asserted before the resume — the recovery path through the real binary.
- Leg 2 (produce window): the boundary is artifact-bytes-durable vs production rows
  short — a genuinely distinct window from the settlement commit. **F5 (minor):**
  the poll predicate explicitly accepts the missed window ("the artifact and its
  production row are both durable", e2e_recovery.rs:528-533), so the kill is
  guaranteed only at artifact durability, not strictly in-window (a sub-millisecond
  window polled at 20 ms from outside will usually be lost). Disclosed in-test and
  the assertions are valid on either side (production exactly-once after resume);
  e2e-real.md:141-143 says "at the OBSERVED produce window", slightly stronger than
  the guarantee. Acceptable with the disclosure; tighten the report wording.
- Leg 3 (R16-F2): the hostile child (directory at its own contract path,
  SIGTERM-immune) meets the composed loop's own machinery: bounded terminal within
  a hard 180 s deadline whose firing is a test FAILURE (a hang is the defect the
  trigger hunts — none), typed `transient_runner_error` pinned in the evidence
  store, zero settlements, no panic, and the RESUME also terminates bounded with
  still-zero settlements (liveness preserved) — exactly the charter fact 7
  requirements, all asserted.
- **F6 (MAJOR — real defect exposed; disclosed; orchestrator-owned follow-up).**
  At that terminal the runner INVOCATION is never cancelled: the transient
  disposition (§11.5 row 2) retries the typed error and never calls the port's
  `cancel`, so the hostile child chain OUTLIVES the run and outlives the resume;
  P8's explicit-cancel retirement is enforced nowhere on this composed path. The
  leg does not merely survive this — it PINS the leak as durable evidence and
  records it (test docs e2e_recovery.rs:602-609; e2e-real.md:146-155), then reaps
  the survivors by their own durable handle files as scoped test hygiene ("never
  part of the assertions"). The charter's hard requirement (bounded honest terminal,
  resume liveness) is met and asserted; the orphan is a real lifecycle defect
  exposed by the discharge, exactly what R16-F2's trigger was for. MAJOR, not
  BLOCKING: no claim in the delivered tree is false — the leak is asserted as
  observed and owned.

## Q8 — the budget target

**Verdict: honest non-convergence, record-derived reserve and termination facts, no
timing fragility; the terminal-class divergence is disclosed (F7, minor).**

- The silent child (`exec sleep 300`; never writes a contract, never exits) is the
  charter's honest non-convergence shape; the composed loop cancels it by the REAL
  stall deadline (2000 ms; no deterministic-clock override here — the deadline is
  real), retries once, and the second timeout escalates per §11.5 row 4.
- Record-derived: zero settlement events from the state DB; the LAST DURABLE
  CHECKPOINT's own `budget_spent` yields `rounds_booked < 4` (the fixture ceiling —
  the withheld remainder being the reserve slice) and honest zeros on
  tokens/cost (e2e_budget.rs:385-419); the escalation queue holds AK-951. The
  clean-termination assertions (non-zero exit, `terminal=escalated`, "timed out
  twice", no panic) are stdout-derived but backed by the durable facts above.
- **F7 (minor — plan step 4's "budget exhaustion" terminates as `escalated`, not
  `budget_exhausted`).** The §11.5 row-4 double-timeout escalates the run out
  before rounds can exhaust, so the reserve machinery is not the binding
  constraint; "reserve intact" is proven as booked-rounds-below-ceiling plus honest
  zeros, which is consistent but indirect. Disclosed in-test (lines 334-339) and in
  e2e-real.md:180-183 ("Pinned as observed; not forced"); the budget_exhausted
  terminal itself remains pinned by the settled p6 at controller level. Also noted:
  `FIXTURE_MAX_ROUNDS: 4` is spelled as the fixture's configuration fact rather
  than parsed from GAUNTLET.md (justified inline; AK-016 tension acceptable).
- Timing: ~10 s observed vs a 180 s bounded deadline (18x headroom); the only
  load-sensitive surface is the stall detector's 2 s timeout against the default
  5 s poll tick, which still terminates far inside the bound. No sleep-and-hope
  anywhere; the bounded wait's firing is a typed failure. Not flaky by design.

## Q9 — the run report (e2e-real.md)

**Verdict: every checked claim is record-derived and consistent with the delivered
tests; the deterministic-child posture is stated plainly and repeatedly; the
divergences are recorded. Two wording nits are already folded into F2 and F5.**

Specifics verified: the RED-first demonstration quotes the exact pre-change refusal;
the gate-output table matches the targets' observed shape (and my own re-runs agree
so far, Q11); the AK-066/AK-044/AK-047/recovery/R16-F2/budget paragraphs each name
where the fact durably lives; the authority-ceiling raise, the compose_teeth re-pin,
the recording-runner defect, the AK-066 settleability divergence, the locator
divergence, plan step 4's terminal, and the unexercised `local` tier are ALL
recorded in "Honest divergences and calibrations". Nothing projected was found; no
charter-required leg lacks evidence. The "configured-but-untested" section honestly
lists tasks 26/27, the local tier, and the absence of any frontier-model claim.

## Q10 — fixture hygiene

**Verdict: clean.**

- Frozen lock: `scripts/reference-digest.sh` over `.wayfinder/answer-key.md` and
  `.wayfinder/reference.yaml` reproduces BOTH digests in
  `.gauntlet/reference.lock` byte-for-byte (executed in this adjudication).
- Policies: exactly `contained.json`, `local.json`, `cross_cutting.json` — complete
  (backend bwrap, deny-by-default network, `$WORKSPACE_ROOT` writable, credential +
  git-dir deny sets) and NO `irreversible` policy (AK-049 posture).
- Unit set validates against §5.3.2: AK-951 contained/critic; AK-952
  cross_cutting/lead_agent (critic forbidden — respected); AK-953
  irreversible/human (coupling rule); all `producer: instrument`; evidence locators
  instrument-producible.
- `agent/child.sh` is fixture data: mechanical field parsing, bounded writes, strict
  `role_result@v1` emission, and three declared test modes (`silent`, `poison`,
  `reap` — the last is the harness's own hygiene argv contract). No orchestration
  logic, no reference/gate/evidence access.
- No debris: the fixture tree carries exactly the delivered files; repo
  `.gauntlet/policies/` has the original three files + `.gitkeep` with pre-task
  mtimes; `git status` shows no modifications to any never-touch path
  (`.wayfinder/**`, `.gauntlet/reference.lock`, repo GAUNTLET.md, `p*.rs`/
  `g*.rs`/`e2e_full_loop_mock.rs`, `tests/fixtures/target/`). The only tracked-file
  change outside the charter's deliverable set is `.agents/CURRENT_STATUS.yml` —
  and its diff is the ORCHESTRATOR's own dispatch/critic-phase bookkeeping
  (orchestrator-owned file), not builder damage. Scratch worlds live under `/tmp`
  and self-destruct via the harness `Drop` impls — verified for THIS critic's own
  runs: zero `gauntlet-e2e-*` entries remain under `/tmp` after the battery
  (`ls /tmp | grep -c gauntlet-e2e-` → 0). The critic planted no scratch file
  anywhere under the repo and mutated no tracked file.

## Q12 — improve_cmd.rs

**Verdict: honest.** Passing `None` keeps the improve surface in the historical
template-only world — `gauntlet improve` fronts no live containment posture and
this change preserves exactly that. The composition of it is fail-closed: if a
policy ever rode an optimizer dispatch, the runner's `(None, Some(policy))` arm
REFUSES the dispatch (never unwraps), and the optimizer's isolation reporting stays
`None`, so the capability-vs-tier conjunct refuses anything above `contained` on
its own. Not a silent posture change — it is the explicit recording that the seam
is a composed-run fact, with the typed refusal as the tripwire.

## Q11 — regressions (battery)

Every invocation below was executed by THIS critic under its own top-level
`flock .agents/reports/task25/compile.lock -c '...'` with `sccache --stop-server`
inside the same flock'd command (the trailing "couldn't connect to server" from
sccache is the expected no-server noise after a successful stop; it is the only
source of nonzero chain exit codes — every cargo exit code is recorded as
`EXIT_*:0`):

```text
cargo nextest run --test e2e_full_loop_real_runner   2/2 passed   EXIT_REAL:0
cargo nextest run --test e2e_recovery                3/3 passed   EXIT_RECOVERY:0
cargo nextest run --test e2e_budget                  1/1 passed   EXIT_BUDGET:0
cargo nextest run --test e2e_full_loop_mock         12/12 passed  EXIT_MOCK:0
cargo nextest run --test p5_hard_gate                4/4 passed   EXIT_P5:0
cargo nextest run --test p7_recovery --test p6_budget_control  7/7 passed  EXIT_P76:0
just boundaries                                    237/237 passed EXIT_BOUND:0
just lint        (clippy --workspace --all-targets -D warnings + fmt --check)  EXIT_LINT:0
just ruler       PASS — digests match the frozen lock                          EXIT_RULER:0
```

`just ruler` was run FIRST (before any inspection battery) and AGAIN last: PASS both
times — the ruler never moved during this adjudication. Q11 result: ALL GREEN; no
BLOCKING finding arises from regressions. p5's own
`containment_wraps_from_process_start_and_confines_the_session` stayed green
unchanged through the production correction, and `just boundaries` (237/237)
confirms the seam stayed ports-clean.

Probe-hygiene proof for this adjudication: `ls /tmp | grep -c gauntlet-e2e-` → 0
after the battery (scratch worlds self-destructed); `.gauntlet/policies/` still the
original three files + `.gitkeep` with pre-task mtimes; no tracked file mutated by
the critic; nothing staged, nothing committed.

**F10 (minor — report miscount).** e2e-real.md:105 records the mock regression as
"14/14 PASS"; the target carries exactly 12 `#[test]` functions
(e2e_full_loop_mock.rs) and my run executed 12/12. The substance (green) is true —
my own run proves it — but the count is wrong on the page.

Findings F1–F10 in the ledger below.

## Findings ledger

- **F2 — MAJOR** — e2e_full_loop_real_runner.rs:360-367 — the wrapped-execution
  evidence is not bound to the AK-952 (cross_cutting) unit; the tier-conditional
  seam mutation ("wrap contained, spawn cross_cutting unwrapped") keeps both legs
  green. Fix: assert the stored `"unit":"AK-952"` artifact carries
  `confinement=confined` + `tmp-write=denied`.
- **F3 — MAJOR** — crates/gauntlet-app/src/compare/recording.rs
  (`RecordingAgentRunner::admitted`) — durable role_invocation record hardcodes
  `tier: Contained`; now reachable for cross_cutting dispatches, the durable record
  misstates the tier. Disclosed by the builder; needs a tier channel (orchestrator
  follow-up).
- **F6 — MAJOR** — the composed loop's transient disposition never cancels the
  runner invocation: the R16-F2 hostile child outlives the run and the resume;
  P8's cancel contract unenforced on this path. Disclosed and pinned as evidence by
  leg 3; orchestrator owns the fix.
- **F1 — minor** — e2e_full_loop_real_runner.rs:301 — test name says "settles every
  dispatchable unit"; AK-952 is dispatchable and does not settle.
- **F4 — minor** — compose_teeth.rs:355-366 (and e2e-real.md:184-188) — "stronger"
  overstates the re-pinned leg's record-level pin; it is a different, at-locus
  weaker pin whose execution strength lives in the E2E (post-F2).
- **F5 — minor** — e2e_recovery.rs:528-533 + e2e-real.md:141-143 — the
  produce-window leg guarantees only the artifact-durability boundary (the in-window
  branch is explicitly optional); report wording "at the OBSERVED produce window"
  slightly overstates.
- **F7 — minor** — e2e_budget.rs:334-347 + e2e-real.md:180-183 — the budget leg's
  terminal is `escalated` (row-4 double timeout), not `budget_exhausted`; reserve
  intactness is indirect (rounds-below-ceiling + zeros). Disclosed; p6 still pins
  the exhaustion terminal at controller level.
- **F8 — minor** — fixture has no `local` unit (plan step 1 "at least one Unit per
  tier up to cross_cutting"); disclosed as configured-but-untested
  (e2e-real.md:202-205); the charter's own deliverable list omits `local`; the
  dispatch path shares the proven mechanisms.
- **F9 — minor** — happy-leg coverage note: AK-953's escalation-queue presence is
  proven in leg 2's governance close, not in the happy leg itself (which asserts
  only AK-952's ticket); the Done-when sentence reads as one run proving both.
- **informational** — Q3: leg 2 cannot discriminate per-dispatch re-validation from
  composition-time validation (mechanism present and typed; contract suite is
  AK-047's primary evidence). Q5: the live-posture authority ceiling raise
  (Critic→LeadAgent) is a recorded posture change the operator should ratify at
  settlement. bwrap adapter mandates `--unshare-pid` + ro baseline, grounding the
  confinement discriminator mechanically.

---

## VERDICT

**CONFIRM**

Counts: **BLOCKING: 0 · MAJOR: 3 (F2, F3, F6) · minor: 7 (F1, F4, F5, F7, F8, F9,
F10) · informational: 3**

Basis. The delivered tree proves what it claims, honestly: AK-066's real-world leg
drives the real binary, real generic CLI adapter, and live bwrap backend end to end
on a seeded reference whose units pin the frozen authority semantics (auto-closable
unit settles with pinned instrument evidence; the cross_cutting unit dispatches
WRAPPED with its closure routed to the operator; the irreversible unit is never
dispatched and stands in the queue — all from durable records); AK-048's wrap is
installed at composition, decided after admission, re-validated per dispatch, and
applied before the first process of every policy-carrying spawn, with every
non-seam path fail-closed and every alternative runner refused by the isolation
rung; AK-047 escalates with exactly one dispatch and no fallback arm anywhere;
AK-044 is durable-record absence plus a queue ticket. Recovery kills land on
observed durable boundaries with both-sides airtight exactly-once assertions; the
budget leg terminates cleanly with record-derived reserve facts. The fixture is
hygienic, the frozen lock matches, never-touch paths are untouched, and the FULL
regression battery — including `just ruler` before and after — is green under this
critic's own runs. The settleability divergence, the compose_teeth re-pin, the
authority-ceiling raise, and both exposed defects are recorded by the builder, not
hidden.

What keeps this from being clean: three MAJOR residuals for the orchestrator —
(F2) bind the confinement evidence to the AK-952 artifact (one assertion; the
current scan is unit-agnostic and a tier-conditional seam mutation slips through),
(F3) the recording runner's hardcoded `tier: Contained` in the durable invocation
record (disclosed, now load-bearing), and (F6) the transient disposition's missing
runner cancel — the hostile child outlives the run (disclosed, pinned, R16-F2's
discharge evidence). None falsifies a delivered claim; all three are recorded in
the delivered tree itself.
