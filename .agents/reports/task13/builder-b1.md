# Task 13 / B1 builder result contract — ports settlement surface + earned admission + store disciplines

Status: COMPLETE. All seven brief items delivered; all gates green; `just ruler`
PASS before starting and after finishing. Fail-first evidence per item is in
`builder-b1-checkpoints.md` (kept current item-by-item) and summarized below.

## Items delivered

### Item 1 — `StateStore` port trait: settlement surface + earned admission

* `crates/gauntlet-ports/src/state_store.rs`
  * `SettlementCommit` moved here from the adapter (now `Debug, Clone,
    PartialEq, Eq`; the §16.3 payload verbatim). Its caller-supplied `at` is
    documented as the event CONSTRUCTION timestamp only.
  * New trait methods (write/replay defaults fail closed with `WriteFailed`,
    read defaults return empty):
    `commit_settlement(&AdmittedSettlement)` · `settlements(run_id)` ·
    `complete_pending_settlements(run_id, anchor, deadline)` ·
    `complete_pending_evidence_production(run_id, anchor, production)` ·
    `record_verification(&VerificationRecordDoc)` ·
    `verification_records(run_id)`.
  * The verification-record pair is the durable §5.3.6/§16.1-condition-1
    substrate the admission validates against (no writer existed before;
    B2's service composes it through the same port).
* `crates/gauntlet-ports/src/producer_authority.rs`
  * `SettlementCommitAuthority::new(Arc<dyn StateStore>)` +
    `admit(SettlementCommit, deadline) -> AdmittedSettlement`, following the
    Task-12 pattern exactly: validates against DURABLE state — (1) the run
    is durable; (2) a verification record with `Outcome::Pass` exists for
    the (run, unit) whose evidence ref IS the commit's (absent record, non-PASS
    outcome, foreign unit, and evidence disagreement each refuse); (3) the
    §16.3 key `(run_id, unit_id, verification_ref)` is not already a durable
    settlement row — then mints the unforgeable `AdmittedSettlement` (no
    public constructor outside the module; pinned by a structural test).
  * `SettlementAdmissionError` typed: `RunNotDurable` · `VerificationNotPass`
    (detail distinguishes the four refusal shapes) · `AlreadyCommitted`
    (names the existing row's offset) · `State`.
  * **Authority vs store division (documented in-code and here, as the brief
    required):** the AUTHORITY checks the durable preconditions above; the
    STORE cross-validates the row against the durable log — the claimed
    offset must name the settlement event that made it real, with every
    field AND the stored event timestamp agreeing (§5.3.6, Item 3). Both
    must hold; neither substitutes for the other. The authority
    deliberately does NOT check §16.1 condition 2's resolve-and-pin half
    (the evidence store is not in its hands — the app layer's
    `settlement_precondition` owns it) or conditions 3–5 (the settlement
    service's scope, B2). Ports cannot reach those; this mint is the
    store-side half of the gate, not the whole of §16.1.
* Adapter (`crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs`):
  inherent `commit_settlement` / `settlements` / `complete_pending_settlements`
  REMOVED (now trait impls); `apply_settlement_row` stays inherent as the
  test-facing §16.3-step-2-alone primitive (still offset- and, since Item 3,
  timestamp-cross-validated). The adapter's `SettlementCommit` re-export is
  removed; the type is imported from ports everywhere.
* Callers updated: `crash_helper settle-via-commit` now EARNS the admission
  (records the run + PASS document, mints, commits — the governed path);
  tamper/crash tests; cli `export_stack` earns; in-crate adapter tests.
* **Intended behavior change (evidenced):** the in-crate test's duplicate-
  commit leg changed from "second commit appends a second event, one row" to
  "second ADMISSION of the committed key is refused (`AlreadyCommitted`)" —
  the ungated duplicate-event append was a tolerated artifact of the raw
  path; the keyed-row idempotency teeth are preserved via a raw duplicate
  EVENT leaving the row at one.

Fail-first: tests written first; `just check-package gauntlet-ports` RED with
25 errors (`no method named 'commit_settlement'`, `cannot find type
'SettlementCommitAuthority'`, …), then GREEN: 94 ports tests (5 new authority
tests), 46 adapter tests, export_stack PASS, workspace `just check` PASS.

### Item 2 — replay anchored at the §18.3 checkpoint offset (A2 store leg)

`complete_pending_settlements` reads `read_entries(&conn, anchor)`: events
below the anchor (the checkpoint's own certification) are not re-scanned; the
entry at the anchor is re-verified idempotently (completion is keyed).
**Documented choice:** the anchor is an EXPLICIT trait argument (the brief's
"accept the anchor explicitly" option); B2 composes `resume` →
`checkpoint.event_log_offset` → this method; a run with no checkpoint replays
from 0 (the whole log — exactly what the Task-6 crash tests pass, so their
exactly-once assertions are untouched).

Fail-first (runtime RED): `anchored_replay_skips_events_below_the_anchor_and_
completes_the_rest` failed before the fix with `left: 2, right: 1` (the old
from-0 body completed the below-anchor orphan too); GREEN after, with all
crash-recovery exactly-once tests green.

### Item 3 — `settled_at` becomes event-derived (A3)

`verify_settlement_event_at` now SELECTs the stored `at`, decodes with it,
refuses a negative stored timestamp (tamper evidence), returns it, and —
checked LAST, after the missing-offset/foreign-kind/field-disagreement
refusals, so those stronger wordings stay authoritative — refuses a commit
whose constructed `at` disagrees with the stored event timestamp. The row
insert takes the RETURNED stored timestamp (never `commit.at`). The replay
path passes inherently (it hands over the log's own values).

Fail-first (runtime RED): `a_commit_timestamp_disagreeing_with_the_durable_
event_is_refused` failed before the fix with `got Ok(1)` (the caller's `at`
landed in the row); GREEN after, including a raw-SQL assertion that
`settled_at` == the event's stored timestamp.

### Item 4 — strict `settlements_of` offset decode (A3)

`u64::try_from(offset).unwrap_or(0)` → typed `Corrupt` naming the run, unit,
and stored value (mirrors the Task-12 N3 message shape).

Fail-first (runtime RED): `a_tampered_settlement_offset_is_corrupt_never_a_
silent_zero` (tamper_evidence.rs, raw-connection UPDATE pattern) failed with
`got 1 rows reading offset Some(0)`; GREEN after.

### Item 5 — evidence-production crash-window completion (A5 store leg)

**Documented deviation in shape (not in discipline):** the brief's signature
was `complete_pending_evidence_productions(run_id, anchor)`. The
`EvidenceProduced` event is deliberately minimal (evidence + locator; Task
11's design — the ROW is the authority for producer/iteration/content facts),
so a settlement-style row rebuild from the event alone would have to
fabricate four NOT NULL columns (`producer_name`, `producer_version`,
`iteration_no`, `content_digest`) — the exact Corrupt-refusing defect class
this store forbids. The primitive therefore takes the authority-MINTED
`AdmittedEvidenceProduction` for the missing facts:
`complete_pending_evidence_production(run_id, anchor, production, deadline)`.

* ports `EvidenceProductionAuthority::mint(...)` added — the governed-read +
  derived-digest half of `record`, split out WITHOUT writing (`record` =
  mint + store write; one validation path, no duplicated rules).
* Adapter impl: row exists → `Ok(None)` (already applied, adds zero); scan
  at/after the anchor for the run's orphan `EvidenceProduced` event for the
  minted ref; locator disagreement → typed `WriteFailed` (the orphan belongs
  to a different production); else the row completes BOUND TO THE ORPHAN
  EVENT'S OFFSET (nothing appended — the settlement-replay pattern) with
  `produced_at` from the event's stored timestamp (Item 3's discipline
  mirrored; the re-mint's clock is not a durable fact); no orphan →
  `Ok(None)` (nothing pending — record normally). Fails closed by default.

Fail-first: tests written first; compile RED `no method named 'mint' found`;
GREEN after: 46 adapter tests including
`a_half_applied_evidence_production_is_completed_exactly_once` (mint writes
nothing; event-derived `produced_at`; no second event; exactly once; locator
refusal) and `a_fully_applied_evidence_production_completes_nothing`.

### Item 6 — shared contract suite extension (R2/A2) — ADD only

Three new cases in `crates/gauntlet-ports/src/contract/state_store.rs`
(existing cases byte-untouched):

1. `settlements_are_earned_committed_and_read_back` — admission refuses
   without a PASS record / with disagreeing evidence; honest PASS admits;
   commit + exact read-back; committed key never re-admits. The
   commit-requires-admission gate is structural (the trait method takes the
   minted type — a hand-built `SettlementCommit` does not type-check).
2. `settlement_replay_is_keyed_idempotent_and_anchored` — anchored replay
   completes exactly the at-or-after-anchor window; second call adds zero;
   duplicate-key event never displaces the row.
3. `settlement_rows_name_durable_exported_events` — every `settlements()`
   row's offset appears among the run's exported event offsets (the
   read-side of offset cross-validation, adapter-agnostic; the write-side
   refusal of a foreign offset stays pinned at the adapter, where a
   caller-supplied offset still exists — documented in the case).

Green reference stub (`DurableStore`) extended to conform (verification
records, keyed settlement rows, anchored replay); its `export_jsonl` now
emits valid-JSON kind tags (previously `{:?}` output that only byte-identity
checks ever read). Teeth: `SettlementLaxStore` fakes the surface (commit
"succeeds" doing nothing; replay always claims zero) and
`state_store_settlement_lax_is_caught` asserts BOTH settlement cases catch
it. `state-sqlite` passes the extended suite
(`sqlite_state_store_passes_the_shared_contract_suite` green).

### Item 7 — export discipline

* Adapter pinned golden (`export_is_the_pinned_golden_and_byte_identical`)
  UNCHANGED and green — the settlements export segment never carried
  `settled_at`, so event-derived timestamps change no export bytes.
* **Intended golden change (evidenced, never silent):** cli `export_stack`'s
  pinned expectation grew 7 → 8 lines — the earned settlement path durably
  records the PASS verification document first, so the records segment gains
  the `verification_record` line after `observation` ((kind, seq) order).
  The app layer already accepted the kind (`GENERIC_KINDS`). Comments in the
  test name the Task-13 cause.
* App export tests: 9 passed; cli export_stack: 1 passed.

## Deviations from the brief

1. **Item 5 signature** — `complete_pending_evidence_production` (singular)
   takes the minted production as a third argument; rationale above
   (no-fabrication). Recorded in the checkpoints at STARTED B1.5.
2. **Item 1's "evidence ref is pinned" authority check** reads as durable-
   state agreement: the commit's evidence ref must be the evidence a durable
   PASS verification record names. The evidence-store resolve-and-pin half
   of §16.1 condition 2 stays in the app layer's `settlement_precondition`
   (an `Arc<dyn StateStore>`-only authority cannot consult the evidence
   store); the split is documented in the authority's docs.
3. **crash_helper / tamper / crash / export_stack tests updated for the
   earned-commit path** (signature + discipline change); all existing
   assertions preserved except the in-crate duplicate-commit leg, whose
   intended change is evidenced above. The Task-4 suite's existing cases
   were not modified; only cases were added.

## Gates (run personally; verbatim tails)

```text
$ just ruler
verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock (/home/sprime01/projects/gauntlet/.gauntlet/reference.lock)

$ just check-package gauntlet-ports
    Finished `dev` profile [unoptimized] target(s) in 7.35s

$ just test-package gauntlet-ports
     Summary [   0.429s] 94 tests run: 94 passed, 0 skipped

$ just check-package gauntlet-adapter-state-sqlite
    Finished `dev` profile [unoptimized] target(s) in 4.95s

$ just test-package gauntlet-adapter-state-sqlite
     Summary [   0.557s] 46 tests run: 46 passed, 0 skipped

$ just check-package gauntlet-app
    Finished `dev` profile [unoptimized] target(s) in 12.21s

$ just lint
    Finished `dev` profile [unoptimized] target(s) in 0.58s
cargo fmt --check
lint exit: 0
```

`just ruler` also PASSed BEFORE any edit (pre-start gate). No migration was
added (settlements schema unchanged, as the brief anticipated). Workspace
`just check` green. Forbidden paths untouched: `.wayfinder/`,
`.gauntlet/reference.lock`, `deny.toml`, nextest config, `AGENTS.md` (its
pre-existing working-tree modification predates this slice and was not
touched), `.jolli/`, `.tmp/`, `.agents/prompts/`, applied migrations
(zero migration-file changes).

## Files touched

* `crates/gauntlet-ports/src/state_store.rs` — trait surface + payload type.
* `crates/gauntlet-ports/src/producer_authority.rs` — settlement authority,
  minted type, error; `mint` split; module docs.
* `crates/gauntlet-ports/src/lib.rs` — exports.
* `crates/gauntlet-ports/src/contract/state_store.rs` — 3 new suite cases.
* `crates/gauntlet-ports/src/contract/stubs.rs` — green stub settlement
  surface; settlement-lax broken stub; export tag fix.
* `crates/gauntlet-ports/src/contract/suite_runs.rs` — settlement teeth test.
* `crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs` — trait
  impls (commit/settlements/anchored replay/evidence completion/
  verification records), event-derived `settled_at` + timestamp refusal,
  strict offset decode, tests.
* `crates/adapters/gauntlet-adapter-state-sqlite/src/lib.rs` — export list.
* `crates/adapters/gauntlet-adapter-state-sqlite/src/bin/crash_helper.rs` —
  earned `settle-via-commit`.
* `crates/adapters/gauntlet-adapter-state-sqlite/tests/crash_recovery.rs` —
  anchor argument (0 = whole log; assertions unchanged).
* `crates/adapters/gauntlet-adapter-state-sqlite/tests/tamper_evidence.rs` —
  earn fixture, anchor argument, new offset-tamper test.
* `crates/gauntlet-cli/tests/export_stack.rs` — earned commit; evidenced
  golden growth (verification_record line).

## Known gaps / adjacent findings (recorded, not fixed)

* The suite's offset-cross-validation case pins the read-side invariant
  adapter-agnostically; the write-side refusal remains adapter-pinned
  (tamper_evidence) because the trait's commit mints its own offset — a
  trait-reachable wrong-offset refusal does not exist by design.
* `complete_pending_evidence_production` has no suite case (the brief's
  Item 6 list names the settlement surface only); its teeth live in the
  adapter's in-crate tests.
* The ports `mint` addition widens `EvidenceProductionAuthority`'s public
  API by one method (needed for the completion path); no production caller
  of completion exists yet — B2 composes resume against it.
* The `run()` fixtures in several test files still use
  `ReferenceVersion::new` (no explicit frozen evidence ref); unrelated to
  this slice's changes and left as-is.
