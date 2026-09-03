# Task 21 — falsifier round 3 adjudication (orchestrator)

Falsifier: blind executing, fresh instance, focused. Verdict: **CONFIRM
(0 blocking / 0 major / 3 minor / 0 new informational)**. All three
COR-2 fixes HELD under executed re-attack (log/non-log boundary intact
both directions; 405/404 semantics byte-identical; backoff rate bound
real: 19.9 lines/s / 1.6 KB/s / 0.2% CPU vs ~1M lines/s / 37 MB/s /
100% pre-fix, with executed EMFILE recovery and 0.01 s SIGINT
shutdown). The mandatory no-bypass regression leg HELD end-to-end
(byte-identical CLI/API refusal, exactly-once earned settlement,
idempotent replay).

## Decision: COR-3 — one class-closing sweep (FINAL correction round)

Three rounds have now found the same defect class at a new locus each
time (r1: HTTP target → r2: ticket-id channel ×2 → r3: accepted arm,
dispatcher lines, list output ×3). Locus-by-locus correction does not
converge; the class needs one systematic pass with an enumeration-free
teeth. Tipping evidence for fixing rather than trigger-recording:
R3-F-3's operator-field vector is argv-reachable through the SANCTIONED
resolve path (no filesystem planting) and writes the payload into the
DURABLE ticket record, forging §20.3-shaped lines in the operator's
inbox view; R3-F-2's forged lines are newline-capable and §20.3-shaped,
sitting beside the very arm COR-2 fixed. The audit/view surface is this
task's product; closing the class is in-scope.

Scope boundary (hard): the sweep is confined to the escalations /
operator-action command family (`escalations_cmd.rs`,
`operator_action.rs` + their tests). The same raw-argument pattern in
OTHER subcommands' usage errors is a CLI-wide style question — recorded
for the next CLI-surface touch, NOT this slice.

| Finding | Sev | Decision |
|---|---|---|
| R3-F-1 accepted arm + CLI acceptance line interpolate ticket id raw (hand-planted record; ESC only) | MINOR | CORRECT via the sweep |
| R3-F-2 dispatcher unknown-subcommand + list unknown-argument lines (newline-capable forged §20.3-shaped lines) | MINOR | CORRECT via the sweep |
| R3-F-3 list output forges 3 ways (quarantined stems raw; operator field argv-reachable into durable record; raw ticket ids) | MINOR | CORRECT via the sweep |

## The converging contract (COR-3 brief)

One shared escaping discipline applied to every dynamic untrusted
string in the family's stderr/stdout output paths; teeth = an
INVARIANT test that plants hostile records through the queue's own
APIs + argv (control bytes and newlines in ticket ids, operator
identities, quarantined stems), drives every command and all three
action arms, and asserts: (1) zero raw bytes < 0x20 except \n and \t
anywhere in captured stderr/stdout; (2) the count of stderr lines
beginning with `operator-action:` equals the number of actions driven
(forged lines fail the count). Enumeration-free — a missed locus fails
the invariant, so the sweep cannot silently stop short.

## Proportionality / finality

COR-3 is the LAST correction round for task21. Verification is a short
focused r4: re-execute r3's three attack families + confirm the
invariant's teeth. If r4 CONFIRMs, settlement proceeds with remaining
informationals recorded. If r4 finds this class again, the invariant
test is defective — that is fixed before settlement, not rounded
again. Any NEW class found at r4 is trigger-recorded unless
blocking/major.
