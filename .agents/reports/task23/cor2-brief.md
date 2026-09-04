# Task 23 — COR-2 brief (correction round 2)

You are the COR-2 builder. The blind executing falsifier round 1
returned NOT_CONFIRM (`.agents/reports/task23/falsifier-round1.md` —
read it in full; its reproductions are your RED evidence templates).
Charter: `.agents/reports/task23/charter.md`. Law: `AGENTS.md`.
Every item is RED-first: reproduce the finding as a failing test (or
direct port-level/suite-level probe), then fix, then green. Never
weaken an assertion; never disable a test.

## R1 (F-1, MAJOR — executed through the real binary) Condition 2 must cover ALL FOUR evaluation arms

`EvaluationRecord` carries only the held-out arms' reference digests;
the regression arms' digests are never recorded, so gate condition 2
structurally sees 2 of 4 arms and a regression task set frozen under a
MOVED ruler evaluates and adjudicates cleanly (the falsifier reached
`improve promote` adjudication past condition 2 with held-out digest
ebba17… and regression digest 3f6179… in one evaluation record).

Fix: (a) the port's `EvaluationRecord` gains
`regression_control_reference_digest` / `regression_treatment_reference_digest`
— port record + codec/json payload + sqlite adapter + shared suite
agreement (`facts_agree`) + harness derivation from the regression
outcomes, all four digests durable; (b) gate condition 2 checks all
four against the current frozen reference digest (one typed
`ReferenceMoved` refusal naming the offending arm); (c) the harness,
at evaluation time, refuses (typed, before any record is appended) an
arm whose resolved reference digest differs from the current frozen
reference — the moved-ruler world is refused at the source, not merely
at adjudication. RED-first: port/suite-level probe of a four-arm
record with a divergent regression digest ⇒ condition 2 refuses with
`ReferenceMoved` naming the regression arm; a harness-level probe of a
regression task set with a different frozen digest ⇒ typed refusal at
evaluate, nothing appended.

## R2 (F-2, MAJOR — executed over the real sqlite adapters) The instrument-series comparison pairs BY UNIT, not by position

`instrument_series_verdict` compares settlements positionally over
store order; an extra deterministic treatment settlement earlier in
the list shields a genuinely model-rung-weakened unit (probe A
PROMOTED with `instrument_series_non_weaker: true`; the aligned
control A2 was correctly rejected — the check bites only while the
pairing aligns).

Fix: pair settlements BY UNIT exactly as `regression_verdict` already
does (join on the unit id): for every control-settled unit, the
treatment settlement of the SAME unit must be non-weaker;
a treatment unit settled under a weaker instrument than control's
same unit is `SettlementAccessWeakened` regardless of list shape or
extra settlements; a treatment-settled unit with no control
counterpart is not shielded either — the held-out instrument series
compares over the SHARED unit set, and any undecidable pairing
(missing instrument facts) stays an evidence gap. Keep the existing
unequal-arm-len behavior consistent with the new per-unit rule (the
position-based "cannot compare over unequal arms" undecidable is
replaced by the unit join; document the new rule in the module docs).
RED-first: the falsifier's probe-A shape verbatim (extra det unit
shielding a model-rung unit) ⇒ REJECTED with
`settlement_access_weakened`; the aligned control shape still
rejects; the honest equal-shape still promotes.

## R3 (F-3) A margin above 10 000 bps is a typed configuration error, never a silent clamp

`margin_holds` saturates (`margin_bps.min(10_000)`), so a declared
15000 bps is enforced as exactly 100% (probe B: control=100,
treatment=0 PROMOTED at a declared 150% margin). Fix: refuse
`margin_bps > 10_000` with a typed error at the gate constructor AND
at config resolution (a margin over 100% cannot be an improvement
requirement; refuse it where it enters). RED-first: constructor and
resolution probes.

## R4 (F-4) The evaluation record's declared `metric` and `protected` set are inside the idempotency agreement

`EvaluationRecord::facts_agree` omits them; a re-append with a
different declared metric or an emptier protected set is swallowed as
the idempotent no-op keeping the ORIGINAL declaration (probe C).
Fix: include both in `facts_agree` (and the sqlite adapter's
agreement path); a disagreeing re-append is the typed
`EvaluationConflict`. Suite case added. RED-first: probe C verbatim.

## R5 (F-5) The governance-target classification is case-normalized

`target_is_governance_bearing` is case-sensitive prefix matching;
`"Reference-pipeline"` and `"Settlement-machinery"` classify as free
(probe E). Fix: normalize the target's case before the prefix match
(document the normalization); add unit pins. RED-first: probe E
verbatim.

## R6 (F-6) The containment scan covers the raw write classes

The optimizer source scan's token lists miss `std::fs::write`,
`File::create` / `OpenOptions`, `std::env::set_var`, and
`Command::new` (probe H: a fixture carrying all four reports zero
violations). Fix: extend the scan's rule tokens to the raw write /
env-mutation / process-spawn classes for the promote module (or
replace with an import-level allowlist — your call, but the scan must
catch the falsifier's fixture verbatim), keep the negative controls
consistent (planted shapes reported; the clean module still passes),
and prove the bite. RED-first: probe H's fixture shape ⇒ the scan
reports violations.

## Records, not code (no fix owed; carry into the closure)

- The `runners.by_role.<role>.model` Inert classification (latent
  policy question, unreachable through the binary today —
  `validate_delta` refuses by_role keys): trigger-record for the
  ruler owner / task24.
- The falsifier's not-attacked list (cross-process double-adjudication,
  full-battery re-run, real-runner dispatch, JSON fuzzing): carry into
  the closure's honest boundaries and the next falsifier round's
  benefit list.

## Discipline

- Build-lock: `flock /home/sprime01/projects/gauntlet/.agents/reports/task23/compile.lock
  -c '<command>'` — one TOP-LEVEL flock per command, NEVER nested
  inside another flock's -c string (a predecessor self-deadlocked the
  repository this way); `sccache --stop-server` INSIDE the same
  flock'd command AFTER the build call. Check
  `pgrep -fa "cargo|flock|nextest"` before editing. Long builds in
  background, poll every 30–60 s. Never raise parallelism.
- Checkpointing: append STARTED/DONE lines + sha256 of edited files to
  `.agents/reports/task23/cor2-progress.md` after EVERY item; finish
  with `.agents/reports/task23/cor2-report.md` (per-item RED → fix →
  GREEN with real numbers; deviations).
- NEVER edit `.wayfinder/answer-key.md`, `.gauntlet/reference.lock`,
  `.wayfinder/reference.yaml`. No new dependencies (no serde). Do not
  stage or commit.
- Acceptance before reporting done (all green in your own runs):
  `cargo nextest run --test p9_self_improvement_containment`;
  `cargo nextest run -p gauntlet-app promote::`;
  `cargo nextest run -p gauntlet-app --test promotion candidate_only`;
  `cargo nextest run -p gauntlet-ports -E "test(revision_ledger)"`;
  `cargo nextest run -p gauntlet-adapter-state-sqlite --test
  contract_suite`;
  `cargo nextest run -p gauntlet-arch-tests` (FULL package — the
  repo-wide scans, not just optimizer);
  `just check`; `just lint`.
