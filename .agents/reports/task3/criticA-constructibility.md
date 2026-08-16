# Critic A — Constructibility of invalid states — `crates/gauntlet-domain/`

Artifact judged: `/home/sprime01/projects/gauntlet/crates/gauntlet-domain/` (16 files, ~6,360 lines, zero dependencies per `Cargo.toml`). Reference: `.agents/specs/Core-spec.md` §5.2–§5.3. Graded claims: AK-003…AK-007 in `.wayfinder/answer-key.md`. Read-only review; no cargo run.

## Verdict

The domain layer is built the way the spec demands: every aggregate and value object has private fields, validating constructors, no `Default` impls anywhere, no `unsafe`, no `unwrap`/`expect` outside tests, no serde (the crate has zero dependencies, so there is no lenient deserialization surface to smuggle an invalid state through), and the only wire surface is exact-match enum token parsing that returns typed `EnumUnknown` errors. All five graded units hold at this layer: the §5.2 vocabularies are distinct types (AK-003), unknown tokens are parse errors (AK-004), `Outcome` is three-state with no boolean collapse and INCONCLUSIVE never settles (AK-005), and the tier/authority coupling is enforced on every construction and retier path via a single `validate_coupling` funnel with an exhaustive 4×3×9 test (AK-006/AK-007). Constructibility is not airtight, however. One §5.2 invariant is bypassable by a single public constructor — `RoleId::Custom(String)` is a public enum variant, so the custom-name grammar and the reserved-name rule are advisory, not enforced (critical). The `settle` path cannot express the "in this run" clause of §5.3.2 because `VerificationRecord` carries no run binding, so a PASS record from a foreign run settles a Unit (major, borderline critical). Three further aggregate-consistency gaps (verdict contradicting blocking residuals; duplicate settlements counting toward promotion) and a set of minor identity/arithmetic edges complete the picture.

## Findings table

| id | severity | trigger | spec / AK violated | one-liner |
|----|----------|---------|--------------------|-----------|
| CA-1 | critical | accidental | §5.2 `RoleId` grammar (spec lines 211–212) | `RoleId::Custom(String)` is a public variant: direct construction bypasses the custom-name grammar and reserved-name rule |
| CA-2 | major | accidental (stale replay) / adversarial | §5.3.2 line 297 ("…for this unit id **in this run**") | `VerificationRecord` has no run binding, so `Unit::settle` accepts a PASS record from a different run |
| CA-3 | major | accidental | §13.5 via §5.3.6 docs; weakens AK-032 | `ComparisonRecord::new` accepts `Verdict::Pass` alongside a blocking residual |
| CA-4 | major | accidental | §5.3.8 + AK-055 ("minimum settlements") | `Capability::record_settlement` accepts duplicate `RunId`s; one run recorded N times satisfies `promote(N, …)` |
| CA-5 | minor | accidental | — (robustness) | `DraftUnit.evidence` is dead: `try_into_unit` silently drops it |
| CA-6 | minor | accidental | internal consistency of §5.2 `UnitStatus` table | `settle()` accepts a Unit in `out_of_scope`, which `transition()` treats as terminal |
| CA-7 | minor | adversarial (config) | — (robustness; §5.3.7 holds) | `forward_ceiling`'s unchecked multiply panics (debug) on huge-but-valid `Limits` |
| CA-8 | minor | accidental | — (robustness) | `Payment::plus` and `Unit::record_attempt` use unchecked `+` (debug panic / release wrap) |
| CA-9 | minor | accidental | §5.3.3/§5.3.4 identity hygiene | duplicate iteration numbers / invocation ids accepted; iterations attachable after `terminate` |
| CA-10 | minor | accidental | §5.3.2 `depends_on` ("ordering only") | `depends_on` never validated: self-cycles, unknown ids, duplicates accepted |
| CA-11 | minor | accidental | I8 ("missing evidence is not passing evidence") | `hard_gate_aggregate(&[], &[])` returns `Pass` |
| CA-12 | minor | accidental | §5.3.2/§16 preconditions (enforcement layering) | `SettlementRecord::new` is total: constructible referencing a FAIL or foreign verification |
| CA-13 | minor | — | — (documentation) | "decision field is write-once" doc contradicted by Pending→Provisional→Promoted path |

Counts: **1 critical, 3 major, 9 minor.**

## Findings

### CA-1 (critical, accidental) — `RoleId::Custom(String)` bypasses the §5.2 grammar

`/home/sprime01/projects/gauntlet/crates/gauntlet-domain/src/values.rs:600-609`:

```rust
pub enum RoleId {
    Orchestrator, Builder, Critic, Verifier, Observer, Optimizer,
    Custom(String),
}
```

In Rust, the variants of a `pub` enum are public constructors. The validating functions `RoleId::custom` (values.rs:624-644, grammar `[a-z][a-z0-9-]{1,31}` + reserved-name check) and `RoleId::parse` (values.rs:649-654) are therefore advisory: any caller — including every downstream crate, since `RoleId` is re-exported at `lib.rs:49` — can construct the invalid state directly.

Traced path (plain public API, no `unsafe`, no internals):

1. `let r = RoleId::Custom("Bad Name!!".to_owned());` — compiles; the §5.2 grammar (`custom(name) -- name matches [a-z][a-z0-9-]{1,31}`, spec lines 211-212) is violated. `r.name()` returns `"Bad Name!!"`.
2. Worse, the reserved-name rule is bypassable with an identity ambiguity: `let r = RoleId::Custom("builder".to_owned());` yields a value with `r != RoleId::Builder` but `r.to_string() == "builder"` and `r.name() == "builder"`. Round-trip is broken: `RoleId::parse(&r.to_string())` returns `RoleId::Builder`, a different value than the one serialized.
3. The invalid value flows into consequential state: `RoleInvocationDefinition { role: RoleId::Custom("builder"), .. }` (role_invocation.rs:110-125) is accepted by `RoleInvocation::admit` (role_invocation.rs:155-178), which validates everything *except* the role, because it trusts the type.

Trigger class: accidental — the validating function `RoleId::custom` and the variant `RoleId::Custom` are one character apart; any ordinary call site can pick the wrong one and the compiler will not object. Violates §5.2 RoleId grammar; undermines the typed-vocabulary guarantee behind AK-003/AK-004 (the type can carry a value the wire grammar forbids).

### CA-2 (major, accidental/adversarial) — `settle` cannot check "in this run"

§5.3.2 line 297: "`status = settled` requires a non-null `evidence` ref **and** a `VerificationRecord` with `Outcome = PASS` for this unit id **in this run**."

`VerificationRecord` (`/home/sprime01/projects/gauntlet/crates/gauntlet-domain/src/verification.rs:19-25`) carries `id, unit_id, evidence, outcome, instrument` — **no run id**. `Unit::settle` (`/home/sprime01/projects/gauntlet/crates/gauntlet-domain/src/unit.rs:569-598`) checks exactly three things: `verification.unit_id() == &self.id`, `verification.outcome() == Outcome::Pass`, and `status != Settled`. The "in this run" clause is not merely unchecked — it is inexpressible: neither `Unit` nor `VerificationRecord` knows a run.

Traced path:

1. Run A (RunId X) verifies AK-014 with outcome PASS; the record `vrf-042` is persisted to the evidence store.
2. Run B (RunId Y) begins. Recovery/replay or operator-resume code reconstructs run A's record — every constructor argument is a plain value object and `VerificationRecord::new` is public and total (verification.rs:31-45). There is no field against which to compare Y.
3. `unit_b.settle(EvidenceRef::new("evd-x")?, &record)` at unit.rs:574-597: unit id matches (AK-014), outcome is PASS → `Ok(())`, `status = Settled` in run B on run A's verification.

The locally-constructed state is well-formed (settled + evidence + PASS for this unit id), which is why this is graded major rather than critical: the violation materializes only in the Unit+Run composite, via a misuse path (a stale record) rather than ordinary construction. But the spec's clause is a named part of the §5.3.2 invariant, and the module's own doc comment (unit.rs:12-14) quotes it in full while enforcing only two of its three conjuncts. The type comment in verification.rs:8-9 defers the deep Annex B schema to "plan Task 11" — the gap is known and deferred, but as the artifact stands the clause is unenforced.

### CA-3 (major, accidental) — `ComparisonRecord` verdict may contradict its residuals

`/home/sprime01/projects/gauntlet/crates/gauntlet-domain/src/comparison.rs:81-93`: `ComparisonRecord::new(reference_digest, observation_digest, residuals, verdict)` is total and performs zero consistency checking between `residuals` and `verdict`. The type's own docs cite the rule twice (comparison.rs:111-112 "§13.5: any blocking residual forces an aggregate fail verdict"; comparison.rs:186-187 repeats it in the test), and the deterministic aggregator that *does* enforce fail-dominance exists — `hard_gate_aggregate` (reference.rs:337-358) — but `ComparisonRecord` neither calls it nor validates against it.

Traced path:

```rust
ComparisonRecord::new(
    reference_digest, observation_digest,
    vec![Residual::new(ResidualId::new("res-1")?, Severity::Blocking, "differs".into(), None)],
    Verdict::Pass,
)
```

→ a record where `blocking_residuals().count() >= 1` while `verdict() == Verdict::Pass` — precisely the state §13.5 forbids and AK-032 ("any blocking residual forces an aggregate fail verdict") claims is impossible. The same totality accepts `Verdict::Fail` with zero residuals. AK-032 is outside the graded AK-003…007 set, hence major ("weakens a named AK claim") rather than critical.

### CA-4 (major, accidental) — duplicate settlements satisfy the promotion count

`/home/sprime01/projects/gauntlet/crates/gauntlet-domain/src/capability.rs:199-201`: `record_settlement(&mut self, run: RunId)` pushes blindly — no dedup. `promote` (capability.rs:239-262) counts `self.settlements.len()`.

Traced path:

```rust
let mut cap = Capability::new(CapabilityId::new("cap-1")?, "x".into());
let run = RunId::new("01ARZ3NDEKTSV4RRFFQ69G5FAV")?;
cap.record_settlement(run.clone());
cap.record_settlement(run.clone());
cap.record_settlement(run);                       // same run, three entries
cap.record_variation_axis("runner".into());
cap.record_recovery_event("recovered".into());
cap.promote(3, 1)                                  // Ok(())
```

One successful run, promoted — which is the exact sentence AK-055 forbids ("One successful run does not promote a capability; promotion requires the configured minimum settlements under recorded variation plus a recovery event"). The structural floor (`max(1)`, capability.rs:244, 251) cannot save it because the count itself is inflatable. `record_variation_axis` and `record_evaluation_run` (capability.rs:208-210, 373-375) share the same duplicate blindness; for `CandidateRevision::promote` the exposure is smaller (only `is_empty()` is checked, capability.rs:426-430). A recovery-retry loop that re-records the same settlement on each replay hits this accidentally. AK-055 is outside the graded set → major.

### CA-5 (minor, accidental) — `DraftUnit.evidence` is dead

`DraftUnit` declares `pub evidence: Option<EvidenceRef>` (unit.rs:311), but `try_into_unit` (unit.rs:344-383) never reads `self.evidence` — the constructed `Unit` comes from `Unit::new`, which forces `evidence: None` (unit.rs:437). A parser or projection that faithfully populates the draft's evidence field silently loses it on conversion. Not an invalid state (the resulting Unit is valid), but a silent-data-loss trap on the exact field §5.3.2 makes load-bearing at settlement.

### CA-6 (minor, accidental) — `settle()` bypasses the status table from `out_of_scope`

`transition` (unit.rs:531-559) treats `OutOfScope` as terminal — there is no `(OutOfScope, _)` row, so `transition(Settled)` from it is refused. `settle` (unit.rs:569-598) consults only `status == Settled`; it accepts any other status. Traced path: `unit.transition(UnitStatus::OutOfScope)` (Ok) → `unit.settle(evidence, &pass_record_for(unit.id()))` → `Ok(())`, status `Settled`. An out-of-scope Unit becomes settled via a different public method on the same aggregate. §5.2 does not spell out the table, so this is an internal-consistency defect, not a quoted-invariant violation.

### CA-7 (minor, adversarial) — `forward_ceiling` unchecked multiply

`/home/sprime01/projects/gauntlet/crates/gauntlet-domain/src/budget.rs:337-339`: `limit * u64::from(100 - self.recovery_reserve_pct)` — no `checked_mul`. `Limits::new` (budget.rs:159-187) rejects only zero ceilings, so `max_tokens = u64::MAX` is a valid `Limits` value; `Budget::new` accepts it; the first `charge`/`would_breach_reserve` then panics with an overflow in debug builds (workspace `[profile.dev]` sets no `overflow-checks = false`, and there is no `[profile.release]` override — release wraps). Importantly, the wrap cannot break the §5.3.7 invariant: for `k = 100 - pct ≤ 99`, `limit*k mod 2^64 = limit*k − m·2^64 ≤ limit*k ≤ 100·limit` only when `m = 0`, and for `m ≥ 1` the result is strictly below `limit*k`, so the wrapped ceiling never exceeds the raw `limit` check that runs first (budget.rs:416-423) — the failure mode is a panic (debug) or over-conservative refusal (release), never reserve consumption. Severity minor for that reason.

### CA-8 (minor, accidental) — unchecked arithmetic in mirrored counters

`Payment::plus` uses plain `+` on all four dimensions (role_invocation.rs:47-54); `Iteration::record_payment` accumulates through it (iteration.rs:176-178); `Unit::record_attempt` does `self.attempt += 1` (unit.rs:503-505). Debug builds panic on overflow; release builds wrap the mirrored payment/attempt counters to fictionally small values. These counters are mirrors (the authoritative gate is `Budget::charge`, which is checked), so the impact is misreporting, not gate bypass.

### CA-9 (minor, accidental) — duplicate identities accepted in Run/Iteration

- `Run::attach_iteration` (run.rs:156-165) checks only `iteration.run_id() == &self.id`. Attaching `Iteration::new(run_id, IterationNo::new(1)?, None)` twice yields two iterations with the same §5.3.4 identity `(run_id, no = 1)`. It also succeeds after `terminate` — a run recorded `TerminalState::Settled` can still grow iterations.
- `Iteration::attach_invocation` (iteration.rs:109-120) checks run and iteration number but not invocation id: the same `RoleInvocation` (clone) attaches twice.

§5.3.3/§5.3.4 do not state uniqueness in so many words, hence minor, but the composite key is the entities' identity and nothing defends it.

### CA-10 (minor, accidental) — `depends_on` never validated

`Unit::new` accepts `depends_on: Vec<UnitId>` untouched (unit.rs:417-418, 440); `DraftUnit.depends_on` passes straight through (unit.rs:376); `Reference::new` (reference.rs:412-470) validates unit-id uniqueness, hard-unit presence, and unknown collisions, but never cross-checks any unit's `depends_on` against the reference's unit set. Constructible: a Unit depending on itself (a scheduling cycle), on a nonexistent id, or with duplicate entries. Spec calls the field "ordering only; not authority" (§5.3.2), so no quoted invariant is broken — but ordering with cycles is unrunnable, and the reference aggregate is the natural place that would be caught.

### CA-11 (minor, accidental) — vacuous aggregate pass

`hard_gate_aggregate(&[], &[])` returns `Verdict::Pass` (reference.rs:337-358 — the loop never runs, `saw_inconclusive` stays false). A reference with zero units and zero dimensions aggregates to Pass, which sits poorly with I8 ("missing evidence is not passing evidence"), quoted approvingly in the function's own doc comment (reference.rs:335-336). The function is a pure aggregator; the caller decides the input set — hence minor.

### CA-12 (minor, accidental) — `SettlementRecord::new` is total

settlement.rs:44-58: any `(RunId, UnitId, EvidenceRef, VerificationRef, EventLogOffset)` constructs. The record carries a `VerificationRef`, not an `Outcome` or the record itself, so a SettlementRecord referencing a FAIL verification — or a verification belonging to another unit or run — is one call away. §5.3.6's own stated invariants for this type (append-only, names the event-log offset) hold structurally; the PASS/evidence preconditions of §5.3.2/§16 are enforced only inside `Unit::settle`, and this standalone type does not route through them. Same "caller must have checked" layering as CA-2/CA-3, listed for completeness.

### CA-13 (minor, documentation) — "write-once" decision is not write-once

capability.rs:288-289 states "the decision field is write-once", but `assert_not_decided` (capability.rs:453-462) permits both `Pending` and `Provisional`, so the path Pending → Provisional → Promoted (or → Rejected) writes the decision field twice. The behavior is defensible (provisional is revisable); the doc is wrong. No invalid state.

## Invariant paths verified to HOLD

- AK-006/AK-007 tier/authority coupling enforced on every path: `Unit::new` (unit.rs:410-445), `DraftUnit::try_into_unit` → `Unit::new` (unit.rs:369-377), `Unit::retier` (unit.rs:631-636) — all funnel `validate_coupling` (unit.rs:269-295); `blast_radius`/`authority` are private with no setter; exhaustive 4×3×9 test at unit.rs:713-751.
- Producer rule: `Producer` has only `Instrument | Human` (unit.rs:32-39) — "model assertion" is unrepresentable; `instrument`-unless-`human_signoff` enforced in `validate_coupling` (unit.rs:288-293), tested over all `VerifyBy` (unit.rs:820-850).
- Tier never lowered, agents never retier: `retier` refuses `Actor::Agent` for any change and any rank-lowering move for everyone, before mutating (unit.rs:607-640); exhaustive test over 9 legal pairings × all targets × both actors (unit.rs:1063-1130).
- `settled` reachable only through `settle` with evidence + PASS for this unit id: `transition(Settled)` always refused (unit.rs:534-543); `settle` checks unit-id match and PASS, refuses FAIL and INCONCLUSIVE (unit.rs:574-585); double-settle refused (unit.rs:586-593); drafts cannot rehydrate `settled` (unit.rs:366-368); `evidence` is private and set only inside `settle`.
- AK-005: `Outcome` is three-state with no `From<Outcome> for bool` and no `is_ok`/`is_success`; `as_pass()` is `Option<bool>` keeping INCONCLUSIVE distinct (values.rs:308-322); a source-scanning guard pins the absence of the surface (values.rs:1176-1211); INCONCLUSIVE never settles (unit.rs:880-888).
- AK-004 on the parse surface that exists: `wire_enum!` parse is exact-match with typed `EnumUnknown` (wire.rs:89-99); `Outcome::parse` is exact uppercase, lowercase refused (values.rs:326-336); no serde/no derives-with-deserialization anywhere (Cargo.toml has zero dependencies), so no wire document can carry an unknown enum value in.
- AK-003: all eleven fixed-value-set fields of the claim are distinct types (values.rs:208-287), plus `Producer`, `CapabilityStatus`, `RevisionDecision`; no stringly-typed fixed-set field found in the crate.
- `Digest` carries its algorithm; bare hex, empty algorithm/hex, uppercase algorithm or hex, odd length, and second-colon shapes are all typed errors (values.rs:512-570, tested 977-1020).
- `RunId`/`InvocationId` are 26-char Crockford Base32, exact alphabet; lowercase and I/L/U/O rejected (values.rs:362-423).
- `UnitId` enforces `^[A-Z]{2,8}-[0-9]{3,6}$` including exactly-one-dash (values.rs:466-490); `IterationNo` is 1-based, zero rejected (values.rs:438-451).
- §5.3.3 frozen reference: `Run.reference_version` is private, set exactly once in `new`, read-only accessor, no method accepts a `ReferenceVersion` after construction (run.rs:94-128); pinned by a source-scan test (run.rs:262-290); terminal state recorded exactly once (run.rs:173-182).
- §5.3.7 recovery reserve: `charge`/`would_breach_reserve` refuse any projected spend above the forward ceiling with no override flag; landing exactly on the ceiling is the allowed boundary (budget.rs:344-439); refused charges book nothing; the CA-7 wrap cannot inflate the ceiling past the raw limit.
- Zero ceilings refused by `Limits::new` — "absence of a configured budget is never permission for unlimited recursion" (budget.rs:159-187).
- §5.3.5 sandbox/tier coupling: admission without a policy ref refused above `contained` (role_invocation.rs:155-158); `InvocationState` machine has no skips and terminal states are terminal (role_invocation.rs:250-267, tested 394-447).
- §5.3.1 reference invariants: unique unit ids, hard units present in the answer key, unknowns named and never colliding with unit ids, unique dimension ids, blocking dimensions must carry evidence (reference.rs:412-470, 219-232); `hard_gate_aggregate` contains no weights, Fail dominates, Inconclusive dominates Pass (reference.rs:337-358).
- Residual ledger: only opened residuals close, never twice (iteration.rs:162-169); observation gaps are mandatory populated-Vec fields, not Options (observation.rs:82-92).
- Structural promotion floors survive lenient arguments: `promote(0,0)` still requires ≥1 settlement, ≥1 axis, ≥1 recovery event (capability.rs:244-261, tested 593-616).
- Hygiene: no `Default` impls anywhere in the crate; `Clone` only clones already-valid states; no `unsafe`; no `unwrap`/`expect` in production (non-`#[cfg(test)]`) paths.
