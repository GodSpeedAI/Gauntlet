# Task 13 — early-probe adjudication (orchestrator, 2026-08-26)

Source: `early-adversarial-probe.md` (read-only agent, static battery).
Adjudication against spec §16.1/§16.3/§16.4, the preregistered R1/A5
intervention class, and the tree. Prereg P2 and P4: FIRED as predicted.

## FIX NOW (correction round 1 — dispatch before B4)

**C1 → COR-1. Capability-gate `record_verification` + close the inherent
fabrication path.** A ports recording authority following
`producer_authority.rs` (validates durable run + iteration anchor +
derives what it can, mints `AdmittedVerificationRecord`, the trait write
consumes it) — the verified chain (VerifiedClaim) is the minter's caller;
test fixtures + crash_helper earn through it. The inherent pub
`apply_settlement_row` becomes test-only visibility (#[cfg(test)] or the
crate's established test-gating idiom) — B1 recorded it as "test-facing";
make that structural. Fail-first: an ungated write attempt that today
compiles+writes must be shown refused/non-compiling after.

**M3 → COR-2 (root of M4's practical attack). Pairing + claim-outcome at
service entry, before ANY durable write.** `settle()` re-derives
conditions 1–2 by calling `settlement_precondition(&claim, run)` INTERNALLY
(the readiness parameter dies; `SettleRequest` carries the claim only) —
portability of the token between claims becomes unrepresentable — AND an
explicit `claim.record().outcome() == Pass` entry check. Fail-first: the
FAIL-claim + stale-PASS-readiness attack from the probe, driven through
the service, must refuse with NO durable write (assert zero events/rows).

**M1 → COR-3. Moor the verification ref in the durable record.**
`VerificationRecordDoc` gains the verification-ref field (the domain
record HAS the id; the Annex-B sample simply doesn't show it — samples are
illustrative, the strict codec evolves with a fail-first round-trip +
golden update as an evidenced intended change). `admit` then requires the
durable PASS record's ref to EQUAL the commit's verification ref
(key component three becomes durable-moored; N-refs-per-PASS dies).
Fail-first: the N-distinct-keys attack — after the fix each commit beyond
the first must refuse (no duplicate settlement event).

**M2 → COR-4. `AlreadyCommitted` compares the stored row.** The arm reads
the existing row and requires evidence AND verification AND unit
agreement; any divergence → typed `AlreadyCommittedDivergence` refusal
(not success). Fail-first: the probe's E2 shape.

**M5 → COR-5. `admit` compares `record.run_id()` to the commit's run.**
One comparison + fake-store divergent test (the ports' own fake that
ignores run scoping becomes the discriminating instrument).

**m2 → COR-6. Escape the operator identity** via the shared
`escape_json_text` (the site `pub(crate)`'d for exactly this); injection
test with quotes/backslashes in the identity.

**m1 → COR-7. Read defaults fail closed.** `settlements()` /
`verification_records()` defaults return the store's typed
not-implemented refusal instead of empty (mirror the write defaults;
the suite's conforming stubs already implement them — the green stub
stays green, the fail-closed doctrine extends to reads). Fail-first: a
stub implementing writes but not reads must now surface loud, not silent.

## LATER-SCOPE (recorded, triggers)

- **M4 (iteration/recency-blind admission)**: after COR-2/COR-3 the
  practical attack (stale readiness) is unrepresentable; what remains is
  the literal §16.1 "exists in this run" reading vs §14.3 recency. Adding
  an iteration-recency predicate at settle duplicates the verification
  chain's discipline. Trigger: the ports revision or a spec ruling on
  §16.1-cond-1 recency. Recorded as a spec question, not silently
  dropped.
- **m3 (recovery-time digest derivation)**: content addressing is the
  evidence port's documented adapter contract; the fs adapter satisfies
  it. Trigger: any non-content-addressed EvidenceStore adapter.
- **m4 (transient never escalates)**: bounded by rounds (proven); an
  attempt ceiling for row-2 retries is a §11.5 tuning decision. Trigger:
  Task 15/20 real-runner composition.
- **m5 (re-read-file claim/requirement at verification)**: window is
  edit+revert-between-boundaries; any net edit trips the boundary
  tripwire. Trigger: next controller/verify touch or the spec-frozen
  claim-vs-frozen-unit equality question at the ports revision.

## REJECTED

None — every finding is evidenced and either fixed or trigger-recorded.
