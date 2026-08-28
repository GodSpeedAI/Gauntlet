# Task 13 / Correction round 2 result contract — COR2-1..COR2-6

**Provenance:** dispatched as a general-purpose builder; three consecutive
dispatch cancellations/deaths left a partially-landed window (COR2-1a
guard, COR2-2, COR2-3 implemented unrecorded); the orchestrator completed
the remainder personally per the operator's standing heal+continue
instruction (lineage recorded in `correction-checkpoints.md`). All
fail-first evidence for orchestrator-run items is re-derived
(neutralize→RED→restore→GREEN) and labeled as such.

## Findings closed (source: `critic-round1.md`, blind spec critic + falsifier)

- **F-1 CRITICAL (double settlement via declared attempted list)** —
  closed at BOTH layers: (a) admission refuses a different-key commit
  when any settlement row exists for the (run, unit)
  (`UnitAlreadySettled`, `producer_authority.rs` — predecessor-landed,
  test + fail-first re-derived); (b) the completion path processes only
  the flight's own unit, pinning `attempted_unit_mismatch` governance
  evidence for anything else (`controller.rs`, COR2-1b).
- **m-3** — Operator closures require an identity for EVERY authority
  (the `(Critic, _) => true` arm narrowed; `settle/mod.rs:1048`
  predecessor-landed, battery-green).
- **m-5** — `settle_operator_action` filters records by run
  (`settle/mod.rs:1702`, predecessor-landed).
- **F-5** — the inherent `append_record` no longer writes
  `RecordKind::VerificationRecord` for ANY caller (typed refusal naming
  the earning authority; the trait impl uses the private raw helper).
  Rescoped from visibility-gating after export_stack's benign generic
  call — the KIND gate is the complete closure.
- **F-6** — anchored replay is loud on a same-key DIVERGENT event (typed
  `Corrupt` naming both sides; evidence+digest compared, offset
  deliberately not — same-facts duplicates at new offsets stay the
  legal §16.3 idempotency). The shared-suite case now pins the refusal
  (intended, evidenced change — the old silent expectation was the
  finding), green stub conforms.
- **m-10** — precondition module doc trued.

## Later-scope (from `critic-round1-adjudication.md`, unchanged)

M-1 minimum_reliability (reference candidate + deferral), M-2/F-2 AK-062
positive half (Task 21), F-3 TOCTOU (concurrency, Task 15/20), F-4
transient non-in-flight edit window, F-7/m3 recovery-time digest
(content-addressed adapter), m-1/2/4/6/7/8/9/11 (residual list for
closure).

## Final battery (orchestrator-run)

ports **105/105** · app **330/330** · sqlite **48/48** · proofs
**12/12** · standing battery **33/33** · lint **0** · boundaries
**215/215** · deny ok · ruler PASS.
