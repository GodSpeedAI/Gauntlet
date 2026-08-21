# Task 10 — fresh blind confirmation, round 5

**Verdict: NOT CONFIRM.** Task 10 remains IN-FLIGHT/BLOCKED.

The critic was fresh and blind (told not to read the round-4/7/8 reports before
forming its own view), was told which properties are explicitly NOT claimed, and
was required to make every bypass COMPILE as an external consumer of the public
API from a scratchpad crate outside the workspace. It compiled two, plus a table
of compile-failures for the routes that must not exist.

Its first run was killed mid-probe by an infrastructure usage limit and was
resumed from its own transcript; the interruption is unrelated to the findings.

---

## F1 — MAJOR, blocking. The port's own contract suite is a second minting path.

Verified independently against the source, not taken on the critic's word:

- `crates/gauntlet-ports/src/contract/mod.rs:90` exports `pub mod comparator;`
  **without** the `#[cfg(test)]` gate that `stubs` (`:104`) and `suite_runs`
  (`:107`) carry. `comparator_suite` (`contract/comparator.rs:297`) is therefore
  callable by any downstream crate, with a caller-supplied `Box<dyn Comparator>`.
- `contract/comparator.rs:80-119` builds a `ComparisonEnvelope` by **struct
  literal** — legal in-crate, since the fields are `pub(crate)` — and
  `TrustedComparisonContext::contract_fixture` (`comparator.rs:1236`) adopts it.
  That path runs neither `assert_blind` nor `verify_envelope_digest` nor
  `from_frozen_projection`, and it synthesises a `RunComparisonAuthority` by
  struct literal with **no** `load_run` and **no** `observation_binding`.
- The suite then lends that context **by reference into the caller's
  `Comparator`**, where `authority()`, `document_from()` and `admit_with()` are
  all usable.

The critic's compiled exploit supplies a `PendingComparison` whose
`invocation_spec()` returns a spec it built from the lent context (every field
`accept` cross-checks is copied straight out of it), collects a fabricated
`verdict: pass` document from a `Comparator` whose `compare` is `unreachable!()`,
and mints an `AdmittedComparison`. Output, three times, once per suite case:

```
[inside suite] *** MINTED AdmittedComparison *** evidence = evd-forged-000000
[inside suite] durable rows now: [(01ARZ3NDEKTSV4RRFFQ69G5FAV, 3, "pass", ...)]
```

**What it falsifies as written:** claim 1 (the caller does not mint the
capability — the ports crate mints it *for* the caller and lends it, which
downstream is indistinguishable), claim 3 (an `AdmittedComparison` with no
comparator dispatch), and claim 5 (`assert_blind` never runs on that envelope).
The doc at `state_store.rs:186-189` — "`AdmittedComparison` … has no constructor
outside the comparator port … durable comparison state is therefore reachable
only from a document a comparator returned under a governed dispatch
capability" — is false as stated.

**Why MAJOR and not CRITICAL, stated as the critic stated it:** the SQLite
adapter's `accept_comparison` independently requires the durable run, the
matching frozen digest, and a recorded `role_invocation` with the same
`context_envelope_digest`; against that adapter the forgery is refused, and the
critic did not compile that half. The fabricated identity is also fixed by the
suite (run `01ARZ3NDEKTSV4RRFFQ69G5FAV`, iteration 3), so it cannot be aimed at
an arbitrary victim run.

**Scope: IN SCOPE**, and **not** covered by the round-8 residual ledger.
Residual #2 covers a lying adapter *selected at the composition root*; this is
the ports crate's own publicly-exported fixture route.

**This is a miss in round 8, not an unlucky discovery.** Round 8 recorded
`contract_fixture` as "crate-visible … avoids an external re-admission escape"
and stopped there, without checking whether the crate lends its product outward
through a public entry point. It does.

**Smallest fix (the critic's second option, which is the structural one):** have
the suite earn its context through `RunComparisonAuthority::open` +
`TrustedComparisonContext::issue` against an in-crate stub store, and delete
`contract_fixture` entirely — restoring digest verification and the blindness
assertion on the suite path and leaving `issue` as the only minting path in
existence. The alternative (a private `governed: bool` discriminant refused by
`document_from`/`admit_with`) is smaller but adds a flag rather than removing a
path.

## F2 — MINOR. Framing correction to ledgered residual #2.

`RunComparisonAuthority::open`, `FrozenReferenceProjection::load` and
`TrustedComparisonContext::issue` each take the store as a **parameter**, and
`MandatoryComparisonAcceptance::new` takes an unrelated `Arc` pair — so at the
raw ports API a puppet `StateStore` + `EvidenceStore` yields a genuine
capability in three public calls, and the consequence can land in a *different*
store from the one the authority was earned against. The application layer does
couple them (`authority.rs:250` rebuilds acceptance from its own substrate).

The mechanism is the ledgered §7.1 trust assumption, so severity stays MINOR —
but the ledger's wording ("selected at the composition root") understates the
surface, and must be corrected to say: any crate depending on `gauntlet-ports`.

## F3 — MINOR. The `assert_blind` CALL SITE is unfalsifiable.

`comparator.rs:1124-1128` is the only wiring of the assertion into minting, and
both existing teeth call `assert_blind` directly on `canonical_bytes()`.
Deleting those five lines breaks no test. The round-8 revert table proved the
deny-list *body* bites, not the call site.

Partly inherent: `ComparisonEnvelope` has a closed, typed member list, so no
honest input can produce a deny-listed *key* and the call can never fire
in-tree. It is a guard against a future added member — which is precisely the
plan's rationale for it, and the one thing no test holds. Recorded as an
understood limitation rather than claimed closed. The F1 fix would route the
suite through `issue` and make a poisoned-suite-envelope case possible.

---

## What the critic verified as genuinely closed

Every structural negative compile-fails as an external consumer (rustc codes
from its run): `AdmittedComparison`, `TrustedComparisonContext`,
`ComparisonEnvelope`, `RunComparisonAuthority`, `AcceptedComparison`,
`CompletedReview` and `ComparisonProduct` all E0451 on private fields;
`contract_fixture`, `assemble`, `ObservationView::new` and
`IssuedComparison::context` all E0624; `compare::{envelope,review,aggregate}`
all E0603; `issued.clone()` E0599 and a second `review` a move error;
`review.accept(subset, dl)` E0061 — **contributor subsetting has no
parameter**; `governed.issue(.., bundle_ref, ..)` E0061 — **no observation
argument exists**; `state.accept_comparison(&doc, dl)` E0308.

Beyond the type wall it verified by reading: the observation-binding refusal and
the genuinely write-once sqlite binding, with
`an_unrecorded_substitute_observation_cannot_enter_comparison` matching the
typed refusal and panicking on anything else; that `teeth()` bites the deny-list
body across all twelve names **and** pins the converse (a denied word quoted in
`reference_view` prose stays legal, so the check is field-structural, not a
substring scan); that `build_envelope` holds no privileged construction of its
own; that aggregation imports no port at all; and that `high_assurance` cannot
silently degrade — a truncated adversarial pass is recorded as
`UnavailableEvidence`, never dropped.

---

## Disposition

Per the operator's stop condition, Task 10 stays **IN-FLIGHT/BLOCKED**. F1 is
the only blocker. No frozen Wayfinder/Core/reference-lock material was modified;
no `CandidateRevision` was created; no reference candidate was created or
promoted. Gates at the time of confirmation: `just test` **691/691**,
boundaries **194/194**, check/lint/deny/ruler green.
