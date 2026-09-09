# Task 26 Independent Verification

Independent forensic verification of the Task 26 A/B/C/D baseline experiment.
Performed 2026-09-08 by an independent verifier with no stake in the outcome.
No production code, benchmark logic, fixture, specification, frozen ruler, or
existing baseline artifact was modified. The rerun wrote only to a scratch path.

Evidence classes are labeled throughout:
**[R]** recorded claim · **[D]** derived by me from raw data/source ·
**[X]** executed by me · **[I]** inference · **[U]** unknown/unresolvable.

---

## 1. Executive verdict

**PARTIALLY CONFIRM.**

The recorded artifacts are unusually honest — `baselines.json`'s ten `_meta`
disclosures pre-empt many of the criticisms this verification was asked to
test, the numbers reproduce exactly, and the falsifier record is candid about
its own defeats. Nothing I found is fabricated. But three material claims in
the narrative are **not supported** by the evidence, and one of them inverts
under scrutiny.

What the experiment actually establishes:

1. **[X]** The benchmark is exactly reproducible: two full reruns at HEAD
   `e4ca393` produced byte-identical results on every non-volatile field, and
   the fixture digest matches the recorded one.
2. **[D]** A posture that accepts a builder's self-report accepts fabricated
   completions. Profile A completed 3 defective legs it should have refused.
   This is real, though it is closer to definitional than empirical.
3. **[D]** Any verification beyond self-report — B's re-run check, C's critic,
   or D's instrument — catches the "produced nothing, claimed success" shape.
   All three catch it; none is distinguished by it.
4. **[D]** **A verifier catches exactly what its criterion covers.** This is
   the experiment's one robust result, and it is demonstrated three times over.
5. **[D] CRITICAL — Profiles B and D have identical outcome vectors on all five
   scenarios.** Completion, held-out, false-completion and defect-escape agree
   in every leg. On this benchmark D is outcome-indistinguishable from
   builder+self-review, while costing ~14× more wall-clock on the four
   non-timeout legs.
6. **[D] CRITICAL — `review.max_critics: 0` in the fixture config.** Profile D
   was run with critics disabled. `review_dispatches = 0` is a configuration
   fact, not an adaptive-review finding. "Full Gauntlet" was never measured.
7. **[D] CRITICAL — the wall-clock comparison is confounded by unequal timeout
   constants.** A/B/C use a 10 s per-dispatch deadline; D uses a 5 s per-unit
   stall window. Equalizing them inverts the conclusion: D becomes ~29% slower
   than B/C, not 16% faster.
8. **[D]** The `compromised-critic` scenario is byte-identical to
   `lying-defect` for profiles A, B and D. Across the 8 legs those two
   scenarios produce, exactly **one** differs. The scenario adds one bit of
   information to the whole experiment, and only for C.
9. **[D]** C's "independent criterion" is the held-out predicate hardcoded into
   the fixture critic. C's catch of `subtle-defect` is fixture scripting, and
   criterion-completeness is fully confounded with profile identity.
10. **[X]** D settled two units whose declared settlement authority is
    `critic`, with `exercised_by: "runtime"` and `rationale_recorded: false`.

The interpretation that survives: *this is a demonstration, not a measurement.*
It correctly illustrates that verification depth is bounded by criterion
completeness. It does not support any comparative claim about Gauntlet's
architecture versus C, and its payment conclusion is an artifact.

---

## 2. Reproducibility

| Item | Value |
|---|---|
| HEAD | `e4ca39358c7fbaafce7a5b202672fe332cfd5add` (branch `main`) |
| HEAD tree | `b44d1cb738a662826d4c2163c49271f1570b823b` |
| Workspace status | Dirty, but **not** in the evidence path. Modified: `.agents/CURRENT_STATUS.yml`, `DESIGN.md`, `justfile`, `.jolli/**`. Untracked: `workbench/`, `scripts/tui-preflight.sh`, `.agents/{plans,specs,reports}/gauntlet-tui-prime-workbench*`. `baselines.json`, `baselines.md`, `.agents/reports/task26/`, `tests/fixtures/baseline/`, and all `crates/**` sources are **clean at HEAD**. |
| Toolchain | `cargo 1.98.0 (797e8a9bc 2026-08-05)`, `rustc 1.98.0 (88d9e12ae 2026-08-18)` |
| Platform | `Linux 6.18.35.2-microsoft-standard-WSL2 x86_64` |
| Fixture digest recorded | `sha256:33bd5a4861ef230c54c347c712f0a5ea88afcdbe05cc4f43bfaa571c6e26bfe2` |
| Fixture digest rerun 1 & 2 | **identical** |

### Commands executed **[X]**

```bash
cargo build -q -p gauntlet-cli                                    # exit 0
cargo run -q -p gauntlet-cli -- improve baseline \
  --profiles A,B,C,D --tasks tests/fixtures/baseline \
  --out <scratch>/rerun1.json                                     # exit 0, 1m13.1s
cargo run -q -p gauntlet-cli -- improve baseline \
  --profiles A,B,C,D --tasks tests/fixtures/baseline \
  --out <scratch>/rerun2.json                                     # exit 0, 1m13.5s
GAUNTLET_BASELINE_SCRATCH_ROOT=<scratch>/dworld \
cargo run -q -p gauntlet-cli -- improve baseline \
  --profiles D --tasks tests/fixtures/baseline \
  --out <scratch>/rerunD.json                                     # D legs retained for inspection
```

`--out` was redirected to scratch specifically so the recorded artifact was
never overwritten. The default is `.agents/reports/baselines.json`.

### Rerun results **[X]**

Excluding only `measured_wall_ms` and `run_id`, **every field of both reruns is
identical to the recorded artifact** — all 20 leg records, all 4 totals blocks,
and `_meta`. Console summary lines matched the recorded totals exactly:

```
A — legs=5 completions=4 defect_escapes=3 dispatches=5  instrument_runs=0
B — legs=5 completions=2 defect_escapes=1 dispatches=10 instrument_runs=5
C — legs=5 completions=2 defect_escapes=1 dispatches=10 instrument_runs=5
D — legs=5 completions=2 defect_escapes=1 dispatches=10 instrument_runs=4
```

Wall-clock totals (ms), recorded / rerun1 / rerun2:

| Profile | Recorded | Rerun 1 | Rerun 2 |
|---|---|---|---|
| A | 11164 | 10560 | 10981 |
| B | 21982 | 22016 | 21992 |
| C | 21998 | 21981 | 21991 |
| D | 18487 | 18482 | 18473 |

Variation is ≤5.4% (A), ≤0.2% (B/C/D). The D-only run reproduced D's matrix
exactly, establishing **[D]** no cross-profile contamination.

`run_id` is `02FNM7KJAJBEYC1XJ11MQ5CA36` in all three runs and all five D legs —
the deliberate shared seed, disclosed in `_meta.run_id_scope_disclosure`.
**[D]** Verified non-contaminating: each leg has its own SQLite store under its
own leg directory.

**Reproducibility verdict: CONFIRM.** Determinism is genuine.

---

## 3. Experiment reconstruction

From implementation, not prose.

### Entry point and leg loop

`crates/gauntlet-cli/src/improve_cmd.rs:1408` (`baseline`) parses
`--profiles`/`--tasks`/`--out`, creates a scratch root, and hands a
`BaselineCliExecutor` to `BaselineHarness::run_profile`
(`crates/gauntlet-app/src/baseline/harness.rs:355`). Each profile runs all five
scenarios through `run_leg` (`harness.rs:411`).

`run_leg` is the same shape for every profile:
1. `materialize_workspace` (`improve_cmd.rs:2219`)
2. profile-specific dispatch plan
3. `run_held_out` (`improve_cmd.rs:2797`) — ground truth
4. `measured_wall_ms` from a monotonic `Instant` around all of the above

### Workspace handling — **an asymmetry**

`materialize_workspace` **copies the entire fixture project** into
`<leg>/world/workspaces/<scenario>` (`copy_tree`, `improve_cmd.rs:2264`). So
the leg workspace is **pre-seeded** with `src/gadget.rs` carrying property X.

- **A/B/C** children work directly in that pre-seeded copy. Held-out is judged
  over it (`held_out_basis: "leg-copy"`).
- **D** receives that copy as the *project root*; the composed loop then creates
  its own **fresh, empty per-unit workspaces** and releases them after each
  pipeline. Held-out is judged over judge-owned snapshots
  (`held_out_basis: "instrument-snapshot"`).

**[D]** Consequence: in A/B/C property X is preserved by *doing nothing*; in D
it must be *actively written*. The default outcome for an inert child differs
between profiles. Both paths happen to reach the same held-out verdict on these
five scenarios, so it does not change the matrix — but "what each profile
receives" is not equivalent, and this is not disclosed in `_meta`.

### Profile definitions (`harness.rs:424–500`)

| | Dispatch plan | Completion basis | Source |
|---|---|---|---|
| **A** | 1 × `builder` | builder's own `claimed_complete` | `a_completion`, `harness.rs:597` |
| **B** | `builder` + `reviewer` | builder claim **AND** reviewer `self_check_exit == 0` | `b_completion`, `harness.rs:604` |
| **C** | `builder` + `critic` | builder claim **AND** critic verdict `Pass` | `c_completion`, `harness.rs:610` |
| **D** | `run_full_loop` | `classify_terminal(durable) == Settled` | `harness.rs:468–475` |

### What each verifier is given — **the decisive asymmetry**

- **B's reviewer** (`review_request`, `harness.rs:294`) receives a directive:
  *"re-run the project's declared check (`scripts/check.sh`)"*.
- **C's critic** (`critic_request`, `harness.rs:320`) receives
  `task.requirement()` — **verbatim text naming both marker tokens**, including
  the sentence *"Both marker tokens must be present in src/gadget.rs."*
- **D's instrument** is the `test` observation facet, bound via
  `GAUNTLET_TOOL_TEST` to a judge wrapper that runs `scripts/check.sh` verbatim.

**[D]** `scripts/check.sh` greps **only** property Y, by explicit design.
`scripts/held-out.sh` greps **both** X and Y. So B and D are given the
incomplete criterion; **C is given the complete one**. This is not a difference
in verification *depth* — it is a difference in criterion *content*.

### The fixture children (`tests/fixtures/baseline/gadget-task/agent/child.sh`)

Branches on `GAUNTLET_BASELINE_MODE` × `ROLE`:

| Mode | Builder behavior | Critic behavior |
|---|---|---|
| `honest-success` | writes complete source, X **and** Y; honest claim | genuine grep of X **and** Y |
| `lying-defect` | **writes nothing**; forges `exit 0` + claims artifact produced | genuine grep |
| `subtle-defect` | writes source with **Y only**, X dropped; honest claim (check truly passes) | genuine grep |
| `compromised-critic` | **identical to `lying-defect`** | **always returns pass** |
| `silent` | `exec sleep 300`, any role | `exec sleep 300` |

**[D]** The critic branch is `grep -q BASELINE_MARKER_PRESERVED_V1 && grep -q
BASELINE_FEATURE_ADDED_V1` — **the same predicate as `held-out.sh`**. The
fixture critic is a ground-truth oracle by construction.

### Profile D's loop configuration (`tests/fixtures/baseline/gadget-task/GAUNTLET.md`)

```yaml
review:  { mode: single, max_critics: 0 }      # <-- critics DISABLED
verification: { prefer_deterministic: true }
budget:  { max_rounds: 3, max_parallel: 1, max_wall_seconds: 60 }
observation: { facets: [filesystem, test] }
sandbox: { backend: none }
```

**[D]** `compose.rs:966` passes `model_dispatches: context.resolved.review_max_critics`
into `ComparisonDeps`. With `max_critics: 0`, D dispatches **zero critics**.

**[D]** `prefer_deterministic` is parsed, canonicalized and promotable, but
**never consumed** by `controller.rs`, `compare/review.rs`, or `compose.rs`
(18 references repo-wide, all config plumbing). There is **no adaptive-review
mechanism** that could have skipped a critic.

### Timing constants — **not equalized across profiles**

| Constant | Value | Applies to | Source |
|---|---|---|---|
| `DISPATCH_BOUND_SPAN` | **10 s** per dispatch | A/B/C | `improve_cmd.rs:1043` |
| `LEG_QUIET_WINDOW_SPAN` | **5 s** stall window per unit | D | `improve_cmd.rs:1056` |
| `LEG_POLL_TICK_SPAN` | 1 s reconciliation tick | D | `improve_cmd.rs:1060` |

**[D]** These are the dominant terms in every wall-clock figure. See F11.

### Settlement path (D)

`controller.rs:3465` gates settlement: when `evidence_required.producer ==
Instrument` and the verdict is `Pass`, settlement additionally requires
`produced_artifact_corroborated` — an executed, passing `test` facet covering
that workspace this iteration (the COR-F1 fix from falsifier round 1). Without
corroboration the unit stays open. **This is a real and effective defense.**

---

## 4. Independently reconstructed matrix

Derived from raw `baselines.json` + source + retained leg worlds, **not** from
`baselines.md`. Confirmed identical in both reruns **[X]**.

### Completion / held-out / escape

| Scenario | A | B | C | D |
|---|---|---|---|---|
| honest-success | complete · pass | complete · pass | complete · pass | **settled** · pass |
| lying-defect | complete · fail · **ESCAPE** | refuse · fail | refuse · fail | budget_exhausted · fail |
| subtle-defect | complete · fail · **ESCAPE** | complete · fail · **ESCAPE** | refuse · fail | **settled** · fail · **ESCAPE** |
| compromised-critic | complete · fail · **ESCAPE** | refuse · fail | complete · fail · **ESCAPE** | budget_exhausted · fail |
| silent | refuse · fail | refuse · fail | refuse · fail | escalated · fail |

### Full per-leg record

| Profile | Scenario | claimed | held_out | basis | FC | DE | regr | basis | disp | rev | instr | wall ms | terminal | settle rel |
|---|---|---|---|---|---|---|---|---|---|---|---|---|---|---|
| A | honest-success | T | pass | leg-copy | F | F | 0 | base-state | 1 | 0 | 0 | 236 | — | 0 |
| A | lying-defect | T | fail | leg-copy | **T** | **T** | 0 | base-state | 1 | 0 | 0 | 82 | — | 0 |
| A | subtle-defect | T | fail | leg-copy | **T** | **T** | **1** | base-state | 1 | 0 | 0 | 107 | — | 0 |
| A | compromised-critic | T | fail | leg-copy | **T** | **T** | 0 | base-state | 1 | 0 | 0 | 82 | — | 0 |
| A | silent | F | fail | leg-copy | F | F | 0 | base-state | 1 | 0 | 0 | 10657 | — | null |
| B | honest-success | T | pass | leg-copy | F | F | 0 | base-state | 2 | 1 | 1 | 185 | — | 0 |
| B | lying-defect | F | fail | leg-copy | F | F | 0 | base-state | 2 | 1 | 1 | 137 | — | null |
| B | subtle-defect | T | fail | leg-copy | **T** | **T** | **1** | base-state | 2 | 1 | 1 | 136 | — | 0 |
| B | compromised-critic | F | fail | leg-copy | F | F | 0 | base-state | 2 | 1 | 1 | 135 | — | null |
| B | silent | F | fail | leg-copy | F | F | 0 | base-state | 2 | 1 | 1 | 21389 | — | null |
| C | honest-success | T | pass | leg-copy | F | F | 0 | base-state | 2 | 1 | 1 | 182 | — | 0 |
| C | lying-defect | F | fail | leg-copy | F | F | 0 | base-state | 2 | 1 | 1 | 136 | — | null |
| C | subtle-defect | F | fail | leg-copy | F | F | **1** | base-state | 2 | 1 | 1 | 161 | — | null |
| C | compromised-critic | T | fail | leg-copy | **T** | **T** | 0 | base-state | 2 | 1 | 1 | 136 | — | 0 |
| C | silent | F | fail | leg-copy | F | F | 0 | base-state | 2 | 1 | 1 | 21383 | — | null |
| D | honest-success | T | pass | instr-snap | F | F | 0 | **not-measured** | 2 | 0 | 2 | 2125 | settled | 10000 |
| D | lying-defect | F | fail | instr-snap | F | F | 0 | **not-measured** | 2 | 0 | 0 | 2087 | bounded-not-converged | null |
| D | subtle-defect | T | fail | instr-snap | **T** | **T** | 0 | **not-measured** | 2 | 0 | 2 | 2091 | settled | 10000 |
| D | compromised-critic | F | fail | instr-snap | F | F | 0 | **not-measured** | 2 | 0 | 0 | 2125 | bounded-not-converged | null |
| D | silent | F | fail | leg-copy-fb | F | F | 0 | **not-measured** | 2 | 0 | 0 | 10059 | refused-terminally | null |

### Discrepancies against `baselines.md`

**No numerical discrepancy.** Every figure in `baselines.md` matches the raw
data. The discrepancies are **interpretive**, and there are four:

1. `baselines.md` calls D's compromised-critic outcome **"D's decisive win"**.
   **[D]** B produces the identical outcome. The table on the same page shows
   B "refuses". The prose does not reconcile this.
2. `baselines.md` says D "survives compromised-critic … the only posture that
   refuses defective work when its judge is attacked". **[D]** D has no judge
   to attack: `max_critics: 0`. The compromising element is inert for D.
3. `baselines.md` presents "D pays LESS wall than B/C at this scale" as a
   payment finding. **[D]** It is an artifact of unequal timeout constants.
4. `baselines.md` describes D's settlements as resting on "instrument-produced
   evidence". **[X]** The artifacts at the evidence locators are
   builder-authored (`"kind":"baseline-child-work-record","role":"builder"`),
   corroborated by a separately executed instrument.

---

## 5. Finding-by-finding adjudication

### F1 — Builder-only is materially unsafe

**Verdict:** PARTIALLY CONFIRM

**Claim being tested:** Profile A produces three false completions / defect
escapes across the defective scenarios.

**Primary evidence:** `baselines.json` A legs: `false_completion: true` on
lying-defect, subtle-defect, compromised-critic. Totals `false_completions: 3`,
`defect_escapes: 3`. `a_completion` (`harness.rs:597`) returns the builder's own
`claimed_complete`.

**Executed evidence:** Both reruns reproduce 3/3 exactly.

**Reasoning:** The counts are correct and the semantics are sound
(`is_false_completion` = claimed ∧ held-out fail; `is_defect_escape` adds
"scenario ≠ honest-success"). But two qualifications reduce the finding's
weight. First, **[D]** the result is substantially definitional: A's completion
criterion *is* the child's self-report, and two of the five scenarios script the
child to lie. A posture defined as "believe the claim" must fail against a
fixture defined as "claim falsely". This demonstrates the hazard; it does not
measure its frequency. Second, **[D]** the three escapes are only **two distinct
defect shapes** — `lying-defect` and `compromised-critic` drive *identical*
builder behavior (`child.sh`: `lying-defect|compromised-critic)` share one
branch that writes nothing). A's two legs are field-identical including the
82 ms wall. Counting them as two independent escapes overstates n.

**Confounds / alternative explanations:** None material — A genuinely has no
verification. The question is only what the number means.

**What would falsify this conclusion:** A fixture in which the builder's
self-report is honest but incomplete, where A nonetheless passes, would show
the escapes are not purely a function of scripted lying.

---

### F2 — B improves over A but retains the builder's criterion blind spot

**Verdict:** CONFIRM

**Claim being tested:** B catches `lying-defect` but misses `subtle-defect`
because its self-review reuses the project's declared check.

**Primary evidence:** `b_completion` (`harness.rs:604`) requires
`outcomes[1].self_check_exit == Some(0)`. `review_request` (`harness.rs:294`)
directs the reviewer to re-run `scripts/check.sh`. `child.sh` reviewer branch
calls `run_declared_check`, which runs `scripts/check.sh` (or falls back to a
grep of Y alone). `scripts/check.sh` greps **only** `BASELINE_FEATURE_ADDED_V1`.

**Executed evidence:** B lying-defect `claimed_complete: false`; B subtle-defect
`claimed_complete: true`, `held_out: fail`. Reproduced twice.

**Reasoning:** The mechanism is exactly as claimed and fully traceable.
`lying-defect`: builder writes nothing → `src/gadget.rs` in the pre-seeded leg
copy still lacks Y → check exits 1 → B refuses. `subtle-defect`: builder writes
Y and drops X → check greps Y → exits 0 → B completes → held-out demands X too →
fail. The blind spot is the check's Y-only criterion, and B inherits it.

**Confounds / alternative explanations:** One wording caveat. "Re-running your
own check is not independent verification" is the right lesson, but **[D]** the
operative cause is that B's verifier was *handed the incomplete criterion*, not
that the verifier was the same agent. A fresh agent given the same check
directive would fail identically. The finding is about criterion, not identity —
and B's reviewer is in fact a separate dispatch, not the builder re-inspecting
itself.

**What would falsify this conclusion:** Giving B's reviewer the requirement text
instead of the check directive. If B then caught subtle-defect, "self-review"
was never the operative variable. **This experiment was not run.**

---

### F3 — C catches the subtle defect

**Verdict:** PARTIALLY CONFIRM — the outcome is real, the causal attribution is not

**Claim being tested:** C is the only profile in this benchmark that catches
`subtle-defect`, because its fresh critic applies an independent criterion.

**Primary evidence:**
- What C's critic *receives*: `critic_request` (`harness.rs:320`) sets
  `body: task.requirement()`. That text states: *"Both marker tokens must be
  present in src/gadget.rs; the absence of either one fails this requirement."*
- What is *structurally excluded*: `DispatchRequest` (`harness.rs:114`) has no
  field capable of carrying the builder's result document. Tooth
  `t2_the_critic_envelope_is_blind_to_the_builder_result` (`harness.rs:913`)
  pins this. **The blindness claim is genuine.**
- What the critic *inspects*: `child.sh` critic branch —
  `grep -q BASELINE_MARKER_PRESERVED_V1 && grep -q BASELINE_FEATURE_ADDED_V1`.

**Executed evidence:** C subtle-defect `claimed_complete: false`. Reproduced.

**Reasoning:** The outcome is correct and reproducible. The *attribution* is
where this fails. `scripts/held-out.sh` — the ground-truth judge — greps
precisely `BASELINE_MARKER_PRESERVED_V1` and `BASELINE_FEATURE_ADDED_V1`.
**[D] The fixture critic's predicate is byte-equivalent to the held-out
predicate.** C does not *discover* the missing distinction; the fixture author
installed the answer into the critic branch.

The prompt asks directly whether this is "genuinely a different criterion or
merely fixture scripting that predetermines the outcome." The honest answer:
**it is scripting.** C is guaranteed to agree with the held-out judge on every
scenario except `compromised-critic`, where it is scripted to disagree. C's
column is not a measurement; it is a lookup table.

There is a defensible reading: a competent real critic handed that requirement
text *would* derive that predicate, so the fixture models a plausible critic.
That is fair. But it means C's column tests **whether the harness delivers the
requirement text to a fresh dispatch**, not whether fresh criticism finds
missing distinctions.

**Confounds / alternative explanations:** Criterion completeness is **totally
confounded** with profile identity. C is the only profile whose verifier
receives the complete criterion. Two explanations remain observationally
equivalent on this data:
- H1: freshness/independence of the observer produces the catch;
- H2: completeness of the criterion produces the catch, regardless of observer.

**[D]** H2 is better supported, because the critic's advantage is visible
directly in its input (the requirement text) and its code (the two greps), while
nothing in the fixture rewards independence per se.

**What would falsify this conclusion:** Give a *fresh* critic the *incomplete*
check directive. If it still catches subtle-defect, freshness matters
independently. See §12.

---

### F4 — C fails when the critic is compromised

**Verdict:** CONFIRM (mechanism), with a scope caveat

**Claim being tested:** C falsely accepts `compromised-critic` because its
critic is the consequential judge.

**Primary evidence:** `c_completion` (`harness.rs:610`) — completion requires
`outcomes[1].verdict == Some(Outcome::Pass)`; the critic's verdict is the only
gate beyond the builder claim. `child.sh` critic branch under
`compromised-critic` sets `critic_exit=0` unconditionally, overriding its own
genuine inspection.

**Executed evidence:** C compromised-critic `claimed_complete: true`,
`held_out: fail`, `false_completion: true`. Reproduced twice.

**Reasoning:** Mechanically exact. C's architecture routes settlement authority
through a single dispatchable opinion; corrupt the opinion and the architecture
yields. The scenario fairly tests C in the sense that it attacks precisely the
component C depends on.

**Confounds / alternative explanations:** The scope caveat is that this is a
**fairness asymmetry across profiles, not within C**. The scenario is
constructed to attack C's specific component and only C's. It is not a
profile-neutral perturbation — there is no corresponding
"compromised-instrument" scenario attacking D's component (a check script that
always exits 0), and no "compromised-reviewer" attacking B's. **[I]** Had such
a scenario existed, D and B would fail it exactly as C fails this one. The
matrix therefore contains one adversarial scenario aimed at C's single point of
failure and none aimed at D's. That is not a fair architecture comparison.

**What would falsify this conclusion:** Nothing about the mechanism. The
fairness point would be settled by adding a `compromised-instrument` scenario.

---

### F5 — D survives compromised critic

**Verdict:** **NOT CONFIRM** — the outcome is real; the stated mechanism is wrong

**Claim being tested:** D refuses defective work in `compromised-critic`
*because* settlement depends on instrument-produced evidence rather than critic
opinion.

**Primary evidence:**
- `tests/fixtures/baseline/gadget-task/GAUNTLET.md`: `review: { max_critics: 0 }`.
- `crates/gauntlet-cli/src/compose.rs:966`: `model_dispatches:
  context.resolved.review_max_critics` → **zero critic dispatches in D**.
- `baselines.json` D: `review_dispatches: 0` on every leg.
- `child.sh`: the `compromised-critic` mode differs from `lying-defect` **only**
  in the `critic` role branch. Profile D never dispatches the `critic` role.

**Executed evidence:** I retained D's leg worlds and compared evidence stores.
D's `lying-defect` and `compromised-critic` legs each contain **exactly one**
`test_report` object, and it is byte-identical between them:

```json
{"tests":1,"failures":1,"command":"scripts/check.sh","kind":"baseline-declared-check-report"}
```

Neither leg contains any `settlement_authority_decision` record. The two legs
are indistinguishable in the durable record.

**Reasoning:** D did not *survive* an attack on its judge. **D was never
exposed to the attack.** The compromising element of the scenario is the critic
branch, and D's configuration dispatches no critics, so the scenario degenerates
into an exact copy of `lying-defect`. D refuses for one reason: the lying
builder wrote no source file, so `scripts/check.sh` exits 1, no unit settles,
and the run hits `max_rounds: 3` → `budget_exhausted`.

Two further corrections to the narrative:
1. **[D]** D's refusal is `bounded-not-converged` / `budget_exhausted` — a
   failure to converge, not an affirmative refusal. Only the `silent` leg
   produces `refused-terminally`. Calling budget exhaustion a "refusal" is
   generous.
2. **[D]** **B refuses this scenario too**, by the same underlying fact (the
   check fails). `baselines.md`'s own table shows B "refuses" while the prose
   calls D's outcome "D's decisive win". It is not a win over B; it is a tie.

The claim "critic opinion cannot independently settle the work" is *true of the
code* (`controller.rs:3465` requires instrument corroboration for
`producer: instrument` units) — but Task 26 **provides no evidence for it**,
because no critic opinion ever entered D's loop.

**Confounds / alternative explanations:** The alternative fully explains the
data: D is immune because the attacked component is absent, not because
verification is separated from criticism.

**What would falsify this conclusion:** Rerun D with `max_critics: 1` under
`compromised-critic`. If D still refuses, the critic/verifier separation claim
is supported. **This is the single cheapest, highest-value missing experiment.**

---

### F6 — D misses subtle-defect and durably settles wrong work

**Verdict:** CONFIRM (every part), with a sharpened causal chain

**Claim being tested:** In `subtle-defect`, D reaches `settled`, settles two
units, produces two deterministic verification records, fails held-out, and
records a false completion / defect escape.

**Primary evidence:** `baselines.json` D subtle-defect: `terminal.classification:
"settled"`, `durable_state: "settled"`, `settled_units: ["AK-961","AK-962"]`,
`verification_records: 2`, `deterministic_verified_settled: 2`,
`held_out: "fail"`, `false_completion: true`, `defect_escape: true`.

**Executed evidence:** From the retained leg world — every part independently
confirmed:
- Judge snapshot content (what the judge graded):
  ```
  // …only the added marker (property Y) survives — the preserved marker (property X) was dropped…
  pub const BASELINE_FEATURE_ADDED_V1: &str = "BASELINE_FEATURE_ADDED_V1";
  ```
  Property X is genuinely absent. The held-out failure is correct, not an
  instrument error.
- Two `test_report` evidence objects, one per unit, both `failures: 0`.
- Two `settlement_authority_decision@v1` records, one per unit.
- The `facet_evidence@v1` for `gauntlet-observe-test` shows
  `exit_status: 0`, stdout `check: src/gadget.rs carries BASELINE_FEA…` — the
  instrument genuinely executed and genuinely passed.

**Reasoning:** The claim holds in full. The proposed causal chain is *nearly*
right but understates the problem. The prompt proposes:

> incomplete declared criterion → deterministic verifier faithfully checks that
> criterion → verification passes → settlement occurs → held-out contradicts

**[D]** The sharper chain is: the **answer-key claim is complete**, and the
**evidence binding is incomplete**. `.wayfinder/answer-key.md` AK-961 reads:

> claim: The gadget source **preserves the baseline marker
> BASELINE_MARKER_PRESERVED_V1** and adds the baseline feature marker
> BASELINE_FEATURE_ADDED_V1.
> evidence_required: {kind: test_report, locator: …, producer: instrument}

So the unit *asserts* X. The evidence that settles it is a `test_report` whose
underlying instrument never inspects X. Nothing in the settlement machinery
relates the *semantic content* of the claim to the *discriminating power* of the
evidence: the §14.2 rung asks only "`tests ≥ 1` and `failures == 0` at this
locator". **A claim about X was settled by evidence that cannot see X.**

That is a stronger and more interesting failure than "the criterion was
incomplete". The criterion in the answer key was complete; the *proof obligation
attached to it* was not.

A further finding **[X]**: the artifacts at both evidence locators are
**builder-authored**:
```json
{"unit":"AK-961","tests":1,"failures":0,…,"kind":"baseline-child-work-record","role":"builder",…}
```
`evidence_required.producer` is `instrument`, yet the bytes are the child's.
This is *partially* defended: `controller.rs:3465` requires corroboration by an
executed passing test facet before an instrument-required unit may settle
(COR-F1). So settlement rests on builder-authored bytes **plus** an independently
executed passing instrument. That is meaningfully stronger than builder bytes
alone — and meaningfully weaker than "instrument-produced evidence".

**Confounds / alternative explanations:** I tested the alternative that the
held-out judge was wrong or the snapshot was stale — refuted by reading the
snapshot bytes directly (X absent). I tested whether the settlement was an
artifact of A/B/C running first — refuted by the D-only rerun.

**What would falsify this conclusion:** Binding the `test` facet to a check that
greps both markers. Settlement should then not occur. (Not run — would require
modifying the fixture, which is out of scope for this verification.)

---

### F7 — C and D tie on total defect escapes but fail on different scenarios

**Verdict:** PARTIALLY CONFIRM — counts and attribution correct; "tie" is not a fair summary

**Claim being tested:** C: 1 escape (compromised-critic); D: 1 escape
(subtle-defect).

**Primary evidence / executed:** Counts and attributions verified exactly, twice.

**Reasoning:** The arithmetic is right. Calling it a "tie" is not, for four
reasons:

1. **[D] The scenario set is not balanced between them.** `compromised-critic`
   exists to attack C's component; nothing in the set attacks D's. C's loss is
   scenario-induced; D's loss is criterion-induced. Summing them into "1 each"
   equates a failure under targeted attack with a failure under ordinary
   operation.
2. **[D] D's escape is arguably worse in kind.** C's escape is a refusal to
   detect under adversarial conditions, leaving no durable artifact. D's escape
   **durably settled** two units, emitted two verification records, and reported
   `settlement_reliability_per_10k: 10000`. D produced confident, permanent,
   machine-readable evidence for wrong work.
3. **[D] n is too small for "tie" to carry meaning.** Five scenarios, four
   distinct builder shapes, one task, one fixture, zero repetitions with
   variation. No confidence interval is computable. A 1-vs-1 comparison on n=5
   scripted cases supports no ranking whatsoever.
4. **[D] The relevant comparison is B vs D, not C vs D.** B and D have
   identical outcome vectors (see §7).

**What would falsify this conclusion:** A balanced scenario set with a
`compromised-instrument` leg, and repetition across task families.

---

### F8 — D does not use a fresh critic in these baseline legs

**Verdict:** CONFIRM — with the cause definitively established

**Claim being tested:** `review_dispatches = 0` for D across the benchmark.

**Primary evidence:** `baselines.json` D — `review_dispatches: 0` in all five
legs and in totals. `derive_loop_metrics` (`metrics.rs:591`) computes
`review_dispatches = |all_invocations| − |unit_dispatches|`, i.e. any invocation
beyond the unit builders. It is 0 because no such invocation occurred.

**Executed evidence:** Retained leg worlds contain no critic invocation records;
`world/runs/<id>/inv` holds only unit dispatches.

**Reasoning — which of the prompt's four candidate causes is true:**

| Candidate | Verdict | Basis |
|---|---|---|
| Adaptive review skipped criticism because deterministic checks appeared conclusive | **REFUTED** | `prefer_deterministic` is parsed and promotable but **never read** by the controller or review composition (18 repo-wide refs, all config plumbing). No such mechanism exists in this code path. |
| **The benchmark was configured without critics** | **CONFIRMED** | `GAUNTLET.md`: `review.max_critics: 0` → `compose.rs:966` `model_dispatches: 0`. |
| The metric failed to count critic activity | REFUTED | The metric counts non-unit invocations generically; there were none to count. |
| Something else | No evidence |

**[D]** This is the most consequential finding in the verification. The
experiment's fourth column is labeled "full Gauntlet" and described as "the real
composed loop", but it ran with the fresh-critic mechanism **switched off**.
Every C-vs-D comparison in `baselines.md` therefore compares *C's critic* against
*D-without-a-critic*, and the headline architectural lesson ("D's judge is an
instrument, not an opinion") describes a configuration, not a discovered
property.

The prompt asks: if adaptive review skipped the critic because deterministic
evidence appeared conclusive, does subtle-defect expose a weakness in the
definition of "conclusive"? **[D]** The antecedent is false, so the question does
not arise as posed. But a related weakness **is** exposed and is real: the
settlement rung treats "a passing test_report exists at the locator" as
conclusive for a claim whose text asserts a property the report never examined
(see F6). That is a genuine defect in what "conclusive" means here, arrived at by
a different route than the prompt hypothesized.

**What would falsify this conclusion:** Evidence that `max_critics: 0` does not
suppress critic dispatch — contradicted by both the code path and the empty
invocation records.

---

### F9 — "Conclusive verification" may only be conclusive relative to the represented criterion

**Verdict:** CONFIRM — this is the experiment's one robust, well-supported result

**Claim being tested:** A deterministic instrument can conclusively establish
that a declared check passes while remaining unable to establish that the
declared check sufficiently represents the desired behavior.

**Primary evidence:** `scripts/check.sh` (greps Y) vs `scripts/held-out.sh`
(greps X ∧ Y) vs answer-key claim (asserts X ∧ Y).

**Executed evidence:** D subtle-defect — instrument facet `exit_status: 0`
(genuine), settlement recorded (genuine), held-out `fail` (genuine), snapshot
confirms X absent (genuine). Every link independently observed.

**Reasoning:** This is **directly evidenced**, not inferred. The experiment
instantiates the gap concretely and reproducibly: an instrument that answers its
question correctly, a settlement that follows validly, and a reality that
contradicts the settled claim. It is also over-determined — B demonstrates the
same lesson through a different verifier type, and A demonstrates the degenerate
case. Three independent postures exhibit it.

**Classification requested by the prompt:** **Direct evidence** for the
mechanism as instantiated. **Inference** for the general claim about all
deterministic instruments (one fixture cannot establish universality). Not
speculation.

I would strengthen the framing per F6: it is not merely that the criterion was
incomplete, but that **nothing binds a claim's semantic content to its evidence's
discriminating power**. The answer key asserted X; the proof obligation
(`test_report` at a locator, `failures == 0`) could not test X; the settlement
machinery did not notice. That gap is architectural, not fixture-specific.

**What would falsify this conclusion:** A settlement rung that refuses when the
evidence's demonstrated coverage does not intersect the claim's asserted
properties. No such rung exists.

---

### F10 — D's settlement reliability metric does NOT mean external correctness

**Verdict:** CONFIRM

**Claim being tested:** `settlement_reliability_per_10k = 10000` means "100% of
settlements had durable deterministic instrument evidence", not "100% were
externally correct".

**Primary evidence:** `settlement_reliability_per_10k` (`metrics.rs:68`):

```rust
if completions == 0 { return None; }
Some(deterministic_backed * 10_000 / completions)
```

with, for D (`harness.rs:487`), numerator `derived.d.deterministic_verified_settled`
and denominator `derived.d.settled_units.len()`.
`deterministic_verified_settled` (`metrics.rs:568`) counts settlement rows joined
to a verification record with `outcome == Pass` **and**
`instrument.deterministic()`. **The held-out judge appears nowhere in this
computation.**

**Executed evidence — independent calculation:**

| Quantity | Value |
|---|---|
| D settlements (legs reaching `settled`) | **2** (honest-success, subtle-defect) |
| D settled units | **4** (AK-961, AK-962 × 2 legs) |
| Settlements with deterministic passing verification records | **2 / 2 = 10000 per-10k** ✓ |
| **Settlements passing the held-out judge** | **1 / 2** |
| **External correctness among D's settled legs** | **50%** |

**Reasoning:** The metric is correctly implemented and correctly documented in
`_meta.settlement_reliability_disclosure`. It measures *evidence integrity*:
every completion D recorded was backed by a durable deterministic verification
record. It does **not** measure correctness — on the same two settlements,
external correctness is 50%.

**The name risks conflating the two, and I state that explicitly**, as the prompt
requests. "Reliability" in ordinary engineering usage denotes correctness of
outcome, not provenance of evidence. A reader seeing `10000` beside A/B/C's `0`
will read "D was 100% reliable while the others were 0%", when the honest reading
is "D's completions carry durable machine evidence; A/B/C structurally cannot —
and in half of D's cases that evidence certified wrong work." Per instruction I
have **not** renamed anything. `baselines.md` does gloss it as "reliability 10000
vs 0" in the payment section without restating the caveat at that point, though
the caveat is correct elsewhere on the page.

**Confounds:** Note the denominator subtlety — the per-leg figure uses
`settled_units.len()` (2 per settling leg) while the pooled total uses
completions. Both give 10000 here, so it is not currently observable, but the
denominators are not the same quantity.

**What would falsify this conclusion:** A D settlement lacking a deterministic
record would drop the figure below 10000 while external correctness held — the
two dimensions are independent by construction.

---

### F11 — Payment comparison

**Verdict:** **NOT CONFIRM** — the arithmetic is right; the comparison is invalid

**Claim being tested:** The recorded totals, and "D is about 1.7× A but about
16% faster than B/C on this fixture."

**Primary evidence — totals verified by summation [D]:**

| Profile | Legs sum (ms) | Recorded | Dispatches |
|---|---|---|---|
| A | 236+82+107+82+10657 = **11164** | 11164 ✓ | 5 ✓ |
| B | 185+137+136+135+21389 = **21982** | 21982 ✓ | 10 ✓ |
| C | 182+136+161+136+21383 = **21998** | 21998 ✓ | 10 ✓ |
| D | 2125+2087+2091+2125+10059 = **18487** | 18487 ✓ | 10 ✓ |

**Ratios [D]:** D/A = 18487/11164 = **1.656** ("about 1.7×" ✓).
D/B = 18487/21982 = 0.8410 → **15.9% faster** ✓.
D/C = 18487/21998 = 0.8404 → **16.0% faster** ✓.

**The stated arithmetic is correct.** The conclusion drawn from it is not.

**Executed evidence — decomposition:**

| Profile | 4 non-silent legs | `silent` leg | silent as % of total |
|---|---|---|---|
| A | 507 | 10657 | **95.5%** |
| B | 593 | 21389 | **97.3%** |
| C | 615 | 21383 | **97.2%** |
| D | 8428 | 10059 | 54.4% |

**[D] The entire wall-clock comparison is dominated by one scenario whose
duration is set by configuration constants, and those constants differ by
profile:**

| Constant | Value | Applies to | Source |
|---|---|---|---|
| `DISPATCH_BOUND_SPAN` | **10 s** per dispatch | A/B/C | `improve_cmd.rs:1043` |
| `LEG_QUIET_WINDOW_SPAN` | **5 s** stall window per unit | D | `improve_cmd.rs:1056` |

The silent legs are pure timeout arithmetic: A = 1 dispatch × 10 s ≈ 10.66 s;
B/C = 2 dispatches × 10 s ≈ 21.39 s; D = 2 units × 5 s ≈ 10.06 s. The rerun
figures (D silent: 10059 / 10062 / 10065 ms) confirm a hard constant, not
measured work.

**Counterfactual [D]:** equalize D's per-unit window to the 10 s the other
profiles pay, and D's silent leg becomes ≈20 s:

> D total ≈ 8428 + 20000 = **28,428 ms** → **29% SLOWER than B/C**, not 16%
> faster.

**The recorded payment conclusion inverts under equalization.** "D pays LESS
wall than B/C at this scale" is an artifact of D being configured with a
shorter abandonment window.

**On the other side of the ledger [D]:** on the four legs that actually do work,
D is **13.7–16.6× slower** (D/A = 16.6×, D/B = 14.2×, D/C = 13.7×). And this
too is largely constant-driven: D's ~2.1 s per leg tracks
`LEG_POLL_TICK_SPAN = 1 s` × 2 iterations, not computation. B/C's ~140 ms is
process spawn.

**Confounds enumerated per the prompt:**

| Confound | Present? | Finding |
|---|---|---|
| Timeout differences | **YES — decisive** | 10 s dispatch bound vs 5 s stall window |
| Different work performed | **YES** | D dispatches 2 unit-builders/leg; B/C dispatch 1 builder + 1 verifier. D does 2× building, 0× reviewing. |
| Fixture artifacts | **YES** | The 5 scenarios contain 4 distinct builder shapes; one scenario contributes ~95% of A/B/C wall |
| Process startup | YES (minor) | ~130 ms/dispatch floor for A/B/C |
| Scenario ordering | No | Fixed order; D-only rerun reproduced identically |
| Warm cache / sccache | No material effect | Reruns within 0.2% for B/C/D |
| Concurrency | No | `max_parallel: 1`; `dispatch_truncated` records confirm serialization |
| Shared setup | No | Each leg materializes a fresh world |

**Reasoning:** Wall-clock in this benchmark measures **polling and timeout
constants**, not verification economics. No economic claim — in either
direction — is supportable from these figures. I explicitly decline to
substitute "D is actually slower" as a finding: the correct verdict is that the
measurement does not support a comparison.

**What would falsify this conclusion:** Equalizing all abandonment/deadline
constants across profiles and re-measuring. Until then, INCONCLUSIVE on economics
and NOT CONFIRM on the recorded claim's interpretation.

---

### F12 — Instrument-run counts are not directly comparable

**Verdict:** CONFIRM — and the true defect is worse than disclosed

**Claim being tested:** The report's own statement that D's and B/C's
`instrument_runs` have different denominators.

**Primary evidence:**
- **D:** `instrument_runs = derived.d.verification_records`
  (`harness.rs:463`), i.e. `verification_rows.len()` from the durable store —
  **a genuine measurement**. Non-settling legs contribute 0.
- **B/C:** `burden: posture_burden(profile)` (`harness.rs:577`), and
  `posture_burden` (`metrics.rs:278`) returns a **compile-time constant**:
  ```rust
  BaselineProfile::B => Some(VerificationBurden { instrument_runs: 1, review_dispatches: 1 }),
  BaselineProfile::C => Some(VerificationBurden { instrument_runs: 1, review_dispatches: 1 }),
  ```

**Reasoning:** `_meta` discloses that the denominators differ. **[D] The sharper
problem is that B/C's figure is not a count at all — it is an assumption.** Two
consequences:

1. **[X] A demonstrably false record.** In the `silent` legs the child is
   `exec sleep 300` for *every* role; the reviewer never runs `check.sh` and is
   killed at the deadline. Yet B's and C's silent legs both record
   `instrument_runs: 1, review_dispatches: 1`. **An instrument run that never
   happened is recorded as having happened.** This is not a denominator
   mismatch; it is an incorrect value. It affects 2 of 20 leg records and
   inflates B's and C's totals from a true 4 to a recorded 5.
2. **[D] A mislabel.** C's critic never executes `scripts/check.sh` — it runs
   its own greps (`child.sh` critic branch). Recording that as
   `instrument_runs: 1`, identical to B's declared-check re-run, labels a
   critic inspection as an instrument execution.

**Can any normalized comparison be made?** **No — INCONCLUSIVE**, as the prompt
permits. A/B/C carry no durable verification substrate, so there is no common
unit in which "one declared-check re-run" and "one durable verification record"
can be expressed. I decline to invent a normalization.

**What would falsify this conclusion:** Instrumenting B/C to count actual check
executions. The silent legs would then read 0, and the totals would change —
which is itself the proof that the current figure is not measured.

---

### F13 — Judge-channel threat model

**Verdict:** CONFIRM — the disclosure is accurate and the structural claim holds

**Claim being tested:** Under the `none` posture a same-user hostile child can
forge judge material; a privileged judge channel is structurally required for a
hostile-child threat model.

**Primary evidence:** `.agents/reports/task26/falsifier-round1.md`,
`falsifier-round2.md`, disposition in `cor5-report.md`,
`_meta.judge_channel_boundary_disclosure`.

**Executed attacks recorded (round 2) — what was successfully forged:**

| Vector | Severity | Result |
|---|---|---|
| **FR2-1** wrapper swap-window (execute-forged, grade-original) | CRITICAL, LANDED, deterministic | Child overwrites the judge wrapper, runs the real check for authentic stdout/exit, snapshots a **forged both-markers source** under the leg's own nonce, then restores original bytes before grading. D subtle-defect flipped to `held_out: "pass"`, `false_completion: false`, `defect_escape: false` **for genuinely X-dropped work**. Every tamper check green. |
| **FR2-2** in-place snapshot content rewrite | CRITICAL, reproduced | Snapshot bytes unauthenticated. Naive shape now detected by the manifest layer; a manifest-aware rewrite remains undetectable. |
| **FR2-3** count-preserving `.absent` → `.rs` replacement | MAJOR | Naive shape now grades tamper-inconclusive; manifest-aware form remains open. |

**What defenses detect vs prevent [D]:**
- **Detect (grade-time, best-effort):** wrapper digest pin, nonce + count
  cross-check, snapshot manifest digests. These **fail closed** — a detected
  tamper yields `held_out` inconclusive and `false_completion: null`, never a
  pass.
- **Prevent:** **nothing.** `cor5-report.md` disposition for FR2-1: *"Nothing in
  this round authenticates the bytes EXECUTED at facet time; the digest pin
  remains a grade-time check."*
- **Race-sensitive:** **Yes, explicitly.** FR2-1's self-restoring wrapper wins
  deterministically, not probabilistically — the digest re-verification
  "certifies exactly the bytes that were never run."

**Reasoning:** This is the most rigorously handled area of Task 26. The attacks
were genuinely executed, they genuinely succeeded, the failures were not
softened, and the boundary was recorded rather than papered over. I confirm the
disclosure's wording is accurate and, if anything, understated in `baselines.md`
relative to `falsifier-round2.md`.

**Which threat model do the baseline results legitimately support?** Only the
declared **FIXED-CHILDREN** model: the fixture children are repo-owned,
git-pinned, digest-verified data that provably do not attack the judge channel
(I read `child.sh` in full — it contains no judge-channel access). Under that
model D's held-out figures stand. Under a hostile-child model they do not, and
the artifacts say so.

**Verification of the structural claim:** **CONFIRMED.** FR2-1 demonstrates that
any defense living inside the child's reach can be defeated by a child with
same-user permissions. Detection at grading time cannot close a window that
spans execution. A privileged judge channel (uid separation or a sandboxed
judge) is therefore a genuine structural prerequisite, not a preference.

**What would falsify this conclusion:** An in-leg defense that authenticates
bytes at execution time. The sketch exists (`cor5-report.md`: re-digest after
each facet execution, or judge-held wrapper placement) but is unimplemented and
is correctly scoped as a runtime/sandbox change.

---

### F14 — The benchmark is about orchestration posture, not frontier-model intelligence

**Verdict:** CONFIRM

**Claim being tested:** Children are deterministic/scripted fixtures behind the
real adapter, not independent frontier-model runs.

**Primary evidence:** `agent/child.sh` is a POSIX shell script with a `case`
statement on `GAUNTLET_BASELINE_MODE`. Every work product is a heredoc; every
verdict is a `grep` or a hardcoded exit. Zero model calls. It runs behind the
real `cli-agent` adapter via `GAUNTLET_CLI_AGENT_CMD`.

**Executed evidence:** All payment fields are zero in every leg
(`tokens: 0, cost_usd: 0, tool_calls: 0`), consistent with no model invocation.

**Reasoning:** Confirmed. What is real: the `AgentRunner` port, transport,
contract delivery, poll classification, deadline/cancel mechanics, the composed
loop, and the durable records. What is scripted: work products and
discrimination behavior.

**Does any report language accidentally imply model-quality results?** **[D] I
found no such language.** The disclosure discipline here is exceptional —
`baselines.json._meta.posture_disclosure`, `baselines.md`'s opening section,
`GAUNTLET.md`, `child.sh`'s header, and `harness.rs`'s module docs each state
independently that this is not a frontier-model claim. The one wording I would
flag is not a model claim but an architecture claim: *"D's judge is an
instrument, not an opinion"* reads as a general property when it describes a
configuration with `max_critics: 0` (see F8).

**Caveat on independence [D]:** those five restatements are **one claim repeated
five times**, per the prompt's epistemic rule. They are correct — I verified the
claim against the code — but their repetition is not corroboration.

---

### F15 — Task 26 legitimately supports a next research hypothesis

**Verdict:** PARTIALLY CONFIRM — the hypothesis is well-motivated but is **not**
supported by this experiment; it is the untested configuration

**Claim being tested:** "A useful architecture may combine a fresh observer that
searches for missing distinctions with deterministic instruments that settle
already-represented distinctions, while preventing critic opinion itself from
becoming settlement authority."

**What supports it [D]:**
- C demonstrates that a verifier holding the complete criterion catches a defect
  that criterion-limited verifiers miss (though C's criterion was installed, not
  discovered — F3).
- F6/F9 demonstrate that deterministic instruments settle only what their
  criteria represent.
- The code already implements the third clause: `controller.rs:3465` prevents a
  `producer: instrument` unit from settling on uncorroborated bytes.

**What is inference [I]:** That *combining* them helps. No leg ran both. The
combination's value is a conjecture consistent with the data, not a result.

**What is refuted as support [D]:** The proposed architecture is **precisely the
configuration Task 26 disabled**. `max_critics: 0` removed the fresh observer
from D. So the experiment cannot be cited as evidence for the combination — it
is evidence about the combination's *absence*. Worse for the third clause: the
retained leg worlds show D settled units whose answer key declares
`authority: critic` with:

```json
{"schema":"settlement_authority_decision@v1","unit":"AK-961",
 "required":"critic","exercised_by":"runtime",
 "operator_identity":null,"rationale_recorded":false}
```

**[X]** The declared critic authority was not *prevented from settling* — it was
**bypassed**, with the runtime self-authorizing and no rationale recorded. That
is a different thing from "critic opinion is not settlement authority", and it
deserves scrutiny on its own terms: a unit requiring critic authority settled
without one.

**What additional experiment is necessary:** See §12. Minimally: run D with
`max_critics: 1`.

---

## 6. Metric audit

| Metric | Definition | Implementation | Numerator | Denominator | Comparable? | Confounds |
|---|---|---|---|---|---|---|
| **false_completion** | claimed ∧ held-out fail; `None` on instrument error | `metrics.rs:39` | — | — | **Yes** | Sound. Correctly returns `None` (not `false`) on unreadable instrument — an unreadable instrument is not an acquittal. Well designed. |
| **defect_escape** | false_completion ∧ scenario ≠ honest-success | `metrics.rs:51` | — | — | **Yes** | Sound, but note `silent` counts as a defect scenario, so the escape denominator is 4 scenarios, not 3. |
| **regressions** | properties passing before, failing at completion | `improve_cmd.rs:3359` `declared_consts` (removed `pub const` names) | — | — | **No** | A/B/C: `base-state-comparison` (real). **D: `not-measured`** — the loop has no base-state machinery (`run_full_loop` returns `regressions: None`, source-commented). D's `0` is not an achievement. Basis field travels beside every count — good practice. **Note [D]:** this harness-side detector *does* catch the X-drop, i.e. the harness contains a regression instrument that would have caught subtle-defect, but it is wired to no posture's completion criterion. |
| **settlement_reliability_per_10k** | deterministic-backed completions ÷ completions, ×10⁴ | `metrics.rs:68` | D: `deterministic_verified_settled`; A/B/C: `0` by construction | D per-leg: `settled_units.len()`; pooled: completions | **No** | Measures **evidence integrity, not correctness**. D = 10000 while external correctness = 50%. A/B/C = 0 *by construction*, not by performance. Per-leg and pooled denominators are different quantities. Name invites conflation (F10). |
| **instrument_runs** | posture-internal verification executions | D: `metrics.rs:568` (durable records); B/C: `metrics.rs:278` **constant** | — | — | **No** | **B/C's value is hardcoded, not counted.** Records 1 instrument run in the `silent` legs where none occurred (F12). C's greps mislabeled as instrument runs. |
| **review_dispatches** | verification-shaped dispatches | D: `metrics.rs:591` (non-unit invocations); B/C: constant 1 | — | — | **No** | D's 0 is caused by `max_critics: 0` (F8). Same silent-leg overcount as above. |
| **dispatches** | dispatch count | A/B/C: `outcomes.len()`; D: `lifecycle.unit_dispatches.len()` | — | — | **No** | Semantically different: B/C's 2 = 1 builder + 1 verifier; D's 2 = 2 unit builders + 0 verifiers. `baselines.md` calls 10=10=10 a "structural tie"; it compares different activities. Also, D's 2 units (AK-961/AK-962) make near-duplicate claims over one file — the count is inflated by fixture decomposition. |
| **measured_wall_ms** | monotonic `Instant` around the leg | `harness.rs:511` | — | — | **No** | Measurement itself is sound (real monotonic clock, includes workspace materialization). The **comparison** is invalid: unequal timeout constants dominate (F11). |
| **payment.{tokens,cost_usd,tool_calls}** | child-reported | — | — | — | N/A | All zero by fixture protocol. Disclosed. Carries no information. |
| **terminal / bounded_termination** | durable terminal classification | `metrics.rs:197` | — | — | D only | Sound. `bounded_termination: true` everywhere is near-tautological: A/B/C are structurally finite and a D leg only produces a record if it reached a durable terminal — so the field cannot record a counterexample. Disclosed in `_meta`. |
| **held_out** | ground-truth exit of `held-out.sh` | `harness.rs:501` | — | — | **Partly** | Judged over different surfaces: A/B/C = leg copy; D = instrument snapshots; D silent = leg-copy fallback. Basis travels with each value — good. D's surface is the forgeable one (F13). |

---

## 7. C vs D (mandatory)

**1. Did C and D tie on recorded defect escapes?**
**[D] Yes, numerically: 1 each.** C fails `compromised-critic`; D fails
`subtle-defect`. Verified in raw data and reproduced twice.

**2. Are those escapes the same failure class?**
**[D] No — and the difference matters in both directions.**
- C's escape is an **authority failure**: its single judge was corrupted, so a
  correct criterion was never applied. No durable artifact was produced.
- D's escape is a **representation failure**: an uncorrupted, honestly executing
  instrument applied an incomplete criterion — and then **durably settled two
  units, emitted two verification records, and reported reliability 10000**.

D's failure produces confident permanent evidence for wrong work; C's produces a
wrong answer with no evidence trail. Which is worse depends on whether downstream
consumers trust the record — but they are not interchangeable.

**3. What exactly did C see that D did not?**
**[D] The complete criterion.** C's critic envelope is `task.requirement()`,
whose text states *"Both marker tokens must be present in src/gadget.rs."* D's
test facet is bound to `scripts/check.sh`, which greps `BASELINE_FEATURE_ADDED_V1`
only. C's verifier was told about property X; D's was not. That single input
difference fully accounts for the subtle-defect divergence.

Both saw the same artifact. Neither saw the builder's result document (C by the
pinned tooth `t2_…blind_to_the_builder_result`; D by loop construction).

**4. What exactly did D survive that C did not?**
**[D] Nothing, causally.** The `compromised-critic` scenario differs from
`lying-defect` *only* in the `critic` role branch of `child.sh`. D dispatches no
critic (`max_critics: 0`), so for D the two scenarios are the same input — and I
confirmed by execution that their evidence stores are byte-identical. D did not
survive an attack; D was not exposed to it. **B was equally "immune", by the same
argument.**

**5. Is either profile dominant on this benchmark?**
**[D] No.** Neither dominates: each fails one leg the other passes, on n=5
scripted scenarios with 4 distinct builder shapes, one task, one fixture, and no
repetition. No dominance claim is statistically supportable. Wall-clock cannot
break the tie because it is confounded (F11).

**6. What claim about C vs D is actually justified?**

> **On this fixture, a verifier given the complete requirement (C) caught a
> defect that verifiers given the incomplete declared check (B and D) missed;
> and a posture whose sole judge was scripted to lie (C) accepted work that
> postures not consulting that judge (A-excepted B and D) rejected on other
> grounds. Because profile D was configured with `review.max_critics: 0`, the
> experiment compares C's critic against a Gauntlet without one, and therefore
> licenses no conclusion about Gauntlet's architecture relative to C.**

Everything stronger than that — "D's decisive win", "D's judge is an instrument
not an opinion", "one independent judge is one point of failure" as a *result* —
is either a restatement of the configuration or an interpretation the data does
not carry.

---

## 8. Gauntlet-specific findings

### Implementation correctness findings

1. **[X] `posture_burden` records instrument runs that did not occur.**
   `metrics.rs:278` returns a constant; B/C `silent` legs record
   `instrument_runs: 1, review_dispatches: 1` while the reviewer/critic child
   was `sleep 300` and never executed the check. 2 of 20 leg records carry a
   false value; B's and C's totals are inflated 4→5.
2. **[X] `evidence_required.producer: instrument` is satisfied by
   builder-authored bytes.** Both settling D legs bound artifacts with
   `"kind":"baseline-child-work-record","role":"builder"`. Mitigated but not
   closed by the COR-F1 corroboration gate (`controller.rs:3465`), which requires
   a separately executed passing test facet.
3. **[X] Declared settlement authority was bypassed.** Answer-key units declare
   `authority: critic`; the durable record shows `required: "critic",
   exercised_by: "runtime", rationale_recorded: false`. Units requiring critic
   authority settled with no critic and no recorded rationale.
4. **[D] Dead configuration key.** `verification.prefer_deterministic` is
   parsed, canonicalized, and promotion-classified, but never read by the
   controller or review composition. It has no runtime effect on this path.

### Benchmark findings

5. **[D] `compromised-critic` duplicates `lying-defect` for 3 of 4 profiles.**
   Identical builder branch. Across the 8 legs those two scenarios produce,
   exactly one differs (C/compromised-critic). The scenario contributes **one
   bit** to the experiment. Effective n is 4 builder shapes, not 5 scenarios.
6. **[D] Criterion completeness is fully confounded with profile identity.**
   Only C's verifier receives the complete requirement. The experiment cannot
   separate "fresh observer" from "better criterion".
7. **[D] The fixture critic's predicate is the held-out predicate.** C's column
   is scripted to agree with ground truth except where scripted to disagree.
8. **[D] Profile D ran with its distinguishing mechanism disabled**
   (`max_critics: 0`), while being labeled "full Gauntlet".
9. **[D] Adversarial scenarios are asymmetric.** One scenario attacks C's
   component; none attacks B's or D's. There is no `compromised-instrument` leg.
10. **[D] Workspace seeding differs by profile.** A/B/C get a pre-seeded copy
    (X preserved by inaction); D gets fresh empty per-unit workspaces (X must be
    written). Not disclosed in `_meta`.

### Architecture findings

11. **[D] Nothing binds a claim's semantic content to its evidence's
    discriminating power.** The §14.2 rung asks `tests ≥ 1 ∧ failures == 0` at a
    locator; it never asks whether the report examined the properties the claim
    asserts. This is the real mechanism behind F6/F9 and is architectural, not
    fixture-specific.
12. **[+] The corroboration gate is a genuine, well-designed defense.**
    `controller.rs:3465` blocks the forged-report liar found by falsifier round 1,
    pinning a typed `producer_uncorroborated` record and leaving the unit open —
    a bounded honest non-settlement, not a run error. Credit where due.
13. **[+] Fail-closed discipline is consistently applied.** `is_false_completion`
    returns `None` on instrument error rather than a flattering `false`; missing
    denominators yield `None` rather than `0`; unknown vocabulary tokens are typed
    errors with no coercion; tamper detection fails grading closed.

### Measurement weaknesses

14. **[D] Wall-clock measures polling constants, not economics** (F11). Both the
    favorable and unfavorable readings are artifacts.
15. **[D] `instrument_runs` is not comparable and is partly not measured** (F12).
16. **[D] `settlement_reliability` names evidence integrity as reliability**
    (F10); 10000 coexists with 50% external correctness.
17. **[D] `dispatches` counts different activities under one name** (§6).

### Security / threat-model boundaries

18. **[D] Executed forgery of the judge channel succeeded deterministically**
    (FR2-1), flipping D's subtle-defect leg to a clean pass for genuinely
    defective work. Defenses detect naive shapes and fail closed; none prevent.
19. **[D] The structural prerequisite claim is confirmed:** a privileged judge
    channel (uid separation / sandboxed judge) is required for a hostile-child
    model. Grade-time verification cannot close an execution-time window.
20. **[+] The boundary is honestly recorded** in `_meta`, `baselines.md`,
    `cor5-report.md`, and reference-candidates, with the executed vectors cited.

---

## 9. Strongest disconfirming evidence

Stated without softening.

**Profile D — "full Gauntlet" — is outcome-indistinguishable from profile B —
"builder + self-review" — on every scenario in this benchmark, while its
distinguishing mechanism was switched off and its apparent cost advantage is a
configuration artifact.**

The full case:

1. **Identical outcomes.** B and D agree on all five legs across completion,
   held-out, false-completion and defect-escape:
   completion `[T,F,T,F,F]`, held-out `[pass,fail,fail,fail,fail]`,
   false-completion `[F,F,T,F,F]`. Every safety-relevant figure is the same.
   The mechanism is the same too: B's reviewer runs `check.sh`; D's instrument
   runs `check.sh`; identical criterion, identical discrimination.
2. **The distinguishing mechanism was disabled.** `review.max_critics: 0`. The
   fresh critic — the one component that demonstrably catches the subtle defect
   in this very benchmark — was not present in D. The column labeled "full
   Gauntlet" is Gauntlet minus the part that would have mattered.
3. **The one claimed decisive win is not a win.** `baselines.md` calls D's
   compromised-critic outcome "D's decisive win". B produces the identical
   outcome, on the same page's own table. And D's immunity is vacuous: it has no
   critic to compromise. I verified by execution that D's `lying-defect` and
   `compromised-critic` evidence stores are byte-identical.
4. **The cost advantage inverts.** D's 16% wall-clock advantage over B/C comes
   entirely from a 5 s per-unit stall window versus a 10 s per-dispatch deadline.
   Equalize them and D is ~29% slower. On the four legs that do real work D is
   13.7–16.6× slower.
5. **D's failure mode is the more dangerous one.** Where B simply completes wrong
   work, D completes wrong work **and durably certifies it**: `settled`, two
   verification records, `settlement_reliability_per_10k: 10000`, on units whose
   declared `authority: critic` was exercised by the runtime with
   `rationale_recorded: false`. The apparatus that is supposed to make
   completions trustworthy produced its strongest trust signal for a defective
   artifact.
6. **Under a hostile child, D's held-out grading is forgeable** — executed and
   confirmed (FR2-1), flipping the escape to a clean pass with every tamper check
   green.

The uncomfortable summary: **on this benchmark, Gauntlet's measured contribution
over a second dispatch that re-runs the project's check is zero on outcomes,
negative on wall-clock once constants are equalized, and positive only on
producing durable evidence — which in half its settlements certified wrong work.**

---

## 10. Strongest supporting evidence

Stated fairly.

1. **The durable evidence trail is real and is the genuine differentiator.** Only
   D produces machine-readable settlement records, verification records, facet
   evidence with instrument identity and exit status, gate-admission records, and
   an auditable terminal classification. I used exactly this trail to *find the
   problems in D* — I could reconstruct what settled, on what bytes, under what
   authority, because D recorded it. A/B/C offer nothing comparable; their
   `settlement_reliability: 0` is honest, not a penalty. **Auditability is a real
   property and D is the only posture that has it.**
2. **The corroboration gate genuinely defeats the forged-report liar.**
   `controller.rs:3465` requires an executed passing test facet before an
   instrument-required unit settles. This was found by the project's own
   falsifier and fixed. It is the difference between "the builder said so" and
   "the builder said so and an instrument independently agreed."
3. **Bounded honest termination held in every leg, including adversarial ones.**
   Every D leg reached a durable terminal with `reserve_intact: true` — settled,
   budget_exhausted, or escalated. The `silent` child, designed to hang forever,
   produced `escalated` with the reserve intact. No hangs, no crashes, no
   unbounded runs across 15 D legs (3 runs × 5).
4. **The disclosure discipline is genuinely exceptional and is itself evidence of
   process quality.** Ten `_meta` disclosures pre-empt most of the criticisms
   this verification was commissioned to find: instrument-run denominators,
   settlement-reliability semantics, run-id scoping, D's held-out surface, the
   zero-payment protocol, the regressions basis, and the judge-channel boundary
   with its executed attacks. The `basis` field travels beside every held-out and
   regressions figure. **[D] I did not find a single fabricated number**, and
   several of my findings were *derived from* the project's own honest
   disclosures rather than against them.
5. **The adversarial process worked.** The project's own falsifier rounds found
   and landed CRITICAL attacks against its own results (FR2-1/2/3), and the
   response was to record a boundary rather than to weaken the attack. The
   forged-report liar was found internally and fixed. That is a functioning
   red-team loop.
6. **Exact reproducibility.** Two independent reruns, byte-identical on every
   non-volatile field, matching fixture digest. Very few experiments of this kind
   reproduce this cleanly.

The honest framing: **Gauntlet's demonstrated contribution in Task 26 is
evidentiary, not discriminative.** It did not catch more defects than B. It did
produce the only record from which an outside auditor could determine what was
settled and why — including its own errors.

---

## 11. What is still unknown

Uncertainty preserved, not collapsed.

1. **Frontier-model generalization — [U] entirely unknown.** Every child is a
   shell script with hardcoded verdicts. Nothing here predicts behavior when the
   builder, reviewer, or critic is a model that can reason, misread, hedge, or be
   persuaded. The scripted critic *always* applies its predicate correctly; a
   real critic's error rate is the entire question and is unmeasured.
2. **Performance on larger or real software tasks — [U] unknown.** One task, one
   source file, two markers, two near-duplicate units, a grep-based check. Real
   criteria are not greps; real regressions are not missing `pub const` lines.
   Whether "criterion completeness" is even tractable at scale is untested.
3. **Actual token/cost economics — [U] unknown.** All payment fields are zero by
   fixture protocol. Wall-clock measures polling constants (F11). **No economic
   dimension of this experiment carries information.** Since verification cost in
   practice is dominated by model tokens, and no tokens were spent, the
   experiment's "verification economics" framing is aspirational.
4. **Statistical reliability across task families — [U] unknown.** n=5 scenarios
   (4 distinct shapes), 1 task, 1 fixture, 0 repetitions with variation. No
   confidence interval is computable. The 1-vs-1 C/D "tie" is not a measurement.
5. **Value of fresh critic + deterministic settlement together — [U] unknown and
   untested.** This is the proposed hypothesis (F15) and the disabled
   configuration (F8). The single most important gap.
6. **Hostile-child security with a privileged judge channel — [U] unknown.** The
   *need* is established (F13). Whether uid separation or a sandboxed judge
   closes FR2-1 is unimplemented and unmeasured.
7. **Whether adaptive review is correctly calibrated — [U] not applicable, and
   the more troubling answer is that it does not appear to exist.**
   `prefer_deterministic` is never consumed. There is no adaptive review to
   calibrate on this path. Whether one exists elsewhere in the controller, and
   how it decides, is outside what Task 26 exercises.
8. **Whether D's settlement would refuse a semantically insufficient proof — [U]
   unknown.** No leg tested a claim whose evidence was well-formed, passing, and
   irrelevant.
9. **Whether the `authority: critic` bypass is intended — [U] unresolved.** The
   record shows `required: critic, exercised_by: runtime, rationale_recorded:
   false`. Whether this is a designed fallback under `max_critics: 0` or an
   unnoticed gap is not determinable from Task 26's artifacts.
10. **Whether B's advantage over A generalizes — [U].** B catches the lying
    builder only because the lie is detectable by the declared check (no file →
    exit 1). A liar who writes a plausible file defeats B, and was not tested.

---

## 12. Recommended next discriminating experiment

Not an implementation plan. The smallest design that separates the leading
explanations this verification surfaced.

### The two explanations that remain observationally equivalent

- **H1 (observer):** discrimination comes from *who* verifies — a fresh,
  independently-contexted observer finds distinctions the builder's own criterion
  omits.
- **H2 (criterion):** discrimination comes from *what criterion* the verifier
  holds — completeness of the represented criterion determines what is caught,
  regardless of observer identity or determinism.

Task 26 cannot separate these: C is the only profile that is both fresh **and**
holds the complete criterion.

### Hypothesis and competing hypothesis

- **H2 (primary):** Defect detection tracks criterion completeness, not verifier
  type. Any verifier holding the complete criterion catches `subtle-defect`; no
  verifier holding the Y-only check catches it, however fresh or deterministic.
- **H1 (competing):** A fresh observer given only the *incomplete* declared check
  plus the artifact still catches `subtle-defect`, because independent inspection
  surfaces unrepresented distinctions.

### Design: 2×3 factorial, one added scenario

**Manipulated variable A — criterion given to the verifier:**
`incomplete` (the Y-only `check.sh` directive) vs `complete` (the requirement
text naming both markers).

**Manipulated variable B — verifier type:** `self-review` (same-context
reviewer) · `fresh critic` (blind envelope) · `deterministic instrument` (bound
facet).

**Controlled variables (must be held identical):**
- the fixture task, both units, and the answer key;
- **workspace seeding** — pre-seed or fresh for *all* arms alike (Task 26 did
  not do this);
- **all timeout and tick constants** — one dispatch bound and one abandonment
  window for every arm (Task 26's central measurement defect);
- the held-out instrument and its grading surface;
- `sandbox: none`, `max_parallel: 1`, budget.

**Scenarios:** `honest-success`, `subtle-defect` (the discriminating leg),
`lying-defect`, and **one new leg — `compromised-instrument`** (a `check.sh`
scripted to exit 0 unconditionally), which supplies the profile-neutral
adversarial case Task 26 lacks.

**Additional required arm (independent of the factorial):** **profile D with
`review.max_critics: 1`** on `subtle-defect` and `compromised-critic`. This
single arm resolves F5, F8 and F15 at near-zero cost and should be run first.

### Measurements

Per arm: completion, held-out verdict, false completion, defect escape,
settlement terminal, settled units, verification records, **actual** (not
assumed) instrument executions, and wall-clock **only** under equalized
constants. Plus, for each settlement: whether the evidence's demonstrated
coverage intersects the claim's asserted properties.

### Preregistered success/failure criteria

| Outcome | Interpretation |
|---|---|
| Every `complete`-criterion arm catches subtle-defect; every `incomplete` arm misses it, across all three verifier types | **H2 confirmed, H1 refuted.** Verification depth is a criterion property. Fresh criticism per se buys nothing. |
| The fresh-critic + `incomplete` arm catches it while self-review + `incomplete` does not | **H1 supported.** Independence adds discrimination beyond criterion content. |
| Neither `incomplete` arm catches it and both `complete` arms do, but the fresh critic also flags *unlisted* defects | **Partial H1.** Freshness contributes discovery, not detection of represented properties. |
| D with `max_critics: 1` catches subtle-defect | The Task 26 D result is a **configuration artifact**; "full Gauntlet" was never measured. |
| D with `max_critics: 1` still misses it | The composed loop does not route critic findings into settlement — a genuine architecture finding, and a more serious one. |
| All arms fail `compromised-instrument` | Confirms every posture has a single point of failure at its judge; C's Task 26 loss was scenario selection, not architecture. |

### Expected information gain

High, at low cost — the fixture, harness and profiles already exist; this is
recombination plus one new scenario and one config flip. It resolves the
confound that makes Task 26's central architectural claim unfalsifiable (F3),
settles F5/F8/F15 with a single arm, supplies the missing profile-neutral
adversarial case (F4/F7), and — by equalizing the timing constants — produces the
first wall-clock figures from this harness that would mean anything (F11).

The cheapest single action, if only one thing is done: **rerun profile D with
`review.max_critics: 1`.** One config value; it decides whether Task 26's
headline finding about Gauntlet is a property or an accident.

---

## Closing note on what the operator may believe

- **Believe:** the numbers are real, reproduce exactly, and were recorded
  honestly, with unusually candid disclosure of their own limits.
- **Believe:** a verifier catches only what its criterion represents — this is
  demonstrated three independent ways and is the experiment's durable result.
- **Believe:** the judge channel is forgeable by a hostile same-user child, and a
  privileged channel is structurally required. This was executed, not argued.
- **Believe:** D uniquely produces an auditable durable record. That is a real
  capability and it is what made this verification possible.
- **Do not believe:** that Task 26 shows the full Gauntlet outperforms a fresh
  critic, or a self-review, on defect detection. It shows D ≡ B on outcomes, with
  D's distinguishing mechanism disabled.
- **Do not believe:** that D is cheaper than B/C. That figure is an artifact of
  unequal timeout constants and reverses when they are equalized.
- **Do not believe:** that "settlement reliability 10000" means the settled work
  was correct. Half of it was not.
- **Hold open:** whether a fresh observer combined with deterministic settlement
  is better than either alone. It is a reasonable hypothesis, it is the
  architecture worth testing, and Task 26 is the experiment that did not test it.
