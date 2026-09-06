# Task 25 — COR-2 progress ledger

Appended at every landed fix (the task4 lesson: a session death must
lose nothing). The completion contract is `cor2-report.md`.

## Phase 0 — preflight (DONE 2026-09-05)

- `just ruler`: PASS (`RULER_EXIT=0`, digests match the frozen lock).
- `git status` reconciled against the falsifier's round-start record:
  production diff in 7 tracked files, 3 new E2E targets, fixture
  `tests/fixtures/target-real/`, report `.agents/reports/e2e-real.md`.
  Nothing staged. (`.agents/CURRENT_STATUS.yml` and `.jolli/**`
  modifications pre-date this session — orchestrator-owned, untouched.)
- Recon loci pinned: no-seam arm `runner.rs:552-560`; cancel-for-recovery
  arm `gauntlet-app/src/compare/recording.rs:139-160` (calls
  `inner.cancel` with the SPEC's deadline); transport stop
  `adapter-agent-cli/src/transport.rs:217-296` (SIGTERM → bounded wait →
  pid-only `inner.kill()` — the pid-only kill is F-R3's leak);
  report generator `gauntlet-app/src/report/mod.rs`
  (`classify_unit` rule 4 renders "never attempted on record" without
  ever reading invocation records; `terminal_state` comes from
  `load_run`, and the `runs` table has NO terminal column — null is
  structural); escalation tickets minted by
  `controller.rs:submit_ticket` via `FsEscalationQueue::escalate`
  (dedup maps in `LoopState` are in-memory per-pass → F-R5 duplicates
  on resume).

## F-R1 — LANDED (teeth for the no-seam fail-closed arm)

- New test: `crates/adapters/gauntlet-adapter-agent-cli/tests/
  cor2_seam_and_recovery_attacks.rs ::
  fr1_a_policy_carrying_dispatch_without_the_seam_is_a_typed_refusal_that_spawns_nothing`
  (no-seam runner + policy-carrying spec → typed `AdmissionRefused`
  naming the policy and the missing seam; zero spawns on the transport
  counter; zero invocation artifacts under the state root; empty
  workspace tree).
- RED-first (falsifier's EXACT mutant, `(None, Some(policy_ref))` arm →
  `(None, Some(_)) => argv`):
  - Pristine sha256 recorded before the mutant:
    `b20960b5ec66b85b57712bfb38546fc0b72184c63bb8c9b0ce2c8473d39ee5b0`.
  - Under the mutant:
    `Summary [4.398s] 61 tests run: 60 passed, 1 failed` — ONLY the new
    test failed (`panicked at cor2_seam_and_recovery_attacks.rs:146:
    a policy-carrying dispatch with no seam must be refused, never run
    unwrapped: InvocationHandle { ... }`); every pre-existing suite in
    the package stayed green — the gap, demonstrated verbatim. (The
    round-1 falsifier additionally showed the three E2E targets and p8
    green under the same mutant; not re-executed here.)
  - Restored byte-identically: sha256 both sides == pristine
    (`b20960b5…9ee5b0`). VERIFIED.
  - Green on the real tree: `Summary [4.452s] 61 tests run:
    61 passed`.
- Doc binding: `lib.rs` no-seam claim now carries
  `teeth: fr1_a_policy_carrying_dispatch_without_the_seam_is_a_typed_refusal_that_spawns_nothing`.

### INCIDENT (recorded, resolved)

The first restore attempt used `git restore runner.rs` — that restored
the COMMITTED (pre-task-25) file and silently dropped the round's
uncommitted production changes (sha256 `ccf0ecf9…` ≠ pristine). Caught
immediately by the sha256 check. Recovery: the file was reconstructed
byte-exactly from this session's own full read of the pristine state;
sha256 re-verified `b20960b5…9ee5b0` exactly. RULE (now followed): a
mutant is reverted by reversing its exact edit, never by
git-restoring a file that carries uncommitted task work.

## F-R3 — LANDED (the cancel-for-recovery arm retires the wrapped tree)

- Empirical shape probe (pure sh, pre-test): `sh -c 'sleep 30 & wait'`
  TERM'd at the driver → driver exit 143, background sleep SURVIVED
  (the round-1 leak shape). Group probe: `setsid sh -c 'sleep 40 &
  wait'` + `kill -TERM -PGID` → driver and payload both retired.
- RED-first: new test
  `fr3_a_post_spawn_refusal_retires_the_whole_wrapped_child_tree`
  (same cor2 test file). It reproduces the no-tier-registration shape
  at the adapter level: a faithful stand-in for
  `RecordingAgentRunner::cancel_after_persistence_failure` (spawn →
  record-refusal → cancel under the SPEC's deadline → typed
  `AdmissionRefused`), a `TreeWrap` seam reproducing the
  driver+forked-payload leak shape (`<command> & wait`), and a bounded
  liveness poll over BOTH pids (driver from the forensic handle record,
  payload from the child's own `.sleeppid`).
  - RED on the delivered stop mechanics (verbatim):
    `panicked at cor2_seam_and_recovery_attacks.rs:378: the post-spawn
    refusal left the wrapped tree alive: driver pid 426979 alive=false,
    wrapped payload pid 426982 alive=true — the cancel-for-recovery arm
    leaked the child` (10.1s bounded window).
- Fix (`transport.rs`, pristine sha256
  `1714a833cf1e36ae9bc1fc814fe3181f272cf71f63c2c5ff0f68e81218bdff21`
  recorded before the edit):
  1. `spawn` puts every child in its OWN process group
     (`process_group(0)`, stable `CommandExt`).
  2. `stop`'s graceful and escalation legs address the WHOLE GROUP
     (`kill -TERM/-KILL` to the negated pid), fixed-argv, pid-only
     words; bounded legs and the COR-F1 reap grace unchanged;
     `inner.kill()` stays as the direct-pid backstop.
  - Intermediate defect found by the package run itself: the first
    helper spelling used `kill -TERM -- "$1"` — dash's kill builtin
    REJECTS `--` ("Illegal number: -"), the graceful leg burned its
    whole bound, and the round-1 `c_cancel_lifecycle_with_real_children`
    regression went RED (the package caught the regression exactly as
    designed). Corrected to the portable negated-pid spelling
    (`kill -TERM "$1"` with $1 = `-<pid>`), probed on dash first.
- Green: full package `Summary [4.355s] 62 tests run: 62 passed` —
  fr1 + fr3 green, round-1 cancel lifecycle + SIGTERM-ignorer
  regressions green.
- Recorded boundary (not silently claimed): a grandchild that ignores
  BOTH TERM and KILL… cannot survive KILL; the honest boundary is a
  grandchild that ignores TERM and whose DRIVER dies of TERM — the
  graceful leg then observes the driver's exit and returns while that
  grandchild (only if it escaped the group via setsid) survives.
  Group members cannot escape the KILL leg. Recorded as a residual in
  cor2-report.md.

## F-R4 — LANDED (record-derived report classification + real terminal)

- RED-first: e2e_budget extended to parse the run's printed report JSON
  and cross-assert it against the durable export. RED on the delivered
  tree (verbatim, the falsifier's exact world shape): the report line
  printed `"terminal_state":null` beside stdout's `terminal=escalated`,
  and `"units":[{"unit":"AK-951","category":"configured-but-untested",
  "basis":"the latest checkpoint names the unit open — configured into
  the run, never attempted on record"}, …]` while the export held two
  `invocation_terminal state=timed_out` events for AK-951's
  invocations. (The export-side sanity anchors — attempted==["AK-951"],
  every AK-951 invocation terminal timed_out — passed before the report
  assertions failed, so the RED is at the classification locus.)
- Production fix (records, not claims):
  1. Port (`gauntlet-ports/src/state_store.rs`): two event kinds —
     `UnitDispatch{unit, invocation_id}` (the unit-attributed dispatch
     join; §5.3.5's invocation record is spec-frozen without a unit and
     the workspace ref is provider-shaped, never parsed) and
     `RunTerminal{state}` (APPEND-ONLY per pass: a re-terminalizing
     resume appends — the transition is visible in the record/export,
     never a silent rewrite); `RunLifecycleFacts` +
     `StateStore::run_lifecycle_facts` with the COR-7 fail-closed
     default (a store that cannot see attempts must not let a report
     call attempted units never-attempted).
  2. sqlite adapter: encode/decode arms for the two kinds; the read
     implemented as ONE strict `read_entries` scan projected onto the
     three kinds (damaged events are typed Corrupt, never silent
     skips). The export's shared contract stub (`contract/stubs.rs`)
     learned the two new kind tokens (exhaustive match there).
  3. Controller: `UnitDispatch` appended at dispatch right after the
     runner admits (a refused admission never enters the record);
     `seal_terminal` appends `RunTerminal` at all three outcome loci of
     `drive()` (boundary abort, decide_progress, loop exit), after the
     pass's final checkpoint. A failed seal surfaces typed — a pass
     that cannot record its terminal does not claim one.
  4. Report (`gauntlet-app/src/report/mod.rs`): attempted units are
     PROVISIONAL with the record-derived attempt basis
     (`attempted-and-timed-out: N dispatch(es) on record, the latest
     invocation terminal is timed_out`; `attempted-and-open` when no
     terminal is recorded); `blocked` units carry their attempt facts in
     the basis; the never-dispatched basis says exactly "the latest
     checkpoint names the unit open; no dispatch attempt for this unit
     is on record" (the string "never attempted on record" no longer
     renders anywhere); `terminal_state` = the latest recorded
     RunTerminal, falling back to the run aggregate, else null.
  5. `gauntlet-app/tests/report.rs`: the ScriptedStore double gained a
     `facts` field (11 literals updated mechanically); NEW unit-level
     teeth `an_attempted_unit_is_provisional_with_the_record_derived_
     attempt_basis`.
- Green: report target 10/10; `just test-package gauntlet-app` 566/566
  (all controller doubles green with the seal + dispatch appends);
  ports 123/123; sqlite 53/53; e2e_budget 1/1 through the real binary.

## F-R5 — LANDED (durable per-run ticket dedup + honest re-entry pinning)

- RED-first: new e2e_budget leg
  `the_escalated_resume_re_affirms_one_ticket_per_fact_and_records_the_re_entry`
  (pass 1: the silent world → `terminal=escalated`; pass 2:
  `gauntlet resume` of the non-settled run). RED verbatim at the dedup
  locus: `assertion left == right failed: a resumed pass re-affirms the
  open AK-953 ticket — never mints a duplicate: [ticket=esc-6a9ccfa1-0
  …, ticket=esc-6a9ccfab-0 …]` — TWO open tickets, byte-identical
  summaries, exactly the falsifier's two-world observation. (The
  visibility assertions in the same leg — the export's run_terminal
  transition and the checkpointed re-spent round — passed before the
  dedup fix, pinning F-R4's record machinery as the re-entry's honest
  witness.)
- Fix (`gauntlet-adapter-escalation-fs/src/queue.rs`, pristine sha256
  `7a0843fbc4e54e57e1db0c200a54c48c57c66af2e44f41a687a25172e81fce2b`):
  `FsEscalationQueue::escalate` first scans the queue (the quarantine-
  safe `list`) and RE-AFFIRMS an OPEN record with the same run_id +
  summary (the summary is the fact-class carrier the port's record
  shape holds) by returning the existing ticket; a RESOLVED ticket
  never dedups (the operator acted; a re-observation after the action
  is a new fact instance). No port change, no flow redesign — the
  deeper ticket↔unit binding question stays the recorded residual.
- Contract-suite impact (the shared suite pins the OLD always-mint
  semantics): two tests used identical (run, summary) fixtures where
  they needed two co-existing tickets (`the_list_is_total_ordered…`,
  `a_lying_ticket_file_cannot_clobber…`). Both adapted mechanically
  (second ticket = a distinct fact; the properties under test — list
  order, cross-wiring defense — untouched), COR-F4's mechanical-
  adaptation precedent. NEW suite test
  `the_same_open_fact_reaffirms_one_ticket_and_a_resolved_one_never_dedups`
  binds the new semantics at the contract level (re-affirm; different
  fact mints; resolved-then-re-observed mints anew; the listing shows
  one open ticket per open fact).
- Green: escalation-fs package 15/15; e2e_budget 2/2 (the resumed
  report's terminal field == the printed terminal == the latest
  run_terminal event; rounds re-spent 2→3 on the record).

## NEXT

Full battery, informational-(a) recording, probe hygiene proof,
`cor2-report.md`.




## ACK (coordinator status check) — ALIVE, in the battery

All four fixes landed. Battery so far: cli/app checks OK; adapter
package 62/62; e2e_full_loop_real_runner 2/2; e2e_recovery 3/3;
e2e_budget 2/2; e2e_full_loop_mock 12/12; p5 4/4; p7+p6 7/7;
p4+p10+p9 12/12; operator_actions+escalations_cli 3/3 — all green.
CURRENT STEP: `just lint` flagged one clippy lint in MY new e2e_budget
helper (`clippy::double_ended_iterator_last` — a `.last()` on a filter
chain in `last_checkpoint_rounds`). Fixing that one line, then lint +
boundaries + final ruler, then cor2-report.md.

## COMPLETE

- Final consolidated battery on the FINAL tree (one background pass):
  e2e_full_loop_real_runner 2/2 · e2e_recovery 3/3 · e2e_budget 2/2 ·
  e2e_full_loop_mock 12/12 · p5_hard_gate 4/4 · p7_recovery+p6_budget_control
  7/7 · p4_corrective_loop+p10_no_false_learning+p9_self_improvement_containment
  12/12 · operator_actions_no_bypass+escalations_cli 3/3 · adapter
  package 62/62 · gauntlet-app 566/566 · ports 123/123 · sqlite 53/53 ·
  escalation-fs 15/15 — ALL GREEN.
- `just lint` (workspace clippy -D warnings + cargo fmt --check): PASS.
- `just boundaries`: 237/237 PASS (after rewording the vendor token
  from my own test doc comments — the arch denylist caught it, exactly
  as designed).
- `just ruler` final: PASS (RULER=0).
- Contract written: `.agents/reports/task25/cor2-report.md`.
- Informational-(a) recorded in cor2-report.md AND the e2e-real.md
  COR-2 section; residuals recorded in both.
- Probe hygiene: repo .gauntlet/policies/ untouched (ls proof in the
  contract report); nothing staged or committed.
