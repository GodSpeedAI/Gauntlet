# Task 10 — architecture diagnosis (pre-implementation)

Scope: the three material residuals carried by
`final-confirmation-round3.md` / `status-update-round2.md`. No code changed to
produce this document. No frozen Wayfinder/Core/reference-lock material read for
modification; no CandidateRevision proposed (see §8).

---

## 1. Root cause

**Task 10 models provenance as value validation at each boundary, not as
authority conferred by a single governed operation.**

Every type in the comparison chain is a *data* type with a public, total
constructor. The two types that are *named* as authority-bearing —
`FrozenReferenceProjection` and `TrustedComparisonContext` — are nominally
distinct but structurally derivable by any caller, because their constructors'
preconditions are **checks over caller-supplied arguments**, not proofs that a
governed operation occurred.

- `FrozenReferenceProjection::load(version, evidence, deadline)`
  (`crates/gauntlet-ports/src/comparator.rs:758`) proves exactly one thing:
  the bytes at `version.evidence_ref()` normalize to `version.digest()`.
  Both `version` and the `&dyn EvidenceStore` are caller-supplied. A caller who
  writes their own bytes and mints `ReferenceVersion::with_evidence(id, n,
  own_digest, own_ref)` satisfies it. **Self-consistency is not provenance.**
- `TrustedComparisonContext::from_frozen_projection(projection, envelope)`
  (`comparator.rs:836`) proves only internal agreement between two values the
  caller already holds: the envelope digest recomputes, and the envelope's
  `reference_digest`/`reference_view` equal the projection's.

Nowhere in the chain does any operation read authority from a source the caller
does not control. There is exactly one place that consults durable state —
`StateStore::accept_comparison`
(`crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs:1222`, which
does load the run and cross-check its frozen digest and admitted invocation) —
and it sits at the far end of the chain, is reachable only through one adapter,
and is bypassed by four other paths (§3).

The reason the chain has no anchor is visible in the code: **there is no
production caller.** `build_envelope` and `compare::review` have zero callers
outside tests (verified by grep across `crates/`). The "intended consequential
path" exists only as test fixtures. With no privileged path in code, every
alternate path is *equally* legitimate — which is precisely why patch rounds 3,
5 and 6 could each land real hardening without reducing the bypass class.

---

## 2. The invariant Task 10 is currently unable to enforce

> A comparison result is consequential only if it was produced by a comparator
> that was dispatched with a context minted from **this run's durable frozen
> reference** and **this run's pinned observation bundle**, and only if it was
> made durable through the single admission service that minted that context.

Task 10 today can enforce four weaker properties, and does enforce them:
shape-blindness over serialized bytes (P2), digest self-consistency at three
boundaries, bounded two-phase fanout with lease/cancellation, and — for the
model adapter alone — an invocation-bound acceptance tuple. None of them is the
invariant above, because all four take their inputs from the caller.

Preserving the distinction the request asks for:

| Level | Present in Task 10 today |
|---|---|
| validly shaped data | yes — typed newtypes, strict codecs, canonical serialization |
| provenance-bound data | **no** — digests are recomputed over caller-supplied bytes; nothing binds bytes to a durable origin |
| authorized data | **no** — `TrustedComparisonContext` is nominal, not earned |
| admitted consequential data | **partial** — only for `ModelComparator::collect`; four other paths reach consequence without it |

---

## 3. Bypass inventory (every path that demonstrates the problem)

**Forging trusted context**

- **B1** `ComparisonEnvelope::assemble` (`comparator.rs:614`) and
  `set_envelope_digest_for_runtime` (`:639`) are `pub` (`#[doc(hidden)]` is not
  an access modifier). With public `ObservationView::new` (`:324`),
  `FacetSummary::new` (`:162`) and `calculate_envelope_digest` (`:117`), any
  crate can build an arbitrary envelope carrying a valid digest.
- **B2** `FrozenReferenceProjection::load` (`:758`) — see §1. No durable read.
- **B3** `TrustedComparisonContext::from_frozen_projection` (`:836`) composes
  B1+B2 into a fully forged dispatch capability. This is not theoretical: it is
  done in-repo by adapter tests against a local fixture store
  (`gauntlet-adapter-compare-diff/src/lib.rs:449-451`,
  `gauntlet-adapter-compare-model/src/lib.rs:852-854`).
- **B4** `build_envelope` takes a caller-supplied `&ObservationBundleDoc`
  (`gauntlet-app/src/compare/envelope.rs:139`) and only recomputes its
  self-declared digest (`:191`). `ObservationBundleDoc::new` is public
  (`observation/bundle.rs:583`). `assert_blind` (`envelope.rs:341`) inspects
  object **field names**, so builder material smuggled into an allowed text
  value — a facet `command`, an `unavailable_evidence` reason — passes. The
  source concedes this at `envelope.rs:346`.
- **B10** `TrustedComparisonContext` is `Clone` and `Deref`s to
  `ComparisonEnvelope` (`comparator.rs:879`). Authority is neither single-use
  nor rebound to the current iteration: one legitimately obtained context can
  be cloned and dispatched arbitrarily often, at any later iteration.

**Reaching consequence without admission**

- **B5** `DiffComparator::comparison_document` (`compare-diff/src/lib.rs:112`)
  and `Comparator::compare` (`:310`) emit consequential records with no
  invocation, no evidence pin, no `accept_comparison`.
- **B6** `compare::review` is `pub` (`review.rs:275`) and returns a
  `ReviewOutcome` whose aggregate `ComparisonRecordDoc` has never been accepted.
  `aggregate` is `pub` over a plain `&[ComparisonRecordDoc]`
  (`aggregate.rs:264`) and `ComparisonRecordDoc::new` is public
  (`comparison_record.rs:597`) — so an aggregate can be fabricated from nothing
  at all.
- **B7** `StateStore::record_comparison` (`ports/state_store.rs:165`) is a
  public port method that writes a durable comparison with **no** invocation
  binding, run-existence check, or reference cross-check; implemented at
  `state-sqlite/src/state_store.rs:1281`. Its own doc comment concedes it is a
  bypass. This is a second durable write path parallel to `accept_comparison`.
- **B8** `MandatoryComparisonAcceptance` is a ports type any caller can
  construct (`:918`) and is invoked **by the adapter**
  (`compare-model/src/lib.rs:460`), not by the application. Acceptance is a
  courtesy an adapter performs, not a gate the application imposes — which is
  exactly why `DiffComparator` can simply omit it.

**Adversarial identity**

- **B9** `AdversarialComparatorSource` (`comparator.rs:972`) is an open public
  trait returning `Box<dyn Comparator>`; `review` requires only
  `Some(adversarial)` (`review.rs:284`) and trusts the box. `high_assurance` is
  therefore a caller declaration.

**Novel same-class bypass the new design must reject without a special case**
(§9 T6): **aggregate subsetting.** Run a genuine `high_assurance` review, then
call the public `aggregate()` over only the two friendly contributor documents,
dropping the adversarial one. All six coupling fields (`run_id`,
`iteration_no`, `reference_digest`, `observation_digest`, `envelope_digest`,
`comparison_contract_version`) still agree, so the merge succeeds and the
resulting aggregate is byte-indistinguishable from an honest one. No deny list,
digest check, or field validation can detect this — only an architecture in
which the accepted contributor set is the one the service *retained*, never one
the caller *supplies*, can refuse it.

---

## 4. Classification of the three findings

**Finding 1 — trusted context forgeable via public assembly/load constructors:
SAME ROOT INVARIANT.** B1–B4, B10. Closed entirely by §5.

**Finding 2 — accepted-record handling bypassable via review/diff paths:
SAME ROOT INVARIANT, plus one INDEPENDENT TASK-10 DEFECT.**
- B5, B6, B8 are the consequence-side face of the same missing authority chain:
  consequence is conferred by *holding* a document rather than *earning* it.
  Closed by §5.
- **B7 (`StateStore::record_comparison`) is an independent bounded defect.** It
  is not a manifestation of the missing chain; it is a second, deliberately
  retained durable write path that no admission governs. Correction: delete the
  port method and its sqlite implementation. Spec §18.1 names comparisons as
  minimum durable records; it does not license an ungoverned append.

**Finding 3 — adversarial-source attestation/composition forgeable or unwired:
SAME ROOT INVARIANT (dispatch/consequence half) + EXPLICIT LATER-SCOPE
(durable attestation half).**
- The Task-10-closable half: the adversarial comparator must be obtained and
  dispatched *inside* the governed service, its invocation recorded like every
  other contributor, and its contribution made a non-droppable member of the
  accepted set (this is what refuses the §3 novel bypass). Closed by §5.
- The later-scope half: proving *from durable evidence* that the adversarial
  instruction was actually used. The instruction lives only in
  `RoleInvocationSpec.prompt`; §5.3.5's `RoleInvocation` has **no prompt
  member**, and B.4's `comparator_identity` is `{runner, model, dimension}`
  with no adversarial carrier. Durable attestation therefore requires a
  §5.3.5/Annex-B carrier that the frozen reference does not have.
  **Exact frozen evidence that Task 10 does not claim it:** plan Task 10 step 5
  claims only "Review-mode dispatch: `single`, `fanout`, `adaptive`,
  `high_assurance`"; Task 10's "Done when" cites AK-026, AK-027, AK-032–AK-034;
  none of those AKs mentions adversarialness (AK-033 is adaptive skipping,
  AK-034 is truncation recording). Core §13.3 defines the mode's *behavior*, not
  a durable attestation obligation. Accordingly Task 10 may claim only the
  in-process property and **must ledger the durable half as an open residual —
  not as closed, and not as "the controller will handle it."**

After §5, the residual forgery surface for all three findings collapses to one
thing: **substituting a lying `StateStore`, `EvidenceStore`, or
`AdversarialComparatorSource` at the composition root.** That is Core §7.1
adapter selection — an audited configuration seam under Task 20 — not a bypass
reachable from a public convenience constructor. That reduction is the point of
the change.

---

## 5. The smallest architectural change that closes the whole bypass class

**One earned, durable-anchored authority token in `gauntlet-ports`, and one
application-owned admission service in `gauntlet-app` that is the sole minter of
dispatch capability and the sole route to consequence.**

### 5.1 The ports token (authority bottoms out in the durable substrate)

```rust
/// Earned, never constructed. Its only constructor performs the durable run
/// lookup; the run's own frozen ReferenceVersion is carried out of it.
pub struct RunComparisonAuthority { /* private */ }

impl RunComparisonAuthority {
    pub fn open(
        state: &dyn StateStore,
        run_id: &RunId,
        iteration_no: IterationNo,
        deadline: Deadline,
    ) -> Result<Self, ComparatorError>;   // refuses when the run is not durable
    pub fn reference(&self) -> &ReferenceVersion;   // the run's, never a caller's
}
```

There is no other constructor. The caller cannot choose the reference, because
no API takes one.

### 5.2 Ports surface that becomes opaque / capability-bearing

| Item | Change |
|---|---|
| `ComparisonEnvelope::assemble` | takes `&RunComparisonAuthority`; `run_id`/`iteration_no`/`reference_digest`/`reference_view` come from it, not from arguments |
| `ComparisonEnvelope::set_envelope_digest_for_runtime` | `pub(crate)`; the digest is set inside assembly |
| `FrozenReferenceProjection::load` | `load(&RunComparisonAuthority, &dyn EvidenceStore, Deadline)` — no caller-supplied `ReferenceVersion` |
| `TrustedComparisonContext` | `from_frozen_projection(&RunComparisonAuthority, projection, envelope)`; **drop `Clone`**, **drop `Deref`** (explicit `envelope()` accessor only) |
| `MandatoryComparisonAcceptance::accept` | requires `&RunComparisonAuthority` |
| `StateStore::record_comparison` | **deleted** (B7) |
| `AdversarialComparatorSource` | returns an opaque `AdversarialComparator` newtype rather than `Box<dyn Comparator>`, so an ordinary `ComparatorSource` cannot be adapted into the slot by relabelling |

### 5.3 The application-owned admission service

New module `crates/gauntlet-app/src/compare/authority.rs`. Linear, consuming,
non-`Clone` typestate — each step is the operation that earns the next state:

```
ComparisonAuthority::for_run(state, evidence, run_id, iteration_no, deadline)
        -> GovernedRun            // durable read; holds RunComparisonAuthority
GovernedRun::issue(bundle_ref, contract, constraints, residuals, deadline)
        -> IssuedComparison       // reopens the bundle FROM PINNED EVIDENCE by ref,
                                  // strict-decodes, recomputes, P2-asserts, mints
                                  // the TrustedComparisonContext. Not Clone.
IssuedComparison::review(deps, mode, budget, deadline)   // consumes self
        -> CompletedReview        // retains the exact contributor set dispatched
CompletedReview::accept(deadline)                        // consumes self
        -> AcceptedComparison     // pins + accept_comparison for every retained
                                  // contributor AND the aggregate
```

`compare::review` and `compare::aggregate` become `pub(crate)`.
`AcceptedComparison` has no constructor outside this module; downstream
consumers (verification, controller, settlement) take `AcceptedComparison`, never
a bare `ComparisonRecordDoc`.

**Why this closes each bypass:** B1/B2/B3 — the constructors now demand a token
whose only source is a durable read. B4 — the bundle is *reopened from pinned
evidence by ref*, so the caller supplies a pointer, not content; smuggled text
requires first getting that content past the observation owner and the pin.
B5/B6/B8 — `review`/`aggregate` are unreachable publicly and consequence is
carried only by `AcceptedComparison`. B7 — deleted. B9 — the adversarial
comparator is obtained inside the service from an injected port, at the same
trust level as the state store. B10 — non-`Clone`, consuming steps, and the
authority carries the iteration.

**Novel bypass (aggregate subsetting) is refused without a special case:**
`accept` has no parameter for a contributor slice. It aggregates and accepts the
set `CompletedReview` retained. There is nothing to subset.

---

## 6. The one intended consequential path after the change

```
durable Run (StateStore::load_run)
  → RunComparisonAuthority::open           [the only source of reference identity]
  → FrozenReferenceProjection::load        [bytes verified against the RUN's digest]
  → GovernedRun::issue                     [bundle reopened from pinned evidence;
                                            envelope assembled, digested, P2-asserted]
  → TrustedComparisonContext               [single-use, non-Clone, iteration-bound]
  → IssuedComparison::review               [the only dispatch; comparators, leases,
                                            fanout, adversarial slot]
  → CompletedReview::accept                [pin + accept_comparison per contributor
                                            and for the aggregate]
  → AcceptedComparison                     [the only consequential value]
```

No branch. Every other route is either deleted or compile-time unreachable.

---

## 7. What remains explicitly later-scope

1. **Durable attestation that the high-assurance pass was adversarial** —
   requires a `RoleInvocation`/Annex-B.4 carrier the frozen reference does not
   define (§4, Finding 3). Ledgered as an open residual of Task 10; Task 10 will
   claim only the in-process property.
2. **Durable aggregate provenance (contributor list on the aggregate record)** —
   already ledgered in-source (`review.rs:205` doc): the v1 B.4 schema cannot
   carry a contributor list without a schema candidate. After this change the
   contributors are each *individually accepted* durably, which is strictly
   stronger than today and needs no schema change; linking them to the aggregate
   *within one record* stays later-scope.
3. **Composition-root wiring of the real adapters** — Task 20 (§20.1 CLI). This
   change gives that root exactly one call sequence to wire; it does not build
   the controller (Task 12) or the runtime loop (Task 20).
4. **Permission/sandbox enforcement of the critic's envelope-only posture** —
   Core §17.2, Task 14's `Isolation` ladder.

---

## 8. CandidateRevision assessment

**None required.** The minimum architecture in §5 is expressible entirely in
port and application types. It adds no wire field, changes no Annex-B schema, and
reinterprets no frozen AK. The one place where a frozen carrier is genuinely
absent (adversarial attestation, §7.1) is *not* required by the minimum
architecture and is not claimed by Task 10 — so it is recorded as later-scope
rather than forced into a revision.

---

## 9. Discriminating fail-first tests

These are chosen so a *local patch* cannot pass them. Runtime-check patches
would keep compiling; T1/T4/T6's compile-fail halves would still fail.

- **T1 (compile-fail — the structural discriminator).** A downstream crate
  cannot name `ComparisonEnvelope::assemble(run_id, iteration_no, digest, view,
  …)`, `FrozenReferenceProjection::load(version, …)`, or
  `TrustedComparisonContext::from_frozen_projection(projection, envelope)` in
  their current forms. Also: `TrustedComparisonContext: !Clone`, and
  `StateStore::record_comparison` does not resolve.
- **T2 (no durable run ⇒ no authority).** `ComparisonAuthority::for_run` against
  a store with no such run is refused. Companion: there is no API by which a
  caller supplies a `ReferenceVersion`, so the "wrong reference" case is
  unrepresentable rather than rejected.
- **T3 (bundle smuggling).** `issue` with a `bundle_ref` whose pinned bytes
  carry builder material in a facet `command` is refused; a `bundle_ref` not
  pinned to this run is refused. Fails today: `build_envelope` accepts the
  document by value.
- **T4 (consequence requires admission).** A hand-built `ComparisonRecordDoc`
  cannot become an `AcceptedComparison` (no constructor), and a `DiffComparator`
  record cannot reach the state store (`record_comparison` gone).
- **T5 (replay / cross-iteration).** `IssuedComparison` is consumed by `review`;
  a context obtained for iteration N cannot be dispatched at iteration N+1
  (authority carries the iteration; the value is not `Clone`).
- **T6 (the novel bypass — aggregate subsetting).** After a `high_assurance`
  review, the durable accepted set contains exactly the dispatched contributors
  including the adversarial one; `aggregate` is `pub(crate)`, so re-aggregating
  a friendly subset does not compile.
- **T7 (regression of earned teeth).** Every existing Task-10 tooth
  (P2 deny-listed field, stale-envelope refusal, truncation-recorded,
  shuffle-invariant aggregation, lease quarantine, expired-deadline cancel)
  must still bite after migration.

---

## 10. Migration surface

| Area | Work |
|---|---|
| `gauntlet-ports/src/comparator.rs` | add `RunComparisonAuthority`; retype `assemble` / `load` / `from_frozen_projection` / `accept`; drop `Clone`+`Deref` on the trusted context; opaque `AdversarialComparator` |
| `gauntlet-ports/src/state_store.rs` | delete `record_comparison` |
| `gauntlet-ports/src/contract/comparator.rs` | fixtures migrate to the existing crate-private `contract_fixture` idiom (`comparator.rs:866`) |
| `state-sqlite` | delete the `record_comparison` impl; no migration change (`accept_comparison` already does the durable cross-checks) |
| `compare-diff` / `compare-model` | test fixtures currently mint trusted contexts directly (`:449`, `:852`) — migrate onto a ports-provided test authority; `ModelComparator` stops owning acceptance |
| `gauntlet-app/src/compare/` | new `authority.rs`; `review`/`aggregate` → `pub(crate)`; `envelope.rs` takes a `bundle_ref` + evidence store instead of a `&ObservationBundleDoc` |
| `gauntlet-cli/tests` | `p1_reference_integrity`, `p2_context_isolation`, `p3_real_observation`, `g6_hide_failed_observations` construct envelopes directly (7 call sites) — migrate to the governed path, which also gives P2 its first *real* end-to-end shape |
| arch/boundaries | new app module + the compile-fail harness must be inventoried |

Estimated: cross-crate, mechanical after the ports retype; the compile errors
enumerate the migration exhaustively.

---

## 11. Verdict

Three findings, **one root invariant** (plus one independent bounded defect,
B7, and one honestly-ledgered later-scope half of Finding 3). The correction is
directly implementable under Task 10 as scoped by the frozen plan: it adds no
controller, no runtime loop, no schema, and no reference change. Task 10 remains
IN-FLIGHT/BLOCKED until the change lands, the teeth in §9 bite, and a genuinely
fresh blind confirmation returns CONFIRM.
