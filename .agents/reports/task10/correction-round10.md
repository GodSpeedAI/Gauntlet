# Task 10 — correction round 10 (closing confirmation round 6)

Round-6 blind confirmation returned **NOT CONFIRM** with three MAJOR
findings. All three were verified independently in the source before being
acted on. Round 6 also confirmed round 9's fix: `contract_fixture` is gone
and the critic could not find a fabricating constructor.

## MAJOR 1 — acceptance never bound the document to the dispatched envelope

`MandatoryComparisonAcceptance::accept` compared
`document.envelope_digest()` against `spec.context_envelope.digest()`.
**Both are supplied by the comparator implementation.** `RoleInvocationSpec`
has public fields and `ContextEnvelope::new(body, digest)` is public, so an
implementation could report an envelope this capability never issued, hand
back a document bound to that envelope, and agree with itself. The critic
compiled it and got an `AdmittedComparison` carrying `verdict: pass`.

Fix (`comparator.rs`): `accept` now anchors the spec to the envelope THIS
context minted, before anything else —

```rust
let issued = product.context.envelope();
if spec.context_envelope.digest() != issued.envelope_digest()
    || spec.context_envelope.body() != issued.to_json()
{ ...refuse... }
```

Everything downstream then chains to the issued bytes rather than to the
comparator's account of them.

**Tooth:** `compare::review::tests::a_comparator_cannot_accept_a_document_bound_to_an_envelope_it_invented`
— a `SelfDealingCritic` admits, reports a spec naming a digest of its own
invention, and returns a self-consistent `pass` document. **Bite proven**:
deleting the anchor makes the test fail.

## MAJOR 2 — the observation bundle's bytes were a caller argument

Two distinct defects behind one claim:

1. `ObservationView::project` copied the bundle's **self-declared**
   `observation_digest` into the envelope without ever hashing the bytes,
   while the reference side of the same envelope has always been verified.
   An attacker-chosen digest would land in the comparison record and in
   durable state.
2. `issue` takes `evidence: &dyn EvidenceStore`. With a genuine authority
   and a store that serves different bytes at the durably bound ref, the
   critic compared a bundle the run never recorded — real run identity, real
   frozen reference, fabricated observation.

Fix, part 1 — the §12.4 digest rule moves INTO ports as
`gauntlet_ports::observation_digest` (the app's
`observation::digest` now delegates to it, so there is one rule, not two).
`project` re-derives the digest from the bundle's own bytes and refuses a
document that misdeclares itself.

Fix, part 2 — the durable binding now records the observation's **content**,
not just a pointer to it. `ObservationBinding { evidence, digest }` replaces
the bare `EvidenceRef` in `bind_observation_bundle` /
`observation_binding`; migration `0005` gains an `observation_digest`
column; write-once now covers the pair. `project` refuses bytes whose
re-derived digest is not the durably bound one, **whoever served them**.
That is what makes the evidence store non-authoritative: correction 1's
"provenance terminates in durable state" now terminates in the content, not
in a pointer some adapter resolves.

**Teeth (both bite):**
- `p2_context_isolation::bytes_that_are_not_the_bound_observation_cannot_be_served_into_a_comparison`
  — a `SwappingEvidence` store serves a valid, internally consistent, same-run
  bundle at the bound ref; the chain refuses, naming the durably recorded
  observation. Deleting the bound-digest check makes it fail.
- `gauntlet-adapter-compare-diff::a_bundle_that_misdeclares_its_own_observation_digest_is_refused`
  — reaches the self-consistency check specifically (the bound digest still
  matches, because the normalization pins the self-referential member).
  Deleting the self-consistency check makes it fail.

  Note: the equivalent p2 test passes even without the port check, because
  the app layer recomputes too. The adapter-level tooth is the one that
  proves the PORT refuses, which is what matters for every caller of `issue`
  that is not the app.

## MAJOR 3 — the authority leaked out of every comparator callback

`TrustedComparisonContext::authority()` was public and
`RunComparisonAuthority` is `Clone`. `Comparator` is a public trait whose
methods receive `&TrustedComparisonContext`, so any adapter implementation
could clone the authority out and outlive the dispatch — directly
contradicting the claim in `p2_context_isolation.rs` that no accessor hands
the dispatch capability out.

Fix: `authority()` is now `pub(crate)`. Nothing outside the ports crate used
it. Enforced by visibility, so there is no runtime test to write; the proof
is that the workspace still compiles with the accessor sealed.

## Documentation corrected rather than defended

- The `assert_blind` call site inside `issue` now says plainly that it
  **cannot fire**: `to_json` emits only fixed literal keys and `project`
  never reads the claims facet, so it guards a future envelope member rather
  than a live hole. It also records that the check is field-NAME-only —
  builder-authored prose can legitimately ride in the VALUES of
  caller-supplied members (`ApplicableConstraint::statement`,
  `RubricEntry::definition`), which is Task 9's structural exclusion doing
  the real work, not this check.

## Residuals — unchanged, or newly recorded

1. **`bind_observation_bundle` has no production writer.** `grep` finds it
   only in tests, the sqlite impl, and the port default; `ComparisonAuthority`
   is not used from `gauntlet-cli/src`. The durable anchor the whole
   invariant rests on is exercised only by the teeth that then prove things
   about it. Wiring the observation owner belongs to the Task-12 controller;
   recorded here rather than papered over.
2. **A puppet `StateStore` still yields an authority** — `open` takes
   `&dyn StateStore`. What that authority is worth is now narrower: the
   content digest must match the puppet's own binding, and durable
   acceptance against a REAL store still requires the run and the admitted
   critic invocation to exist there. The unacknowledged case the round-6
   critic named — mixing a forged-authority context with a real
   `MandatoryComparisonAcceptance` — is recorded explicitly: it produces a
   context, and acceptance then fails at the real store's own guard, not at
   the port's.
3. **The public contract suite hands external code a live capability.** With
   `contract_fixture` gone it is a genuinely earned context over fixture
   state, but `comparator_suite` is public and dispatches into a
   caller-supplied `Comparator`, so the capability is capturable. Bounded to
   the fixture run's identity, which no real store has recorded.

## Gates

`just test` 696/696 · `just check` · `just lint` · `just deny` ·
`just boundaries` 194/194 · `just ruler` — all green.
