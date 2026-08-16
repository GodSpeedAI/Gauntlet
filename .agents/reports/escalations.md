# Escalations

Recorded per the standing rule: a genuine blocker (missing tool, ambiguous spec, failed
teeth check that cannot be fixed, irreversible-tier action, human sign-off) is written here
with full context; work continues on the next unblocked task. A blocker is never worked
around by downgrading a tier or an authority.

---

## ESC-001 — Delegation capacity exhausted; Task 3 not started

- **Opened:** 2026-08-15
- **Status:** **CLOSED** 2026-08-15 — the session limit reset, delegation resumed, and the
  deferred blind verification ran. It found 4 critical findings and Task 2 was reopened
  (`.agents/reports/task2/verification-round3.md`). Recording that outcome is the point: the
  escalation's stated risk — *"Task 2's corrected artifact has never been independently
  reviewed… do not upgrade AK-001/002/016 beyond `implemented but provisional`"* — was
  correct, and holding the line on it is what caught the four holes.
- **Blocks:** Task 3 (domain types) and every task after it
- **Does not block:** nothing else; Tasks 0–2 are complete and committed

### What happened

The Task 2 correction workflow (`wf_8d6d78e3-347`) ran one builder and two blind critics.
All three agents terminated with `You've hit your session limit · resets 1:20pm
(America/New_York)`. The builder had already landed its corrections before dying; the two
blind critics had not started reviewing. Subsequent delegation is unavailable until the
limit resets.

### Why this is an escalation rather than something to work around

`.agents/plans/gauntlet-impl.md`, Guardrails: *"Do not let the orchestrator implement. While
delegation is available, decompose and dispatch. If delegation is genuinely unavailable, stop
and report `blocked{no_delegation_mechanism}` unless the override variable is explicitly
set."* The override variable (`GAUNTLET_ALLOW_ORCHESTRATOR_IMPLEMENTATION`) is not set.

So the correct action is to stop and record, not to implement Task 3 inline. Implementing it
directly would be exactly the silent workaround the standing rules forbid — it would also
destroy the independent-blind-critic property that every builder slice is supposed to have.

**Reported state: `blocked{no_delegation_mechanism}`.**

### Two consequences that must not be lost

1. **Task 2's corrected artifact has never been independently reviewed.** It has a passing
   gate and a demonstrated teeth check (seven cases, all biting), but the blind re-critique
   that was supposed to verify the corrections died before running. The artifact is
   *implemented and teeth-checked*, not *independently verified*. Do not upgrade AK-001,
   AK-002, or AK-016 beyond `implemented but provisional` on the strength of the gate alone.

2. **The first form of teeth case T1 was INCONCLUSIVE and is recorded as such.** Adding a
   path dependency on a crate absent from `[workspace].members` made `just boundaries` exit
   non-zero — but via cargo's own resolver error (`no matching package named 'infra'`), not
   via our violation message. That run proves nothing about our mechanism. The mechanism was
   then tested directly (a stray package directory under `crates/` not listed in members),
   which does fail with our check. `INCONCLUSIVE` was not recorded as `PASS`.

### Clearing this

When delegation is available again:

1. Run a blind critic (read-only, non-compiling) over `crates/gauntlet-arch-tests/` and
   `tests/contract/`. Any surviving **critical** finding reopens Task 2.
2. Then start Task 3 per `.agents/CURRENT_STATUS.md`.
