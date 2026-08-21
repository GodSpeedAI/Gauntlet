# Task 10 — fresh blind confirmation round 7: **CONFIRM**

An external-consumer attack crate was written and compiled against the
workspace by path. All five architecture claims survived. No CRITICAL and no
MAJOR finding.

## Attacks and outcomes

| Attack | Result |
|---|---|
| Direct construction / struct literals of `ComparisonEnvelope`, `TrustedComparisonContext`, `AdmittedComparison`, `ComparisonProduct` | **Does not compile** — private fields (E0451), private `ObservationView::new` (E0624) |
| Retain the authority past dispatch | **Does not compile** — `authority` is private (E0624); retaining the context borrow is a lifetime error |
| Forge a `RoleInvocationSpec.context_envelope` and accept a document bound to it | **Refused**: "mandatory acceptance refused an invocation whose context envelope is not the one this capability issued" |
| Serve a bundle that misdeclares its own digest | **Refused at the port**: "declares observation_digest sha256:eeee… but its own bytes digest to sha256:822e…" |
| Serve other bytes at the durably bound ref | Refused (repo tooth, mutation-verified by the critic) |

Attacker-supplied `StateStore`/`EvidenceStore` still yields a context — the
acknowledged §7.1 residual, documented at the port — and gets no further than
the real store's own guards.

## Findings, all MINOR

1. **`comparator_suite` lends a fixture capability to dependents.** Public and
   non-`cfg(test)`, so any dependent crate can implement `ComparatorFactory`,
   receive a genuinely issued context, and capture it. Consequential only
   against stores the caller already holds; the identity is the fixture run's,
   which no real store has recorded. The suite's own module doc overstates
   this and should be trimmed.
2. **The observation-owner write is ungated.** `bind_observation_bundle` is a
   plain public port write and `ObservationBinding::new` is public, so any
   in-process holder of the real state store can DEFINE the observation for a
   not-yet-bound iteration (write-once still prevents substitution). This is
   asymmetric with `accept_comparison`, which is capability-gated. The critic
   names it the largest residual risk and suggests a capability token on the
   observation-owner write. Recorded as the successor to the "no production
   writer" residual: when the Task-12 controller wires the real writer, that
   write should be gated, not merely called.
3. **Blindness is field-name-only** — builder prose can ride in
   `unavailable_evidence[].reason`, facet `command`, and
   `observation_failures[].detail`. Already stated in the source; P2's
   canaries are excluded structurally, not by scanning.
4. **`assert_blind` inside `issue` is unfalsifiable** — self-declared in the
   source. Honest, not hidden.

## Toothless checks the critic found — both fixed in this round

- **`verify_envelope_digest` was deletable with every test green**, and
  `p2_adapter_refuses_a_stale_envelope_digest` was named for a refusal it
  never exercised (its whole body asserted `is_ok()` on a well-formed
  envelope). Fixed: the refusal branch is now exercised in-crate, where a
  stale envelope can actually be built, by
  `comparator::canonical_tests::an_envelope_whose_digest_does_not_label_its_bytes_is_refused`
  (**bite proven**: neutering `verify_envelope_digest` to always-`Ok` fails
  it). The CLI test is renamed
  `a_stale_envelope_digest_is_unrepresentable_from_outside_the_port` and its
  doc now states exactly what it proves — that no sequence of public calls
  produces a stale envelope — and points at the test that proves the refusal.

## One correction to the critic's report

The critic reported the port-side self-digest re-derive
(`comparator.rs`, `ObservationView::project`) as "deletable with all tests
green". It is not: they mutation-tested `gauntlet-ports`, `gauntlet-app` and
`p2_context_isolation` but not the adapter suites. Deleting that check fails
`gauntlet-adapter-compare-diff::a_bundle_that_misdeclares_its_own_observation_digest_is_refused`
— re-verified here. The check is tested; the app-layer recompute masks it in
the CLI test only, which is why the adapter-level tooth exists.

## Gates

`just test` **697/697** · `just check` · `just lint` · `just deny` ·
`just boundaries` **194/194** · `just ruler` — all green.

No frozen Wayfinder material was modified; no reference candidate was created
or promoted.
