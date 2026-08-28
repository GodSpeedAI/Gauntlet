# Task 13 — correction round 3 (COR3): the settlement event kind is earned, never appended

Source: fresh blind confirmation round 1, F-1 (falsifier, executed) —
`.agents/reports/task13/final-confirmation-b-falsifier.md`; adjudication
`.agents/reports/task13/final-confirmation-round1-adjudication.md`; brief
`correction-brief-r3.md`.

## What changed

1. **Typed errors (ports).** `StateStoreError::SettlementEventNotEarned
   { run_id, unit_id }` and `EventLogError::SettlementEventNotEarned
   { run_id, unit_id }` — both Display strings name the earned path
   (`commit_settlement` consuming an authority-minted `AdmittedSettlement`)
   and state that nothing was recorded.
2. **Gates at both public append surfaces (adapter).**
   `SqliteStateStore::append_event` (trait impl) and `SqliteEventLog::append`
   (the `EventLog` port over the same DB file) each refuse
   `RunEventKind::SettlementCommitted` before any durable write — the
   COR2-4 `append_record` kind-gate idiom, mirrored at the event layer.
3. **Earned internal paths (adapter).** `commit_settlement` keeps its own
   internal append (unchanged semantics); a new pub half-step
   `append_settlement_event(&AdmittedSettlement)` lands §16.3 step 1 ALONE
   over an EARNED admission — the crash-recovery instrument's route
   (`crash_helper`'s `settle after-append` script now earns its admission
   and aborts after the earned half-step); a crate-private
   `append_settlement_event_unchecked` exists for the adapter's own
   fixtures that model real crash-window states the public API can no
   longer create on purpose.
4. **Shared contract suites extended (A2 class).** StateStore suite:
   `settlement_events_are_earned_and_replay_never_disturbs_rows` (renamed
   and re-scoped from `settlement_replay_is_keyed_idempotent_and_anchored`)
   — three earned commits stand; the F-1 forged payload through the generic
   append is refused with the typed error and changes nothing durable;
   replay over complete commits is the idempotent no-op at anchors 1 and 2;
   all rows stand. EventLog suite: new case
   `settlement_kind_appends_are_refused` — the forged append through the
   log port is refused typed and leaves the log empty. Both suites'
   generic seeding now uses ordinary runtime kinds only (InvocationAdmitted
   / CheckpointWritten / EvidenceProduced). Green stubs conform: the
   DurableStore's `append_event` refuses the kind and its
   `commit_settlement` appends through a private ungated helper;
   GrowingLog's `append` refuses the kind.
5. **Coverage delegation (intended reduction, recorded).** The trait no
   longer admits raw settlement events, so the half-applied crash window,
   the anchor's below/above discrimination, and the COR2-5
   divergent-duplicate refusal are no longer contractable through the
   shared suite. They remain pinned at the adapter level (in-crate
   replay/crash fixtures via the internal earned append; `crash_helper`
   real-abort windows; tamper-evidence COR2-5 tests) and in the composed
   `p7_recovery` proof. Ports-revision trigger recorded: a factory-level
   crash-window injector if trait-level anchoring coverage is wanted again.

## Caller audit (brief item)

In-repo callers of the gated surfaces, exhaustively (`rg` over crates/):
- Production: `commit_settlement`'s internal append (earned, unchanged) and
  the controller's ordinary event writes (no settlement kinds) — the
  settle path already went through the service. NO production caller
  appended settlement events through the generic surfaces.
- Instruments: `crash_helper` — the `settle after-append` script converted
  to the earned half-step (`earn_settlement_admission` +
  `append_settlement_event`); its remaining `append_event` uses are
  `InvocationAdmitted` kinds (ordinary).
- Tests/fixtures: two in-crate sqlite fixtures converted to
  `append_settlement_event_unchecked`; one tamper-evidence test converted
  to the earned `earn_settlement` + `commit_settlement` idiom (its
  corruption step was already raw SQL); the dead `settlement_event` helper
  removed.

## Fail-first evidence (RED→GREEN, recovered-honest)

The builder died to the usage limit before recording any evidence (see
`correction-checkpoints.md` RECOVERED-AND-VERIFIED). The orchestrator
re-derived fail-first personally:

- RED (both gates neutered via a one-statement rewrite, backups kept):
  `settlement_append_gate` = **0 passed, 2 failed** — leg A panics "the
  forged settlement event must be refused by the generic append surface,
  but it appended durably at offset 1" and leg B the mirror at the
  EventLog port — exactly the falsifier's executed F-1.
- Restored via `cp` + `cmp` byte-identical → GREEN: **2/2 passed**.
- Pre-fix RED at suite level (the landed-but-unrecorded state, observed
  before any fixture conversion): sqlite package 45/50 — the two in-crate
  fixtures, both shared contract suites, and the tamper test failed
  against the gated surfaces, demonstrating the gate changes behavior
  (not vacuous).

## Gates (orchestrator-run, 2026-08-28, verbatim counts)

`just ruler` PASS before and after; ports **105/105**; app **330/330**;
sqlite **50/50** (includes the 2 new gate tests); proofs p7/p11/g10
**12/12**; standing battery p4/p6/p12/p2/p3 **33/33**; `just lint` **0**
(after `cargo fmt -p` the touched packages); `just fmt-check` clean;
boundaries **215/215** (one denied-identifier literal in a comment fixed —
"sqlite" is a vendor name, core crates may not spell it); deny **ok**;
`just prereg-check 13` **PASS** (frozen d949e1e0…).

## Deviations

- The suite case rename (`…_anchored` → `…_earned_and_replay_never_disturbs_rows`)
  with the coverage-reduction note: driven by COR3 itself — the anchored
  discrimination required raw settlement injection, which the correction
  removes by design. Teeth preserved: `StateStoreSettlementLax` is still
  caught by the renamed case (the earned commits/rows assertions fail on
  the lax fake).
- The `u64::MAX` anchor probe was dropped from the case (the sqlite offset
  binding is i64; past-the-end read semantics belong to the EventLog suite).

## Attribution

COR3's gates, typed errors, earned half-step, and fail-first test file were
landed by the dispatched builder (2026-08-27 21:29–21:39 EDT) before its
19th-instance death to the usage limit; recovered, verified, completed
(contract suites, stubs, fixture conversions, caller audit, gates, records)
by the orchestrator 2026-08-28 per the operator's standing heal+continue
instruction and explicit "continue" directive (sub-agent dispatch was
usage-limited; the compiler lane is orchestrator-exclusive either way).
