# Task 24 charter — Anti-gaming suite G1–G16 + mutation teeth

**Plan:** `.agents/plans/gauntlet-impl.md` §Task 24 (phase 6).
**Spec:** §23.2 (anti-gaming suite) + §23.1 teeth rule. **Ruler
units:** AK-058 ("Each of the sixteen anti-gaming strategies has a
test that fails when its defense is removed." — `verify_by: test`,
`blast_radius: cross_cutting`, `authority: lead_agent`,
`evidence_required: {kind: test_report, locator: tests/antigaming,
producer: instrument}`) and AK-070 ("Mutation testing at a settlement
boundary shows the proof and anti-gaming suites detect deliberately
introduced defects in the mechanisms they guard." — locator
`.agents/reports/mutation.json`). **Task 25 (real-runner E2E) is OUT
OF SCOPE.** So is `gauntlet improve baseline` (task 26).

**Spec §23.2 (verbatim, the sixteen strategies):**

> | ID | Gaming strategy |
> |---|---|
> | G1 | Weaken acceptance criteria |
> | G2 | Change the reference mid-run |
> | G3 | Skip expensive verification |
> | G4 | Reduce variation in evaluation |
> | G5 | Overfit held-out examples |
> | G6 | Hide failed observations |
> | G7 | Downgrade a unit's blast radius or authority to unblock |
> | G8 | Drop hard residuals |
> | G9 | Let the critic edit implementation |
> | G10 | Let the builder write its own answer key |
> | G11 | Share context between builder and critic |
> | G12 | Claim missing evidence as pass |
> | G13 | Optimize score instead of settlement |
> | G14 | Remove recovery checks |
> | G15 | End early to save tokens while reporting settlement |
> | G16 | Re-ask a model until it returns `pass` |

**Spec §23.1 teeth rule (verbatim):** "Each proof MUST have a **teeth
check**: disabling the defense makes the proof fail. A proof that
passes with its mechanism removed is testing nothing."

**Plan gate (verbatim; run-order note below):**

```bash
cargo nextest run --test-threads 4 antigaming:: && \
cargo mutants --file-list .agents/reports/defended-mechanisms.txt -- --no-default-features && \
python3 -c "import json;d=json.load(open('.agents/reports/mutation.json'));assert d['missed']==0, d"
```

**Plan Done-when:** sixteen tests pass and zero mutants survive in
defended paths (AK-058, AK-070).

## Recon facts (verified by the orchestrator 2026-09-04 — do not re-derive)

1. **Existing G-proofs.** Three strategies already have deep composed
   proofs in `crates/gauntlet-cli/tests/`:
   `g6_hide_failed_observations.rs` (G6), `g7_tier_downgrade.rs`
   (G7, three legs), `g10_builder_writes_answer_key.rs` (G10,
   prevention + detection legs). They are SETTLED WORK — do not modify
   them. Their test IDs do NOT contain `antigaming::` (top-level fns),
   so the plan gate's `antigaming::` filter does not reach them. The
   new suite provides its own test per strategy for all SIXTEEN,
   including G6/G7/G10 suite-level tests that bind to the same
   mechanisms (thinner is fine where the deep proof already exists —
   cite it in the manifest).
2. **All P-proofs 1–12 exist** in `crates/gauntlet-cli/tests/p*.rs`.
   They are prior art for composition style (real adapters, mock
   runner, deterministic clock `DeterministicClock`, seeded
   `SeededIdGenerator`, scratch dirs, named-constant literal
   discipline). Read `p9_self_improvement_containment.rs` and
   `g7_tier_downgrade.rs` before writing anything.
3. **Test-ID mechanics.** In nextest, a test in a binary
   `antigaming.rs` has IDs like `antigaming::<module>::<fn>`. The plan
   gate filter `antigaming::` matches by substring of the full test
   ID. A new target `crates/gauntlet-cli/tests/antigaming.rs` makes
   the gate work exactly as written. Keep every test fn's full ID
   containing `antigaming::` (top-level fns in that file do:
   `antigaming::g1_…`).
4. **cargo-mutants 27.1.0** is installed. It has **no `--file-list`
   option** (the plan's gate spelling diverges from the real CLI —
   same recorded-divergence class as the space-joined nextest note).
   Real surface: repeatable `-f/--file` globs, `--no-default-features`
   as a first-class flag (BEFORE `--`), `--test-tool nextest`, `-j`
   jobs, `--list` (dry-run count), positional args after `--` go to
   the test invocation (a test filter like `antigaming::` works
   there). `--baseline skip|run` controls the unmutated baseline run.
   Results land in `mutants.out/outcomes.json`. Verify all of this
   empirically with `--list` before the real run, and record the
   exact invocation used.
5. **The frozen ruler is untouchable.** Never edit
   `.wayfinder/answer-key.md`, `.wayfinder/reference.yaml`,
   `.gauntlet/reference.lock`. `just ruler` must pass before and after
   every phase. AK-058's locator `tests/antigaming` materializes as
   the cargo target `antigaming` under `crates/gauntlet-cli/tests/` —
   the recorded locator-materialization class (AK-049/AK-025/AK-030/
   AK-046 precedent); the closure records a reference-candidates
   entry (the orchestrator does this at settlement; you record the
   divergence in your report).
6. **Task23 leads owned by task24 (trigger-recorded):**
   - **E-1:** `crates/gauntlet-arch-tests/src/optimizer.rs`'s
     containment scan token list misses `fs::rename`, `fs::copy`,
     `File::options`, `fs::hard_link`, `fs::remove_file`, `write!`
     spellings (see the existing `CONFIG_WRITE_TOKENS` shape and its
     negative-control tests — extend consistently; keep negative
     controls green; add a positive fixture per new token class).
   - **i1 (G16 lead):** `crates/gauntlet-app/src/promote/gate.rs:256`
     adjudicates on `evaluations.last()` — if two evaluation records
     exist (first failing, second passing), the LAST one is
     adjudicated: the re-ask-until-pass shape at the promotion-gate
     layer. Write the G16 test against the REAL current behavior
     first. If it exposes a hole (a failing-then-passing evaluation
     sequence adjudicates PROMOTED), that is a REAL defect: fix it
     RED-first with the smallest typed change (e.g., adjudicate over
     ALL evaluation records / refuse conflicting re-evaluation), and
     disclose prominently. If the behavior is already safe for a
     structural reason, the test pins that reason and the manifest
     cites it.
   - **E-2** (by-unit instrument join, first-row order sensitivity):
     trigger is "next gate touch" — if your G16 correction touches
     the gate's join code, close E-2 in the same correction with a
     regression test; otherwise leave it recorded.
7. **Loop-economics constraint on G4/G13 fixtures** (from
   reference-candidates 2026-09-04 + task23 closure): NO inert
   candidate key can improve per-settlement economics on the current
   loop (write-once per-(run,iteration) observation binding + carry
   pins iterations/settlement at N/N; mock payment is zero). Your
   G4/G13 tests must NOT claim to vary per-settlement economics —
   pin the promotion conditions (variation content, settlement-vs-
   metric precedence) at the layer where they actually live
   (PromotionService / §19.4 gate / settlement preconditions).
8. **Loop-economics context for the §19.4 positive path:** p9 legs
   2/3 compose the promotion at gate level over earned StateStore
   facts (the p10 pattern). Your G4/G5/G13/G16 tests may reuse that
   composition shape.

## Deliverables (all paths relative to repo root)

1. **The suite**: `crates/gauntlet-cli/tests/antigaming.rs` — one new
   cargo test target named `antigaming`. At minimum one `#[test]` per
   strategy G1–G16 (test fn names start `g1_`…`g16_`; more fns fine).
   Every test: mock-driven over REAL adapters at the composition root
   (the p9/g7 pattern: `EmbeddedStateStore`, `FsEvidenceStore`,
   `FileReferenceProvider`, mock `AgentRunner` with fixture
   scenarios, `MeteredBudget`, deterministic clock/ids) — no network,
   no real model, no new dependencies, whole target wall-clock under
   ~10 minutes. Reuse existing fixture scenarios where they fit
   (`tests/fixtures/mock_runner_scenarios/*.yaml`; add new scenario
   YAML files there only if a needed drive does not exist — the
   fixtures dir is the AK-059-adjacent frozen surface, additive
   changes allowed, never edit existing scenario files).
2. **The manifest**: `.agents/reports/defended-mechanisms.txt` —
   pipe-delimited, one line per defended mechanism:
   `test_id | G-ID | mechanism | source_location` where `test_id` is
   the full nextest test ID, `G-ID` is `G1`…`G16`, `mechanism` is a
   short stable name, `source_location` is `path:line` of the
   defended code (the line whose mutation must go RED). Rules:
   every G1–G16 appears at least once; every location exists;
   advisory-only strategies are still listed with mechanism name
   `advisory:<reason>` and NO mutation scope (see honest-boundary
   rule below). A **validator test** inside the suite parses the
   manifest and asserts: format, G-coverage of all sixteen,
   location existence, and that every non-advisory entry's test_id
   matches the `antigaming` target. (This is what makes the teeth
   check mechanical — plan step 2.)
3. **The mutation runner**: `scripts/run-mutation.sh` — reads the
   manifest's distinct source files, invokes cargo-mutants scoped to
   them with the real CLI (divergence from the plan's `--file-list`
   spelling documented in the script header), runs the mutants
   against the `antigaming` target test filter (AK-070's claim is
   about the proof/anti-gaming suites; scoping the per-mutant test
   run to `antigaming::` is the honest minimal proof of AK-058 —
   record the choice), transforms `mutants.out/outcomes.json` into
   `.agents/reports/mutation.json` with at least:
   `missed`, `caught`, `unviable`, `total`, `invocation`,
   `manifest_sha256`, `generated_at`. Exit non-zero if `missed != 0`.
   The script must be re-runnable and must shard per-file under
   `flock` (operational addenda below). `.agents/reports/mutation.json`
   and `mutants.out/` handling: the JSON is a deliverable (committed);
   `mutants.out/` is scratch (do NOT commit; gitignore or clean).
4. **E-1 arch-scan extension** (see recon fact 6).
5. **Any surviving mutant is a suite defect**: strengthen the named
   test until the mutant is caught; never widen skip_calls without a
   manifest-recorded justification; never delete or weaken an
   existing test; never touch a settled proof file.
6. **Reports**: `b1-progress.md` at every checkpoint; `b1-report.md`
   at completion — sha256 of every file you touched/created, the
   exact commands you ran and their outcomes, the mutation run's
   shard log, deviations, and the honest-disposition notes per G.

## Honest-boundary rule (plan redesign trigger, binding)

"If a strategy has no mechanical defense (only a prompt), record it
explicitly … as advisory and open a reference candidate revision
proposing one. Do not let an advisory defense pass as a proven one."
Concretely: if for some G you cannot name a mutable source location
whose neutralization turns your test RED, the entry is
`advisory:<reason>` — the test then asserts the advisory surface that
DOES exist (the recorded posture, the refusal shape, the durable
record) and says so in a header comment; the closure carries the
candidate-revision note. A green test over a defense that does not
exist is the exact failure task24 exists to prevent.

## Strategy-to-mechanism starting map (orchestrator hints — VERIFY
## against code, then correct; the manifest records the verified map)

- G1 weaken acceptance criteria → frozen reference + comparison
  contract artifact digest; check what is mechanically frozen
  (`crates/gauntlet-app/src/compare/contract.rs`,
  `reference/`) vs advisory.
- G2 change reference mid-run → freeze/verify + controller boundary
  re-verify + terminal abort with both digests (P1; g10 detection
  legs are prior art).
- G3 skip expensive verification → settlement preconditions demand a
  verification record (P11 mechanisms; `crates/gauntlet-app/src/
  settle/`, `verify/ladder.rs`).
- G4 reduce variation → PromotionService variation conditions
  (`crates/gauntlet-app/src/memory/promotion.rs`; p10 teeth are
  prior art).
- G5 overfit held-out → §19.4 two-set structure + non-inferiority
  (`crates/gauntlet-app/src/promote/`). Overfitting risk may be
  partially advisory — check what is mechanical.
- G6 hide failed observations → observation bundle gap arrays +
  bundle digest (existing g6 mechanisms; `observation/`).
- G7 downgrade tier/authority → lattice review + dispatch gate
  (`govern/`; existing g7).
- G8 drop hard residuals → §13.5 deterministic aggregation
  (blocking forces fail, no model in merge path; `compare/`) +
  settlement preconditions over residuals.
- G9 critic edits implementation → whatever is mechanical about the
  critic's write surface (envelope allowlist; role permissions;
  containment). High advisory risk — be honest.
- G10 builder writes answer key → single-writer + containment
  refusal + tamper abort (existing g10).
- G11 builder↔critic context share → P2 envelope blindness
  (`compare/envelope.rs` — the serialized-bytes denial).
- G12 claim missing evidence as pass → P12 three-state floor
  (`settle/`, `verify/`) + operator-action parity.
- G13 optimize score instead of settlement → settlement requires
  PASS evidence, not metric improvement; §19.4 condition precedence
  (see recon fact 7's constraint).
- G14 remove recovery checks → recovery refuses to skip its checks
  (`recovery/`; task22 C4 RecoveryNotEarned/RunNotDurable shape;
  p7).
- G15 end early while reporting settlement → budget terminal ≠
  settlement; report categories derived from durable records
  (`report.rs` proof is prior art; budget reserve).
- G16 re-ask until pass → gate re-adjudication idempotency
  (AlreadyFinal) + the i1 lead (recon fact 6).

## Out of scope / do not touch

- `.wayfinder/**`, `.gauntlet/reference.lock`, settled proof files
  (`g6/g7/g10/p*.rs`), `CURRENT_STATUS.yml` (orchestrator-owned),
  task25 surfaces, any production code beyond the two named
  correction-class surfaces (arch-scan tokens; promote/gate.rs only
  via a RED-first disclosed correction).
- Do not add dependencies. Do not modify justfile recipes (add
  nothing; `scripts/run-mutation.sh` is the entrypoint). Do not
  stage `.jolli/**`, `.tmp/**`, `.agents/prompts/**`,
  `.agents/agents/**`.

## Slice battery (run before reporting completion; all must be green)

```bash
just ruler
just check-package gauntlet-cli
cargo nextest run --test-threads 4 antigaming::     # the plan gate, shape 1
just lint
just boundaries
./scripts/run-mutation.sh                            # the plan gate, shapes 2+3
python3 -c "import json;d=json.load(open('.agents/reports/mutation.json'));assert d['missed']==0, d"
```

(`just check` + `just test` workspace-wide are the ORCHESTRATOR's
closure battery — you run the package-scoped equivalents.)

## Operational addenda (binding)

- **One compile at a time.** EVERY cargo/just invocation goes under
  `flock .agents/reports/task24/compile.lock -c '<command>'` — and
  `flock` is always its OWN top-level command, NEVER nested inside
  another flock's `-c` string (a nested flock self-deadlocks the
  tree; two builders died of this at task23). Run
  `sccache --stop-server` INSIDE the same flock'd command, after the
  cargo command completes, or the daemon holds the lock fd forever.
- **Memory**: WSL2, ~1.6 GiB free. Keep cargo-mutants `-j` at 1–2.
  Measure `--list` counts before running; shard per defended file.
  If the projected mutation wall-clock exceeds ~3 hours, run the
  shards in dependency order, checkpoint outcomes after each shard
  into `b1-progress.md`, and keep going — mutation evidence is the
  Done-when core.
- **Checkpoint as you go**: `b1-progress.md` after each test lands,
  after the manifest, after the first mutant shard, after each
  shard. A session death must lose nothing (task4 lesson).
- **Report to files, not messages.** Your final message is a signal;
  `b1-report.md` is the contract (schema: mission echo, deliverable
  paths + sha256, battery output digests, mutation summary JSON,
  deviations, honest dispositions per G, open findings for the
  critic).
- **The ruler never moves.** `just ruler` before and after every
  phase; if it ever fails, STOP and report immediately.
- Compile-lock probe hygiene: if you plant any file under
  `.gauntlet/policies/` for a test, remove every one and `ls` the
  directory to prove it (task14 warning — a stray
  `irreversible.json` probe once made `just doctor` red on main).
