# Task 13 / Builder brief B2 — `settle::` service + controller wiring + resume completion

You are a BUILDER. Read `AGENTS.md` first. This brief is your complete scope.
B3 (arch rule) and B4 (composition proofs) are NOT yours.

## Context (read, in order)

1. `AGENTS.md`.
2. `.agents/reports/task13/recon-code.md` §4–§5 (verifier + controller maps —
   line numbers may have drifted a few lines after B1; locate by content).
3. `.agents/reports/task13/recon-spec.md` (§16 in full, §18, B.6).
4. `.agents/reports/task13/residual-adjudication.md` — items A1, A2, A4, A6,
   A7 are yours; the store legs (A3, A5) landed in B1.
5. `.agents/reports/task13/builder-b1.md` — the landed surface you compose:
   trait `commit_settlement(&AdmittedSettlement)` / `settlements(run_id)` /
   `complete_pending_settlements(run_id, anchor, deadline)` /
   `complete_pending_evidence_production(run_id, anchor, production,
   deadline)` / `record_verification(&VerificationRecordDoc)` /
   `verification_records(run_id)`; ports
   `SettlementCommitAuthority::admit(SettlementCommit, deadline)`.
6. `crates/gauntlet-app/src/verify/precondition.rs` — conditions 1–2 mint
   `SettlementReadiness` (no public constructor); its module doc names
   conditions 3–5 as yours.

## Deliverables

**Item 1 — the `settle` module (new `crates/gauntlet-app/src/settle/mod.rs`).**
A `SettlementService` that is the ONLY settle path in the application layer.
Input: the `VerifiedClaim`, its minted `SettlementReadiness` (conditions 1–2,
already computed in the loop — CONSUME the token, never re-derive from
caller-held values), the FROZEN unit (from the frozen projection, not an
in-memory caller snapshot), the closing context. It enforces, in order, with
typed refusals carrying the failure locus (failure class `settlement`):

- **Condition 3 (authority chain)** — read the frozen unit's `authority`:
  `critic` → automatic (exercised_by: runtime); `lead_agent` → orchestrator
  closure REQUIRES a recorded rationale (typed input; absent rationale
  refuses); `human` → operator action with recorded identity required — an
  in-loop automatic closure REFUSES and routes to the escalation surface
  (never loops, §11.5). The B.6 `authority_decision{required, exercised_by,
  operator_identity}` shape is recorded in the durable settlement payload or
  an accompanying record — follow what the schema can carry; document the
  mapping. Authority-blind settlement is the adjudicated defect (A7); a test
  must show a `human`-authority unit refused under automatic closure.
- **Condition 4 (blocking residuals)** — no blocking residual references the
  unit. Check against the controller's tracked open residuals AND, where a
  durable residual record exists, the durable state; on divergence between
  them, the DURABLE state wins and the divergence is a typed refusal cause
  (the preregistered P4 seam — implement the durable-side grounding now).
- **Condition 5 (settle-time reference re-verify)** — consult the reference
  provider's `verify_unchanged` (file-derived; NEVER compare against a digest
  the controller froze itself — D4 tautology rule). Mismatch → the run's
  terminal `aborted{reference_mutated}` path WITH both digests persisted as
  evidence (the deferral's text; the typed error already carries them —
  `evidence.put` + pin at the abort path).
- **The commit** — durably record the verification document if not already
  recorded (`record_verification`; idempotent on its identity), construct the
  `SettlementCommit` (reference_digest from the run's frozen
  `ReferenceVersion`), mint `AdmittedSettlement`, `commit_settlement`.
- **Idempotence on the resume path** — `AlreadyCommitted` for the SAME
  `(run, unit, verification)` key is an idempotent SUCCESS (returns the
  existing durable facts), not an error; any other admission refusal
  propagates typed. (The loop must be able to re-enter after a crash.)
- **In-memory advance LAST** — only after the durable commit returns:
  `Unit::settle` + `state.unit_states` update + claim release. The service's
  return type should make the ordering observable for B4's commit-order
  proof (e.g., return the durable offset + the advanced projection).

Module name `settle` exactly — the plan's gate filter is
`cargo nextest run -p gauntlet-app settle::` and must become non-vacuous
(it matches zero tests today). All your in-crate tests live under this
module path.

**Item 2 — controller wiring (the PASS leg).**
Replace the current in-memory-only settle (recon: `settlement_precondition`
asserted, readiness DISCARDED, `Unit::settle` + `unit_states.insert`, no
durable event) with the service call, in this order: verify → precondition
(readiness) → service (conditions 3/4/5 → record → admit → commit) →
in-memory advance. Thread the settlement append into the `state.last_offset`
water-mark (C8.1 discipline: every append). Precondition failures keep the
existing pinned-forensic-evidence behavior. The Task-12 proofs (p4/p6) MUST
stay green — if a proof depended on settle being non-durable, that is a
proof-harness adjustment ONLY where the assertion itself is unchanged (never
weaken an assertion; report any such adjustment as a deviation).

**Item 3 — resume completion (§18.3 step 1).**
`resume_run` gains, after `state.resume` and BEFORE re-planning:
(a) `complete_pending_settlements(run_id, anchor = checkpoint
event_log_offset, deadline)`; (b) the evidence-production completion loop for
orphan `EvidenceProduced` events at/after the anchor — re-mint each missing
production from DURABLE facts only (`EvidenceProductionAuthority::mint` +
store reads; producer identity/iteration/content digest must be recovered
from durable state — if any input is not durably recoverable for an orphan,
record the limitation honestly in a typed outcome rather than fabricating);
(c) after completion, reconcile in-memory unit states with the durable
settlement rows (a unit durable-settled but projected open in the checkpoint
becomes Settled — the projection is rebuilt from events per §5.3.3).

**Item 4 — §21.3 non-integrity mid-run dispositions (A6).**
- Answer-key READ failure inside the loop (currently hard
  `ControlError::Reference` from the plan path): becomes a typed
  non-terminal degradation — stop new dispatch for the iteration, keep the
  process alive, keep reconciliation running (§21.3 row 1), resume-able.
- Runner poll transport error (currently `Err(ControlError::Agent(..))`):
  routes through the §11.5 disposition table (retry/escalate per its rows)
  instead of error-propagating. The existing in-crate test
  `a_crash_after_a_settlement_never_re_runs_settled_work` pins crash
  semantics — update it to pin the DISPOSITION while preserving its
  resume-recovery assertion.
If either leg turns out to require more than disposition routing (design
change), STOP that leg, record evidence, and mark it deferred — do not
redesign the controller error architecture (preregistered R3/A1).

**Item 5 — AK-062 service-level no-bypass (the half Task 13 owns).**
The service treats every caller identically: an operator-action-shaped
settle request (a flagged request type or a documented composition seam —
design the minimal honest shape) lacking PASS evidence is refused with the
SAME typed error as the automatic path. In-crate test proving no-bypass.
(The CLI operator surface itself is Task 21 — do not build CLI commands.)

## Test requirements

- Fail-first evidence for every behavioral change (RED observation → fix →
  GREEN), recorded per item.
- The module-path filter `settle::` must select your service tests; include:
  preconditions matrix (AK-040 service level — each refused condition
  distinct), authority-chain trio (critic/lead_agent-without-rationale/
  human-under-automatic), blocking-residual refusal incl. the durable-wins
  divergence case, settle-time digest tripwire (both digests persisted),
  commit order observable (event before row before advance), idempotent
  re-entry, resume completion (settlement + evidence production),
  no-bypass (AK-062 half).
- Keep `control::` (75), `verify::`, `compare::`, p4/p6, p12, p2 green.

## Hard constraints

Same as B1: never touch `.wayfinder/`, `.gauntlet/reference.lock`,
`deny.toml`, nextest config, `AGENTS.md`, `.jolli/`, `.tmp/`,
`.agents/prompts/`, applied migrations (NO new migration — the schema is
sufficient). No sub-agents. No parallelism increases. `just ruler` before
and after (PASS). No `unwrap`/`expect` in production paths.

## Checkpoint discipline (mandatory)

Append to `.agents/reports/task13/builder-b2-checkpoints.md` after EACH item:
STARTED before editing (one-line intent), DONE after (what landed, test
names, fail-first evidence RED/GREEN).

## Result contract

`.agents/reports/task13/builder-b2.md`: items, deviations, fail-first
evidence, gate tails verbatim, files touched, known gaps.

## Gates (run personally, record verbatim tails)

```bash
just ruler
just check-package gauntlet-app
just test-package gauntlet-app
cargo nextest run --test p4_corrective_loop --test p6_budget_control --test p12_missing_is_not_passing --test p2_context_isolation
just lint
```
