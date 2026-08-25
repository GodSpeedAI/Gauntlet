# Task 12 — correction FINISHER F1: scheduler wiring + per-tick re-read

You are a correction finisher in `/home/sprime01/projects/gauntlet`. The
correction round was cancelled by operator direction mid-way. **You are
slice 1 of 3.** Do only what this brief lists.

## Read first (in this order)

1. `AGENTS.md`
2. `.agents/reports/task12/correction-adjudication.md` — authoritative C1–C9
3. `.agents/reports/task12/residual-readjudication.md` — the orchestrator's
   mid-round re-audit; it **supersedes** the C-list's scope
4. `.agents/reports/task12/correction-checkpoints.md` — what already landed,
   plus the cancelled builder's recorded designs in its preamble
5. `.agents/reports/task12/critic-b-result.md` §A3, §A4 (the source findings)

You are NOT blind — read any task12 report you need.

## CRITICAL reconciliation facts (do not re-derive, do not re-implement)

- **C1, C2, C3, C4 are DONE** and checkpointed.
- **C5 (all five sub-parts) IS ALREADY IN THE TREE, UNRECORDED.** The
  cancelled builder worked past its last checkpoint entry (file mtimes
  09:57–10:11 vs the 10:20 stop) and was killed before writing it. Do **not**
  re-implement C5. Your item 0 is to *verify and record* it.
- **C9.1** (`Reservation` mint gating) landed early as a C2 prerequisite.
- The tree is currently **lint-red**: `cargo check -p gauntlet-app
  --all-targets` warns `function compose_with_policy is never used`
  (`controller.rs:3657`), and `just lint` is `clippy --all-targets -D
  warnings`.

## Method (mandatory)

- **Fail-first per item.** Write/adjust the discriminating test, *observe the
  failure and record its exact message*, then fix. A fix with no observed
  prior failure is not evidence.
- **Checkpoint after EVERY item** — append to
  `.agents/reports/task12/correction-checkpoints.md`. Sessions die here; an
  unrecorded landed item is what created this brief.
- Result contract: `.agents/reports/task12/correction-f1.md`.
- Preserve epistemic claim type: report bounded claims. "Not found within
  <scope>" — never universal absence.

## Items

### 0. C5 — verify and record (no implementation)

Confirm each of the five C5 sub-parts is present and *asserted*, then write
the missing checkpoint entry honestly labelled as landed-by-the-cancelled-
builder-and-verified-by-you (not as your own implementation):

- C5.1 `CorrectionSelection::new` refuses an empty rejected list —
  `gauntlet-domain/src/correction.rs:448-457`, error variant `:382-387`,
  pinning test `:573-596`.
- C5.2 selection recorded on the **residual (correcting)** iteration —
  `controller.rs:1791-1798`, plus the completion path `:1808-1810`.
- C5.3 real candidates, non-zero forward ask on the correcting path —
  `controller.rs:2006-2018`, `:2043-2077`. (A zero ask on the *completion*
  path is recorded intent, not a residue — leave it.)
- C5.4 `required_authority` from the real unit fact vs a distinct
  `exercisable_authority` (`Authority::Critic`, the §17.1 ceiling today) —
  `controller.rs:2053` vs `:2085-2089`. Do not raise it; that is L1/Task 14.
- C5.5 no hardcoded tier list erroring cross_cutting —
  `controller.rs:2090-2093`, `:2309-2316`. **Do not "fix"**
  `dispatchable_tier_capacity` admitting cross_cutting at the global parallel
  bound — that is critic-A m7, declared Task-14 scope; touching it regresses
  C5.5.

Verification bar: run `cargo nextest run -p gauntlet-app control::` and
`cargo nextest run --test p4_corrective_loop --test p6_budget_control`, and
cite the composed C5 assertions (`p4:1252-1296`, `p6:933-953`). If any
sub-part is in fact absent, say so plainly and implement it fail-first.

### 1. C6.1 — over-capacity meter scope refusal must skip-and-continue

`controller.rs:1129-1132` — `state.meter.reserve(...).map_err(ControlError::Meter)?`
propagates **any** `BudgetMeterError` out of `dispatch` → `plan_and_dispatch`
→ `drive` → the whole run returns `Err`. The parallelism scope refusal is at
`gauntlet-adapter-meter-memory/src/lib.rs:179-192`.

Reproduction (critic B's): 3 independent contained units, `max_parallel = 2`,
nothing in flight → the 3rd reserve refuses → run errors.

**Authoritative grounding** (quote it in the fix's doc comment):
- §10.4: "a fan-out that would breach the recovery reserve is **truncated** in
  dimension-priority order, and the truncation is **recorded** as
  `unavailable_evidence`, not silently dropped."
- §21.3 degradation table: **only** "Integrity violation" aborts the run.
  An over-capacity refusal is not one.
- §11.2: the meter check is a per-Unit **eligibility** conjunct — its refusal
  is a skip, not a run error.

Distinguish the **scope/parallelism refusal** (skip this dispatch, keep the
loop alive, leave the unit eligible next iteration, record the truncation)
from a genuine forward-ceiling breach (which already routes to
`budget_exhausted` via C1/C2 — do not disturb that path).

### 2. C6.2 — full graph membership with separate eligibility

`controller.rs:931-941` filters `PlanInput.units` to in-memory-open units;
`scheduler.rs:703-711` `validate_graph` requires every `depends_on` target to
be **present in the input** → the first plan after a dependency settles fails
`UnknownDependency` and errors the run. Multi-hop graphs cannot complete.

**This is a TWO-PART fix. Naively re-including settled members breaks two
other things** — diagnose against the current code, do not copy a prior patch:

- eligibility conjunct 2 (`scheduler.rs:468-481`) resolves a dependency's
  status from `input.units` — the **file-parsed** status. The controller never
  writes status back to the answer key, so a settled dependency's file status
  reads `open` forever and you would trade `UnknownDependency` for
  `DependencyUnresolved`.
- conjunct 1 (`scheduler.rs:452-457`) returns `StatusNotOpen` for the
  re-included settled members, and C1's `classify_no_admission`
  (`controller.rs:1049-1053`) treats operator-frozen `StatusNotOpen` as a
  **governance block** — a fully-settled run would terminate `blocked`.

The cancelled builder's recorded design (checkpoints preamble, C6) is a new
`statuses: BTreeMap<UnitId, UnitStatus>` projection on `PlanInput`, durable
status authoritative, consulted by **both** `validate_graph`'s membership
check and conjunct 2, with eligibility filtering separately. Evaluate that
design against §11.2 and §5.3.2 ("`status = settled` requires … a
`VerificationRecord` with `Outcome = PASS`" — durable status is the fact) and
adopt it *or a better-grounded one*; if you deviate, say why in the report.
**It must agree with item 3's answer to "which source wins for a queued
unit".**

### 3. C7 — the per-tick re-read is authoritative for ALL units

§11.4 step 3: "Re-read the answer key … The in-memory snapshot is never
authoritative over the file." Today `reread_answer_key`
(`controller.rs:1068-1111`) re-parses **all** units into `state.projections`
(`:1109`) — so the "all units" half exists — but the projection's status is
**inert**: eligibility runs on `state.unit_states` (`:933-936`), which the
re-read never writes (`unit_states` writers are `seed_unit_states` `:791-794`
via `or_insert` only, the settle path `:1745-1748`, `queue_retry_dispatch`
`:2391-2393`, and the resume restore `:734`).

Three legs:

- **C7.1** propagate each **queued** unit's fresh file status into the loop's
  authoritative state, so an operator hand-edit marking a queued unit
  `settled`/`out_of_scope` is honoured rather than silently ignored.
- **C7.2** units **removed** from the answer key must leave scope. Today
  `state.unit_states` is never pruned, so a deleted unit blocks
  `all_closed` (`:2167-2171`) forever. Prune `unit_states` **and** the
  per-unit side maps (`attempts`, `pending_attachments`, `timeout_retries`,
  `reminders`). Do **not** prune an in-flight unit's entry without cancelling
  it — the existing edit-diff at `:1074-1082` already cancels an in-flight
  unit whose projection vanished; keep those consistent.
- **C7.3** units **newly added** mid-run must seed `Open`. They are currently
  admitted to the plan (the filter matches `None`) but invisible to
  `all_closed`, so a run whose only remaining work is a newly added unit can
  report `settled`. `seed_unit_states` is called only from `start_run`
  (`:684`) and `resume_run` (`:750`).

**Teeth note:** tooth T3 (`teeth.md`) breaks `reread_answer_key`. If you
change its shape, say so in your report — the orchestrator re-runs and
re-records teeth at closure.

### 4. C6.4 — record L4 honestly (record, do not compose)

Independent spec/ruler traceability confirmed **SPEC_ONLY_NO_SHIPPED_SURFACE**
on a declared bounded search: §11.3 (`Core-spec.md:835`) states the
non-separability *criterion* but names no marker; the frozen ruler's field set
is closed (`{claim, verify_by, blast_radius, authority, evidence_required,
depends_on}`), the parser's field set is closed and typed-refusing
(`reference/parse.rs:40-49,103-107`), and `domain::Unit` carries no cohort. So
`PlanInput.cohorts` is structurally unfeedable on every shipped path.

Deliverable: a module-doc paragraph at **both** `BTreeMap::new()` sites
(`controller.rs:569` and `:973`) recording that cohort slicing is unreachable
because its input is structurally unavailable, citing §11.3 and Task 20's E2E
reference fixture as where it gets exercised. §11.6 ("Provisional capability
is never reported as complete") is why this record is mandatory.
**Do not** invent a cohort source, and **do not** touch `.wayfinder/**` or the
parser's frozen field set.

### 5. C6.3 — cohort charge/prompt consistency

`controller.rs:1124-1128` keeps only `planned.units.first()` while
`:1129-1132` reserves `planned.estimated_slice`, which `scheduler.rs:606-618`
summed over **all** members; `:1149-1156` prompts one unit. Latent only
because cohorts are always empty.

The adjudication's rule is "charge what you prompt; prompt what you charged."
Given item 4's finding that cohorts cannot form on any shipped path, the
honest minimum is to make the inconsistency **unrepresentable or loudly
refused** rather than silently latent — e.g. a typed refusal if a planned
slice carries more than one member while the controller cannot dispatch a
slice. Choose and justify; do not leave a silent drop.

### 6. N6 — remove the dead `compose_with_policy`

`controller.rs:3657-3676`, a `#[cfg(test)]` helper with zero references
repo-wide (superseded — the two tests needing a custom policy call
`compose_full` directly at `:4046` and `:4109`). Delete it. `just lint` must
go green.

## Gates (run personally, one at a time, record the outputs verbatim)

```
just check-package gauntlet-app
cargo nextest run -p gauntlet-app control::
cargo nextest run --test p4_corrective_loop --test p6_budget_control
just lint
```

Leave the full workspace gates to slice 3 / the orchestrator. **You hold the
single-compiler build lock: never run two cargo/nextest commands
concurrently.**

## Hard rules

Never edit `.wayfinder/**`, `.agents/specs/**`, `.agents/plans/**`,
`AGENTS.md`, `justfile`, `deny.toml`, `.agents/CURRENT_STATUS.md`, the frozen
prereg files, `.agents/prompts/**`, `.tmp/**`, `.jolli/**`. **No commits.**
Named-const literal idiom; typed errors carrying the failure locus; no
weakened assertions.

If a finding cannot be fixed without weakening something, **STOP** and record
the conflict in the checkpoints and your report instead of weakening it.

If implementation exposes an apparent defect in the frozen reference, do not
modify or work around the ruler — record an evidenced entry in
`.agents/reports/reference-candidates.md` and name the affected Units/tasks.

Do not fix items outside this brief. Specifically leave alone: critic-A m7
(tier posture, Task 14), the answer-key-read-failure error policy (newly
deferred — see `residual-readjudication.md`), and everything assigned to
slices F2/F3.
