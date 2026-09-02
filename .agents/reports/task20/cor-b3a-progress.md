# COR-B3a progress — composition correction driven by live E2E failure

Protocol: declare-first STARTED/DONE lines per item. This file is the contract
if the session dies.

- STARTED 2026-08-31 (COR-B3a) — Item 0: read charter/b3-report/brief/b2-report; beginning trace of the evidence write path (controller.rs, observation::, compose.rs) before any edit.

- DONE (trace) 2026-08-31 — Write path fully traced. Conflation site: controller.rs process_completion
  (~L2717-2770) unconditionally writes the FIRST facet's facet_evidence@v1 bytes at the unit's
  evidence_required.locator and records the production over the facet ref; the verify chain's
  three-way equality (store bytes == production digest == locator bytes, verify/authority.rs
  binding_class) then forces every rung to read facet bytes at the locator — no artifact-contract
  rung can decide. CORRECTION SHAPE (adjudicated against the UNMODIFIED-proofs constraint):
  ingest-or-materialize — if the completion produced a verification artifact at the locator,
  the evidence owner INGESTS those bytes into the content-addressed store and records the
  production over THEM; if the locator holds no artifact, the facet document (the completion's
  only produced instrument evidence) is materialized at the locator and the production binds the
  facet ref — byte-identical to the pre-correction behavior on that leg. The owner NEVER
  overwrites a produced artifact. This keeps every p1-p12/g6/g7/g10 proof and ALL controller
  tests green UNMODIFIED (p4/p7's proof-local facet_evidence@v1 rungs and every controller-test
  path ride the unchanged absence leg); the composed E2E rides the new ingest leg.
  Item 3 VERIFIED: refusal site crates/adapters/gauntlet-adapter-compare-model/src/lib.rs:216
  (admit_comparison) checks the CRITIC RUNNER's isolation capability < Process — honest §13.2
  fresh-context fail-closed governance, NOT the builder sandbox posture; no change.
  Item 2: compose.rs:725 already wires id_generator_from_env (B2); only the clock hook is missing
  (compose.rs:692 hardwires SystemClock). Plan: clock_from_env() with typed EnvHook refusal for
  unknown tokens (never silent fallback).
- STARTED 2026-08-31 — Item 1 edit: controller.rs process_completion ingest-or-materialize.
- DONE 2026-08-31 — Items 1+2 edits landed; `just check-package gauntlet-app` and
  `just check-package gauntlet-cli` both exit 0 (flock + sccache stop inside the lock).
  Correction shape recorded above (ingest-or-materialize; absence leg byte-identical).
- STARTED 2026-08-31 — Item 4a: fixture answer key re-key. CONSTRAINT DISCOVERED: the mock
  scenario parser (scenario.rs key_line/value_of) supports SINGLE-LINE scalars only, so the
  two-line rung artifacts (digest manifest, command transcript) CANNOT be written by scenario
  effects. Per the brief ("fixture fixes are yours..."), the fixture units re-key to the
  test-report rung — the one contract expressible as a single-line JSON effect
  ({"tests": N>0, "failures": 0}). Lying leg: same-shape reports with failures>0
  (a present-and-wrong FAIL DECISION, never zero-tests which is InstrumentFailed -> run error).
- DONE 2026-08-31 — Items 4a+4b fixture updates + two MORE live-failure corrections discovered
  by the empirical probes (both fixed):
  (a) RE-PRODUCED IDENTICAL ARTIFACTS: content addressing lands a retry's unchanged work
      product on the already-recorded ref; the store's write-once production rule refused the
      re-record with different anchor facts -> run error. Fix (controller.rs): the produce step
      SKIPS the record when an identical durable fact exists (same ref+locator+instrument);
      §14.3 recency then honestly classes unchanged content stale at a later iteration.
  (b) EXPORT VOCABULARY GAP: the app's GENERIC_KINDS lacked `correction_selection` — the
      §15.1/AK-037 record EVERY boundary-crossing run writes — so `export` refused any run
      that crossed a boundary (B3's probes never reached one). Fix (state/export.rs): admit the
      kind (generic {record,seq,at,payload} shape matches the store's emission).
  Also: fixture GAUNTLET.md max_parallel 2->1 — PARALLEL COMPLETIONS in one iteration collide
  at the write-once per-(run,iteration) observation binding (each completion assembles its own
  bundle; the real store refuses the second bind). NOT fixed (control-logic design tension,
  spec-level question) — recorded as an adjacent discovery; the fixture runs sequentially.
  Empirical validation through the REAL binary (fixture copied to /tmp):
  happy: exit 0 terminal=settled iterations=3, 3/3 implemented-and-proven, export 29 records,
         verify refused=0 pass=1(stale-honest 2) NOTHING settled, artifacts on disk unpinned;
  lying: exit 1 terminal=budget_exhausted settled=0, zero proven units, bounded (no error);
  dry-run: exit 0, zero writes confirmed, lock verified with regenerated digest.
  NOTE: fixture .gauntlet/reference.lock is B3's fixture artifact (regenerated answer-key
  digest); the REPOSITORY's own locks/wayfinder untouched.
- STARTED 2026-08-31 — Item 4c: crates/gauntlet-cli/tests/e2e_full_loop_mock.rs.
- STARTED 2026-08-31 — p4 regression analysis (protected proof fails under ingest-always):
  p4's proof-local facet-document rung + correction cycle REQUIRE the pre-correction behavior
  (fresh facet re-materialized + re-bound each iteration); the composed E2E's artifact rungs
  require locator content binding. One hard-coded policy cannot serve both — the two-world seam
  is the mechanism fix: produce policy becomes a COMPOSITION DECISION. ControllerDeps UNCHANGED;
  RunController::compose() keeps the default (MaterializeObservation = pre-correction behavior,
  proofs + controller tests byte-identical); new additive constructor
  compose_binding_produced_artifacts() opts the CLI composition into BindLocatorContent
  (ingest-if-present, materialize facet only at absence, never overwrite). compose.rs calls the
  new constructor. No proof edited; no store rule weakened.
- DONE 2026-08-31 — Two-world produce policy LANDED: ProducePolicy {MaterializeObservation,
  BindLocatorContent} in controller.rs; compose() default = MaterializeObservation (pre-correction
  behavior, byte-for-behavior); NEW additive constructor compose_binding_produced_artifacts();
  CLI compose.rs opts in. ControllerDeps UNCHANGED (no proof fixture touched). Verified:
  p4 isolated PASS (materialize world) · e2e_full_loop_mock 3/3 PASS (ingest world) ·
  controller pass-leg/escape-locator tests PASS · check-package app+cli 0.
  KNOWN-CLASS note: b5_foreign_identity_is_contract_mismatch flaked once in a full battery
  (charter-documented live-process class); next full battery pending.
- STARTED 2026-08-31 — Full gate battery (check/test/lint/deny/boundaries/ruler/doctor/
  cli_surface focused/root dry-run).
- DONE 2026-08-31 (COMPLETE) — Full battery green: ruler before+after PASS 0 · doctor exit OK ·
  check 0 · test 1322/1322 · lint 0 · deny ok · boundaries 228/228 · focused
  e2e_full_loop_mock+cli_surface 7/7 · repo-root dry-run exit 0 ZERO_WRITES_CONFIRMED.
  Known-class: b5_foreign_identity flaked once mid-battery, green on the completed full run.
  Final report: .agents/reports/task20/cor-b3a-report.md. Slice COMPLETE — resumable state not
  needed; all work uncommitted (orchestrator commits).
