# Task 12 — correction FINISHER F2: checkpoint fidelity + small closes

You are a correction finisher in `/home/sprime01/projects/gauntlet`.
**You are slice 2 of 3.** Slice F1 (C5 verify/record, C6, C7, N6) has already
landed — read its result contract before you start, because it moved code you
are about to move.

## Read first (in this order)

1. `AGENTS.md`
2. `.agents/reports/task12/correction-adjudication.md` — authoritative C1–C9
3. `.agents/reports/task12/residual-readjudication.md` — the orchestrator's
   mid-round re-audit; N3/N4/N5 below come from it
4. `.agents/reports/task12/correction-f1.md` **and** the F1 entries in
   `correction-checkpoints.md` — F1 edited `plan_and_dispatch`,
   `reread_answer_key`, `PlanInput`, and the per-unit side maps. **Re-derive
   every line number in this brief against the current tree.**
5. `.agents/reports/task12/critic-b-result.md` §A9, §A11, §A12

You are NOT blind.

## Reconciliation facts (do not re-derive, do not re-implement)

- C1, C2, C3, C4 are DONE. **C5 landed unrecorded** and was verified+recorded
  by F1. **C9.1** (`Reservation` mint gating) landed early under C2 —
  `budget_meter.rs` has no `Reservation::new`; construction is only through
  `BudgetMeter::mint`. Do not redo it.
- C6, C7, N6 are F1's; do not revisit them.

## Method (mandatory)

- **Fail-first per item**: write/adjust the discriminating test, RUN it, paste
  the observed failure verbatim into the checkpoint, then fix, then re-run.
- **DECLARE-FIRST checkpointing** → `.agents/reports/task12/correction-checkpoints.md`.
  Before you edit anything for an item, append a one-line
  `## <item> — STARTED <timestamp>` marker with your intended design. Append
  the full `## <item> — DONE <timestamp>` entry after. Sessions die here — four
  times in this task — and the single most expensive failure in its history was
  an item that landed with no checkpoint entry, so the handoff said "outstanding"
  when the code was already in the tree and correct. Write-after-completion
  leaves a window one item wide in which silence is ambiguous; declare-first
  makes a `STARTED` with no `DONE` mean exactly one thing.
  See `.agents/lessons/unrecorded-landed-work.md`.
- **Before you implement anything**, run
  `./scripts/reconcile-checkpoints.sh 12` and read what it flags. Any file it
  lists was modified after the last checkpoint write, so the record cannot be
  describing it — check whether your item is already partly done before you
  start. Never re-implement an item without checking that list first.
- Result contract: `.agents/reports/task12/correction-f2.md`.
- Bounded claims only: "not found within <declared scope>", never universal
  absence or minimality.

## Items

### 0. ITEM 0 — bound dispatch-eligible membership by the FROZEN REFERENCE (the real C7 remainder)

**Ruling context (orchestrator, `residual-readjudication.md` "C7 CONFLICT"
section, post-F1; supersedes the original C7.2/C7.3):** C7.2/C7.3 as
originally specified were MIS-SPECIFIED and must NOT be implemented as
written. The answer key cannot express a unit status on any shipped surface
(C7.1 closed vacuous-by-surface, pinned by a test). Pruning/seeding
`unit_states` to track file edits would make the controller ADAPT to a
mutated ruler instead of ABORTING on it — the inverse of freeze/digest/
re-verify — and the iteration-boundary abort (tooth T2) is where a mutated
answer key is already correctly handled.

The REAL residual is F1's blocker 2: a unit present in the FILE but absent
from the FROZEN REFERENCE is dispatched during the window before the next
boundary abort, then hard-errors at verification with `UnitUnavailable` —
no terminal, no close-out checkpoint, no report (the C1-family
terminal-truth defect). That is what "C7 remainder" means now.

**Fix:** one guard — dispatch-eligible membership is bounded by the frozen
reference, not the file. A file-only unit is never dispatched; the mutation
is then caught where it already is, by the existing iteration-boundary
abort (tooth T2). Fail-first under the §21.3 unavailability window (the
same window F1's C7.3 observation used). No pruning, no seeding, no §21.3
policy change, nothing weakened. The frozen unit set is reachable through
the verification authority's frozen projection (`verify/authority.rs`
`frozen_unit`, which parses the freeze snapshot) — locate the shipped
surface that exposes the frozen membership and consult it at the
plan/eligibility boundary; record in your checkpoint which surface you used
and why. If no shipped surface exposes the frozen membership set to the
controller, STOP and record that as a governed finding (do not widen a port
or a frozen surface to get it).

### 1. C8.1 — real `event_log_offset` (no constant 0)

`state.last_offset` starts at `0` in `start_run`; the checkpoint and the
`CheckpointWritten` event both write that stale value, and the freshly
appended event's returned offset is explicitly discarded (`let _ = offset;`).
§18.2 lists the event-log offset as a required field and §18.3 step 1 makes it
the **anchor replay starts from** — a constant 0 is not cosmetic.

**This is a MULTI-SITE fix.** `append_terminal_event` already *returns* the
store's `EventLogOffset`, but four of its five callers discard it (hand-edit
cancellation, drain cancellation, timeout disposition, failure disposition);
only the resume caller assigns it. `StateStore::append_event -> Result<EventLogOffset, _>`
is the real source, and the in-crate `FakeState` mints monotonic offsets.
Thread the returned offset at **every** controller append site, not just the
checkpoint's own — otherwise the water-mark still lags by every terminal event
of the iteration.

### 2. N3 — the checkpoint offset READ path must be strict

Composed by `residual-readjudication.md` from the Task-6 R3 ledger item
(trigger "next state-sqlite touch" — fired: migration `0008` + ~1085 lines).

`gauntlet-adapter-state-sqlite/src/state_store.rs` decodes the checkpoint's
offset as `EventLogOffset::new(u64::try_from(offset).unwrap_or(0))`. Once C8.1
makes the offset load-bearing for resume, a tampered or negative stored offset
silently reading `0` re-introduces the exact constant-zero defect through the
read path. Make the **checkpoint** decode strict — a non-representable stored
offset is `Corrupt`, not `0` — with a fail-first test.

**Bounded to the checkpoint leg only.** Do NOT touch the settlement,
evidence-production, or event-log coercions or `settled_at`: those are
Task 13's surface and are recorded as later-scope in
`residual-readjudication.md`.

### 3. C8.2 — checkpoint at the iteration BOUNDARY

Today the checkpoint is written **mid-pipeline** — after the meter/budget
charge, but *before* observe/compare/verify/settle. §11.1's iterate order puts
it last ("… dispose residuals → checkpoint"). A crash after settle but before
the next completion's checkpoint loses the settlement, and §18.3 says
"Re-running settled work is a defect, not a safe default."

**Do not assume C2 already moved this — it did not.** C2 moved the *round
booking* to the boundary; the checkpoint call is still mid-pipeline.

**CRITICAL HAZARD — read before moving it.** The mid-pipeline checkpoint is
currently the **producer-authority anchor**. `gauntlet-ports/src/producer_authority.rs`
refuses any owner write whose declared iteration ≠ the latest durable
checkpoint's iteration (defaulting to 1 when no checkpoint exists), and the
controller's evidence-production owner write happens *later in the same
iteration* declaring `state.iteration`. Deleting the mid-pipeline call without
writing a checkpoint that **declares iteration k before iteration k's owner
writes** makes every iteration ≥ 2 fail with a producer-authority refusal.

The cancelled builder's recorded design resolves this: write the boundary
checkpoint at the **top** of iteration k (which declares k, and is
simultaneously "post settle-in-memory of iteration k−1"). Evaluate that
against §11.1/§18.2 and adopt it or a better-grounded one; justify any
deviation.

**PROOF ARITHMETIC THAT MUST SURVIVE:** `p6_budget_control.rs` asserts
`checkpoints == FORWARD_ROUNDS_CEILING + 1` (exact equality: 5 boundary + 1
close-out) and `p4_corrective_loop.rs` asserts `>= 2`. A top-of-iteration
boundary checkpoint reproduces that count; a per-completion-**plus**-boundary
scheme doubles it and fails p6. **Do not change the assertion to fit the
implementation** — if the count genuinely must change, that is a STOP-and-
record condition, not an edit to the proof.

### 4. C8.3 — resume restores per-unit attempts from durable state

`resume_run` re-initialises `attempts`, `pending_attachments`,
`timeout_retries`, and `reminders` to empty and never repopulates them, so a
crash-retry loop re-arms the retry-once, one-reminder, and max-attempts
ceilings on every crash (critic-B A11).

**Scope: the `attempts` leg ONLY.** The adjudication explicitly parks
retry-ledger *persistence* (`timeout_retries`, `reminders`,
`pending_attachments`) in L2 because the §18.2 field set is spec-fixed and
adding a checkpoint field is a ports revision. The recorded design
reconstructs `attempts` from the loop's own durable producer writes
(`evidence_productions_at`) rather than from a new field. Verify that source
actually supports the reconstruction before relying on it; if it does not, say
so and record L2 as covering the whole leg rather than inventing a field.

**Report back one conditional:** the ledger item "checkpoint-only run
projection indistinguishable" is later-scope *unless* C8.3's resume needs to
distinguish a checkpoint-only projection from a full one. Check, and say
which.

### 5. C9.2 — escape escalation-evidence JSON

`submit_ticket` interpolates the summary raw into a JSON evidence record; a
summary containing `"` produces a malformed forensic record. The asymmetry
critic-B named still holds — the reference-unavailable record and the
quarantine reason both escape; this path does not.

**Note C1 raised the stakes:** C1 extracted the old `escalate()` body into
`submit_ticket`, so this single unescaped `format!` is now the funnel for
**all three** escalation terminals — `escalated`, C1's new `blocked` ticket,
and `inconclusive`. Prefer one shared escaping helper over a third ad-hoc
`replace` (three copies of an escaping rule is the next defect).

### 6. C9.3 — no silent result drop on charge refusal

On a charge refusal the collected result is dropped with nothing pinned and no
disposition recorded: the code sets `terminal = BudgetExhausted` and returns
*before* the checkpoint, before observe, before settle.

**There are now TWO drop sites, not the one critic-B described** — C2 created
the second (the loop-budget charge, whose comment cites C2). Both need the
record + typed disposition.

**Authoritative grounding:** §21.2 "never a swallowed error"; §10.4 "not
silently dropped"; §12.3 gaps are first-class REQUIRED arrays — and note §12.3's
forensics precedent: an artifact can be **stored for forensics while excluded
from verification inputs**, which is the right shape for a result that arrived
but whose charge was refused (keep it, record it, let it settle nothing).

**Additional defect in the same window, not in the critic's text:** on the
meter-charge refusal path the reservation is never released — the memory meter
decrements `outstanding` only after a *successful* charge, and
`release_reservation` is not called on either path, so the parallel slot
leaks. Mostly moot because the run is terminating, but fix it.

### 7. C9.4 — bounded drain (max polls per tick)

The drain's `while` has only two exits (a terminal, or an empty in-flight
list): no counter, no wall-left check, no early break. A runner that never
completes and never trips stall detection spins the tick loop indefinitely.

**BLAST-RADIUS WARNING — diagnose, do not copy.** The obvious fix (a new
`max_polls_per_tick` on `ControlPolicy`) is **not** controller-local: it
touches `ControlPolicy::new`'s validation, `from_settings`' typed
`RequiredSettingMissing` mapping, and the Annex E config surface — and
critic-A m2 already flags that four Annex E control settings are REQUIRED
contrary to §8.3, so adding a fifth REQUIRED setting compounds a known open
minor. **Prefer a derived bound** from the two settings the drain already
reads and already requires (`stall_timeout_ms`, `poll_interval_ms`). The
recorded design for past-the-bound behaviour is: cancel in-flight, pin the
fact as evidence, escalate.

### 8. N4 — evidence-locator containment at the controller write path

From `residual-readjudication.md` (critic-B A12's unrouted INCONCLUSIVE,
independently confirmed): `validate_plain_ref` in
`gauntlet-domain/src/values.rs` rejects a leading `/` but **permits `..`**, and
the controller's verification path writes bytes with `create_dir_all` +
`fs::write` at a locator joined to the workspace root. A `../…` locator
escapes the workspace. Task 12 is the **first production writer** at that path.

Fix at the **controller's containment boundary**: refuse, with a typed error
naming the locus, any locator that does not resolve inside the workspace root.
Fail-first with a `..`-bearing locator.

**Do NOT widen `validate_plain_ref`** — that is a Task-3-closed public value
grammar shared by every locator-like newtype; changing it is a domain/ports
revision, not a controller fix. If you conclude the domain is the only correct
home, STOP and record that as a governed finding instead of doing it.

### 9. N5 — the durable §15.1 record must report FORWARD headroom

`remaining_of` in `gauntlet-ports/src/budget_meter.rs` reports raw-limit
headroom (`limits.max_tokens() - spent.tokens()`), and `select.rs` copies it
straight into the durable §15.1 `BudgetFacts.remaining_*`. The refusal
arithmetic is correct (the gate is `would_breach_reserve`) — but **every C5
selection record durably overstates headroom by the reserve**.

AK-037's claim is about the *record*; §15.1's affordability dimension is what
the record purports to explain. A record that reports reserve-inclusive
headroom as available does not explain the decision that was actually made.

Diagnose where the fix belongs — `remaining_of`'s own semantics, or the
record-construction site in `select.rs` — against §15.1 and §17.4, and say
why. If `remaining_of`'s raw-limit meaning is load-bearing for another caller,
the record site is the right place; check before choosing. Note that
`budget_meter.rs` is a **tracked** file with a shared contract suite — if you
change port semantics, the contract suite must move with it.

## Gates (run personally, one at a time, record outputs verbatim)

```
just check-package gauntlet-app
cargo nextest run -p gauntlet-app control::
cargo nextest run --test p4_corrective_loop --test p6_budget_control
cargo nextest run -p gauntlet-ports
cargo nextest run -p gauntlet-adapter-state-sqlite
just lint
```

**You hold the single-compiler build lock: never run two cargo/nextest
commands concurrently.** Memory-constrained WSL — do not raise build or test
parallelism. Leave the workspace-wide gates to slice F3.

## Hard rules

Never edit `.wayfinder/**`, `.agents/specs/**`, `.agents/plans/**`,
`AGENTS.md`, `justfile`, `deny.toml`, `.agents/CURRENT_STATUS.md`, the frozen
prereg files, `.agents/prompts/**`, `.tmp/**`, `.jolli/**`. **No commits.**
Typed errors carrying the failure locus; named-const literal idiom; no
weakened assertions and no proof-assertion edits to accommodate an
implementation.

If a finding cannot be fixed without weakening something, **STOP** and record
the conflict instead.

If implementation exposes an apparent defect in the frozen reference, do not
modify or work around the ruler — record an evidenced entry in
`.agents/reports/reference-candidates.md` and name the affected Units/tasks.

Leave alone: critic-A m7 (tier posture — Task 14), critic-A m5 (selector fold
loser notes), the answer-key-read-failure error policy (newly deferred), and
everything in slices F1/F3. Free cleanup if you are already in the file:
critic-A m4's vacuous assertion in `p4_corrective_loop.rs`
(`!format!("{:?}", …).is_empty()`, never false) — it sits in the direct-selector
test, not in the composed C5 assertions.

**Teeth:** T1, T3, T4 and T5 in `teeth.md` break mechanisms this round has
moved (the deleted `RoundsSupplementedMeter`, `reread_answer_key`,
`record_selection`). If you move a teeth-bearing mechanism, say so in your
report — the orchestrator re-runs and re-records all teeth at closure.
