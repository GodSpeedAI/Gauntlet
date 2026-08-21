# Task 10 correction round 8 — the governed authority/admission architecture

Scope: the §5 architecture of `architecture-diagnosis.md`, completed, plus the
two corrections the operator required before implementation. Task 10 remains
**IN-FLIGHT/BLOCKED** and is closed only on a fresh **CONFIRM**.

No frozen Wayfinder/Core/reference-lock material was modified. No
`CandidateRevision` was created; no reference candidate was created or promoted.

---

## 1. The two operator corrections, and what they changed

### Correction 1 — observation provenance must terminate in durable state

Round 7 reopened a caller-supplied `bundle_ref` from the content-addressed
evidence store and required the reopened document to declare this run and
iteration. That proves immutability and self-consistency. It does **not** prove
the bundle is *this run's* observation: any well-formed, correctly hashed,
pinned bundle that names the run satisfied it.

Round 8 adds the missing durable fact and removes the caller's ability to name
one at all:

- `StateStore::bind_observation_bundle(run, iteration, evidence)` — the
  observation owner's **write-once** binding. Re-binding the same ref is
  idempotent; re-binding a *different* ref is refused, never overwritten
  (sqlite migration `0005_observation_bindings`, primary key
  `(run_id, iteration_no)`).
- `StateStore::observation_binding(run, iteration)` — the read.
- `RunComparisonAuthority::open` now performs **two** durable reads: the run
  (its own frozen `ReferenceVersion`) and the iteration's observation binding.
  An iteration with no binding yields no authority: there is nothing the
  comparison could be about.
- `GovernedRun::issue` **no longer takes an observation argument.** Neither
  does anything downstream. Substituting a different valid bundle is
  unrepresentable rather than detected.

### Correction 2 — builder-originated TEXT is not contraband

The diagnosis's T3 proposed refusing a facet `command` that contains "builder
material". That was wrong and is not implemented. Task 9's *structural* claims
exclusion is the governing distinction: claims have no representation in the
envelope at all, so nothing needs to inspect text for origin. An exact executed
command is instrument provenance (AK-028) and legitimately contains text a
builder wrote.

The test proves **provenance substitution refusal**, not textual contamination
detection — and one tooth asserts the *opposite* of the withdrawn T3:
`the_envelope_is_structurally_blind_and_carries_instrument_reality` now asserts
that the exact executed command reaches the comparator **and** that its text is
byte-identical to the builder's own declared self-check.

### Documented, not overclaimed: what the authority token represents

`RunComparisonAuthority` is **durable provenance authority**: possession proves
a durable read established this run, this iteration, this frozen reference, and
this observation binding. It is **not actor permission authority** — the
constructor is a public durable-state lookup, and nothing in it establishes that
the caller is permitted to act. The type doc says so explicitly. Permission and
sandbox enforcement remain Core §17.2 / Task 14.

---

## 2. What else round 8 closed (final-confirmation-round4 C1–C4)

| Finding | Closure |
|---|---|
| **C1** public `StateStore::accept_comparison` | It now takes an `AdmittedComparison`, which has no public constructor. Only `MandatoryComparisonAcceptance::accept` mints one, and that takes a `ComparisonProduct`, which only a dispatch through a `TrustedComparisonContext` mints. A caller holding a hand-built `ComparisonRecordDoc` has nothing to pass. |
| **C2** public `assemble` + `from_frozen_projection` | Both are `pub(crate)`, as is `ObservationView::new`. `TrustedComparisonContext::issue` is the single public minting path: it reads the bound bundle, projects the observation view from those bytes, assembles under the authority, computes the digest, and runs the P2 blindness assertion — which moved into the port for exactly this reason. There is no weaker variant to route around it. |
| **C3** `AdversarialComparator::instructed` accepted any comparator | The mechanism is unchanged (an adapter must call it deliberately) and the **doc no longer claims more than that**. It now states plainly that whether the wrapped comparator carries the adversarial instruction is an adapter obligation at the §7.1 trust level, and that the earlier claim was false. Durable attestation stays ledgered as later scope. |
| **C4** `IssuedComparison::dispatch_context()` (introduced by round 7) | Deleted. No accessor hands the dispatch capability out. Every caller — including the CLI composition proof — now dispatches through the governed review. |

The model adapter also **stopped accepting its own output**. It surfaces its
admitted `RoleInvocationSpec` through `PendingComparison::invocation_spec`, and
the governed review accepts each collected product against that tuple.
Acceptance is a gate the application imposes, not a courtesy an adapter performs.

---

## 3. The one intended consequential path

```
durable Run + durable observation binding   (StateStore)
  -> RunComparisonAuthority::open           [both durable reads; nothing else]
  -> FrozenReferenceProjection::load        [bytes verified against the RUN's digest]
  -> ComparisonAuthority::for_run -> GovernedRun
  -> GovernedRun::issue                     [no observation argument; view projected
                                             from the bound bytes; digest; P2 blindness]
  -> IssuedComparison                       [not Clone; no capability accessor]
  -> IssuedComparison::review               [consumes self; the only dispatch]
  -> ComparisonProduct                      [minted only by dispatch]
  -> MandatoryComparisonAcceptance::accept  [-> AdmittedComparison, the only thing
                                             StateStore::accept_comparison takes]
  -> CompletedReview::accept                [pins the retained contributor set]
  -> AcceptedComparison                     [the only consequential value]
```

---

## 4. Teeth (T1–T7, T3 corrected), and the bites proven by revert

- **T1 — compile-time structural closure.** Not claimed as a runtime assertion.
  The inventory is written out at the head of the teeth section in
  `p2_context_isolation.rs`. The workspace compiles only because every former
  direct call site was migrated onto the governed chain.
- **T2 — no durable run ⇒ no authority.**
  `comparison_authority_requires_a_durable_run`.
- **T3 (corrected) — provenance substitution.**
  `an_unrecorded_substitute_observation_cannot_enter_comparison`: two
  individually valid, correctly hashed, pinned bundles for the same run and
  iteration, only one durably recorded. Re-binding the substitute is refused by
  name, and the re-issued envelope carries the recorded bundle while the
  substitute's ref, digest, and evidence ref appear nowhere in the bytes.
  Companions: `an_iteration_with_no_durable_observation_binding_has_no_authority`,
  `a_bound_observation_whose_bytes_are_absent_is_refused_never_treated_as_empty`,
  `an_observation_that_is_not_this_runs_cannot_be_compared` (diff adapter),
  `an_iteration_the_run_never_observed_has_no_comparison_authority` (model
  adapter), and the tampered-bundle refusal, which now has to become some run's
  own durably bound observation to be presentable at all.
- **T4 — consequence requires admission.**
  `mandatory_acceptance_failure_is_non_success_and_never_writes_a_comparison`
  and the crosswire half of
  `durable_critic_invocation_and_accepted_document_survive_reopen_and_reject_crosswire`,
  both now driven through the governed review.
- **T5 — replay / cross-iteration.** Structural: `IssuedComparison` is consumed
  by `review` and is not `Clone`; the authority carries the iteration.
- **T6 — aggregate subsetting (the diagnosis's novel bypass).** Structural:
  `accept` has no contributor-slice parameter and `aggregate` is crate-private.
- **T7 — previously earned teeth.** All preserved; workspace **691/691**.

**Bites proven by temporarily reverting the check** (each restored afterwards):

| Reverted | Failing test |
|---|---|
| the missing-binding refusal in `RunComparisonAuthority::open` | `an_iteration_with_no_durable_observation_binding_has_no_authority` |
| the write-once refusal in `bind_observation_bundle` | `an_unrecorded_substitute_observation_cannot_enter_comparison` |
| the bundle's declared-run check in `ObservationView::project` | `an_observation_that_is_not_this_runs_cannot_be_compared` |
| acceptance's error propagation in the review | `mandatory_acceptance_failure_is_non_success_and_never_writes_a_comparison` |
| the deny-list comparison in the port's `assert_blind` | `teeth` |

---

## 5. Honest residuals — no closure claim

1. **Durable adversarial attestation.** Unchanged and still later scope: the
   instruction lives only in `RoleInvocationSpec.prompt`; §5.3.5 `RoleInvocation`
   has no prompt member and B.4 `comparator_identity` is
   `{runner, model, dimension}`. No Task-10 plan step or AK claims it.
2. **The composition root's adapters.** A lying `StateStore`, `EvidenceStore`,
   `Comparator`, or `AdversarialComparatorSource` selected at the composition
   root defeats every property here. That is Core §7.1 adapter selection under
   Task 20, not a public convenience constructor.
3. **The observation owner's binding.** Round 8 makes the comparison chain
   *consume* a durable binding it cannot choose. Whoever performs the
   observation still performs the binding write; the binding's immutability is
   what protects the comparison, not an authorization check on the writer.
4. **Deterministic-probe contributors** have no `RoleInvocation` to bind to, so
   they reach durability only through the aggregate's pinned evidence, never the
   invocation-bound tuple.
5. **`EvidenceStore` has no pin-membership query,** so the chain binds by
   durable ref rather than by proving pin membership.
6. **`RunComparisonAuthority` is provenance authority, not permission
   authority** (§1 above).
7. **No production composition root** wires this chain yet — Task 20.

---

## 6. Gates

`just test` **691/691** · `just check` · `just lint` · `just deny` ·
`just boundaries` **194/194** · `just ruler` — all green (see the status entry
for the run that accompanied the fresh confirmation).
