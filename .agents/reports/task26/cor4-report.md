# COR-4 report — task 26 corrections round 4 (falsifier round 1)

Builder: COR-4. Inputs: `falsifier-round1.md` (F1–F7, executed repros),
`charter.md` (binding), the dispatch brief's COR-4 adjudications. All work
in the exclusive scope: `crates/gauntlet-app/src/baseline/**`,
`crates/gauntlet-app/tests/baseline_harness.rs`,
`crates/gauntlet-cli/src/improve_cmd.rs` (cfg-test additions only in
`lib.rs`-adjacent code — no `lib.rs` change needed),
`crates/gauntlet-cli/tests/improve_baseline.rs`,
`.agents/reports/task26/cor4-{progress,report}.md`. NO changes to
`tests/fixtures/**` (frozen), `.wayfinder/`, `.gauntlet/`, the justfile,
or `crates/gauntlet-arch-tests/src/` (no new production write-API sites —
all new writes stay inside the existing materialize/judge sites, so no
inventory entries were required). No `git add`/`commit`.

## Per-finding disposition

**F1 CRITICAL (fix) — judge-channel provenance, fail-closed.** All three
adjudicated legs implemented in the CLI executor (`improve_cmd.rs`):

1. **Wrapper pin.** At D-leg materialization the executor digests the
   materialized wrapper file with the repo's own
   `gauntlet_app::verify::content_digest` and pins the digest IN PROCESS
   ONLY (`JudgeChannel.wrapper_digest`, never written into the leg world).
   Before any grading, `grade_judge_channel` re-digests the wrapper ON
   DISK; a mismatch — or an unreadable/missing wrapper — returns the
   tamper verdict (`held_out = "inconclusive"`,
   `held_out_basis = "judge-channel-tampered"`, tamper fact in the leg
   detail) and NOTHING is executed or graded (the wrapper is never
   executed by the grading path at any point; the tamper short-circuit is
   the first decision).
2. **Snapshot nonce + count cross-check.** The wrapper template now bakes
   a per-leg executor-issued NONCE (a fresh ULID minted in
   `run_full_loop`, held only in `JudgeChannel`) into every snapshot name:
   `<cwd-basename>-<nonce>-<nanostamp>-<pid>.rs|.absent|.copy-failed` (the
   nonce sits after the cwd basename so the gate test's existing
   `pass-wrap-*`/`.rs` assertions hold unchanged). Grading order after the
   digest: (a) every snapshot-dir entry must carry the nonce — an entry
   outside the nonce set was not created by the pinned wrapper (F2's
   additive-planting vector) and is tamper; (b) the nonce-set count must
   EQUAL the leg's durable test-facet-execution count (the wrapper writes
   exactly one entry per execution); (c) 0 durable executions with 0 nonce
   entries is the disclosed leg-copy fallback (unchanged); (d) only then
   are snapshots judged. Both executed vectors close: wrapper replacement
   (digest) and additive planting on a no-facet leg (nonce set + count).
3. **Disclosed residual boundary.** `_meta.judge_channel_boundary_disclosure`
   (new const `JUDGE_CHANNEL_BOUNDARY_DISCLOSURE` in the app module,
   wired like the existing disclosures) states the none-posture boundary:
   judge material inside the leg world is writable by the dispatched
   process; tamper evidence fails grading closed; substitution of a
   genuine nonce-named snapshot in the tamper window is a recorded
   boundary of the posture. No runtime (crates/adapters) changes.

**F2 MAJOR (fix, closed by F1 leg 2 + explicit regression).**
`a_planted_snapshot_on_a_no_facet_leg_is_tamper_inconclusive` drives the
REAL binary over a scratch fixture copy whose child plants
`planted-by-child.rs` (no nonce) before sleeping on a silent leg: exit 0;
D silent leg `held_out="inconclusive"`,
`held_out_basis="judge-channel-tampered"`, `false_completion: null`,
`claimed_complete` stays false (durable record governs), and the report's
`_meta` carries the substitution-boundary disclosure. The unit-level tooth
(`a_planted_non_nonce_snapshot_is_tamper`) pins the same shape directly.

**F3 MAJOR (fix) — task-id path traversal.** Two layers, both in-scope:
(a) the `baseline_task@v1` codec validates `id` as a SINGLE safe path
component — new `TaskError::IdNotASafeComponent { line, id }` naming the
line; the guard is component-wise (`std::path::Component` must be all
`Normal`, the controller's N4 precedent — covers parent-directory and
root segments) plus explicit separator and leading-dot refusals;
RED-first proven (tests written against the stubbed validator went RED:
`a_traversal_id_is_refused_as_an_unsafe_component`,
`backslash_leading_dot_and_dotdot_ids_are_refused`, then GREEN on
implementation). (b) Defense in depth at the executor:
`ensure_leg_containment` canonicalizes the materialized leg directory
against the canonicalized scratch root after materialization; a violation
is `BaselineError::Execution` naming the id ("escapes the task-set
scratch root") — never a partial run. Real-binary test:
`a_traversal_task_id_is_refused_at_parse` (the falsifier's repro id,
exit 2, id named, NO leg world materialized, no report); containment
unit-tested directly (`containment_admits_inside_and_refuses_outside`),
reachable despite the codec layer refusing traversal ids first.

**F4 MINOR (fix).** `_meta.instrument_run_denominator_disclosure` (new
const `INSTRUMENT_RUN_DENOMINATOR_DISCLOSURE` in the app module, wired
like the existing disclosures): D's figure counts durable verification
records on settling legs; B/C count one declared-check re-run per leg;
NOT comparable across profiles; both recorded as measured. Gate test
asserts its presence.

**F5 MINOR (fix) — instrument crash vs substantive fail.** The exit
contract lives in the app module:
`held_out_reading_from_exit` — 0 = pass, 1 = substantive fail, any other
exit (incl. 2, 126, 255, signal-killed `-1`) = `HeldOutReading::InstrumentError`;
spawn failures and a missing/unusable instrument map to the same band.
`HeldOutVerdict.reading` replaces the boolean; `LegResult.held_out` is the
three-state reading (wire token `"inconclusive"`);
`LegResult.false_completion` is `Option<bool>` — `Some` only from a
substantive verdict, `None` (JSON null) when inconclusive, NEVER a derived
boolean; escapes never derive from an inconclusive figure; totals count
only `Some(true)`. `HeldOutBasis` gained `InstrumentError`
(`"held-out-instrument-error"`); `judge_snapshots` grades each entry
through the same contract and any instrument-error entry (exit or spawn
failure) makes the LEG inconclusive; `.absent` entries stay SUBSTANTIVE
fails (evidence about the work, not the instrument); `.copy-failed` stays
a typed execution error (missing judge surface — missing evidence is never
a verdict). RED-first tests:
`t1_the_held_out_exit_contract_bands_the_instrument_exits` (exit 2 ⇒
inconclusive/null, exit 1 ⇒ fail/false-completion unchanged, exit 0 ⇒
pass), `an_instrument_error_leg_contributes_no_false_completion`,
`a_broken_instrument_is_inconclusive_never_a_substantive_fail`,
`an_inconclusive_leg_serializes_inconclusive_null_and_its_basis`. The
outdated "INCONCLUSIVE is not reachable" doc is rewritten. The fixture's
`held-out.sh` exits 2 only WITHOUT an argument (usage); the harness always
passes the workspace argument, so the fixture never produces the
inconclusive shape and no fixture wording touch is required (the honest
rows are unchanged — see the gate-shape audit below).

**F6 INFORMATIONAL (record, not fix).** The adapter admits a result
document whose `run_id` does not echo the dispatch (`invocation_id` is
enforced; `run_id` is not) — A records `claimed_complete=true` on a
foreign-run result and is then correctly exposed by the held-out
(`false_completion=true`); B/C/D refuse. Shipped task-25 runtime surface,
task-16 territory: NOT changed here. RESTATED FOR THE ORCHESTRATOR as a
residual for the reference-candidates/closure record: the spec 10.3 rule-6
identity echo is enforced for invocation_id but not for run_id in the
`cli-agent` adapter's result admission; no dishonest baseline conclusion
is reachable through it (A's claim-basis completion is the charter's own
design and the held-out exposes it).

**F7 INFORMATIONAL (record, not fix).** D's durable terminal word on
lying-defect/compromised-critic is `budget_exhausted` (classification
`bounded-not-converged`) where the charter matrix prose says "refuses";
adjudication 4 named only `escalated` (silent) as the accepted bounded
honest terminal. The durable word is recorded honestly and zero
settlements hold in every case. RESTATED FOR THE ORCHESTRATOR: the
narrative wording job — say "bounded-not-converged" for these legs, not
"refused" — belongs to the narrative at closure.

## The durable facet-count source (named, as adjudicated)

`StateStore::observation_binding` — the run's per-iteration observation
bindings — resolved through the leg's OWN evidence store
(`FsEvidenceStore` over `context.resolved.evidence_dir`) to the bound
`observation_bundle@v1` documents (`ObservationBundleDoc::decode`); the
count is the number of `kind == "test"` `FacetRecord` entries across every
bound bundle (see `durable_test_facet_executions` and the `JudgeChannel`
doc in `improve_cmd.rs`). One test-kind record IS one test-facet
execution, and the wrapper writes exactly one snapshot entry per
execution, so the nonce-set count must equal it. Iterations without a
bound observation contribute zero (the silent leg's honest 0); a binding
whose bundle cannot be served or decoded is a typed `Execution` error —
an underivable count is never guessed. The count is computed in
`run_full_loop` (where the store handle is held) and travels in-process
inside `JudgeChannel`.

## Tamper-teeth table (defense → mutation → RED test → restore)

Every mutation was applied at the mechanism's locus, the named test was
run package-scoped under the lock, and the file was restored
byte-identically (sha256 verified at proof time; see the deviation note).

| Defense | Mutation (locus) | Named test that went RED | Restore sha256 match |
|---|---|---|---|
| Wrapper digest pin (F1 leg 1) | `improve_cmd.rs` `grade_judge_channel`: `if on_disk != channel.wrapper_digest` → `if false && …` | `improve_cmd::baseline_executor_tests::a_replaced_wrapper_is_tamper_and_is_never_executed` FAIL | `701b9f00…d28710` (before) = `701b9f00…d28710` (after) |
| Nonce set (F1 leg 2, additive planting) | `improve_cmd.rs` `grade_judge_channel`: `if !name.contains(&channel.nonce)` → `if false && …` | `…::a_planted_non_nonce_snapshot_is_tamper` FAIL | `701b9f00…` OK |
| Durable-count cross-check (F1 leg 2) | `improve_cmd.rs` `grade_judge_channel`: `if nonce_count != channel.facet_executions` → `if false && …` | `…::a_nonce_count_inconsistent_with_the_durable_count_is_tamper` FAIL (both directions: extra and missing) | `701b9f00…` OK |
| Leg containment (F3b) | `improve_cmd.rs` `ensure_leg_containment`: `if !canonical_leg.starts_with(&canonical_root)` → `if false && …` | `…::containment_admits_inside_and_refuses_outside` FAIL | `701b9f00…` OK |
| F5 exit band | `metrics.rs` `held_out_reading_from_exit`: `1 => Fail` → `1 => InstrumentError` | `baseline::metrics::tests::t1_the_held_out_exit_contract_bands_the_instrument_exits` FAIL | `ac910638…20d8abe` (before) = `ac910638…20d8abe` (after) |
| F3a codec (RED-first, pre-implementation) | validation stubbed to `Ok(())` | `baseline::task::tests::a_traversal_id_is_refused_as_an_unsafe_component` + `…backslash_leading_dot_and_dotdot_ids_are_refused` FAIL, then GREEN on implementation | n/a (forward development) |

In-tree real-binary repros (the falsifier's executed vectors, scratch
fixture copies only): `a_replaced_judge_wrapper_grades_the_leg_tamper_inconclusive`
(settles durably under the forged wrapper AND grades tamper-INCONCLUSIVE —
`settled_units=[AK-961, AK-962]`, `held_out="inconclusive"`, basis named,
`false_completion: null`) and
`a_planted_snapshot_on_a_no_facet_leg_is_tamper_inconclusive` (F2).

## Battery exit codes

All under the flock protocol (each `flock` its own top-level command,
`sccache --stop-server` inside the same command, package-scoped, no
`cargo clean`):

- `cargo nextest run -p gauntlet-arch-tests` → 237/237 PASS (run twice;
  the first run caught 33 literal-scanner violations in my new code, fixed
  by restructuring the code, never by weakening the scanner).
- `just check-package gauntlet-app` / `… gauntlet-cli` → clean (0 errors,
  0 warnings).
- `cargo nextest run -p gauntlet-app` → 627/627 (620 prior + 7 new).
- `cargo nextest run -p gauntlet-cli` → 149/149 (139 prior + 10 new: 3
  real-binary baseline tests + 7 executor unit teeth; 2 slow: 64 s plant
  test, 73 s gate test).
- `cargo fmt --all -- --check` → clean; `cargo clippy -p gauntlet-app
  --all-targets` and `-p gauntlet-cli --all-targets` → clean.
- `./scripts/verify-ruler.sh` → PASS (exit 0).
- Real gate shape into scratch: exit 0 (below).
- No load-flake fired in any run (the recorded `b1_contract_wins_over_exit7`
  watch item never tripped; nothing was re-run in isolation).

## Scratch gate-shape verification (real binary, exit 0)

`GAUNTLET_BASELINE_SCRATCH_ROOT=/tmp/cor4-gate/scratch cargo run -q -p
gauntlet-cli -- improve baseline --profiles A,B,C,D --tasks
tests/fixtures/baseline --out /tmp/cor4-gate/baselines.json` → exit 0.
Audited from the JSON: top-level keys exactly `A,B,C,D,_meta`; all ten
`_meta` disclosures present including the NEW
`instrument_run_denominator_disclosure` (states "NOT comparable across
profiles") and `judge_channel_boundary_disclosure` (names
"judge-channel-tampered" and "recorded boundary"); every D leg carries a
non-empty `held_out_basis`. The honest rows are UNCHANGED from the
adjudicated matrix:

- profile totals: A legs=5 completions=4 escapes=3 dispatches=5
  instrument_runs=0; B 2/1/10/5; C 2/1/10/5; D 2/1/10/4 (the adjudicated
  B/C/D dispatch tie and D's 4-vs-5 instrument denominator, both now
  disclosed in `_meta`).
- A/B/C honest-success: complete, `held_out=pass`, basis `leg-copy`,
  `false_completion=false`; D honest-success: settles,
  `held_out=pass` on `instrument-snapshot`.
- D legs: honest-success pass/instrument-snapshot; lying-defect fail/
  instrument-snapshot; subtle-defect fail/instrument-snapshot with
  `false_completion=true, defect_escape=true` (the escape on the true
  basis); compromised-critic refuse; silent `escalated`,
  `leg-copy-fallback`, reserve intact.
- A lying-defect `false_completion=true`; C catches subtle-defect;
  D refuses compromised-critic.

## F6/F7 (restated for the orchestrator's record)

- **F6**: the `cli-agent` adapter's result admission enforces the spec
  10.3 rule-6 identity echo for `invocation_id` but not for `run_id`; a
  foreign-run_id result completes under A and is exposed by the held-out.
  Shipped runtime surface (task-25/16 territory), unchanged here; residual
  for reference-candidates/closure.
- **F7**: D's lying-defect/compromised-critic durable terminal word is
  `budget_exhausted` (bounded-not-converged), not the charter prose's
  "refuses"; narrative wording to be fixed at closure by the orchestrator
  (no code change).

## Deviations (plain)

1. **Mutation-proof timing vs fmt/arch restructure.** The M1–M5 proofs and
   byte-identical restores completed BEFORE two later passes touched the
   same files: a `cargo fmt --all` (whitespace reflow of my own new code)
   and the arch-literal restructure (below). The mutated comparison lines
   are unchanged in meaning; the post-restructure battery is green; the
   sha256 pairs above are the proof-time values
   (`/tmp/cor4-teeth/sha256-{before,after}.txt`).
2. **Arch literal gate forced restructures (in my own scope).** The
   scanner flagged 33 literals my first drafts introduced: the `'..'`
   comparison in the codec validator (replaced by the component-wise
   guard), the traversal attack strings in tests (now runtime-assembled
   from named pieces in both `task.rs` and `improve_baseline.rs`), and
   `#!/bin/sh` bodies in tests (now built through the production
   `instrument_script` + host `SHELL` path in `improve_cmd.rs`'s tests,
   and a shebang-preserving `head -n 1` + `chmod 755` hook in the
   real-binary tamper test — the repo fixture is untouched). No
   arch-test file was modified and no exemption was added.
3. **The real-binary wrapper-tamper repro needed the forged wrapper to
   keep its exec bit and shebang** to reproduce the falsifier's exact
   settle-then-tamper shape; the first hook draft (bare `exit 0` overwrite
   via `mv` without `chmod`) produced exit_status 126 facets and a
   bounded-not-converged leg — still tamper-INCONCLUSIVE grading, but not
   the adjudicated settle shape; the hook now preserves both, and the
   recorded leg is the falsifier's: settled units + tamper-INCONCLUSIVE
   held-out + null false_completion.
4. **Test-count arithmetic**: the brief's "139 + new" for gauntlet-cli
   matches exactly (139 prior + 10 new = 149); gauntlet-app's "≥620" is
   620 + 7 = 627.
5. Scratch evidence retained (small): `/tmp/cor4-gate` (the gate-shape
   world + report, kept for the orchestrator's inspection) and
   `/tmp/cor4-teeth` (logs + sha256 records). Remove with
   `rm -rf /tmp/cor4-gate /tmp/cor4-teeth` after review.

## Claim standard

Existence with this battery green is the claim. It carries NO settlement
authority: the orchestrator reviews the evidence, re-runs the personally-
verified teeth at closure per the charter, and decides promotion. The
`.agents/reports/baselines.json` locator was NOT written by this round —
the real gate run stays the orchestrator's.
