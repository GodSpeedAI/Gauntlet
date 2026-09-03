# Task 21 — falsifier round 4 adjudication (orchestrator)

Falsifier: blind executing, fresh instance, FINAL focused round.
Verdict: **CONFIRM (0 blocking / 0 major / 1 minor / 1 informational)**.

## Held (executed)

All three round-3 attack families re-executed against the COR-3 tree:
zero raw control bytes in the §20.3 and acceptance surfaces, escaped
forms present, plain lines uncorrupted, exactly one §20.3 line per
action, correct exit codes, no forged lines. The class-closing sweep
holds under its commissioning attack.

## R4-F-1 — MINOR — the invariant test's declaration vs the recorded boundary

The one surviving raw-control-byte sink is the RECORDED COR-2 boundary:
the human CLI failure detail line prints queue-error Display verbatim
(embedding grammar-valid hostile ticket ids raw). The behavior is
correct-by-decision and was verified boundary-intact in BOTH directions
by r3 and now re-executed by r4. The defect is in the TEETH, not the
code: the enumeration-free invariant accommodates the boundary by
AVOIDANCE (its drive set never takes a hostile-id read-failure shape),
so no shipped tooth observes the boundary, the carve-out is undeclared,
and a future widening (a newline-capable string entering an error
Display) would be invisible to both invariant clauses. Today unreachable
(every error-embedded id is grammar-checked whitespace-free) — hence
MINOR, teeth-declaration class.

## Decision: tests-only addendum, then settlement (no r5)

Fixed by the COR-3 builder as a tests-only addendum: the invariant's
drive set takes the three hostile-id read-failure shapes; the boundary
is asserted BY CONSTRUCTION (raw bytes only inside the single human
failure line's verbatim segment, nowhere else; §20.3 line-count
invariant unchanged); the carve-out is declared in the test's doc as
the tooth pinning the recorded decision. The behavior itself is
double-confirmed held by two independent executing rounds; per the r3
finality rule this closes the task — no fifth round. The addendum's
gates are re-run personally and the full workspace closure battery runs
on the final tree before the settlement commits.

## Informational

Other subcommands' usage-error interpolation: out of the family scope,
not attacked, recorded for the next CLI-surface touch (consistent with
the COR-3 scope boundary).
