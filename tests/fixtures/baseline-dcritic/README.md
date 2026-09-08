# tests/fixtures/baseline-dcritic/ — the D+critic apparatus fixture

Committed by task26-dcritic-apparatus-repair to make the intended D+critic
arrangement (`review.mode: single`, `review.max_critics: 1`, process-or-
stronger sandbox isolation) genuinely runnable against a real, pinned,
independently-addressable fixture — a SIBLING of `tests/fixtures/baseline/`,
never a modification of it.

## Why a separate directory, not a new task under `tests/fixtures/baseline/`

`tests/fixtures/baseline` is the exact, frozen path the plan gate's own
command spelling names (`gauntlet improve baseline --profiles A,B,C,D
--tasks tests/fixtures/baseline`, `.agents/plans/gauntlet-impl.md`) and
the source of the committed `.agents/reports/baselines.json` (task 26,
AK-069's provisional basis). `improve baseline` enumerates EVERY task
subdirectory under `--tasks`, so adding a task there would silently widen
what that frozen command measures — exactly the unrecorded-fixture-drift
failure mode the independent verification report
(`.agents/reports/task26-followup-independent-verification.md`) found in
the invalid follow-up. This directory is deliberately outside that path:
running the D+critic apparatus never touches, and can never accidentally
widen, the task-26 canonical fixture or its committed report.

## What's here

`gadget-task/` — a full copy of `tests/fixtures/baseline/gadget-task/`
(same units AK-961/AK-962, same scenarios, same declared-check
incompleteness) with exactly three differences:

1. **`GAUNTLET.md`**: `review.max_critics: 1` (was `0`) and
   `sandbox.backend: bwrap` (was `none`) — the configuration Core-spec
   §13.2 actually requires for a model critic to dispatch at all
   ("a model critic requires process-or-stronger runner isolation").
2. **`.gauntlet/policies/contained.json`** (new): the committed
   contained-tier sandbox policy the bwrap backend needs to resolve —
   mirrors `tests/fixtures/target-real/.gauntlet/policies/contained.json`
   exactly (same schema, same `deny_by_default` network posture, same
   `$WORKSPACE_ROOT`-only writable root).
3. **`agent/child.sh`**'s `critic` role branch: emits a real
   `comparison_record@v1` payload (echoing the four fields
   `validate_document_binding` requires: `run_id`, `iteration_no`,
   `reference_digest`, `observation_digest`, read from the delivered
   `ENVELOPE-PATH` JSON — never assumed), instead of the original
   fixture's `builder_result@v1`-shaped placeholder (correct for
   Profile C's own separate, simpler executor, but not a document the
   real `ModelComparator` can admit). See the inline comment in the file
   for the full rationale.

Everything else — `task.yaml`, `.wayfinder/{answer-key.md,reference.yaml}`,
`.gauntlet/{comparison-contract.yaml,reference.lock}`,
`.agents/roles/{builder,critic}.md`, `scripts/{check,held-out}.sh`,
`src/gadget.rs` — is byte-identical to the canonical fixture.

## Scope: Profile D only

This fixture's critic branch answers with `comparison_record@v1`, which
is what the real `ModelComparator`/D's composed loop requires — and which
Profile C's own lightweight executor does NOT parse (it expects the
canonical fixture's `builder_result@v1`-shaped critic reply). Running
`--profiles A,B,C,D` against this fixture is possible (A/B are
unaffected; only D and C's own dispatch shape differ) but C's own
completion figure against THIS fixture is not meaningful and must not be
read as "Profile C regressed" — it is this fixture's critic branch
answering a different contract than C's executor expects, by design.
**The authoritative D+critic experiment runs `--profiles D` only.** See
`.agents/reports/task26-dcritic-apparatus-repair/handoff.md`.

## Fixture digest

Computed by the same canonical algorithm `gauntlet improve baseline`
itself uses (`crates/gauntlet-cli/src/improve_cmd.rs::fixture_digest` —
SHA-256 over every file's relative path and bytes, sorted, from
`gadget-task/` down):

```text
sha256:fe8cf5a6b9a852991d73d05698742b5fb94c8ead496571c3cd2ab35fce6c4f2d
```

This digest is authoritative only once this fixture is committed exactly
as delivered by task26-dcritic-apparatus-repair — see
`.agents/reports/task26-dcritic-apparatus-repair/apparatus-manifest.json`
and `handoff.md` for the exact pinned state a future experimental run
must verify against before proceeding.
