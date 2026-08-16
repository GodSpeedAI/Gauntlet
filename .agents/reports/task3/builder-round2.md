# Task 3 — Round 2 builder report (correction round)

Artifact: `crates/gauntlet-domain/` (spec §5.2–§5.3, Core-spec.md lines 185–404).
Inputs: `criticA-constructibility.md`, `criticB-fidelity-vacuity.md` (this directory).
Scope held: only the two criticals and the four folded majors below; everything else is a
recorded deferral (see "Gaps"). No commits made. `just ruler` run first (PASS) and last (PASS).

## Closure map

| finding | severity | spec quote that decides the fix | mechanism | status |
|---|---|---|---|---|
| CA-1 | critical | §5.2 lines 211–212: `RoleId := … \| custom(name) -- name matches [a-z][a-z0-9-]{1,31}` | opaque `CustomRoleName` payload + source-shape guard | closed |
| C1/CA-2 | critical | §5.3.2 line 297: "… a `VerificationRecord` with `Outcome = PASS` for this unit id **in this run**"; §16.1 line 1021: "A `VerificationRecord` with `Outcome = PASS` exists for it in this run" | `VerificationRecord` carries `run_id`; `settle(run, …)` checks it; typed error; doc states exactly what is checked | closed |
| CA-3 | major | §13.5 line 945: "Any `blocking` residual makes the aggregate verdict `fail` regardless of other dimensions and regardless of weighted scores." | constructor-level typed rejection (`ComparisonError`) | closed |
| CA-4 | major | §19.1 line 1136: "One successful run is not capability."; §19.2 line 1140: "≥ `GAUNTLET_PROMOTION_MIN_SETTLEMENTS` (default 3) settlements across recorded variation axes" | `record_settlement` rejects duplicate `RunId` (typed error); promotion counts distinct settlements | closed |
| B-major 1 (M1) | major | §5.2 line 232: "`Outcome` is a three-state type. No implementation may provide `impl From<Outcome> for bool`." | guard widened to `fn is_pass`/`fn passed`/`fn to_bool`/`fn as_bool`, `impl PartialEq<bool>`, any `… for bool` impl, ` as bool` casts | closed |
| B-major 2 (M2) | major | §5.3.5 line 348: `sandbox_policy_ref: # null only when tier=contained and backend=none` | tier loop extended to `Irreversible` (refused bare, pinned with policy) | closed |
| B-major 3 (M3) | major | §13.5 line 945 (as above) | fixture-echo test rewritten against the constructor (folds into CA-3) | closed |

## What changed, file by file

### CA-1 — `values.rs`

- `RoleId::Custom(String)` → `RoleId::Custom(CustomRoleName)`. `CustomRoleName` is a public
  opaque struct with a **private** `String` field and **no public constructor**: the only
  construction paths for a custom `RoleId` remain `RoleId::custom(name) -> Result<Self,
  ValueError>` (grammar `[a-z][a-z0-9-]{1,31}` + rejection of the six reserved spellings) and
  `RoleId::parse` (which funnels through `custom`). Direct variant construction outside
  `values.rs` is now a type error — the payload cannot be named into existence.
- Design note (honest): the payload cannot be a fully *private* struct, because a private type
  in a `pub` enum variant trips rustc's deny-by-default `private_interfaces` lint under the
  repository's `-D warnings` stance (verified with a scratch probe). The pub-opaque form gives
  the same guarantee the task demands: no public construction path produces an unvalidated
  name; external code can only ever move a payload that this module already validated.
- Display/parse round-trip preserved and now asserted (`parse(&custom.to_string()) == custom`)
  in `role_id_custom_names_follow_the_grammar` — impossible to break while reserved names are
  rejected at construction.
- Regression guard `role_id_custom_construction_is_only_via_the_validating_paths` pins the
  source shape (comment-stripped production half of `values.rs`): the variant must carry
  `CustomRoleName`, never `Custom(String)`, and the `impl CustomRoleName` block (tracked by
  brace depth) may contain no `pub ` item. Rationale recorded in the test: the guarantee is a
  compile-time fact, so it cannot be exercised at runtime; the guard pins the shape that keeps
  it a fact — same mechanism as the AK-005 and §5.3.3 guards already in this crate.
- During teeth, the guard's first (line-local) version missed a `pub fn` on its own line inside
  the impl block; the break passed green, the hole was fixed to the brace-depth scan, and the
  break then failed as required. Transcript below.

### C1/CA-2 — `verification.rs`, `unit.rs`

- `VerificationRecord` gains `run_id: RunId` (required constructor field + `run_id()`
  accessor); `Display` includes it. Consistent with §16.3's settlement-event key
  `(run_id, unit_id, verification_ref)`.
- `Unit::settle(run: RunId, evidence, verification)` — the run context is a parameter because
  a Unit does not know its run. Checks, in order: unit-id match (existing), **run-id match
  (new)** → `UnitError::VerificationFromDifferentRun { expected, actual }`, outcome PASS
  (existing), not-already-settled (existing).
- The `settle` doc comment now states exactly which conjuncts are checked here (unit id, run
  id, PASS, evidence pinned by this call) and names the one that is not (store resolution and
  pinning, §16.1 conjunct 2 → plan Task 13) — it no longer implies more than the code checks.
- Named regression `a_pass_record_from_a_foreign_run_does_not_settle`: PASS record with the
  right unit id but a foreign run → typed error, status stays `open`, evidence stays `None`;
  the same record settles in its own run.

### CA-3 — `comparison.rs`

- `ComparisonRecord::new` returns `Result<Self, ComparisonError>`; new error
  `ComparisonError::BlockingResidualRequiresFailVerdict { blocking }` (quote of §13.5 on the
  variant and in the Display text). Rule enforced: any blocking residual ⇒ verdict must be
  `Fail`; this covers `Pass`+blocking (the named violation) and `Inconclusive`+blocking under
  the same sentence.
- `ComparisonError` exported from `lib.rs`.

### CA-4 — `capability.rs`

- `record_settlement(run)` now rejects a `RunId` already recorded:
  `CapabilityError::DuplicateSettlement { run }`. `promote` therefore counts distinct
  settlements by construction. Rejection (not silent dedup) chosen to surface replay bugs at
  the call site, matching the crate's typed-error posture; §16.3's replay-idempotency rule
  belongs to the durable event log (Task 13), not the ledger — noted in the doc comment.
- `CapabilityError` dropped its `Copy` derive (the new variant carries a `RunId`); no caller
  relied on `Copy`.
- Named regression `replaying_one_run_three_times_is_not_three_settlements`: three records
  with the same `RunId` → two `DuplicateSettlement` errors, `settlements().len() == 1`,
  `promote(3, 1)` fails `PromotionRequiresSettlements { have: 1, need: 3 }`; three distinct
  runs then promote.

### B-major 1 — `values.rs` (guard widening)

Needles added beyond the original two: `fn is_pass` (also covers `fn is_passed`),
`fn passed`, `fn to_bool`, `fn as_bool`, `impl PartialEq<bool>`, any ` for bool` trait impl
(catches `impl PartialEq<Outcome> for bool` and the spec-literal `impl From<Outcome> for
bool` in one net), and the ` as bool` cast form. Fragment-assembly discipline kept (the old
`" for bool"` literal had to be re-split — it self-matched the new needle; that failure was
caught by the suite, not by review). Comment-stripping rationale updated to match the wider
claim.

### B-major 2 — `role_invocation.rs` (test only)

`above_contained_tier_a_sandbox_policy_is_required` now loops over
`[Local, CrossCutting, Irreversible]` for the `None`-policy refusal, and additionally pins
the legal shape at the irreversible tier (`Some(policy)` → admitted, `tier()` asserted).
Domain half only; the dispatch-side gate remains Task 14, untouched.

### B-major 3 — `comparison.rs` (test rewrite)

`blocking_residuals_are_selectable_and_drive_the_verdict_rule` (fixture echo) replaced by
`a_blocking_residual_forces_a_fail_verdict_at_construction`, which exercises the rule where
it lives: Pass+blocking refused, Inconclusive+blocking refused, Fail+blocking accepted
(with the `blocking_residuals()` filter and `is_blocking` assertions retained on the accepted
record), Pass+non-blocking accepted. Removing the §13.5 check fails this test — proven in
teeth.

## Test accounting

- Before: 72 (matches round-1 claim and baseline run).
- After: **75** — three new tests (`a_pass_record_from_a_foreign_run_does_not_settle`,
  `role_id_custom_construction_is_only_via_the_validating_paths`,
  `replaying_one_run_three_times_is_not_three_settlements`); one renamed/rewritten
  (`a_blocking_residual_forces_a_fail_verdict_at_construction` replaces the fixture-echo
  test). All 72 originals still pass.
- Existing-test changes, each with its reason:
  - `role_id_custom_names_follow_the_grammar` — constructs via `CustomRoleName` (payload type
    changed) and gains the round-trip assertion (CA-1 asks for it).
  - `settling_requires_evidence_and_a_pass_verification_record` — `settle` calls pass the run
    context (new required parameter).
  - verification.rs helpers/tests — constructor gained `run_id`; accessor asserted.
  - `one_successful_run_does_not_promote_a_capability`,
    `the_structural_floor_survives_lenient_arguments` — `record_settlement` returns `Result`.
  - `a_comparison_record_carries_both_digests` — constructor returns `Result`.
  - `above_contained_tier_a_sandbox_policy_is_required` — extended to Irreversible (B-major 2).
- No assertions weakened; no test disabled.

## Gate tails (final run, serial, in this order)

```
cargo nextest run -p gauntlet-domain
     Summary [  0.077s] 75 tests run: 75 passed, 0 skipped
just check    -> cargo check --workspace --all-targets: Finished, no warnings
just lint     -> clippy --workspace --all-targets -- -D warnings: Finished; cargo fmt --check: clean
just deny     -> advisories ok, bans ok, licenses ok, sources ok
just boundaries
     Summary [  1.177s] 183 tests run: 183 passed, 0 skipped
just test     -> Summary [258 tests run: 258 passed, 0 skipped]
just ruler (last)
verify-ruler.sh: PASS — answer key and manifest digests match the frozen lock
```

## Teeth transcript (break → named regression FAIL → revert → green)

Protocol: mechanism broken in source; focused `cargo nextest run -p gauntlet-domain <filter>`;
revert (byte-identical `cp` from a pre-break snapshot, verified with `cmp`, except where
noted); focused re-run green. Full-suite re-runs interleaved; 75/75 at the end.

1. **CA-1** — break: `pub fn unvalidated(name: String) -> Self` added to `impl CustomRoleName`.
   - First guard version (line-local `pub fn` check) **passed green against the break** — the
     check never saw a line containing both `CustomRoleName` and `pub fn`. Hole fixed to a
     brace-depth impl-block scan while the break was in place.
   - Re-run: `FAIL values::tests::role_id_custom_construction_is_only_via_the_validating_paths
     — "CustomRoleName must not gain a public construction surface: pub fn unvalidated(…)"`
   - Break removed (exact Edit revert; guard improvement retained): PASS.
   - The `Custom(String)` revert shape is additionally pinned by the guard's literal
     `Custom(String)` ban (demonstrating it requires multi-site edits to keep the crate
     compiling, so the shown break is the cheap compilable revert of the same validation).
2. **C1/CA-2** — break: the `verification.run_id() != &run` check deleted from `settle`
   (absorbed via `let _ = &run;`).
   - `FAIL unit::tests::a_pass_record_from_a_foreign_run_does_not_settle — assertion failed:
     matches!(… Err(UnitError::VerificationFromDifferentRun { .. }))`
   - Byte-identical restore (`cmp` clean): PASS.
3. **CA-3** — break: §13.5 constructor check replaced by `let _ = (&residuals, &verdict);`.
   - `FAIL comparison::tests::a_blocking_residual_forces_a_fail_verdict_at_construction —
     Pass+blocking expected Err, got Ok`
   - Byte-identical restore (`cmp` clean): PASS.
4. **CA-4** — break: duplicate-run check deleted from `record_settlement`.
   - `FAIL capability::tests::replaying_one_run_three_times_is_not_three_settlements —
     "re-recording the same run is refused"`
   - Byte-identical restore (`cmp` clean): PASS.
5. **Bonus, B-major 1** — break: `fn is_pass(self) -> bool` added to `impl Outcome`.
   - `FAIL values::tests::outcome_has_no_boolean_conversion_surface_in_this_crate —
     "values.rs: fn is_pass collapses Outcome's three states (AK-005)"` (the exact evasion
     critic B named as missed)
   - Exact Edit revert: PASS.
6. **Bonus, B-major 2** — break: refusal narrowed to
   `matches!(def.tier, Local | CrossCutting)` (critic's named mutation).
   - `FAIL role_invocation::tests::above_contained_tier_a_sandbox_policy_is_required —
     "policy-less irreversible admission must be refused"`
   - Byte-identical restore (`cmp` clean): PASS; full suite 75/75.

`grep -rn "TEETH BREAK" crates/` — clean at the end.

## Honest gaps

- **Deferred, out of folded scope (do not touch):** CA-5 through CA-13 and B's m1–m12 minors,
  plus the parts of CA-4's finding text outside the folded instruction:
  `record_variation_axis` and `record_evaluation_run` still accept duplicate entries. The
  folded fix covers exactly what the task named (duplicate `RunId`s vs the promotion count);
  variation-axis duplication inflating `min_variation_axes` is the same class and remains
  open — recorded here as the residual risk of CA-4.
- **CA-1 guard is textual, not semantic.** It pins the source shapes that keep unvalidated
  construction a compile error; a sufficiently creative reformulation (e.g. a public
  constructor on a *different* type that returns `RoleId::Custom` from inside `values.rs`)
  would need the scan extended. Compile-time impossibility remains the real enforcement; the
  guard is the tripwire, same standing as the AK-005/§5.3.3 guards.
- **§16.1's other conjuncts** (evidence-store resolution/pinning, authority-to-close,
  blocking-residual-free, reference digest) are not `settle`'s to check in this crate; the
  doc comment now says so explicitly rather than implying coverage.
- **`settle` signature change is breaking** for any future caller; no other crate in the
  workspace calls it today (verified by search), so the blast radius is contained to this
  crate's tests.
- Uncommitted working tree also carries unrelated changes under `.agents/plans/` and
  `.jolli/` from other processes; not touched by this round.

## Files touched

`crates/gauntlet-domain/src/{values.rs, verification.rs, unit.rs, comparison.rs,
capability.rs, role_invocation.rs, lib.rs}` — all under the delegated scope. No new
dependencies; no `unwrap`/`expect` outside `#[cfg(test)]`; no `Default`; typed errors carry
the failure locus throughout.

## Addendum: public re-export audit (micro-fix)

Audit method: extracted every type named in `pub fn` signatures (multi-line aware),
public struct fields, and public enum-variant payloads across
`crates/gauntlet-domain/src/*.rs`, and diffed them against the `pub use` lists at the
crate root. Three types were reachable through the crate's public API but not nameable
from outside; all were added to `crates/gauntlet-domain/src/lib.rs` (nothing removed or
renamed):

- **`VerificationRecord`** (verification.rs) — the parameter type of the public method
  `Unit::settle`; the whole settle path was uncallable by name from the future app layer.
  Added `pub use verification::VerificationRecord;`.
- **`Digest`** (values.rs) — public fields `RunDefinition.config_fingerprint` /
  `.environment_fingerprint`, accessors on `Run`, `ReferenceVersion::new`/`digest()`, and
  `Reference.digest()`. Added to the `values` re-export list.
- **`NarrativeField`** (reference.rs) — payload of the exported
  `ReferenceError::NarrativeMissing` variant; match arms outside the crate receive a
  value of a type they could not name. Added to the `reference` re-export list.

Permanent integration test `crates/gauntlet-domain/tests/settle_public_api.rs` (one test,
public API only, from outside the crate): a legal Unit; a foreign-run PASS
`VerificationRecord` is refused by `Unit::settle` with exactly
`UnitError::VerificationFromDifferentRun { expected, actual }` and the Unit stays `open`;
a same-run PASS record settles and the status becomes `settled`.

Teeth: with the `VerificationRecord` re-export temporarily commented out, the integration
test fails to compile (`error[E0432]: unresolved import
'gauntlet_domain::VerificationRecord'` — "no `VerificationRecord` in the root"); restored
afterwards. Note for posterity: the boundary scanner denies path-shaped literals in core
crates — the test's evidence locator uses the plain token `"locator-0001"`, matching the
existing in-crate test convention.

Gates after the fix: `cargo nextest run -p gauntlet-domain` 76/76 (75 existing + 1 new);
`just check`, `just lint`, `just boundaries` exit 0; `just ruler` PASS; fmt clean; no new
dependencies; not committed (build lock held).
