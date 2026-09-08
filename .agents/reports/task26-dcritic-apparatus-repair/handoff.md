# Task 26 D+Critic Apparatus — Handoff to the Experimental Agent

You are the fresh agent who will run the authoritative D+critic
experiment. This repair built and verified the APPARATUS only — it did
not run, and must not be read as having run, the five-scenario
semantic experiment. Read `diagnosis.md`, `repair-plan.md`, and
`repair-evidence.md` in this directory before proceeding; they explain
WHY the apparatus is shaped this way, including two production defects
this repair found and fixed that no prior report had identified.

---

## Exact pinned state

```
HEAD:                  e4ca39358c7fbaafce7a5b202672fe332cfd5add
                        (unchanged by this repair — no commit was made)
fixture path:           tests/fixtures/baseline-dcritic/gadget-task
fixture digest:         sha256:fe8cf5a6b9a852991d73d05698742b5fb94c8ead496571c3cd2ab35fce6c4f2d
GAUNTLET.md digest:     sha256:7066be53aba01059c36c1a9207ffafbe808b07cd178c517c886ea48b4b96c100
sandbox policy digest:  sha256:dd923be057e3a1f612c17599b7a9821b3b91db2d01fcbb4f2126504c9d98a4d8
                        (tests/fixtures/baseline-dcritic/gadget-task/.gauntlet/policies/contained.json)
child.sh digest:        sha256:60b6f28492d8a5721b63eb02c3520a307a44a8eca5399cb21883293282390e71
```

**The repository was NOT committed by this repair.** The apparatus change
set is fully enumerated and battery-green (see
`apparatus-manifest.json`'s `change_set_files`), but it sits in the
working tree alongside unrelated pre-existing dirt this repair did not
touch (`apparatus-manifest.json`'s
`unrelated_preexisting_dirt_not_touched_by_this_repair`). **Before running
the authoritative experiment, get the change_set_files committed** — by
the operator, or by you if the operator has authorized you to commit —
so the experiment runs against a pinned, reviewable tree rather than
uncommitted working state. Re-verify every digest above against the
committed tree before proceeding; do not trust this document's digests
if the commit step changed anything.

---

## Exact command to run the experiment

**Do not point `--tasks` at `tests/fixtures/baseline-dcritic` directly
without first deciding your scenario scope** — `improve baseline`
enumerates every scenario in `task.yaml` (`[honest-success,
lying-defect, subtle-defect, compromised-critic, silent]`) for every
listed profile.

Recommended, matching the design's own five canonical scenarios:

```
cargo build -p gauntlet-cli --bin gauntlet
./target/debug/gauntlet improve baseline --profiles D \
    --tasks tests/fixtures/baseline-dcritic \
    --out <your-scratch-dir>/dcritic-experiment-results.json
```

**Read the `--profiles D` constraint carefully:** the report assembler
refuses unless a result exists for EVERY named profile
(`--profiles is required` / "requires exactly one result for profile
A" is the exact refusal you will see if you pass `D` alone and the
assembler still expects A/B/C). If you need the full A/B/C/D matrix for
comparison, know before you run it that **this fixture's Profile C
result is not meaningful** — see `tests/fixtures/baseline-dcritic/
README.md`'s "Scope: Profile D only" section: this fixture's critic
branch answers `comparison_record@v1` (required for D), which Profile
C's own lightweight executor does not parse as its expected shape. Do
not report a Profile C figure from this fixture as a finding about
Profile C.

Use `GAUNTLET_BASELINE_SCRATCH_ROOT=<dir>` to retain each leg's scratch
world (including its SQLite state store) for durable-record inspection
after the run — this is how this repair itself found and fixed the two
metric defects in `diagnosis.md` B3a/B3b, and you should not trust the
aggregate `review_dispatches`/`instrument_runs` figures alone for your
own conclusions either (see "Required evidence to preserve" below).

---

## Required preflight assertions

Before treating any result from this apparatus as evidence, confirm
ALL of the following. If any fails, STOP (see "Stop conditions").

1. `git rev-parse HEAD` matches the pinned HEAD above (or a HEAD you
   have independently confirmed contains an unmodified copy of
   `change_set_files`).
2. `git status --porcelain` shows no modification to
   `tests/fixtures/baseline-dcritic/**` or any file in
   `change_set_files` relative to that commit.
3. Recompute the fixture digest
   (`python3` one-liner in `tests/fixtures/baseline-dcritic/README.md`,
   or `crates/gauntlet-cli/src/improve_cmd.rs::fixture_digest`'s exact
   algorithm) and confirm it matches `fe8cf5a6…` above.
4. `tests/fixtures/baseline-dcritic/gadget-task/GAUNTLET.md` contains
   `review.max_critics: 1` and `sandbox.backend: bwrap` verbatim.
5. `tests/fixtures/baseline-dcritic/gadget-task/.gauntlet/
   policies/contained.json` exists and its digest matches `dd923be0…`
   above.
6. `bwrap --version` succeeds on the execution host (bubblewrap must be
   installed; this repair's environment had 0.11.1).
7. `just ci` (or at minimum `cargo nextest run --workspace` and
   `cargo nextest run -p gauntlet-arch-tests`) is green on the commit
   you are running against.

---

## Required evidence to preserve

For EACH experimental leg, preserve — durably, not just the aggregate
report — the following, using `GAUNTLET_BASELINE_SCRATCH_ROOT` to keep
the leg's own state store and reading it directly (sqlite3, or a small
script against the committed `StateStore` trait's
`run_lifecycle_facts`/`export_jsonl` reads):

- builder invocation IDs (from `unit_dispatch` events / `role_invocation`
  rows with `role: "builder"`)
- critic invocation IDs (`role_invocation` rows with `role: "critic"`,
  `permissions: "comparison-envelope-only"`)
- critic envelopes (the `<state>/runs/<run_id>/inv/<inv_id>/
  <inv_id>.envelope.json` files, or the `comparison` record's
  `envelope_digest` field cross-checked against them)
- comparison records (the `comparison` record_type rows — full JSON,
  including `residuals`, `hard_gate_results`, `verdict`)
- residual IDs (inside the comparison records above — this repair's
  smoke critic emits `residuals: []` on every leg; the authoritative
  experiment's own scenario logic, if it needs richer residuals for
  `subtle-defect`, is YOUR design decision — this repair deliberately
  left it open, see `tests/fixtures/baseline-dcritic/gadget-task/
  agent/child.sh`'s inline comment on the critic branch)
- verification records (`verification_record` rows)
- settlement records (`settlement` rows / `settlement_committed`
  events)
- held-out result (the harness's own `held_out`/`held_out_basis` report
  fields — unchanged by this repair)
- terminal classification (`run_terminal` event / the report's
  `terminal.durable_state`/`terminal.classification`)

**Do not rely on `review_dispatches`/`dispatches`/`instrument_runs`
alone as proof a critic ran or a residual was admitted** — this
repair's own diagnosis.md documents two real defects in exactly those
aggregate counters that were only found by reading the raw event log
directly. The counters are now fixed and this repair's own tests pin
that fix, but treat them as a cross-check, not a substitute, for the
durable records above.

---

## Stop conditions

STOP and report rather than repair, proceed under a workaround, or
reinterpret the apparatus, if:

- HEAD differs from the pinned commit and you cannot independently
  confirm `change_set_files` are unmodified in the HEAD you are on;
- the tree is dirty in any file under `change_set_files` or
  `tests/fixtures/baseline-dcritic/`;
- the fixture digest does not match `fe8cf5a6…`;
- the critic configuration does not resolve (a `Configuration` /
  `WrongShape` / `EnvelopeInvalid` error from `compose_run`);
- the critic fails to dispatch (an `AgentError` before any
  `invocation_admitted` event for a `role: "critic"` row appears);
- required durable state cannot be preserved (the scratch root is not
  retained, or the state store cannot be opened/read after the run);
- sandbox admission fails (`WrapRefused`, "no sandbox policy exists",
  or "process-or-stronger runner isolation" — any of B1's original
  refusal shapes reappearing means the pinned state has drifted from
  what this repair verified).

A failed preflight or a stop condition is EVIDENCE, exactly as this
repair's own instructions treated a failed reproduction — preserve it,
report it, do not silently work around it.

---

## What this repair does NOT claim

- It does not claim the D+critic architectural hypothesis is confirmed.
- It does not claim any of the five canonical scenarios (honest-success,
  lying-defect, subtle-defect, compromised-critic, silent) behave as the
  original design intended under this apparatus — only that a critic
  CAN be dispatched, admitted, sandboxed, and durably recorded, and that
  its comparison IS accepted and its terminal state IS observable.
- It does not claim the smoke critic's plain, non-scenario-tuned verdict
  text is what a real subtle-defect/compromised-critic residual should
  look like — that is squarely the authoritative experiment's own design
  question.
- `producer: instrument` remains an open CandidateRevision
  (`candidate-revision-producer-instrument.md`), unresolved and
  independent of apparatus readiness.

---

## AUTHORITATIVE D+CRITIC EXPERIMENT: NOT RUN BY REPAIR AGENT
