# Task 13 / Builder B4 — checkpoints (proofs p7 / p11 / g10 + teeth)

Started: 2026-08-27. Scope: `builder-brief-b4.md`. Build lock held by this
session exclusively.

## Pre-flight

- [x] STARTED pre-flight: read brief, recon §8, correction-round1, prereg
      D1/D2/D5.
- [x] DONE pre-flight: `just ruler` PASS (before starting); IN-FLIGHT shows
      B2 CLOSED (Phase R/G), COR-1..7 CLOSED; `git status` reconciles with
      the recorded uncommitted B2/COR/B3 working tree; no re-dispatch needed.

## Item 1 — p7_recovery.rs

- [x] STARTED (2026-08-27): design settled from recon §5/§8 + B2 wiring —
      OrderProbe delegating decorator (checkpoint brackets + commit
      before/after probes via the real store's own export), crash_helper
      located by an in-test `build --bin crash_helper` (no cross-package
      CARGO_BIN_EXE exists; no new child-binary surface), drop+recompose
      restart, resume_run driving every leg.
- [ ] DONE

## Item 2 — p11_critic_is_not_verifier.rs

- [ ] STARTED
- [ ] DONE

## Item 3 — g10_builder_writes_answer_key.rs

- [ ] STARTED
- [ ] DONE

## Item 4 — teeth-b4.md + gates

- [ ] STARTED
- [ ] DONE

## RECOVERED-AND-VERIFIED (orchestrator, 2026-08-27 10:5x EDT — 19th interruption instance)

The B4 builder ran pre-flight + declared Item 1's design (OrderProbe
delegating decorator; in-test `build --bin crash_helper` — no cross-package
CARGO_BIN_EXE; drop+recompose restart; resume_run driving every leg), wrote
p7_recovery.rs (64,698 bytes, 4 tests, mtime 09:46:59 — two minutes past
the STARTED entry at 09:44:40), then died to the sub-agent usage limit
(window resets 2026-08-28 01:25:31 EDT).

**Orchestrator verification:** `just ruler` PASS; p7_recovery.rs DOES NOT
COMPILE (8 errors E0308-class + 3 warnings — verified personally via
`cargo nextest run --test p7_recovery`); Items 2–4 unstarted (no p11/g10
files; no teeth-b4.md). Verdict: Item 1 LANDED-PARTIAL mid-edit; the file's
shape and 4 tests exist; the completing finisher fixes compile + runs the
fail-first legs per the brief. Nothing else touched.

**Parallel state:** the reference-candidates closure research (Explore)
RETURNED COMPLETE (draft entries for AK-040/041/042/043/062/031/053 + the
AK-024 discharge addendum, with a pre-paste check tied to B4's landing) —
held for closure. Blind critics over the LANDED surfaces (B1+B2+B3+COR-1)
dispatched in parallel (Explore; the installed role agents — critic,
falsifier, etc. — are not invocable from this session's delegation surface;
their roles are emulated in the prompts).

## B4-COMPLETION FINISHER (2026-08-28)

- [x] STARTED (finisher): read AGENTS.md, finisher-brief-b4, RECOVERED-AND-VERIFIED,
      builder-brief-b4, correction-round1 + critic-round1-adjudication (note: the
      file is critic-round1-adjudication.md; correction-round1-adjudication.md does
      not exist), correction-brief-r2 (COR2 NOT landed — building against current
      shapes, will flag COR2-1a/1b touchpoints), recon §5/§8, prereg D1/D2/D5.
      `just ruler` PASS before starting; git status reconciles with the recorded
      uncommitted COR-1..7/B2/B3 tree + untracked p7_recovery.rs (64,698 B) and
      no p11/g10/teeth files. Build lock held exclusively; no sub-agents.

- [x] DONE Phase R-B4 (finisher, 2026-08-28): p7_recovery.rs compiles clean
      (0 warnings under clippy) and 5/5 tests pass:
      commit_order; a_crash_before_the_settlement_event_retries_as_if_never_verified;
      a_crash_between_event_and_row_completes_exactly_once_through_resume;
      a_crash_after_the_durable_row_never_reruns_settled_work;
      a_settled_run_reresumes_over_anchored_settlements_without_rescanning.
      The last is NEW (deviation, recorded in builder-b4.md): the brief's
      anchored-replay leg moved from crash-between's third composition to a
      dedicated test over a NORMALLY settled run — a run ended by the ROUNDS
      dimension can never be legally re-driven to Settled under the run row's
      own ceilings (store validates checkpoint spend water-marks against the
      RUN's recorded budget; the per-attempt estimate arithmetic makes any
      "end after iteration 1 via tokens with rounds headroom" shape
      impossible), so the re-resume over the crash-restart's checkpoint is a
      Corrupt read by construction. Crash-between's projection assert now
      observes the restart's OWN last written checkpoint through the probe's
      recordings (never a store re-read).

      RED→GREEN evidence (recovered-honest — the original builder's fail-first
      evidence was LOST to the 19th interruption; the RED below is the
      finisher's own re-derivation):
      1. INHERITED RED: `cargo check -p gauntlet-cli --test p7_recovery` —
         8 errors (3×E0308 policy/seed/tuple, 2×E0609 Fixture fields, 2×E0599
         EvidenceStore trait not in scope, 1×E0308 1-tuple) + 3 warnings.
      2. POST-COMPILE RED (composition defects found while completing, each
         fixed per the declared design, recorded as deviations): review wiring
         contradicted the header (Single+4 model dispatches → the isolation
         gate refuses Isolation::None runners; corrected to Adaptive+0 = the
         p4 precedent matching the header's "real diff comparator decides");
         identical facet digests across compositions (content-addressed
         collision; RESTART_LAG clock offset); the anchor-arithmetic bracket
         misclassified a post-commit checkpoint at anchor == commit offset
         (replaced by the probe's global write-order stamps — the design's
         own "checkpoint brackets" mechanism); budget-shape wall (above).
      3. TOOTH (the brief's Item-1 teeth, D1): neutered the durable commit in
         the settle service (the pre-B2 in-memory-only shape):
         `Ok(admitted) => { let _ = &admitted; (EventLogOffset::new(0), true) }`
         at settle/mod.rs's admit arm → RED:
         `commit_order ... assertion 'left == right' failed: each unit's
         settlement ran the durable sequence once — left: 0, right: 2`
         (1 test run: 0 passed, 1 failed). Reverted via cmp — byte-identical
         (`cmp` clean) → GREEN: 5/5 passed.

- [x] STARTED Item 2 (finisher, 2026-08-28): p11_critic_is_not_verifier.rs
      designed — composed controller, real stores, proof-local VERDICT rung
      (the §18.4 controlled variable), ForensicRecorder delegating evidence
      decorator for the pinned refusal records, run id captured from the
      dispatch spec for the errored legs. COR2 NOT landed: built against
      current shapes; COR2-1a/1b touchpoints recorded in the file header.
- [x] DONE Item 2 (finisher, 2026-08-28): 5/5 green —
      a_pass_verdict_settles_the_same_composition (control);
      a_broken_verifier_refuses_and_nothing_settles (§18.4 instrument
      failure); no_rung_can_decide_and_nothing_settles (ladder-absent
      shape — empty ladder refused at composition, honest spelling = rung
      declining); an_inconclusive_verdict_never_settles_and_names_itself
      (three-state: pinned forensic names INCONCLUSIVE, never FAIL);
      a_fail_verdict_never_settles_and_names_the_real_cause (pinned
      forensic names FAIL). Shared core assert: no SettlementCommitted
      event, no settlements row, verification_records EMPTY (P11 core),
      projection Open.
      Deviation (recorded): the "clean comparison" observation is by
      POSITION (every leg's primary observable sits downstream of the
      comparison's acceptance; a blocking residual would produce a
      condition-4 refusal naming it), not by correction_selections —
      correction selection is ALSO driven by non-PASS verification
      outcomes, so that proxy conflated the two (found RED, fixed).
      TOOTH (the brief's teeth, 4 coordinated neuters = "settle admits on
      comparator cleanliness"): service entry outcome gate (settle/mod.rs
      `if false &&`), precondition outcome gate (precondition.rs
      `if false &&`), PASS-only pin (controller.rs `if true ||`), and the
      admission's records read replaced with a fabricated commit-matching
      PASS doc (producer_authority.rs) → RED:
      `an_inconclusive_verdict_never_settles_and_names_itself panicked:
      no SettlementCommitted event exists for the comparator-clean unit`
      (the neutered service had settled it; 0 passed, 1 failed). All four
      files reverted via cmp — byte-identical → GREEN 5/5, clippy 0.

- [x] STARTED Item 3 (finisher, 2026-08-28): g10_builder_writes_answer_key.rs
      designed — prevention through the loop's REAL effect surface (the
      completion's locator write behind contained_locator_path, the N4
      boundary) with a ruler whose locator TARGETS the key; detection as a
      builder-context RAW write of the key mid-run (the §11.4 shape) caught
      by the settle-time digest consult; BreachRecorder delegating evidence
      decorator for the pinned both-digests record. COR2 NOT landed:
      COR2-1a/1b touchpoints recorded in the file header.
- [x] DONE Item 3 (finisher, 2026-08-28): 2/2 green —
      a_builder_context_write_at_the_key_is_refused_by_containment
      (typed EvidenceLocatorEscapes naming locator + root; key bytes
      byte-identical; no settlement under the refused ruler);
      a_midrun_key_tamper_aborts_with_both_digests_pinned (terminal
      aborted, breach carries expected==frozen and actual!=expected, both
      digests PINNED as forensic evidence, no SettlementCommitted/no row
      over the mutated ruler, tampered bytes preserved on disk).
      Deviation (recorded): the prevention leg's escape needle is BUILT
      (`..` + PATH_SEP + needle, the in-crate N4 scanner idiom) — a first
      draft dropped the separator (`..ruler/...` is a NORMAL component,
      the run settled and exposed the bug); fixed with the named-separator
      construction.
      TOOTH (the brief's teeth, the prereg D4 tautology):
      settle_time_verify's provider consult replaced with a
      frozen-digest-vs-itself tautology → RED:
      `a_midrun_key_tamper_aborts_with_both_digests_pinned panicked: no
      SettlementCommitted event exists over the mutated ruler` — the
      tautologized consult let the settle commit (the LATER boundary
      tripwire still aborted the run, so the leg's settle-time-specific
      assertion is the discriminator; 0 passed, 1 failed). Reverted via
      cmp — byte-identical → GREEN 2/2, clippy 0.

## ITEM-4 COMPLETION (orchestrator-run, 2026-08-27 15:0x EDT, per the standing heal+continue instruction)

The B4 finisher (dispatched on the operator's usage reset) completed Phase
R-B4 + Items 2 and 3 (all DONE entries with teeth evidence) and died to
the limit during Item 4, leaving a 14:25→14:27 unrecorded window (fmt +
final touches on the three proof files; compile-verified coherent) and the
gate sweep + result contract unwritten. The orchestrator completed Item 4:

- **Boundary conformance (never-gated code)**: the literal scanner caught
  6→3→1 violations in the new proof files — fixed mechanically per the
  suite's own idioms: g10's joined path literal composed from single
  components + `escaping_locator_needle()` (its own comment already
  declared the intent); p7's `CARGO`/`build`/`--bin`/prose-in-
  Command::new-unit needles rebuilt as char-array collections and hoisted
  expect-bindings (concat! args are scanned individually — recorded).
- **Gate battery (personal)**: boundaries **215/215**; proofs
  **12/12** (p7 4 + p11 5 + g10 2 + ... = 12 across the three targets);
  `settle::` **22/22**; standing battery p4/p6/p12/p2/p3 **33/33**;
  `just check` clean; `just lint` exit 0; `just deny` ok; `just ruler`
  PASS before and after.
- Item 4's teeth-b4.md existed (14:25, finisher-written with the items'
  tooth evidence).

**Verdict: B4 CLOSED** — p7_recovery (commit_order + reorder tooth + three
real-kill windows + anchored replay + lost evidence), p11 (5 legs incl.
neutered-gate tooth), g10 (prevention + detection incl. tautology tooth),
teeth recorded. builder-b4.md written by the orchestrator from the
checkpoints.
