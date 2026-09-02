# Task 20 — COR-R4 correction report (falsifier round 4 → corrections)

Builder: COR-R4. Work order: `falsifier-adjudication-r4.md` (authoritative).
Inputs: `falsifier-round4.md` (executed attacks + loci), `charter.md`
(+ operational addenda), `cor-r3-report.md` (the recovery machinery being
adjusted), AGENTS.md. Checkpoint trail: `cor-r4-progress.md`. `just ruler`
PASS before the first edit and after the last (digests match the frozen
lock both times).

VERDICT (builder claim, no settlement authority): all three adjudicated
corrections implemented and pinned. The settled no-op now closes an
otherwise-settled run as `terminal=settled` at the checkpoint's own
iteration even when the recovery COMPLETES a production (never books an
iteration for a fully-settled durable projection), through both a composed
controller test and the falsifier's real-binary reproduction; skills verify
digests RAW bytes exactly as roles does and both normalization-equivalent
forgery shapes refuse; the writer-lock refusal never asserts a live holder
and the create-vs-pid-write race is closed by a bounded reread. Full battery
green (below). No assertion weakened; no shipped proof, E2E leg, or gate
disabled or bypassed.

---

## COR-R4-1 (MAJOR) — a recovery-COMPLETED production on an
otherwise-all-settled run closes as the honest settled no-op

### Mechanism

The controller's settled no-op predicate (`resume_run`,
`crates/gauntlet-app/src/control/controller.rs`) dropped exactly one
conjunct: `completion.productions_completed.is_empty()`. The §18.3 step-1
completion runs BEFORE the predicate, so the predicate's projection is
already the POST-recovery projection — the adjudication's "re-assess the
terminal projection AFTER recovery completes" is structural there. The
mint half inserts a row cross-validated against the durable
`EvidenceProduced` event, appends no event, and moves no field a
checkpoint records — durable by itself, the same class as the forensic
pins COR-R3-1 already forgave. Re-entering the loop from that fact set
books a zero-dispatch round whose booking is refused (rounds spent) and
sets `budget_exhausted` BEFORE `decide_progress` is consulted — the exact
burn the falsifier reproduced. The `settlements_completed == 0` conjunct
STANDS (adjudicated scope is the production half only), as do in-flight,
non-empty membership, and the all-settled event-ordered projection.

The distinction is documented in the predicate's own comment and in both
new tests: the forged-checkpoint-offset precondition of the executed
reproduction stays the recorded "checkpoint() forged-row projection class
(planning-layer)" deferral — the predicate keys on the event-ordered
projection being all-settled, whatever surgery produced it, and never
validates or repairs checkpoint offsets. Nothing in this correction
widens what the store accepts.

### Tests

- Controller
  `a_recovery_completed_production_on_a_settled_run_resumes_as_the_settled_noop`
  (produced-world fixture): settled run → A5 end-state (produce event
  durable above the anchor, row missing) → resume ⇒ terminal=settled at
  the checkpoint's OWN iteration, zero re-dispatch, the settlement
  unduplicated, no invocation re-admitted, ZERO forensic pins (a
  completed production is not damage), no `budget_exhausted` anywhere,
  and the second resume is the same no-op. The mint is pinned at the
  EVENT'S OWN identity: the store-reported offset, the event's ref, and
  the event's stored timestamp (planted timestamp 1_750_000_000, not the
  recovery clock's).
- E2E `the_resume_of_a_settled_run_with_a_recovery_completed_production_is_the_settled_noop`
  (`crates/gauntlet-cli/tests/e2e_full_loop_mock.rs`): the falsifier's
  exact compound reproduction through the real composed binary — one
  production row deleted AND the close-out checkpoint's anchor regressed
  strictly below the locator's durable produce event. Pins: exit 0,
  `terminal=settled`, `iterations=` the run's own, zero
  `budget_exhausted`, zero forensic pins, the re-minted row bound to the
  event's own ref/timestamp/offset (byte-level via the new sqlite
  readers), the durable export stream unchanged by the resume, and the
  second resume a byte-identical zero-write no-op (whole-scratch content
  hash equal). The leg's doc carries the deferral distinction.

### Fixture facts (recorded, not defects)

- The FakeState fixture's produce write is deliberately row-only (no
  event), the inverse of the real store's event+row write; the controller
  test therefore PLANTS the event from the row's own facts
  (`plant_orphan_evidence_production_event`), where it naturally sits
  above the close-out anchor. The real store's forged-anchor route is
  exactly what the E2E leg exercises.
- FakeState's append counter diverges from event positions wherever a
  production consumed an offset without an event; the planted event's
  offset is read back positionally — the store's own read convention.

---

## COR-R4-2 (MINOR) — skills verify digests RAW source bytes exactly as
roles does

### Mechanism

- New `gauntlet_app::skills::digest_raw(bytes) -> String`: SHA-256 over
  RAW bytes as `sha256:<hex>` — the roles lock's own instrument
  (`roles/install.rs::sha256_hex`), shared so the two installed-content
  surfaces cannot drift into two rules.
- `crates/gauntlet-cli/src/skills_cmd.rs`: all three digest sites
  switched to the raw instrument (install's lock digest via
  `digest_of_file`, verify's source anchor, verify's installed half); the
  `normalize::digest_of` normalizer pass is gone from the surface.
  Install writes the source bytes verbatim, so the honest round trip is
  unchanged (lock = raw(source) = raw(installed)); the refusal messages
  (skill + both digests) are untouched.

### Tests

- NEW round-trip leg
  `a_normalization_equivalent_body_under_a_reforged_lock_refuses_byte_drift`
  (`crates/gauntlet-cli/tests/skills_roundtrip.rs`), pinning the
  falsifier's F-R4-1 shape and a discovery the fix surfaces:
  - The CRLF + trailing-blank swap is byte-different and provably inside
    the normalizer's fixed-point class (normalized digests equal — the
    precondition is asserted, not assumed).
  - Under RAW digesting the falsifier's normalized-digest reforge is a
    FIXPOINT of the honest lock: the canonical source is
    normalization-canonical (raw == normalized), so the "reforged" lock
    IS the honest lock and the forgery cannot even land — pinned with an
    assert_eq. The drift still refuses, at the INSTALLED half (raw
    installed bytes ≠ the lock), naming the skill and both digests,
    exactly where roles refuses the same shape; the source anchor
    correctly passes and "no drift" is never printed.
  - The raw-reforged shape (lock internally consistent over the swapped
    bytes) refuses through the SOURCE anchor, naming both digests.
  - Re-install repairs from the canonical source byte-exactly.
- Shipped `a_self_consistent_lock_over_tampered_bodies_is_drift_against_
  the_canonical_source` (COR-R3-2): UNMODIFIED in behavior and assertion
  strength; one MECHANICAL adaptation — its two digest computations use
  the surface's new raw instrument so the forged lock stays internally
  consistent the way the falsifier's attack was (reason recorded in the
  test's doc). All six round-trip tests green.

---

## COR-R4-3 (MINOR) — writer-lock Held refusal: hedged wording + the
create-vs-pid-write window closed

### Mechanism (`crates/gauntlet-cli/src/writer_lock.rs`)

- The refusal NEVER asserts a live holder. The guard probes no process
  state, so pid reuse is always plausible; every Held refusal now reads
  "stale or unreadable writer lock (<path> names pid X) — … the named pid
  may be a dead holder or an unrelated process (pid reuse), so confirm no
  writer is running, then remove the lock file manually and retry",
  keeping the pinned "never guesses liveness / never takes over
  automatically" posture. A blank or unreadable pid reports `<unreadable>`
  — the falsifier's blank "holds pid " shape is structurally impossible.
- Bounded reread: on `AlreadyExists`, the pid read is retried over
  `PID_REREAD_ATTEMPTS × PID_REREAD_PAUSE_SPAN` (10 × 10ms, production)
  and exits early the moment a pid lands — the pid-present case reads on
  the first attempt, so an honest refusal is never delayed; only the
  microsecond create-vs-pid-write race waits, and a lock whose pid never
  lands reports stale-or-unreadable after the window. Exclusivity is
  untouched: the O_EXCL create alone admits, and the retry closes only
  the READ. Test seam: `acquire_with_rereads(state_dir, rereads, pause)`.
- A failed pid `writeln` now removes the just-created file (the failed
  admission must not leave its own blank stale lock behind).
- No new write APIs: the arch gate's three inventoried sites
  (`fs::create_dir_all`/`OpenOptions` under `fn acquire`,
  `fs::remove_file` under `impl Drop for WriterLock`) are untouched, and
  `just boundaries` is green with no inventory change.

### Tests

- NEW unit tests: the hedged wording for a readable (live, unrelated)
  pid, including the negative pin that "another writer holds" never
  prints; the blank lock refused as `<unreadable>` after the window (no
  blank where the pid belongs, fail-closed, file left for the operator);
  the race-window regression (a pid landing 50ms late is picked up by a
  widened reread and the refusal names the real holder).
- The two COR-R3 unit tests pass UNMODIFIED (the new message keeps their
  pinned substrings).
- E2E leg 8 (`the_single_writer_guard_bounds_the_concurrent_double_resume_attack`):
  one MECHANICAL adaptation, two assertion tokens — the pinned refusal
  substring "another writer holds" → "stale or unreadable writer lock"
  (reason: the adjudication reworded the refusal to stop asserting a live
  holder; the assertions' strength is unchanged — exit non-zero, typed
  refusal, pid named, manual recovery + no-takeover named). All other
  leg-8 assertions and both E2E concurrency shapes green.

---

## Files touched

- `crates/gauntlet-app/src/control/controller.rs` — predicate conjunct
  removed + comment rewritten (COR-R4-1), `plant_orphan_evidence_production_event`
  fixture helper, 1 new controller test.
- `crates/gauntlet-app/src/skills/mod.rs` — `digest_raw`.
- `crates/gauntlet-cli/src/skills_cmd.rs` — raw digest sites + module doc.
- `crates/gauntlet-cli/src/writer_lock.rs` — hedged report, bounded
  reread (`acquire` → `acquire_with_rereads`), writeln-failure cleanup,
  module/variant docs, 3 new unit tests.
- `crates/adapters/gauntlet-adapter-state-sqlite/src/lib.rs` — doc-hidden
  `test_support::regress_latest_checkpoint_offset` (the forged-anchor
  surgery, documented as the recorded planning-layer deferral exercised
  as GIVEN damage), `evidence_produced_events`, `evidence_production_rows`.
- `crates/gauntlet-cli/tests/e2e_full_loop_mock.rs` — 4 thin wrappers,
  1 new E2E leg, 2 mechanical assertion-token adaptations (leg 8).
- `crates/gauntlet-cli/tests/skills_roundtrip.rs` — instrument adaptation
  (recorded) + 1 new test.
- `.agents/reports/task20/cor-r4-progress.md`, this report, battery logs
  (`cor-r4-*.log`), one IN-FLIGHT.md ledger line.

## Assertion changes, with reasons

- The settled no-op predicate dropped exactly one conjunct
  (`productions_completed.is_empty()`) per the work order; every other
  conjunct stands; the shipped settled-resume no-op proofs are untouched
  and green (healthy completions complete nothing).
- Leg 8's two refusal-substring assertions track the adjudicated rewording
  (strength unchanged, reason in comments at both sites).
- skills_roundtrip COR-R3-2 test: digest instrument swap in the forgery
  setup only (the surface under test changed instruments); every
  behavioral assertion identical.
- No assertion was weakened; nothing was skipped, disabled, or excluded.

## Battery (evidence = what the runs printed; logs in .agents/reports/task20/)

| Gate | Result | Log |
|---|---|---|
| `just ruler` (before edits) | PASS — digests match the frozen lock | cor-r4-ruler-before.log |
| `just check` | PASS (exit 0) | cor-r4-bat-check.log |
| `just test` | PASS — **1371 tests run: 1371 passed, 0 skipped** | cor-r4-bat-test5.log |
| `just lint` | PASS (exit 0; one clippy op_ref fix in my new test + two rustfmt passes — formatting only) | cor-r4-bat-lint3.log |
| `just deny` | PASS (exit 0) | cor-r4-bat-deny.log |
| `just boundaries` | PASS — **228 tests run: 228 passed** | cor-r4-bat-boundaries2.log |
| `just doctor` | PASS (exit 0) | cor-r4-bat-doctor.log |
| `just ruler` (after edits) | PASS (exit 0) | cor-r4-bat-ruler2.log |
| Package tests | sqlite **50/50**, app **472/472**, cli **95/95** | cor-r4-test-{sqlite2,app2,cli3}.log |
| `cargo nextest run -p gauntlet-cli --test e2e_full_loop_mock --test cli_surface` | **14/14** (incl. the new leg + all shipped legs + 4 cli_surface pins) | cor-r4-bat-e2e.log |
| repo-root dry-run | exit 0 — "dry-run complete — no freeze, no state rows, no evidence pins, no dispatch"; before/after `git status --porcelain` diff **0 bytes** | cor-r4-bat-dryrun.log |

Over-rejection leg clean: honest builders settle in both worlds (the
shipped E2E legs and the p1–p12/g6/g7/g10 proofs are inside the green
suites, UNMODIFIED); the settled no-op proofs (both worlds), the
pins-don't-defeat-the-no-op legs, and the shipped E2E damage legs are all
green as shipped.

## Deviations / findings for the record

1. **My first controller test failed for a fixture fact, not the
   mechanism**: FakeState's produce write is row-only (no
   `EvidenceProduced` event) and its append counter diverges from event
   positions — the test was restructured to plant the event and read the
   offset back positionally (fixture facts documented at the helper).
2. **My first E2E leg over-claimed zero-writes**: the export stream's
   checkpoint segment carries each checkpoint's own `event_log_offset`,
   so the ANCHOR SURGERY was stream-visible; the baseline export is now
   taken after the surgery, bracketing exactly the resume.
3. **A discovery pinned, not papered over**: under raw digesting the
   falsifier's normalized-digest reforge is a fixpoint of the honest
   lock, so the discrimination moves to the installed half for that
   shape — the same place roles refuses it. Pinned with positive and
   negative assertions.
4. **Gate adaptations in NEW code only**: clippy op_ref (my test);
   hardcoded-literal scan flagged the reread pause/race figures —
   rewritten per the defaults module's composed-figure convention
   (hint-free factor names, no single literal carries a span value);
   rustfmt on the touched packages. No gate edited or weakened.
5. **RECURRING KNOWN FLAKE, escalated for the orchestrator**: the
   `gauntlet-adapter-agent-cli` live-process family
   (`falsifier_round1_live_attacks.rs`) failed in full-battery runs
   1–3 (`b5_foreign_identity_is_contract_mismatch` — the charter's own
   named member — plus `c_two_invocations_no_cross_wiring`,
   `d_env_and_prompt_channels_and_envelope_sibling_are_exact`,
   `f_collect_serves_the_poll_snapshot_not_the_file`, varying by run)
   and once as a whole isolated target, then PASSED: the settled full
   run was 1371/1371 and each member passes alone. Forensics from a left
   scratch (`/tmp/gauntlet-falsifier-*`) show both result documents
   complete and correct while the poll classified a terminal
   `MalformedResult` — consistent with `ProcessChildTransport` reading
   `result.json` while the non-atomic probe child is mid-write
   (torn-read classified terminal instead of retried; the same
   read-window class COR-R4-3 closes for the writer lock, but in the
   agent-cli adapter — OUT OF THIS WORK ORDER'S SCOPE, recorded here and
   in CURRENT_STATUS context for adjudication; nothing weakened, nothing
   "fixed" by me). Charter protocol followed: known class, recurrence
   noted with mechanism and loci.
6. `sccache --stop-server` after `just ruler` before edits errored
   (no server was running — nothing had compiled); benign, the protocol's
   purpose was served trivially.
