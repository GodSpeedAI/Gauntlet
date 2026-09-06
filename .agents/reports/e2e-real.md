# E2E real-run report — plan Task 25 (phase 7)

Written 2026-09-05 by builder B1 from the ACTUAL runs executed this
session. Nothing here is projected. Gate outputs are quoted verbatim at
the bottom. The completion contract with the full command ledger is
`.agents/reports/task25/b1-report.md`; the progress ledger is
`.agents/reports/task25/b1-progress.md`.

## What ran

The composed Gauntlet loop — the REAL `gauntlet` binary, frozen-reference
freeze through plan, dispatch, observation, verification, settlement,
report, export — over the seeded bounded project
`tests/fixtures/target-real/`, through the REAL generic CLI adapter
(`runners.default: cli-agent`) under a LIVE containment backend
(`sandbox.backend: bwrap`, bubblewrap 0.11.1 on this host, live per the
task-0 environment report and re-verified empirically this session).

## Honest posture statement (charter UNK-003, recorded everywhere)

The "real AgentRunner" is the generic CLI adapter — a REAL runner proven
by P8 (AK-013/AK-014) — driven by a DETERMINISTIC FIXTURE CHILD
(`agent/child.sh` in the fixture project) for CI-practicality. A
deterministic child under the real adapter is NOT a frontier-model claim
and is never worded as one: the adapter, transport, contract delivery,
poll classification, deadline and cancel mechanics are the real
production code paths; the child's "intelligence" is fixture data. The
fixture-level `runners.default: cli-agent` selection is the recorded
operator resolution for this task; the repository root's GAUNTLET.md
stays unset and UNK-003 itself remains open with terminal disposition at
Task 27.

## The fixture (deliverable 1)

`tests/fixtures/target-real/` — its own GAUNTLET.md (`runners.default:
cli-agent`, live sandbox backend, full budget block, `max_parallel: 1`
with the recorded write-once binding reason), its own frozen reference
(`.wayfinder/{answer-key.md,reference.yaml}` + `.gauntlet/reference.lock`
frozen via `scripts/reference-digest.sh`; digests verified by every run's
doctor and by `just ruler` semantics), its own three tier policies
(contained / local / cross_cutting; NO irreversible policy — AK-049), its
real source + real check command (`src/gadget.rs`, `scripts/check.sh`),
and the deterministic child agent (`agent/child.sh`: normal / silent /
poison / reap / alive modes, all empirically exercised).

Units (fixture answer key AK-951..AK-954; AK-954 added by corrections
round 1, F8):

- **AK-951** — contained / critic. Auto-settleable.
- **AK-952** — cross_cutting / **lead_agent** (the §5.3.2 coupling
  forbids critic authority on cross_cutting outright; this is forced by
  the domain, not a fixture choice).
- **AK-954** — local / critic. Auto-settleable (added by corrections
  round 1 so the fixture carries a unit for every dispatchable tier up
  to cross_cutting, per plan step 1).
- **AK-953** — irreversible / human. Never an automatic dispatch
  candidate (AK-044).

## The production correction (deliverable 2, RED-first)

RED was demonstrated BEFORE the correction: with the task-20 composition,
`gauntlet run` on this fixture refused with exactly the recorded
deferral — `unit AK-951 needs a live-sandbox wrap verdict; routing a
wrapped builder command through the composed loop is plan Task 25's leg —
refusing here rather than running unwrapped (spec 21.3, AK-047)`.

The correction retires `CompositionError::LiveContainmentDeferred` and
routes the gate's `Sandboxed` verdict to the actually-spawned child:

- **Adapter** (`gauntlet-adapter-agent-cli`): a typed
  `CommandContainment` seam + `ContainmentRefusal`, installed at
  construction (`with_containment`). At `invoke`, a dispatch carrying an
  admitted policy ref is POSIX-quote-joined and wrapped through the seam;
  the wrapped command (ONE shell-invocable string whose FIRST process is
  the sandbox — AK-048) is spawned as `["sh", "-c", wrapped]`. A
  policy-carrying dispatch with NO seam installed is a typed admission
  refusal — fail-closed, never an unwrapped run. Policy-free dispatches
  (the §8.6 no-backend world) spawn verbatim, byte-for-byte as before.
  **R-2 posture re-adjudication (recorded for the operator)**: the
  adapter historically consumed ONLY a command template and reported
  `isolation: none`. With the seam installed it consumes a WRAPPED
  command and reports `Isolation::Process` — derived from the
  configuration exactly the way `supports_model_selection` derives from
  the template, never aspirational. Gate semantics, tier/authority
  coupling, and the capability-vs-tier conjunct are untouched (p5 and
  g7 stay green unchanged; the full battery passed).
- **Composition** (`gauntlet-cli/src/compose.rs`): the govern posture is
  composed BEFORE the runner; under a live posture the SAME sandbox +
  policy instances back a `LiveContainment` seam (resolve the admitted
  policy BY REF over the dispatchable tiers, re-validate at the spawn
  boundary — validation precedes use, again — then wrap through the
  backend). The composition-time smoke check still admits + wraps the
  FIRST planned dispatch before any run starts; refusals still fail the
  composition fail-closed. The live posture's exercisable-authority
  ceiling is raised from Critic to `Authority::LeadAgent` (recorded: the
  coupling forbids critic authority on cross_cutting, so a live posture
  that could not exercise lead-agent authority could never dispatch the
  tier it fronts a backend for; the conservative none posture keeps its
  Critic ceiling and its behavior is unchanged).

## Gate outputs (the slice battery, actually executed)

```
just ruler                     PASS (digests match the frozen lock)
just check-package gauntlet-cli                       PASS
cargo nextest run --test e2e_full_loop_real_runner    2/2 PASS (~16s)
cargo nextest run --test e2e_recovery                 3/3 PASS (~21s)
cargo nextest run --test e2e_budget                   1/1 PASS (~10s)
cargo nextest run --test e2e_full_loop_mock           12/12 PASS (regression)
cargo nextest run --test p5_hard_gate                 PASS (gate teeth)
cargo nextest run --test p7_recovery --test p6_budget_control  PASS
just lint                                             PASS
just boundaries                                       237/237 PASS
```

(Combined re-run of the six gauntlet-cli targets after the final format
pass: 29/29.)

## What the runs PROVED (record-derived; where each fact lives)

1. **AK-066's wrap leg (AK-048 real-world)**: the cross_cutting unit's
   durable `role_invocation` record carries the admitted policy ref
   (`tier-cross-cutting-real-v1`); the gate admission evidence pinned
   BEFORE the invocation names the tier, the policy, and the validation
   rung; the wrapped child OBSERVED its own confinement from inside —
   pid 1 of its process namespace was the sandbox and out-of-root writes
   were denied by the read-only baseline — and AK-952's OWN locator
   artifact carrying those facts is durably ingested
   (`confinement=confined`, `tmp-write=denied` bound to the
   `"unit":"AK-952"` record — the unit binding is asserted explicitly,
   corrections round 1, F2, so the evidence cannot be satisfied by the
   contained unit's artifact). An unwrapped run produces the opposite
   facts (host init, writable /tmp) — the discriminator was verified
   empirically on this host before pinning.
2. **AK-044 real-world**: AK-953 has ZERO `role_invocation` records in
   the durable export across every leg, and its ticket stands in the
   escalation queue in the SAME run as the never-dispatched scan
   (corrections round 1, F9): the scheduler's first-refusal ticket reads
   "dispatch not scheduled: Unit AK-953 is irreversible: never an
   automatic dispatch candidate — scheduler-level, unconditional (spec
   11.2, 17.1)"; in the governance-closed run the close-out additionally
   records "run blocked: Unit AK-953 is irreversible: never an automatic
   dispatch candidate — scheduler-level, unconditional (spec 11.2,
   17.1)".
3. **AK-047 real-world**: a scratch-world policy naming a backend the
   posture does not front resolves as an artifact and FAILS validation —
   the escalation ticket reads "sandbox policy
   tier-cross-cutting-real-v1 failed validation for unit AK-952 (1
   finding(s)) — escalation, never an unsandboxed fallback (spec 21.3,
   AK-047)"; the durable record holds exactly one dispatch (the
   contained unit's); the run closes with the §11.6 governance terminal.
4. **AK-040/AK-042 real-world (recovery)**: SIGKILL at the OBSERVED
   first-settlement boundary (and at the ARTIFACT-DURABILITY boundary for
   the produce-window leg — the task-12 ledger's crash-injection trigger;
   the in-window branch is explicitly optional in the test's own
   disclosure, so the guaranteed boundary is the artifact's durability,
   not the window itself), the documented stale-lock refusal + operator
   lock removal, then `gauntlet resume` — exactly-once settlement,
   preserved run identity, prior evidence intact.
5. **R16-F2 DISCHARGED against the composed loop**: the hostile child
   (directory at its own contract path, SIGTERM-immune) meets the loop's
   own machinery — typed `transient_runner_error` pins in the evidence
   store, a bounded honest terminal (never a hang; the test's bounded
   wait is a hard failure), zero settlements, and a subsequent resume
   terminates too. **RECORDED FINDING — FIXED by corrections round 1
   (F6), see below**: the transient disposition never cancelled the
   runner invocation, so the hostile child OUTLIVED the run; the leg
   reaped the survivors by their own durable handle files (test
   hygiene). p8's explicit-cancel retirement was enforced nowhere on
   this path — the orchestrator owned the follow-up.
6. **AK-037..AK-039/AK-051/AK-052 real-world (budget)**: the silent
   child (never writes a contract, never exits) is cancelled by the REAL
   stall deadline, retried once per the §11.5 row-4 table, and the
   second timeout ESCALATES and terminates the run — bounded (~10s),
   exit non-zero, zero settlements, tokens/cost billed at honest zeros,
   and the booked rounds stop strictly below the fixture's ceiling: the
   recovery reserve provably never spent, read from the run's own final
   durable checkpoint.

## Honest divergences and calibrations (for the ruler's owner)

- **AK-066 "settles for every blocking Unit" vs the frozen authority
  semantics**: the §5.3.2 coupling forces any cross_cutting unit to
  non-critic authority, and §16.1 condition 3 forbids the runtime to
  auto-close lead_agent work — so a cross_cutting unit in a seeded
  reference can NEVER settle in an unattended run. The honest run:
  AK-951 settles (wrapped), AK-952 dispatches WRAPPED, verifies, and has
  its closure ROUTED to the operator ("requires lead_agent authority for
  closure; the in-loop actor is runtime"), AK-953 never dispatches; the
  terminal is the loop's own bounded honest one. Recorded here and in
  the task report; the ruler owner owns the reconciliation (same class
  as the already-recorded AK-066 locator divergence: the plan's cargo
  target spellings `e2e_*` materialized under `crates/gauntlet-cli/tests/`,
  not `tests/e2e/full_loop_real_runner`).
- **Plan step 4's "budget exhaustion"**: with a non-converging unit, the
  §11.5 row-4 timeout table terminates the run (escalated) before rounds
  can exhaust — the budget-control machinery's own terminal, with the
  reserve intact. Pinned as observed; not forced.
- **compose_teeth leg 4 re-pinned** (settled-surface edit, disclosed in
  the task report): the leg pinned the task-20 DEFERRAL this task
  retires. It now pins the routed world — the composition proceeds, the
  dispatch runs, and the durable invocation record carries the admitted
  sandbox policy. DIFFERENTLY PINNED, not stronger: the coverage moved
  to the routed dispatch (the wrap now actually executes somewhere) and
  the at-locus strength — a pin that distinguishes wrapped from
  unwrapped SPAWN — lives in the real-runner E2E, where the confinement
  evidence is bound to the dispatched unit's own artifact.
- **Latent defect EXPOSED, FIXED by corrections round 1 (F3)**:
  `RecordingAgentRunner::admitted`
  (`crates/gauntlet-app/src/compare/recording.rs`) hardcoded
  `tier: BlastRadius::Contained` in the DURABLE role_invocation record —
  latent until this task made live cross_cutting dispatches reachable.
  The corrections round added the smallest typed tier channel: the
  controller registers the gate-admitted tier per invocation
  (`RecordingAgentRunner::record_dispatch_tier`), the durable record
  states it, a no-policy dispatch is honestly `contained` (the only
  §5.3.5-legal shape for no policy), and a policy-carrying dispatch with
  no recorded tier fact is refused fail-closed. The runner PORT was not
  widened.

## Configured-but-untested / out of scope

- Tasks 26 (baselines) and 27 (conformance/handoff): out of scope per
  the charter.
- The `local` tier: NO LONGER UNTESTED — corrections round 1 (F8) added
  AK-954 (local/critic) to the fixture; its dispatch, wrapped execution,
  settlement, and pinned evidence are asserted in the happy leg. (This
  bullet's original text — "no unit carries it in the fixture ... and
  is otherwise unexercised" — described the pre-COR-1 fixture and is
  superseded.)
- Frontier-model behavior: intentionally absent (see the posture
  statement); nothing here may be cited as model evidence.
- The repo-root GAUNTLET.md and `.gauntlet/policies/` were never
  modified (probe hygiene proof in the task report: original mtimes and
  file set verified; all leg mutations live in per-test scratch worlds).

## Corrections round 1 (COR-1, 2026-09-05)

The blind design-as-built critic adjudicated this task CONFIRM with 0
BLOCKING / 3 MAJOR / 7 minor (`.agents/reports/task25/critic-design.md`);
the orchestrator owned the dispositions and dispatched the corrections
builder (contract: `.agents/reports/task25/cor1-report.md`). What
changed, by finding:

- **F2 (MAJOR, fixed)**: the confinement evidence is now BOUND to the
  AK-952 artifact (`"unit":"AK-952"` + `confinement=confined` +
  `tmp-write=denied`), demonstrated RED-first under the brief's
  tier-conditional seam mutation (cross_cutting spawned verbatim kept
  the old scans green; the new assertion went RED with
  `confinement=uncontained pid1=systemd`), compose.rs restored
  byte-identically (sha256 in the contract report).
- **F3 (MAJOR, fixed)**: the durable role_invocation record states the
  dispatch's REAL tier (typed per-invocation tier channel; the runner
  port was not widened). RED-first via the strengthened happy-leg
  assertion; regressions green (recording-runner users p2/p9/p10, mock
  E2E, gauntlet-app package).
- **F6 (MAJOR, fixed)**: the composed loop's §11.5 row-2 transient
  disposition cancels the prior invocation through the AgentRunner
  port's idempotent cancel before every re-dispatch of unknown-fate
  work, and a terminal run cancels everything still in flight. The
  R16-F2 hostile child no longer outlives the run or the resume; leg 3
  now asserts the retirement (pinned records + `"how":"cancelled"` +
  bounded process-liveness poll over the durable handle files). RED
  first at controller level (two tests); p4/p6/p7/p5 and the mock E2E
  stayed green throughout — the cancel bound is a new
  `cancel_operation_bound` at the §11.4 stall-window scale (the previous
  `operation_bound` was the run's remaining wall horizon — a
  SIGTERM-immune child would have blocked one cancel for up to 600s).
- **F8 (minor, fixed)**: AK-954 (local/critic) joined the fixture; the
  fixture's OWN reference lock was re-frozen with
  `scripts/reference-digest.sh` (the repo ruler untouched, `just ruler`
  green before and after). The happy leg proves the local unit
  dispatches (record tier `local`), settles, and pins confined
  evidence; recovery leg 1's exactly-once now covers both auto-closable
  units (2 events, 1 row each).
- **F9 (minor, fixed)**: AK-953's ticket now stands in EVERY run's
  queue — the scheduler's first governance refusal of an owed unit
  mints it ("dispatch not scheduled: ... never an automatic dispatch
  candidate"); the terminal decision is untouched. RED-first: the
  happy run's queue held only AK-952 tickets before the fix.
- **F1 (minor, fixed)**: the happy-leg test is renamed
  `the_composed_loop_settles_the_auto_closable_units_wraps_the_cross_cutting_dispatch_and_escalates_the_rest`
  — it no longer claims AK-952 settles.
- **F4/F5/F10 (minor, fixed)**: this file's "STRONGER" overstatement is
  now the honest differently-pinned wording (mirrored in
  compose_teeth.rs), the produce-window wording states the
  artifact-durability boundary, and the mock regression count is the
  real 12/12.
- **F7 (minor, RECORD-only)**: the budget leg's `escalated` terminal
  (the §11.5 row-4 double timeout) vs plan step 4's "budget exhaustion"
  stays as disclosed; the exhaustion terminal itself is pinned by the
  settled p6 at controller level. No change.

Post-correction battery: all green (the full ledger with exit codes is
in `cor1-report.md`).

## Corrections round 2 (COR-2, 2026-09-05)

The blind EXECUTING falsifier round 1 returned NOT_CONFIRM with 0
BLOCKING / 2 MAJOR / 2 minor / 3 informational
(`.agents/reports/task25/falsifier-round1.md`); the orchestrator owned
the dispositions and dispatched the corrections builder (contract:
`.agents/reports/task25/cor2-report.md`). What changed, by finding —
every fix RED-first, evidence verbatim in the contract report:

- **F-R1 (MAJOR, fixed)**: the no-seam fail-closed arm's claim ("a
  policy-carrying dispatch with no seam installed is a typed admission
  refusal") now has BINDING executed evidence — the adapter-level TEETH
  test `fr1_a_policy_carrying_dispatch_without_the_seam_is_a_typed_refusal_that_spawns_nothing`
  (typed refusal + zero spawns + zero artifacts), demonstrated RED
  under the falsifier's exact disabling mutant while every pre-existing
  suite stayed green under it; the arm's doc comment now cites the
  test.
- **F-R4 (MAJOR, fixed)**: the run report's classification basis is
  RECORD-DERIVED. The budget-terminal report no longer prints "never
  attempted on record" for the twice-timed-out unit (its basis is
  `attempted-and-timed-out: N dispatch(es) on record, the latest
  invocation terminal is timed_out`, category `provisional`), the
  never-dispatched units' basis states exactly "no dispatch attempt for
  this unit is on record", and `terminal_state` is the run's REAL
  terminal (the append-only `run_terminal` event the loop seals every
  pass with), never a null beside stdout's terminal. New durable events:
  `unit_dispatch{unit, invocation_id}` (the unit-attributed dispatch
  join — the §5.3.5 record's field set is spec-frozen without a unit)
  and `run_terminal{state}` (append-only; a re-terminalizing resume
  APPENDS — the transition is visible in the record, never a silent
  rewrite). Typed read `StateStore::run_lifecycle_facts`, COR-7
  fail-closed default.
- **F-R3 (minor, fixed)**: the cancel-for-recovery arm now retires the
  WHOLE wrapped tree — every spawned child is its own process-group
  leader and both stop legs signal the group (SIGTERM → bounded grace →
  group SIGKILL + reap; the negated-pid spelling, dash-compatible).
  RED-first: the round-1 leak shape reproduced at the adapter level
  (driver TERM'd, wrapped payload survived) and killed by the fix;
  the hardened liveness assertion re-proven RED against the reverted
  mechanics and green against them.
- **F-R5 (minor, fixed / pinned)**: durable per-run ticket dedup in the
  fs escalation queue — an OPEN ticket for the same run carrying the
  same fact (the summary) is RE-AFFIRMED, never minted twice; a
  resolved ticket never dedups. The resumed escalated world shows ONE
  open AK-953 ticket, and the re-entry is honest on the record: the
  export carries the `run_terminal` transition (`escalated` → the
  resumed pass's terminal) and the re-spent round is in the checkpoint
  facts (pinned in the new e2e_budget resume leg). The escalation FLOW
  itself was not redesigned.
- **Informational (a) — RECORD, no fix**: the resumed pass minted an
  invocation id identical to the run id (id-space collision across
  record kinds). This is the SEEDED DETERMINISTIC id generator of the
  fixture/test surface (`GAUNTLET_ID_SEED`), not the production
  IdGenerator (ULID — unguessable, unique per kind); the collision is
  therefore fixture-shaped, and a typed per-kind namespace would touch
  the id-port fixture contract for zero production effect — recorded
  rather than fixed. The composed loop's behavior was and is unaffected
  (the ids collide across KINDS, never within a kind's own key space).
- **Informational (b), (c) — fixed inside F-R4**: (b) the report's
  `terminal_state` is the real terminal; (c) the open-unit basis text
  states the unit's own record truth (no dispatch attempt on record)
  — the operator-routing fact itself lives in the operator queue, which
  the report's StateStore read surface cannot see; that boundary is
  recorded as a residual (see below), not papered over with a claim
  the records cannot support.

Post-correction battery: all green on the final tree (the full ledger
with counts is in `cor2-report.md`).

### Residuals recorded (not changed)

- Ticket↔unit binding and terminal-refusal posture (F-R5 item 3): the
  dedup keys on (run, summary) because the port's ticket record shape
  has no unit field; escalation summaries that embed per-attempt
  forensics (the §10.3 rule-5 session reference riding the row-4
  timeout ticket) are deliberately unique per attempt and do NOT dedup
  across passes. A typed ticket↔unit binding is a ports revision —
  recorded for the operator, out of scope here by disposition.
- The stop path's group-kill retires every tree member that has not
  escaped the process group; a grandchild that ignores TERM and whose
  DRIVER dies of TERM can outlive the graceful leg's observation window
  only by having escaped the group (setsid) — beyond this bounded
  claim, recorded.
- The F-R3 cancel-for-recovery trigger (a post-spawn admission
  refusal) is reachable in composed runs only through a composition
  defect (the no-tier-registration shape); the retirement mechanics are
  now bound by the adapter-level test regardless of reachability.

## Corrections round 3 residual — F2-7 (falsifier round 2, MAJOR; recorded, NOT built)

The designed operator flow dead-ends for every ticket this fixture world
actually mints. The composed loop's operator-routed ticket classes — the
§11.5-row-6 lead_agent-closure class and the irreversible never-dispatched
class (AK-953 here) — carry no durable-PASS verification record, and no
reachable path produces one for them: the row-6 class's proof would only
materialize at a settlement, and settlement is exactly what is routed to the
operator; the irreversible class may never dispatch at all. Both operator
actions (`escalations resolve`, `escalations retry`) demand that record as
their precondition (§16.1 condition 1), so every resolve/retry is refused
typed (`verification_not_pass` — "no PASS verification record exists for unit
… of run … — settlement requires one (spec 16.1 condition 1)"), the tickets
are unresolvable through the designed surface, and the run can never reach
`settled` in this world. The falsifier executed the dead end twice through
the real binary (worlds `F1B8H9YCD9JGFC0AJJ9DYV8ZK5` and
`VHEMRP47VQGBYAZ47BNVE5FKD0`); COR-3 pinned the honest refusal as a
regression in the composed budget leg (`e2e_budget::
resolving_a_governance_routed_ticket_is_refused_typed_no_pass_record_exists`:
resolve refused typed, ticket stands open, zero settlements).

The refusal itself is CORRECT — it is AK-062's class doing its job: an
operator bypass without earned evidence is refused, exactly as the automatic
path would be. What is missing is the EARNED path: how an operator-routed
Unit ever acquires the §16.1 PASS evidence its closure requires. That is a
RULER-OWNER question (CandidateRevision material — §16.1's evidence
requirement for human closure vs §11.5's operator routing); the operator
surface itself is task 21's. Recorded here per disposition; no flow was
built, and the ruler and the gate were not touched to make the flow
complete.

## Corrections round 4 + the orchestrator's closure (2026-09-06)

The closure battery's full-workspace runs executed one REAL defect and
one load-flake, then the round that fixed the record-keeping:

- **`just test` run 1**: `b1_contract_wins_over_exit7_and_process_is_
  never_consulted` (agent-cli) failed once under full-workspace load;
  passed in isolation (20/20) and in run 2 and never recurred in any
  later full run — recorded as the task22 load-flaky protocol class.
- **`just test` run 2**: `e2e_recovery`'s produce-window leg FAILED for
  real: the kill landed after the first pass's production row was
  durable (the leg's "missed window" branch), the resume re-dispatched
  AK-951, and the durable store held TWO production rows for
  `real-marker-report.json` with different digests. The leg's
  "assertions hold either way" was executed-false.
- **COR-4** (`cor4-report.md`) diagnosed the resume path and invoked
  the brief's STOP condition with a case the orchestrator verified
  against the cited mechanisms: p7's settled
  retry-as-if-never-verified pin (`p7_recovery.rs:1454`) and the §14.3
  prior-iteration staleness machinery
  (`verify/authority.rs:1149/:1182`) make RE-PRODUCTION the
  spec-forced resume semantics — adoption of a pre-crash production
  would contradict two settled proofs. **Orchestrator adjudication:
  disposition (a)** — the leg's per-locator exactly-once claim was
  stronger than the spec; both recovery legs were re-scoped
  (tests-only, zero production change) to the spec-honest semantics:
  pre-crash production rows survive untouched as write-once facts, the
  resumed pass mints its own per-iteration production, exactly-once
  SETTLEMENT (row + event) binds the current ref, re-dispatch is
  bounded per-branch (durable AK-951 counts: 3 missed-window / 2
  in-window / 1 settled-before, asserted), run identity and prior
  bytes preserved.
- **F-C4-1 (recorded, not fixed)**: the resume's first re-dispatch
  inherits the crashed attempt's workspace (the WorkspaceProvider's
  counter restarts with the process), so that attempt rewrites
  byte-identical bytes, dedupes onto the pre-crash row, and is classes
  stale — a WASTED attempt before the settling one. With an attempts
  ceiling of 2 this crash class would escalate instead of settle. The
  lever is the workspace provider's resume-time counter:
  CandidateRevision material (filed with the settlement's
  reference-candidates).
- **The Done-when teeth, executed personally by the orchestrator**:
  the F2-1 mutant (dedup per-run conjunct deletion) and the F2-4
  mutant (KILL leg direct-pid revert) each went RED exactly as their
  tests promise and were restored byte-identically (sha256 both
  sides). The AK-044 Done-when ladder: disabling the gate's
  irreversible conjunct, then the scheduler's irreversibility
  rejection, then the gate's authority-ceiling conjunct — three of the
  FIVE independent layers — left the never-dispatch invariant holding
  every time (AK-953's `human` authority and the policy-absent tier
  are two further structural layers, each with its own executed unit
  pin). The composed invariant is over-determined by design; the E2E's
  durable-record absence scan pins it, and each layer carries its own
  tooth. All restorations byte-verified (gate.rs sha256; select.rs
  zero-diff vs HEAD).

**Final closure battery (the orchestrator's own runs, on the final
tree, 2026-09-06):**

```
just check                                             exit 0
cargo nextest run --test e2e_full_loop_real_runner \
  --test e2e_recovery --test e2e_budget                9/9 PASS (the plan gate, verbatim)
just ruler                                             PASS
just test                                              1548/1548 PASS, exit 0
just lint                                              exit 0
just deny                                              exit 0
just boundaries                                        237/237 PASS
just ruler                                             PASS (final)
```

The single-writer, exactly-once, and evidence-preservation claims of
the recovery legs now state the spec's actual semantics and hold under
full-workspace load. The ruler never moved across the entire task:
`just ruler` passed at every phase transition of every round, in every
agent's hands and in the orchestrator's.
