# Task 25 — COR-2 completion report (corrections round 2)

Builder: the corrections builder dispatched by the orchestrator after
the blind EXECUTING falsifier round 1 returned **NOT_CONFIRM** (0
BLOCKING / 2 MAJOR / 2 minor / 3 informational across 12 executed
attacks — `.agents/reports/task25/falsifier-round1.md`). The binding
brief: `.agents/reports/task25/cor2-brief.md`. Dispositions were
orchestrator-owned; every fix landed RED-first with verbatim evidence
below. Nothing staged, nothing committed.

**VERDICT: ALL FOUR DISPOSITIONS LANDED AND THE FULL BATTERY IS GREEN
ON THE FINAL TREE.**

## Mission echo

F-R1 — TEETH test for the `CliAgentRunner` no-seam fail-closed arm
(`(None, Some(policy_ref))` → `AdmissionRefused`), adapter-level,
RED-first via the falsifier's exact disabling mutant with sha256
restore proof. F-R4 — record-derived report classification (never
"never attempted on record" when invocation records exist), real
`terminal_state` in the report JSON, operator-routed closure's own
honest basis; RED-first via a strengthened e2e_budget report-JSON
assertion; the mock E2E's report legs pinned green. F-R3 — the invoke
path's cancel-for-recovery arm retires the just-spawned wrapped child
(bounded SIGTERM → grace → SIGKILL → reap through the transport's own
stop path); RED-first via an adapter-level post-spawn-refusal liveness
test. F-R5 — durable per-run ticket dedup (re-observed unresolved facts
re-affirm, never duplicate) RED-first via a resume leg, re-entry
semantics pinned honestly WITHOUT redesigning the escalation flow.
Informational-(a) RECORD-only.

## Per-finding disposition + evidence

### F-R1 (MAJOR — FIX) — the no-seam fail-closed arm's teeth

- New test: `crates/adapters/gauntlet-adapter-agent-cli/tests/
  cor2_seam_and_recovery_attacks.rs ::
  fr1_a_policy_carrying_dispatch_without_the_seam_is_a_typed_refusal_that_spawns_nothing`.
  Constructs the runner WITHOUT `with_containment`, drives a
  policy-carrying dispatch, asserts the typed `AdmissionRefused`
  naming the policy and the missing seam, ZERO spawns on the transport
  counter, zero invocation artifacts under the state root, empty
  workspace tree. Needs no bwrap (the refusal fires before any spawn).
- RED-first (the falsifier's EXACT mutant — the `(None,
  Some(policy_ref))` arm replaced by `(None, Some(_)) => argv`):
  - Pristine sha256 recorded first:
    `b20960b5ec66b85b57712bfb38546fc0b72184c63bb8c9b0ce2c8473d39ee5b0`
    (matches the falsifier's round-start record).
  - Under the mutant: `Summary [4.398s] 61 tests run: 60 passed,
    1 failed` — the ONLY failure is the new test:
    `panicked at cor2_seam_and_recovery_attacks.rs:146: a
    policy-carrying dispatch with no seam must be refused, never run
    unwrapped: InvocationHandle { invocation_id: …01ARZ3NDEKTSV4RRFFQ69G5FAV…,
    session_ref: Some(SessionRef { raw: "cli-child:1" }) }`.
    Every pre-existing suite in the package stayed green under the
    same mutant — the gap, demonstrated verbatim. (The falsifier had
    additionally shown the three E2E targets and p8 green under the
    same mutant at round 1; not re-executed here — the package run is
    the binding demonstration.)
  - Restored byte-identically: sha256 both sides == pristine
    (`b20960b5…9ee5b0`), VERIFIED.
  - Green on the real tree: `61 tests run: 61 passed`.
- Doc binding: the adapter `lib.rs` claim now carries
  `teeth: fr1_a_policy_carrying_dispatch_without_the_seam_is_a_typed_refusal_that_spawns_nothing`.
- Note: `runner.rs` needed NO production change — the arm was correct;
  what was missing was the executed evidence.

### F-R4 (MAJOR — FIX) — the report's classification basis is record-derived

- RED-first (e2e_budget extended): the new assertions parse the run's
  printed report JSON and cross-assert against the durable export
  (attempted units derived from `role_invocation` records; every
  attempted invocation's `invocation_terminal` checked `timed_out`
  BEFORE the report is believed). RED on the delivered tree, verbatim:
  the report line carried `"terminal_state":null` beside stdout's
  `terminal=escalated`, and `"units":[{"unit":"AK-951","category":
  "configured-but-untested","basis":"the latest checkpoint names the
  unit open — configured into the run, never attempted on record"}, …]`
  while the export held two timed-out invocation terminals for AK-951.
  The export-side anchors passed before the report assertions failed —
  the RED is exactly at the classification locus.
- Production fix (records, not claims):
  1. `crates/gauntlet-ports/src/state_store.rs`: two event kinds —
     `UnitDispatch{unit, invocation_id}` (the unit-attributed dispatch
     join; §5.3.5's invocation record is spec-frozen without a unit
     and the workspace ref is provider-shaped, never parsed) and
     `RunTerminal{state}` (APPEND-ONLY: a re-terminalizing resume
     appends — the transition is visible in the record/export, never a
     silent rewrite); `RunLifecycleFacts` +
     `StateStore::run_lifecycle_facts` with the COR-7 fail-closed
     default (a store that cannot see attempts must not let a report
     call attempted units never-attempted).
  2. `crates/adapters/gauntlet-adapter-state-sqlite/src/{encode,
     state_store}.rs`: encode/decode arms for both kinds; the read
     implemented as ONE strict `read_entries` scan projected onto the
     three kinds (damaged events are typed `Corrupt`, never silent
     skips). The export's shared contract stub
     (`crates/gauntlet-ports/src/contract/stubs.rs`) learned the two
     new kind tokens (exhaustive match there).
  3. `crates/gauntlet-app/src/control/controller.rs`: `UnitDispatch`
     appended at dispatch right after the runner admits (a refused
     admission never enters the record); `seal_terminal` appends
     `RunTerminal` at all three outcome loci of `drive()` (boundary
     abort, decide_progress, loop exit), after the pass's final
     checkpoint. A failed seal surfaces typed — a pass that cannot
     record its terminal does not claim one.
  4. `crates/gauntlet-app/src/report/mod.rs`: attempted units are
     PROVISIONAL with the record-derived attempt basis
     (`attempted-and-timed-out: 2 dispatches on record, the latest
     invocation terminal is timed_out`; `attempted-and-open` when no
     terminal is recorded); `blocked` units carry their attempt facts
     in the basis; the never-dispatched basis says exactly "the latest
     checkpoint names the unit open; no dispatch attempt for this unit
     is on record" (informational (c): an operator-routed unit's own
     record truth, never a claim about operator routing the store
     cannot see — that boundary is a recorded residual); the string
     "never attempted on record" renders nowhere. `terminal_state` =
     the latest recorded `RunTerminal` event, falling back to the run
     aggregate, else null (informational (b)).
  5. `crates/gauntlet-app/tests/report.rs`: the `ScriptedStore` double
     gained a `facts` field (11 literals updated mechanically; no
     assertion weakened); NEW unit-level teeth
     `an_attempted_unit_is_provisional_with_the_record_derived_attempt_basis`.
- Green: report target 10/10; gauntlet-app package 566/566; ports
  123/123; sqlite 53/53; e2e_budget 2/2 through the real binary; the
  mock E2E's report legs (parsed category pairs from durable records)
  green throughout — a report change that broke them would have been a
  defect in the change.

### F-R3 (minor — FIX) — the cancel-for-recovery arm retires the wrapped tree

- Empirical shape probe (pure sh, pre-test): `sh -c 'sleep 30 & wait'`
  TERM'd at the driver → driver exit 143, background sleep SURVIVED
  (the round-1 leak shape). Group probe: `setsid sh -c 'sleep 40 &
  wait'` + `kill -TERM -PGID` → driver and payload both retired.
- RED-first: new test
  `fr3_a_post_spawn_refusal_retires_the_whole_wrapped_child_tree` (the
  no-tier-registration shape constructed at the adapter level: a
  faithful stand-in for `RecordingAgentRunner::
  cancel_after_persistence_failure` — spawn → record-refusal → cancel
  under the SPEC's deadline → typed `AdmissionRefused` — plus a
  `TreeWrap` seam reproducing the driver+forked-payload leak shape,
  plus a bounded liveness poll).
  - RED against the delivered pid-only stop mechanics, verbatim:
    `panicked at cor2_seam_and_recovery_attacks.rs:378: the post-spawn
    refusal left the wrapped tree alive: driver pid 426979 alive=false,
    wrapped payload pid 426982 alive=true — the cancel-for-recovery arm
    leaked the child`.
- Fix (`crates/adapters/gauntlet-adapter-agent-cli/src/transport.rs`;
  pristine sha256 `1714a833cf1e36ae9bc1fc814fe3181f272cf71f63c2c5ff0f68e81218bdff21`
  recorded before the edit): every spawned child is its OWN process
  group (`process_group(0)`, stable `CommandExt`); both stop legs
  signal the WHOLE GROUP (`kill -TERM/-KILL` to the negated pid),
  fixed-argv, pid-only words; bounded legs and the COR-F1 reap grace
  unchanged; `inner.kill()` stays as the direct-pid backstop.
- Intermediate defect found by the package run itself: the first
  helper spelling used `kill -TERM -- "$1"` — dash's kill builtin
  REJECTS `--` ("Illegal number: -"); the graceful leg burned its
  bound and the round-1 regression `c_cancel_lifecycle_with_real_children`
  went RED (the package caught the regression exactly as designed).
  Corrected to the portable negated-pid spelling, probed on dash first.
- Harness hardening (recorded honestly): with the fix in place, the
  cancel can retire the tree BEFORE the payload writes its pid file —
  a race the FIX made reachable, and a legitimate retirement outcome.
  The test now treats the PROCESS GROUP as the authoritative liveness
  fact (`kill -0 -PGID`: members alive vs ESRCH/empty) with the
  recorded payload pid as the secondary witness; a retired-before-
  announcement outcome passes, an alive group fails.
- RED re-proof of the hardened test: with the transport fix TEMPORARILY
  reverted (reverse edits only), the hardened test went RED at the leak
  locus (`panicked at …:409: the post-spawn refusal left the wrapped
  tree alive … process group still has members=true`); the fix was
  restored byte-identically (sha256 both sides ==
  `1aced2a79326ab88f82a314f5f7464f6d584c23f5416df8c5412b61014ab0c1c`),
  then 5/5 isolated green and full-package green.
- Recorded boundary (not silently claimed): group members cannot
  escape the SIGKILL leg; a grandchild that ignores TERM and whose
  DRIVER dies of TERM can outlive the graceful observation window only
  by having ESCAPED the group (setsid) — beyond this bounded claim,
  recorded as a residual.

### F-R5 (minor — FIX dedup, PIN re-entry, NO flow redesign)

- RED-first: new e2e_budget leg
  `the_escalated_resume_re_affirms_one_ticket_per_fact_and_records_the_re_entry`
  (pass 1: the silent world → `terminal=escalated`; pass 2:
  `gauntlet resume` of the non-settled run). RED verbatim at the dedup
  locus: `assertion left == right failed: a resumed pass re-affirms
  the open AK-953 ticket — never mints a duplicate: [ticket=esc-6a9ccfa1-0 …,
  ticket=esc-6a9ccfab-0 …]` — TWO open tickets, byte-identical
  summaries, exactly the falsifier's two-world observation. The leg's
  visibility assertions (the export's `run_terminal` transition and
  the checkpointed re-spent round) passed before the dedup fix,
  pinning the re-entry semantics honestly — that is the PIN, and the
  flow itself was not redesigned.
- Fix (`crates/adapters/gauntlet-adapter-escalation-fs/src/queue.rs`;
  pristine sha256 `7a0843fbc4e54e57e1db0c200a54c48c57c66af2e44f41a687a25172e81fce2b`
  recorded before the edit): `FsEscalationQueue::escalate` first scans
  the queue (the quarantine-safe `list`) and RE-AFFIRMS an OPEN record
  with the same run_id + summary (the summary is the fact-class
  carrier the port's record shape holds) by returning the existing
  ticket; a RESOLVED ticket never dedups (the operator acted; a
  re-observation after the action is a new fact instance). No port
  change.
- Contract-suite impact (the shared suite pinned the OLD always-mint
  semantics): `the_list_is_total_ordered_oldest_first_and_complete`
  and `a_lying_ticket_file_cannot_clobber_another_tickets_record` used
  identical (run, summary) fixtures where they needed two co-existing
  tickets. Both adapted mechanically (second ticket = a distinct fact;
  the properties under test — list order, the cross-wiring defense —
  untouched), following the COR-F4 mechanical-adaptation precedent
  recorded in the suite itself. NEW suite test
  `the_same_open_fact_reaffirms_one_ticket_and_a_resolved_one_never_dedups`
  binds the new semantics at the contract level.
- Green: escalation-fs 15/15; e2e_budget 2/2 (the resumed report's
  terminal field == the printed terminal == the latest run_terminal
  event; rounds re-spent on the record).

### Informational (a) — RECORD, no fix

The resumed pass minted an invocation id identical to the run id
(id-space collision across record kinds). This is the SEEDED
DETERMINISTIC id generator of the fixture/test surface
(`GAUNTLET_ID_SEED`), not the production IdGenerator (ULID — unique
per kind); the collision is fixture-shaped, and a typed per-kind
namespace would touch the id-port fixture contract for zero production
effect — recorded rather than fixed. The composed loop's behavior is
unaffected (the ids collide across KINDS, never within a kind's own
key space). Recorded here AND in `.agents/reports/e2e-real.md`'s
COR-2 section, as the brief requires.

## Deliverable paths + sha256

Round-touched production files (final tree):

| file | sha256 |
|---|---|
| crates/adapters/gauntlet-adapter-agent-cli/src/transport.rs | `9f7197e42f0edd9df6df0fb270fe6f51093c7b75367f7b9e5e0bb7d9b55de8e7` |
| crates/adapters/gauntlet-adapter-agent-cli/src/lib.rs (doc line) | `220acebb1100cbe75466f549a8bf23842905fb999f74214bcc440bcb6e3fb3af` |
| crates/adapters/gauntlet-adapter-agent-cli/src/runner.rs (UNTOUCHED — restored to round-start `b20960b5…9ee5b0`) | `b20960b5ec66b85b57712bfb38546fc0b72184c63bb8c9b0ce2c8473d39ee5b0` |
| crates/adapters/gauntlet-adapter-escalation-fs/src/queue.rs | `493a16069c753889b1073d495fec9ecd7a48c9a67aaa7fada94acafc29a8f181` |
| crates/gauntlet-ports/src/state_store.rs | `075363abd55bbd9f64eda9f7c0e1c8256caaf3973f128ed6fdfe1e5cdc2f3ba8` |
| crates/gauntlet-ports/src/contract/stubs.rs | `d6d0db4e2405084f350d21246cb17b496631fbc9797f298f59a015261c022a8c` |
| crates/adapters/gauntlet-adapter-state-sqlite/src/encode.rs | `88a84f3a811fd745e2b3f47eeb4e021ce3ab2060e40fd5f05393fe9d497a743b` |
| crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs | `94b79743e3cf95a1d4c24942305b4413f6465a0f0b929973ecff876c6eb1ea7b` |
| crates/gauntlet-app/src/control/controller.rs | `fddf3f5e6b1d809965adfc99bd8ddfc71d56158223d25d26c0bb772a07e07e91` |
| crates/gauntlet-app/src/report/mod.rs | `0ea3ae3d973138f54db48642c5db98b4d3d83c4206d8d78d3b1c6bba18cebc9b` |

Round-touched test files:

| file | sha256 |
|---|---|
| crates/adapters/gauntlet-adapter-agent-cli/tests/cor2_seam_and_recovery_attacks.rs (NEW) | `f30fcc847db4c63b5c6595c4376bdfd95189dde0bc2033545b965480af9e2d66` |
| crates/adapters/gauntlet-adapter-escalation-fs/tests/contract_suite.rs | `40a4a5757b0dd0a27978c3e4d6295a94fed97863ca1542bfa68aa4ebc1dfc170` |
| crates/gauntlet-app/tests/report.rs | `6d0e610688dee468e08a19a88229c97b8ade0f9e752f4a3edb706d4b33c73b72` |
| crates/gauntlet-cli/tests/e2e_budget.rs (b1's file, extended) | `726d04c735854422def08ba3efeddf07b6b1fb613e18b6f9b3634b485d5b7fc9` |

Records:

| file | note |
|---|---|
| `.agents/reports/e2e-real.md` | COR-2 section appended (incl. the informational-(a) recording and residuals) |
| `.agents/reports/task25/cor2-progress.md` | the live checkpoint ledger (incl. the coordinator ACK) |

## Command ledger (condensed; every cargo/just invocation under
`flock .agents/reports/task25/compile.lock -c '<cmd>'` with
`sccache --stop-server` inside the same command)

- `just ruler` — before every phase and at the end: PASS every time
  (final `RULER=0` on the final tree).
- `cargo nextest run -p gauntlet-adapter-agent-cli` — mutant RED
  (60+1), restored 61/61, post-F-R3 62/62, battery 62/62 (x2).
- `cargo nextest run --test e2e_budget` — F-R4 RED (compile fix, then
  verbatim RED), post-F-R4 1/1, F-R5 leg RED, post-F-R5 2/2 (x3 on the
  final tree).
- `cargo nextest run --test e2e_full_loop_real_runner` — 2/2 (x2).
- `cargo nextest run --test e2e_recovery` — 3/3 (x2).
- `cargo nextest run --test e2e_full_loop_mock` — 12/12 (x2).
- `cargo nextest run --test p5_hard_gate` — 4/4 (x2).
- `cargo nextest run --test p7_recovery --test p6_budget_control` —
  7/7 (x2).
- `cargo nextest run --test p4_corrective_loop --test p10_no_false_learning
  --test p9_self_improvement_containment` — 12/12 (x2).
- `cargo nextest run --test operator_actions_no_bypass --test
  escalations_cli` — 3/3 (x2).
- `just check-package gauntlet-cli` / `gauntlet-app` — 0 both.
- `just test-package gauntlet-app` — 566/566 (x2); `gauntlet-ports` —
  123/123 (x2); `gauntlet-adapter-state-sqlite` — 53/53 (x2);
  `gauntlet-adapter-escalation-fs` — 15/15 (x2).
- `just lint` — clippy `-D warnings` + `cargo fmt --check`: PASS on
  the final tree (intermediate failures were MY files: one clippy
  `double_ended_iterator_last` in the new e2e_budget helper, one
  `redundant_closure` in the cor2 test, plus rustfmt reflows of my own
  additions — each fixed, never a gate weakened).
- `just boundaries` — 237/237 on the final tree (one intermediate RED:
  the arch vendor-denylist caught the word "bwrap" in MY new test doc
  comments inside the vendor-free adapter crate — reworded to generic
  "live-sandbox pair / sandbox-driver pair", test re-run green).
- Final consolidated battery (one pass, background, on the FINAL tree):
  e2e_full_loop_real_runner 2/2 · e2e_recovery 3/3 · e2e_budget 2/2 ·
  e2e_full_loop_mock 12/12 · p5 4/4 · p7+p6 7/7 · p4+p10+p9 12/12 ·
  operator+escalations 3/3 · adapter package 62/62 · app 566/566 ·
  ports 123/123 · sqlite 53/53 · escalation-fs 15/15 — ALL GREEN.

## Deviations, incidents, design decisions (with reasons)

1. **The git-restore incident (F-R1)**: the first mutant-restore used
   `git restore runner.rs`, which restored the COMMITTED pre-task-25
   file and dropped the round's uncommitted production changes (caught
   immediately by the sha256 check). Recovery: the file was
   reconstructed byte-exactly from this session's own full read of the
   pristine state; sha256 re-verified `b20960b5…9ee5b0` exactly. Rule
   for the failure log: a mutant is reverted by reversing its exact
   edit, never by git-restoring a file that carries uncommitted task
   work.
2. **F-R4's mechanism**: the report cannot derive attempt facts from
   anything that existed (no typed invocation read; the §5.3.5 record
   is spec-frozen without a unit; the workspace ref is provider-shaped;
   the checkpoint field set is §18.2-frozen). The unit join is recorded
   ONCE, typed, at dispatch (`UnitDispatch`), and the terminal is
   sealed per pass (`RunTerminal`, append-only — which is ALSO what
   makes F-R5's re-terminalization visible). One new typed read with
   the COR-7 fail-closed default. This is a small additive ports
   revision; the brief forbade ports revision only for F-R5's
   ticket↔unit binding, and this one is the honest typed alternative
   to string-parsing provider workspace refs.
3. **F-R5's dedup key**: (run_id, summary) over OPEN tickets — the
   summary is the fact-class carrier the port's record shape holds;
   byte-identical summaries are the same fact (the falsifier's observed
   duplicate class). Summaries that embed per-attempt forensics (the
   row-4 timeout ticket's session reference) are deliberately unique
   per attempt and do not dedup across passes; a typed ticket↔unit
   binding stays the recorded residual (F-R5 item 3's ports revision).
4. **F-R3's test hardening** is described under the finding: the race
   the fix introduced into the harness (retire-before-announcement) is
   a legitimate retirement outcome, and the group-liveness assertion is
   the authoritative check. The RED evidence exists in both harness
   generations.
5. **Pre-existing uncommitted state**: the tree arrived with the b1
   builder's uncommitted task-25 production diff; my rustfmt pass on
   `controller.rs` touched only my own additions (verified: the diff
   grew by exactly my +4 formatting lines; the pre-existing hunks —
   e.g. the F6 `cancel_operation_bound` rename — are content changes
   rustfmt never makes).

## Probe hygiene

- Repo `.gauntlet/policies/`: exactly `.gitkeep`, `contained.json`,
  `cross_cutting.json`, `local.json` (Aug 14–28 mtimes) — UNTOUCHED.
- `tests/fixtures/target-real/` — untouched (Sep 5 delivered mtimes);
  every leg mutation lives in per-test scratch worlds under /tmp.
- No gauntlet/bwrap/sleeper processes survive any world (checked after
  the battery; the fr3 test kills its own tree on failure paths).
- Nothing staged, nothing committed; `.agents/CURRENT_STATUS.yml` and
  `.jolli/**` show only pre-existing modifications from before this
  round (orchestrator-owned, untouched by me).

## Residuals for the record

- Ticket↔unit binding + terminal-refusal posture: F-R5 item 3's ports
  revision — recorded (see `.agents/reports/e2e-real.md` COR-2
  section), not changed.
- Forensics-bearing escalation summaries (row-4 timeout ticket) are
  unique per attempt and do not dedup across passes — deliberate (the
  §10.3 rule-5 note rides the ticket); the dedup covers the
  byte-identical fact class.
- The stop path retires every tree member that has not escaped the
  child's process group; a setsid-escaping, TERM-ignoring grandchild
  whose driver dies of TERM is beyond the bounded claim.
- The F-R3 cancel-for-recovery trigger is unreachable in delivered
  composed runs (a composition defect is required to reach it); the
  retirement mechanics are now bound by the adapter-level test
  regardless of reachability.
- Informational (a): fixture-generator id-space collision across
  record kinds — recorded, no fix (production ids are ULIDs).
