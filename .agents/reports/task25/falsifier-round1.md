# Task 25 — blind EXECUTING falsifier report, round 1

Written live by the round-1 executing falsifier (fresh context; the
previous falsifier instance died in recon and left nothing). Inputs:
charter, spec sections, ruler units AK-066/AK-044/AK-047/AK-048, the
delivered tree. No builder/critic/correction history read (blindness
rule honored).

Status: IN PROGRESS — checkpointed after every executed attack.

## Environment facts at start

- `just ruler` at round start: PASS (`RULER_EXIT=0`, digests match
  `.gauntlet/reference.lock`).
- `git status` reconciled: production diff in 7 tracked files, new
  tests `e2e_full_loop_real_runner.rs` / `e2e_recovery.rs` /
  `e2e_budget.rs`, fixture `tests/fixtures/target-real/`, report
  `.agents/reports/e2e-real.md`. Nothing staged.
- bwrap: present on host (charter fact 12).

## Baseline (flakiness pass, mandated twice per target)

| target | run 1 | run 2 |
|--------|-------|-------|
| e2e_full_loop_real_runner | 2/2 PASS (15.75s) | 2/2 PASS (15.45s) |
| e2e_recovery | 3/3 PASS (20.69s) | 3/3 PASS (18.62s) |
| e2e_budget | 1/1 PASS (10.28s) | 1/1 PASS (10.24s) |

Observed: nextest's default profile runs each target's tests CONCURRENTLY
(recovery: 55s of test time in a 20.7s wall window), so the concurrency
dimension is already exercised by the baseline; no hang, no flake. New
surface wall-clock total (both passes, six invocations): ~2.5 min, well
under the charter's ~10 min bound for one pass.

## Executed attacks

(append-only table; one row per executed attack)

| # | attack | mechanism | result |
|---|--------|-----------|--------|
| 1a | Tier-conditional seam mutant (runner.rs): match guard `if !policy_ref.as_str().contains("cross")` wraps contained/local but spawns cross_cutting VERBATIM (`(Some(_), Some(_)) => argv`) | The F2 unit-binding assertion (e2e_full_loop_real_runner.rs:386) must catch an unwrapped AK-952 spawn that record-level scans cannot see | **KILLED — RED at the binding locus.** `panicked at e2e_full_loop_real_runner.rs:386:5: the AK-952 (cross_cutting) child ran INSIDE the sandbox and observed it — its own artifact must carry the verdict: {"unit":"AK-952",...,"confinement":"confinement=uncontained pid1=systemd tmp-write=ok workspace-write=ok",...}`. Restore sha256 `b20960b5…9ee5b0` == pristine (verified both sides). The record-level assertions stayed green under the mutant (the durable record still carries the policy ref), confirming line 386 is the load-bearing wrapped-spawn discriminator |
| 1b | Fail-open mutant (runner.rs): the `(None, Some(policy_ref))` arm — the typed refusal for a policy-carrying dispatch with NO seam installed — replaced by `(None, Some(_)) => argv` (run it unwrapped) | Spec §23.1 teeth rule: disabling this delivered fail-closed defense must make some proof fail | **SURVIVED — proof gap (MAJOR, F-R1).** ALL GREEN: e2e_full_loop_real_runner 2/2, e2e_recovery 3/3, e2e_budget 1/1, p8_adapter_substitution PASS, and the full adapter package 60/60 (contract_suite + falsifier_round1_live_attacks + live_probe + probe_template_honesty). No delivered test anywhere distinguishes the no-seam refusal from an unwrapped run. The arm is defense-in-depth (composed `gauntlet run` cannot currently reach it: containment is None only under the none backend, whose dispatches carry no policy; `improve_cmd` passes `None` and dispatches no policy today), but the delivered claim in lib.rs docs and e2e-real.md ("a policy-carrying dispatch with no seam installed is a typed admission refusal") is bound by ZERO executed evidence. Restore sha256 `b20960b5…9ee5b0` == pristine (verified both sides) |
| 2a | Tier-hardcode mutant (recording.rs): the durable record's `tier` field reverted to hardcoded `BlastRadius::Contained` (undo of F3) | The happy leg's record-truth assertion (e2e_full_loop_real_runner.rs:427) must catch a record that misstates a cross_cutting dispatch's tier | **KILLED — RED.** `panicked at e2e_full_loop_real_runner.rs:427:5: the durable invocation record states the dispatch's REAL tier, not a hardcoded default: {"record":"role_invocation",...,"workspace_ref":"…AK-952.000004","tier":"contained","sandbox_policy_ref":"tier-cross-cutting-real-v1",...}` — the lie is visible verbatim (contained tier beside the cross_cutting policy ref). Restore sha256 `6807fff0…9cf39a` == pristine (verified both sides) |
| 2b | No-op registration mutant (recording.rs): `record_dispatch_tier` body replaced with `let _ = (invocation_id, tier);` — the tier fact never arrives, every policy-carrying record build must refuse | The channel's fail-closed consumption arm (record must not guess) must bite when registration is skipped | **KILLED — RED.** Both real-runner legs fail (run never reaches its honest terminal; panics at e2e_full_loop_real_runner.rs:316 and :590). The skip-registration route cannot make the record lie — it refuses instead. Restore sha256 `6807fff0…9cf39a` == pristine (verified both sides) |
| 3 | Invocation lifecycle on the budget-terminal path: manual scratch world (`/tmp/falsifier-r1/budget-world`), real pristine binary, silent wrapped children, bounded `timeout 180`; run reached `terminal=escalated`, `AK-951 timed out twice`, exit 1. /proc liveness sweep (`pgrep bwrap/child.sh/sleep`, `kill -0` on handle pids) ~90 s after exit and again later | In-flight hostile children must not outlive a terminal run (the F6 class) on the row-4 escalation path, which no delivered leg asserts liveness on | **NO LEAK on the pristine path.** The run's two wrapped dispatches (handle pids 228118, 228236) were reaped by the loop's own stall/cancel machinery; zero world-related processes survive. A leak WAS observed live (bwrap pair + in-sandbox `sh` for workspace `.AK-951.000000` alive post-exit) but ATTRIBUTED — by handle-pid mismatch (survivor pids 224604/5/6 predate and differ from the fresh run's recorded spawn pids) and namespace count (one survivor vs two fresh dispatches) — to an earlier run under my mutant-2b STALE binary, whose recording-refusal arm ("admitted child … was cancelled for recovery") did not kill the just-spawned wrapped child. Recorded as minor F-R3 (the delivered cancel arm can lose a just-spawned wrapped child; trigger is a post-spawn admission refusal, unreachable in delivered composed runs — see F-R1). Also observed: bwrap `--die-with-parent` did not prevent the leaked namespace outliving its driver (informational). All survivors died/exited; world verified clean afterward |

### Attack 3 sub-probe — resume of a terminal run (findings F-R4, F-R5)

`gauntlet resume` of the ALREADY-TERMINAL escalated run
(CGXHYVG7SKM68YSMAY7CY4ADW3) was NOT refused: it re-entered the loop,
burned iteration 3 (checkpoints seq 4–5 in the export; rounds booked
2→3), admitted and timed out a NEW dispatch (iteration 3), changed the
run's terminal from `escalated` to `budget_exhausted`, and minted a
SECOND open AK-953 ticket for the identical unresolved fact
(esc-6a9c8018-0 pass 1, esc-6a9c8189-0 resume pass; both
`state=open`, summaries byte-identical).

- F-R4 (**MAJOR**): the run report JSON printed by the budget-terminal
  run classifies every unit `configured-but-untested` with basis "the
  latest checkpoint names the unit open — configured into the run,
  **never attempted on record**" — FALSE against the run's own durable
  export: 2 dispatch attempts with `invocation_terminal state=timed_out`
  events existed after the FIRST pass (3 after the resume). The charter
  deliverable 5 requires an honest classification; the delivered
  e2e_budget leg never asserts the units array, so the false basis rides
  the printed JSON unasserted. Executed against the pristine binary on
  the delivered leg's exact world shape.
- F-R5 (**minor**): resume of a non-settled terminal run re-spends the
  budget and duplicates open tickets for the same fact. The mock E2E
  leg 6 (settled, COR-R2-1) pins the SETTLED no-op only; no delivered
  leg covers the escalated-resume path, and the F9 one-ticket-per-unit
  ledger is per-pass (in-memory `scheduler_governance_blocked`), so the
  queue accumulates duplicates. (Re-entry itself may be the designed
  operator flow — adjudication left to the orchestrator; the duplicate
  ticket and the unguarded re-spend are the objective teeth.)
- Informational: the resumed pass's new invocation id EQUALS the run id
  (CGXHYVG7SKM68YSMAY7CY4ADW3 appears as both `record:run`.run_id and a
  `role_invocation`.id in the same export); and the report JSON carries
  `terminal_state:null` while stdout prints `terminal=budget_exhausted`.

| 4 | Recovery at a NEW durable boundary: rec2-world, real binary as a child; poller (python, sqlite RO, 0.2 s tick, 120 s bound) waits for `settlement_committed events>=2` then SIGKILLs mid-run (exit 137). Then stale-lock refusal → operator `rm writer.lock` → `resume` | Exactly-once + run identity must hold when the kill lands AFTER both settlements (the legs kill at the FIRST settlement; no delivered leg uses this boundary) | **HELD — no defect.** 2 settlement events, 1 settlement row per unit (AK-951, AK-954), 1 evidence production per locator after resume; export answers for the SAME run id (`S4HYE7XN9HJ439SSYX8KT889G0`); AK-953 never dispatched. The stale-lock refusal fired verbatim with the dead holder's pid named. Operator disclosure: my FIRST recovery world (rec-world) was an invalid kill — API latency between tool calls let the run reach its own terminal first, so the SIGKILL hit an exiting/dead process and no lock persisted; that world's "resume re-entry" observations were re-derived in rec2-world and in the budget world properly. The writer-lock lifecycle was re-verified independently: SIGKILL mid-run leaves `writer.lock` naming the dead pid (module contract holds) |
| 6 | e2e-real.md claims falsified from the durable stores of the rec2 world (not from any report): export records, evidence objects, escalations list, state DB | AK-066/AK-048 (wrap leg), AK-044 (never dispatched + queue), closure routing, record tier truth | **CONFIRMED from stores.** AK-952 `role_invocation`: `tier=cross_cutting`, `sandbox_policy_ref=tier-cross-cutting-real-v1`; AK-951/AK-954 records state `contained`/`local` respectively; the AK-952 evidence object of kind `child-work-record` carries `confinement=confined pid1=bwrap tmp-write=denied workspace-write=ok`; AK-953: zero dispatch records, queue ticket names "irreversible: never an automatic dispatch candidate — scheduler-level, unconditional"; AK-952 closure ticket names "requires lead_agent authority for closure … settlement refused and routed to the operator". ONE caveat riding the same stores: TWO open "dispatch not scheduled: Unit AK-953 …" tickets exist in this single run (one minted per pass — original + resume), the F-R5 duplicate-ticket fact visible in a second world |

Budget/reserve claims (from the budget world's store, attack 5/6
budget half): final checkpoint `budget_spent` rounds=3 < fixture
ceiling 4 (reserve never spent), tokens=0, cost_usd=0 — honest zeros;
zero settlement events in the whole run; escalation terminal honest on
stdout. The delivered budget leg's own assertions hold.

| 7a | Frozen-answer-key tampering (scratch copy only): appended unit "## AK-999" to `.wayfinder/answer-key.md` without re-freezing | The freeze/doctor must refuse the run; no dispatch | **REFUSED.** Exit 1 at the §8.5 preflight: "answer key holds 5 unit blocks; the frozen record declares 4", "block AK-999 … missing its required field verify_by", "digest fail". Run dirs created: 0 |
| 7b | Malformed tier policy (scratch copy only): `cross_cutting.json` replaced with invalid JSON | No unsandboxed fallback; typed refusal/escalation for the affected unit; honest terminal | **HONEST.** Run completes `terminal=blocked`; AK-952 never dispatched; AK-951/AK-954 settle; AK-953 ticket stands; no crash. (The preflight does not check policy JSON — the refusal lives at the gate's per-dispatch validation, AK-047's posture; recorded as an observation, not a defect) |
| 7c | Removed `.gauntlet/reference.lock` (scratch copy only) | Freeze must refuse | **REFUSED.** Exit 1 at preflight: "reference record not readable at reference.lock (entity not found)". Run dirs: 0 |

## Regression battery at round end (all green, nothing of mine left behind)

- `cargo nextest run --test e2e_full_loop_mock` — 12/12 PASS
- `cargo nextest run --test p5_hard_gate` — 4/4 PASS
- `cargo nextest run --test p7_recovery --test p6_budget_control` — 7/7
  PASS (p7 5 + p6 2; p6 also run standalone: 2/2)
- `just ruler` at round end: PASS (`RULER_EXIT=0`)
- Pristine-source proof: all four mutation-target files sha256-verified
  against the round-start record (`/tmp/falsifier-r1/pristine-sha256.txt`,
  all OK). Nothing staged; nothing committed; tree differs from round
  start only by this report file.
- Probe hygiene: repo `.gauntlet/policies/` holds exactly `.gitkeep`,
  `contained.json`, `cross_cutting.json`, `local.json` (Aug 14–28
  mtimes, untouched); fixture `tests/fixtures/target-real/` untouched
  (Sep 5 08:08 delivered mtimes). No gauntlet/bwrap/child processes
  survive from any attack world.

## Findings (severity-adjudicated)

- **F-R1 MAJOR** — the no-seam fail-closed arm
  (`CliAgentRunner` `(None, Some(policy_ref))` → `AdmissionRefused`,
  runner.rs) SURVIVES its own disabling mutant across the whole
  battery (3 new E2E targets, p8, full adapter package 60/60). The
  delivered claim "a policy-carrying dispatch with no seam installed is
  a typed admission refusal — never an unwrapped run" (lib.rs docs,
  e2e-real.md) is bound by zero executed evidence; spec §23.1's teeth
  rule ("disabling the defense makes the proof fail") is violated for
  this defense. Reproduction: mutant 1b above.
- **F-R4 MAJOR** — the run report JSON's classification basis is
  factually false on the budget-terminal path: "the latest checkpoint
  names the unit open — configured into the run, **never attempted on
  record**" while the run's own export holds 2+ `invocation_admitted` /
  `invocation_terminal state=timed_out` records for that unit. The
  charter's deliverable 5 requires an honest classification; the
  delivered budget leg never asserts the units array, so the false
  basis ships unasserted. Reproduction: attack 3's world + export
  (verbatim JSON in the transcript above).
- **F-R3 minor** — the recording-refusal arm's recovery cancel
  ("admitted child … was cancelled for recovery") left a just-spawned
  WRAPPED child alive (bwrap pair + in-sandbox shell observed live
  post-exit; bwrap `--die-with-parent` did not reap it). Executed under
  a mutant-triggered path (no-op tier registration), attributed via
  handle-pid mismatch; the path is unreachable in delivered composed
  runs today (see F-R1). The delivered pristine budget/poison paths
  retire their children cleanly (verified by /proc sweep).
- **F-R5 minor** — `gauntlet resume` of a NON-settled terminal run
  (escalated) is not refused: it re-enters the loop, burns another
  booked round, rewrites the run's terminal (escalated →
  budget_exhausted), and mints a DUPLICATE open ticket for the same
  irreversible-block fact (observed in two independent worlds; F9's
  one-ticket ledger is per-pass). The mock leg 6 pins only the SETTLED
  no-op (its COR-R2-1 precedent adjudicated iteration-burning on a
  terminal run a blocking defect for the settled case); whether
  escalated re-entry is the designed operator flow is left for
  orchestrator adjudication — the duplicate ticket and the unguarded
  re-spend are the objective teeth.
- **Informational** — (a) a resumed pass minted an invocation id
  identical to the run id in the same export (id-space collision
  across kinds); (b) the report JSON carries `terminal_state:null`
  while stdout prints the terminal; (c) the generic open-unit basis
  text also renders for units whose escalation was operator-routed
  (tamper-policy world), diluting "configured-but-untested".

VERDICT: NOT_CONFIRM

Counts: BLOCKING 0 · MAJOR 2 (F-R1, F-R4) · minor 2 (F-R3, F-R5) ·
informational 3. Executed attacks: 12 (1a, 1b, 2a, 2b, 3, 3-resume,
4, 6, 7a, 7b, 7c, + the twice-per-target flakiness pass); mutants
applied 4, all restored byte-identically (sha256 both sides recorded).

Basis: the delivered mechanism core is genuinely hardened — the wrap
seam's binding assertion kills tier-conditional unwrapping at its own
locus, the tier channel refuses lies in both directions, exactly-once
holds at a kill boundary the legs do not use, the freeze refuses
tampered references, and every store-facing claim in e2e-real.md
checked out true. NOT_CONFIRM (rather than CONFIRM) because of F-R1:
a delivered fail-closed defense with a recorded rationale has no
executed proof binding it anywhere in the battery, which is exactly
the class the teeth rule exists to prevent — plus F-R4's false
printed basis on a delivered path.

## Not attacked (next round's benefit)

- The remaining disposition paths for child retirement: diagnostic
  exhaustion, reference-mutated abort, governance refusals mid-flight
  (only the budget/stall and poison paths were /proc-swept).
- Cancel-during-retry-window races at fine grain (the row-2 retirement
  was only verified via the delivered poison leg's records).
- `gauntlet improve`'s composition (None containment) — static
  inspection only; no execution probe that it can never dispatch a
  policy-carrying invocation.
- The escalation-resolve → resume intended operator flow (only the
  unguarded re-entry was probed).
- Explicit `--test-threads 4` pressure run (the default profile already
  overlaps the recovery tests; a dedicated pressure pass was not done).
- The remaining settled proofs (p1–p4, p9–p12, g6/g7/g10, antigaming)
  were only regression-adjacent, not attacked through.
- bwrap namespace kill-semantics stress (grandchildren of grandchildren)
  beyond the budget/poison worlds.
- The repo-root GAUNTLET.md / UNK-003 surface (out of scope; the
  fixture-level resolution was taken as recorded).

## Findings

(none yet)

## Not attacked (handoff to the next round)

- (initial list, may grow)
