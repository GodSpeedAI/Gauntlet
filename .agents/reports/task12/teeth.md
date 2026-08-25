# Task 12 — Teeth (B4 slice; proofs p4/p6 + controller + observer)

One entry per tooth. First recorded by the B4 finisher on 2026-08-24;
**re-run personally by the orchestrator on 2026-08-25 at closure** after the
F1/F2/F3 correction round moved or deleted several break sites — the
re-run below is the evidence of record. Discipline per the brief: break
the mechanism → run the named test → observe the FAILURE → revert
byte-identical (snapshot outside the repo + `cmp` clean) → re-run green.
No tracked file was left with a tooth break (`cmp` verified after every
revert; controller.rs and the proof files are untracked/new in this tree,
so `cmp` against the pre-break snapshot is the byte-identity proof).

Correction-round changes that moved teeth: **C2 deleted
`RoundsSupplementedMeter`** (T1/T4's original break class no longer
exists — both re-expressed below against the post-C2 mechanisms); **C5
strengthened** p4's selection-record assertion (T5 now fails `right: 2`,
both records); **C4 grew the p4 target to 4 tests** (T3's old "3/3"
revert count); **N2 grew the observe-command package to 10 tests**.

## T1 — the run budget's rounds authority (post-C2 re-expression)

- **Original (B4):** `RoundsSupplementedMeter::reserve`'s rounds gate
  deleted + refused booking silently skipped — the class C2 deleted.
- **Re-expressed break (2026-08-25):** `controller.rs`, BOTH arms of the
  single rounds authority C2 built — `rounds_exhausted` neutered to
  `false` (the predicate behind both rounds gates) AND
  `book_iteration_round`'s refused booking silently skipped (the
  boundary charge's error no longer sets the `budget_exhausted`
  terminal).
- **Test:** `cargo nextest run --test p6_budget_control` — main FAIL:
  `the terminus is exhaustion, not diagnosis or escalation (escalation
  summary: Some("AK-014 exhausted its attempt ceiling at 7 (ticket
  ticket-p6)"), iterations: 7) — left: Escalated, right:
  BudgetExhausted`. With both rounds arms neutered the run escapes the
  rounds ceiling entirely and only the §11.5 attempt ceiling stops it —
  defense in depth in the new wiring is the attempt/escalation ceiling,
  and p6's terminus assertion catches the escape. (No degenerate spin:
  the attempt ceiling terminates the run.)
- **Revert:** snapshot `cmp` byte-identical; p6 re-run 2/2 PASS.

## T2 — the iteration-boundary `verify_unchanged` call

- **Break (unchanged from B4):** `controller.rs` `boundary_verify` —
  early `Ok(None)`; the boundary re-verify certifies nothing.
- **Test:** `cargo nextest run --test p4_corrective_loop
  a_mid_run_answer_key_edit` — FAIL: `left: Escalated, right: Aborted`
  (same observable as B4 recorded). Without the boundary re-verify the
  mid-run edit never aborts with both digests; the run rides into the
  stall→retry→escalate path instead.
- **Revert:** snapshot `cmp` byte-identical; p4 re-run 4/4 PASS.

## T3 — the per-tick answer-key re-read (cache once)

- **Break (unchanged from B4):** `controller.rs` `reread_answer_key` —
  early return of the parse before the projection diff and the in-flight
  cancellation (the re-read's enforcement effect deleted; the first
  boundary's projections are reused forever).
- **Test:** `cargo nextest run --test p4_corrective_loop
  a_mid_run_answer_key_edit` — FAIL: "the re-read cancelled the edited
  in-flight attempt: left: 0, right: 1" (same observable as B4
  recorded). The boundary abort (T2's mechanism) still fires at the next
  boundary, but the §11.4 step-3 cancellation of the IN-FLIGHT attempt
  is gone — which is exactly what this test pins.
- **Revert:** snapshot `cmp` byte-identical; full p4 target re-run 4/4
  PASS (4 tests since C4, not the 3 the B4 record cited).
- Note (B4, still true): a degenerate break SHAPE (returning NO units)
  hangs the drive loop — no units, no in-flight, no terminal, no exit.
  That non-termination is an artifact of the break shape; C9.4 has since
  bounded the drain tick, but the "nothing open and nothing in flight
  and no terminal" drive-loop exit remains the residual B4 recorded.

## T4 — the reservation requirement on the dispatch path (post-C2/C9.1 re-expression)

- **Original (B4):** `RoundsSupplementedMeter::reserve` no-op
  pass-through returning `Reservation::new(slice)` — the class C2
  deleted; C9.1 additionally removed public `Reservation::new`.
- **Re-expressed break (2026-08-25):** `controller.rs` `dispatch` — the
  `state.meter.reserve(planned.estimated_slice)` call replaced by
  `state.meter.mint(planned.estimated_slice)` behind `Ok(...)`: the
  minted reservation carries no §5.3.7 gate, no scope check, no
  outstanding-slot increment, and no admission count — a dispatch with
  an unaccounted reservation.
- **Test:** `cargo nextest run --test p6_budget_control` — main FAIL:
  `reservations admitted == the forward rounds ceiling — left: 0,
  right: 5`. AK-052's rows-vs-reservations truth lives on the meter
  itself (`admitted_reservations_total`), and the bypassed dispatch
  books rows with zero counted reservations.
- **Revert:** snapshot `cmp` byte-identical; p6 re-run 2/2 PASS.

## T5 — the loop's `record_correction_selection` call

- **Break (unchanged from B4):** `controller.rs` `record_selection` —
  the durable store write deleted; the selection is computed and
  dropped.
- **Test:** `cargo nextest run --test p4_corrective_loop
  an_injected_defect` — FAIL at the read-back: `the correcting iteration
  and the closing iteration each recorded — left: 0, right: 2`. (B4
  observed `right: 1`; C5 strengthened the assertion to require BOTH
  records — the correcting and the closing iteration each record a
  §15.1 selection. The tooth discriminates the same mechanism.)
- **Revert:** snapshot `cmp` byte-identical; re-run PASS.

## T6 — observer deadline enforcement (already-expired refusal)

- **Break (unchanged from B4):**
  `crates/adapters/gauntlet-adapter-observe-command/src/lib.rs`
  `run_bounded` — the expired-deadline refusal removed; the instrument
  spawns regardless.
- **Test:** `cargo nextest run -p gauntlet-adapter-observe-command` —
  FAIL: `tests::an_expired_deadline_refuses_without_spawning` — the
  sentinel proof shows the instrument RAN (the observer proceeded to
  store facet evidence instead of refusing: "storing the facet evidence
  document failed: … the kind-binding test never observes"). Same
  observable class as B4 recorded.
- **Revert:** snapshot `cmp` byte-identical; package re-run 10/10 PASS
  (10 tests since N2 added the spawn-classification test, not the 9 the
  B4 record cited).

All six teeth bite on the post-correction tree; all six reverts are
byte-identical (`cmp` clean against out-of-repo snapshots); every named
test is green again after its revert. Full-suite confirmation after the
teeth re-run: p4 4/4, p6 2/2, observe-command 10/10.

## T7 — the meter consult on the critic-fan-out leg (correction F4; run by the orchestrator 2026-08-25)

- **Context:** correction F4 (confirmation round 1's MAJOR) wired the run
  `BudgetMeter` into review dispatch — `schedule_model` reserves
  `ReviewDeps::critic_slice` beside the count lease, and a Reserve refusal
  is the §10.4 truncation (`ModelAdmission::MeterRefused`, the dimension
  recorded `unavailable_evidence` with the typed cause, never a run
  error).
- **Break:** `compare/review.rs` `schedule_model` — the
  `meter.reserve(critic_slice)` consult replaced by
  `meter.mint(critic_slice)` behind `Ok(...)`: the critic admission runs
  with an unaccounted reservation (the T4 break class, on the critic leg —
  no gate, no slot, no refusal).
- **Test:** `cargo nextest run -p gauntlet-app control::controller::tests::a_meter_refused_critic_dispatch_is_truncated_and_the_run_continues`
  — FAIL: `the critic dispatch is refused at admission — the model
  comparator never ran — left: 1, right: 0` (the refusing meter no longer
  refuses; the critic dispatched). The review-level record leg is
  separately pinned by
  `compare::review::tests::a_meter_refusal_truncates_the_dimension_with_the_typed_cause`,
  and the slot-release leg by
  `control::controller::tests::a_critic_reservation_releases_at_fan_in_so_a_later_iteration_still_dispatches`.
- **Revert:** snapshot `cmp` byte-identical; re-run green — control::
  75/75, compare:: 36/36.

All seven teeth bite on the post-F4 tree; all reverts byte-identical;
every named test green after its revert.
