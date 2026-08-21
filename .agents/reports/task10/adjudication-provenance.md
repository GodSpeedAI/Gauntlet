# Task 10 provenance adjudication

## Decision

**A — direct Task 10 repair.** The current envelope binds a digest to arbitrary
caller-supplied display text; it does not establish that the reference text sent
to the comparator is the frozen reference or a projection of it. That is exactly
Task 10's construction boundary, not a request to change the frozen ruler and
not a controller-only concern.

This adjudication read only the Core specification, implementation plan, current
Reference/Answer-Key and comparator code, and their tests. It did not read Task
10 critic/correction reports or `CURRENT_STATUS`.

## Governing requirements

* Core spec §9.1 says that everything downstream is a projection of the
  reference (`.agents/specs/Core-spec.md:618-620`), and §9.3 requires the answer
  key to be a projection rather than a rewrite (`:647-651`).
* At freeze, the runtime must write normalized bytes to evidence and record
  `ReferenceVersion { id, version, digest, evidence_ref }` on the run
  (Core §9.4, `:653-660`). The evidence copy is outside builder-writable paths
  and its digest is rechecked at boundaries (`:662-667`).
* The comparison allowlist is explicitly “reference (frozen, or the relevant
  projection of it)” (Core §13.2, `:908-924`). P2 is a structural assertion over
  the serialized envelope, not a prompt claim (`:920-924`; AK-026/027 at
  `:1581-1594`).
* Task 10 independently requires `EnvelopeBuilder` to accept only “frozen
  reference (or projection)” plus the other allowlisted inputs
  (`.agents/plans/gauntlet-impl.md:370-383`). Task 7 is the prerequisite that
  persists normalized bytes and records a `ReferenceVersion` (`:285-308`).

## Current evidence and locus

The public Task-10 input contains independent, public `reference_digest` and
`reference_view: String` fields (`crates/gauntlet-app/src/compare/envelope.rs:107-126`).
`build_envelope` copies both directly to `ComparisonEnvelope` (`:213-229`), then
computes a digest only *after* doing so (`:232-244`). Thus the digest proves that
later consumers receive the same arbitrary text; it does not prove the text was
rendered from the frozen artifact.

The composition proof demonstrates the gap: it reads and digests the repository
answer key directly (`crates/gauntlet-cli/tests/p2_context_isolation.rs:211-217`),
but supplies the unrelated literal `"the frozen reference's answer key, rendered
view"` as `reference_view` (`:315-334`). Its assertions consequently establish
digest coupling and blindness, but cannot establish frozen-reference/projection
provenance (`:451-465`). The unit test itself acknowledges that caller-supplied
projection provenance is separate from its lexical check
(`crates/gauntlet-app/src/compare/envelope.rs:309-315, 557-566`).

There is a narrow pre-existing carrier omission: `ReferenceVersion` stores only
id, version, and digest (`crates/gauntlet-domain/src/reference.rs:112-143`), even
though the provider obtains and pins `evidence_ref` during freeze
(`crates/gauntlet-app/src/reference/provider.rs:261-271`) and discards it by
returning `reference.freeze()` (`:272`; `Reference::freeze` at
`crates/gauntlet-domain/src/reference.rs:472-476`). This is a necessary enabling
change, not a ruler conflict: no answer-key content, comparison contract, or
acceptance criterion needs alteration.

## Smallest coherent alteration

1. Extend `ReferenceVersion` to retain the frozen `EvidenceRef`, matching Core
   §9.4; have `FileReferenceProvider::freeze` return that value after the
   existing put/pin operation.
2. Introduce an opaque `FrozenReferenceProjection` whose only construction path
   reads `ReferenceVersion.evidence_ref` from the evidence store, verifies the
   normalized bytes against `ReferenceVersion.digest`, and renders the allowed
   reference view from those bytes. It carries that same version/digest.
3. Replace `EnvelopeInputs.reference_digest` and `.reference_view` with this
   projection. `build_envelope` obtains both serialized fields solely from it.
   Keep the existing envelope-digest and deny-list checks.
4. Change P2 to resolve/normalize/freeze through `FileReferenceProvider`, build
   the projection from the returned version and evidence store, and assert the
   envelope view is the projection of the frozen bytes. Add a negative test that
   a view not derived from those bytes cannot enter the builder.

This is bounded to the Task-10 input boundary plus the missing frozen-version
provenance field it consumes. It does not require the later controller/settlement
callsites (so **not C**), and it preserves rather than revises the frozen
reference (so **not B**).
