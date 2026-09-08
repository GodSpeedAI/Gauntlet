# Task 26 D+Critic Apparatus Repair — Plan

Recorded BEFORE most of the execution below (per instruction: "record
the bounded repairs before executing them... if reality forces a
deviation, append it rather than rewriting the original plan").

## Original plan (as formed after reading the diagnosis materials and
tracing B1's refusal paths)

1. Commit a new sibling fixture, `tests/fixtures/baseline-dcritic/
   gadget-task/`, cloned from the canonical `tests/fixtures/baseline/
   gadget-task/`, with `review.max_critics: 1`, `sandbox.backend: bwrap`,
   and a committed `.gauntlet/policies/contained.json` mirroring
   `tests/fixtures/target-real/`'s existing artifact.
2. Extend the new fixture's `agent/child.sh` critic branch to emit a
   real `comparison_record@v1` payload (the shape the real
   `ModelComparator` requires), reading the four fields
   `validate_document_binding` checks from the delivered
   `ENVELOPE-PATH` JSON.
3. Smoke-run `improve baseline --profiles D` (a single-scenario scratch
   copy, never the five-scenario matrix) against the new fixture; expect
   GREEN admission through settlement.
4. Audit the three uncommitted production changes: reproduce what each
   was fixing, add discriminating tests, keep or repair each on its own
   merits.
5. Verify `review_dispatches` becomes non-zero and durably truthful;
   fix the metric if it does not.
6. Run the full `just ci` battery; fix anything the new fixture or code
   changes break.
7. Write the required artifacts and leave the tree in an authoritative
   (committable) state.

## Deviations forced by reality (appended, in the order discovered)

### D1 — Change 2 ("workspaces/workspaces") is NOT a defect

The plan assumed (from the independent verification report) that this
double join was a probable defect needing justification or a fix. Once
step 3's smoke run required tracing exactly what the composition-time
wrap check binds `$WORKSPACE_ROOT` to, the doubled path turned out to be
a correct, intentional consequence of two independently-composed path
segments (`AnchorRoots.workspace_root = <state>/workspaces` and a fixed
synthetic pre-flight `WorkspaceRef` literally named `"workspaces"`) —
confirmed by an EXISTING task-25 E2E test that manually pre-creates the
identical path as a caller-side workaround. Plan step 4 changed from
"repair or justify" to "verify and document the correction to the prior
diagnosis" — no code change was needed for this one.

### D2 — B1's fix alone did not make `review_dispatches` truthful

Step 3's first successful smoke run (config admits, sandbox admits,
critic dispatches, comparison accepted, unit settles) still reported
`review_dispatches: 0`. This was NOT anticipated by the original plan,
which treated B3 ("critic invocations must be durably measurable") as
already addressed by the existing `RecordingAgentRunner` wrap (change
3). Direct durable-store inspection (raw `event_log` rows) was needed to
find the actual cause, which turned out to be TWO independent defects,
neither previously identified in any prior report:

- **D2a**: `MandatoryComparisonAcceptance::accept`
  (`gauntlet-ports/src/comparator.rs`) durably records an accepted
  comparison but never appends the sibling `InvocationTerminal` event
  the builder dispatch path always writes. Repair: append it, threading
  a new `at: EpochSeconds` parameter through `MandatoryComparisonAcceptance
  ::new` and `IssuedComparison::review(...)` (6 call sites: 1 production
  in `controller.rs`, 5 test in `p2_context_isolation.rs` and
  `antigaming.rs`).
- **D2b**: even after D2a, `derive_loop_metrics`'s `review_dispatches`
  formula (`gauntlet-app/src/baseline/metrics.rs`) computed a SIZE
  subtraction between two sets that are DISJOINT in the real runtime
  (builder invocations never get their own terminal event either),
  not the SUPERSET relationship the formula silently assumed. Repair:
  a genuine set-difference count.

This is the plan's largest deviation: it turned a "verify an existing
mechanism" step into two new, independently-scoped production fixes,
each with its own discriminating test (RED reproduced and recorded for
both — see `repair-evidence.md`).

### D3 — Existing test extended rather than a new file written for B3

The plan did not specify where B3's regression test would live. Given
`p2_context_isolation.rs` already carried
`durable_critic_invocation_and_accepted_document_survive_reopen_and_
reject_crosswire` — an existing test exercising the EXACT accept path
D2a touches, down to the same `RecordingAgentRunner` + durable reopen —
extending it with one new assertion (on `run_lifecycle_facts().
invocation_terminals`) was the smaller, more precise change than a new
standalone test file duplicating its setup.

### D4 — README added to the new fixture directory (not in the original
plan's file list)

Not strictly required by any lettered blocker, but necessary to make
the fixture's THREE differences from the canonical one reviewable
without a diff, and to record — in the one place a future reader of
`tests/fixtures/` would look — why this is a sibling directory and not a
modification of `tests/fixtures/baseline/` (B2/B8's constraint). Added
as `tests/fixtures/baseline-dcritic/README.md`.

## What did NOT deviate

- No production code weakening of Core-spec §13.2 was made or
  considered (B1).
- No attempt was made to reconstruct the lost fixture (B2).
- No change was made to alter or "fix" Profile C's behavior on the
  canonical fixture (B8) — it is byte-identical to before this repair.
- `producer: instrument` (B9) was not touched, redefined, or resolved.
- AK-069 was not upgraded and no conformance artifact was hand-edited.
