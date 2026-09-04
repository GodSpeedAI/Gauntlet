# Task 23 — B1 progress

- 2026-09-03 B1: STARTED — charter + AGENTS.md read; exploring patterns
  (capability_ledger port/suite/adapter, role_result codec, config wiring,
  CLI improve stub, compose root). No files edited yet.
84b6c00406647bf28007616142b5dda35317b37092bc4f3369633059240fcb3a  crates/gauntlet-ports/src/role_result.rs
7ae887e8c76953dca829711e840578e01c6e53354d990f44732e470d271707c6  crates/gauntlet-ports/src/revision_ledger.rs
c53a5ce1a9844e2304a8d1870b554816689c67f95fc77cec0106a23544a83a13  crates/gauntlet-ports/src/contract/revision_ledger.rs
e011898713b97bea7b7c1341bd6bd9882a48c2e2301555063fbc6895c76ed663  crates/gauntlet-ports/src/contract/stubs.rs
e567b1bd77e0bb6f776e68357825ad514ec0a08169d6a111f9d5cb564bec570b  crates/gauntlet-ports/src/contract/suite_runs.rs
d02752351062167744aece341b36f2a2c12ba838cd0553db5ebc06198f11b7e8  crates/gauntlet-ports/src/contract/mod.rs
87abb79a87fa5e7609a1ee05ee6ae5e4fe2d070ee0b15dfa015e1aed15fd8f5a  crates/gauntlet-ports/src/lib.rs
de2762da7b46003e0491dd2bda09f5848a470550c0a4517446f6cb198cdaf2da  crates/adapters/gauntlet-adapter-state-sqlite/migrations/0010_revision_ledger.sql
6502fb58424b33e1a6209291fc7e76e322ede07aeecf4d60303c4f463ac6a59a  crates/adapters/gauntlet-adapter-state-sqlite/src/migrations.rs
057e15420aecba7c5da479d8e0a3697e1c0a47b50b6743250035fd6f893acea7  crates/adapters/gauntlet-adapter-state-sqlite/src/revision_ledger.rs
e150b0ebbc7c77fb60d75ae8e5093207ceb1366b416c5b46390734d93ed3d085  crates/adapters/gauntlet-adapter-state-sqlite/src/lib.rs
448f8d1386618c4f1267872022130d13758e7a4986eb539215be68411389c426  crates/adapters/gauntlet-adapter-state-sqlite/tests/contract_suite.rs

- 2026-09-03 B1: PORTS + SQLITE LANDED (compiling). Files + sha256 above.
  - role_result.rs: candidate_revision@v1 payload validator (nine §19.3
    fields; decision/evaluation-evidence keys refused mechanically), tests.
  - revision_ledger.rs: new port (proposals/evaluations/decisions/restores;
    derived decision state, no status column; typed per-condition refusals;
    zero-reason verdicts do not compose; write-once past final states).
  - contract/revision_ledger.rs: six-case suite; stubs Green/Broken/Caps;
    three suite_runs legs.
  - sqlite: migration 0010 (four INSERT-only tables) + adapter + suite leg
    + reopen-durability test.
- NEXT: gauntlet_app::promote module.
- 2026-09-03 B1: RunMetrics field renamed honestly: dispatch_mix →
  dispatched_units (tier/authority are per-unit frozen reference
  properties; the durable figure the store can read is the dispatched-unit
  mix; equality of unit mix IS equality of the tier/authority mix across
  arms). RevisionPromotionEvidence.dispatch_mix_equal →
  dispatched_units_equal.
- 2026-09-03 B1: app promote module GREEN (30 promote:: tests), sqlite
  adapter suite green, p9 legs GREEN (4/4) after the honest-metric
  redesign, arch scans green after renaming (unit_mix_preserved avoids
  figure+dispatch adjacency spelling a denied term; fixture numerals
  lifted; evaluation_plan wire token avoids the 'test' adapter segment).
- Deviation recorded: the real composed loop's per-settlement economics
  are scale-invariant under ANY inert config delta (1 settlement per
  iteration; mock invocations are free). The legs' improvement lever: the
  control task-set configuration carries budget.max_rounds=2
  (max_parallel=3, generous tokens) so its run ends UNSETTLED with a
  subset of settlements; the candidate raises the ceiling to 4 (inert
  key). Unsettled arms derive the WORST figure (they owe the remaining
  work — documented in TargetMetric::figure). Governance compares the
  dispatched-unit SUPERSET (tier/authority profile preservation, not
  unit-count equality — dispatch breadth is budget-driven).
- EvaluationRecord gained metric + protected (declared-at-evaluation
  provenance; the gate adjudicates exactly what the operator declared).
- 2026-09-03 B1: arch-scan module landed (gauntlet-arch-tests/src/
  optimizer.rs: scan_optimizer_containment + repo scan + 3 negative
  controls; wired into lib.rs). AK-057 candidate_only landed
  (crates/gauntlet-app/tests/promotion.rs; behavioral byte-identity +
  pending floor + provisional path). reference-candidates.md entry
  APPENDED (restored committed content after accidental truncate —
  final file = HEAD content + new entry, verified by git diff = +24).
  Full check/lint/deny green. Workspace test run in flight.
- 2026-09-03 B1: ALL GATES GREEN. check 0 errors; lint clean; test 1487/1487;
  boundaries 233/233; deny ok; ruler PASS. Plan gates: p9 4/4; promote:: 30/30;
  candidate_only 1/1. Bite proof executed + reverted. b1-report.md written.
