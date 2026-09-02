# Task 20 — falsifier round 5 adjudication (orchestrator)

Verdict: NOT_CONFIRM (0 blocking / 1 major / 1 minor). Every COR-R4 fix
held under re-attack; over-rejection leg clean. Two corrections
commissioned; both are the settlement arm of the recovery/no-op seam
rounds 4-5 have been squeezing.

- **F-R5-1 (MAJOR) → COR-R5-1.** An all-settled durable projection closes
  as `terminal=settled` BEFORE (or regardless of) iteration booking: when
  every unit is closed-and-settled, the loop's close-out must fire even
  when the round booking is refused by budget — the booking refusal must
  not short-circuit the settled close. Reachability is honest (ordinary
  §16.3/A5 crash, no forgery). The falsifier's exact reproduction becomes
  a shipped regression leg (resume after settlement-arm damage ⇒ exit 0,
  terminal=settled, zero re-dispatch, export unchanged). The distinction
  from a genuinely unfinished run stays pinned (no false no-op).
- **F-R5-2 (MINOR) → COR-R5-2.** Symmetry: below-anchor settlement-row
  damage on a certified settled no-op gets the same honest forensic pin
  the production arm emits (per-row, naming the world/locator-class/
  missing row — checked against the store), so no damage class closes
  silently while its sibling is pinned. Over-rejection guard: a run with
  no settlement damage pins nothing new.

Constraints unchanged: p1-p12/g6/g7/g10 + shipped E2E legs green
UNMODIFIED (mechanical adaptations listed); over-rejection leg stays
clean. Full battery at the end; `just ruler` before/after.
