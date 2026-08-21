# Task 10 correction round 5 — deadline enforcement

## Scope actually changed

Implemented the final-confirmation round-3 deadline defect (item 3). The model
adapter previously passed a deadline to `collect` but its fan-in probe loop
ignored it. It now owns an injected `Clock`, checks the absolute deadline before
every poll and before result collection, and cancels the admitted invocation
immediately on expiry. A cancellation refusal returns typed `RecoveryRequired`;
the review layer retains/quarantines the lease through its existing recovery
path.

The default constructor uses the adapter's system-clock implementation. Tests
and composition tests use `with_clock` with the existing deterministic clock.
This prevents fixed historic test deadlines from silently becoming host-time
dependent.

## Tooth

`expired_deadline_cancels_before_the_first_poll` builds a pending hanging critic
with an already expired injected deadline. It proves the poll count is zero and
the owned pending handle is cancelled exactly once, returning `TimedOut`.

## Gates

- `cargo test -p gauntlet-adapter-compare-model`: 8 passed
- `cargo test -p gauntlet-cli --test p2_context_isolation`: 5 passed
- `just test`: 685 passed
- `just check`, `just lint`, `just deny`, `just boundaries` (194 passed),
  `just ruler`: passed

## Honest residual state

This correction does **not** claim to close Task 10. The round-3 provenance,
accepted-record split, and adversarial-source boundary defects require the
broader cross-crate API refactor described in that confirmation report; they
remain to be resolved and must not be reclassified as theoretical. No frozen
reference, CURRENT_STATUS, or reference-candidate artifact was changed.
