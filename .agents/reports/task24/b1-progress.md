# B1 progress — task24 anti-gaming suite

## Checkpoint 1 (2026-09-04) — recon done, E-1 landed

- `just ruler` baseline: PASS (answer key + manifest digests match the
  frozen lock). Every cargo/just invocation flocked, sccache stopped
  inside the same flock'd command.
- Recon complete. Mechanism map (verified against source):
  - G1 `crates/gauntlet-app/src/compare/contract.rs:347` (rubric
    coverage loop in `ComparisonContract::parse`)
  - G2 `crates/gauntlet-app/src/reference/provider.rs:318`
    (`verify_unchanged` digest compare)
  - G3 `crates/gauntlet-ports/src/producer_authority.rs:701`
    (`SettlementCommitAuthority::admit` — no verification record at all)
  - G4 `crates/gauntlet-app/src/memory/promotion.rs:486`
    (`settled_with_real_variation`)
  - G5 `crates/gauntlet-app/src/promote/gate.rs:756` (`regression_verdict`)
  - G6 `crates/gauntlet-ports/src/observation_digest.rs:29`
    (`normalized_bytes` — gap-array coverage)
  - G7 `crates/gauntlet-app/src/govern/posture.rs:124`
    (`review_lattice_movement`)
  - G8 `crates/gauntlet-app/src/compare/aggregate.rs:383`
    (`blocking || any_failed_gate → Verdict::Fail`)
  - G9 ADVISORY — no runtime write-scope enforcement for the critic
    exists; `roles/definition.rs` FilesystemMode is declarative (only
    parsed/recorded; grep shows no runtime consumer), `roles/install.rs`
    `Enforcement::Advisory` is the recorded-weakness posture. Test will
    assert the advisory surface + say so.
  - G10 `crates/gauntlet-app/src/reference/provider.rs:267` (freeze
    tripwire — the settle-consult detection half; deep proof g10 cited)
  - G11 `crates/gauntlet-ports/src/comparator.rs:1486` (`assert_blind` +
    `DENY_LISTED_FIELD_NAMES`)
  - G12 `crates/gauntlet-ports/src/producer_authority.rs:715`
    (`pass_records.is_empty()` — three-state floor, store-side)
  - G13 `crates/gauntlet-app/src/promote/delta.rs:159`
    (`settlement_access_verdict` — AK-056)
  - G14 `crates/gauntlet-app/src/memory/developmental.rs:304`
    (`record_recovery` completed==0 → RecoveryNotEarned)
  - G15 `crates/gauntlet-app/src/report/mod.rs:485` (`classify_unit` —
    proven = settlement row + PASS record pair)
  - G16 `crates/gauntlet-app/src/promote/gate.rs:256`
    (`evaluations.last()` — i1 lead)
- E-1 LANDED: `optimizer.rs` CONFIG_WRITE_TOKENS extended with
  std+glob spellings of rename/copy/hard_link/remove_file +
  `File::options`; `write!` added as a target rule (sink target banned,
  `write!(f, …)` formatter exemption documented — 33 legit formatter
  uses in the promote module would otherwise outlaw the module itself).
  Negative controls added per class. arch-tests optimizer tests: 9/9
  PASS (incl. the real-module-clean repo test).

## Next

- antigaming.rs suite (16 tests + validator), G16 RED-first probe,
  manifest, run-mutation.sh, mutation shards, slice battery.

## Checkpoint 2 (2026-09-04) — G16 hole CONFIRMED and fixed RED-first

- Stage-1 gate-world fixture landed in `crates/gauntlet-cli/tests/
  antigaming.rs` (p9 composition: real proposal service, real harness,
  real gate, real embedded-SQL ledgers, earned-authority executor seam).
- **G16 i1 lead CONFIRMED as a REAL DEFECT.** RED evidence (verbatim,
  before the fix):
  `a failing evaluation was laundered by a passing re-run — the
  re-ask-until-pass shape adjudicated PROMOTED: Promoted(
  RevisionPromotionEvidence { metric: "payment_per_settlement",
  control_figure: 30000000, treatment_figure: 20000000, ... })`
  Root: `gate.rs:256` adjudicated `evaluations.last()` alone.
- **Fix (smallest typed change):** `PromotionGate::adjudicate` now sweeps
  EVERY earlier evaluation record's record-derived FAILED conditions
  (`earlier_record_failures`: conditions 1, 3-series, 5, 7, 8) into the
  rejection reasons, each naming the earlier record. Evidence GAPS and
  input/ledger-derived conditions (2, 4, 6) are deliberately NOT swept:
  gaps are superseded by the freshest record (the honest road stays
  open — proven by `g16_the_honest_road_...` which was green BEFORE the
  fix too), and 2/4/6 are only honestly comparable over current inputs.
  Module doc records the re-ask floor.
- Post-fix: antigaming 4/4 PASS; `improve_promote_gates` 1/1; p9 4/4;
  gauntlet-app promote unit tests 49/49.
- E-1 (earlier checkpoint) green: 9/9 arch optimizer tests.

## Checkpoint 3 (2026-09-04) — stage 2 landed, 11/11 green

- antigaming.rs now has: g1 (contract parse refuses weakened rubric /
  empty dimensions / unsupported version), g2 (verify_unchanged I1
  tripwire with both digests), g5, g6 (gap-array digest coverage +
  timestamp-independence), g7 (lattice: agent downgrade refused w/
  recorded+proposed pair; operator raise-only), g8 (REWRITTEN to the
  public governed route: ComparisonAuthority → issue → review(Single,
  scripted critic) → accept; blocking residual under PASS intent forces
  aggregate FAIL; clean positive control), g10 (freeze-boundary tripwire
  after a builder raw write), g11 (assert_blind over every
  DENY_LISTED_FIELD_NAMES + allowlist/quoted-word controls), g13 (weakened
  delta + genuinely improving metric → REJECTED settlement_access_weakened
  only), g16 x2. 11/11 PASS.
- g8 note: `compare::aggregate` is crate-private ON PURPOSE (task10
  diagnosis); the suite reaches the merge through the governed chain's
  public route with a proof-local Comparator in the review source seam —
  its document flows the same acceptance/aggregation path a dispatched
  critic's does.

## Checkpoint 4 (2026-09-04) — suite COMPLETE: 18/18 green

- Full suite: g1-g16 (one per strategy; g16 has two legs) + the manifest
  validator = 18 tests, all PASS in ~0.2s wall.
- g9 landed as the ADVISORY test (recorded posture, closed vocabulary,
  refusal-to-widen; no mutation scope — honest-boundary rule).
- Manifest written: .agents/reports/defended-mechanisms.txt (16 G-IDs,
  g9 = advisory:, every location verified to exist post-edit).
- Validator enforces: 4-field format, G1..G16 coverage, non-advisory
  test_id ∈ antigaming::, advisory reasons spelled out, path:line
  existence.
- Next: scripts/run-mutation.sh; measure mutant counts per shard with
  --list first (memory-tight: -j 2 max).

## Checkpoint 5 (2026-09-04) — run-mutation.sh written; mutation run STARTED

- `scripts/run-mutation.sh` landed. Documented divergences (header):
  (1) the plan's `cargo mutants --file-list` does not exist in
  cargo-mutants 27.1.0 — the real surface is repeated `-f` + `-F/-E`
  regexes; (2) scope = the defended loci (span table per manifest entry,
  derived from source_location lines), not whole files — AK-070's claim
  is about the mechanisms the suites guard; (3) per-mutant tests scoped
  to `--test antigaming antigaming::` (the honest minimal AK-058 proof).
- Two mutants excluded with manifest-recorded justifications
  (defended-mechanisms.txt comments): G4's `with false` fail-safe
  direction; G15's `493:57 ||` flip unreachable in earned store states
  (COR-3 mooring).
- Suite strengthened for span mutants: g16 asserts EXACTLY ONE rejection
  reason (kills the sweep-bound `+`/`/` mutants' duplication);
  g11 adds the ARRAY arm case (visit's array-arm deletion);
  g6 asserts facet content survives normalization (kills the
  strip-to-default mutant); g12 adds the §16.3 re-entry legs
  (evidence mooring, ref mooring, AlreadyCommitted replay + one-row
  invariant) — covering SettlementCommitAuthority::admit's span.
- Pilot shard (developmental.rs, 2 mutants): both CAUGHT; outcomes.json
  schema verified; cold-build cost ~158s/mutant, incremental after.
- FULL RUN LAUNCHED (background). 14 shards, ~78 examined mutants.
- Suite: 18/18 PASS after strengthening.

## Checkpoint 6 (2026-09-04) — mutation evidence pipeline CORRECTED; honest run in flight

- Two pipeline defects found and fixed (both of the exact class task24
  exists to prevent — green/red verdicts over no evidence):
  1. The plan gate's `antigaming::` positional parses as nextest's DSL
     package selector and matches NOTHING (`gauntlet-cli::antigaming <fn>`
     display ids; the binary name is not an `antigaming::`-prefixed id
     component) — every mutant reported "caught" through the no-tests
     error. Recorded divergence; the script uses `--test antigaming`.
  2. cargo-mutants pins the test invocation to the MUTATED package
     (`--package=<pkg>@<ver>`); the antigaming target lives in
     gauntlet-cli, so without appending `-p gauntlet-cli` the suite never
     runs. Verified fix: Test phase now executes the 18 tests (~10s) and
     fails with nextest's test-failure exit under live mutants.
- The corrected run's shard 3 (producer_authority admit) LEGITIMATELY
  found 2 missed mutants: 698:53 (`&&`->`||` in the COR-5 record filter)
  and 776:46 (`&&`->`||` in the existing-row key probe). Strengthened:
  - g3 now settles unit A honestly in the same run and attacks unit B —
    under `||` the foreign same-run record stands in for B's verification
    and the typed detail changes (caught).
  - g12 leg (d): a second, fully EARNED key for an already-settled unit
    must refuse UnitAlreadySettled naming BOTH verifications; under `||`
    the refusal decays into a divergence/replay classification (caught).
    Also pins the exactly-once floor (spec §16.3).
- Suite after strengthening: 18/18 PASS (g3/g12 re-verified).
- Script now records each shard's cargo-mutants exit and continues; the
  final merge (missed==0) is the single gate authority.
- Full honest re-run LAUNCHED (14 shards, ~74 examined mutants).

## Checkpoint 7 (2026-09-04) — COMPLETE

- Honest mutation run finished: 61 examined, 54 caught, 7 unviable,
  missed=0, exit 0. mutation.json written with manifest_sha256
  (0120e354...) matching the on-disk manifest.
- Misses found by the honest pipeline and fixed by strengthening:
  observation_digest ×2 (g6 now shifts facet windows), producer_authority
  698:53 (g3 foreign-unit same-run record) and 776:46 (g12 leg (d)
  exactly-once UnitAlreadySettled). Justified exclusions recorded in the
  manifest for G4 fail-safe direction, G15 493:57 (COR-3-unreachable),
  G7 equivalent rank-arm mutants; G8 span narrowed to the verdict rule.
- Slice battery ALL GREEN: ruler ×3, check-package, --test antigaming
  18/18, lint (clippy + fmt), boundaries 237/237, mutation assert.
- Probe hygiene: .gauntlet/policies/ contains only the tracked repo
  files (contained.json, cross_cutting.json, local.json) — no probes
  planted, none left.
- Deliverable report written: b1-report.md.
