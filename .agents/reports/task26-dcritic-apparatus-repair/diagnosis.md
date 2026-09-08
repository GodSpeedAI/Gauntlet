# Task 26 D+Critic Apparatus Repair — Diagnosis

Repairer role (not the experimenter): this document maps each blocker the
independent verification report
(`.agents/reports/task26-followup-independent-verification.md`) raised to
its root cause and repair, re-derived against source rather than accepted
on the report's authority alone. Two blockers turned out to need
correction from that report's own framing (noted inline); the rest are
confirmed as stated.

---

## B1 — Critic configuration cannot run

**Status:** FIXED (fixture-only repair; no spec/production weakening).

**Root cause:** the canonical fixture's `sandbox.backend: none` plus
`review.max_critics: 1` triggers Core-spec §13.2's own correct refusal
("a model critic requires process-or-stronger runner isolation") — this
is not a defect, it is the spec working as intended. Switching to
`sandbox.backend: bwrap` then failed differently: `compose_run`'s
composition-time dry-run wrap check (`govern::AuthorityGate::admit`,
`crates/gauntlet-cli/src/compose.rs`) requires a resolvable
contained-tier sandbox policy artifact at `<project>/.gauntlet/policies/`
(the default `sandbox.policy_dir`, `gauntlet_app::config::defaults::
policy_dir_at`), which the canonical fixture never carried (it is
conservatively `none`-postured by design, per its own README).

**Source evidence:**
- `crates/gauntlet-app/src/config/resolve.rs:609` — `sandbox_policy_dir`
  defaults to `<project_root>/.gauntlet/policies`.
- `crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs` — the
  `PolicyProvider` that resolves `<policy_dir>/<tier-token>.json`.
- `tests/fixtures/target-real/.gauntlet/policies/contained.json` — the
  existing, task-25-precedent policy artifact shape this repair mirrors
  exactly (same `reference`/`tier`/`backend`/`network`/`writable_roots`/
  `denied_paths` schema, same `bwrap` backend, same
  `deny_by_default` network posture, same `$WORKSPACE_ROOT`-only
  writable root).
- Executed refusal transcripts (both reproduced by me before the fix,
  against SCRATCH copies only — see `repair-evidence.md` §B1):
  - `sandbox.backend: none` + `max_critics: 1`: *"a model critic requires
    process-or-stronger runner isolation; isolation:none cannot
    establish fresh-context containment (spec 13.2)"*.
  - `sandbox.backend: bwrap` + `max_critics: 1`, no policy artifact:
    *"the contained tier policy could not be resolved for unit AK-961:
    no sandbox policy exists for the contained tier"*.

**Repair:** a NEW, committed sibling fixture,
`tests/fixtures/baseline-dcritic/gadget-task/` (never the canonical
`tests/fixtures/baseline/gadget-task/`, per B8 — see below), carrying:
`review.max_critics: 1`, `sandbox.backend: bwrap`, and a committed
`.gauntlet/policies/contained.json` (both AK-961 and AK-962 are
`blast_radius: contained`, so this one tier file is sufficient).

**Test:** R1/R6 (`repair-evidence.md` §R1/R6) — the fixture's
configuration and sandbox policy resolve and admit a real critic
dispatch under bwrap, executed twice from clean scratch state with
identical nonvolatile results.

**No spec weakening:** §13.2's isolation requirement is untouched and
its refusal under `none` is unchanged and still tested (R5).

---

## B2 — Fixture must be authoritative and reproducible

**Status:** FIXED.

**Root cause (as the independent verifier established):** the invalid
follow-up ran against a fixture whose digest (`sha256:29dc2ed0…`)
matches neither the committed nor the current on-disk fixture
(`sha256:33bd5a48…`, identical to each other) — an unrecorded, reverted
mutation.

**Repair:** rather than reconstruct the lost fixture (explicitly
disallowed unless independently required — it is not: the intended
design only needs `max_critics: 1` + a resolvable sandbox policy, both
addressed by B1's minimal, explicit change set), this repair creates the
smallest new committed fixture that supports the experiment, in its own
directory (`tests/fixtures/baseline-dcritic/`) so it can NEVER be
confused with, or silently widen, the canonical `tests/fixtures/
baseline/` fixture the plan gate's frozen command spelling and the
committed `baselines.json` (AK-069) depend on.

Every changed file relative to the canonical fixture is enumerated in
`tests/fixtures/baseline-dcritic/README.md` (exactly three: `GAUNTLET.md`,
the new `.gauntlet/policies/contained.json`, and `agent/child.sh`'s
critic branch) and individually reviewable via `git diff` against the
copy origin.

**Canonical fixture digest** (same algorithm `improve_cmd.rs::
fixture_digest` uses):
```
sha256:fe8cf5a6b9a852991d73d05698742b5fb94c8ead496571c3cd2ab35fce6c4f2d
```
recorded in `apparatus-manifest.json` and `handoff.md`, and it MUST match
at experiment time — see `handoff.md`'s stop conditions.

**Test:** R7 — two runs against fresh scratch copies of the committed
fixture reproduce identical fixture digests and identical nonvolatile
leg results (`repair-evidence.md` §R7).

---

## B3 — Critic invocations must be durably measurable

**Status:** FIXED — and this is where the diagnosis materially departs
from the independent verification report's own framing.

**What the verification report got right:** `review_dispatches = 0`
does NOT mean the aggregate metric failed to notice a real critic — it
correctly reported that no durable evidence of critic execution existed
under the invalid follow-up's (unreconstructable) run.

**What this repair discovered that the verification report could not
reach** (it never got a working D+critic apparatus to inspect): once B1
made a real critic dispatch possible, TWO independent, distinct,
previously-latent defects surfaced, both upstream of the report's own
diagnostic reach:

### B3a — `MandatoryComparisonAcceptance::accept` never wrote a terminal event

Durable inspection (raw `event_log` rows from a real `improve baseline
--profiles D` run, `repair-evidence.md` §B3) showed: BOTH invocations
(the builder AND the newly-dispatched critic) get an `invocation_admitted`
event at dispatch (`RecordingAgentRunner::invoke`,
`crates/gauntlet-app/src/compare/recording.rs`). Only the BUILDER path
(`controller.rs`'s own dispatch loop, `append_terminal_event` at
`crates/gauntlet-app/src/control/controller.rs:1374`) ever wrote a
matching `InvocationTerminal` event. The critic/review path
(`gauntlet-app/src/compare/review.rs`'s `collect_next` →
`accept_product` → `gauntlet_ports::comparator::
MandatoryComparisonAcceptance::accept`) recorded the accepted comparison
document (`comparison` record, `accept_comparison`) but never appended
the sibling `InvocationTerminal` event — so an admitted, ACCEPTED critic
invocation was durably invisible to every reader that reconstructs "which
invocations actually ran" from lifecycle facts, `derive_loop_metrics`'s
`review_dispatches` among them.

**Repair:** `MandatoryComparisonAcceptance::accept` now appends a
`RunEventKind::InvocationTerminal { invocation_id, state:
InvocationState::Completed }` event immediately after
`accept_comparison` durably succeeds — mirroring, in meaning and event
kind, exactly what the builder dispatch path already does (spec §5.3.5:
"An invocation reached a terminal state"). `MandatoryComparisonAcceptance`
gained a fourth field, `at: EpochSeconds` (the timestamp on that event),
threaded from the same composition-time snapshot
`RecordingAgentRunner::admitted_at` already uses for every invocation it
admits in one run — no new "real now" dependency was introduced; the
existing per-run-snapshot convention was extended, not replaced. This
required a small, mechanical signature change to `IssuedComparison::
review(...)` (one new `at: EpochSeconds` parameter) and its six call
sites (1 production, 5 test) — see `repair-evidence.md` §B3a.

### B3b — `derive_loop_metrics`'s `review_dispatches` formula assumed a superset that does not hold

Independently, once B3a made critic terminal events exist, the baseline
report STILL read `review_dispatches: 0` for a leg with two real,
accepted critic invocations. Root cause:
`crates/gauntlet-app/src/baseline/metrics.rs`'s
`derive_loop_metrics` computed
`all_invocations.len().saturating_sub(dispatched_invocations.len())` — a
SIZE subtraction, silently assuming `invocation_terminals` is always a
SUPERSET of `unit_dispatches` (every unit-dispatched invocation also
carries its own terminal fact, so subtracting counts leaves exactly the
"extra" invocations). Durable inspection of the real composed loop
disproves this: unit-dispatched BUILDER invocations are recorded via
`unit_dispatch` events only and (independently of B3a) never receive
their own `InvocationTerminal` event either. The two sets are DISJOINT
in the real runtime, not nested, so the old formula computed `|{critic1,
critic2}| − |{builder1, builder2}| = 2 − 2 = 0` for a leg where two
critics genuinely ran and were accepted.

This module's own existing unit test
(`derive_loop_metrics_reads_the_durable_record`) never caught this
because its fixture (`settled_store`) constructs `invocation_terminals`
as a superset containing the builder's own id ALONGSIDE the observer's —
an assumption about the real runtime's shape that this repair's own
durable inspection refutes.

**Repair:** the formula is now a genuine SET DIFFERENCE
(`all_invocations.difference(&dispatched_invocations).count()`), correct
under both the existing fixture's superset shape (identical result,
`derive_loop_metrics_reads_the_durable_record` unchanged and still
green) and the real disjoint-sets shape (new test
`review_dispatches_counts_a_disjoint_terminal_set_not_a_size_difference`,
`repair-evidence.md` §B3b).

**Do not rely on aggregate counts alone (per this task's own
instruction):** independent of the metric fix, raw durable-store rows
were inspected directly for every claim above — `role_invocation` rows
(role, tier, sandbox_policy_ref, session_ref), `event_log` rows
(`invocation_admitted`, `invocation_terminal`, kind and payload), and
`comparison` records — see `repair-evidence.md` for the full transcripts.

**Test:** R3 (`p2_context_isolation.rs`'s existing
`durable_critic_invocation_and_accepted_document_survive_reopen_and_
reject_crosswire` test, extended with a new assertion on
`run_lifecycle_facts().invocation_terminals` — RED before B3a, GREEN
after) plus the new `metrics.rs` unit test for B3b.

---

## B4 — Three uncommitted production changes

**Status:** all three resolved; one KEPT as-is with a corrected rationale
(change 2 — the independent verification report's own suspicion about it
was WRONG, corrected here against source), one REPAIRED with a proper
named constant and a discriminating test (change 1), one VERIFIED and
extended with a targeted test (change 3). See `repair-evidence.md` for
full RED→GREEN transcripts.

### Change 1 — comparator polling sleep

**Diagnosis:** genuine runtime defect. `drive_to_terminal_with`
(`gauntlet-adapter-compare-model/src/lib.rs`) polled a REAL child process
in a tight loop with NO delay between polls — 24 back-to-back
non-blocking `poll()` calls complete in microseconds, giving a real
(non-mock) child process no wall-clock budget to reach a terminal state
before the probe ceiling exhausts.

**Repair:** the ad hoc `50` was replaced with a named, documented
`POLL_INTERVAL_MS: u64 = 10` local constant (an adapter crate cannot
depend on `gauntlet-app`'s `config::defaults::POLL_INTERVAL_MS` — the
composition-root-only dependency direction the architecture boundary
suite polices — so this mirrors, rather than imports, the SAME 10ms
cadence `gauntlet-adapter-agent-cli`'s own transport poll loop already
uses). The doc comment states the resulting bound in wall-clock terms:
`PROBE_CEILING * POLL_INTERVAL_MS` = 24 × 10ms = 240ms.

**Test:** a new discriminating test,
`the_probe_interval_gives_a_real_child_wall_clock_to_finish`, using a new
`Derailment::DelayedTerminal(n)` scripted-runner mode that reports
`Running` for a fixed number of real polls before `Completed` — RED at
`POLL_INTERVAL_MS = 0` (elapsed ~489µs, executed and recorded), GREEN at
`10` (elapsed ≥10ms, by construction ≥2 real 10ms sleeps).

### Change 2 — the doubled `workspaces/workspaces` join

**Diagnosis: this repair CORRECTS the independent verification report's
own suspicion.** That report flagged this as a "probable defect" without
tracing what consumes the path. Re-derived from source: `compose_run`
sets `AnchorRoots.workspace_root = state_dir.join(WORKSPACES_DIR_NAME)`
(`compose.rs:813`) — i.e. `<state>/workspaces` — and the composition-time
dry-run wrap check
(`govern::GateFacts.workspace = WorkspaceRef::new(WORKSPACES_DIR_NAME)`,
`compose.rs:1133`) deliberately uses a FIXED, literal workspace ref
named `"workspaces"` (not any real per-unit workspace, which does not
exist yet at that point in composition) so the policy resolver's
`$WORKSPACE_ROOT` token — scoped per call to
`workspace_root.join(<that call's workspace ref>)`
(`gauntlet-adapter-policy-fs/src/lib.rs`) — resolves to
`<state>/workspaces/workspaces`, the SAME doubled path, by design, for
this one synthetic pre-flight probe. This is not a copy-paste duplicate;
it is two independent, correctly-composed path segments that happen to
share a name.

**Confirming precedent, independently found:** the task-25 E2E test
(`crates/gauntlet-cli/tests/e2e_full_loop_real_runner.rs::World::
ensure_layout`) ALREADY manually pre-creates this exact directory
(`self.state_dir().join("workspaces").join("workspaces")`, its own
comment: *"the wrap-time workspace directory"*) before invoking the real
binary — because until this repair's change, `compose_run` never created
it itself, forcing every caller through this composition path to
replicate that workaround.

**Repair:** KEPT exactly as delivered — this is a correct fix that moves
a caller-side workaround into the production path it belongs to. No
code change made here beyond what was already in the working tree.

**Test:** covered by every R1–R7 apparatus test, none of which
pre-creates `workspaces/workspaces` — the composition-time wrap
succeeding at all (rather than refusing with `WrapRefused`, the RED
shape independently reproduced during the earlier verification pass)
IS the regression proof.

### Change 3 — `RecordingAgentRunner` wraps the critic runner

**Diagnosis:** conceptually correct, exactly as the independent
verification report concluded — the change that, combined with B3a, now
actually closes the loop.

**Verified:**
- critic dispatch is wrapped: `compose.rs` constructs
  `recording_critic_runner` and passes it (not the bare `critic_runner`)
  into `ModelComparatorSource::new(ModelComparator::with_clock(
  recording_critic_runner, ...))`.
- durable records are written: `role_invocation` rows with `role:
  "critic"`, `permissions: "comparison-envelope-only"`,
  `sandbox_policy_ref: null` (correctly reflecting the envelope-only
  permission scope, distinct from the builder's `sandbox_policy_ref:
  "tier-contained-dcritic-v1"`) — confirmed via raw durable-store
  inspection (`repair-evidence.md`).
- existing semantics unchanged: the builder dispatch path is untouched;
  `RecordingAgentRunner`'s `invoke`/`poll`/`collect`/`cancel` pass-through
  behavior for the critic is identical to its existing behavior for the
  builder.
- the metric sees the invocation: only after B3a+B3b, confirmed by the
  final `review_dispatches: 2` result (`repair-evidence.md` §R3).

**Test:** the existing `p2_context_isolation.rs` durable-binding test,
now extended (B3a's test) to assert the terminal fact this change's
recording makes possible.

---

## B5 — Sandbox posture made real

**Status:** DONE. `bwrap` (the smallest legitimate process-or-stronger
posture Gauntlet ships an adapter for — the same backend
`target-real`'s task-25 fixture uses) with the committed
`contained.json` policy. Verified in order: configuration resolves →
sandbox policy resolves → workspace exists (change 2) → critic runner
admits → critic process launches → critic invocation is durably recorded
(B3a) → envelope is inspectable (B6). No scenario-semantic assertion was
made — the smoke critic answers `comparison_record@v1` genuinely (both
marker tokens really checked) but with plain, non-scenario-tuned
rationale text; see `tests/fixtures/baseline-dcritic/gadget-task/
agent/child.sh`'s own comment for the explicit disclaimer that scenario
semantics are NOT pre-baked by this repair.

---

## B6 — Critic context isolation

**Status:** ALREADY PROVEN, retained. `crates/gauntlet-cli/tests/
p2_context_isolation.rs::the_envelope_is_structurally_blind_and_carries_
instrument_reality` asserts over the SERIALIZED envelope (not type
shape) that it excludes builder trajectory, reasoning, identity, prior
verdicts, and held-out information — exactly Core-spec §13.2's
allowlist. This repair did not need to add new isolation coverage; it
retained this test (`cargo nextest run -p gauntlet-cli --test
p2_context_isolation` — full suite green, `repair-evidence.md` §R4) and
additionally confirmed, via the new fixture's own `agent/child.sh`
critic branch, that the real envelope delivered to a bwrap-sandboxed
critic process contains exactly the documented allowlist fields
(`run_id`, `iteration_no`, `reference_digest`, `reference_view`,
`observation`, `contract`, `constraints`, `open_residuals`) and nothing
from the builder's own result document, trajectory, or identity — the
child script structurally cannot read what was never delivered to it.

---

## B7 — Metric truthfulness

**Status:** FIXED (see B3 above for the two defects and their repairs).
Final state, confirmed by direct execution:
- `review_dispatches` now reflects actual durable critic invocations
  (`2` for a 2-unit honest-success leg with two accepted critic
  comparisons — matches `dispatches: 2` exactly, one critic per unit).
- `dispatches` is documented (in this repair's comments and in
  `metrics.rs`'s own doc comment, unchanged) as unit-attributed BUILDER
  dispatches — never described as critic dispatches anywhere this
  repair touched.
- actual critic invocation IDs ARE recoverable: durable `role_invocation`
  rows carry the invocation id, role, runner, tier, permissions, and
  session ref; `run_lifecycle_facts` recovers the terminal fact by id.
- no metric generalization across A/B/C/D was attempted beyond the two
  fixes above — A/B/C's `review_dispatches`/`instrument_runs` semantics
  (already documented as non-comparable across profiles in the report's
  own `_meta` disclosures) are untouched.

---

## B8 — Profile C preserved

**Status:** DONE — the CANONICAL fixture (`tests/fixtures/baseline/
gadget-task/`) is BYTE-IDENTICAL to before this repair (verified: `git
diff` against it is empty; its digest is still `sha256:33bd5a48…`).
Profile C's behavior against the canonical fixture is unchanged (a full
`--profiles A,B,C,D --tasks tests/fixtures/baseline` run, done as part of
the existing `the_plan_gate_shape_writes_the_charter_report_and_the_
matrix_holds` test in `just ci`, remains green with the ORIGINAL task-26
numbers).

Profile C's completion figure AGAINST THE NEW `baseline-dcritic` fixture
reads `0` — because that fixture's critic branch answers
`comparison_record@v1` (required for D), which C's own lightweight
executor does not parse as C's expected shape. This is disclosed
explicitly in `tests/fixtures/baseline-dcritic/README.md` ("Scope:
Profile D only") rather than "fixed": tuning C to also parse this new
shape would mean maintaining two critic response contracts in one
fixture script for a profile this repair was never asked to make work,
and would risk exactly the kind of untested, dual-purpose complexity the
task's own instruction ("do not tune C") warns against.

---

## B9 — `producer: instrument` question preserved, not resolved

**Status:** preserved as an open question; NOT touched by this repair,
and NOT a blocker for apparatus readiness (no D+critic admission,
dispatch, recording, or isolation path depends on how this question
resolves). See `.agents/reports/task26-dcritic-apparatus-repair/
candidate-revision-producer-instrument.md` for the recorded ambiguity,
carried forward verbatim from the independent verification report's §10
without reinterpretation.
