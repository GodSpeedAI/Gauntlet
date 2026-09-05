# Task 24 — falsifier round 1 (blind EXECUTING falsifier)

Falsifier session, 2026-09-04/05. Fresh context; blind to
`b1-*/critic-*/cor1-*` files under `.agents/reports/task24/` (never
opened). Inputs used: `falsifier-brief.md`, `charter.md`,
spec §13.5/§16/§19.4/§23.1/§23.2, ruler AK-058/AK-070 (read-only),
the delivered tree.

## Preconditions

- `just ruler` at start: PASS
  (`verify-ruler.sh: PASS — answer key and manifest digests match the
  frozen lock`). Same flock discipline as every cargo/just call below
  (`flock .agents/reports/task24/compile.lock -c '... ;
  sccache --stop-server || true'`; `run-mutation.sh` always invoked
  directly — it takes the lock itself).
- Baseline manifest sha256 (delivered):
  `625c9304606b3550a2c500bd64359da382d3b4ca2dc103294b7e405cad3102a9`
  — equals `mutation.json.manifest_sha256` (T3d pin verified at
  session start).
- Baseline sha256 of every file this session touches (recorded before
  first touch; posture.rs matches the brief's stated
  `560e0ebc…e3ee`):

| file | sha256 (baseline = restored target) |
|---|---|
| .agents/reports/defended-mechanisms.txt | 625c9304606b3550a2c500bd64359da382d3b4ca2dc103294b7e405cad3102a9 |
| crates/gauntlet-app/src/compare/contract.rs | 66bca2eddbd131f28e29e271e1a749622d30240df052042eedb733d703899e51 |
| crates/gauntlet-app/src/reference/provider.rs | 4376e391c3c17eedc7d62fc11e95e5ceda354197d70a7f41a62e6c734502095f |
| crates/gauntlet-ports/src/producer_authority.rs | d261263cdce9427ecb8de26705b9fc910ca410b9d07883680840f4eb11d8630c |
| crates/gauntlet-app/src/memory/promotion.rs | 39d7709d728172dfc8530a58fc9e38f08eaa77264a8550241a05fc4608de5b71 |
| crates/gauntlet-app/src/promote/gate.rs | 953cf2a8c6598fee9a3fa49e75ff9f08e15bc1f353674a6ac3109dcabc6a17ff |
| crates/gauntlet-ports/src/observation_digest.rs | 05444185efdcd5e9517aa766f4b212893f4afe5e5d41294c4394f44eabc0b3cc |
| crates/gauntlet-app/src/govern/posture.rs | 560e0ebc0dfff36c593355f14f07f458bea9f6f9f5edf4d9ba1b56fa7e17e3ee |
| crates/gauntlet-app/src/compare/aggregate.rs | 69b518344ff740439b1979264363e57185c9d1fc9d9353c6104b5c1e1226fde8 |
| crates/gauntlet-ports/src/comparator.rs | 1b225710f5513c2d39fd4996643de09cf9deb7119283b97935ba2caa2b093a9c |
| crates/gauntlet-app/src/promote/delta.rs | 4276075ececd7e80b08b0995a8162786ca6c1e264301cceb7995a5d37b30b9bc |
| crates/gauntlet-app/src/memory/developmental.rs | 71c0037d2358961c8099f6587a2fb3b8ed5525198caeb2f5950a3da10a69cf96 |
| crates/gauntlet-app/src/report/mod.rs | d5d9b4e8e89125b8c2f71603f514e8dd59de60d4b1efdc2e8f64651f23bf1a0e |
| crates/gauntlet-app/src/roles/install.rs | be66b3a2de0fbe49f4839a635d0528284fd351cc4008ec32220b11199310494a |
| crates/gauntlet-cli/tests/antigaming.rs | b32649694a7b6a95ec3f4f14171a76e0c1c01ee7239ea541ec28e6a858e09a81 |

(This file is appended per attack as they execute — checkpoint
discipline.)

## T1 — per-strategy teeth (AK-058's core claim)

Method per attack: backup file to `/tmp/gauntlet-falsifier/` (outside
the repo), apply ONE minimal hand neutralization at/near the
manifest's named source_location, run the named test via
`flock .agents/reports/task24/compile.lock -c 'cargo nextest run
--test-threads 4 -p gauntlet-cli --test antigaming <fn-substring>;
sccache --stop-server || true'`, record RED/GREEN, restore from
backup, verify sha256 equals the baseline above.

Baseline before any attack: `cargo nextest run --test-threads 4
-p gauntlet-cli --test antigaming` → **18/18 PASS**.

| # | G | neutralization (named defense) | named test result | restore proof |
|---|---|---|---|---|
| A1 | G1 | `contract.rs` rubric-coverage loop (manifest :347) → `if false && !rubric.iter().any(...)` (missing-severity refusal dead) | **RED** — `a rubric without every severity is refused: ComparisonContract{…}` panicked at `antigaming.rs:1179` (the weakened contract parsed) | sha256 `66bca2ed…9e51` ✔ |
| A2 | G2 | `provider.rs:318` `verify_unchanged` digest compare `==` → `!=` (tripwire inverted) | **RED** — panicked at `antigaming.rs:1235` (unmoved artifact no longer verified unchanged) | sha256 `4376e391…095f` ✔ |
| A3 | G3 | `producer_authority.rs:701` no-records arm `if unit_records.is_empty()` → `if false && …` | **RED** — refusal detail degraded to `"hold outcomes  — none is PASS"`; assertion at `antigaming.rs:2042` failed (named detail gone) | sha256 `d261263c…630c` ✔ |
| A4 | G4 | `promotion.rs:497` variation axis compare `!=` → `==` (identical axes = "real variation") | **RED** — `insufficient_variation` refusal vanished (refusals `[NoRecoveryObserved]` only), assertion at `antigaming.rs:2302` failed | sha256 `39d7709d…5b71` ✔ |
| A5 | G5 | `gate.rs:780` `regression_verdict` → early `Ok(())` (regression-suite condition dead) | **RED** — skipped regression unit **PROMOTED** (`regression_units_treatment_settled: 2` vs control 3), panic at `antigaming.rs:979` | sha256 `953cf2a8…17ff` ✔ |
| A6 | G6 | `observation_digest.rs` `normalized_bytes` drops `"observation_failures"` key (gap arrays exempted from digest) | **RED** — digests coincided, `assert_ne!` at `antigaming.rs:1340` failed | sha256 `05444185…b3cc` ✔ |
| A7 | G7 | `posture.rs:142` AgentRole arm → early `return LatticeMovement::Allowed` (agent downgrade permitted) | **RED** — `an agent tier downgrade must be refused, got Allowed` at `antigaming.rs:1429` | sha256 `560e0ebc…e3ee` ✔ |
| A8 | G8 | `aggregate.rs:383` verdict rule `blocking ||` dropped (§13.5 blocking-forces-fail dead) | **RED** — blocked world aggregated `Pass` with a blocking residual (`left: Pass, right: Fail`) at `antigaming.rs:1514` | sha256 `69b51834…6fde8` ✔ |
| A9a | G9 | ADVISORY-CLAIM attack 1: forge enforcement — `install.rs:116` `AdvisoryPromptArtifacts => Self::Writable` (lock would record "writable") | **GREEN** (expected — entry is declared `advisory:` with no mutation scope; no test asserts the lock record) | sha256 `be66b3a2…494a` ✔ |
| A9b | G9 | ADVISORY-CLAIM attack 2: widen the `FilesystemMode` closed vocabulary — `definition.rs:76` adds `"run-state" => Some(Self::ReadWrite)` | **RED** — `a mode word outside the vocabulary is refused` at `antigaming.rs:2562` (the refusal-to-widen posture the test DOES claim is load-bearing) | sha256 `c3300e62…b8bcc` ✔ |
| A10 | G10 | `provider.rs:267` freeze-boundary guard `!=` → `==` (tampered key freezes) | **RED** — `a tampered standard must never freeze as the run's ruler, got Ok(ReferenceVersion{…})` at `antigaming.rs:1297` | sha256 `4376e391…095f` ✔ |

A9 judgment (the brief's G9 direction): the advisory entry HONESTLY
describes reality. The runtime genuinely has no mechanical write-scope
enforcement for a critic-context process (nothing at the composition
root gates a critic's writes); the recorded posture that DOES exist
(closed two-token `FilesystemMode` vocabulary, typed
`UnknownFilesystemMode` refusal) is real and its neutralization turns
the test red (A9b); the `Enforcement::Advisory` recording at
`install.rs:113/116` exists and is recomputed by the roles-verify path
(`install.rs:895`), and no test or manifest line claims more than
advisory for it. The g9 test header and manifest text match observed
reality. Finding F-1 below (INFORMATIONAL).

Attacks A11–A16 follow.

| # | G | neutralization (named defense) | named test result | restore proof |
|---|---|---|---|---|
| A11 | G11 | `comparator.rs:1494` deny-list match `if key == denied` → `if false && key == denied` (structural blindness dead) | **RED** — injected builder field accepted, `expect_err` panicked at `antigaming.rs:1837` | sha256 `1b225710…3a9c` ✔ |
| A12 | G12 | `producer_authority.rs:715` no-PASS arm `if pass_records.is_empty()` → `if false && …` | **RED** — FAIL record's refusal degraded to the evidence-mooring detail; `antigaming.rs:2112` assertion failed (named `none is PASS / FAIL` detail gone) | sha256 `d261263c…630c` ✔ |
| A13 | G13 | `delta.rs:159` `settlement_access_verdict` → early `Ok(())` (AK-056 classification dead) | **RED** — weakening delta no longer refused by name; verdict degraded to Provisional (unrelated gap), `antigaming.rs:1049` panic | sha256 `4276075e…b9bc` ✔ |
| A14 | G14 | `developmental.rs:304` `completed == 0` → `completed == usize::MAX` (zero-completed credit admitted) | **RED** — unearned credit accepted: `unearned recovery credit is refused: ()` panic at `antigaming.rs:2365` | sha256 `71c0037d…cf96` ✔ |
| A15 | G15 | `report/mod.rs:493` proven-pair outcome test `== Outcome::Pass` → `!=` (outcome test flipped, per the test header's named attack) | **RED** — positive control broke: `the honest settlement reads implemented-and-proven` assertion failed at `antigaming.rs:2461` | sha256 `d5d9b4e8…f1a0e` ✔ |
| A16 | G16 | `gate.rs:286` sweep bound `take(evaluations.len() - 1)` → `take(0)` (re-ask floor dead — adjudicates last record only) | **RED** — the exact §23.2 G16 shape: `a failing evaluation was laundered by a passing re-run — the re-ask-until-pass shape adjudicated PROMOTED` at `antigaming.rs:840` | sha256 `953cf2a8…17ff` ✔ |
| A17 | G16 (2nd entry, `gate.rs:837` span interior) | sweep fn margin arm: `!margin_holds(...)` conjunct → `false && …` (earlier-record margin failures never swept) | **RED** — laundering test again RED at `antigaming.rs:840`, while `g16_the_honest_road…` stayed GREEN (the honest road correctly does not depend on the margin arm) — two-leg discrimination holds | sha256 `953cf2a8…17ff` ✔ |

**T1 verdict: 16/16 strategies attacked. Every named non-advisory
defense is load-bearing (test RED when neutralized); the G9 advisory
entry honestly describes reality and its posture assertion has real
teeth (A9b RED). No §23.1 "passes with its mechanism removed" failure
was found in the named defenses.**

## T2 — manifest attacks

Validator attack surface:
`the_defended_mechanisms_manifest_covers_all_sixteen_strategies`
(manifest edits need no recompile — the validator reads the file at
test time). Restores verified by sha256 after each direction.

| # | corruption direction | validator result |
|---|---|---|
| T2a | G4 entry deleted (coverage hole) | **RED** — `strategy G4 has a manifest entry (all sixteen are covered)` at `antigaming.rs:2668` — coverage check enforced |
| T2b | G11 test_id moved to a foreign target (`p2_context_isolation::g11_…`) | **RED** — `a non-advisory entry's test_id must be in the antigaming target` at `antigaming.rs:2633` — target binding enforced |
| T2c | G8 source_location → `aggregate.rs:100` (exists; contains `struct MergedResidual {`, unrelated) | **PASS SILENTLY** → finding F-2 |
| T2d | G5 test_id → `antigaming::g4_identical_content_axes_…` (right target, WRONG fn, cross-wired mechanism) | **PASS SILENTLY** → finding F-2 |

False-justification direction (no corruption possible to execute —
verified instead): all four recorded exclusions were re-derived from
source and NONE is false:
- G7 (`posture.rs:136:42/139:50` `<`→`<=` equivalent): each `.map`
  closure evaluates only when its own axis MOVED (`!=` at :130/:131),
  so on every evaluation `to != from` and `to <= from` coincide; the
  non-moving side is `unwrap_or(false)`. Sound.
- G4 (`settled_with_real_variation → false`): body-false can only
  over-refuse variation (fail-safe); it cannot admit a promotion.
  Sound.
- G8 (`aggregate.rs:381/385`): dispute/incomplete-gate routing, a
  completeness concern outside the G8 blocking-forces-fail claim at
  :383. Sound (scope, not equivalence — as the manifest says).
- G15 (`report/mod.rs:493:57` `&&`→`||`): every durable settlement
  row is produced only through `SettlementCommitAuthority::admit`
  (which moors the row's verification ref to a durable PASS record
  before `commit_settlement` accepts the admitted handle), so in every
  reachable store state the ref-match disjunct is already true and the
  mutant is indistinguishable. Sound.

Attacks T3 onward follow.

## T3 — pipeline attacks (AK-070)

- **T3a-1 (does the pipeline catch, with the real suite?)** — ran the
  G8 shard MANUALLY with the script's exact cargo-mutants arguments
  (`--no-config --baseline skip -j 2 --test-tool nextest
  --no-default-features -f …/aggregate.rs -F ":(383|…|390):"
  -E "compare/aggregate.rs:38[15]:" -- -p gauntlet-cli --test
  antigaming`), under the repository flock, script NOT involved:
  `Found 1 mutant to test … 1 caught`, `MUTANTS_EXIT:0`,
  `CaughtMutant … aggregate.rs:383:30: replace || with && in
  aggregate`. Cross-checks mutation.json's aggregate shard
  (examined 1 / caught 1) exactly.
- **T3a-2 (does a surviving mutant fail loudly?)** — neutralized the
  g8 test's central assertion IN PLACE (expected `Verdict::Pass` — a
  temporary test weakening, restored after), re-ran the same shard:
  `MISSED … aggregate.rs:383:30 … in 148s build + 7s test`,
  `1 mutant tested … 1 missed`, **`MUTANTS_EXIT:2`** (nonzero), and
  outcomes.json recorded `MissedMutant`. The script's merge reads
  exactly these outcomes (`normalize` maps MissedMutant→missed) and
  ends `sys.exit(0 if missed == 0 else 1)` — so a survivor is loud at
  BOTH layers. antigaming.rs restored: sha256 `b3264969…09a81` ✔.
- **T3b (bogus zero-test "caught" class?)** — no. The pinned test
  args append `-p gauntlet-cli --test antigaming` (script divergence
  note 4); empirically, T3a-2's MISSED outcome is only producible if
  the Test phase really executed the suite (a vacuous zero-test run
  would have reported the mutant "caught"); T3a-1's 7s caught phase
  matches a real 18-test run. The two script failure modes that
  produce vacuous caughts (missing `-p`, `antigaming::` positional
  matching zero tests) are defended by the pinned args and
  fail-closed respectively.
- **T3c (merge counts honestly?)** — verified by reading the merge:
  `normalize` covers Caught/Missed/Unviable/Timeout/Skipped/NotCovered
  (+ unknown→unknown, still counted in `statuses` and `total`);
  `total` sums every examined outcome; per-shard `shard_exit` recorded;
  `missed_mutants` names each survivor; the single gate is merged
  `missed == 0`. Unviable/timeout outcomes are counted and surfaced
  (mutation.json: 10 unviable across shards), never silently dropped.
- **T3d (does manifest_sha256 pin the delivered manifest?)** — yes:
  `mutation.json.manifest_sha256 = 625c9304…02a9` equals
  `sha256sum .agents/reports/defended-mechanisms.txt` at session start
  and after every restore. The runner recomputes it at invocation
  (`MANIFEST_SHA256`), so a re-run over a moved manifest writes a
  different pin — stale evidence is detectable.

**T3 verdict: the mutation pipeline is honest. No finding.**

## T4 — promotion-gate sweep adversarial shapes

Executed as a temporary probe batch appended to `antigaming.rs`
(restored byte-identically afterward, sha256 `b3264969…09a81` ✔).
Reused the file's real composition (GateWorld, EconomyExecutor,
harness, gate).

| probe | shape | result |
|---|---|---|
| T4-1 | poisoned earlier record (unparseable metric + treatment payment INCREASED) + clean complete re-roll | **PASS** — `REJECTED` with exactly one reason, `payment_increased`: the payment floor survives the unparseable metric; the re-roll launders nothing |
| T4-2 | re-adjudication after a final REJECTED decision | **PASS** — typed `AlreadyFinal` (write-once covers rejections, not only promotions) |
| T4-3 | two failing re-rolls + one passing re-roll | **PASS** — `REJECTED` with exactly TWO `margin_not_met` (each earlier failure recorded once; no laundering, no loss) |
| T4-4 | earlier-record FAILURE + fresh-record GAP | **PASS** — `REJECTED` (never Provisional): the earlier failure rides; the fresh gap is superseded by the failure's precedence. (Probe note: the first probe run asserted the gap would also appear in the rejection reasons — it does not; the rejection carries only failed conditions. That is the conservative direction — REJECTED is terminal, Provisional is not — so this is probe miscalibration, not a gate defect; corrected probe passes.) |

Failed⟹Rejected / gaps⟹Provisional for EVERY condition (code
verification, `gate.rs` adjudicate): conditions 1, 2, 3(series),
4, 5, 7, 8 push to `failed`; 3(series-undecidable), 6, 7
(underivable), 8 (underivable) push to `gaps`; the sweep pushes ONLY
failures (gaps deliberately excluded, documented in its doc comment).
Verdict assembly: `if !failed.is_empty() → Rejected; else if
!gaps.is_empty() → Provisional; else Promoted` — an earlier-record
failure structurally can never leak into a Provisional (probe T4-4
executes this).

**Two-sweep-shard boundary (280:292 vs 842:912):** gate.rs lines
~293–841 (plus 912+) sit between/outside the shards — the current-half
condition bodies, `governance_verdict`, `instrument_series_verdict`
(780 region partly), `margin_holds` (~919+), `protected_verdict`.
Mutations there are NOT examined by the mutation pipeline. This is NOT
a claim violation: mutation.json's scope_note declares examined =
defended loci from the manifest; every manifest-named locus IS inside
an examined span (verified line-by-line, incl. the manifest-line →
span table: contract 307–360, provider 260–325, producer 681–790,
promotion 489–501, gate 781–800 + 280–292 + 842–912, digest 29–80,
posture 130–150, aggregate 383–390, comparator 1487–1510, delta
160–180, developmental 304–306, report 491–495). Behaviorally, several
between-shard mutations ARE suite-detectable (A5/A17 prove the
regression condition and sweep margin arm; the g16 honest-road leg
detects condition-6 mutations), while others (e.g. `governance_verdict`
interior) are outside both the pipeline's scope and this suite's
authored shapes — recorded as finding F-3 (INFORMATIONAL, scope
honest, no overclaim).

## T5 — regression sweep on the restored tree (all executed after the
## final restore, under the compile lock)

| battery | result |
|---|---|
| `cargo nextest run --test-threads 4 --test antigaming` | **18/18 PASS** |
| `cargo nextest run -p gauntlet-app promote::` | **46/46 PASS** (517 skipped) |
| `cargo nextest run -p gauntlet-cli --test improve_promote_gates` | **1/1 PASS** |
| `cargo nextest run -p gauntlet-cli --test p9_self_improvement_containment` | **4/4 PASS** |
| `just boundaries` | **237/237 PASS** |
| `just lint` | **clean** (clippy finished; `cargo fmt --check` passed) |
| `just ruler` | **PASS** (answer key and manifest digests match the frozen lock) |

## Restore proof (final)

Every file this session touched, hashed after the final restore — all
equal their session-start baselines (recorded above):

defended-mechanisms.txt `625c9304…02a9` · contract.rs
`66bca2ed…9e51` · provider.rs `4376e391…095f` · producer_authority.rs
`d261263c…630c` · promotion.rs `39d7709d…5b71` · gate.rs
`953cf2a8…17ff` · observation_digest.rs `05444185…b3cc` · posture.rs
`560e0ebc…e3ee` · aggregate.rs `69b51834…6fde8` · comparator.rs
`1b225710…3a9c` · delta.rs `4276075e…b9bc` · developmental.rs
`71c0037d…cf96` · report/mod.rs `d5d9b4e8…f1a0e` · install.rs
`be66b3a2…494a` · roles/definition.rs `c3300e62…b8bcc` ·
tests/antigaming.rs `b3264969…09a81`.

`git status --porcelain` after the sweep is IDENTICAL to session
start: the tracked modifications vs HEAD (gate.rs +117,
optimizer.rs +173, .gitignore, CURRENT_STATUS.yml, .jolli/*) are the
PRE-EXISTING delivered task24 diff and orchestrator/jolli-owned files
— this session's net tracked-file delta is ZERO. Scratch: `mutants.out`
removed (did not exist at session start); `.gauntlet/policies/`
contains only the three pre-existing tier policies (nothing planted).
`/tmp/gauntlet-falsifier/` (backups) is outside the repository.
Blindness rule honored: the `b1-*/critic-*/cor1-*` files were never
opened (directory names only, via `ls`).

## Findings

| id | severity | finding | evidence |
|---|---|---|---|
| F-1 | INFORMATIONAL | The G9 advisory entry honestly describes reality. No mechanical critic write-scope enforcement exists; the recorded posture is real and its refusal-to-widen has teeth (A9b RED). A forged-enforcement mutation (`install.rs:116` Advisory→Writable) stays green — expected and honest, since the entry declares `advisory:` with no mutation scope and no test claims the lock record as enforcement. | A9a/A9b; `install.rs:895` (roles-verify recomputes enforcement) |
| F-2 | MINOR | The manifest validator's bindings are prefix/bounds checks, not identity checks: (i) a `source_location` that exists but contains something else passes silently (T2c: G8 → `aggregate.rs:100`, `struct MergedResidual`); (ii) a cross-wired `test_id` (right target, wrong fn) passes silently (T2d: G5 → the g4 fn). Nothing downstream consults the manifest's line content or fn identity (`run-mutation.sh`'s scope table is hand-maintained and does not parse manifest lines), so the delivered manifest's factual correctness — which this session PROVED by executing all sixteen neutralizations — rests on human discipline. Hardening candidate: hash/normalize the named line's content, or derive run-mutation.sh spans from the manifest. | T2c/T2d outputs; `antigaming.rs:2648–2659` (bounds check); `run-mutation.sh:77–131` (handwritten scope table) |
| F-3 | INFORMATIONAL | The mutation pipeline's examined scope is exactly the manifest loci (honestly declared in mutation.json's scope_note); gate.rs lines between the two G16 shards (`governance_verdict`, `margin_holds`, `protected_verdict`, current-half condition bodies) are unexamined. No claim is violated — AK-058 is per-strategy and every named defense is both examined and execution-proven load-bearing — but defects injected there would be caught by neither the mutation gate nor this suite's authored shapes (some are covered by other suites: compare-module units, p-proofs). | T4 boundary analysis; mutation.json scope_note |

No CRITICAL finding. No MAJOR finding.
(One probe miscalibration during T4-4 was the probe's own assertion,
not a suite or gate defect — corrected in place and re-run PASS.)

## Verdict

**CONFIRM** — no finding ≥ MAJOR. The delivered anti-gaming suite is
what it claims: AK-058's per-strategy teeth hold under execution
(16/16 neutralizations turned the named tests RED), AK-070's mutation
evidence is honest (survivors fail loudly, catches are real, the
manifest pin holds), and the G16 re-adjudication discipline survived
every adversarial shape tried.
