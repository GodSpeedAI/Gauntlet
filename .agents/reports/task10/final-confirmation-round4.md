# Task 10 — fresh blind confirmation, round 4

**NOT CONFIRM.**

A blind critic (no access to the builder's reasoning; instructed not to read
`.agents/reports/task10/` or `CURRENT_STATUS.md`) was asked to break three
claims about the round-7 governed-admission chain. It compiled and RAN two
bypass probes from a foreign crate against the real adapters. Its findings are
accepted in full; none is disputed.

## Executed bypasses

**C1 — durable comparison forgery (CRITICAL).**
`StateStore::accept_comparison` is a **public trait method**
(`crates/gauntlet-ports/src/state_store.rs:150`), so a caller holding any store
handle calls it directly and never touches
`MandatoryComparisonAcceptance` — and therefore never touches the
`RunComparisonAuthority` check at `crates/gauntlet-ports/src/comparator.rs:1035`.
The sqlite gate
(`crates/adapters/gauntlet-adapter-state-sqlite/src/state_store.rs:1229-1271`)
verifies only tuple self-consistency, and every input to that tuple is publicly
constructible: `RoleInvocation::admit` with a caller-chosen
`context_envelope_digest`, `StateStore::record_role_invocation`, and
`ComparisonRecordDoc::new` with public `RecordContent`.
The critic's probe wrote and exported a durable `comparison_record@v1` with
`verdict: pass`, rationale "everything is fine, trust me", **no envelope, no
observation, no comparator, no review, no aggregate.**
Round 7 deleted `record_comparison` and left the same class open one method
over. That is the same defect, not a new one.

**C2 — ports-level envelope/capability forgery (CRITICAL).**
`ComparisonEnvelope::assemble` (`comparator.rs:621`) and
`TrustedComparisonContext::from_frozen_projection` (`comparator.rs:911`) are
public. A ports caller assembles an envelope with an arbitrary
`ObservationView` / `ContractView` and mints a dispatch capability **without
ever entering `build_envelope`** — so the observation-digest recompute
(`crates/gauntlet-app/src/compare/envelope.rs:190`) and `assert_blind`
(`envelope.rs:266`) are skipped entirely. The repo does exactly this from
another crate in its own adapter tests
(`gauntlet-adapter-compare-diff/src/lib.rs:517`).
The `pub(crate)` sealing of `compare::{aggregate,envelope,review}` is defeated
one layer down. Round 7 made the *authority* unforgeable and left the
*assembly* public — an incomplete application of its own diagnosis.

## Not executed but valid

**C3 — the adversarial capability is nominal, not structural (CRITICAL as
documented).** `AdversarialComparator::instructed` (`comparator.rs:1110`) is
public and accepts *any* comparator, so
`AdversarialComparator::instructed(ordinary.across_dimensions()?)` compiles.
The trait doc at `comparator.rs:1089-1104` — "A normal `ComparatorSource`
cannot occupy this slot merely by attaching a label" — **is false as written**
and must be corrected regardless of whether the mechanism changes. The P2
assertion (`p2_context_isolation.rs:826`) proves trait-distinctness only.

**C4 — unbudgeted, repeatedly-accepting dispatch (MAJOR).**
`IssuedComparison::dispatch_context()`
(`crates/gauntlet-app/src/compare/authority.rs:246`) hands out the capability
before `review` consumes it. A holder invokes `ModelComparator` arbitrarily
often with no `ModelDispatchBudget` lease, and each call durably accepts a
comparison (`gauntlet-adapter-compare-model/src/lib.rs:472`). AK-034's
truncation accounting is bypassed rather than violated.
**This defect was introduced by round 7**, to keep migrated tests compiling.
The single-use property claimed in the diagnosis (§9 T5) therefore holds only
for the governed `review` call, not for the capability.

**C5 — blindness is field-name-only (MAJOR, pre-existing).** `assert_blind`
never inspects string values; facet `command`, `UnavailableGap::reason` and
`FailureGap::detail` are free text on the wire. AK-026's wording is stronger
than what the check proves. Inside the governed chain this is mitigated (the
bundle must be content-addressed and declare this run); via C2 it is not
mitigated at all.

**C6 — `AGGREGATE_RUNNER` is a public string, not a capability**
(`crates/gauntlet-app/src/compare/aggregate.rs:55`) — any fabricated document
can claim aggregate provenance. Nothing consumes it as trust today; downstream
code must not start.

## What round 7 did close, per the same critic

- `AcceptedComparison` is unforgeable — private fields, sole constructor
  `CompletedReview::accept` (`authority.rs:310`), zero other constructions.
- **Contributor subsetting HOLDS.** `accept` has no slice parameter; the
  retained set comes from `review`. The novel bypass the diagnosis named is
  structurally unrepresentable.
- `aggregate` is unreachable externally; the diff comparator writes nothing
  durable.
- **Replay across runs/iterations is not reachable** — `IssuedComparison` is
  non-`Clone` and consumed; `from_frozen_projection` binds run + iteration +
  reference digest.
- Within the governed chain, the reference is not selectable and the bundle
  must be pinned evidence declaring this run/iteration.

## Scope note recorded verbatim from the critic

> Claims A/B/C as stated are stronger than Task 10's plan steps and
> AK-026/027/032–034, none of which claim unforgeable durability or adversarial
> attestation. Judged against the plan and AKs, Task 10's own gate is met;
> judged against the three properties as posed to me, A, B, and C each have a
> concrete public-API bypass, two of which I executed.

The three properties were posed by the builder, not by the frozen reference.
That distinction is recorded, **not used to soften the verdict**: the stated
goal is that the discovered bypass class be structurally unavailable, and it is
not.

## Verdict

**NOT CONFIRM. Task 10 remains IN-FLIGHT/BLOCKED** — not closed, not settled.
Gates are green (`just test` 690/690, boundaries 194/194, check/lint/deny/ruler)
and remain green; a green gate is again not the argument.

## Next correction, if authorized

C1, C2 and C3 are one mechanism away from closure and are the same shape as the
round-7 correction — the durable write, the envelope assembly, and the
adversarial capability each need a value that only the admission service can
construct, rather than a public constructor guarded by checks over
caller-supplied arguments. C4 is a bounded removal (`dispatch_context`). C5 and
C6 are separately bounded. No frozen material was modified and no
`CandidateRevision` is implicated.
