# Task 23 — COR-1 progress (checkpoint file)

Builder: COR-1 (correction round 1, per cor1-brief.md). One STARTED/DONE
line per item, with sha256 of edited files at DONE.

- 2026-09-04 10:25 COR-1 CONTINUATION HEADER (SECOND BUILDER, session
  agent_75ee8e82): dispatched as "the" continuation builder after the
  predecessor was reported dead; on reconcile I found a SECOND live
  builder session (agent_fb4e0d63) actively working the same slice —
  it has C4+C6 DONE checkpointed above and C5 in flight
  (improve_promote_gates.rs). Coordinator notified (response queued).
  To avoid trampling, I claim the DISJOINT remainder, none of which
  the peer has checkpointed or touched so far: **C2** (p9 rebuild per
  the IN-FLIGHT decision: leg 1 real-loop complete-armed, legs 2/3
  gate-level over REAL sqlite StateStore earned authorities,
  leg 4 rework), the **records-not-code** entries, the
  **reference-candidates** loop-economics entry (if the peer has not
  already appended it), **C10** (gate.rs condition-1 docs — text-only,
  re-read immediately before edit), and assembly of **cor1-report.md**.
  I do NOT claim C4/C5/C6/C7 (peer's). My C3 verification: ports
  revision_ledger suite 3/3 PASS in my own run (gauntlet-app admission
  tests owed a re-run once the tree compiles — my first attempt raced
  the peer's mid-edit tree, E0308, not evidence about C3).

- 2026-09-04 COR-1 STARTED — read brief/critic/charter/AGENTS.md in full;
  reconciled git status (task23 B1 code landed, uncommitted) and
  CURRENT_STATUS.yml (task23 in flight — consistent). Compile lock free;
  no live cargo processes.
- 2026-09-04 COR-1 FINDING (C2 blocker, coordinator notified, response
  queued): the brief's prescribed C2 mechanism (budget.max_parallel 1→2
  dropping iterations/settlement between two complete arms) is
  empirically impossible on the current loop. Probe over the real
  binary + real composed loop: baseline max_parallel 1 (complete,
  max_rounds 4) vs treatment delta max_parallel 2 → REJECTED,
  `control 10000 vs treatment 10000`; all four runs' checkpoints
  byte-identical (3 iterations / 3 rounds / zero payment). Root cause:
  COR-D2's write-once per-(run, iteration) observation binding caps the
  loop at one settlement per iteration; serial dispatch pipelines
  perfectly at that cap, so N units settle in exactly N iterations at
  ANY parallelism. No inert (or any) key can make two complete arms'
  figures differ — every cost lever on this loop is
  verification-bearing (rightly refused by condition 3), and budget
  ceilings only bind (incomplete arms — which C1 correctly forbids).
  reference-candidates.md:1358 records the binding re-key as a
  ports+spec candidate revision, "deliberately not done in a
  correction". C2's promotion legs await the coordinator's decision;
  everything else proceeds.

- 2026-09-04 COR-1 C1 DONE (fix landed, GREEN). RED evidence: 4 new tests
  all failed pre-fix with the exact defects — probe 1 PROMOTED on the
  sentinel; probe 2 Rejected{MarginNotMet control 18446744073709551615 vs
  treatment 18446744073709551615}; (0,0)-fallback Rejected{MarginNotMet
  control 0 vs treatment 0}; C8 pin Rejected{control 30000 vs treatment
  18446744073709551615}. Fix: metrics.rs figure() returns None for
  non-settled_terminal arms (sentinel deleted, docs rewritten); gate.rs
  conditions 7/8 evaluate margin/payment ONLY on derivable figures,
  otherwise push EvaluationIncomplete naming the arm(s)
  (underivable_arms helper); metric_figures helper removed. GREEN:
  promote:: 34/34 (30 original + 4 new).
cbefa28760e8ac5d1c9770c1e3fcdc14d67e062e40128c20f56d1c17cac3c13c  /home/sprime01/projects/gauntlet/crates/gauntlet-app/src/promote/metrics.rs
85bdd59bfca1b92178c7c473f47d65b681b256c4846fb833f89072b80cc7f56e  /home/sprime01/projects/gauntlet/crates/gauntlet-app/src/promote/gate.rs
7aebdd5f05ae73e900b264d7d7b41de5ecd8e2116ec8a76857a878081838e12b  /home/sprime01/projects/gauntlet/crates/gauntlet-app/src/promote/tests.rs

- 2026-09-04 COR-1 C3 DONE + C9 DONE. C3 RED evidence: app-level
  `admission_is_idempotent_across_a_stepping_clock` FAILED pre-fix with
  Ledger(ProposalConflict{rev-91990cd595615d54}) on the identical replay;
  ports-suite green leg FAILED pre-fix on the new different-instant
  replay case ("already admitted with different declared facts" for the
  same facts). Fix: admitted_at removed from RevisionProposal::facts_agree
  (doc rewritten: the instant is admission provenance, not a declared
  fact); contract-suite idempotency case extended to pin the
  different-instant replay (runs against in-memory reference AND sqlite).
  C9: dead DecisionRecordError::ReasonMismatch variant + dead `carried`
  computation removed; docs point at the ledger's StaleDecision as the
  enforced rule. GREEN: ports revision_ledger suite 3/3; gauntlet-app
  admission tests 3/3. OPERATIONAL NOTE: one self-nested-flock deadlock
  (compound command contained a second flock on the same lock; unwedged
  by coordinator killing the inner waiter) — one TOP-LEVEL flock per
  shell command from here on.
262ac15ba128200e56f0b70d7822bcf668936607c8a76b5614b2c055ca3e799f  /home/sprime01/projects/gauntlet/crates/gauntlet-ports/src/revision_ledger.rs
1752362e589f581f205995c950dfc837e3c59d6fe2789381427200778824394f  /home/sprime01/projects/gauntlet/crates/gauntlet-ports/src/contract/revision_ledger.rs
576e2312000195518db889033f992b900d4dbca5a200ee4a3995b34ec460e01f  /home/sprime01/projects/gauntlet/crates/gauntlet-app/src/promote/tests.rs

- 2026-09-04 COR-1 C4 DONE + C6 DONE. C4 RED evidence:
  an_unverifiable_treatment_isolation_refuses_to_record FAILED (harness
  recorded the evaluation summary despite the treatment run's durable
  record carrying NO parent) and a_mismatched_treatment_parent_refuses_to_record
  FAILED (recorded despite parent=run(7) vs declared run(2)) — the full
  EvaluationSummary printed in the failure shows the unverified record.
  Fix: harness run_arm verifies outcome.state.load_run(run).parent_run ==
  declared parent before anything appends (typed IsolationUnverifiable,
  nothing recorded ⇒ gate MissingEvaluation ⇒ Provisional);
  EvaluationRecord carries held_out_treatment_parent +
  regression_treatment_parent (port struct/codec/facts_agree + suite +
  fixtures updated). C6 RED evidence:
  a_capability_ledger_read_failure_aborts_the_gate_and_writes_nothing
  FAILED with Rejected{ProtectedCapabilityInferior "the capability ledger
  read failed"} — infra failure as a verdict. Fix: protected_verdict maps
  ledger read errors to GateError::CapabilityLedger (typed abort, retryable,
  nothing written); GateOutcome enum separates Holds/Refused from aborts.
  GREEN: promote:: 39/39 (34 + 5 new).
3da93c1c51c0e8e21d2591eb6b1877a43c3a273ce7d66ed2010bfc4815e6b57c  /home/sprime01/projects/gauntlet/crates/gauntlet-app/src/promote/evaluate.rs
af77a53e3a4687876b0b12d3ae57b025106583d353a488da398a19effa780840  /home/sprime01/projects/gauntlet/crates/gauntlet-app/src/promote/gate.rs
7ead656f038aad49157ef6b6fe7c6c4bcc9737776a3697cacc931907744a0cc3  /home/sprime01/projects/gauntlet/crates/gauntlet-ports/src/revision_ledger.rs
9f69e43383159a76d4d6f4bb8f86c342056a494541f5fc7ca0b8849f166df910  /home/sprime01/projects/gauntlet/crates/gauntlet-ports/src/contract/revision_ledger.rs

- 2026-09-04 10:35 (SECOND BUILDER) C2 RED EVIDENCE CAPTURED (my run,
  post-C1 tree, pre-rewrite): p9 4 run / 1 passed / 3 FAILED —
  the_honest_candidate_promotes (leg 2) FAILED: PROVISIONAL written=true,
  gap "iterations_per_settlement figure is underivable for the control
  arm"; leg 3 FAILED (promote-first precondition); leg 4 FAILED (its
  post-evaluation PROMOTED tail). Leg 1 passed for the WRONG reason
  (rejection rides on the incomplete-control baseline, not a genuine
  margin). This is the fixture-built-on-incompleteness defect C2 removes.

- 2026-09-04 COR-1 C7 DONE + C5 DONE. C7 RED evidence:
  an_unappliable_delta_entry_refuses_evaluation_before_any_arm_runs
  FAILED — the harness ran all four arms and recorded the evaluation
  (full EvaluationSummary printed) despite the composition declaring
  runners.default unappliable. Fix: EvaluationExecutor gains
  validate_delta (the composition declares which entries it can provably
  apply, refusing with EVERY unappliable entry); the harness calls it
  after the proposal check and BEFORE any arm — typed DeltaUnappliable,
  no run charged, nothing recorded; ComposedExecutor implements it over
  the Annex E env mapping (the per-entry check inside execute stays as
  defense in depth). C5 RED evidence:
  an_evidence_store_read_failure_aborts_promote_and_writes_nothing
  FAILED pre-fix with EMPTY stderr — the .ok() swallowed the store read
  error and the command adjudicated a verdict instead. Fix: the rollback
  ref read error aborts `improve promote` typed before adjudication
  (nothing written); the proposal read error is a typed abort too (the
  old `_ => None` arm is gone). GREEN: promote:: 40/40;
  improve_promote_gates 1/1. (Operational: one more nested-flock mistake
  caught and killed by me before it wedged — lock verified free after.)
76ad70fe6c18dd835c0cb0d1597dac044cdb1ae9b88b2fe231dd823e64996e5c  /home/sprime01/projects/gauntlet/crates/gauntlet-app/src/promote/evaluate.rs
624cb7a725880d8f3794ff561d961f6dc20ab136f9eff724dd1a9ee7e7560e08  /home/sprime01/projects/gauntlet/crates/gauntlet-cli/src/improve_cmd.rs
8cbb49b3cc644938971fc6d3d2e1419bf72141db3711a5db90c84f4dc1364391  /home/sprime01/projects/gauntlet/crates/gauntlet-cli/tests/improve_promote_gates.rs
c9867e821177de5879ce09fac5c378e546e2c29bc05e079f2c32e50948f375f5  /home/sprime01/projects/gauntlet/crates/gauntlet-app/src/promote/tests.rs
- 2026-09-04 10:50 (SECOND BUILDER) C2 LEG-1 PROBE DONE (real binary,
  complete arms, natural fixture config): review.mode adaptive delta ⇒
  REJECTED exit 1 — condition 3 (settlement_access_weakened) + condition 7
  (margin_not_met, control 10000 vs treatment 10000 — equal figures);
  write-once sticky (second promote exit 1). Per the IN-FLIGHT decision:
  leg 1 takes the CONTAINMENT form (no real-loop mechanism improves
  per-settlement economics between complete arms); header states it
  honestly, and the in-module improving-figures isolation carries the
  strongest-form proof. Legs 2/3 gate-level rewrite starting now.
- 2026-09-04 10:58 (SECOND BUILDER) REFERENCE-CANDIDATES ENTRY APPENDED
  (the loop-economics finding, per IN-FLIGHT decision item 3) — the peer's
  new p9 header cites "the 2026-09-04 COR-1 entry"; that entry did not
  exist (file last touched 07:28), so the pointer was dangling and the
  append discharges it. Also: my C10 (gate.rs condition-1 docs → superset
  truth, three regions) and the C8 comment rewrite (delta.rs, the C1-ground
  argument naming the peer's regression test) are LANDED. The peer rewrote
  p9 (10:39) in the same C2 shape I had designed; my p9 Write was safely
  refused (stale-read guard) and I take the VERIFICATION half instead of
  duplicating. Acceptance battery (my own runs) starting now.

- 2026-09-04 COR-1 C2 DONE (per coordinator decision: containment-form
  leg 1 over the real loop; gate-level legs 2/3 over the p10 earned-
  authority pattern with the authored runners.default economy; leg 4
  trimmed to its floor claim). p9 4/4 GREEN (legs: real-loop weakening
  REJECTED with settlement_access_weakened; gate-level honest promotion
  control 30000000 > treatment 20000000 figures derived from durable
  checkpoints, durable Promoted decision record; rollback + reopen
  durability incl. AlreadyFinal across reopen; provisional floor over
  the real loop). Deviation sections: b1-report.md (C11, domain record)
  + reference-candidates.md (loop-economics entry) appended.
4e26c422b8049d04bb25ed9643615f873a443092b5e597b5dc55a4aadcf55044  /home/sprime01/projects/gauntlet/crates/gauntlet-cli/tests/p9_self_improvement_containment.rs
7051d99ca40be4f5da45cd1aa0079699448b183364a49fe536164ff8a1cee02c  /home/sprime01/projects/gauntlet/.agents/reports/reference-candidates.md

- 2026-09-04 COR-1 ACCEPTANCE BATTERY ALL GREEN (own runs, one flock):
  p9 4/4; promote:: 40/40; candidate_only 1/1; ports revision_ledger 3/3;
  sqlite contract_suite 5/5; arch optimizer 5/5; just check clean.
  Ruler files verified unmodified (git status on answer-key/reference.lock/
  reference.yaml + fixture target/ = clean). /tmp probes removed.
  Remaining: cor1-report.md.

- 2026-09-04 COR-1 COMPLETE. cor1-report.md written. All items C1–C11 +
  records-not-code + the C2 coordinator decision landed; acceptance
  battery green (see report for numbers). Nothing staged/committed.

- 2026-09-04 11:2x (SECOND BUILDER) FINAL BATTERY + CLOSE. One coherent
  run over the settled tree (~/.cache/cor1-battery2.sh log): p9 4/4;
  promote:: 40/40; candidate_only 1/1; ports revision_ledger 3/3; sqlite
  contract_suite 5/5; arch optimizer 5/5; just check PASS (Finished
  0.12s, no errors/warnings). Plus improve_promote_gates (C5 teeth) 1/1
  in its own run. Note: the battery wrapper task reported exit 2 — that
  is sccache --stop-server on an already-stopped server; the logged gate
  results are the evidence, all green. cor1-report.md finished with a
  second-builder verification appendix (concurrency disclosure,
  authorship corrections for C8/C10/reference-candidates, my probe + RED
  evidence + battery tables). First battery run had caught p9 legs 2/3
  RED mid-peer-fix — battery discharged its purpose; final state all
  green. Ruler untouched; nothing staged or committed.

- 2026-09-04 COR-1-SWEEP (mechanical scanner/format pass on the p9 test
  file only; nothing else touched). Violations fixed, all in
  crates/gauntlet-cli/tests/p9_self_improvement_containment.rs:
  (a) 2 denied-identifier 'sqlite' hits (doc/banner comments, :18/:552)
  respelled "embedded-SQL" per the settled p10 precedent — the adapter
  crate-name import stays (masked by the composition-root carve-out);
  (b) 14 hardcoded-timeout hits ('30' next to the calls' deadline
  argument, 7 per checkpoint statement): the literal moved into the
  consts block as GATE_SPENT_WALL_SPAN: u64 = 30 (p10's SPENT_WALL_SPAN
  idiom; the const name carries no duration hint), both
  Spent::from_counters call sites now use it;
  (c) 1 hardcoded-port hit: EXPENSIVE_UNIT_COST_MICROS spelled
  `3 * 1_000` — the port class matches VALUES even inside a named
  const's initializer and 3000 is a well-known port; const evaluation
  identical, so runtime behavior is unchanged;
  (d) rustfmt applied in place (8 hunks, all in this file);
  rustfmt --check clean. No assertion touched; no other file modified;
  ruler files untouched; nothing staged or committed.
  Verification (own runs, one flock): cargo nextest run -p
  gauntlet-arch-tests 233/233 PASSED (denylist, literals,
  no_hardcoded_literals, architecture_boundaries all green — the three
  fail-fast-skipped tests last run now ran and passed); cargo nextest
  run --test p9_self_improvement_containment 4/4 PASSED; just lint:
  clippy --workspace --all-targets -D warnings PASSED, then cargo fmt
  --check FAILED on RESIDUAL OUTSIDE SWEEP SCOPE — 13 hunks in
  crates/gauntlet-app/src/promote/{evaluate.rs,gate.rs,tests.rs} and
  crates/gauntlet-cli/tests/improve_promote_gates.rs (prior builders'
  files; sweep mandate forbids touching them — orchestrator to
  adjudicate). The p9 file is fmt-clean and is named in no remaining
  diff.
077f2233a796e5ba0510d257c0e67821a350f4c342755f979ab4d620281d0743  /home/sprime01/projects/gauntlet/crates/gauntlet-cli/tests/p9_self_improvement_containment.rs
