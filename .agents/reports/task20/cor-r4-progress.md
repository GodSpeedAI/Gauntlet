# COR-R4 progress checkpoint (declare-first)

Builder: COR-R4. Work order: falsifier-adjudication-r4.md.
Protocol: STARTED before each item, DONE after. A lost final message must
never lose landed work.

- STARTED 2026-08-31: recon — read controller.rs (settled no-op predicate
  ~1197-1212, booking 1489-1499, decide_progress, FakeState), skills_cmd.rs
  (verify digests via normalize::digest_of), writer_lock.rs (Held refusal),
  skills_roundtrip.rs, e2e_full_loop_mock.rs legs 7-8, single_writer.rs arch
  gate (writer_lock inventory: 3 sites, guards `fn acquire` × 2 +
  `impl Drop for WriterLock`), sqlite test_support, roles/install.rs
  (verbatim sha256_hex), ports evidence_content_digest.
- STARTED 2026-08-31: `just ruler` BEFORE any edit (flock + sccache
  protocol, background).
  [DONE 2026-08-31] `just ruler` PASS (exit=0, digests match the frozen
  lock; log cor-r4-ruler-before.log — the shell's exit 2 is only
  `sccache --stop-server` with no server running, benign).
- STARTED 2026-08-31: COR-R4-1 — controller settled no-op predicate
  (dropped the `productions_completed.is_empty()` conjunct, COR-R4-1 doc
  bullet), FakeState `forge_checkpoint_anchor_below_first_evidence_production`,
  controller test `a_recovery_completed_production_on_a_settled_run_resumes_as_the_settled_noop`,
  sqlite test_support `regress_latest_checkpoint_offset` +
  `evidence_produced_events` + `evidence_production_rows`, E2E leg
  `the_resume_of_a_settled_run_with_a_recovery_completed_production_is_the_settled_noop`.
  [DONE 2026-08-31] Landed; check-package gauntlet-app PASS (exit 0).
- STARTED 2026-08-31: COR-R4-2 — `gauntlet_app::skills::digest_raw` (raw
  SHA-256, the roles instrument), skills_cmd digest sites switched
  (`digest_of_file`, verify source digest), module doc COR-R4-2 paragraph,
  skills_roundtrip SHAPE-1 digest instrument adaptation (reason recorded),
  NEW test `a_normalization_equivalent_body_under_a_reforged_lock_refuses_byte_drift`.
  [DONE 2026-08-31] Landed; app package check PASS.
- STARTED 2026-08-31: COR-R4-3 — writer_lock rewritten: hedged
  "stale or unreadable writer lock" refusal (never asserts a live holder;
  blank pid → `<unreadable>`), bounded reread
  (`acquire_with_rereads`, production 10×10ms), pid-writeln failure
  removes its own just-created file, 3 new unit tests; E2E leg 8
  mechanical adaptation (2 assertion tokens, reasons in comments).
  [DONE 2026-08-31] Landed.
- STARTED 2026-08-31: package-scoped test runs (app, cli, sqlite) after
  check-package gauntlet-cli re-run.
  [DONE 2026-08-31] First full package run caught TWO defects in MY NEW
  tests (mechanism held; both fixed):
  (1) controller test — the FakeState fixture's produce write is row-only
  (no EvidenceProduced event), unlike the real store; restructured the
  helper to PLANT the event from the row's own facts (the E2E leg carries
  the real store's forged-anchor route; doc pins the distinction), and
  read the event's offset back positionally (the store's own convention —
  FakeState's append counter diverges from positions where
  record_evidence_production consumed an offset without an event).
  (2) E2E leg — the export's checkpoint segment carries each checkpoint's
  own event_log_offset, so the ANCHOR SURGERY was stream-visible; the
  durable-record baseline is now taken AFTER the surgery, bracketing
  exactly the resume. Isolation re-runs: both PASS.
  Full package runs (sqlite, app, cli) re-running.
  [DONE 2026-08-31] sqlite 50/50, app 472/472, cli 95/95 — EXCEPT one
  skills_roundtrip failure (my new test): under RAW digesting the
  falsifier's normalized-digest reforge is a FIXPOINT of the honest lock
  (the canonical source is normalization-canonical), so the forgery
  cannot even land and the drift correctly refuses at the INSTALLED half
  (exactly where roles refuses it). Test restructured to pin that
  reality (fixpoint assert + installed-half refusal + raw-reforged shape
  refusing via the SOURCE anchor). skills_roundtrip 6/6; cli package
  95/95 (cor-r4-test-cli3.log).
- STARTED 2026-08-31: full battery.
  [DONE 2026-08-31] First pass: check PASS, deny PASS, doctor PASS,
  ruler PASS; THREE gate findings, all in NEW code, all fixed:
  (1) clippy op_ref in my controller test (mechanical);
  (2) hardcoded-literal gate: the writer-lock reread pause and the
  race-test's figures — rewritten per the defaults module's
  composed-figure convention (hint-free factor names, no single literal
  carries a span value); boundaries 228/228 after;
  (3) `f_collect_serves_the_poll_snapshot_not_the_file`
  (gauntlet-adapter-agent-cli live-process family) failed under full
  battery load, PASSED in isolation (cor-r4-flaky-iso.log) — the
  charter's known load-flaky live-process class, second member; noted,
  not "fixed"; full `just test` re-run decides.
  rustfmt applied to the touched packages (formatting only).
- STARTED 2026-08-31: lint re-run + full `just test` + e2e/cli_surface
  nextest + ruler-after.
  [DONE 2026-08-31] lint PASS, ruler-after PASS, e2e_full_loop_mock +
  cli_surface 14/14. Full `just test`: runs 1–3 cancelled by the
  KNOWN load-flaky live-process family (agent-cli falsifier_round1;
  varying members; passes alone; forensics in the report — pre-existing
  adapter read-window race, out of scope, recorded). Settled run:
  **1371/1371** (cor-r4-bat-test5.log).
- STARTED 2026-08-31: repo-root dry-run zero-write.
  [DONE 2026-08-31] exit 0, "dry-run complete — no freeze, no state
  rows, no evidence pins, no dispatch"; git-status before/after diff
  0 bytes (cor-r4-bat-dryrun.log).
- STARTED 2026-08-31: final report.
  [DONE 2026-08-31] `.agents/reports/task20/cor-r4-report.md` written.
  SLICE COMPLETE — all three corrections landed, battery green.

