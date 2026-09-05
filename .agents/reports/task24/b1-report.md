# B1 report — Task 24: anti-gaming suite G1–G16 + mutation teeth

**Status: COMPLETE.** All deliverables landed; the full slice battery is
green; the mutation run shows **missed = 0** over examined defended loci.

## Mission echo

Built the anti-gaming suite proving AK-058 (each of the sixteen §23.2
strategies has a test that fails when its defense is removed) and AK-070
(mutation testing at a settlement boundary shows the proof/anti-gaming
suites detect deliberately introduced defects in the mechanisms they
guard): one new cargo test target `crates/gauntlet-cli/tests/antigaming.rs`
(18 tests, one per strategy plus a two-leg G16 and the manifest
validator), the machine-readable teeth map
`.agents/reports/defended-mechanisms.txt`, the re-runnable
`scripts/run-mutation.sh` producing `.agents/reports/mutation.json`, the
E-1 arch-scan token extension, and the G16 i1 lead
(RED-first, confirmed as a REAL defect, fixed, disclosed below).

## Files created / modified (sha256)

| File | Status | sha256 |
|---|---|---|
| `crates/gauntlet-cli/tests/antigaming.rs` | created | `8cd0bd9a09c31b63d4ca3f2676f485786bf347f2d8298e0ba4dd88838187e126` |
| `crates/gauntlet-arch-tests/src/optimizer.rs` | modified (E-1) | `c78192efda231710c46244047272914cf30202baa0e8df6b045cb4be895a19e3` |
| `crates/gauntlet-app/src/promote/gate.rs` | modified (G16 fix) | `00cac8e38b77ad240419190551ef93941064c904f10f593e37951a8aa883f5a5` |
| `.agents/reports/defended-mechanisms.txt` | created | `0120e3544c91d5a78ac8eea2fbf5b603510fac0d6fa755850e0fb97614a5b49e` |
| `scripts/run-mutation.sh` | created | `a849b6c260103dd36acb1bd56f9b19067e527c96e9dd4fe1c1ae5d2ecbb1e8a3` |
| `.agents/reports/mutation.json` | generated deliverable | `015432e1446c47994f1366d7e01399a0f832f28babf5c2200a41df4864ee8218` |
| `.agents/reports/task24/b1-progress.md` | checkpoints 1–6 | `fe9c904d6516bbda8f744f4716ac98406aa08c65b8c420630b2e903e3dc306b1` |
| `.gitignore` | modified (one entry: `mutants.out/`) | `ce782fc5deb216059f7e52facd3cbd5e853ed24e0115d408781b827109dee011` |

Scratch (regenerable, not for commit):
`.agents/reports/task24/mutation-scratch/` (per-shard outcomes),
`.agents/reports/task24/mutation-run.log`. Settled surfaces
(g6/g7/g10/p*.rs, `.wayfinder/**`, `.gauntlet/reference.lock`,
`CURRENT_STATUS.yml`, task25 surfaces) untouched.

## The suite (18 tests, `antigaming` target, all green, ~0.2 s wall)

g1 contract-parse rubric coverage · g2 verify_unchanged I1 tripwire ·
g3 settlement admission (no verification record) · g4 recorded-variation
content axes · g5 gate regression-suite condition · g6 observation-digest
gap-array coverage · g7 lattice review · g8 governed-chain aggregation
(blocking residual forces Fail) · g9 ADVISORY (recorded posture) ·
g10 freeze-boundary tripwire · g11 structural blindness over serialized
bytes · g12 admission three-state floor + §16.3 mooring/replay legs ·
g13 settlement-access precedence over the margin · g14 recovery-credit
floor · g15 report proven-pair classification · g16 two legs (laundering
refused; honest road preserved + AlreadyFinal) · manifest validator.

## Slice battery — exact commands and outcomes

| Command | Outcome |
|---|---|
| `just ruler` (before phases) | PASS |
| `just ruler` (after mutation run) | PASS |
| `just ruler` (post-battery) | PASS |
| `just check-package gauntlet-cli` | Finished, no errors/warnings |
| `cargo nextest run --test-threads 4 --test antigaming` (plan gate shape; see divergence D1) | 18 run, 18 passed |
| `python3 -c "...assert d['missed']==0..."` on mutation.json | OK (missed == 0) |
| `just lint` (clippy -D warnings + fmt --check) | both green (after `cargo fmt`) |
| `just boundaries` | 237 run, 237 passed |
| `./scripts/run-mutation.sh` | exit 0; missed == 0 |

## Mutation summary (`.agents/reports/mutation.json`)

```json
{
  "total": 61, "missed": 0, "caught": 54, "unviable": 7,
  "timeout": 0, "skipped": 0, "not_covered": 0,
  "manifest_sha256": "0120e3544c91d5a78ac8eea2fbf5b603510fac0d6fa755850e0fb97614a5b49e",
  "generated_at": "2026-09-05T03:11:29Z",
  "invocation": "scripts/run-mutation.sh (cargo-mutants 27.1.0, --no-config --baseline skip -j 2 --test-tool nextest --no-default-features -f <defended file> -F <scope span> [-E <justified exclusion>] -- -p gauntlet-cli --test antigaming)"
}
```

Per-shard: contract 2/2, provider 3/3, producer_authority 12 caught + 1
unviable, promotion 2/2, gate-regression 4/4, observation_digest 8 + 2
unviable, posture 6 + 1 unviable, aggregate 1/1, comparator 7/7, delta
2/2, developmental 2/2, report 2 + 2 unviable, gate-sweep 2/2,
gate-earlier_record_failures 1 + 1 unviable. **Every caught mutant's Test
phase executed the 18-test suite** (durations 5–30 s verified, not the
0.3 s no-tests failure class).

## Per-G honest dispositions

| G | Mechanism (defended locus) | Disposition |
|---|---|---|
| G1 | contract.rs:347 rubric-coverage validation in `ComparisonContract::parse` | mechanical — mutation-examined, caught |
| G2 | provider.rs:318 `verify_unchanged` digest tripwire | mechanical (composed loop leg = g7 leg 3 + p1, cited) |
| G3 | producer_authority.rs:701 admit no-records arm | mechanical |
| G4 | promotion.rs:486 `settled_with_real_variation` | mechanical (pins the §19.2 variation condition; no per-settlement-economics claim — recon fact 7 honored) |
| G5 | gate.rs:780 `regression_verdict` | mechanical |
| G6 | observation_digest.rs:29 gap-array coverage | mechanical (composed observer proof = g6, cited) |
| G7 | posture.rs:124 `review_lattice_movement` | mechanical (dispatch conjunct + frozen-ruler leg = g7, cited) |
| G8 | aggregate.rs:383 blocking-forces-fail rule | mechanical, exercised through the governed chain's public route |
| G9 | **ADVISORY** | `advisory:` in the manifest. Reason: the runtime has NO mechanical write-scope enforcement for a critic-context process; the role file's `filesystem: read-only` is a recorded typed posture (closed vocabulary, refusal-to-widen) and `Enforcement::Advisory` is the recorded-weakness class. The g9 test asserts exactly that advisory surface. Reference candidate for closure: runtime write-scope enforcement for role contexts. |
| G10 | provider.rs:267 freeze-boundary tripwire | mechanical (prevention leg = g10's composed proof, cited) |
| G11 | comparator.rs:1486 `assert_blind` | mechanical (composed envelope bytes proof = p2, cited) |
| G12 | producer_authority.rs:715 no-PASS arm | mechanical (operator parity + every-layer INCONCLUSIVE = p12, cited) |
| G13 | delta.rs:159 `settlement_access_verdict` | mechanical (AK-056 precedence; full-loop shape = p9 leg 1, cited) |
| G14 | developmental.rs:304 recovery-credit floor | mechanical (crash/resume exactly-once = p7, cited) |
| G15 | report/mod.rs:485 proven-pair | mechanical |
| G16 | gate.rs:263 re-ask floor | mechanical — REAL DEFECT FOUND AND FIXED (below) |

## The G16 i1 finding — REAL DEFECT, fixed RED-first

**RED (verbatim, before the fix):** `a failing evaluation was laundered by
a passing re-run — the re-ask-until-pass shape adjudicated PROMOTED:
Promoted(RevisionPromotionEvidence { metric: "payment_per_settlement",
control_figure: 30000000, treatment_figure: 20000000, ... })`

`PromotionGate::adjudicate` read `evaluations.last()` alone
(gate.rs:256). A failing evaluation followed by a cleaner re-run
(the §23.2 G16 "re-ask until pass" shape, unadjudicated in between)
adjudicated PROMOTED on the re-roll.

**Fix (smallest typed change):** the adjudication now sweeps EVERY earlier
evaluation record's record-derived FAILED conditions
(`earlier_record_failures`: conditions 1, 3-series, 5, 7, 8) into the
rejection reasons, each reason naming the earlier record's held-out runs.
Evidence GAPS and input/ledger-derived conditions (2, 4, 6) are NOT
swept — gaps are superseded by the freshest record (the honest road stays
open, pinned by `g16_the_honest_road_...` which was green before AND
after the fix), and 2/4/6 are only honestly comparable over current
inputs. Post-fix: antigaming 4/4 gate legs green; `improve_promote_gates`
1/1; p9 4/4; gauntlet-app promote unit tests 49/49.

## Deviations (recorded)

- **D1 — plan-gate filter spelling:** `cargo nextest run antigaming::`
  matches zero tests from the workspace root: a positional containing
  `::` parses as nextest's filter-DSL package selector, and the binary
  name is not a `::`-component of nextest test ids (ids display as
  `gauntlet-cli::antigaming <fn>`). The battery uses the binary-scoped
  gate `cargo nextest run --test-threads 4 --test antigaming`, which runs
  exactly the suite. (Same recorded-divergence class as the plan's
  `--file-list` spelling.)
- **D2 — `--file-list` does not exist** in cargo-mutants 27.1.0; the
  script uses repeatable `-f` + `-F`/`-E` (documented in the script
  header per the charter).
- **D3 — scoped examination:** mutants are examined per manifest locus
  (defended-function line spans), not whole files: AK-070's claim is
  about the mechanisms the suites guard. Three exclusions carry
  manifest-recorded justifications (defended-mechanisms.txt comments):
  G4's fail-safe `with false` direction; G15's unreachable `493:57 ||`
  flip (COR-3 mooring makes the distinguishing state unproducible); G7's
  two equivalent rank-arm mutants (`<=`/`==` coincide with `<` under the
  movement guard). G8's span is narrowed to the verdict rule (381/385
  dispute bookkeeping is outside the claim; covered by the compare
  module's own unit tests).
- **D4 — appended `-p gauntlet-cli`** to cargo-mutants' test args: it
  pins the test invocation to the mutated package, and the antigaming
  target lives in gauntlet-cli — without this, EVERY mutant reports
  "caught" through a zero-test run. The Task-24 anti-failure lesson
  applied to the evidence pipeline itself: two such artifact runs were
  detected (Test phase 0.3 s / exit 101) and discarded before the honest
  run whose results stand above.
- **D5 — G16 sweep bound lines** are examined (they are part of the
  re-ask floor); the `with +` mutant is caught by the exactly-one-reason
  assertion in g16 leg A.
- Suite header note: no leg dispatches an agent (p10 precedent); the
  charter's mock-runner inventory applies to loop-level tests, and the
  deep proofs that do dispatch are cited. DeterministicClock /
  SeededIdGenerator-style determinism comes from the mock adapter's
  clock; deterministic fixtures throughout.

## Open findings for the critic

1. **G9 advisory posture** — the runtime cannot mechanically prevent a
   critic-context process from writing implementation files. The
   candidate revision (runtime write-scope enforcement for role
   contexts) should be opened at closure (orchestrator-owned).
2. **G16 fix breadth** — the sweep covers record-derived conditions
   (1, 3-series, 5, 7, 8). Conditions 2/4/6 are re-derived only for the
   freshest record by design (input/ledger-derived). If the ruler-owner
   wants earlier records' digests checked against THEIR OWN freeze-time
   digests, that is a separate ports-revision candidate.
3. **Unviable mutants (7)** are recorded per shard in mutation.json —
   they are type-level impossibilities (e.g., `Default` for enums
   without one), not suite gaps.
4. `improve evaluate` records an evaluation per invocation; repeated
   evaluations accumulate records by design. The gate now adjudicates
   all records' failures; an operator seeing a Rejected reason naming
   "an earlier evaluation" should re-run a fresh evaluation only after
   genuinely addressing the failure.
5. Battery note: the charter's battery line
   `cargo nextest run --test-threads 4 antigaming::` was replaced by
   `--test antigaming` (D1); every other battery line ran as written.
