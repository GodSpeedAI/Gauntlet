# Task 21 — falsifier round 2 adjudication (orchestrator)

Falsifier: blind executing, fresh instance, against the COR-1 tree.
Verdict: **CONFIRM (0 blocking / 0 major / 3 minor / 2 informational)**.
All ten COR-1 corrections held under executed attack; the round-1
regression sweep held in full; the over-rejection leg passed (genuine
earned-PASS settlements through BOTH surfaces, exactly once).

## Decision: one micro-correction slice (COR-2), then a focused r3

The three minors are the same defect classes this task already treats
as in-scope (log injection, accept-loop liveness), found by the round
commissioned to attack them, each with an executed repro and a cheap
behavior-preserving fix. Per the correction-selection rule (affordable,
governable, recoverable, settleable) they are fixed, not
trigger-recorded. The two informationals are recorded.

| Finding | Sev | Decision | Fix contract |
|---|---|---|---|
| R2-F-1 ticket-id strings log raw (2 loci: pre-parse error line forges lines; §20.3 failed arm embeds raw ESC) | MINOR | **CORRECT** | escape at the sinks (no ports change): `escalations_cmd.rs` pre-parse argument print + the §20.3 failed arm's `detail` interpolation get `{:?}`/escaped treatment; test: control-byte ticket id produces no raw 0x1b / no forged second line |
| R2-F-2 access log interpolates raw METHOD | MINOR | **CORRECT** | escape the method in `access_log_line` (same shape as C4's target fix); behavior-preserving (405/404 semantics unchanged); test extends the existing escape test |
| R2-F-3 transient-continue branch hot-spins (persistent EMFILE: 100% CPU, ~37 MB/s log flood) | MINOR | **CORRECT** | bounded backoff on the transient branch (short constant sleep before continue; documented at the classifier); C3's injected-sequence test stays green (may assert the sleep is bounded); classification semantics unchanged |
| R2-I-1 out-of-key unit read serves `unit:null` + true `settled:false` | INFO | **RECORD** — honest disclosure, unlike F-5's invented `open`; no action | closure |
| R2-I-2 matcher false-positive surface + hand-enumerated drift pin | INFO | **RECORD** — substring posture disclosed; compiler-enforced token association is the recorded future shape (next govern touch) | closure |

## Staging observation (recorded)

Mid-r2 the COR-1 changes were observed STAGED; the COR-1 builder's
final message said "nothing staged". Current state: exactly the ten
COR-1 task21 files staged, unstaged delta empty, nothing else staged,
nothing committed. The staging set is correct for the closure commit;
the claim inaccuracy is recorded here. No action.

## Proportionality note

r2 already CONFIRMs the commissioned invariants with the full sweep.
COR-2 tightens three already-attacked mechanisms; r3 is a FOCUSED
round on exactly these three fixes plus a no-bypass regression leg —
not a full re-commission.
