# Task 25 — closure record (SETTLED 2026-09-06)

Plan: `.agents/plans/gauntlet-impl.md` §Task 25 (phase 7). Charter:
`charter.md` (binding, incl. the UNK-003 operator-delegated fixture-level
runner resolution). Ruler units: AK-066 (both the locator divergence and
the settleability tension filed at settlement), plus real-world
confirmation of AK-044, AK-047, AK-048.

## What settled

- **Fixture** `tests/fixtures/target-real/`: frozen reference AK-951
  (contained/critic), AK-952 (cross_cutting/lead_agent — coupling-forced),
  AK-953 (irreversible/human), AK-954 (local/critic, COR-1 F8); own
  GAUNTLET.md (`runners.default: cli-agent`, `sandbox.backend: bwrap`),
  own three tier policies + NO irreversible policy (AK-049), real source +
  check script, deterministic child agent (normal/silent/poison/reap/alive
  modes).
- **Production correction** (RED-first, the charter's deliverable 2):
  `LiveContainmentDeferred` retired — a typed `CommandContainment` seam on
  the generic CLI adapter, installed by the composition exactly when the
  posture is live (same Sandbox+PolicyProvider instances as the gate);
  wrap decided after admission, before any filesystem effect or process
  start; policy resolved BY REF and re-validated per dispatch; capabilities
  honestly `Isolation::Process` iff the seam is installed (every other
  runner refused by the capability-vs-tier conjunct); live-posture
  exercisable ceiling Critic → LeadAgent (coupling-forced; recorded for
  operator ratification); `none` posture byte-for-byte unchanged.
- **Targets** `crates/gauntlet-cli/tests/{e2e_full_loop_real_runner,
  e2e_recovery,e2e_budget}.rs` through the REAL binary: the plan gate runs
  verbatim — `cargo nextest run --test e2e_full_loop_real_runner --test
  e2e_recovery --test e2e_budget` → 9/9 at closure.

## Confirmation ladder

Blind design-as-built critic r1 **CONFIRM** (0 blocking / 3 MAJOR / 7
minor / 3 informational; `critic-design.md`) → **COR-1** (all RED-first;
`cor1-report.md`) → blind EXECUTING falsifier r1 **NOT_CONFIRM** (0/2
MAJOR/2 minor; 12 executed attacks; `falsifier-round1.md`; first instance
died on an API outage in recon — reconciled clean, zero loss) → **COR-2**
(all RED-first; a git-restore incident clobbered uncommitted work and was
reconstructed byte-exactly under the sha256 discipline; `cor2-report.md`)
→ focused falsifier r2 **CONFIRM** with 3 MAJOR + 3 minor, all executed
(`falsifier-round2.md`) → **COR-3** (teeth tests RED under the falsifier's
own recipes; unit-bearing summaries; mutate-path lock; F2-7 pinned
honestly and NOT built; `cor3-report.md`) → **the orchestrator's closure**:
personal teeth (F2-1/F2-4 mutants RED verbatim → byte-identical restores;
AK-044's never-dispatch invariant held with 3 of its 5 defensive layers
successively disabled — over-determination by design, every layer carrying
its own executed unit pin) and the closure battery **executed a real
defect** (the produce-window recovery leg: two durable production rows
after a post-production kill) → **COR-4**: the STOP case (re-production is
spec-forced) verified by the orchestrator against p7's settled retry pin
(`p7_recovery.rs:1454`) and the §14.3 staleness machinery
(`verify/authority.rs:1149/:1182`); **adjudication (a)** re-scoped both
recovery legs tests-only to the spec-honest semantics (`cor4-report.md`).

## Final closure battery (the orchestrator's own runs, final tree)

`just check` 0 · plan gate verbatim 9/9 + `just ruler` PASS · `just test`
1548/1548 exit 0 · `just lint` 0 · `just deny` 0 · `just boundaries`
237/237 · `just ruler` PASS (final). The ruler never moved at any phase of
any round.

## Honest boundaries (recorded, none silent)

- A cross_cutting unit in a seeded reference can never settle unattended
  (§5.3.2 coupling + §16.1 condition 3 as implemented): the run routes its
  closure to the operator — AK-066's wording tension filed for the ruler
  owner.
- The operator surface's resolve/retry dead-ends for governance-routed
  tickets (F2-7, executed twice): the refusal is AK-062's class working;
  the missing EARNED path is a ruler-owner question.
- Recovery re-produces on resume (p7 + §14.3); the wasted middle attempt
  (F-C4-1, workspace-counter restart) is recorded with its
  attempts-ceiling cost.
- `b1_contract_wins_over_exit7` failed once under full-workspace load and
  never recurred (isolation 20/20; later full runs green): the task22
  load-flaky protocol class.
- The deterministic child under the real adapter is NOT a frontier-model
  claim (the posture statement in `../../e2e-real.md`).

## Ruler-owner items filed at settlement (reference-candidates.md, 2026-09-06)

1. AK-066 locator divergence (three target spellings).
2. AK-066 "every blocking Unit settles" settleability tension.
3. F2-7: how does an operator-routed Unit ever acquire §16.1 PASS evidence?
4. §18.3 adoption vs §14.3 staleness + F-C4-1's wasted attempt.
5. The live-posture authority-ceiling raise (Critic → LeadAgent) for
   ratification.
