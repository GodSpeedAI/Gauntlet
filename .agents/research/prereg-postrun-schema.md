# Preregistration `post_run` schema and scoring rules

**Status:** proposal against `.agents/research/preregistrations/`. Research instrumentation, non-normative — same posture as the prereg files themselves. Nothing here gates a task or touches the ruler.

**What it is for.** The prereg establishes what was expected before the work existed. The post_run block establishes what was observed, *typed by how it differed*. The differential is the second-order signal: one residual is a bug, but a residual class repeating along the same dimension is a representational defect. That structure is only visible if the differentials are typed consistently across tasks, which free prose cannot support.

**Filling rule.** The post_run block is written by whoever closes the task, after closure, and appended to the frozen prereg file — never by editing the `pre_run` block, whose immutability is the whole point. Append-only, same discipline as the reference-candidate queue.

---

## Residual taxonomy

Every prediction, blind spot, and risk resolves to exactly one type.

| Type | Definition | Representational implication |
|---|---|---|
| `match` | Expectation held, in the form stated, at roughly the stated magnitude. | None. Confirms current representation. |
| `calibration` | Right distinctions named; wrong magnitude, timing, count, or severity. | Tune parameters. Do not restructure. |
| `blindspot` | The outcome turned on a distinction the prereg never named. | Missing representation → **differentiate**. |
| `spurious` | A named distinction turned out not to matter. | Over-representation → **integrate or retire**. |
| `misattribution` | Right distinction named; wrong causal role, wrong location, or wrong mechanism assigned to it. | Relation defect → re-encode, not add. |
| `vacuous` | The prediction could not have failed as stated, or its null result is uninterpretable. | **No signal.** Excluded from all rates. |

`misattribution` is the highest-value type and the easiest to mislabel as `match`. P2 in the Task-13 prereg is the live example: it names three candidate seams for the R1-class defect. If an R1-class seam appears at a *fourth* location not among the three, that is `misattribution`, not `match` — the class was right, the localization was wrong. Scoring it `match` because "P2 fired" discards exactly the information the second-order loop needs.

`vacuous` must be counted separately and excluded from the denominator of every rate. Folding it into `match` is the single most common way a calibration series lies. Two things route here: a prediction whose stated outcome was already established by recon (Task-13 P1), and a prediction whose miss is uninterpretable because it is conditioned on a bounded search (Task-13 P2/P4/P5 as currently written).

---

## Schema

```yaml
post_run:
  closed_at:
    git_commit: string          # closure commit
    timestamp_utc: string
    rounds:
      correction: int
      confirmation: int
      to_first_confirm: int     # confirmation rounds until first CONFIRM

  predictions:
    - id: P1                    # must match a pre_run prediction id
      type: match | calibration | blindspot | spurious | misattribution | vacuous
      prior: float              # 0-1, the writer's honest pre-run belief; see note
      informativeness: baseline | routine | risky
      observed: >
        What actually happened, in one or two sentences. Not a defence of
        the prediction.
      divergence: >
        Present unless type is `match`. States precisely what differed:
        the unnamed distinction (blindspot), the wrong location or mechanism
        (misattribution), the distinction that did not matter (spurious).
      resolution_evidence: [string]   # report paths, test names, commits
      interpretable: bool       # false if the outcome could not distinguish
                                # hypotheses — e.g. a bounded-search null.
                                # Uninterpretable outcomes are `vacuous`.

  blindspots:
    - index: 0                  # position in pre_run.known_blindspots
      materialized: yes | no | partial
      cost_if_materialized: >
        Rounds, rework, or deferral attributable to it. Empty if `no`.
      note: >
        If `no`: was declaring it wasted attention, or did declaring it
        cause the mitigation that prevented it? These are different and
        the distinction is not recoverable later.

  unforeseen:
    - id: U1
      description: >
        A material defect, seam, or class encountered that no prediction,
        blind spot, or risk anticipated. These are the pure blind spots and
        the most important entries in the file.
      class: >
        Existing residual class name if it belongs to one, else `new`.
      would_a_prior_prereg_have_caught_it: yes | no
      first_seen_task: int|null

  interventions:
    - risk_id: R1
      fired: bool
      class_used: A1..A5 | unpreregistered
      matched_preregistered: bool
      note: >
        If `unpreregistered`, name the class used and whether it should be
        added to the standing intervention vocabulary. Unpreregistered
        interventions are where post-hoc rationalization re-enters.

  residual_classes_touched:
    - name: string              # e.g. "R1 caller-supplies-both-sides"
      instance_number: int      # nth observed instance of this class
      predicted_in_advance: bool

  method_burden:
    findings: {critical: int, major: int, minor: int}
    findings_that_evaded_a_prior_defense: int
    toothless_checks_found: int
    residuals: {opened: int, closed: int, deferred: int}
    interruptions: int
    green_gate_was_wrong: bool
    operator_adjudication_minutes: float|null   # null where unrecorded;
                                                # never backfill by estimate
```

**On `prior`.** Written post-hoc but about the pre-run belief, so it is the one field in the block that is not blind — record it, do not trust it in isolation, and treat any series where every prior is above 0.9 as evidence of hindsight leakage rather than of good judgment. The honest use is coarse: `baseline` for what recon already established, `routine` for expected-but-not-established, `risky` for predictions that forbid a region of outcome space.

---

## Scoring rules

**Rates, all computed over interpretable predictions only** (`type != vacuous`):

```text
hit_rate          = |match| / |interpretable|
calibration_rate  = |calibration| / |interpretable|
blindspot_rate    = (|blindspot| + |unforeseen where class == new|) / |interpretable|
spurious_rate     = |spurious| / |interpretable|
misattribution_rate = |misattribution| / |interpretable|
vacuity_rate      = |vacuous| / |all predictions|
```

**`vacuity_rate` is a quality metric for the prereg, not for the work.** It rises when predictions retreat toward safety. Rising vacuity with rising hit rate is the hedging signature — the agent has learned it is scored on accuracy and is buying accuracy with width. Track them together or neither number means anything.

**Directional expectations, if the representational-development claim is true:**

| Series | Expected direction | What the opposite means |
|---|---|---|
| `blindspot_rate` | falling | representation not acquiring the distinctions that matter |
| `spurious_rate` | **falling** | accretion: representation growing monotonically, burden rising |
| `vacuity_rate` | flat or falling | hedging; predictions retreating to safety |
| `rounds_to_first_confirm` | falling at fixed difficulty | the headline burden claim |
| `findings_that_evaded_a_prior_defense` | ambiguous alone | rising = critics sharpening **or** defenses degrading |
| `toothless_checks_found` | falling | disambiguates the row above: falling toothless with rising evasions = critics sharpening; both falling = defenses may be degrading unobserved |
| `predicted_in_advance` on class instances | rising | class-level learning is transferring into the prereg |

**`spurious_rate` is the neglected half and the one most likely to embarrass the claim.** Everyone measures whether blind spots fall. Almost nobody measures whether the representation is shedding distinctions that stopped mattering — because keeping a check that no longer earns its cost never hurts locally. If `blindspot_rate` falls while `spurious_rate` is flat, the representation is growing monotonically and burden should be *rising*, which contradicts the framework's central claim. Retirement is exactly as load-bearing as acquisition, and it is the operator that never fires unprompted.

**Difficulty confound.** Task N is not Task 3, so a falling `rounds_to_first_confirm` across the whole series is confounded by task variation. The series is suggestive; the load-bearing comparisons are the near-matched pairs where difficulty is roughly held fixed and representational state varies:

- **Task 10 internal:** rounds 1–6 under provenance-as-value-validation vs. rounds 7–10 under provenance-as-conferred-authority. Same task, same ruler, same critics, one representational change between them.
- **The E4/F1/F3 re-litigation:** ledger-mandated re-encounter of the same defect classes later, with better representation available.
- **AK-014 evasion search:** Task 4 versus its subsequent reappearances.

Those three are worth more than the aggregate curve.

---

## Two repairs to `pre_run`, for the next prereg

Both are cheap and both address defects visible in the Task-13 file.

**1. Bounded-search predictions need a positive fallback.** A prediction of the form "the blind critic will find X" resolves as `vacuous` on a miss, because the null is uninterpretable — bounded search cannot distinguish absence from scope. Add a fallback clause and the null becomes decidable:

```yaml
- id: P2
  prediction: >
    ...
  expects: fire_during_critique
  fallback_if_not_found: >
    If no critic surfaces this seam, the closer writes the failing test
    directly against the named candidates. If that test passes, the
    prediction is FALSE — the seam does not exist — rather than unresolved.
  declared_critic_scope: >
    What the critics were briefed to search. A miss inside declared scope
    is a real miss; a miss outside it is scope, not evidence.
```

**2. At least one prediction per task must forbid a region of outcome space.** In the Task-13 file only P3 does this ("no intermediate outcome is expected"). A prediction set where every outcome was anticipated cannot register that the problem model was wrong — only that it was incomplete. The available candidate is sitting in R1:

```yaml
- id: P6
  prediction: >
    The authority-minted admission pattern (A5) is the correct containment
    shape for the settlement write, as it was at Tasks 10, 11, and 12.
  expects: match
  informativeness: risky
  falsification_consequence: >
    If critics show the settlement write requires a materially different
    containment shape, then R1 is not one class and the four prior
    instances were coincidence. That is a restructuring finding about the
    residual taxonomy itself, not a patch to Task 13.
```

`falsification_consequence` is the field that makes a prediction risky rather than merely stated: it names, in advance, what being wrong would cost. A prereg where no entry has one is a problem model, not a bet.
