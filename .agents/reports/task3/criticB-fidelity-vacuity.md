# Critic B — spec fidelity + test vacuity: `crates/gauntlet-domain/`

Artifact judged on disk against `.agents/specs/Core-spec.md` §5.2 (lines 204–232) and §5.3.1–§5.3.8 (lines 234–400), graded on answer-key units AK-003…AK-007. No builder history, reasoning, or attempt counts were consulted. Read-only; no cargo/nextest/just was run.

## Verdict

The domain layer is a high-fidelity rendering of §5.2 and §5.3: every one of the eleven §5.2 fixed-value-set fields exists as a distinct enum with exactly the spec's variants and lowercase wire spellings (Outcome uppercase, as specified); the identifier newtypes enforce their stated grammars with real negative tests; every §5.3 aggregate exists with the spec's field set, and the tier/authority coupling (AK-006/AK-007) is enforced on every construction path and tested exhaustively over BlastRadius × Authority × VerifyBy. The recording of deferrals is exemplary — `units_state`, Annex B schemas, the I1 cross-check, the sandbox backend half, and the atomic commit sequence are all named, with plan-task pointers, rather than silently dropped. The crate's weaknesses are one genuine fidelity hole and a guard/test fringe that claims more than it proves: the "in this run" half of §5.3.2's settled invariant is enforced by nothing and deferred nowhere (`VerificationRecord` cannot even express a run binding, so a PASS record from run A settles the same Unit in run B); the AK-005 no-boolean guard and the sandbox-tier test each leave their most adversarial evasion unprobed; and several tests pin fixtures rather than rules. Verdict: **accept with corrections — 1 critical, 3 major, 12 minor findings.**

## Findings table

| ID | Severity | Trigger | Location | Spec / AK at stake | Summary |
|----|----------|---------|----------|--------------------|---------|
| C1 | critical | accidental | `unit.rs` `settle`, `verification.rs` | §5.3.2 line 297, §16 line 1021; AK-005 context | "…in this run" half of the settled invariant absent and undeferred; `VerificationRecord` carries no run binding |
| M1 | major | adversarial | `values.rs` `outcome_has_no_boolean_conversion_surface_in_this_crate` | §5.2 line 232; AK-005 | No-bool guard misses `fn is_pass`, `fn passed`, `impl PartialEq<bool> for Outcome` while its doc claims Outcome "cannot gain one" |
| M2 | major | adversarial | `role_invocation.rs` `above_contained_tier_a_sandbox_policy_is_required` | §5.3.5 line 348 | Sandbox-tier test probes Local and CrossCutting but never Irreversible — the most dangerous tier is the untested one |
| M3 | major | accidental | `comparison.rs` `blocking_residuals_are_selectable_and_drive_the_verdict_rule` | §5.3.6; §13.5 (quoted in-module) | Verdict asserted is a constructor input (fixture echo); the quoted blocking-residual⟹fail rule is enforced nowhere and its deferral is not recorded |
| m1 | minor | accidental | `values.rs` `plain_refs_reject_empty_whitespace_and_rooted_values` | §5.2 ref hygiene | Rooted-path (`/…`) rejection branch named in the test, never exercised |
| m2 | minor | accidental | `values.rs` `validate_ulid` | §5.2 line 207 | ULID validator accepts non-canonical ULIDs (first char 8–Z); no negative test |
| m3 | minor | accidental | `budget.rs` tests | §5.3.7 lines 364–379 | Reserve boundary exercised only on tokens and rounds; wall_seconds and cost_usd forward ceilings never tested |
| m4 | minor | adversarial | `wire.rs` (rationale), `values.rs` round-trip macro | §5.2 wire spellings | Spec tokens `build`, `test`, `diff` never pinned to any literal; symmetric derivation bug for them passes the suite (documented tradeoff) |
| m5 | minor | accidental | `values.rs` `every_fixed_value_set_enum_rejects_unknown_values` | §5.2 line 230 | Case-variant unknown spellings probed only for Outcome; e.g. `BlastRadius::parse("CONTAINED")` never probed |
| m6 | minor | accidental | `reference.rs` `SettlementCriteria` | §5.3.1 lines 259–263 | `hard_units` is not a §5.3.1 settlement field; invented acceptance structure |
| m7 | minor | accidental | `unit.rs` `is_hard_gate` | §5.3.1 line 270; P5/AK-044/045 | Hard gate equated with irreversible tier; the spec never states that equivalence in §5 |
| m8 | minor | accidental | `unit.rs` `transition` table | §5.3.2 | UnitStatus transition table beyond `settled` is invented (e.g. Blocked→Open refused); spec is silent |
| m9 | minor | accidental | `capability.rs`, `lib.rs` | §5.1 line 200, §5.3.8; §24.2 | Aggregate root `CapabilityLedger` has no type; only the entry `Capability` exists; deferral unrecorded |
| m10 | minor | accidental | `role_invocation.rs` `tier`, `unit.rs` `verification` | §5.3.5/§5.3.2 shapes | Extra fields beyond the spec's aggregate shapes (`tier` on RoleInvocation, `verification` on Unit) |
| m11 | minor | adversarial | `run.rs` `no_public_surface_can_supply_a_reference_version_after_construction` | §5.3.3 line 317 | Source-grep guard evadable by multi-line signatures; assignment check only line-initial |
| m12 | minor | accidental | `verification.rs` `the_three_outcomes_are_all_representable_and_distinct` | §5.3.6 line 361 | Constructor echo: asserts back what it passed in; near-cannot-fail |

---

## C1 — the settled invariant's "in this run" is neither enforced nor deferred

**[critical · accidental · spec §5.3.2 line 297, §16 line 1021]**

Spec §5.3.2:

> `status = settled` requires a non-null `evidence` ref **and** a `VerificationRecord` with `Outcome = PASS` for this unit id **in this run**.

Restated at §16 line 1021: "A `VerificationRecord` with `Outcome = PASS` exists for it in this run," and line 973 ties evidence freshness to "produced in this run."

The domain enforces exactly two of the three conjuncts. `unit.rs` `Unit::settle`:

```rust
if verification.unit_id() != &self.id {
    return Err(UnitError::VerificationForDifferentUnit { ... });
}
if verification.outcome() != Outcome::Pass {
    return Err(UnitError::SettledWithoutPassVerification { ... });
}
```

and `VerificationRecord` (`verification.rs`) has no run binding to check against:

```rust
pub struct VerificationRecord {
    id: VerificationRef,
    unit_id: UnitId,
    evidence: EvidenceRef,
    outcome: Outcome,
    instrument: InstrumentName,
}
```

`SettlementRecord` carries a `run_id`, but nothing connects it to `settle`; no comment in `unit.rs` or `verification.rs` defers the run-scoping (unlike the sandbox-backend and I1 deferrals, which are documented). Smallest consequence: a PASS `VerificationRecord` produced in run A settles the same Unit in run B — precisely the stale/cross-run evidence class §16 lines 973/1021 exist to exclude. The doc comment on `settle` quotes the spec sentence in full, including "in this run," while the implementation checks only the parts it can see — the quote masks the gap.

## M1 — the AK-005 no-boolean guard does not catch the collapses it claims to exclude

**[major · adversarial · spec §5.2 line 232; AK-005]**

`values.rs`:

```rust
let bool_conversion = ["impl From<Outcome>", " for bool"].concat();
let ok_method = ["fn is_", "ok"].concat();
let success_method = ["fn is_", "success"].concat();
```

with the rationale: "with no fn is_ok or fn is_success anywhere in this crate, Outcome cannot gain one, and Outcome lives only in this crate."

The spec's letter (`impl From<Outcome> for bool`) is covered. The claim in the comment is broader than the needles: `fn is_pass(&self) -> bool`, `fn passed(&self) -> bool`, `fn to_bool(&self) -> bool`, and `impl PartialEq<bool> for Outcome` (which makes `outcome == true` compile, collapsing INCONCLUSIVE into false) all pass this guard green. Each is the same three-state collapse the guard exists to prevent, and each is reachable without the banned `From` spelling. Smallest real defect missed: a `PartialEq<bool>` impl lands, `if record.outcome() == true { settle() }` starts compiling elsewhere, INCONCLUSIVE silently satisfies a check, and this test — the domain-level sentinel for AK-005 — stays green. (`as bool` casts cannot compile for a fieldless enum, so that particular evasion in the task brief is not real; the method-name and trait-impl evasions are.)

## M2 — the sandbox-tier test skips the Irreversible tier

**[major · adversarial · spec §5.3.5 line 348]**

Spec: `sandbox_policy_ref` is "null only when tier=contained and backend=none."

`role_invocation.rs` test:

```rust
for tier in [BlastRadius::Local, BlastRadius::CrossCutting] {
    assert!(matches!(
        RoleInvocation::admit(definition(tier, None)),
        Err(RoleInvocationError::SandboxPolicyRequiredForTier { .. })
    ));
}
```

`BlastRadius::Irreversible` — the tier where an unsandboxed dispatch is most dangerous and which §17/AGENTS treat as the never-automatic tier — is never probed with a `None` policy. The production check (`def.tier != BlastRadius::Contained`) is correct today. Smallest real defect missed: a mutation narrowing the refusal to `matches!(def.tier, Local | CrossCutting)` admits policy-less irreversible invocations and the suite stays green — the guard fails exactly where the blast radius is largest. (Related, smaller: the same test is also the only coverage of this rule, and it never probes Irreversible with a *present* policy either, so the legal shape at that tier is unpinned.)

## M3 — the blocking-residual/verdict test asserts its own fixture

**[major · accidental · spec §5.3.6; §13.5 as quoted in-module]**

`comparison.rs` documents the rule on `Residual::is_blocking`:

> Spec §13.5: a blocking residual forces an aggregate fail verdict.

and the test claims to prove it:

```rust
let record = ComparisonRecord::new(
    reference_digest(),
    observation_digest(),
    vec![
        residual("res-000001", Severity::Blocking),
        residual("res-000002", Severity::Minor),
        residual("res-000003", Severity::Blocking),
    ],
    Verdict::Fail,
);
...
assert_eq!(record.verdict(), Verdict::Fail);
```

`Verdict::Fail` is a constructor *input*; asserting it back proves nothing about any rule. The genuinely tested parts are the `blocking_residuals()` filter and `is_blocking`. What no code enforces and no comment defers: `ComparisonRecord::new` happily accepts a blocking residual alongside `Verdict::Pass` — the residual→verdict aggregation the module itself quotes exists nowhere in the crate (`hard_gate_aggregate` in `reference.rs` aggregates supplied *verdicts* and never sees severities), and unlike the I1 and Annex-B deferrals, this one is not recorded. Smallest real defect missed: a comparator that pairs `Severity::Blocking` residuals with `Verdict::Pass` constructs records the domain blesses, and the test named for the rule stays green.

## m1 — rooted-path rejection claimed in a test name, never exercised

**[minor · accidental · §5.2 ref hygiene]**

`values.rs` `plain_refs_reject_empty_whitespace_and_rooted_values` asserts `RefInvalid` only for `""` and `"has space"`. The `!input.starts_with('/')` branch of `validate_plain_ref` — the one that keeps `ResultContractPath`/`WorkspaceRef` configuration-relative — is never probed with `/abs/path`. Smallest defect missed: deleting the rooted-path check passes the suite, and absolute paths (machine-specific literals) enter refs the spec wants configuration-resolved. (The 1024-byte bound is likewise untested.)

## m2 — ULID validator accepts non-canonical ULIDs

**[minor · accidental · §5.2 line 207]**

`values.rs` checks only length and Crockford charset:

```rust
let valid = input.len() == ULID_LEN && input.bytes().all(|b| ULID_ALPHABET.contains(&b));
```

Canonical ULID encodes 128 bits in 26 base-32 chars, so the two most significant bits are zero and the first character must be `0`–`7`; `"8…"`, `"9…"`, `"A…"`…`"Z…"` leads are out-of-range values, not ULIDs. `RunId::new("ZZZZZZZZZZZZZZZZZZZZZZZZZZ")` is accepted. The negative test covers length, case, excluded letters (I/L/U/O), and empty — but not the high-bit edge. (Unicode input is bytewise-checked and correctly rejected, though also untested.)

## m3 — reserve boundary proven on two of four chargeable dimensions

**[minor · accidental · §5.3.7 lines 364–379]**

`the_recovery_reserve_boundary_is_exact` and `exhaustion_past_the_raw_limit_is_its_own_error` exercise the 800/801 token boundary (limit 1000, 20%) and the 8/9 rounds boundary (limit 10). The wall_seconds and cost_usd forward ceilings are never driven to their boundary in any test. Smallest defect missed: a cost-ceiling unit bug — e.g. computing the ceiling in dollars against spend tracked in `MicroUsd` — passes the entire budget suite, since `charges_book_every_dimension` only charges 10 micro-usd against a 500-micro-usd limit.

## m4 — the spec tokens `build`, `test`, `diff` are pinned by nothing

**[minor · adversarial · §5.2 wire spellings]**

`wire.rs` documents why those three literals are absent (the architecture boundary scanner denies them as adapter segments) and derives all tokens from variant names instead. Consequence: the actual §5.2 spellings of `VerifyBy::Build`, `VerifyBy::Test`, and `EvidenceKind::Diff` are asserted nowhere against a literal — the round-trip in `every_fixed_value_set_enum_rejects_unknown_values` compares `parse(wire())`, i.e. both sides against the same derivation, so a symmetric change to those tokens' derivation passes everything. The `wire_case` literal pins (`benchmark`, `static_check`, …) cover the mechanism, including one single-word pin (`Benchmark`), which is why this is minor and not major — but the tradeoff leaves exactly the three tokens an adversary would want to move unpinned. The module's own comment acknowledges the constraint; it does not acknowledge the residual test gap.

## m5 — case-variant unknown spellings probed for one enum only

**[minor · accidental · §5.2 line 230]**

`every_fixed_value_set_enum_rejects_unknown_values` probes two spellings per enum: `token + "x"` and `"nope"`. Only `outcome_wire_tokens_are_exact_and_case_sensitive` probes case folding (`"pass"` must not parse as PASS). No test asserts `BlastRadius::parse("CONTAINED")`, `UnitStatus::parse("Settled")`, or `VerifyBy::parse("Static_Check")` is an error. A lenient normalizer (case-insensitive or underscore-insensitive matching) added to `wire_enum!`'s parse would pass the suite for ten of the eleven vocabularies — precisely the "lenient coercion" §5.2 forbids.

## m6 — `hard_units` is not a §5.3.1 settlement field

**[minor · accidental · §5.3.1 lines 259–263]**

Spec settlement shape: `success_criteria`, `required_evidence`, `minimum_reliability`, `terminal_conditions`. The implementation adds:

```rust
pub struct SettlementCriteria {
    ...
    hard_units: Vec<UnitId>,
}
```

and builds an acceptance structure around it (`HardUnitMissing`). The §5.3.1 invariant says hard-gate Units must be "individually representable in the answer key" — it does not define a settlement-level hard-unit registry. This invents a mechanism where the spec's own mechanism is representability in `units`. It strengthens rather than weakens the invariant, but it is an invented acceptance surface a conforming runtime must now satisfy.

## m7 — hard gate equated with the irreversible tier

**[minor · accidental · §5.3.1 line 270; P5; AK-044/AK-045]**

```rust
pub fn is_hard_gate(&self) -> bool {
    self.blast_radius == BlastRadius::Irreversible
}
```

§5 uses "hard-gate Unit" (§5.3.1) and "hard requirement" (P5, AK-045) without ever defining the hard-gate set as `blast_radius = irreversible` (AK-044 says irreversible units are never auto-dispatched — a different rule). The equivalence is an interpretation presented as mechanism, and it feeds m6's `hard_units` check. A cross_cutting blocking unit is not a hard gate under this definition; whether that matches the spec's intent is asserted, not derived.

## m8 — invented UnitStatus transition table

**[minor · accidental · §5.3.2]**

```rust
let allowed = matches!(
    (from, to),
    (Open, InProgress) | (Open, Blocked) | (Open, OutOfScope)
        | (InProgress, Blocked) | (InProgress, OutOfScope) | (InProgress, Open)
        | (Blocked, InProgress) | (Blocked, OutOfScope)
);
```

§5.3.2 constrains only entry into `settled`. The remaining table — in particular the refusal of `Blocked → Open` and any exit from `OutOfScope` — is invented. It is stricter than the spec, not looser, but it is unwritten spec: a legitimate runtime flow (unblocking a unit back to open) is now a domain error with no spec citation behind it.

## m9 — `CapabilityLedger` exists in name only

**[minor · accidental · §5.1 line 200, §5.3.8; §24.2 vocabulary]**

§5.1 names the Memory context's aggregate root `CapabilityLedger`; §5.3.8's yaml shapes the *entries* (`capability:`). The crate implements `Capability` (the entry, complete per the yaml) and exports it; `lib.rs`'s doc banner even lists "CapabilityLedger" — but no such type exists, and unlike `units_state` and the Annex B schemas there is no recorded deferral of the ledger itself. One canonical name per concept (§24.2): the aggregate root's name is currently claimed by a doc comment and implemented as something else.

## m10 — extra fields beyond the spec's aggregate shapes

**[minor · accidental · §5.3.5 / §5.3.2 shapes]**

`RoleInvocation` carries `tier: BlastRadius` and `Unit` carries `verification: Option<VerificationRef>` — neither is in the §5.3.5/§5.3.2 field lists. Both are defensible (the tier field is how the domain enforces the sandbox rule the spec phrases as "tier=contained"; the verification ref evidences the settled precondition), and neither breaks vocabulary — but they are additions to the frozen shapes, noted here for the record. (`SettlementCriteria.hard_units`, the consequential one, is m6.)

## m11 — the frozen-reference source guard has grep holes

**[minor · adversarial · §5.3.3 line 317]**

```rust
let mentions = line.contains("reference_version");
if mentions && line.contains("fn ") { ... }
assert!(!line.trim_start().starts_with("self.reference_version ="), ...);
```

The check binds the field name and `fn` to the *same line*: a multi-line signature (`pub fn rebase(\n    &self,\n    v: ReferenceVersion,\n)`) contains the type name but never the string `reference_version` on the `fn` line, so a new method taking a `ReferenceVersion` evades the guard; the assignment check catches only line-initial `self.reference_version =`. The real enforcement is structural (private field, no setter, no `&mut` accessor), so impact is low — but the test's name promises more than the grep delivers, same pattern as M1.

## m12 — constructor-echo test

**[minor · accidental · §5.3.6 line 361]**

```rust
assert_eq!(record(Outcome::Pass).outcome(), Outcome::Pass);
assert_eq!(record(Outcome::Fail).outcome(), Outcome::Fail);
assert_eq!(record(Outcome::Inconclusive).outcome(), Outcome::Inconclusive);
```

The record stores `outcome` and the getter returns it; these three assertions cannot fail under any storage-faithful implementation. The `assert_ne!` pair (records differing only in outcome are unequal) is the only bit carrying information. Harmless, but it inflates the suite's apparent coverage of §5.3.6's binding rule, which is actually carried by the other test in the module.

---

## Fidelity checks and tests verified to hold

- AK-003: all eleven §5.2 fixed-value-set fields (VerifyBy, BlastRadius, Authority, UnitStatus, Verdict, Outcome, Severity, TerminalState, ReviewMode, InvocationState, EvidenceKind) exist as distinct enums in `values.rs` — variant-for-variant, no extras, none stringly typed.
- Every §5.2 wire spelling round-trips to the spec's lowercase form, name-checked one by one for all 55 variants (Outcome uppercase `PASS|FAIL|INCONCLUSIVE` per spec).
- `RoleId`: six named variants plus `Custom`, grammar `[a-z][a-z0-9-]{1,31}` enforced (2–32 bytes, lowercase lead), reserved-name collision refused, parse of grammar-conforming unknowns is `Custom` — spec-correct, not coercion.
- `RunId`/`InvocationId`: 26-char Crockford Base32, distinct types; negative tests cover short, long, lowercased, I/L/U/O substitutions, empty.
- `IterationNo` rejects zero (1-based); `UnitId` enforces `^[A-Z]{2,8}-[0-9]{3,6}$` with eleven on-point negatives including multi-dash, case, and digit-count edges.
- `Digest` carries its algorithm; bare hex, empty parts, uppercase, non-hex, odd-length, and double-colon inputs are all typed errors; same payload under two algorithms is unequal.
- AK-004 domain side: unknown enum tokens are `ValueError::EnumUnknown` carrying the right `ValueKind`, never coerced, for all ten wire enums plus Outcome (probed with corrupted and plain-unknown spellings).
- AK-005 domain side: no `impl From<Outcome> for bool` exists; `as_pass()` preserves three states (`Some(true)/Some(false)/None`); `Unit::settle` refuses FAIL and INCONCLUSIVE with distinct typed errors, and lowercase `"pass"` does not parse as PASS.
- AK-006/AK-007: `validate_coupling` runs on `Unit::new`, `DraftUnit::try_into_unit`, and `retier`; `exhaustive_tier_authority_coupling_holds_on_every_construction_path` iterates all 4×3×9 BlastRadius×Authority×VerifyBy combinations through both construction paths with independently derived expectations — genuinely exhaustive.
- §5.3.2 producer rule: instrument required unless `verify_by = human_signoff`, tested across all nine VerifyBy values for both producers.
- §5.3.2 tier governance: agents refused every tier change, lowering refused for everyone, exhaustively over all nine valid pairings × all destinations; refusal leaves state unchanged (asserted).
- §5.3.1: all manifest fields present including `quality_dimensions` (all eight sub-fields), `settlement`, `unknowns`, `assumptions`; unknowns must be named and must not collide with Unit ids; duplicate unit/dimension ids refused; blocking dimensions must carry evidence; `hard_gate_aggregate` contains no weights — any Fail forces Fail, Inconclusive dominates passes.
- §5.3.3: reference version frozen structurally (private field, set once in `new`, no post-construction path); behavioral test pins identity across attach/terminate; terminal state write-once; foreign iterations refused.
- §5.3.4: every field of the iteration shape present; invocation ownership (run + iteration number) enforced; residual ledger cannot close-unopened or close-twice; payment additive; checkpoint recorded with time and evidence ref.
- §5.3.5: all seventeen fields present; sandbox rule's tier half enforced at admission; InvocationState machine has no skips and no resurrections (tested for skip, happy path, fail, timeout, both cancels, and terminality).
- §5.3.6: `ObservationBundle` takes `unavailable_evidence` and `observation_failures` as mandatory populated vecs (I8); `ComparisonRecord` always carries both digests; `VerificationRecord` binds claim/evidence/outcome/instrument structurally; `SettlementRecord` requires an `EventLogOffset` and exposes no mutation.
- §5.3.7: all eight dimensions present (six limits + reserve pct + diagnostic allowance) plus mirrored `Spent`; zero ceilings refused; reserve must be 1–100%; charge to exactly the forward ceiling allowed, one past it refused with nothing booked; diagnostic spend walled off from forward and reserve; parallel/depth checked instantaneously.
- §5.3.8: `Capability` carries all yaml fields with `orchestration_burden` as a trend series; promotion requires configured minimums over a structural floor (≥1 settlement, ≥1 variation axis, ≥1 recovery event) even under lenient arguments (AK-055 shape); `CandidateRevision` decision write-once, promotion gated on evaluation runs plus all three held-out/regression/protected results, rejection requires a rollback ref.
- Deferrals are recorded, not silent: `units_state` (run.rs → plan Task 6), Annex B deep schemas (verification.rs → Task 11, observation.rs → Task 9), I1 digest cross-check (comparison.rs → Tasks 10/13), sandbox backend half (role_invocation.rs → Task 14), commit sequence (settlement.rs → Task 13), permission-set schema (values.rs), wire-literal avoidance rationale (wire.rs).
- Crate hygiene: zero dependencies in `Cargo.toml`; 72 in-module tests, matching the claimed count; typed error enums with failure loci throughout; no `unwrap`/`expect` outside `#[cfg(test)]`.
