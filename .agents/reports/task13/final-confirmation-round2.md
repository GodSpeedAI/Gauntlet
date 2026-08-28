# Task 13 — fresh blind confirmation round 2, record + adjudication (orchestrator, 2026-08-28)

Sources: `final-confirmation-round2-a-spec.md` (Role A, spec critic,
static-only — **VERDICT: CONFIRM**, 0 blocking / 0 major / 7 minors
FC2-A1..A7) and `final-confirmation-round2-b-falsifier.md` (Role B,
falsifier, executed probes — **VERDICT: CONFIRM**, 0 CRITICAL / 0 MAJOR /
1 MINOR F-1 + 3 observations OBS-1..3). Both agents adopted the operator's
role charters verbatim (`.agents/agents/{critic,falsifier}.agent.md`;
general-purpose transport — typed role agents still not invocable from the
session tool). Blindness held in both reports; the round ran over the
COMPLETE post-COR3 tree.

## Combined verdict: CONFIRM — Task 13 settles

Both roles CONFIRM with nothing above MINOR. The confirmation bar
("nothing above MINOR") is met. Adjudication of the minors (all
trigger-recorded in the deferral ledger / task-13.outcome.yaml; none
blocks settlement):

- **Round-2 falsifier F-1 (MINOR)** — the public earned half-step
  `append_settlement_event` (the crash instrument's route) combined with a
  second admission under a different key can mint two durable rows,
  because the admission's one-per-(run,unit) guard reads settlement ROWS
  only, not appended-but-unapplied settlement EVENTS. Executed
  out-of-crate against the real adapter; NOT reachable on any production
  path (the sole production writer is the service's atomic
  `commit_settlement`); flagged MAJOR-if-ever-relied-on across process
  boundaries. Trigger: next state-store/ports touch (admission should also
  see pending settlement events).
- **OBS-1 / FC2-A3** — the two public port authorities compose a
  chain-free settlement for trusted in-process callers: the module's
  DOCUMENTED composition-root trust assumption (roles never hold the
  store), not a hidden hole; trigger: the ports revision (chain-shaped
  earning). FC2-A2 (admit does not re-validate the commit's
  reference_digest against the frozen run; production derives it from the
  durable run) — same class, same trigger.
- **OBS-2 / F-3(round 1)** — lexical-only locator containment vs §17.5.2
  canonicalization; AK-043's "prevented OR detected" holds (the detection
  leg re-reads the key non-tautologically; g10 green). Trigger: the
  §17.5.2/workspace-provider write-boundary touch.
- **OBS-3** — re-invoking the half-step with one admission appends
  duplicate same-key events; replay stays the idempotent no-op (same
  trusted instrument surface as F-1; same trigger).
- **FC2-A1** — resume does §18.3 step 2 before step 1 (re-verify first):
  documented deviation (m-1 lineage); the mutation+crash-window
  intersection is untested — recorded with the RES-A1/A4 cluster.
- **FC2-A4** — §18.3 steps 4–5 absent (RES-A4 lineage; AK-053 scope note).
- **FC2-A5** — "mounts the key read-only" landed as
  placement-outside-workspace + typed write-boundary refusal + detection;
  §16.4's disjunction satisfied; real mounts are Task 14.
- **FC2-A6** — the B.6 authority-decision pin's post-commit crash window
  (= round-1 F-2); unchanged disposition.
- **FC2-A7** — p11 instantiates "comparator pass" as a clean comparison
  (position-based reasoning, documented in the proof; the PASS control
  leg keeps it discriminating). Recorded; no action.

## Why round 2 confirms where round 1 did not

Round 1's F-1 (the ungated `SettlementCommitted` append) was fixed as
COR3: both public append surfaces (the StateStore trait's `append_event`
and the `EventLog` port) now refuse the kind with typed
`SettlementEventNotEarned` errors; the earned producers
(`commit_settlement` and the crash instrument's half-step) consume the
authority-minted `AdmittedSettlement`; the shared contract suites pin the
refusal (green stubs conform) and the fail-first legs are in-suite
(`settlement_append_gate.rs`, recovered-honest RED 0/2 → GREEN 2/2).
Round 2's falsifier EXECUTED both gate legs and the full proof battery
(settle 22/22, ports admission suite incl. fresh-key UnitAlreadySettled
teeth, sqlite 50/50, composed p7/p11/g10 12/12, arch 215/215) and
attacked the new gate surfaces fresh — the residual F-1 above is the
narrow instrument-route seam, honestly scoped MINOR.

## Settlement basis (the orchestrator's own verification, 2026-08-28)

- Gates personally run this session: ruler PASS (before and after every
  phase); ports 105/105; app 330/330; sqlite 50/50; proofs p7/p11/g10
  12/12; standing battery p4/p6/p12/p2/p3 33/33; lint 0; fmt-check clean;
  boundaries 215/215; deny ok; prereg-check 13 PASS.
- Teeth personally re-verified: COR3's gate tooth (neuter both gates →
  both legs fail with the falsifier's exact panic; cp+cmp byte-identical
  restore → 2/2 green) and B4 Tooth 1 re-run at closure (commit-neuter →
  commit_order RED 0/1; byte-identical restore → 5/5 green). Teeth 2 and
  3 stand on their finisher-run records (teeth-b4.md) plus the round-2
  falsifier's independent executed proof runs.
- Reference-candidates entries pasted (9 dated 2026-08-28 entries; every
  locator verified on the tree by the closure-research verifier;
  `closure-reference-candidates-draft.md` holds the provenance notes).
- task-13.outcome.yaml written per TEMPLATE (P1 supported, P2 supported
  ×4 firings, P3 supported both-poles, P4 supported, P5 falsified — the
  predicted masked-spelling escape never fired within declared bounds).

**TASK 13 SETTLED.** Next work task: task14 (Authority gate, sandbox
port, tier policies — P5).
