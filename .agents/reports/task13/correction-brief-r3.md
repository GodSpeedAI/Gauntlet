# Task 13 / Correction brief round 3 — COR3 (fresh blind confirmation round 1, F-1)

You are the CORRECTION BUILDER. Read `AGENTS.md`, then
`final-confirmation-b-falsifier.md` (finding F-1, with the executed probe)
and `final-confirmation-round1-adjudication.md` (YOUR SCOPE — COR3 only;
the later-scope list is NOT yours). Checkpoint discipline: STARTED/DONE
appended to `correction-checkpoints.md`, never batched; fail-first first
(the falsifier's executed sequence IS the RED). Result contract:
`correction-round3.md`. You hold the build lock exclusively; no sub-agents;
/tmp is ~95% full (keep cargo temps off it — the workspace target/ dir is on
a disk with ample space; do NOT clean /tmp/opencode).

## COR3 — gate `RunEventKind::SettlementCommitted` on both public append surfaces

**Invariant to establish:** no public path can append a
`SettlementCommitted` event except `commit_settlement` (which consumes the
authority-minted `AdmittedSettlement`). The two public surfaces that must
refuse the kind with a TYPED error naming the earned path:

1. `StateStore::append_event` (ports trait `crates/gauntlet-ports/src/
   state_store.rs:291`; sqlite impl `crates/adapters/
   gauntlet-adapter-state-sqlite/src/state_store.rs:1567` → shared
   `append_to_log`).
2. The `EventLog` port's `append` (`crates/gauntlet-ports/src/event_log.rs:59`;
   sqlite `crates/adapters/gauntlet-adapter-state-sqlite/src/event_log.rs:211`
   → the same `append_to_log` over the same DB file).

`commit_settlement`'s internal append keeps working (same module; give it
the ungated internal path — the COR2-4/`apply_settlement_row` idiom: the
public surface refuses, the earned internal path is the only producer; do
NOT change replay/`complete_pending_settlements` semantics at all).

**Fail-first RED (both legs of the falsifier's executed sequence, written
as in-suite tests — the /tmp probe is not a dependency):**
- Leg 1: sqlite store, append `SettlementCommitted{invented facts}` via
  `StateStore::append_event` → TODAY succeeds at an offset and
  `complete_pending_settlements` (or honest resume) mints a durable
  settlement row with zero verification records. AFTER: typed refusal, zero
  durable writes (no event row at a new offset, no settlement row — assert
  on the store's own export).
- Leg 2: same forged event via `SqliteEventLog::append` over the same DB
  file → same RED → same typed refusal.
- If some conforming surface legitimately needs the append today (audit the
  in-repo callers of `append_event`/`EventLog::append` for
  SettlementCommitted use besides `commit_settlement` — there should be
  none in production; crash_helper and tests may need the internal path or
  an earned admission), record the audit in the result contract.

**Shared contract suites (A2 class — extend, don't exempt):** the
StateStore contract suite and the EventLog contract suite each gain a case
pinning the refusal (a conforming adapter/stub refuses the kind with the
typed error). Update the green stubs to conform. Typed errors carry the
failure locus per repo policy (no bare strings).

**Explicitly OUT of scope (do not touch):** replay/anchored-completion
semantics; the `checkpoint()` forged-row projection class
(trigger-recorded, not yours); F-2 (B.6 re-pin), F-3 (canonicalization),
RES-A1..A5; any unrelated cleanup; migrations (none needed — this is a
surface gate, not a schema change).

**Gates (verbatim tails in your result contract):** `just ruler` before and
after; `just check-package gauntlet-ports` + `just test-package
gauntlet-ports`; same for `gauntlet-app` and
`gauntlet-adapter-state-sqlite`; the three proof filters
(`cargo nextest run -p gauntlet-cli --test p7_recovery --test
p11_critic_is_not_verifier --test g10_builder_writes_answer_key`); the
standing battery (`p4_corrective_loop`, `p6_budget_control`,
`p12_missing_is_not_passing`, `p2_context_isolation`, `p3_real_observation`);
`just lint`; `just boundaries` (expect 215/215 — if your change grows the
surface-module inventory, re-pin ONLY with the evidence that the new spelling
is intended behavior, never silently); `just deny`; `just prereg-check 13`.
All standing constraints unchanged: no frozen-path edits, no weakened
assertions, no golden re-pins without evidence, no sub-agents, one cargo at
a time.
