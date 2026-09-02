# Task 20 — COR-F correction record (falsifier round 1, COR-F1..F6)

Builder COR-F, 2026-08-31. Work order: `falsifier-adjudication-r1.md`. All six
adjudicated corrections landed and verified. Tree outranks this record; every
claim below is backed by the battery at the end and the per-correction tests
named in place. `just ruler` PASS before and after the edits; no frozen file
(`.wayfinder/**`, `.agents/specs/**`, `.gauntlet/*.lock`, the fixture's
`reference.lock`) was touched; builders did not commit.

## COR-F1 (BLOCKING) — producer corroboration in the composed produced-artifact world

**Root cause (falsifier F-1).** In the `ProducePolicy::BindLocatorContent`
world the §14.2 produce step ingested whatever bytes the dispatched child left
at the unit's locator and the durable production record attributed them to the
instrument; the test-report rung then decided purely over those bytes. A liar
who forges the report CONTENT (`failures: 0`) over marker-less source settled
every unit `implemented-and-proven` — the loop's only reality probes (the
observation facets) were never consulted at the settlement boundary.

**The exact corroboration matching rule implemented** (the design question the
work order delegated; strictest honest reading, named here as instructed):

> A produced locator artifact of an `evidence_required.producer = instrument`
> unit is **corroborated** iff THIS completion's observation bundle — the
> bundle assembled and bound at this (run, iteration), re-verified by
> equality of the document's `run_id` and `iteration_no` against the settling
> loop's own facts ("produced by the observation instruments in THIS run") —
> contains at least one instrument facet record where ALL of:
> 1. `kind` is `test` — compared through the ports vocabulary
>    (`FacetKind::Test.token()`, never a spelled string), the instrument
>    class whose executions produce test reports;
> 2. `command` is present AND `exit_status` is present — a REAL instrument
>    execution over the completion's evidence workspace (the observation
>    request anchors at the same workspace the produce step bound the
>    locator artifact in — COR-D3's one-surface anchoring is the
>    "covering" relation, structurally, not per-fact);
> 3. `exit_status == 0` — the instrument's own execution SUCCEEDED,
>    agreeing with the passing report the artifact claims.

No such record ⇒ the unit does NOT settle: a typed
`{"class":"producer_uncorroborated", run, unit, locator, detail}` record is
pinned as evidence, the settle call is never reached, the unit stays open,
and the run continues to its bounded non-settled terminal (never a run
error, never a silent downgrade). The gate fires only when
`ProducePolicy::BindLocatorContent` AND `producer == instrument` AND the
governed verification decided PASS; the default `compose()` world
(`MaterializeObservation`) never consults it — its locator artifact IS the
instrument's own facet document, and the frozen proofs decide through it
byte for byte (p1–p12/g6/g7/g10 all green UNMODIFIED, below). Note the gate
also covers the produce step's fallback branch (locator empty → observation
facet materialized): stricter than the minimum, per the strictest-honest
instruction.

**Loci touched.**
- `crates/gauntlet-app/src/control/controller.rs`:
  `RunController::produced_artifact_corroborated` (the rule, documented in
  place) + the pre-settlement gate in `process_completion`.
- `tests/fixtures/target/GAUNTLET.md`: `observation.facets: [filesystem, test]`
  + a fixture section documenting the facet and the `GAUNTLET_TOOL_TEST`
  binding — the instrument now ACTUALLY executes the project's check each
  completion.
- `tests/fixtures/mock_runner_scenarios/e2e-forged-report.yaml` (NEW): the
  falsifier's own attack shipped — marker-less source, FORGED passing reports
  at every locator, result document claims success.
- `crates/gauntlet-cli/tests/e2e_full_loop_mock.rs`: `World::env` now sets
  `GAUNTLET_TOOL_TEST` to the world's check command (derived from the
  fixture's scripts dir, made executable test-side; no command/path literals —
  AK-016 compliant); NEW leg `the_report_forging_liar_is_never_settled`
  asserts the NON-settlement: non-zero exit, bounded non-settled terminal,
  the per-unit negative (`implemented-and-proven` unpopulated for AK-901/902/
  903 in the parsed report JSON), `settled=0`, zero `"record":"settlement"`
  export rows, the forged bytes durably in the store, AND the pinned
  `producer_uncorroborated` record — proving the block was THIS gate.

**Both-ways tests.**
- Corroborated settles: black-box, `the_composed_loop_settles_every_unit_...`
  (happy leg with the live test facet, exit 0) — green; white-box,
  `a_corroborated_produced_artifact_settles_in_the_produced_world`.
- Uncorroborated blocks: black-box, the new forged-report leg — green;
  white-box, `an_uncorroborated_produced_artifact_never_settles_in_the_
  produced_world` (a filesystem-only observation; rung PASS; zero settlement
  rows; pinned block record) — green.
- The rule itself is pinned token-by-token in
  `the_corroboration_rule_demands_this_runs_executed_passing_test_facet`:
  passing test facet in this run/iteration ⇒ true; filesystem-kind facet ⇒
  false; failing test facet ⇒ false; command-less attestation ⇒ false;
  foreign run ⇒ false; later iteration ⇒ false; empty bundle ⇒ false.

**Red-state evidence.** The falsifier executed the attack against the
pre-fix binary and observed `terminal=settled` with all three units proven
(`falsifier-round1.md` F-1 reproduction); the shipped leg now pins the
opposite verdict on the same attack.

## COR-F2 — `load_scenario` stem containment

**Root cause.** The loader joined the raw `GAUNTLET_MOCK_SCENARIO` stem into
the frozen-locator dir; the only guard was the `scenario:` identity
agreement, so a traversal stem with a matching identity field loaded any
host YAML (F-1's delivery channel).

**Fix.** `crates/adapters/gauntlet-adapter-agent-mock/src/scenario.rs`:
before ANY path join the stem must be a simple fixture name — non-empty, no
leading `/`, no whitespace, no path separator (`std::path::is_separator`),
and every path component a plain `Normal` component (refuses `..`, `.`).
Refusals are typed `ScenarioError::ValueInvalid` naming the stem rule. The
identity check remains as a consistency pin, no longer a containment guard.

**Tests.** `a_traversal_or_pathed_stem_is_refused_before_any_join` (the
falsifier's own `../../../../../../tmp/falsifier-t20/forged-report` stem
among the refused class) and
`a_simple_fixture_stem_still_loads_from_the_frozen_locator` (real fixtures
still load; an absent simple name stays the typed `FixtureUnreadable`).
Platform honesty recorded in the test: a backslash stem is NOT refused on
POSIX (it cannot traverse there; it can only name an absent fixture).

## COR-F3 — TicketId grammar + queue-root containment on read AND write

**Root cause.** The port's id grammar accepted `..` (non-empty, ≤128, no
leading `/`, no whitespace) and the fs queue joined it unsanitized; the
falsifier read an out-of-root ticket file and was one grammar change from an
out-of-root write through a forged record's own `ticket` field.

**Fix.**
- `crates/gauntlet-ports/src/human_escalation.rs`: `TicketId::new` additionally
  refuses every path separator and requires all-Normal components — the id is
  an opaque token, never a path fragment.
- `crates/adapters/gauntlet-adapter-escalation-fs/src/queue.rs`: new
  `contained_ticket_path` validates, before every read AND write, that the
  file name carries no separator, the join's parent IS the queue root, and no
  traversal component survives; typed `RecordInvalid` refusal otherwise. Used
  by `record`, `poll`, `escalate` (collision probe + write) and `resolve`
  (which writes to the record's OWN ticket field — now doubly guarded: the
  tightened grammar refuses a traversal field at decode, and the write path
  validates containment independently).

**Contract suites updated.** `tests/contract_suite.rs` gains:
`a_traversal_ticket_id_is_refused_by_the_grammar`,
`an_out_of_root_ticket_file_is_refused_on_the_read_path` (the falsifier's F-3
reproduction shipped as a tooth: nothing outside the root is read),
`a_forged_records_own_traversal_ticket_field_cannot_steer_the_write` (the
forged write-back is refused at decode; no out-of-root or traversal-named
file appears). Ports/app/cli suites that mint plain ids are untouched and
green.

## COR-F4 — `escalations list` quarantines corrupt files

**Root cause.** `queue.list()` failed the WHOLE call on the first
`RecordInvalid`, so one truncated/corrupt file blinded the operator's entire
queue (typed but denial-of-view).

**Fix.** `queue.list()` returns `QueueListing { records, quarantined }`:
valid records as always (same total order), each corrupt file as a typed
`QuarantinedTicket { ticket: <file stem>, class: QuarantineClass }` where the
class is COARSE (`Unreadable` | `RecordInvalid`) — the file's bytes, fields,
and decode-detail are never echoed, so a tamper attempt learns nothing from
the operator's own list. `gauntlet escalations list` prints quarantined
entries (`state=QUARANTINED reason=<class>`) and exits 0; valid tickets still
list. Actions (`resolve`/`retry`) on a quarantined ticket fail closed at the
queue's read path — pinned by
`an_action_on_a_quarantined_ticket_fails_closed`. Also added:
`a_corrupt_file_is_quarantined_beside_the_valid_tickets`,
`an_unreadable_file_quarantines_as_unreadable`.

**Mechanical adaptation (declared per the round constraint).**
`the_list_is_total_ordered_oldest_first_and_complete` now reads
`listing.records`/`listing.quarantined` — the list surface changed because
quarantining IS the correction; the original assertions (both tickets listed,
oldest-first, completeness) are unchanged in strength.

## COR-F5 — skills lock writer emits the reader's schema

**Root cause.** `skills_cmd::install` bound the entries array inside a second
nested object under the root `skills` key (`{"skills":{"skills":[…]}}`),
while `read_lock` requires the root `skills` field to BE the array — the
documented install → verify round trip failed typed on every tree (F-6), and
no shipped test noticed.

**Fix.** `crates/gauntlet-cli/src/skills_cmd.rs`: the WRITER emits the
reader's schema — root `skills` is the array. The reader was NOT widened.

**New round-trip proof.** `crates/gauntlet-cli/tests/skills_roundtrip.rs`
(NEW) drives the real binary in a scratch project root seeded with the eight
`REPO_OWNED_SKILLS` sources copied from the repository's own `.agents/
skills/` tree:
- `install_then_verify_round_trips_and_the_lock_is_the_readers_schema` —
  install exits 0 (lint: pass), the written lock parses with the root
  `skills` ARRAY at count == the closed owned list, verify passes;
- `an_out_of_band_edit_is_named_drift_and_reinstall_repairs` — verify fails
  typed naming the skill and both digests; re-install repairs;
- `a_forged_lock_is_a_typed_refusal_and_the_old_nested_shape_never_reads` —
  a forged-digest lock is refused typed, AND the old writer's doubly-nested
  shape is refused with `carries no skills array` (the exact regression,
  pinned shut).

## COR-F6 — resume of an all-settled run is an honest settled no-op

**Root cause.** `resume_run` always re-entered the loop; an all-settled run
planned zero dispatches and burned empty iterations until a budget dimension
refused, ending `budget_exhausted` (rc=1) — a terminal misrepresenting a
fully settled run (F-7).

**Fix.** `crates/gauntlet-app/src/control/controller.rs`, in `resume_run`
after the §5.3.3 event-ordered projection merge: short-circuit to
`RunOutcome { terminal: Settled, iterations: <the checkpoint's own> }` when
(1) the §18.3 step-1 completion had NOTHING to complete
(`settlements_completed == 0`, no production completions, no limitations —
durable state moved ⇒ the full recovery path must run and write its
reconciliation checkpoint), (2) nothing is in flight, and (3) the merged
projection carries every frozen-membership unit Settled. ZERO writes, ZERO
new iterations. The §18.3 step-2 re-freeze-verify runs before the arm, so a
mutated reference still aborts first.

**Design note (recorded honestly).** The first draft keyed the predicate on
the durable settlement ROW set; the EXISTING proof
`resume_restores_the_budget_from_the_checkpoint_watermark` correctly caught
it over-firing: a checkpoint may legitimately project a unit Open postdating
a settlement (the §5.3.3 residual-retry re-open class), and a run that merely
LOOKS settled must take the full path. The shipped predicate uses the
event-ordered projection instead. That existing test is GREEN UNMODIFIED, as
are p7/g7 (`resume` recovery proofs) and the half-applied-settlement
reconciliation proof.

**New pin.** `resuming_an_all_settled_run_is_an_honest_settled_no_op`:
settled terminal at the checkpoint's own iteration (no iteration burned),
exactly the original run's one dispatch, one settlement row, one invocation,
close-out checkpoint unrewritten.

## Files touched

- `crates/gauntlet-app/src/control/controller.rs` (COR-F1 gate + rule, COR-F6
  short-circuit, 4 new tests)
- `crates/adapters/gauntlet-adapter-agent-mock/src/scenario.rs` (COR-F2, 2 new
  tests)
- `crates/gauntlet-ports/src/human_escalation.rs` (COR-F3 grammar)
- `crates/adapters/gauntlet-adapter-escalation-fs/src/queue.rs`,
  `.../src/lib.rs` (COR-F3 containment, COR-F4 quarantine + exports)
- `crates/adapters/gauntlet-adapter-escalation-fs/tests/contract_suite.rs`
  (6 new tests + the one declared mechanical adaptation)
- `crates/gauntlet-cli/src/skills_cmd.rs` (COR-F5 writer)
- `crates/gauntlet-cli/src/escalations_cmd.rs` (COR-F4 CLI arm)
- `crates/gauntlet-cli/tests/e2e_full_loop_mock.rs` (COR-F1 leg + tool binding)
- `crates/gauntlet-cli/tests/skills_roundtrip.rs` (NEW, 3 tests)
- `tests/fixtures/target/GAUNTLET.md` (test facet + docs)
- `tests/fixtures/mock_runner_scenarios/e2e-forged-report.yaml` (NEW)

## Assertion changes with reasons

1. `the_list_is_total_ordered_oldest_first_and_complete` — mechanical
   adaptation to the `QueueListing` surface (COR-F4 IS the surface change);
   assertion strength unchanged. No other existing assertion was weakened,
   disabled, or regoldened; `cli_help.txt` is untouched (no CLI surface
   change); p1–p12, g6, g7, g10, compose_teeth, escalations_cli, export_stack
   all green UNMODIFIED.

## Battery (all personally run under the compile-lock protocol, 2026-08-31)

- `just check` — clean (workspace, all targets)
- `just test` — **1349/1349** (was 1333; +16 new correction tests)
- `just lint` — clean (clippy `-D warnings`)
- `just deny` — clean (advisories only: license-not-encountered ×4, known
  winnow duplicate)
- `just boundaries` — **228/228**
- `just ruler` — **PASS** (answer key + manifest digests match the frozen
  lock), run before and after the edits
- `just doctor` — exit OK (70 units, frozen digests match)
- `cargo nextest run -p gauntlet-cli --test e2e_full_loop_mock --test
  cli_surface` — **9/9**
- repo-root dry-run — exit 0, `lock verified (70 unit(s) frozen)`, and a
  before/after whole-tree content-hash diff plus `git status` diff proving
  ZERO writes
- format gate: `cargo fmt --all` applied; arch scanners (literals, denylist,
  escapes, deps, envfile) green inside the boundaries run

Logs: `corf-*.log` in this directory.

## Deviations / residuals

- None from the adjudication. Notes: (a) the COR-F1 gate's "covering"
  relation rests on COR-D3's one-workspace anchoring (the observation request
  and the locator live in the same completion workspace), stated in the
  rule's doc — strengthening it to per-artifact coverage would need a
  facet-schema change (B.3 wire), which is a spec revision, not a correction;
  (b) the fixture's composed runs now REQUIRE `GAUNTLET_TOOL_TEST` (the test
  facet fails composition closed without it) — supplied by the E2E harness;
  compose_teeth/escalations_cli write their own GAUNTLET.md and are
  unaffected (both green); (c) `escalations resolve/retry` success remains
  unreachable by design (F-5/carried posture, cor-d-report.md) — untouched.
