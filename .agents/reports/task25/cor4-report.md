# Task 25 — COR-4 report (produce-window recovery round — STOP disposition, then ORCHESTRATOR ADJUDICATION executed)

Builder: COR-4. Binding brief: `.agents/reports/task25/cor4-brief.md`.
Tree inherited: `.agents/reports/task25/cor3-report.md` (uncommitted
prior-round work; `git diff --cached` empty; HEAD `c7eca24`, the task-24
settlement commit). Date: 2026-09-06.

**`just ruler` PASS at session start, after every phase, and at the end
(both the STOP round and the adjudication resume round).**

## ADJUDICATION (round 2, executed on top of the STOP disposition)

The orchestrator verified the two load-bearing citations of the STOP
case (p7_recovery.rs's retry-as-if-never-verified pin;
verify/authority.rs's prior-iteration staleness machinery) and took
**disposition (a): re-scope, no production change** — the leg's
per-locator exactly-once claim was stronger than the spec. The resume
contract: re-scope legs 2 and 2b to the spec-honest semantics (bounded
re-dispatch — no storm; pre-crash rows intact; the resumed pass's own
per-iteration production; exactly-once settlement over the current ref;
run identity; prior bytes), correct the docs, name the ruler-owner
question verbatim, and run the battery + one full `just test` with both
recovery legs green.

**Executed, all green.** Final tree:
`sha256(crates/gauntlet-cli/tests/e2e_recovery.rs) =
2612be51d9e9ee93830295ec79a57eb6678bd6637b864877d766d0672206d187`.

### What the re-scope found: finding F-C4-1 (the one contract clause that is not what the machinery does)

The contract specified "exactly ONE fresh dispatch on resume". The
shipped machinery delivers TWO fresh resumed dispatches whenever the
crashed attempt's production row already exists (the missed-window
branch), deterministically. Probe-backed mechanism (export-stream event
dumps, instruments removed after use):

1. the resume's FIRST re-dispatch inherits the CRASHED attempt's
   workspace (`...AK-951.000000` — the WorkspaceProvider's counter
   restarts with the process, so the first resumed allocation reproduces
   the pre-crash ref);
2. the child therefore rewrites BYTE-IDENTICAL artifact bytes (the
   workspace path is embedded in the artifact); the produce step's
   `already_durable` dedupes onto the pre-crash row — no new production
   event, no mint;
3. the §14.3 staleness rule classes the only (pre-crash-iteration)
   production `evidence_stale` at the resumed iteration → INCONCLUSIVE →
   the unit stays open;
4. §11.5 re-attempts ONCE more (iteration 3, fresh workspace
   `.000002`, new bytes, own production at its own iteration) — which
   verifies and settles.

So the durable AK-951 dispatch count across both passes is: 3 (1 crashed
+ 2 resumed) when the crashed attempt's row existed; 2 (1 + 1) when it
did not (the true in-window landing — the re-dispatch's produce mints
fresh and settles directly); 1 when the unit was already settled at the
kill (settled work never re-dispatched). All three shapes are asserted
per-branch; the count is ceiling-bounded (GAUNTLET_MAX_ATTEMPTS_PER_UNIT
= 3 in the fixture world) and never a storm.

F-C4-1's cost, recorded honestly: the wasted middle attempt burns one
§11.5 attempt — with an attempts ceiling of 2, AK-951 would ESCALATE
instead of settling on this crash class. Root causes, both frozen-surface
facts: the workspace provider's per-process counter restart (the resume
cannot allocate a fresh workspace for its first re-dispatch) × the §14.3
staleness economics the adjudication itself named. Adjacent observation,
same root: the resumed pass's FIRST drawn invocation id equals the run
id (the IdGenerator's sequence also restarts; benign for uniqueness —
recorded so nobody re-discovers it by surprise).

The leg assertions pin the durable counts per branch (the contract's
"assert the durable invocation/dispatch count" clause), name F-C4-1 in
the assertion messages and leg docs, and the finding is recorded here
for the orchestrator — per the resume contract's own rule: a finding to
report, never a skip, and never a silent weakening. If the orchestrator
wants ONE-fresh-dispatch semantics, the CandidateRevision lever is the
workspace provider's resume-time counter (fresh workspaces after a
restart), not the recovery path.

### The re-scoped assertions as landed

Leg 2b (the deterministic-class missed-window pin; classified
crash-before-settlement-event landing): AK-951 dispatches == 3; the
crashed row's exact `(ref, produced_at, offset)` tuple survives
untouched; marker rows == 2 with the freshest differing from the crashed
ref; exactly one settlement row + one settlement event for AK-951, the
row's evidence ref == the freshest production ref; total settlement
events == 2 (AK-951 + AK-954); the export answers for the same run with
the settlement on its own durable record; prior artifact bytes survive.

Leg 2 (branch-aware, all three landings honest — asserted against
POST-KILL durable facts `rows_before` / `ak951_settled_before`, never
guessed): pre-crash rows survive untouched; unsettled-before → dispatches
== 1 + (rows_before empty ? 1 : 2) and rows_after == rows_before + 1;
settled-before → dispatches == 1 and nothing mints; exactly-once
settlement (row + event) over the freshest ref in every landing; total
events == 2; run identity + prior bytes preserved. The per-locator
count==1 assertion is GONE.

Both legs' docs carry the adjudication pointer (cor4-report.md, this
section) and the ruler-owner question verbatim: "should §18.3 recovery
adopt pre-crash productions (changing the §14.3 staleness economics)? —
CandidateRevision, reference-candidates at task25 settlement."

### Resume-round battery (final tree; every invocation flock'd, sccache stopped inside, default threading)

```text
just ruler                                PASS (start, per phase, end)
just check-package gauntlet-cli           Finished (clean)
just check-package gauntlet-app           Finished (clean)
cargo nextest run --test e2e_recovery     4/4 PASS (both recovery legs under
                                          the adjudicated semantics; leg 2b
                                          also re-run solo twice: PASS, PASS)
cargo nextest run --test e2e_full_loop_real_runner   2/2 PASS
cargo nextest run --test e2e_budget       3/3 PASS
cargo nextest run --test e2e_full_loop_mock          12/12 PASS
cargo nextest run --test p7_recovery --test p6_budget_control   7/7 PASS
cargo nextest run --test p4_corrective_loop  4/4 PASS
cargo nextest run -p gauntlet-adapter-agent-cli      63/63 PASS
cargo nextest run -p gauntlet-adapter-escalation-fs  18/18 PASS
just lint                                 PASS (clippy -D warnings + fmt --check)
just boundaries                           237/237 PASS
just test  (the closure gate, final tree) 1548 run, 1548 PASSED, exit 0 —
                                          both recovery legs green under the
                                          full run's load; b1_contract green
                                          (no flake recurrence)
```

The original defect's lifecycle is now fully pinned: leg 2 races the
real window with landing-honest assertions; leg 2b places the
missed-window branch deterministically-class (busy bounded poll +
post-kill durable classification); both legs pass under the adjudicated
semantics on every landing, in isolation and under full-workspace load.

---

# Round 1 (the STOP disposition, superseded in disposition by the adjudication; the diagnosis below remains the verified case)

## Disposition header

**STOP, per brief disposition 3 — invoked with the diagnosis proven.**
No production behavior was changed: zero files under `crates/` other than
the test file named below were touched (mtime-scanned proof below). The
brief's STOP condition fired:

> if the spec actually prescribes re-production, stop and document
> instead of changing behavior

The diagnosis proves the frozen spec (§16.3) and its SETTLED proof pin
re-production (the retry) for exactly this crash class, and both fix
shapes the brief offers contradict frozen rules. The leg's
exactly-once-PER-LOCATOR claim, extended to the missed-window branch, is
therefore a **ruler-owner question**, not a code defect. Executed round
work: the deterministic-class RED pin (leg 2b, per the brief's own leg
spec), leg 2's false comment corrected, the load-sensitivity record
landed, the full battery run, and this report.

## Mission echo

Fix the defect the orchestrator's closure battery executed: leg 2 of
`crates/gauntlet-cli/tests/e2e_recovery.rs`
(`the_kill_in_the_produce_window_resumes_with_the_production_recovered`)
failed under full-workspace load with two durable production rows for one
locator after a post-production SIGKILL — the resume re-dispatched the
open unit and re-produced at its locator instead of adopting the orphan
production. Diagnose RED-first; fix with the smallest spec-consistent
recovery change; STOP if the spec prescribes re-production.

## The diagnosis (brief disposition 1, required)

The executed failure is leg 2's MISSED-WINDOW branch: the kill landed
after AK-951's production row was durable but before its §16.3
settlement-event append. Traced resume path for an OPEN unit whose
production row is durable above the checkpoint anchor:

1. **What the event-log replay restores.** `RunController::resume_run`
   (`crates/gauntlet-app/src/control/controller.rs:1106`) executes
   §18.3 step 1 via
   `SettlementService::complete_pending_after_checkpoint`
   (`crates/gauntlet-app/src/settle/mod.rs:906`): the idempotent keyed
   settlement completion plus the A5 orphan re-mint (row missing, event
   durable). For this kill both event and row are already durable, so
   the completion is a no-op. The checkpoint's projection word says
   Open, so the unit is re-planned. Attempts are reconstructed FROM the
   durable production (`controller.rs:1344-1355` — the controller's own
   reading: a durable production IS a completed attempt), making the
   retry attempt 2 of 3.
2. **What the produce step keys on.** `produce_unit_evidence`
   (`controller.rs:3679`) in the composed `BindLocatorContent` world
   ingests the locator artifact's bytes and checks `already_durable`
   (`controller.rs:3824-3833`) — keyed on the CONTENT REF. The store's
   write-once production rule is per-(run, ref), never per-locator
   (`crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs:2199`:
   identical facts idempotent, different facts for the same ref
   refused; nothing refuses a second row at the same locator — by
   design, the corrective loop legitimately re-produces at a locator
   after a residual). The re-dispatched fixture child writes a fresh
   per-dispatch workspace path into its artifact
   (`tests/fixtures/target-real/agent/child.sh` embeds `$workspace`),
   so the bytes differ, the digest differs, the ref is fresh, the check
   misses, and a SECOND production row is minted at the locator
   (`controller.rs:3837-3845`). Settlement then commits over the fresh
   (non-stale) production — the run settles honestly, but the locator
   now carries two production rows. Exactly the executed dump: marker
   rows at offsets 4 and 11.
3. **The exact locus where adoption is missing.** Between the attempt
   reconstruction and the loop handoff (`controller.rs:1330-1371`) and
   in `plan_and_dispatch` (`controller.rs:1713`): nothing consults the
   un-consumed durable production to route the open unit into
   verification instead of re-dispatch.
4. **Why the adoption cannot be inserted there — the frozen walls.**
   This is the decisive finding, each wall frozen and test-pinned:
   - **§16.3 prescribes the retry.** "A crash before 1 means the unit
     is retried as if verification never happened." This kill is
     squarely crash-before-step-1 (before the SettlementEvent append).
     The SETTLED proof
     `a_crash_before_the_settlement_event_retries_as_if_never_verified`
     (`crates/gauntlet-cli/tests/p7_recovery.rs:1449ff`) pins exactly
     this disposition: "exactly ONE fresh dispatch of the owed unit"
     plus exactly-once settlement. p7 is settled surface; its semantics
     are the ruler's.
   - **C5 anchor rule** (`crates/gauntlet-app/src/verify/authority.rs:298-312`):
     a verification may only anchor at the LATEST durable checkpoint's
     iteration (`IterationUnsupported` otherwise); forging an older
     anchor is the recorded forged-anchor class. Adoption at the
     ORIGINAL iteration is therefore impossible after the resumed pass
     books its first checkpoint.
   - **§14.3 staleness rule** (`authority.rs:701-718`, pinned by its own
     tests at 1149/1182/1387): a production whose iteration is below
     the governed iteration classes `evidence_stale` → INCONCLUSIVE.
     Adoption at the RESUMED iteration can therefore never verify the
     pre-crash production — the unit would never settle, breaking the
     leg's own settlement assertion and p7's pinned retry-to-settlement
     semantics.
   - **Write-once production rule** (store-side, above): the old ref
     cannot be re-recorded at the new iteration. **COR-D2's write-once
     observation binding** per (run, iteration): re-entering the old
     iteration collides.
   - **§18.3's resume powers are frozen**: step 1 completes half-applied
     SETTLEMENTS (plus the A5 row re-mint); step 3 collects CHECKPOINT
     in-flight result contracts. The completed-and-produced invocation
     of this window is in neither (the top-of-iteration checkpoint
     precedes its dispatch). §18.3 prescribes re-planning open units.
     An anchor-iteration adoption (verify+settle at the pre-booking
     anchor, where C5/staleness would NOT object) would extend §18.3
     step 1's frozen scope into a new recovery pipeline — workspace
     reconstruction from the event log, corroboration over durable
     bundles, and §16.1 condition 4 over possibly-absent comparisons —
     a ruler-level semantic extension the frozen spec prescribes
     nowhere, not a corrections-sized fix.
   - **The brief's option B (locator-idempotent produce)** contradicts
     the same staleness machinery (the re-bound old production verifies
     stale at the new iteration) and destroys the corrective loop's
     legitimate re-production semantics (p4_corrective_loop is in the
     battery and stayed green).

**Conclusion:** re-production is not an accident — it is the ONLY
machinery-legal settlement route for the resumed unit, and the frozen
spec prescribes the retry that causes it. The leg's exactly-once claim
on the missed-window branch contradicts the ruler; per the brief I
stopped before changing behavior and left the adjudication to the
orchestrator.

## Executed work

### 1. Leg 2b — the deterministic-class RED pin (landed, RED by diagnosis)

`the_kill_after_the_production_record_pins_the_resume_exactly_once_claim`
in `crates/gauntlet-cli/tests/e2e_recovery.rs`, built to the brief's leg
spec (poll the production row, SIGKILL, resume, assert exactly-once) —
with two executed corrections to its mechanics, each probe-backed:

- **The brief's 20 ms poll cannot land the kill in the branch.** First
  execution PASSED on the defective tree. Probe evidence (temp
  instrument, removed after): AK-951's production event (offset 4) and
  settlement event (offset 5) are ADJACENT offsets in the same
  wall-clock second — the produce→verify→settle span is record-derived
  (the check tool runs at observation time) and closes in milliseconds,
  faster than any external poll's reaction. A second probe ruled out an
  alternative deterministic route (a plain second resume of the
  terminal run never re-dispatches the produced-open AK-952 — the
  shipped budget exhausts rounds first).
- **A zero-sleep busy bounded poll wins the race 12/13** (measured
  across five solo executions plus one full-target run). The landed leg
  therefore busy-polls the production row (a durable fact, never a
  sleep; hard `BOUNDARY_BOUND` with a typed failure), SIGKILLs, then
  CLASSIFIES the landing AFTER the kill from durable state: the §16.3
  settlement-event count. Event absent = crash-before-step-1 = the
  branch this leg discharges (the claim fires); event durable =
  post-settlement = precondition gone (bounded 3-round retry, a
  provable miss never asserts). Total miss at the measured rate <1%,
  and even then the failure is the leg's own typed placement failure —
  never a false pass.

**Verbatim RED (solo run, exit 100), same signature as the closure
failure:**

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

The brief's "must PASS after the fix" is moot under STOP — the report is
the handoff; the orchestrator re-adjudicates (below).

### 2. Leg 2's false comment corrected

The false "the assertions hold either way" branch comment is replaced by
the now-true statement: on the missed-window branch the §16.3 resume
retries the open unit and the retried attempt produces anew, so the
per-locator exactly-once claim does NOT hold there, with a pointer to
leg 2b and this report. The assertion itself is untouched (not mine to
move).

### 3. Load-sensitivity record (brief disposition 4)

Recorded in leg 2b's docs: leg 2's real produce window is
sub-millisecond on this host, its branch is load-sensitive (passed every
prior battery in-window; failed only under the orchestrator's
full-workspace load), leg 2b is the regression pin, and the original leg
keeps racing the real window. Disposition 4's other record:
`b1_contract_wins_over_exit7_and_process_is_never_consulted` PASSED in
this round's agent-cli battery (63/63, slot 26/63) AND in the final full
`just test` — the load-flaky class did NOT recur.

## Battery (final tree; every cargo/just invocation
`flock .agents/reports/task25/compile.lock -c '…'`, `sccache --stop-server`
inside, default threading)

```text
just ruler                                PASS (start, after each phase, end)
just check-package gauntlet-cli           Finished (clean)
just check-package gauntlet-app           Finished (clean)
cargo nextest run --test e2e_recovery     3 PASS + 1 RED (leg 2b — the pinned
                                          ruler question; see above)
cargo nextest run --test e2e_full_loop_real_runner   2/2 PASS
cargo nextest run --test e2e_budget       3/3 PASS
cargo nextest run --test e2e_full_loop_mock          12/12 PASS
cargo nextest run --test p7_recovery --test p6_budget_control   7/7 PASS
cargo nextest run --test p4_corrective_loop  4/4 PASS
cargo nextest run -p gauntlet-adapter-agent-cli      63/63 PASS
cargo nextest run -p gauntlet-adapter-escalation-fs  18/18 PASS
just lint                                 PASS (clippy -D warnings + fmt --check;
                                          after cargo fmt -p gauntlet-cli rewrote
                                          ONLY my e2e_recovery.rs — mtime proof:
                                          every other file in the package is
                                          pre-session)
just boundaries                           237/237 PASS
just test  (the closure gate, final tree) 1548 run: 1546 passed, 2 FAILED —
                                          both e2e_recovery, both the SAME
                                          claim: leg 2b (the pin) and leg 2
                                          itself, which failed under the full
                                          run's load exactly as it did in the
                                          orchestrator's closure battery
                                          (two marker rows, offsets 4 and 11,
                                          left: 2, right: 1 — verbatim in
                                          cor4-progress.md step 3)
```

**The workspace gate on this tree is RED solely on the adjudicated
claim** — there is no other failure anywhere in the workspace, and the
two red legs are the same diagnosed question, not independent defects.
This is the honest terminal for a STOP round: the brief's green-battery
rule cannot be met without either the fix the spec forbids or an
assertion change that belongs to the ruler's owner.

## The orchestrator's decision menu (recorded, not chosen)

1. **Re-scope the claim (ruler-side).** The spec-honest claim is: the
   RECOVERY half never duplicates (the A5 re-mint is keyed, idempotent,
   cross-validated — already pinned by the mock's COR-R4-1 legs), the
   §16.3 retry is exactly-one-fresh-dispatch (p7-pinned), and the
   retried attempt's fresh production is a NEW attempt's evidence (per-
   ref write-once; per-locator multiplicity is legitimate). Leg 2/2b's
   assertions would then be reworded to pin THAT (e.g. the pre-kill
   row's (ref, ts, offset) survives the resume byte-identical and
   un-duplicated, plus at most one new production per attempt) — the
   assertion change is deliberate, ruler-owned, and non-silent.
2. **Commission the §18.3-step-1 extension (CandidateRevision).** A
   recovery-side verify+settle at the pre-booking anchor over the
   adopted production — machinery-legal (C5/staleness satisfied at the
   anchor) but a NEW recovery semantic with a §16.1-condition-4 design
   question (residuals over possibly-absent comparisons). This changes
   p7-pinned retry semantics and needs the ruler moved first.
3. Leg 2b stays as the deterministic-class pin either way; its
   classification machinery (post-kill durable branch detection) makes
   it usable under both dispositions.

## Deviations and disclosures (none silent)

1. **STOP disposition taken** — no production change, by brief item 3.
   The battery is therefore red on exactly the adjudicated claim.
2. Leg 2b's placement is a **measured 12/13 race with post-kill durable
   classification**, not the pure determinism the brief specified — the
   brief's premise (an external poll can always react inside the
   produce→settle span) was falsified by execution; the leg's docs carry
   the full mechanics and the measured rate.
3. `cargo fmt -p gauntlet-cli` rewrote exactly one file — my
   `e2e_recovery.rs` (joined one boolean line rustfmt wanted). Proof:
   package-wide mtimes after fmt — every other file pre-session
   (`compose.rs` Sep 5 12:52, `improve_cmd.rs` Sep 5 08:31,
   `compose_teeth.rs` Sep 5 14:38, `context.rs` Aug 31; mine Sep 6 09:27).
4. Temp probe instruments (eprintln dumps, a probe second-resume) lived
   only inside the leg during execution and were REMOVED before the
   final landing; probe worlds ran only in /tmp scratch dirs.

## Tree integrity and probe hygiene

- **Files this round changed (complete list — the mtime scan
  `-newermt 2026-09-06 08:00` over the repo, excluding
  .git/target/.jolli/.tmp/mutants.out/prompts/agents, returns exactly):**
  - `crates/gauntlet-cli/tests/e2e_recovery.rs` (test-only: leg 2b + leg
    2 comment) — sha256
    `bf3c0a45b76f018141a2e66e23b35b9b5ce181afe3292c369264db09afac4f2a`
  - `.agents/reports/task25/cor4-progress.md` (new, checkpoint record)
  - `.agents/reports/task25/cor4-report.md` (this file)
  - NO production file was touched. The M entries under
    `crates/gauntlet-app/**`, `crates/adapters/**`, `crates/gauntlet-ports/**`
    in `git status` are the PRIOR rounds' uncommitted work (present in
    the session's entry `git status`, mtimes pre-session).
- **No-touch list honored**: repo-root `.wayfinder/**`, repo
  `.gauntlet/**`, repo-root `GAUNTLET.md`, settled `p*.rs`/`g*.rs`/
  `antigaming.rs`/`e2e_full_loop_mock.rs` (READ as prior art, never
  edited — p7's crash-before proof and the mock's COR-R4-1 legs are
  cited above), `.agents/CURRENT_STATUS.yml` — never opened for edit.
- **Repo `.gauntlet/policies/` ls proof**: exactly `contained.json`,
  `cross_cutting.json`, `local.json` (+ `.gitkeep`), mtimes Aug 14-28 —
  nothing planted; all probes ran in /tmp scratch worlds.
- **Nothing staged, nothing committed**: `git diff --cached` empty;
  HEAD still `c7eca24` ("status: task24 settled (64267b4) — ready set
  task25"). Working tree carries 34 entries, all prior-round work plus
  this round's three files.
- **Reaping**: every killed/completed child is reaped by
  `kill_and_collect`/`wait_bounded` as before; the full suite left no
  survivors (boundaries 237/237 green, the process-tree scanners).
- **No new dependencies**; no sleep-and-hope (every wait is a bounded
  poll on a durable fact, including leg 2b's busy poll with its typed
  deadline failure).

## Honest residue

- The claim violation is REAL and now reproducible on demand (leg 2b);
  what is adjudicated is whether the CLAIM or the CODE moves. Both
  resolutions are above the corrections round's authority — the brief's
  STOP branch applies literally.
- Leg 2, once the claim is re-scoped, should be re-worded to the new
  pin; its current form (the exactly-once assertion + the corrected
  branch comment) is internally honest — the comment no longer promises
  what the assertion cannot hold.
- The full-gate reds are load-coupled (leg 2b's busy polling widens
  sibling windows). An orchestrator wanting a quiet gate before
  adjudication may `#[ignore]` leg 2b deliberately — recorded here as
  their option, never done silently by this round.
