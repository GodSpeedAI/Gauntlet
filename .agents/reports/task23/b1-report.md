# Task 23 — B1 final report: process optimizer + the §19.4 self-improvement gate (P9)

**Status: COMPLETE — all plan gates and global gates green.**

## What landed

### 1. `candidate_revision@v1` payload validator (ports, D1)
`crates/gauntlet-ports/src/role_result.rs`: strict nine-field payload
(claimed_problem, supporting_evidence, affected_component, predicted_improvement,
potential_regression, candidate_change, test→`evaluation_plan`, held_out_evaluation,
rollback). Empty declarations refused; decision/evaluation-evidence keys
(`decision`, `evaluation_run_ids`, `held_out_result`, `regression_result`,
`protected_capabilities_result`) are mechanically refused as unknown fields. The
codec registry learned exactly one new schema token. 5 new tests.

### 2. `RevisionLedger` port + shared contract suite (ports, D2)
`src/revision_ledger.rs`: proposals (idempotent on RevisionId, content-derived),
evaluation records (idempotent on the four-run identity; carry the declared
metric + protected set), decision records (the ONLY decision writer; `claimed`
validated against the DERIVED state; write-once past promoted/rejected), restore
records (append-only; the rolled-back fact is row existence). Zero-reason
verdicts do not compose. `src/contract/revision_ledger.rs`: 6-case suite;
green/broken/caps stubs + 3 `suite_runs` legs (`revision_ledger_broken` caught
on idempotency, `caps_broken` on exactness).

### 3. SQLite adapter (D3)
Migration `0010_revision_ledger.sql` (four INSERT-only tables, no decision
column), `SqliteRevisionLedger` passing the shared suite UNMODIFIED (5/5 in
`tests/contract_suite.rs`) + a reopen-durability test (derived
promoted-but-rolled-back state survives a full reopen).

### 4. `gauntlet_app::promote` (D4) — delta / metrics / proposal / evaluate / gate / rollback
- **proposal**: struct holds ONLY ledger+clock. Content-addressed revision id
  (`rev-<sha256-16>`); dispatched `propose` through AgentRunner
  (invoke → handle → bounded probe loop → collect → §10.3 rule-6 check → same
  admission path); `--from-result` takes the same strict decode.
- **delta**: config-delta document (canonical JSON `[{key,value}]`), keys
  validated against the §8.2 vocabulary (`ConfigKey::from_path`), typed TOTAL
  classification (verification-bearing / inert / protected fail-closed).
  AK-056's refusal: `review.mode` downgrade ⇒ `settlement_access_weakened`.
- **metrics**: `TargetMetric` (iterations/payment per settlement, lower-better,
  fixed-point per 10 000); `derive_run_metrics` from durable facts
  (checkpoint iterations/payment, settlements joined to verification records
  for instrument facts, dispatched-unit mix, derived settled-terminal from
  latest checkpoint plan ⊆ settlements). An arm that did not reach settled
  terminal derives the WORST figure (it owes the remaining work).
- **evaluate**: `EvaluationExecutor` seam (app-owned trait) + harness running
  control/treatment per task set with `parent_run` set on treatment, both arms
  same frozen reference, durable `EvaluationRecord`, per-run developmental
  import under the declared protected capabilities. Executor failure records
  nothing.
- **gate**: ALL EIGHT conditions, all evaluated, typed per-condition refusals;
  FAILED ⇒ Rejected (all reasons + rollback ref); gaps ⇒ Provisional (never
  promoted, never rejected); zero-evidence shape stays PENDING and writes
  nothing (domain floor). Margin: strict improvement at margin 0; u128
  cross-multiplied arithmetic.
- **rollback**: infallible record composition, LEDGER-validated claim; refuses
  never-promoted and already-restored; one-way.

### 5. Config (D5)
`GAUNTLET_PROMOTION_MARGIN` / `promotion.margin`, u64 basis points, default
`PROMOTION_MARGIN_BPS = 0` with honest doc (strict-improvement floor; 0
accepted, unlike the §19.2 minimum). Full PromotionMinSettlements wiring
pattern: key, env, kind, static-env row, effective+resolved fields, GAUNTLET.md
walk, canonical serialize/parse, set_whole arm, resolution + fingerprint tests.

### 6. CLI (D6)
`improve propose [--from-result <path>] [--from-run <run-id>]` (dispatches the
optimizer through the configured runner with an allowlisted envelope: role body
+ operator-named run projection), `improve evaluate <revision> --held-out <dir>
--regression <dir> [--protected caps] [--metric token]` (executor seam over
`compose_run`; per-run writer locks), `improve promote <revision>` (gate; fail-
closed exit: only promotion exits 0), `improve rollback <revision>`. Manifest
row flipped wired; usage text + both help snapshots updated deliberately;
`cli_surface` pin flipped. No `baseline` subcommand.

### 7. Proofs (D7/D8)
- `crates/gauntlet-cli/tests/p9_self_improvement_containment.rs` — 4 legs over
  the REAL binary + REAL composed loop (mock runner, REAL sqlite ledger,
  REAL evidence store): leg 1 weakening candidate (review.mode downgrade +
  genuine metric improvement) REJECTED with `settlement access weakened`
  recorded + rollback named + fail-closed exit; leg 2 honest candidate promotes
  through the full conjunction; leg 3 rollback durable across reopen (second
  rollback process refuses with rolled_back=true); leg 4 no-evaluation ⇒
  PROVISIONAL, never promoted, honest road still open. Each leg declares its
  tooth in the module header.
- `crates/gauntlet-app/tests/promotion.rs` `candidate_only` (AK-057 verbatim
  nextest id): behavioral containment — dispatched + document admissions leave
  the project surface (GAUNTLET.md bytes + installed-roles tree)
  BYTE-IDENTICAL, revisions sit PENDING, gate refuses promotion (provisional,
  nothing written).
- `gauntlet-arch-tests/src/optimizer.rs` `scan_optimizer_containment`: rule A
  (config-write tokens) + rule B (roles-install tokens) over
  `promote/**`; repo scan green; 3 negative controls prove rule A, rule B, and
  the no-silent-pass property. LIVE BITE PROOF EXECUTED AND REVERTED: planting
  `FlagOverrides` + `::install` in rollback.rs turned the scan RED on both
  rules; revert restored green (5/5 optimizer tests).

## Gate evidence (real numbers)
- `just check` — 0 errors
- `just lint` (clippy -D warnings + fmt) — clean
- `just test` — **1487/1487 passed** (baseline 1436 + 51 new)
- `just boundaries` — **233/233 passed**
- `just deny` — advisories/bans/licenses/sources ok
- `just ruler` — PASS (answer key + manifest digests match frozen lock)
- Plan gate: `cargo nextest run --test p9_self_improvement_containment` — 4/4;
  `cargo nextest run -p gauntlet-app promote::` — 30/30;
  `promotion::candidate_only` — 1/1.
- sqlite contract_suite — 5/5; ports revision_ledger suite legs — 3/3.

## Deviations (recorded, not smuggled)
1. **Locator divergence (charter-directed):** AK-056's
   `tests/proofs/p9_...` materializes as
   `crates/gauntlet-cli/tests/p9_self_improvement_containment.rs`; AK-057's
   `tests/contract/promotion::candidate_only` materializes verbatim as the
   nextest id `promotion::candidate_only`. Appended to
   `.agents/reports/reference-candidates.md` (AK-049 class). Ruler untouched.
2. **Improvement lever (structural finding):** the composed loop's
   per-settlement economics are scale-invariant under any inert config delta
   (1 settlement/iteration; mock invocations are free), so an inert delta can
   only improve the metric by letting the arm FINISH its task set. The proof's
   control task-set is authored with `budget.max_rounds: 2` (bounded honest
   non-settlement, units dispatched but not all settled) and the inert
   candidate raises the ceiling to 4; an arm that did not reach settled
   terminal derives the WORST figure (documented in `TargetMetric::figure`:
   the remaining work is still owed). Governance compares the dispatched-unit
   SUPERSET (tier/authority profile preservation — dispatch breadth is
   budget-driven, not a candidate-selectable governance change).
3. **Wire token `evaluation_plan`:** §19.3's bare word `test` is a denied
   adapter-distinguishing segment in the boundary scanner; the wire field is
   `evaluation_plan`, mapping table documented in `role_result.rs` (concept
   names preserved).
4. **Restore claim validation lives in the LEDGER** (the record composes any
   claimed prior state), so the suite pins "the LEDGER is the gate" for
   restores exactly as for decisions.
5. **Condition-6 protected set is declared at EVALUATION time** and recorded in
   the durable `EvaluationRecord.protected`; the gate adjudicates exactly that
   set (provenance: the operator named it when the arms ran). Undeclared set ⇒
   undecidable ⇒ provisional.
6. **Dispatched optimizer in tests:** the propose dispatch is unit-proven at
   the app layer with a scripted runner following the mock's own §10.3
   discipline; the committed runner fixtures replay builder-shaped scenarios
   (no candidate_revision scenario fixture exists and the mock is stateless
   per run), so the p9 CLI legs drive proposals via `--from-result` — the SAME
   strict decode, bypassing only the model call.
7. **Delta→env translation:** only delta keys with an Annex E variable ride the
   executor's environment; `review.mode` (GAUNTLET_REVIEW_MODE) is the leg-1
   verification-bearing downgrade; keys without a variable are still
   gate-classified but cannot be applied by this composition.

## Settle-path and ruler integrity
- `commit_settlement` + earned authority untouched (no settlement-path edits).
- `.wayfinder/answer-key.md`, `.gauntlet/reference.lock`,
  `.wayfinder/reference.yaml` untouched (`just ruler` PASS).
- No new crate, no new dependencies, no rust-toolchain/deny/nextest config
  changes.
- `reference-candidates.md` was accidentally truncated during an edit and
  immediately restored from git; final diff vs HEAD is exactly the appended
  AK-056 entry (+24 lines).

## Provisional notes
- The `propose` envelope's run-history projection is currently the
  operator-named run's durable summary (`--from-run`); a whole-history
  projection awaits a substrate enumeration read (no port exposes run
  enumeration; not invented here).
- `UnitStatus` values in checkpoints are not kept current by the loop (units
  read `open` even after settling); the terminal derivation therefore uses the
  checkpoint plan KEYS vs durable settlements, not the statuses.

## File inventory (sha256)
```
12c7862aa75d8ca83a60edc4a7270ca83d0daa37140115c4dc4b8dc9765e46cd  crates/gauntlet-ports/src/role_result.rs
860ca43427c799f7c64c95163a18709b203064e91f08f88895c76b1da513a328  crates/gauntlet-ports/src/revision_ledger.rs
c801f879f10d25c62d5e244965cea1d85f3fa33e56b7ad78b9b157f8c1eb7e27  crates/gauntlet-ports/src/contract/revision_ledger.rs
d95a1ae759af73769d5aa497b6c08acc1ad82613257236a587404b4217611564  crates/gauntlet-ports/src/contract/stubs.rs
e567b1bd77e0bb6f776e68357825ad514ec0a08169d6a111f9d5cb564bec570b  crates/gauntlet-ports/src/contract/suite_runs.rs
d02752351062167744aece341b36f2a2c12ba838cd0553db5ebc06198f11b7e8  crates/gauntlet-ports/src/contract/mod.rs
87abb79a87fa5e7609a1ee05ee6ae5e4fe2d070ee0b15dfa015e1aed15fd8f5a  crates/gauntlet-ports/src/lib.rs
de2762da7b46003e0491dd2bda09f5848a470550c0a4517446f6cb198cdaf2da  crates/adapters/gauntlet-adapter-state-sqlite/migrations/0010_revision_ledger.sql
af3858156b2fe4601b52c0bd977feca7876da7e494c2f43d8f896e5e3c4a2409  crates/adapters/gauntlet-adapter-state-sqlite/src/migrations.rs
9dadd4ed475d0ec349072a50bbe9368ab929f3a12bc8a4c0ad9a10bb0055977d  crates/adapters/gauntlet-adapter-state-sqlite/src/revision_ledger.rs
51a60889bbf626ebb0830bd4674fbb2e973ec4485f51b6869b7e5c82cc944a42  crates/adapters/gauntlet-adapter-state-sqlite/src/lib.rs
06bec7c944420e551b6394de08f5532355c5e853f8633477f24f4b7ebcf7a9f2  crates/adapters/gauntlet-adapter-state-sqlite/tests/contract_suite.rs
658d9d4180cc9dbdbb990fc74c539ac02007016f700f9bf6ef7ebcc5942c9502  crates/gauntlet-app/src/promote/mod.rs
ef3a52776db08826af7593d5f63fa8198dfc5e80ee89bce9198fd723b55a5d83  crates/gauntlet-app/src/promote/delta.rs
7e67eb0f11117b26c51b08afadeb5c440ff829aa49e3f45da127c51fc3d4defe  crates/gauntlet-app/src/promote/metrics.rs
33d87c279cd01864ea3010769f8e6fbfa2be2acd1b4b6861534a23b26b3fd630  crates/gauntlet-app/src/promote/proposal.rs
feb096200a6d2e55c807b467a8f2628e55dcda359e16e45c1374cdbb04f24637  crates/gauntlet-app/src/promote/evaluate.rs
daeeecf6f8c1310533815fa24fa308e224e287209a8b901763ee0421a10e89ca  crates/gauntlet-app/src/promote/gate.rs
037c8b2d84ad913857ea814ad70d46f8ec72bc30077b9e69339cac88201c51fc  crates/gauntlet-app/src/promote/rollback.rs
9a52274957b30699245f8cf6a080a5af4116e62ebd4a6ca267f83052563d3bee  crates/gauntlet-app/src/promote/tests.rs
78c25f7a6ad706c9d5b539926403cdc247d1de9d65049d62a3ca994699c8c754  crates/gauntlet-app/src/lib.rs
c4e82b62d5998b1c3cd2dc6d03daceb111522e14b4c3c1a6752ffc4379464e78  crates/gauntlet-app/src/config/resolve.rs
5394e609df3ce2aeb16de7d90849d66a35ece3ffacfb2ea72f056f4b5e8afc05  crates/gauntlet-app/src/config/canonical.rs
9d5d43768570b5cfd04ac34cb24c6ab16910cfbfbdc375303943c4c479b88174  crates/gauntlet-app/src/config/defaults.rs
44ab6878d7a8c53c8d8501d39b1d3df7c7a51e46a5c2f4b49593aa4c7218b43b  crates/gauntlet-app/src/control/controller.rs
12c09f957f6d12e62d88faae81c3fa95f2501089400be64bebe2b95ab211d4dc  crates/gauntlet-cli/src/improve_cmd.rs
f1b6baf9528a428afdb69689832ea484e2c555eac7fad8cbbc29fea69bf653c9  crates/gauntlet-cli/src/compose.rs
2ebc3e546989cd7e70c3282426961d2532f8bf04f885dcc58eb0c068e6705ff7  crates/gauntlet-cli/src/report_manifest.rs
9a4bf31bdef9d1c94ede28945b9cb36cc0b3116e118aa1df3c0c09d66ac82851  crates/gauntlet-cli/src/lib.rs
ee8c4b364b3f44aa969df36509bc1169a55d911a1cb92cf9b1dd33bd96c2a13a  crates/gauntlet-cli/tests/cli_help.txt
6802d5a145d44cb99f4a909eaad3f4af9ea864542a7c0bbfc296a0e0ba8f2c13  crates/gauntlet-cli/tests/cli_help_surfaces.txt
874c88bc78d3dde6c5c98cafc8d2f13b409ef423902f4897d456f4946268f379  crates/gauntlet-cli/tests/p9_self_improvement_containment.rs
cd9b3b92798b8978c90b7f2bb02b355367201fba9c0a4389c906b9609fd85af2  crates/gauntlet-cli/tests/cli_surface.rs
c49c2779441376acee962b63b5a138ceeb3eab076a616b98a00c05ab96981600  crates/gauntlet-arch-tests/src/optimizer.rs
40e1c7de9eb8f0de16c5949fa3be16d9350c7224a51ec8d4c6bdc9116fddaf4d  crates/gauntlet-arch-tests/src/lib.rs
9b49ae68345ebdbfd6292bd90f010e3b5d72c06957236335002680a4c73a820a  crates/gauntlet-app/tests/promotion.rs
54b5c3c4730af13c2db8b7ef76d4bda0f2d3c6ee6ab46c0fe307598ba6542051  docs/reference/configuration.md
b39c895f42ba6ac33f191df08ba1a0f9831ac609031c3a8bac6bb44f5a238d88  .agents/reports/reference-candidates.md
```

---

## COR-1 (2026-09-04) — recorded deviation: the domain `CandidateRevision` record is unused

Charter discovered-facts said "reuse it, do not duplicate it" and D4 said
"write-once decision via the domain record" (`crates/gauntlet-domain/src/
capability.rs:316`). As built — and unchanged by COR-1 — the domain record
is NOT on the promotion path: the floors are re-derived in the port and
gate (transition-derived decision state in the `RevisionLedger`,
`GateDecision::rejected` requiring the rollback ref, the gate's
`written:false` zero-evidence shape, `StaleDecision` claim validation).

The re-derivation is deliberate, not an oversight: D2's append-only,
transition-derived ledger design genuinely conflicts with the domain
record's mutable `decision` field — a mutable decision column would
reintroduce exactly the "silently active" state the transition history
exists to exclude (a promoted-then-restored revision must read
promoted-but-rolled-back, which a write-once mutable field cannot express
without a second mutator). The port-level floors are behaviorally pinned by
the contract suite (green/broken/caps legs) and the promote:: unit tests.

Per the charter's own rule, the deviation is recorded rather than smuggled.
The closure (and any future ruler pass over §5.3.8) should either retire the
unused domain record or re-express it as a value shape over the ledger's
history; that is a ruler-owner decision, not a correction-slice change.
