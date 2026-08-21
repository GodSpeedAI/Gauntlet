# Task 10 correction round 7 — one governed admission chain

Companion to `.agents/reports/task10/architecture-diagnosis.md`, which
established that the three final-confirmation-round-3 residuals are one root
invariant plus one independent bounded defect plus one honestly-ledgered
later-scope half.

## Root cause corrected

Task 10 modelled provenance as **value validation at each boundary** — every
check compared two values the caller already held. `FrozenReferenceProjection`
and `TrustedComparisonContext` were nominally authority-bearing but
structurally derivable: `load` proved only that caller-supplied bytes hashed to
a caller-supplied digest, and `from_frozen_projection` proved only internal
agreement between two caller-held values. Nothing in the chain read authority
from a source the caller did not control.

The correction introduces one earned token and one application-owned service.

## What changed

**`gauntlet-ports`**

- New `RunComparisonAuthority`. Its only constructor is
  `open(&dyn StateStore, &RunId, IterationNo, Deadline)` — a durable run read.
  The run's own `ReferenceVersion` comes out of it; no downstream API accepts a
  caller-supplied one, so choosing the reference is now unrepresentable rather
  than rejected.
- `ComparisonEnvelope::assemble` takes `(&RunComparisonAuthority,
  &FrozenReferenceProjection, …)`. `run_id`, `iteration_no`,
  `reference_digest` and `reference_view` are no longer parameters. The
  envelope digest is computed inside; `set_envelope_digest_for_runtime` is
  **deleted**.
- `FrozenReferenceProjection::load` takes the authority instead of a
  `ReferenceVersion`.
- `TrustedComparisonContext::from_frozen_projection` takes the authority,
  additionally proves the projection is the durable run's frozen reference and
  that the envelope names this run at this iteration, carries the authority
  inside itself, and is no longer `Clone`. Its `Deref` to `ComparisonEnvelope`
  is removed — the observational shape is reached only through `envelope()`.
- `MandatoryComparisonAcceptance::accept` requires the authority and
  cross-checks the run/iteration/reference tuple before the existing
  invocation-tuple checks.
- `AdversarialComparatorSource` returns an opaque `AdversarialComparator`
  instead of `Box<dyn Comparator>`.
- **`StateStore::record_comparison` deleted** (the independent defect: a public
  durable comparison append with no invocation binding, no run-existence check
  and no reference cross-check — its own doc comment conceded it). Its sqlite
  implementation is deleted with it.

**`gauntlet-app`**

- New `compare::authority` — the only public route from a durable run to a
  consequential comparison, as a consuming typestate:
  `ComparisonAuthority::for_run → GovernedRun::issue → IssuedComparison::review
  → CompletedReview::accept → AcceptedComparison`.
  `issue` **reopens the observation bundle from the evidence store by ref** and
  requires it to declare this run and iteration — the caller supplies a
  pointer, never bundle content, which closes the "smuggle builder material
  into an allowed text value of a caller-constructed bundle" path that the
  field-name deny list could not see.
- `compare::{envelope, review, aggregate}` are now `pub(crate)`. `aggregate`
  taking a caller-supplied `&[ComparisonRecordDoc]` was the aggregate-subsetting
  bypass; `accept` has no contributor-slice parameter and works over the set the
  review retained.
- `AcceptedComparison` has no constructor outside that module: it is the only
  value in the system meaning "a comparison happened and counts".

**Migration** — every former direct call site was moved onto the governed
chain: both comparator adapters' fixtures, the ports contract fixtures, and
`p1_reference_integrity` / `p2_context_isolation`. The workspace compiles only
through the governed path.

## Residual trust surface, stated plainly

After this change the only remaining way to influence what the chain trusts is
**substituting the `StateStore`, `EvidenceStore` or `AdversarialComparatorSource`
adapter at the composition root** — Core §7.1 adapter selection at an audited
seam, not a public convenience constructor. That reduction is the point.

## New teeth (all proven to bite by revert)

In `crates/gauntlet-cli/tests/p2_context_isolation.rs`:

- `comparison_authority_requires_a_durable_run` — an empty state store yields no
  authority; the refusal must name the absent durable run specifically.
  **Bite proven twice:** deleting the run-existence refusal (fabricating an
  identity) and weakening its message both fail the test.
- `an_observation_bundle_from_another_run_is_refused_at_issue` — a well-formed
  bundle re-declared under another run id, stored and presented by ref, is
  refused. **Bite proven:** removing the run/iteration check in
  `GovernedRun::issue` fails the test.
- `the_issued_envelope_carries_the_durable_runs_own_frozen_digest`
- `an_absent_observation_ref_is_refused_never_treated_as_empty`
- `frozen_evidence_that_disagrees_with_the_durable_digest_yields_no_projection`

Rewritten to target the new structure (the old shapes are now unrepresentable):
the P2 tamper tooth now stores the tampered bundle and presents it by ref;
`a_foreign_runs_envelope_is_refused_before_diff_comparison` and
`an_envelope_from_another_iteration_is_refused_before_critic_admission` replace
the old "attacker-chosen reference view" fixtures, because choosing the view is
no longer expressible.

The compile-time half of the closure is not claimed as a runtime assertion. It
is enforced by the type system and is why the migration above was mandatory.

## Gates

- `just test`: **690/690** (685 before; the five new teeth)
- `just boundaries`: **194/194**
- `just check`, `just lint`, `just deny`, `just ruler`: all pass

## Honest residuals — no closure claim

1. **Durable attestation that the high-assurance pass was adversarial** remains
   open. The adversarial instruction lives only in `RoleInvocationSpec.prompt`;
   §5.3.5's `RoleInvocation` has no prompt member and B.4's
   `comparator_identity` is `{runner, model, dimension}`. Task 10 claims only
   the in-process property (plan step 5 and AK-026/027/032–034 claim no more),
   and this is recorded as later scope — not as closed, and not as "the
   controller will handle it".
2. **`EvidenceStore` has no pin-membership query**, so `issue` binds the
   reopened bundle by content-addressed ref plus its declared run/iteration
   rather than by proving pin membership.
3. **Deterministic-probe contributors have no `RoleInvocation`** to bind to, so
   they reach durability through the aggregate's pinned evidence rather than the
   invocation-bound tuple. The aggregate itself is durable as pinned,
   content-addressed evidence; a dedicated `RunEventKind` for it would be a
   ports/export change beyond this correction's surface.
4. **No production composition root yet** — Task 20 owns the CLI wiring. This
   change gives that root exactly one call sequence to wire.

No frozen Wayfinder/Core/reference-lock material was modified, no
`CandidateRevision` was created, and no reference candidate was promoted.
Task 10 stays IN-FLIGHT/BLOCKED until a genuinely fresh blind confirmation
returns CONFIRM.
