# Task 12 — activated-residual RE-adjudication (orchestrator, mid-correction)

`residual-adjudication.md` dispositioned every Task-12-**triggered** ledger
item *before implementation began*. Four builder slices and a partial
correction round later, Task 12 touches surfaces that adjudication could not
have known about, and several **surface-touch** triggers (as opposed to
task-name triggers) have since fired. Per the standing ledger rule
(`CURRENT_STATUS.md:743-745` — "a deferred finding upgrades to blocking the
moment a task walks into its trigger"), each is dispositioned here. Nothing
below is carried silently.

Source: a blind read-only trigger audit over the deferral ledger, the two
prior adjudications, and the current tree (`git status --porcelain` plus the
Task-12 file set). **Bounded claim:** the audit enumerated the ledger entries
present in `CURRENT_STATUS.md:530-747` and matched them against the tree's
modified/untracked paths. It is not a claim that no other residual exists.

## Surfaces Task 12 touches that the pre-implementation adjudication did not anticipate

| Surface | Evidence |
|---|---|
| `gauntlet-ports` public API (breaking) | `Reservation::new` removed + `BudgetMeter::mint` added (`budget_meter.rs:73-113`); new `producer_authority` module (925 ln); three new `StateStore` methods; two trait signatures re-typed to `Admitted*` |
| `gauntlet-app::config` | six new `ConfigKey` variants + Annex-E env names (`resolve.rs:91-111,195-206`); new `control` object in the **canonical/fingerprinted** form (`canonical.rs:232-268`) |
| `gauntlet-app::reference` | `provider.rs:84-96` — new `pub fn read_text`, `describe` made public |
| `gauntlet-adapter-observe-command` | `run_bounded`, `drain_capped`, `OUTPUT_CEILING_BYTES` (+305 ln) |
| `gauntlet-adapter-state-sqlite` | `state_store.rs` +1085 ln, `encode.rs`, `migrations.rs`, new migration `0008` |
| `gauntlet-domain` | new `correction.rs` (796 ln), `budget.rs` +345 (`Budget::restore`) |

## Disposition of every fired-but-undispositioned trigger

### COMPOSED into Task 12 (added to the correction round)

- **N1 — Doctor check 2's manifest clause stays shape-only**
  (T7-M2 / B-F11; ledger `CURRENT_STATUS.md:642-649`).
  *Trigger text:* "next doctor touch **or Task 12 (config/reference surface
  work)** — deepen the clause and promote the fixture to a full manifest."
  **FIRED.** The pre-implementation reason ("Task 12 … modifies no
  doctor/config/reference-parsing surface. Not activated by consumption") is
  **factually false against the current tree**: Task 12 widened the config
  canonical form (`canonical.rs:232-268`) and the reference provider
  (`provider.rs:84-96`). The trigger's own words name Task 12 doing exactly
  that work as a sufficient disjunct. Composed, bounded to: make the clause
  non-vacuous (`top_level_mapping_keys` returning `Ok(0)` for an all-indented
  file currently passes) and run the strict `parse_manifest`; promote the
  doctor scratch fixture to a manifest that deep-parses.
  *Governed stop condition:* if the **real** `.wayfinder/reference.yaml`
  fails deep parse, that is a reference finding — stop, record in
  `reference-candidates.md`, do not loosen the parser.

- **N2 — observe-command adapter lacks an in-crate spawn-classification test**
  (T9; ledger `CURRENT_STATUS.md:609-611`).
  *Trigger text:* "next observe-command touch." **FIRED** — the adapter
  gained `run_bounded`/`drain_capped`/output ceiling this task, and Task 12's
  own C4 proof *leans on* the classification ("the REAL command observer over
  an absent program"). The property is still discriminated only cross-crate.
  Composed: one in-crate test pinning `Unavailable` for a non-spawnable
  program, in the same `mod tests` that now holds the five new deadline/ceiling
  tests.

- **N3 — checkpoint `event_log_offset` read path coerces with `unwrap_or(0)`**
  (T6-R3 half; ledger `CURRENT_STATUS.md:712-715`, trigger "next state-sqlite
  touch"). **FIRED** — migration `0008` plus +1085 lines of `state_store.rs`.
  Composed **into C8**, and only the checkpoint leg: C8 makes the checkpoint's
  offset load-bearing for resume, so a tampered/negative stored offset
  silently reading `0` (`state_store.rs:1706`) re-introduces the exact
  constant-zero defect C8 exists to remove, through the read path instead of
  the write path. Strict decode → `Corrupt`.
  *Not composed:* `settled_at` caller-attestation and the settlement /
  evidence-production / event-log coercions — those are Task 13's settlement
  surface and are not load-bearing for any Task-12 claim. Recorded as
  later-scope below.

- **N4 — evidence-locator containment at the controller's write path**
  (critic-B A12, filed **INCONCLUSIVE** and never routed into a C-item).
  Independently confirmed: `validate_plain_ref`
  (`gauntlet-domain/src/values.rs:709-721`) rejects a leading `/` but
  **permits `..`**, and the controller writes verification bytes with
  `create_dir_all` + `fs::write` at a locator joined to the workspace root.
  A `../…` locator therefore escapes the workspace. Task 12 is the **first
  production writer** at that path, so this is Task-12 scope, not inherited.
  Composed: the controller's write path refuses a locator that does not
  resolve inside the workspace root, typed, fail-first.
  *Diagnosis note:* the fix belongs at the controller's containment boundary,
  **not** in the frozen-by-Task-3 domain newtype — widening
  `validate_plain_ref` would change a closed task's public value grammar for
  every locator kind, which is a ports/domain revision, not a controller fix.

- **N5 — `remaining_of` reports raw-limit headroom, not forward headroom**
  (critic-A m3, corroborated by critic-B A7; never adjudicated).
  `budget_meter.rs:136-154` computes `limits.max_tokens() - spent.tokens()`,
  and `select.rs:382-385` copies it straight into the durable §15.1
  `BudgetFacts.remaining_*`. The refusal arithmetic is correct (the gate is
  `would_breach_reserve`), but **every C5 selection record durably overstates
  headroom by the reserve**. Composed because AK-037's claim is about the
  *record*, and §15.1's affordability dimension is what the record purports to
  explain: a record that reports reserve-inclusive headroom as available is
  not an explanation of the decision that was actually made.

- **N6 — dead `compose_with_policy`** (`controller.rs:3657`): orphaned
  `#[cfg(test)]` helper, zero references repo-wide. Not a critic finding —
  it is a **live gate failure**: `just lint` is `clippy --all-targets -D
  warnings`, so the tree is currently lint-red. Composed.

### LATER-SCOPE — trigger fired, reason re-grounded against the current tree

- **T9-m10/D9 command-adapter kind↔command binding.** The written reason
  ("Task 12 neither constructs command observers from configuration") is
  falsified: `p6_budget_control.rs:700-726` and `p4_corrective_loop.rs:989-992`
  derive program+args from the fixture's parsed declared self-check while
  hand-choosing `FacetKind::Test` beside it — that *is* the misbinding shape.
  **Re-grounded reason:** the shape occurs only in *proof composition*, where
  the fixture author is also the binding author and the binding is the thing
  under test (`the_facet_kind_is_the_callers_binding` pins the convention
  deliberately). No production path constructs an observer from configuration;
  the plan assigns real-tool binding to Task 15 and the composition root to
  Task 20. Trigger stands at Task 15/20, now on a true premise.

- **Ports-revision cluster** (ladder rungs 5-9/11; locator contract-defined
  matching; facet `Optionality` / `FacetKind` vocabulary; checkpoint-only run
  projection). Task 12 *is* a breaking ports revision, so "trigger: ports
  revision" fired literally. **Reason:** each cluster member names a
  *different* port surface than the ones Task 12 revised — Task 12 revised
  `BudgetMeter`, `StateStore`, and added `producer_authority`; it did not
  touch the `Verifier` ladder rung set, `EvidenceLocator` matching semantics,
  or the `Observer` facet vocabulary (`FacetKind` verified unchanged at 8
  variants, `observer.rs:27-36`). The plan sequences Task 12 as controller
  composition; its port changes are consequences of that, not a deliberate
  port redesign, and re-opening four unrelated frozen surfaces mid-task
  widens scope against `AGENTS.md` ("Do not widen scope to fix adjacent
  issues. Record them separately").
  **Conditional:** the *checkpoint-only run projection* member becomes
  activated if C8's resume needs to distinguish a checkpoint-only projection
  from a full one. The C8 builder must check and report; if it does, it
  composes.

- **T6-R1/R3 remainder** — `settled_at` caller-attested; the non-checkpoint
  `unwrap_or(0)` coercions (settlements, evidence productions, event log).
  Trigger fired (state-sqlite touched). Not load-bearing for AK-037/038/039/
  051/052; the settlement-row half is Task 13's own surface, where P7 will
  exercise it. The checkpoint leg is composed as **N3** above.

- **T3 domain-crate source-scan guard-evasion minors.** `gauntlet-domain` was
  touched, but the named evasion routes live in the **arch/boundary scanners**
  (`crates/gauntlet-arch-tests/**`, `tests/contract/**`), which this task does
  not modify — verified by their absence from `git status --porcelain`.
  `CURRENT_STATUS.md:698-701` sets the standing rule that arch-suite residuals
  re-litigate at the next arch-suite touch; opening the frozen suite mid-task
  to close them would *be* that touch, unforced.

- **T7-B-F10/F12/F13 doctor/lock message nits.** Weakly fired (the CLI crate's
  `Cargo.toml` and its proof tests changed). The nits live in
  `crates/gauntlet-cli/src/**` and `crates/gauntlet-app/src/doctor/**`
  argument/message surfaces; no file under `gauntlet-cli/src/**` is modified.
  N1 touches `doctor/checks.rs`'s manifest clause only — it does not reach
  these three message loci.

- **T6-A-MINOR-4 replay reads the whole log from offset 0.** Borderline: C8
  makes offsets real and composes a partial resume. **This is the same
  obligation as `correction-adjudication.md` L2** (full §18.3 resume →
  Task 13 P7) and is cross-referenced to it here so the ledger entry and L2
  are visibly one item, not two independent deferrals.

### L4 — cohort non-separability: honestly spec-awaiting (CONFIRMED)

Independent spec/ruler traceability returned **SPEC_ONLY_NO_SHIPPED_SURFACE**
on a declared bounded search:

- The requirement exists — Core-spec §11.3 (`Core-spec.md:835`) and §15.1
  step 3 (`:995`) state the *criterion* ("not meaningfully separable") but
  name no marker, field, or key.
- No shipped surface can express it. The frozen ruler's complete field set is
  `{claim, verify_by, blast_radius, authority, evidence_required, depends_on}`
  (`depends_on` annotated "ordering only; not authority",
  `Core-spec.md:287`); `reference.yaml` carries no grouping key; the parser's
  field set is **closed and typed-refusing** (`reference/parse.rs:40-49,103-107`),
  so adding a `cohort:` key to the ruler today is a hard parse failure; and
  `domain::Unit`'s twelve fields carry no cohort.
- The consumer already exists and is fed empty: `PlanInput.cohorts` +
  `CohortId` are built and spec-cited (`scheduler.rs:98-131`), and both
  production call sites hardwire `BTreeMap::new()` (`controller.rs:569,973`).

*Bounded-claim discipline:* this is "not found within the four shipped
surfaces named, under the enumerated regex" — **not** universal absence.

**Disposition:** L4 stands as spec-awaiting, and C6.4's deliverable is the
**honest record**, not a composition — a module-doc paragraph at both
`BTreeMap::new()` sites stating that cohort slicing is unreachable because its
input is structurally unavailable on every shipped surface, and that Task 20's
E2E reference fixture is where it gets exercised. §11.6's "Provisional
capability is never reported as complete" makes that record mandatory.

### New deferrals opened by this audit (recorded, with triggers)

- **Answer-key READ failure inside the loop is a hard `ControlError::Reference`**
  (`controller.rs:1069` reached from the plan and every drain tick),
  inconsistent with `boundary_verify`'s tolerant Unavailable policy
  (`:864-885`, which pins a gap and continues). Raised by critic-B A4 as
  "adjacent"; routed into no C-item. §21.3 puts "Config/reference invalid" at
  "stop new dispatch; keep the process alive", which suggests the tolerant
  side — but changing it mid-correction is an unforced policy change to a
  surface no critic finding requires moving. **Trigger: Task 13 recovery
  composition, or the next §21.3 degradation-policy touch.**
- **Critic-A m4 vacuous assertion** at `p4_corrective_loop.rs:1604`
  (`!format!("{:?}", …).is_empty()` — never false). It sits in the direct-
  selector test, not in the composed C5 assertions, so it weakens no
  Task-12 claim. Free cleanup if a builder is already in the file.

## BLOCKERS

None.

## Consequent change to the correction work list

`correction-brief.md`'s C1–C9 is superseded by the following, after
reconciling the cancelled builder's *unrecorded* progress:

| Item | State |
|---|---|
| C1, C2, C3, C4 | DONE, checkpointed |
| **C5 (all five sub-parts)** | **LANDED IN TREE BUT UNRECORDED** — the builder worked past its last checkpoint entry (file mtimes 09:57–10:11 vs the 10:20 stop) and was killed before writing it. Must be *verified and recorded*, never re-implemented. |
| C9.1 (`Reservation` mint gating) | DONE — landed early as a C2 prerequisite |
| C6.1–C6.4, C7.1–C7.3, C8.1–C8.3, C9.2–C9.4 | OUTSTANDING |
| N1–N6 | OUTSTANDING (composed by this document) |

---

## C7 CONFLICT — orchestrator adjudication (2026-08-25, post-F1)

F1 stopped on C7.2/C7.3 per the hard rule rather than weakening an assertion.
Correct call. Its three load-bearing claims were **re-verified personally**:

- `reference/parse.rs::field_keys()` = `{claim, verify_by, blast_radius,
  authority, evidence_required, depends_on}` — **no `status` key**;
- `domain/unit.rs:351` hardwires every parsed unit to `status: Open`;
- `verify/authority.rs::frozen_unit` refuses any unit absent from the frozen
  reference: `UnitUnavailable { detail: "no block with that id exists in the
  frozen reference (digest …)" }`.

### Ruling: C7.2/C7.3 were MIS-SPECIFIED. Do not implement them as written.

The adjudication derived "removed units leave scope; new units seed Open"
from critic-B A4 without checking two facts that decide it:

1. **The answer key cannot express a unit status on any shipped surface.**
   So A4's stated attack — "an operator hand-edit marking a queued unit
   `settled`/`out_of_scope` is silently ignored" — is **unconstructible**.
   C7.1 is therefore vacuous-by-surface, not unimplemented. F1 recorded it and
   pinned it with a test. CLOSED.
2. **The run's settleable membership is the FROZEN REFERENCE's, not the
   file's.** Adding or removing an AK block changes the file digest, so the
   iteration-boundary `verify_unchanged` (tooth T2) already aborts the run
   `reference_mutated` — §21.3's *only* abort row, "Integrity violation →
   Abort the run; preserve everything."

Pruning and seeding would have the controller **adapt to a mutated ruler
instead of aborting on it** — the inverse of the standing structural rule
("the builder never modifies the standard its work is evaluated against";
freeze, digest, re-verify). F1's two failures are that rule biting correctly,
not obstacles to route around: pruning lets `all_closed` claim `settled` for
frozen-ruler work never done; seeding admits work verification then refuses.

### What IS a real defect (the residue worth fixing)

F1's blocker 2. `plan_and_dispatch`'s filter matches `None`, so a unit present
in the **file** but absent from the **frozen reference** is dispatched during
the window before the next boundary abort, then hard-errors at verification
with `UnitUnavailable` — **no terminal, no close-out checkpoint**. That is a
C1-family terminal-truth defect and it is Task-12 scope.

**Fix (reassigned to F2 as item 0):** bound dispatch-eligible membership by the
frozen reference — a file-only unit is never dispatched. The mutation is then
handled where it already is, by the boundary abort. One guard; no pruning, no
seeding, no §21.3 policy change, nothing weakened.

*Note the shape:* this is the second time this round that a critic finding,
adopted verbatim into an adjudication, encoded an attack the shipped surface
cannot express (cf. L4 cohorts). A finding names a hazard; whether the hazard
is *reachable* is a separate question that has to be asked against the tree.
