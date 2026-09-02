# Task 20 — COR-R3 correction report (falsifier round 3 → corrections)

Builder: COR-R3. Work order: `falsifier-adjudication-r3.md` (authoritative).
Inputs: `falsifier-round3.md` (executed attacks, reproductions, code-confirmed
loci), `charter.md` (+ operational addenda), `cor-r2-report.md` (the
key-surface unification being extended to the mint half), AGENTS.md.
Checkpoint trail: `cor-r3-progress.md`.

VERDICT (builder claim, no settlement authority): all three adjudicated
corrections implemented and pinned. F-R3-1's exact reproductions are shipped
regression teeth with their honest outcomes (settled terminal + per-locator
forensics through the real binary); F-R3-2's swapped-body and stale-source
shapes refuse typed through the real binary; M-R3-1's concurrent double-resume
attack is a shipped E2E leg bounded by the new single-writer guard. The
settled no-op pins, p1–p12/g6/g7/g10 proofs, and the shipped E2E legs are
green UNMODIFIED.

---

## COR-R3-1 (MAJOR) — the recovery's MINT half is unified with the produce
step's key surface per world

### Mechanism

`SettlementService::complete_pending_after_checkpoint` now dispatches on the
run's own `ProducePolicy` (the COR-R2-1 seam) into two world-shaped walks:

- **Default world (`MaterializeObservation`)** — the pre-correction facet
  walk, byte-identical in decisions: the produce step records the facet
  document's own ref, so the facet ref IS the mint ref, per facet (the
  applied read is ref-keyed, so per-facet granularity is exact and no
  masking is possible). Only the fallback limitation TEXT changed, to name
  the world, the facet ref, and the anchor-bounded search — the old text
  presupposed an orphan event ("no frozen locator agrees with the orphan
  EvidenceProduced event for {ref}") and asserted "not durably recoverable"
  (a byte-presence claim the recovery never made).
- **Produced world (`BindLocatorContent`)** — a NEW locator-driven walk. The
  produce step records the LOCATOR ARTIFACT's own content ref — a ref no
  facet names — so the facet-shaped mint could never match a durable orphan
  event and the honest A5 crash window (event appended, row not yet
  inserted) ended in false "unrecoverable" pins. The walk consults the
  durable `EvidenceProduced` events at/after the anchor per locator through
  a new StateStore read (`pending_evidence_production_events`) and mints at
  the EVENT'S OWN ref, with the producer identity derived by the same rule
  the produce step applies (the bound bundle's FIRST facet's instrument),
  the durable anchor's iteration, and the event's own timestamp. The store's
  `complete_pending_evidence_production` cross-validates the mint against
  the same events before any row lands — no fact is fabricated.

The port read (`pending_evidence_production_events`) is a defaulted method
whose default FAILS CLOSED (`ReadFailed`), per the COR-7 discipline for
settlement-surface reads. The event is deliberately minimal; the returned
facts are durable events only, and the returned type
(`OrphanEvidenceProduction { evidence, offset, at }`) carries no
producer/iteration/content fact that could be fabricated.

### Honest forensics (adjudication b) and per-locator granularity (c)

- A unit is only pinned as damaged when the damage is PROVABLE: the unit is
  durably settled (a settled unit's §14.3 chain requires its production row)
  or a pending orphan event exists for its locator. An OPEN unit's missing
  row is normal mid-run state and records nothing (over-rejection pin:
  `an_open_units_missing_row_is_never_pinned_as_damage`).
- Every limitation names the world ("produced world:" / "default world:"),
  the locator or ref, what is actually missing, and the anchor-bounded fact
  ("no EvidenceProduced event at or after the resume anchor (offset N)
  names this locator") — never a global claim, never a byte-presence claim.
- Granularity is per-unit: one intact locator's row can never mask a
  sibling's missing row (the falsifier's one-row case), and the intact
  locator is never mentioned.

### The dishonest `budget_exhausted` terminal (adjudication d)

The controller's settled no-op predicate (COR-F6) drops its
`completion.limitations.is_empty()` clause: forensic pins land in the
EVIDENCE store and are durable by themselves — no STATE moved that a
reconciliation checkpoint would need to record, and re-entering the loop
from an all-settled fact set can only burn empty iterations into a dishonest
terminal. All other clauses are unchanged (settlement/production completions
still force the full path — durable state moved; in-flight still forces it;
the all-units-Settled clause still gates everything). The shipped
settled-resume no-op legs are unaffected (healthy runs pin nothing).

### Tests (the falsifier's reproductions, shipped with honest outcomes)

- E2E `the_resume_of_a_settled_run_with_all_production_rows_deleted_is_honest`
  (`crates/gauntlet-cli/tests/e2e_full_loop_mock.rs`): the falsifier's exact
  reproduction through the real binary — happy run settles 3/3, all
  `evidence_productions` rows deleted out-of-band (every durable event and
  evidence byte left in place), first `gauntlet resume` exits 0 with
  `terminal=settled iterations=3`, zero `budget_exhausted` anywhere, exactly
  THREE forensic records (one per damaged locator, each naming world +
  locator + missing row + anchor-bounded search, none claiming bytes
  absent), and the durable event log byte-identical.
- E2E `the_resume_of_a_settled_run_names_exactly_the_damaged_locator`: the
  one-row case — exactly ONE forensic record, naming exactly the damaged
  locator; the intact locators never masked, never mentioned.
- E2E `the_single_writer_guard_bounds_the_concurrent_double_resume_attack`
  (also COR-R3-3's leg, see below).
- Settle `the_produced_world_crash_window_completes_from_the_orphan_events_ref`:
  the A5 window self-completes in the PRODUCED world — orphan event naming
  the artifact ref above the anchor, row missing → completed, bound to the
  event's own offset, `produced_at` event-derived, content digest derived
  from the store's bytes; no limitation; idempotent second call.
- Settle `damaged_produced_world_rows_are_honest_per_locator_forensics`:
  settled checkpoint anchor ABOVE the production events + two rows deleted →
  nothing rewritten by the replay, exactly two honest records, the intact
  locator unnamed, the class token `evidence_production_orphan_unrecoverable`
  pinned.
- Settle `a_produced_world_orphan_whose_bytes_are_absent_is_an_honest_limitation`:
  the honest "does not resolve in the evidence store" record names the ref
  and offset; nothing fabricated.
- Settle `an_open_units_missing_row_is_never_pinned_as_damage`: over-rejection
  leg (healthy mid-run state is silence).
- Controller
  `damaged_production_bookkeeping_on_a_settled_produced_world_run_resumes_settled`:
  the composed resume reports terminal=settled at the checkpoint's own
  iteration, zero re-dispatch, the durable settlement unduplicated, the
  honest forensic record pinned, and NO `budget_exhausted` anywhere.

### Mechanical adaptations (listed with reasons; no assertion changed)

- `crates/gauntlet-ports/src/state_store.rs`: new `OrphanEvidenceProduction`
  type + new defaulted port method (fail-closed default).
- `crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs`:
  implements the orphan-event read (the same replay scan
  `complete_pending_evidence_production` performs, surfaced as facts).
- `crates/adapters/gauntlet-adapter-state-sqlite/src/lib.rs`: doc-hidden
  `test_support::delete_evidence_production_rows` — the falsifier's
  out-of-band row-deletion surgery for the E2E legs, living beside
  `crash_helper` where the engine's name belongs (a first cut added rusqlite
  as a gauntlet-cli dev-dependency; the denylist + dependency-direction
  gates rightly refused the vendor channel in the composition root, so the
  surgery moved into the adapter and the dependency was removed — gates NOT
  weakened, the honest route taken).
- `crates/gauntlet-app/src/settle/mod.rs`: fixture `SettleState` implements
  the orphan-event read; `checkpoint_at` refactored into a
  `checkpoint_at_offset` wrapper (identical behavior at offset 0); damage
  helper `drop_production_at` and settlement-row helper for the damage
  shapes.
- `crates/gauntlet-app/src/control/controller.rs`: `FakeState` implements
  `complete_pending_evidence_production` + `pending_evidence_production_events`
  (positional-offset convention, same as its settlement replay) and gains
  the `drop_production_at` damage helper. No existing produced-world resume
  path reached either method before (they were never called on the all-settled
  healthy shapes), so no existing behavior changes; the fail-closed default
  would have refused the new tests, hence the implementations.

---

## COR-R3-2 (MAJOR) — skills verify content-verifies against the canonical
repo sources, the way roles verify does

- `crates/gauntlet-cli/src/skills_cmd.rs` `verify`: for every locked skill,
  the installed-tree digest/lint checks are preceded by a SOURCE-anchor
  check against `.agents/skills/<name>/SKILL.md` (the repo's reviewed
  copy): a missing source is typed drift; a source whose digest disagrees
  with the lock is typed drift naming the skill and BOTH digests ("SOURCE
  DRIFTED from the lock — lock {digest}, source {actual} — the canonical
  .agents/skills body is the anchor; re-install to repair"). This is the
  CONTENT half the strict reader cannot see: a shape-valid, internally
  consistent lock over tampered installed bodies refuses HERE (the falsifier's
  swapped-body attack), and a source edited after install refuses the same
  way (the roles surface's "source file modified since install" class).
- Re-install repairs from the canonical source (unchanged semantics).
- NEW round-trip test
  `a_self_consistent_lock_over_tampered_bodies_is_drift_against_the_canonical_source`
  (`crates/gauntlet-cli/tests/skills_roundtrip.rs`): the swapped body is
  deliberately lint-clean so the ANCHOR is provably the refusing check; the
  forged lock names both digests in the refusal; re-install repairs; the
  stale-source shape refuses; restoring the source restores the clean
  verify. All four shipped round-trip tests stay UNMODIFIED and green.
- Duplicate-entry and every strict shape from COR-R2-3 still refuse (the
  reader is untouched).

---

## COR-R3-3 (MINOR) — state-dir single-writer guard on run/resume

- NEW `crates/gauntlet-cli/src/writer_lock.rs`: `WriterLock::acquire` takes
  the state dir's writer role via an `O_EXCL` (`create_new(true)`) lock file
  `<state_dir>/writer.lock` recording the holder's pid; a second acquire
  refuses typed, naming the holder pid read from the file and the manual
  stale-lock recovery ("if the holder pid is dead, remove the lock file
  manually and retry … never guesses liveness and never takes over
  automatically"). Release is Drop-bound: the file is removed on every exit
  path of the holding command; a killed holder leaves the file for the
  operator. Fail-closed: no liveness probing anywhere, no takeover.
- Wired into BOTH composed paths (`gauntlet run` non-dry-run and
  `gauntlet resume`) after the doctor gate, before composition; the dry-run
  stays read-only and takes no lock (charter D1 zero-write preserved —
  pinned by the shipped dry-run leg and leg 6's whole-scratch hash diff,
  both green).
- In-scope unit tests: exclusivity + pid recording + drop-release;
  stale-lock refusal naming the pid + manual recovery + no-takeover
  posture + the lock file left untouched.
- NEW E2E leg
  `the_single_writer_guard_bounds_the_concurrent_double_resume_attack`
  (the falsifier's M-R3-1 attack through the real binary): (a) a held lock
  refuses a second driver typed, naming the holder pid and the manual
  recovery; (b) the manual recovery path resumes the clean settled no-op;
  (c) TWO concurrent resume drivers race the same run — every exit is
  either an honest settled no-op or the typed refusal, at least one driver
  is admitted (the O_EXCL create decides; no deadlock, no double refusal),
  and the export stream + whole scratch are byte-identical to before the
  attack (no double dispatch, no corruption, no leftover locks).

---

## Files touched

- `crates/gauntlet-ports/src/state_store.rs` — `OrphanEvidenceProduction`,
  `pending_evidence_production_events` (fail-closed default), docs.
- `crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs` — the
  orphan-event read implementation.
- `crates/adapters/gauntlet-adapter-state-sqlite/src/lib.rs` — doc-hidden
  `test_support` damage injector (E2E row-deletion surgery).
- `crates/gauntlet-app/src/settle/mod.rs` — per-world recovery arms, module
  doc, honest fallback texts, fixture helpers, 4 new tests.
- `crates/gauntlet-app/src/control/controller.rs` — settled no-op predicate
  (limitations clause dropped, comment rewritten), `production_recorded` doc
  (mint-half note), `FakeState` impls + damage helper, 1 new test.
- `crates/gauntlet-cli/src/skills_cmd.rs` — source-anchor verify + module doc.
- `crates/gauntlet-cli/src/writer_lock.rs` — NEW (guard + unit tests).
- `crates/gauntlet-cli/src/lib.rs` — module export.
- `crates/gauntlet-cli/src/run_cmd.rs` — writer-lock wiring + module doc.
- `crates/gauntlet-cli/tests/skills_roundtrip.rs` — NEW content-anchor test.
- `crates/gauntlet-cli/tests/e2e_full_loop_mock.rs` — `records_containing`
  reader, `fixture_locators` derivation (AK-016), 3 new E2E legs.
- `crates/gauntlet-arch-tests/src/single_writer.rs` — three inventoried
  write sites for writer_lock.rs (reason + guard each), the pin test and
  clean fixture extended (the gate's own documented re-litigation flow).

## Assertion changes, with reasons

- The settled no-op predicate dropped exactly one conjunct
  (`completion.limitations.is_empty()`), per adjudication (d); every other
  conjunct stands. The pinned no-op tests are unaffected (healthy runs pin
  nothing) and stayed green.
- The default-world fallback limitation text was rewritten (honesty fix,
  adjudication b); the pinned tests assert the pin CLASS and counts, not
  the old text, and stayed green.
- No assertion was weakened; no shipped proof, E2E leg, or gate was disabled.
  The gauntlet-arch-tests inventory/pin/fixture edits EXTEND the gates per
  their own documented flows ("add the site to WRITE_API_INVENTORY with its
  reason and guard"; "growth of any inventory is a deliberate, recorded
  edit that updates these assertions").

## Battery (evidence = what the runs printed; logs in .agents/reports/task20/)

| Gate | Result | Log |
|---|---|---|
| `just ruler` (before edits) | PASS — digests match the frozen lock | session log |
| `just check` | PASS (exit 0) | corr3-bat-check.log |
| `just lint` | PASS (exit 0; one needless-borrow clippy fix and two fmt passes on new code — formatting only) | corr3-bat-lint5.log |
| `just test` | PASS — **1365 tests run: 1365 passed, 0 skipped** | corr3-bat-test6.log |
| `just boundaries` | PASS (exit 0) | corr3-bat-boundaries5.log |
| `just deny` | PASS (exit 0) — "advisories ok, bans ok, licenses ok, sources ok" | corr3-bat-deny.log |
| `just doctor` | PASS (exit 0) | corr3-bat-doctor.log |
| `just ruler` (after edits) | PASS (exit 0) | corr3-bat-ruler2.log |
| Package tests | ports 112/112, sqlite 50/50, app 471/471, cli 90/90 | corr3-{ports,sqlite,app,cli}-test.log |
| `cargo nextest run -p gauntlet-cli --test e2e_full_loop_mock --test cli_surface` | **13/13** (incl. the 3 new legs + the 4 cli_surface pins) | corr3-bat-e2e2.log |
| repo-root dry-run | exit 0 — "dry-run — preflight clean, zero writes ahead", "lock verified (70 unit(s) frozen)"; before/after `git status --porcelain` diff EMPTY (0 bytes) | corr3-bat-dryrun.log |

Over-rejection leg clean: honest builders settle in both produce worlds —
the produced world through the shipped E2E legs (happy, corroborated pin,
parallel carry, settled-resume no-op — all UNMODIFIED and green) and the
default world through the in-app proofs (p1–p12/g6/g7/g10, all inside the
1365). The new `an_open_units_missing_row_is_never_pinned_as_damage` leg
pins that the recovery does not pin healthy mid-run state as damage.

## Deviations

- None from the work order's mechanics. Three implementation notes for the
  record:
  1. The E2E damage injection first shipped as a rusqlite dev-dependency of
     gauntlet-cli; the denylist and dependency-direction gates refused the
     vendor channel in the composition root (as they should). The surgery
     moved into the state adapter's doc-hidden `test_support` module (the
     `crash_helper` precedent) and the dependency was removed. Gates were
     never relaxed; the full battery re-ran green after the move.
  2. The scope-escape scanner's conservative attribute matcher reads
     `let path = <runtime expr>` inside a scanned unit as a `#[path = …]`
     key (a false positive on the writer lock's locals); the locals were
     renamed `lock_path` — adapting new code to the scanner's lexing, not
     touching the gate.
  3. The known load-flaky live-process attack test
     (`b5_foreign_identity_is_contract_mismatch`, gauntlet-adapter-agent-cli)
     failed once under full-battery load and passed in isolation and in both
     subsequent full runs (1365/1365). Charter-known class; noted, not
     "fixed".
