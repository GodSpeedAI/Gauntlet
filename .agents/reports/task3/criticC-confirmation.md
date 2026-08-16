# Critic C — blind confirmation, `crates/gauntlet-domain/`

Scope: §5.2/§5.3 of `.agents/specs/Core-spec.md` (lines 204–404), graded claims AK-003…AK-007 in `.wayfinder/answer-key.md` (lines 15–48). Judged only the artifact on disk: public API surface (lib.rs re-exports + public methods), in-module tests, `tests/settle_public_api.rs`, and spec fidelity. No cargo/nextest run; rg/read only. Fresh eye — no prior critique consulted.

## Verdict

**Confirm, with residuals.** I could not construct a single §5.2/§5.3 invalid state from outside the crate: the tier/authority coupling is validated on every construction and mutation path (`Unit::new`, `DraftUnit::try_into_unit` → `Unit::new`, `retier`); `settled` is reachable only through `Unit::settle`, which checks unit id, run id, and `Outcome::Pass` individually; custom role names cannot bypass the grammar or the six reserved names (the `CustomRoleName` payload has no public constructor today); every §5.2 enum parse is exact-match with a typed `EnumUnknown` error carrying the right locus; `Run.reference_version` is private with no post-construction supplier; the budget reserve arithmetic is exact at the boundary and refused charges book nothing; a blocking residual cannot coexist with a non-`fail` verdict; capability settlements are deduplicated by `RunId`. The re-export surface is complete — every type appearing in a public signature is nameable outside the crate — and the crate has zero dependencies, so there is no serde/deserialization side door into any aggregate. The in-module tests genuinely bite: the exhaustive 108-combination coupling test, the reserve boundary test, and the enum-rejection test would all fail under the regressions they name. The residuals: (1) none of the five graded claims' **declared evidence locators** resolve to a real test target — the semantics are covered in-module, but the answer key's evidence contract is unmet (and knowingly so: deferred to Task 7); (2) `SettlementRecord::new` is an infallible, unbound constructor — the "durable fact that a Unit settled" is mintable with no PASS evidence anywhere, resting entirely on caller discipline; (3) a handful of state-machine asymmetries and guard-test evasion routes at minor severity.

Counts: **0 critical / 2 major / 7 minor.**

## Findings table

| # | Severity | Trigger | Stake | Location |
|---|---|---|---|---|
| M1 | major | process | AK-003…007 evidence contract | `tests/contract/`, `tests/proofs/` (absent); `.wayfinder/answer-key.md:15-48` |
| M2 | major | adversarial | §16.1/§5.3.6 spirit (settlement preconditions) | `crates/gauntlet-domain/src/settlement.rs:44-58` |
| m1 | minor | accidental | §5.3.2 status semantics | `crates/gauntlet-domain/src/unit.rs:542-571, 585-621` |
| m2 | minor | accidental | §5.3.2 draft fidelity | `crates/gauntlet-domain/src/unit.rs:355-394` |
| m3 | minor | adversarial | §5.3.8 / AK-055 (adjacent, not graded) | `crates/gauntlet-domain/src/capability.rs:230, 264-287` |
| m4 | minor | accidental | §5.3.3/§5.3.4 iteration identity | `crates/gauntlet-domain/src/run.rs:156-165` |
| m5 | minor | accidental | spec line 348 (§5.3.5) | `crates/gauntlet-domain/src/role_invocation.rs:155-158` |
| m6 | minor | adversarial regression | AK-005 / CA-1 guard bite | `values.rs:1229-1238, 1306-1321`; `run.rs:262-290` |
| m7 | minor | accidental | §16.1 conjuncts 3–5 | `crates/gauntlet-domain/src/unit.rs:573-584` |

## M1 — the graded claims' declared evidence locators do not exist

Severity: major. Trigger: process/evidence gap. At stake: AK-003, AK-004, AK-005, AK-006, AK-007 evidence contracts (`.wayfinder/answer-key.md` lines 15–48).

The answer key pins each graded claim to a locator:

- AK-003 → `tests/contract/typed_enums` — **no such file/target**; `tests/contract/` contains only `adapter_registry.rs`, `architecture_boundaries.rs`, `env_example_complete.rs`, `no_hardcoded_literals.rs`.
- AK-004 → `tests/contract/answer_key_parse::unknown_enum_is_error` — **absent**.
- AK-005 → `tests/proofs/p12_missing_is_not_passing` — **absent**; `tests/proofs/` is an empty directory.
- AK-006/007 → `tests/contract/answer_key_parse::tier_authority_coupling` — **absent**.

This is not ambiguous: the repository's own convention is that a locator names a real, runnable target (`tests/contract/architecture_boundaries.rs:1-7`: "Declared as a cargo test target … so the frozen answer key's locator resolves to a real, runnable target"), and `gauntlet-arch-tests` even pins a manifest declaring "exactly the four" existing locators (`crates/gauntlet-arch-tests/src/lib.rs:2100-2140`) — knowingly excluding these five. `CURRENT_STATUS.md:268` records the deferral: the `answer_key_parse` locators are "Task 7's parser's concern."

Why major and not critical: the charter's critical bar is a claim "unproven by any test." Each claim's semantics is proven by real, biting tests inside this crate — AK-003 (eleven distinct enum types, all used in public signatures), AK-004 (`values.rs:1126 every_fixed_value_set_enum_rejects_unknown_values`; `values.rs:1166` for Outcome's uppercase form), AK-005 (`values.rs:1194`, `values.rs:1206`; `unit.rs:913-920` INCONCLUSIVE refuses settlement; `reference.rs:337 hard_gate_aggregate`), AK-006/007 (`unit.rs:745 exhaustive_tier_authority_coupling_holds_on_every_construction_path`, `unit.rs:786`). But under the repo's own rule that missing evidence is not passing evidence, none of the five units' evidence refs resolve at their declared locators today. Until Task 7 lands the parser (and a `typed_enums`/`p12` target lands wherever planned), these claims cannot settle at their pinned evidence.

## M2 — `SettlementRecord::new` mints a settlement fact with no binding to any precondition

Severity: major (adversarial). At stake: §16.1 (spec lines 1018–1026), §5.3.6 (line 362), and the module's own claim.

`settlement.rs:30-33` documents: "Constructed only after those preconditions held." Nothing enforces this. The constructor (`settlement.rs:44-58`) is infallible and takes opaque refs:

```rust
// Outside-crate call sequence — no Unit, no VerificationRecord, no PASS anywhere:
let record = SettlementRecord::new(
    RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV")?,   // any run
    UnitId::new("AK-014")?,                       // any unit, never verified
    EvidenceRef::new("evd-forged")?,              // any evidence
    VerificationRef::new("vrf-never-existed")?,   // may name a FAIL/INCONCLUSIVE record, or nothing
    EventLogOffset::new(0),                       // any offset, nothing checks the log
);
```

Contrast the same crate's `Unit::settle` (`unit.rs:585-621`), which takes the *live* `VerificationRecord` and refuses wrong-unit, wrong-run, and non-PASS records; and `VerificationRecord`, whose validity rests on value objects. `SettlementRecord` is the one durable-fact type whose integrity rests entirely on caller discipline — the application layer is free to construct it without ever calling `Unit::settle`, and §16.1 conjunct 1 is then void at the type level. The §5.3.6 letter ("append-only and references the event-log offset") is met — no mutation methods, offset required — so this is weaker-than-spirit rather than a violated line, hence major rather than critical. Note also that `Unit::settle` is the only path the graded AK-005 settlement clause attaches to, and that path holds.

## m1 — `Unit::settle` ignores the from-status; `blocked`/`out_of_scope` → `settled` is reachable

Severity: minor. Trigger: accidental. At stake: §5.3.2 (line 297) status semantics.

`transition` (`unit.rs:542-571`) treats `out_of_scope` as terminal — no row leaves it — and routes every move into `settled` through `settle`. But `settle` (`unit.rs:585-621`) checks only unit id, run id, outcome, and not-already-settled; it never consults the current status. Outside-crate sequence: `Unit::new(...)` → `unit.transition(UnitStatus::OutOfScope)` (Ok) → `unit.settle(run, evidence, &pass_record)` (Ok) → status `Settled`. The spec's letter for `settled` (evidence + same-run PASS, line 297) is satisfied, so this is an internal inconsistency between the aggregate's two transition systems, not a spec-line violation.

## m2 — `DraftUnit::try_into_unit` silently drops `DraftUnit.evidence`

Severity: minor. Trigger: accidental. At stake: §5.3.2 draft fidelity.

`try_into_unit` (`unit.rs:355-394`) copies status, attempt, last_residual_ref, and notes onto the built unit but never reads `self.evidence`. Since a `Settled` draft is refused (correctly), a non-null `evidence` on a draft is always discarded. No invalid state results (the drop direction is the safe one), but the field is dead weight and a fidelity trap: a caller round-tripping units through drafts silently loses pinned evidence refs.

## m3 — duplicate variation axes count toward promotion

Severity: minor. Trigger: adversarial. At stake: §5.3.8 / AK-055 (adjacent, not in the graded set).

`record_settlement` deduplicates by `RunId` (held), but `record_variation_axis` (`capability.rs:230`) pushes any string, and `promote` counts `variation_axes.len()` (`capability.rs:276-282`). Three recordings of `"runner"` satisfy `min_variation_axes = 3`. Promotion inflates axis count without real variation. The graded settlement-dedup question holds; the axis count does not.

## m4 — `Run::attach_iteration` accepts duplicate and out-of-order iteration numbers

Severity: minor. Trigger: accidental. At stake: §5.3.3/§5.3.4 (iterations as an ordered 1-based sequence).

`attach_iteration` (`run.rs:156-165`) checks only `iteration.run_id() == self.id`. Attaching iteration `no = 1` twice, or `no = 5` before `no = 1`, is accepted. The spec nowhere states the invariant in so many words, so minor.

## m5 — sandbox rule: only the tier half of spec line 348 is enforced

Severity: minor. Trigger: accidental (documented deferral). At stake: §5.3.5, spec line 348 — "`sandbox_policy_ref` … null only when tier=contained **and backend=none**".

`RoleInvocation::admit` (`role_invocation.rs:155-158`) refuses a missing policy above the contained tier, but accepts `None` for contained regardless of backend. The backend half is not knowable in the domain and the deferral is documented in the module header; until the enforcement point exists (Task 14), a contained-tier, policy-less invocation is constructible under a sandbox backend where the spec demands one.

## m6 — the source-scan guard tests have evasion routes; names overclaim

Severity: minor. Trigger: adversarial regression. At stake: AK-005 and the CA-1 guard.

Three concrete holes in otherwise well-built pins:

1. `role_id_custom_construction_is_only_via_the_validating_paths` (`values.rs:1306-1321`): the brace-depth scan skips every depth-0 line that is not an `impl CustomRoleName` header — including the struct definition. Changing the payload to `pub struct CustomRoleName(pub String);` is a public construction surface that bypasses grammar and reserved names and **evades the scan** (the `Custom(CustomRoleName` line assertion still holds). The compile-time fact is true today; the guard does not prove what its name claims.
2. `outcome_has_no_boolean_conversion_surface_in_this_crate` (`values.rs:1229-1238`): needles miss `fn into_bool`, `fn ok`, `fn truthy`-style spellings (only `is_ok/is_success/is_pass/passed/to_bool/as_bool` and `" for bool"`/`" as bool"` are covered). The spec's literal rule (`impl From<Outcome> for bool`, line 232) is fully covered — by the `" for bool"` needle in-crate and by the orphan rule outside it — but "no boolean conversion surface" is broader than what the needles guarantee. (Verified: no multi-line block comments exist in `src/`, so the documented strip blind spot is presently empty.)
3. `no_public_surface_can_supply_a_reference_version_after_construction` (`run.rs:262-290`): catches `self.reference_version =` assignments and `fn` lines mentioning the field, but a restructuring write like `*self = Self { reference_version: v, .. }` evades both. The structural privacy of the field is the real invariant and holds.

## m7 — `Unit::settle` cannot represent §16.1 conjuncts 3–5; only conjunct 2's deferral is documented

Severity: minor. Trigger: accidental. At stake: §16.1 (spec lines 1018–1026).

§16.1 requires, additionally: authority permits closure by the closing actor (conjunct 3), no blocking residual references the unit (conjunct 4), and the reference digest is unchanged (conjunct 5). `settle`'s signature (`unit.rs:585-590`) has no parameters for an actor, the residual set, or the frozen digest, so none are checkable; the doc comment (`unit.rs:581-584`) documents only the evidence-store conjunct (2) as "the durable layer's conjunct." §5.3.2's letter (line 297) is what the domain must hold here, and it does; the undocumented scope of the remaining deferrals is the finding.

## What held

- Tier/authority coupling: no invalid pairing constructible or reachable by mutation — `Unit::new`, `DraftUnit::try_into_unit`, and `retier` all funnel through `validate_coupling`, and retier separately refuses agent actors and all lowering (spec lines 295–296, 298; AK-006/007).
- Unknown enum tokens: exact-match parse with typed `EnumUnknown` carrying the correct `ValueKind` on every §5.2 enum plus `Outcome` (spec line 230; AK-004).
- `Outcome`: three-state, `as_pass()` keeps all three distinguishable, `INCONCLUSIVE` refuses settlement and yields `Inconclusive` aggregation — never a soft pass (spec line 232; AK-005).
- `RoleId` custom names: grammar `[a-z][a-z0-9-]{1,31}` and the six reserved names enforced on the only construction paths; `CustomRoleName` has no public constructor today (spec line 212).
- Settlement gate: `status = settled` reachable only via `Unit::settle` with same-unit, same-run, `PASS` record; foreign-run PASS refused with the typed locus, including from outside the crate (spec line 297; `tests/settle_public_api.rs`).
- Frozen reference: `Run.reference_version` private, set once at construction, no post-construction supplier; terminal state write-once (spec line 317).
- Budget reserve: forward-ceiling arithmetic exact at the boundary (800 allowed / 801 refused at 20% of 1000), refused charges book nothing, diagnostic spend separate and never touches the reserve (spec line 379).
- Comparison integrity: blocking residual + non-`fail` verdict not constructible; both digests required constructor arguments (spec §5.3.6).
- Capability settlements: deduplicated by `RunId`; one run replayed N times counts once.
- Public surface completeness: every type in every public signature is re-exported and nameable outside the crate; `settle_public_api.rs` is compile-teeth for the `VerificationRecord` export.
- Zero dependencies: no serde or any other deserialization side door into any aggregate; ULID/UnitId/Digest validation hand-rolled per spec §5.2.
