# Task 10 — correction round 9 (closing F1 from confirmation round 5)

Scope: exactly the one blocking finding from the round-5 blind confirmation.
No new architecture, no widening into Task 12 or Task 20.

## F1 as found

`crates/gauntlet-ports/src/contract/mod.rs:90` exports `pub mod comparator;`
with no `#[cfg(test)]` gate (unlike `stubs` and `suite_runs`). The suite's
`envelope()` helper built a `ComparisonEnvelope` by struct literal for a
fabricated identity (run `01ARZ3NDEKTSV4RRFFQ69G5FAV`, iteration 3) and
adopted it through `TrustedComparisonContext::contract_fixture`, which
fabricated a matching `RunComparisonAuthority` by struct literal too.

That path skipped, in one call: `load_run`, `observation_binding`,
`FrozenReferenceProjection::load`'s digest verification,
`ObservationView::project`, and `assert_blind`. Because `comparator_suite`
is public and hands the context into a caller-supplied `dyn Comparator`,
any crate depending on `gauntlet-ports` could implement `Comparator`,
capture the fabricated capability, and mint a `ComparisonProduct` — the
critic compiled exactly that, producing a `pass` from a comparator whose
`compare` is `unreachable!()`.

This was a miss in round 8, not an unlucky discovery: round 8 recorded
`contract_fixture` as "crate-visible ... avoids an external re-admission
escape" and did not check whether the crate lends its product outward.

## The fix

`TrustedComparisonContext::contract_fixture` is **deleted**. There is no
fabricating constructor of comparison authority anywhere in the tree; grep
for it returns nothing.

`contract/comparator.rs` now earns its capability the production way:

1. `FixtureStores` — an in-suite value implementing BOTH `StateStore` and
   `EvidenceStore`: one durable `Run` pinned to the fixture's frozen
   reference, one `observation_binding` at iteration 3, and the evidence
   both live in.
2. `RunComparisonAuthority::open(&stores, ...)` — the two real durable
   reads.
3. `FrozenReferenceProjection::load(&authority, &stores, ...)` — the real
   digest verification. The fixture's pinned digest is COMPUTED from the
   fixture's own reference bytes, so the fixture cannot drift into a shape
   that would not survive `load`.
4. `TrustedComparisonContext::issue(...)` — the observation is read at the
   authority's durably bound ref and PROJECTED from the §5.3.6 wire bytes;
   the envelope digest is computed over the canonical bytes; `assert_blind`
   runs.

The one envelope member a case legitimately varies — the contract version —
is now a parameter of `issued_context`, so
`unknown_contract_versions_are_rejected` reaches the adapter's version
refusal without any post-hoc envelope mutation or digest re-labelling.

The failing instrument facet of
`facet_reality_is_visible_to_the_comparator` now enters the envelope by
projection from the bound bundle bytes rather than by composition, which
makes that case strictly stronger than before.

`issue` is now the only minting path in existence.

## New tooth

`contract::suite_runs::the_comparator_suite_dispatches_only_contexts_it_earned`

A capturing `ComparatorFactory` records what the suite dispatches and
asserts:

- the reference digest equals the SHA-256 of the fixture's own frozen
  reference bytes (a struct-literal fixture would carry whatever digest the
  literal named — the old one carried `sha256:c4d3e2f1a0b1c2d3`);
- the envelope digest is self-consistent with the canonical bytes;
- the dispatched envelope survives `assert_blind`;
- exactly one failing instrument facet arrives, i.e. by projection.

**Bite proven.** Temporarily changing `FIXTURE_REFERENCE_VIEW` to different
text makes the test fail with
`left: d2081caa... right: 13547497...` — "it was fabricated, not earned".
Restored afterwards.

## F2 — ledger correction

Round-8 residual #2 said the puppet-ports surface is reachable "at a
composition root". That is wrong and is corrected here: `RunComparisonAuthority::open`
takes `&dyn StateStore`, so ANY crate depending on `gauntlet-ports` can
pass its own puppet store and earn an authority for a run it invented. What
that authority is worth is bounded elsewhere — a real `StateStore`
implementation still refuses `accept_comparison` without the matching
durable run and admitted critic invocation — but the port-level statement
"the residual trust assumption is the composition root's adapter selection"
overclaims. The honest statement is: **a puppet store yields a capability
that is real for a run only that puppet believes in.**

## F3 — acknowledged, not closed

The `assert_blind` call site inside `issue` remains unfalsifiable. The
envelope's JSON keys are fixed by `to_json`, and `ObservationView::project`
structurally never reads `claims`, so no reachable input makes that call
fire. It is defense-in-depth against a future envelope member, not a check
with teeth. Recorded as a residual rather than dressed up as one.

## Gates

`just test` 692/692 · `just check` · `just lint` · `just deny` ·
`just boundaries` 194/194 · `just ruler` — all green.
