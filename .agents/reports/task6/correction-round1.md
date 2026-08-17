# Task 6 correction round 1 — builder report (2026-08-17)

Adjudicated corrections from critic A (`critic-a-result.md`) and critic B
(`critic-b-result.md`), implemented test-first where a mechanism changed.
Checkpoint log: `builder-checkpoints.md` (one line per item, with gate
results). All 13 items implemented; none skipped.

## Fixes

1. **commit_settlement direct coverage** (critic A MAJOR-1 + B A3.2).
   (a) In-crate test `commit_settlement_is_the_spec_sequence_and_idempotent_by_key`
   (`crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs`)
   calls the packaged primitive directly: returned offset == settlement row's
   `event_log_offset`; the event at that offset decodes to
   `SettlementCommitted` with exactly the commit's fields (run, at, unit,
   evidence, verification, digest); `settlements()` shows one row; replay
   adds 0; a second `commit_settlement` of the same key appends a second
   event (offset strictly greater; append-only) while the keyed row count
   stays 1 and replay still adds 0.
   (b) `crash_helper settle-via-commit <db> <run-id> <unit-id> <abort-point>`
   (`src/bin/crash_helper.rs`) calls `commit_settlement` — the PACKAGED
   primitive, not the two steps — with abort points `before` (nothing
   durable, unit retryable) / `after` (fully settled; killed anyway) /
   `none`; module docs updated. Real-child-process tests in
   `tests/crash_recovery.rs`:
   `the_packaged_commit_primitive_settles_completely_when_killed_after`
   (one event + one row; replay adds 0) and
   `a_crash_before_the_packaged_commit_leaves_the_unit_retryable`.
   Fail-first observed: the `after` test FAILED pre-fix (unknown script →
   nothing durable). The `before` test passes vacuously against script
   absence (an unknown script exits non-zero with no durable state — the
   same observable); the `after` test is the discriminating one.
   (c) Recorded deferral, stated plainly: a pure two-statement SWAP inside
   `commit_settlement` remains undetectable without mid-function abort
   injection; that discrimination is Task 13's
   `tests/proofs/p7_recovery::commit_order` — AK-041's declared locator.
   This round proves the packaged primitive's contract at its edges, not
   full intra-call ordering.

2. **Negative timestamps are damage, not clamped** (B A4.3). All four
   `.max(0) as u64` sites removed: `event_log.rs` read path →
   `EventLogError::Damaged` naming the offset and value; `state_store.rs`
   `created_at` → `Corrupt` naming the run; projection `MIN(at)` →
   `Corrupt` naming the run; records-export `at` → `Corrupt` naming the
   row's seq. Tests in `tests/tamper_evidence.rs` write the negative rows
   via a raw second connection (INSERT, mirroring crash_helper's
   corrupt-tail instrument). All four FAILED pre-fix (epoch-0 clamp / Ok)
   and pass now.

3. **Settlement-row offsets cross-validated against the log** (B A3.4).
   `verify_settlement_event_at` runs before every settlements insert
   (`insert_settlement`, so all three callers get it): SELECTs the
   event_log row at the claimed offset and verifies run id,
   `settlement_committed`, and payload unit/verification/evidence/digest
   against the commit; a missing row or any mismatch is a typed
   `StateStoreError::WriteFailed` naming both what was claimed and what is
   there. `complete_pending_settlements` passes trivially (it hands over
   the log's own offsets — pinned by a test). Tests: foreign-event offset
   and beyond-log offset both FAILED pre-fix (row landed / Ok) and are
   refused now; the own-offset happy path is pinned green.

4. **Half-settled error names the durable state** (B A3.5). When
   `commit_settlement`'s step 2 fails after step 1 is durable, the error
   detail now states: the settlement event IS durable at offset N;
   `complete_pending_settlements` will complete it on replay; re-running
   the unit is wrong (it would append a duplicate settlement event).
   Test (`a_half_settled_failure_names_the_durable_offset_and_the_recovery`)
   injects a real step-2 failure with a trigger whose body references a
   missing table — a hard error `INSERT OR IGNORE` cannot swallow (unlike
   `RAISE(ABORT)`) — asserts the wording carries the offset, then drops
   the injection and proves the guidance is real: replay completes the
   settlement. FAILED pre-fix (bare WriteFailed, no offset/recovery
   wording).

5. **Damaged → Corrupt, not ReadFailed** (B A4.6). The ports'
   `StateStoreError::Corrupt` doc — "Stored state exists but does not
   decode into the domain types" — fits a damaged log row exactly, so both
   mapping sites (`complete_pending_settlements`, `export_jsonl`) now map
   `EventLogError::Damaged` → `Corrupt` preserving the damage detail
   ("the event log is damaged: …"); other log errors stay `ReadFailed`.
   Test `log_damage_surfaces_as_corrupt_through_the_store` FAILED pre-fix
   (ReadFailed) and passes. `SqliteEventLog::open`'s mapping to
   `AppendFailed` (frozen-enum hygiene) gained a one-sentence doc note; no
   behavior change.

6. **Offset-gap detection on replay** (B A4.5). `read_entries` tracks the
   next expected offset and returns `EventLogError::Damaged`
   ("expected offset N but found M — rows were removed out of band; an
   append-only log has no holes") on a hole. Contiguity is checked from
   the FIRST RETURNED row forward: `read_from(3)` over a log missing
   offset 2 stays green. Test deletes a middle row via raw connection:
   FAILED pre-fix (silent gap) and now Damaged naming the hole, with the
   pre-tamper contiguous replay and the past-gap tail both green.

7. **record_run checks the full identity** (B A1.10). The PK-conflict path
   now selects ALL identity columns and compares each: created_at,
   reference_id/version/digest, intent, both fingerprints, parent_run, and
   the budget field-by-field (decoded; an undecodable stored budget is
   `Corrupt`). Full agreement = idempotent no-op; any disagreement = typed
   `WriteFailed` naming every differing field with both values, closing
   with the frozen-reference invariant (the existing moved-digest test's
   phrase survives). Test: same digest, different intent → refusal naming
   intent and both values — FAILED pre-fix (silent no-op) and passes.

8. **Evidence get() re-verifies content against its address** (B A1.8).
   `get` hashes the bytes it read and compares to the address; mismatch →
   `EvidenceStoreError::ReadFailed` "content at sha256:X does not hash to
   its address — tampered or corrupted (it hashes to sha256:Y)". Cost note
   in the doc comment: one SHA-256 pass per read; evidence reads are not a
   hot path. Test overwrites the object out of band: FAILED pre-fix
   (tampered bytes served as Ok) and is refused now.

9. **Object paths must be regular files** (B A1.4, partial). On the
   write-verify entry (`verify_untouched`) and on `get`, an existing
   object path that is not a regular file (checked with
   `symlink_metadata`, which does not follow) is refused with a typed
   error naming the path kind ("a symbolic link" / "a directory" / "not a
   regular file"). Test creates a symlink at an object path pointing at a
   file with the SAME bytes — the case the old code silently accepted
   through the link: put FAILED pre-fix (Ok through the link) and both
   put/get refuse now. Residual (accepted, per adjudication):
   fanout-DIRECTORY symlink containment is not added — §17.5's placement
   of the store outside every builder-writable path is the primary guard;
   this is recorded defense-in-depth scope, not a claim of full path
   containment.

10. **Kind sidecar: unique temp + conflict refusal** (B A1.6). The sidecar
    temp name is pid/seq-unique (`.incoming-kind-<pid>-<seq>`, matching
    the objects' scheme), and an existing sidecar is read and compared:
    same kind = idempotent no-op; DIFFERENT kind = typed `WriteFailed`
    naming both wire tokens — a contradicting provenance assertion is
    loud, never silently discarded. The rename race-loser branch also
    compares. Test: same bytes, kind A then kind B → typed error naming
    both — FAILED pre-fix (second kind silently discarded) and passes;
    same-kind no-op green.

11. **export_run type-checks `at` fields** (B A2.2 residual). Event
    records' `at` and generic records' `at` now route through
    `uint_field` (shape validated even though the value is not an ordering
    key): `"at":"soon"` → `ExportError::FieldType { field: "at" }`.
    Test FAILED pre-fix (silent passthrough) and passes.

12. **Export stack composition test** (A MINOR-6 + B A6.2).
    `crates/gauntlet-cli/tests/export_stack.rs` (tests dir created — the
    crate had none): a real `EmbeddedStateStore` on a temp file seeded
    with a recorded run, two events, two checkpoints written OUT of
    iteration order, a settlement via `commit_settlement`, and one generic
    record; `gauntlet_app::state::export_run` run twice → byte-identical,
    canonical tag order, checkpoints by iteration (1 before 2, written
    2-then-1), settlement naming offset 2, and the canonical bytes equal
    the adapter's own export for this history (the seam pin between the
    two hand-maintained shape lists). Coverage addition — passed on first
    run; there was no mechanism to fix. `gauntlet-app::state::export_run`
    was ALREADY public (`pub mod state` → `pub mod export` → `pub use` in
    lib.rs): no visibility widening was needed.

13. **Trivia.** crash_helper duplicate println removed; crash_recovery.rs
    duplicated settlements-is-empty assertion removed (one kept);
    contract_suite.rs factory comment now states the truth: the scratch is
    deliberately leaked via `mem::forget` because the store must outlive
    the guard; bounded, one per suite run.

## Decisions

- **Corrupt fits (item 5).** `StateStoreError::Corrupt`'s doc — "Stored
  state exists but does not decode into the domain types" — covers a
  damaged event-log row (it is stored state failing to decode into
  `RunEvent`), so the remap was made as expected; detail text preserved.
- **Capability-named adapter exports (item 12 collateral).** The
  adjudication expected the boundary suite to accept CLI naming adapters.
  It accepts the DEPENDENCY (`deps.rs`: cli's internal set extends with
  every adapter member, dev-deps counted) but the denylist independently
  refuses vendor identifiers in core-crate sources: it masks the adapter
  CRATE NAME only, so `SqliteStateStore` cannot be spelled in gauntlet-cli
  (a bare vendor substring remains after masking). Resolved exactly as the
  denylist's own documentation prescribes ("an adapter must export
  capability-named items", written in Task 2 against Task 20): the state
  adapter now also exports `EmbeddedStateStore` / `EmbeddedEventLog`
  (capability names for the composition root; the vendor-named paths
  remain canonical inside the adapter layer, where vendor names belong per
  AGENTS.md Naming). Recorded here because it is a small public-API
  addition to the Task 6 artifact beyond the literal correction list.
- **Pre-existing collision test adapted (item 8 consequence).** The old
  test wrote bytes at a hand-chosen address that is not their hash, then
  read them back. Demonstration of why it had to change: `get` now
  re-verifies content against the address BY DESIGN (the correction), so
  content whose address is not its hash is unreadable by construction —
  the adapter can only produce matching pairs via `put`. The test now
  drives the collision at a REAL content address (put, then offer
  different bytes at that digest): the assertions are equally strong
  (typed refusal naming the digest; the original still readable and
  intact).
- **Step-2 failure injection (item 4).** A trigger with a missing-table
  body produces a hard SQL error that `INSERT OR IGNORE` cannot swallow
  (constraint-style `RAISE(ABORT)` would be ignored); chosen for
  determinism, documented in the test. The injection is created and
  dropped by the test's raw connection only.
- **Dev-dependencies.** The state adapter declares `rusqlite` in
  `[dev-dependencies]` (same version/features as its normal dependency,
  already on the adapter external allowlist) for the tamper instrument's
  raw second connection; gauntlet-cli declares
  `gauntlet-adapter-state-sqlite` in `[dev-dependencies]` (composition
  root's designed freedom; the boundary suite accepts cli→adapter wiring
  and flags the reverse). No new external dependencies; no `unsafe`.
- **Literals discipline in the CLI test.** Epoch values hoisted to consts
  because core crates' literals scanner refuses a clock number on a line
  that also names the bound (spec §8.1: timeouts are configuration).
- **Fail-first ledger.** Tests that FAILED pre-fix and pass now: 11 in
  state-sqlite (items 1b-after, 2×4, 3×2, 4, 5, 6, 7) + 3 in evidence-fs
  (items 8, 9-symlink, 10) + 1 in gauntlet-app (item 11). Tests that are
  coverage additions and passed pre-fix by design (no mechanism change):
  item 1a (the primitive's behavior already existed — the finding was
  zero coverage, not wrong behavior), item 3's own-offset happy path,
  item 1b's `before` case (vacuous against script absence — noted above),
  item 9's directory-on-get (was already loud via EISDIR text; now pinned
  deliberately), and item 12.

## Gates (verbatim tails)

```
=== cargo nextest run -p gauntlet-adapter-state-sqlite ===
────────────
     Summary [   0.048s] 31 tests run: 31 passed, 0 skipped
=== cargo nextest run -p gauntlet-adapter-evidence-fs ===
────────────
     Summary [   0.010s] 8 tests run: 8 passed, 0 skipped
=== cargo nextest run -p gauntlet-app state:: ===
────────────
     Summary [   0.011s] 8 tests run: 8 passed, 62 skipped
=== cargo nextest run -p gauntlet-cli ===
────────────
     Summary [   0.008s] 2 tests run: 2 passed, 0 skipped
=== just check ===
    Checking gauntlet-cli v0.1.0 (/home/sprime01/projects/gauntlet/crates/gauntlet-cli)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.23s
(check status: 0)
=== just lint ===
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 0.48s
cargo fmt --check
(lint status: 0)
=== just deny ===
advisories ok, bans ok, licenses ok, sources ok
(deny status: 0)
=== just boundaries ===
────────────
     Summary [   3.813s] 190 tests run: 190 passed, 0 skipped
=== just ruler ===
./scripts/verify-ruler.sh
verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock (/home/sprime01/projects/gauntlet/.gauntlet/reference.lock)
=== just test ===
────────────
     Summary [   4.557s] 433 tests run: 433 passed, 0 skipped
```

## Anything-not-done

- **Intra-call commit ordering (item 1c).** A pure two-statement swap
  inside `commit_settlement` is not detectable by this round's tests; it
  requires mid-function abort injection and belongs to Task 13's
  `p7_recovery::commit_order` (AK-041's declared locator). Recorded
  deferral, not a claim of full order proof.
- **Fanout-directory symlink containment (item 9).** Not added, per the
  adjudication's "partial" scope: §17.5 placement (the evidence store
  outside every builder-writable path) is the primary guard; this remains
  an accepted defense-in-depth residual.
- **pin() object-existence check.** `pin` still uses `path.exists()`
  (follows symlinks) for its unknown-ref check; the adjudication named
  only the write-verify entry and `get`. Recorded here rather than widened
  silently (scope discipline: adjacent issue, recorded separately).
- **Non-zero budget_spend resume (critic A MAJOR-2).** Not in the
  adjudicated corrections; untouched. It needs a domain/port change
  (a `Spent` reconstruction path), recorded as a port gap in the module
  docs.
- **crash_helper's `settle` script** still exists alongside
  `settle-via-commit`; both are instruments (two-step edge control vs
  packaged call) and the crash tests still use `settle` for the mid-step
  abort points a single call cannot offer. Not merged deliberately.
