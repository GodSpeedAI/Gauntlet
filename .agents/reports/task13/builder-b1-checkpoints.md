# Task 13 / B1 builder checkpoints

Format: STARTED/DONE per item, appended as the item progresses (never batched).
Fail-first evidence: command + observed RED then GREEN.

- 2026-08-25 B1 pre-start: `just ruler` → PASS
  (`verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock`).
- STARTED B1.1 — move the settlement write surface to the `StateStore` port
  trait behind an earned admission (`SettlementCommitAuthority`, following
  `producer_authority.rs`), plus the durable verification-record read/write the
  admission validates against.
- DONE B1.1 — settlement surface moved to the `StateStore` port trait behind
  an earned admission.
  Landed:
  * `crates/gauntlet-ports/src/state_store.rs`: `SettlementCommit` moved here
    from the adapter (Eq payload); trait methods `commit_settlement` (consumes
    `AdmittedSettlement`), `settlements`, `complete_pending_settlements(run_id,
    anchor, deadline)` (anchor param present; behavior still replay-from-0 —
    B1.2 lands the anchoring), `complete_pending_evidence_production` (default
    only; impl in B1.5), `record_verification` + `verification_records` (the
    durable §5.3.6/§16.1-condition-1 substrate the admission validates
    against). Write/replay defaults fail closed (Err), read defaults empty.
  * `crates/gauntlet-ports/src/producer_authority.rs`:
    `SettlementCommitAuthority::new(Arc<dyn StateStore>)` + `admit(...)`
    validating against DURABLE state — durable run; PASS verification record
    for the (run, unit) whose evidence ref IS the commit's; §16.3 key not
    already committed — minting `AdmittedSettlement` (no public constructor).
    `SettlementAdmissionError` typed (RunNotDurable/VerificationNotPass/
    AlreadyCommitted/State). Authority-vs-store division documented in the
    trait + authority docs (authority: durable preconditions; store: offset↔
    event cross-validation incl. the stored timestamp, B1.3).
  * Adapter: inherent `commit_settlement`/`settlements`/
    `complete_pending_settlements` REMOVED (now trait impls);
    `apply_settlement_row` stays inherent (test-facing §16.3-step-2 primitive,
    still offset-cross-validated). Adapter `SettlementCommit` re-export
    removed; callers updated (crash_helper `settle-via-commit` now EARNS the
    admission; tamper/crash tests; cli export_stack earns).
  Fail-first: tests written first → `just check-package gauntlet-ports`
  RED: `error[E0599]: no method named 'commit_settlement' ... error: could
  not compile gauntlet-ports (lib test) due to 25 previous errors`; then
  GREEN: `just test-package gauntlet-ports` → 93 passed (incl. new
  a_settlement_admission_requires_a_durable_run,
  a_settlement_admission_requires_a_durable_pass_record_for_the_unit,
  an_already_committed_settlement_refuses_re_admission,
  an_earned_admission_commits_through_the_store_and_reads_back,
  the_minted_settlement_admission_has_no_public_constructor);
  `just test-package gauntlet-adapter-state-sqlite` → 41 passed (crash_recovery
  + tamper_evidence + reworked commit_settlement_is_the_spec_sequence_and_
  idempotent_by_key, whose duplicate-commit leg is now the authority refusal +
  a raw duplicate EVENT leaving the keyed row at one — intended behavior
  change: re-admission of a committed key is refused instead of appending a
  duplicate event). cli export_stack PASS. `just check` (workspace) PASS.
- STARTED B1.2 — `complete_pending_settlements` replays from the §18.3
  checkpoint anchor (events BELOW the anchor are not re-scanned), with a
  fail-first test that is RED under the current replay-from-0 body.
- DONE B1.2 — `complete_pending_settlements` replays from the §18.3 anchor.
  Landed: the trait impl reads `read_entries(&conn, anchor)` (events below
  the anchor — the checkpoint's own certification — are not re-scanned; the
  entry at the anchor is re-verified idempotently because completion is
  keyed). Documented choice: the anchor is an EXPLICIT trait argument (the
  brief's "accept the anchor explicitly" option); B2 composes
  `resume` → `checkpoint.event_log_offset` → here; a run with no checkpoint
  replays from 0 (the whole log — exactly what the Task-6 crash tests pass,
  so their exactly-once assertions are untouched).
  Fail-first: new test `anchored_replay_skips_events_below_the_anchor_and_
  completes_the_rest` run BEFORE the fix:
  `cargo nextest run -p gauntlet-adapter-state-sqlite anchored_replay`
  RED: "assertion `left == right` failed: exactly the post-anchor
  half-applied settlement is completed; the below-anchor event is not
  re-scanned — left: 2, right: 1" (the old from-0 body completed the
  below-anchor orphan too). After the fix: `just test-package
  gauntlet-adapter-state-sqlite` → 42 passed (crash_recovery + tamper
  exactly-once suites green).
- STARTED B1.3 — `settled_at` becomes event-derived: the row takes the
  DURABLE event's stored timestamp and a commit whose constructed timestamp
  disagrees with the stored event is refused (discrimination test RED first).
- DONE B1.3 — `settled_at` is event-derived; the unchecked timestamp seam is
  closed.
  Landed: `verify_settlement_event_at` now SELECTs the stored `at`, decodes
  the event with it, refuses a negative stored timestamp (tamper evidence,
  adapter-never-writes-one), returns the stored timestamp, and — checked
  LAST, after the missing-offset/foreign-kind/field-disagreement refusals so
  those stronger wordings stay authoritative — refuses a commit whose
  constructed `at` disagrees with the stored event timestamp. The row insert
  takes the RETURNED stored timestamp as `settled_at` (never `commit.at`).
  Fail-first: `a_commit_timestamp_disagreeing_with_the_durable_event_is_
  refused` run BEFORE the fix → RED: "a commit timestamp that disagrees
  with the durable event must be refused, got Ok(1)" (the old code landed
  the row with the caller's `at`). After: `just test-package
  gauntlet-adapter-state-sqlite` → 43 passed (incl. the agreeing-commit leg
  asserting via raw SQL that `settled_at` == the event's stored timestamp).
- STARTED B1.4 — strict `settlements_of` offset decode: a tampered negative
  offset is typed `Corrupt`, never `unwrap_or(0)` (mirror the Task-12 N3
  checkpoint discipline); fail-first with a tampered row.
- DONE B1.4 — strict `settlements_of` offset decode.
  Landed: `u64::try_from(offset).unwrap_or(0)` → typed `Corrupt` naming the
  run, unit, and stored value (message mirrors the Task-12 N3 checkpoint
  shape: "holds a non-representable event-log offset (-1)").
  Fail-first: `a_tampered_settlement_offset_is_corrupt_never_a_silent_zero`
  (tamper_evidence.rs, raw-connection UPDATE pattern) run BEFORE the fix →
  RED: "a non-representable stored settlement offset is corrupt, never a
  silent zero (got 1 rows reading offset Some(0))". After: `just
  test-package gauntlet-adapter-state-sqlite` → 44 passed.
- STARTED B1.5 — `complete_pending_evidence_production` store primitive for
  the `EvidenceProduced` crash window. DESIGN NOTE (documented deviation in
  shape, not in discipline): the `EvidenceProduced` event is deliberately
  minimal (evidence + locator; Task 11's design — the ROW is the authority
  for producer/iteration/content facts), so a settlement-style row rebuild
  from the event alone would have to fabricate four NOT NULL columns — the
  exact Corrupt-refusing defect class this store forbids. The primitive
  therefore takes the authority-MINTED `AdmittedEvidenceProduction` for the
  missing facts, scans events at/after the anchor for the orphan, and
  completes the row BOUND TO THE ORPHAN EVENT'S OFFSET with produced_at
  from the event's stored timestamp (the A3 discipline mirrored). No new
  event is appended; no fact is fabricated.
- DONE B1.5 — `EvidenceProduced` crash-window completion.
  Landed:
  * ports `EvidenceProductionAuthority::mint(...)` — the governed-read +
    derived-digest half of `record`, split out WITHOUT writing (record =
    mint + store write; one validation path). Needed because the minimal
    event cannot rebuild the row's producer/iteration/content facts.
  * adapter `complete_pending_evidence_production(run_id, anchor,
    production, deadline)`: row-exists → Ok(None) (already applied adds
    zero); scan at/after the anchor for the run's orphan EvidenceProduced
    event for the minted ref; locator disagreement → typed WriteFailed
    refusal; else the row is completed bound to the ORPHAN event's offset
    (nothing appended) with produced_at = the event's stored timestamp
    (A3 mirrored); no orphan → Ok(None) (nothing pending — record normally).
  Fail-first: tests written first → `cargo check -p
  gauntlet-adapter-state-sqlite --all-targets` RED: "error[E0599]: no method
  named `mint` found for struct `EvidenceProductionAuthority`" (the
  completion method itself resolved only to the fail-closed trait default).
  After: `just test-package gauntlet-adapter-state-sqlite` → 46 passed
  (a_half_applied_evidence_production_is_completed_exactly_once — incl.
  mint-writes-nothing, event-derived produced_at, no-second-event, exactly
  once, locator refusal; a_fully_applied_evidence_production_completes_
  nothing); `just test-package gauntlet-ports` → 93 passed.
- STARTED B1.6 — Task-4 shared contract suite extension (ADD-only): earned
  commit + read-back + admission refusals; keyed idempotent + anchored
  replay; settlement rows name durable exported events; green reference
  stub extended to conform; a settlement-faking broken stub must be caught.
- DONE B1.6 — shared contract suite extension (ADD-only; existing cases
  untouched).
  Landed (crates/gauntlet-ports/src/contract/state_store.rs):
  * `settlements_are_earned_committed_and_read_back` — admission refuses
    without a PASS record / with disagreeing evidence; honest PASS admits;
    commit + exact read-back; committed key never re-admits. (The
    commit-requires-admission gate is structural — the trait method takes
    the minted type; documented in the case doc.)
  * `settlement_replay_is_keyed_idempotent_and_anchored` — anchored replay
    completes exactly the at-or-after-anchor window (below-anchor not
    re-scanned), second call adds zero, duplicate-key event never displaces
    the row.
  * `settlement_rows_name_durable_exported_events` — every settlements()
    row's offset appears among the run's exported event offsets (the
    read-side of offset cross-validation; the write-side refusal of a
    foreign offset stays pinned at the adapter, where a caller-supplied
    offset still exists — documented in the case doc).
  * Green reference stub (DurableStore) extended to conform:
    verifications, keyed settlement rows, anchored replay; its export_jsonl
    now emits valid-JSON kind tags (previously `{:?}`, unparseable — only
    ever checked for byte-identity before).
  * Teeth: `SettlementLaxStore` (commit fakes success without appending or
    recording; replay always claims zero) + `state_store_settlement_lax_
  is_caught` asserting BOTH settlement cases catch it.
  Evidence: `just test-package gauntlet-ports` → 94 passed (green clean,
  both teeth green); `just test-package gauntlet-adapter-state-sqlite` →
  46 passed (sqlite_state_store_passes_the_shared_contract_suite green
  over the EXTENDED suite).
- STARTED B1.7 — export discipline: verify the settlements segment
  ordering/pinning still holds now that rows carry event-derived
  `settled_at`; evidence any intended golden change explicitly.
- DONE B1.7 — export discipline verified.
  * The adapter's pinned golden (export_is_the_pinned_golden_and_byte_
    identical) is UNCHANGED and green: the settlements export segment never
    carried `settled_at`, so event-derived timestamps change no export
    bytes; its fixture earns no verification records.
  * INTENDED golden change (evidenced, never silent): cli export_stack's
    pinned expectation grew 7 → 8 lines — the earned settlement path now
    durably records the PASS verification document first, so the records
    segment gains the `verification_record` line after `observation`
    ((kind, seq) order). The test's tags/count assertions were updated with
    comments naming the Task-13 cause; the app layer already accepted the
    kind (GENERIC_KINDS).
  * App export tests: `cargo nextest run -p gauntlet-app export` → 9 passed;
    `cargo nextest run -p gauntlet-cli --test export_stack` → 1 passed.
- 2026-08-25 B1 FINAL GATES (all green; verbatim tails in builder-b1.md):
  `just ruler` → PASS (pre-start and post-finish);
  `just check-package gauntlet-ports` → Finished;
  `just test-package gauntlet-ports` → 94 passed;
  `just check-package gauntlet-adapter-state-sqlite` → Finished;
  `just test-package gauntlet-adapter-state-sqlite` → 46 passed;
  `just check-package gauntlet-app` → Finished;
  `just lint` → exit 0 (clippy -D warnings + fmt --check).
  Result contract written: .agents/reports/task13/builder-b1.md.
  B1 COMPLETE.
