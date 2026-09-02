# How-To: Handle Operator Escalations

This guide explains how to inspect, resolve, and retry human escalation tickets created when an operation exceeds autonomous agent authority or requires human approval.

---

## Goal

Resolve a pending escalation ticket (e.g. an `irreversible` blast radius action or a sandbox policy denial) and resume the governed run.

---

## Prerequisites

- Access to the `gauntlet` CLI.
- Access to the target repository containing `.gauntlet/escalations/`.

---

## Why Escalations Occur

An escalation ticket is automatically generated when:
1. **Irreversible Blast Radius**: A Unit declares `blast_radius: irreversible` (e.g. database schema migrations, cloud infrastructure destruction, credential issuance).
2. **Authority Mismatch**: A Unit requires `authority: human`, but an autonomous agent is driving execution.
3. **Sandbox Denial**: A unit requires `cross_cutting` containment, but the host environment lacks a configured isolation backend (such as Bubblewrap).
4. **Diagnostic Exhaustion**: Non-converging residuals exhaust the allocated diagnostic budget.

---

## Procedure

### Step 1: List Open Escalations

Run `gauntlet escalations list` to view all pending tickets:

```bash
gauntlet escalations list
```

### Sample Output
```text
Open Escalation Tickets (1 pending):
  - ID: TKT-01J6A7BC9D2F8G1H0J
    Unit: AK-003 (Production schema migration)
    Reason: blast_radius=irreversible requires authority=human
    Created: 2026-09-02 18:15:00 UTC
    Path: .gauntlet/escalations/pending/TKT-01J6A7BC9D2F8G1H0J.json
```

### Step 2: Inspect Ticket Details

Examine the JSON ticket payload in `.gauntlet/escalations/pending/`:

```json
{
  "ticket_id": "TKT-01J6A7BC9D2F8G1H0J",
  "run_id": "01J6A7A8Z9...",
  "unit_id": "AK-003",
  "requested_tier": "irreversible",
  "reason": "AuthorityGate refused automated dispatch for irreversible unit",
  "created_at": "2026-09-02T18:15:00Z"
}
```

### Step 3: Resolve the Escalation

After performing the necessary manual inspection, review, or out-of-band action, resolve the ticket:

```bash
gauntlet escalations resolve TKT-01J6A7BC9D2F8G1H0J \
  --resolution approved \
  --evidence .agents/reports/migration-approval.json
```

> [!IMPORTANT]
> **No Bypass Invariant (AK-062)**:
> An operator cannot force a Unit to `PASS` without attaching valid evidence. The `resolve` command routes through the exact same `SettlementService` validation path as automatic settlement. If the attached evidence is missing or invalid, the resolution is rejected.

### Step 4: Resume the Run

Once the escalation is resolved, resume the run from its durable checkpoint:

```bash
gauntlet resume 01J6A7A8Z9...
```

The runtime detects that ticket `TKT-01J6A7BC...` has transitioned to `resolved` and proceeds with execution.

---

## Common Failure Symptoms

| Symptom | Cause | Fix |
|---|---|---|
| `resolve` fails: `evidence_not_found` | The path passed to `--evidence` does not exist. | Generate or provide the real evidence artifact before resolving. |
| `resolve` fails: `missing_passing_verification` | Attached evidence failed verification check. | Fix the underlying issue so the test passes before resolving. |
| Run terminates immediately with `Escalated` | Unit was re-dispatched without resolving the ticket. | Resolve the pending ticket first using `gauntlet escalations resolve`. |
