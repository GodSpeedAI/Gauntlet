# Task 25 — COR-4 progress (produce-window recovery round)

Builder: COR-4. Brief: `.agents/reports/task25/cor4-brief.md`.
`just ruler` PASS at session start (2026-09-05). Tree state on entry =
cor3's handoff (uncommitted prior-round work; `git diff --cached` empty).

## Step 1 — diagnosis (COMPLETE — disposition: STOP, per brief item 3)

The executed failure: leg 2's MISSED-WINDOW branch (kill after the
production row is durable, before the settlement event). Traced resume
path for an OPEN unit whose production row is durable above the anchor:

1. **What the replay restores.** `resume_run`
   (crates/gauntlet-app/src/control/controller.rs:1106) runs §18.3 step 1
   via `SettlementService::complete_pending_after_checkpoint`
   (crates/gauntlet-app/src/settle/mod.rs:906): idempotent keyed-settlement
   completion + the A5 orphan re-mint (row missing, event durable). For
   this kill BOTH event and row are durable → the completion is a no-op.
   The checkpoint's projection word says Open → the unit is re-planned.
   Attempts are reconstructed FROM the durable production
   (controller.rs:1344-1355 — the controller's own reading: a durable
   production IS a completed attempt) → the retry is attempt 2 of 3.
2. **What produce keys on.** `produce_unit_evidence`
   (controller.rs:3679) in the composed `BindLocatorContent` world ingests
   the locator artifact's bytes; the idempotence check `already_durable`
   (controller.rs:3824-3833) keys on the CONTENT REF (`evidence_production`
   is write-once per (run, ref), store-side
   crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs:2199 —
   identical facts idempotent, different facts for the same ref refused;
   NOTHING refuses a second row at the same LOCATOR). The re-dispatched
   child writes a fresh per-dispatch workspace path into the artifact
   (tests/fixtures/target-real/agent/child.sh embeds `$workspace`) → fresh
   digest → fresh ref → the check misses → a SECOND production row is
   minted at the locator (controller.rs:3837-3845). Settlement then
   commits over the fresh (non-stale) production.
3. **Where the adoption is missing.** Between the attempt
   reconstruction and the loop handoff (controller.rs:1330-1371) and in
   `plan_and_dispatch` (controller.rs:1713): nothing consults the
   un-consumed durable production to route the open unit into
   verification instead of re-dispatch.
4. **Why adoption cannot be inserted there without contradicting frozen
   rules** — the decisive finding:
   - §16.3 (frozen): "A crash before 1 means the unit is retried as if
     verification never happened." This kill is squarely crash-before-
     step-1 (before the SettlementEvent append). The RETRY is the
     prescribed disposition, pinned by the SETTLED proof
     `a_crash_before_the_settlement_event_retries_as_if_never_verified`
     (crates/gauntlet-cli/tests/p7_recovery.rs:1449ff): "exactly ONE fresh
     dispatch of the owed unit" + exactly-once settlement.
   - C5 anchor rule (crates/gauntlet-app/src/verify/authority.rs:298-312):
     a verification may only anchor at the LATEST durable checkpoint's
     iteration — an older anchor is refused (`IterationUnsupported`;
     forging one is the recorded forged-anchor class).
   - §14.3 staleness (authority.rs:701-718): a production whose iteration
     is below the governed iteration is `evidence_stale` → INCONCLUSIVE
     (pinned by its own tests, authority.rs:1149/1182/1387). The adopted
     pre-crash production can therefore NEVER verify in the resumed
     iteration, and the store's write-once rule refuses re-anchoring the
     old ref at the new iteration; the (run, iteration) observation
     binding is write-once (COR-D2), so re-entering the old iteration
     collides. The ONLY machinery-legal settlement route for the resumed
     unit is a fresh production at a fresh iteration — i.e. the retry's
     re-production is structurally forced, not an accident.
   - Locator-idempotent produce (the brief's option B) contradicts the
     same staleness machinery AND breaks the corrective loop's legitimate
     second-production semantics (p4_corrective_loop; the write-once rule
     is per-ref precisely so re-attempts can produce anew).
   - Anchor-iteration adoption (verify+settle at the pre-booking anchor)
     would extend §18.3 step 1's frozen scope ("complete any half-applied
     settlement idempotently") into a new recovery pipeline (verify +
     settle from durable facts, workspace reconstruction, corroboration
     over durable bundles, §16.1 condition 4 over possibly-absent
     comparisons) — a ruler-level semantic extension the frozen spec does
     not prescribe anywhere (§18.3 step 3 collects only CHECKPOINT
     in-flight contracts; our completed-and-produced invocation is in no
     checkpoint's in_flight).

**Disposition: STOP (brief item 3).** The spec prescribes the retry;
adoption contradicts frozen rules; the leg's exactly-once-PER-LOCATOR
claim on the post-production branch is a ruler-owner question. Behavior
is NOT changed. Executed round work: the deterministic RED leg 2b (the
brief's own reproduction spec, landed as the pin of the ruler question)
+ leg 2's false comment corrected + the load-sensitivity record. Every
other battery target must stay green; leg 2b is intentionally RED and
named as such in the report.

## Step 2 — leg 2b (deterministic missed-window kill), landed RED

Execution found TWO facts the brief's leg-2b premise did not anticipate,
both probed before landing anything:

1. **The brief's 20 ms poll cannot land the kill in the branch.** First
   execution of the specified poll (`artifact_durable &&
   production_rows_for_unit >= 1`, TICK=20 ms) PASSED on the current
   tree. Probe dump (temp instrument, since removed): AK-951's
   production event (offset 4) and its settlement event (offset 5) are
   ADJACENT offsets in the SAME wall-clock second — the produce→verify→
   settle span is record-derived (no process spawn at verify time; the
   check tool ran at observation time) and closes in a few milliseconds.
   The poll noticed the production row only after the settlement event;
   the kill landed post-settlement; the resume did not re-dispatch
   (settled units are never re-dispatched) and the claim held honestly.
2. **A zero-sleep busy poll CAN win the race** — measured 12/13 across
   trials (two 3-run probes with the temp instrument, then 1 solo RED,
   then 5/5 RED): the kill lands inside the branch (production durable,
   §16.3 settlement event absent) and the assertion fires with the
   closure failure's exact shape. A second probe (second resume of the
   terminal run) confirmed there is NO alternative deterministic route:
   the shipped budget exhausts rounds before a produced-open unit
   (AK-952) would ever be re-dispatched on a plain re-resume.

**The landed leg** (bound: the branch's span is externally unplaceable
EVERY time — the leg is honest-mechanics deterministic): busy bounded
poll on the production row → SIGKILL → POST-KILL classification from
durable state (the §16.3 settlement-event count is the branch
discriminator: absent = crash-before-step-1 = the branch this leg
discharges; durable = post-settlement = honestly retryable) → bounded
3-round placement (a round that provably missed asserts nothing; total
miss <1% at the measured rate, and even that fires this leg's own typed
placement failure, never a false pass) → on a hit: the operator's
stale-lock dance, resume, and the exactly-once-per-locator claim.

**Verbatim RED (solo run, exit 100):**

```text
FAIL [  20.654s] (1/1) gauntlet-cli::e2e_recovery the_kill_after_the_production_record_pins_the_resume_exactly_once_claim
thread 'the_kill_after_the_production_record_pins_the_resume_exactly_once_claim' (1551910) panicked at crates/gauntlet-cli/tests/e2e_recovery.rs:776:9:
assertion `left == right` failed: the locator's production is durable EXACTLY once after the resume:
  [("sha256:6fcf2afc23e51b6b910f7f8c9ab50f761c31f08c400aa4b013cf97606ed27890", "evidence/real-marker-report.json", 1788699463, 4),
   ("sha256:53596cedea4902f94c88fd04d83d3d17e7a4701427537c0feeab03dd417899a3", "evidence/real-marker-report.json", 1788699473, 11),
   ("sha256:771cff9fd31d2352e159192efb12793abcb1bf5d5e89011613098a3bcc551631", "evidence/real-local-report.json", 1788699479, 16)]
  left: 2
 right: 1
Summary [  20.658s] 1 test run: 0 passed, 1 failed, 3 skipped
```

Same defect signature as the orchestrator's closure failure (two
`real-marker-report` rows at offsets 4 and 11; left: 2, right: 1).

Full-target state (legs 1, 2, 3 PASS; leg 2b RED by diagnosis — the
pinned ruler question):

```text
PASS [  16.216s] the_kill_at_the_first_settlement_boundary_resumes_to_exactly_once
PASS [  18.879s] the_contract_path_poisoning_reaches_a_bounded_terminal_and_resume_terminates
PASS [  21.204s] the_kill_in_the_produce_window_resumes_with_the_production_recovered
FAIL [  21.345s] the_kill_after_the_production_record_pins_the_resume_exactly_once_claim
Summary [  21.362s] 4 tests run: 3 passed, 1 failed, 0 skipped
```

Leg 2's false comment ("the assertions hold either way") corrected to the
now-true statement of both branches; the load-sensitivity record is in
leg 2b's docs (brief disposition 4). `just ruler` PASS after the phase.
All temp probes removed; probe worlds live only in /tmp scratch dirs.

## Step 3 — remaining battery + report

Battery executed (every cargo/just invocation under
`flock .agents/reports/task25/compile.lock -c '…'` with
`sccache --stop-server` inside; default threading):

```text
just ruler                                   PASS (start, after each phase)
just check-package gauntlet-cli              Finished (clean)
just check-package gauntlet-app              Finished (clean; 2m44s)
cargo nextest run --test e2e_full_loop_real_runner   2/2 PASS
cargo nextest run --test e2e_budget          3/3 PASS
cargo nextest run --test e2e_full_loop_mock  12/12 PASS
cargo nextest run --test p7_recovery --test p6_budget_control   7/7 PASS
cargo nextest run --test p4_corrective_loop  4/4 PASS
cargo nextest run -p gauntlet-adapter-agent-cli                 63/63 PASS
cargo nextest run -p gauntlet-adapter-escalation-fs             18/18 PASS
just lint                                    PASS (after cargo fmt -p gauntlet-cli
                                             rewrote ONLY my e2e_recovery.rs —
                                             mtime proof: every other file in the
                                             package pre-session)
just boundaries                              237/237 PASS
cargo nextest run --test e2e_recovery        3 PASS + 1 RED (leg 2b — the pinned
                                             ruler question; see step 2)
just test (full workspace, final tree)       (executing; expected RED solely on
                                             leg 2b — recorded verbatim in the
                                             report)
```

Disposition-4 record: `b1_contract_wins_over_exit7_and_process_is_never_
consulted` PASSED inside this battery's agent-cli run (63/63, slot 26/63)
— the load-flaky class did NOT recur; the round's own red is leg 2b's
diagnosed claim violation, not the flake.

## Step 4 — the full closure gate on the final tree (executed once)

`just test` (full workspace): **1548 run, 1546 passed, 2 FAILED** — both
in e2e_recovery, both the SAME adjudicated claim:

- leg 2b (`the_kill_after_the_production_record_pins_the_resume_
  exactly_once_claim`) — the landed pin, RED by diagnosis;
- leg 2 (`the_kill_in_the_produce_window_resumes_with_the_production_
  recovered`) — failed under the full run's load EXACTLY as it did in
  the orchestrator's closure battery: two marker rows (offsets 4 and
  11), left: 2, right: 1, verbatim at `e2e_recovery.rs:617:5` (its
  unchanged assertion).

`JUST_TEST_EXIT=100`. No other failure anywhere in the workspace. This
is the honest STOP-round terminal: the gate cannot be green without
either the fix the spec forbids or the ruler-side assertion change that
belongs to the orchestrator. Full case: `cor4-report.md`.

## Step 5 — final hygiene proofs

- `just ruler` final PASS.
- Session mtime scan (`find . -newermt 2026-09-06 08:00`, repo scope,
  excluding .git/target/.jolli/.tmp/mutants.out/.agents prompts+agents):
  exactly `crates/gauntlet-cli/tests/e2e_recovery.rs` and this progress
  file (+ the report) — ZERO production files.
- Final digest: `sha256(e2e_recovery.rs) =
  bf3c0a45b76f018141a2e66e23b35b9b5ce181afe3292c369264db09afac4f2a`.
- `git diff --cached` empty; HEAD `c7eca24` (task-24 settlement).
  Nothing staged, nothing committed.
- `.gauntlet/policies/` ls proof: `contained.json`, `cross_cutting.json`,
  `local.json` (+ `.gitkeep`) only, mtimes Aug 14-28 — nothing planted.
- Round files: `cor4-progress.md`, `cor4-report.md` — the completion
  contract. ROUND COMPLETE (STOP disposition).

## RESUME (orchestrator adjudication: disposition (a), re-scope, tests-only)

The orchestrator verified the two load-bearing citations (p7's
retry-as-if-never-verified pin; the staleness machinery tests) and took
disposition (a): the leg's per-locator exactly-once claim was stronger
than the spec; re-scope the assertions, no production change.

### R1 — leg 2b re-scoped to the adjudicated assertions

- exactly ONE fresh dispatch on resume: `ak951_dispatch_count` (export
  `role_invocation` records naming AK-951) == 2 across both passes;
- the pre-crash production row intact byte-level: the post-kill tuple
  `(ref, produced_at, offset)` still present post-resume (write-once);
- the resumed pass's OWN per-iteration production beside it: marker rows
  == 2, freshest ref != crashed ref;
- exactly-once SETTLEMENT over the CURRENT ref: one keyed row + one
  event for AK-951, the row's evidence ref == the freshest marker ref;
  total settlement events == 2 (AK-951 + AK-954);
- run identity preserved (export settlement record names AK-951); prior
  artifact bytes survive.
- docs: the adjudication + the ruler-owner question verbatim.

### R2 — leg 2 re-scoped (branch-aware, all three landings honest)

Post-kill durable facts (`rows_before`, `ak951_settled_before`) drive the
assertions: pre-crash rows survive untouched; if the unit was unsettled
at the kill → exactly one fresh dispatch (== 2 total) and exactly one new
minted row (before+1); if already settled → never re-dispatched (== 1
total), nothing new mints. Exactly-once settlement (row + event for
AK-951) over the CURRENT (freshest) ref in both cases; total events == 2;
run identity + prior bytes preserved. The per-locator count==1 assertion
is GONE. Header docs state the adjudicated both-branch semantics + the
ruler-owner question verbatim; the branch comment now points at the
post-kill-fact-driven assertions.

`just ruler` PASS before the resume phase. check-package gauntlet-cli
clean. (target run + battery + full just test next)

### R3 — first target run exposed a wrong contract assumption (probed, then resolved as finding F-C4-1)

The contract's "exactly ONE fresh dispatch on resume" clause FAILED
twice (both legs): the durable AK-951 dispatch count was 3, not 2.
Probe dumps (temp eprintln instruments, removed) over the export stream
showed the resumed pass dispatching AK-951 at iterations 2 AND 3, with
no failure record and no terminal event for the first one:

- iteration 2's re-dispatch inherits the CRASHED attempt's workspace
  (`.AK-951.000000` — the workspace provider's counter restarts with the
  process), so the child rewrites BYTE-IDENTICAL artifact bytes; the
  produce step dedupes onto the pre-crash row (`already_durable`);
  no new production event; the §14.3 staleness rule then classes the
  attempt `evidence_stale` → INCONCLUSIVE → the unit stays open;
- iteration 3's §11.5 re-attempt gets a FRESH workspace (`.000002`) →
  new bytes → own production at its own iteration → verifies → settles.

So the resume delivers TWO fresh dispatches (3 total = the §11.5
attempts ceiling), deterministically, when the crashed attempt's row
already exists; ONE fresh dispatch when it does not (the true in-window
landing: the re-dispatch's produce mints fresh at its own iteration and
settles directly); ZERO when the unit was already settled at the kill.
Under the adjudicated semantics this is honest, ceiling-bounded §11.5
behavior — but it means a fixture with attempts ceiling 2 would ESCALATE
AK-951 instead of settling it. Recorded as **finding F-C4-1** in the
report (with the adjacent observation that the resumed pass's first
drawn invocation id equals the run id — the id generator's sequence also
restarts with the process; benign for uniqueness, recorded beside it).
The contract clause was implemented as the honest durable count, never
silently weakened.

### R4 — re-scoped target state

- leg 2b: re-scoped assertions landed; solo + full-target + 2 stability
  runs all PASS (4/4 target, repeatedly).
- leg 2: re-scoped branch-aware assertions (rows_before/settled_before
  drive the expected counts); PASS.
- Battery on the final tree: check gauntlet-app 0; real_runner 2/2;
  budget 3/3; mock 12/12; p7+p6 7/7; p4 4/4; agent-cli 63/63;
  escalation-fs 18/18; lint PASS (fmt -p gauntlet-cli touched only my
  file — mtime proof); boundaries 237/237.
- `just ruler` PASS before the full `just test`.

### R5 — full `just test` on the final tree

**`just test`: 1548 run, 1548 passed, 0 skipped — exit 0.** Both
recovery legs green under the adjudicated semantics (leg 2 held through
the full run's load — its branch-aware assertions are landing-honest).
`b1_contract_wins_over_exit7_and_process_is_never_consulted` green
inside the full run (no flake recurrence). `just ruler` final PASS.

### R6 — final hygiene

- Final digest: `sha256(e2e_recovery.rs) =
  2612be51d9e9ee93830295ec79a57eb6678bd6637b864877d766d0672206d187`.
- Session mtime scan: only `crates/gauntlet-cli/tests/e2e_recovery.rs` +
  the three report files (cor4-progress/cor4-report + the orchestrator's
  own IN-FLIGHT.md, which this round never touched). ZERO production
  files.
- `git diff --cached` empty; HEAD `c7eca24`. Nothing staged, nothing
  committed.
- `.gauntlet/policies/` ls proof: `contained.json`, `cross_cutting.json`,
  `local.json` (+ `.gitkeep`) only.
- RESUME ROUND COMPLETE (adjudication disposition (a) executed).
