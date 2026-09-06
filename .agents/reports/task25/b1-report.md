# B1 report — task 25 (E2E real runner + recovery + budget) — COMPLETE

2026-09-05. Builder B1. This file is the completion contract; it carries
NO settlement authority (the critic and the orchestrator own that).

## Mission echo

Implement the task25 slice exactly per the charter
(`.agents/reports/task25/charter.md`): the fixture, the RED-first
production correction retiring `LiveContainmentDeferred`, three E2E
targets through the REAL binary (`e2e_full_loop_real_runner`,
`e2e_recovery`, `e2e_budget`), the run report `.agents/reports/e2e-real.md`,
progress checkpoints, and the full slice battery green. Done.

## Deliverables (paths + sha256 at completion)

| Deliverable | sha256 |
|---|---|
| `tests/fixtures/target-real/` (13 files, see below) | (per-file below) |
| `tests/fixtures/target-real/GAUNTLET.md` | `bfd89ec4343cf3703ff32d87efa501576fb99a4b19818470e408fdf9f3c98534` |
| `tests/fixtures/target-real/.wayfinder/answer-key.md` | `5129f9cd798c393f95489207cc94dbf7ca70b4e5ed28e64ec92a7df137416ff7` |
| `tests/fixtures/target-real/.wayfinder/reference.yaml` | `fd2c85ffee599897213e8c17bb5a5c1c275e9c8f814445389714c49ae95cf68e` |
| `tests/fixtures/target-real/.gauntlet/reference.lock` | `e5aa10820b3e7cc4e99f400998ee016ef91e3a647bb1b483a128f10b28a731ee` |
| `tests/fixtures/target-real/agent/child.sh` | `f905d7c17a84c016228b756fb2c71c05ee64f45030d1d0cb0d39d1d1e6878631` |
| `crates/gauntlet-cli/tests/e2e_full_loop_real_runner.rs` | `87ac9bb1f9898f93703acaefd1b4f5945f2f03570976e29ee8ab2a18a5b55bba` |
| `crates/gauntlet-cli/tests/e2e_recovery.rs` | `8c532f4f321420e7c782e27ded7c886085f3b90480546b05cbea4c8c4109c0c6` |
| `crates/gauntlet-cli/tests/e2e_budget.rs` | `8d276393277071e7c9216e245174114c1df6057d3f511afe44d43d8b895a33f9` |
| `crates/gauntlet-cli/src/compose.rs` (correction) | `63efd01341fce31b6c33dfeaac00a591bb4aab14d05d09df1d143c918ebc301b` |
| `crates/adapters/gauntlet-adapter-agent-cli/src/runner.rs` (seam) | `b20960b5ec66b85b57712bfb38546fc0b72184c63bb8c9b0ce2c8473d39ee5b0` |
| `crates/adapters/gauntlet-adapter-agent-cli/src/lib.rs` | `86856432d67740995f13072e1b74015211b77cb3e4d9ff158ffadf2be99d22bb` |
| `crates/gauntlet-cli/tests/compose_teeth.rs` (re-pin, disclosed) | `f2865879dc83e4dac2ccde412ef9e8edec1c98506f8b0ab200b806b555dddee5` |
| `.agents/reports/e2e-real.md` (run report) | `ae4512377a779d3edf44d25e1cca2f2544419f5be9aede19738f5a1a413dc7ec` |

Also touched (minimal, disclosed): `crates/gauntlet-cli/src/improve_cmd.rs`
(`runner_for(.., None)` — the improve surface fronts no live posture),
`tests/fixtures/target-real/.gauntlet/policies/{contained,local,cross_cutting}.json`,
`tests/fixtures/target-real/{scripts/check.sh,src/gadget.rs}`,
`tests/fixtures/target-real/.agents/roles/{builder,critic}.md` (copied
from the task-20 fixture), `.agents/reports/task25/b1-progress.md`, this
file. NOTHING committed or staged (orchestrator owns commits).

## Command ledger (every cargo/just invocation, all under
`flock .agents/reports/task25/compile.lock -c '...'` as the top-level
command, `sccache --stop-server` inside the same lock)

1. RED development runs of `--test e2e_full_loop_real_runner` (first:
   2 FAIL at the doctor — fixture bugs, fixed in the fixture; second:
   FAIL at the exact `LiveContainmentDeferred` message — THE RED; post-
   correction iterations to green).
2. `just check-package gauntlet-cli` — multiple times during the
   correction; final PASS.
3. `cargo nextest run --test e2e_full_loop_real_runner` — final PASS 2/2.
4. `cargo nextest run --test compose_teeth --test e2e_full_loop_mock
   --test p5_hard_gate` — after the re-pin: 20/20 PASS.
5. `cargo nextest run --test e2e_recovery` — iterations to green; final
   PASS 3/3.
6. `cargo nextest run --test e2e_budget` — iterations to green; final
   PASS 1/1.
7. `just ruler` — PASS (run twice; never failed).
8. Final battery, in charter order, all PASS:
   `just ruler`; `just check-package gauntlet-cli`;
   nextest `e2e_full_loop_real_runner` (2/2), `e2e_recovery` (3/3),
   `e2e_budget` (1/1), `e2e_full_loop_mock` (14/14), `p5_hard_gate`,
   `p7_recovery`+`p6_budget_control`; `just lint`; `just boundaries`
   (237/237). Combined six-target re-run after the final format: 29/29.

## Design decisions (with reasons)

1. **Wrap seam**: typed `CommandContainment` in the adapter, injected at
   construction — no ports-type change, no signature break; the wrap
   runs after template render and BEFORE any filesystem effect
   (fail-closed leaves nothing); wrapped commands execute as
   `["sh","-c",wrapped]` (the bwrap render emits ONE shell-invocable
   string whose first process is the sandbox — AK-048; dash replaces
   itself for a single simple command, so the spawned pid IS bwrap and
   the transport's TERM/KILL stop hits the sandbox).
2. **Per-dispatch containment facts**: the seam receives the dispatch's
   workspace + admitted policy ref (from the spec's own
   `sandbox_policy` field — the gate's admission guarantee) and
   resolves the policy BY REF over the dispatchable tiers, re-validates
   (validation precedes use at every spawn), wraps via the backend. No
   gate conjunct is re-decided or bypassed; the controller's durable
   admission remains the authority record.
3. **Capabilities**: `Isolation::Process` iff the seam is installed —
   required because the §17.3 table demands `process` for
   cross_cutting (AK-050) and the honest-derivation posture already
   existed (`supports_model_selection`). Without the seam: `none`,
   unchanged.
4. **Exercisable ceiling**: live posture → `Authority::LeadAgent`
   (coupling forbids critic on cross_cutting — a Critic ceiling would
   make the tier undispatchable under the very posture that exists to
   carry it); none posture → Critic, unchanged.
5. **Fixture unit set**: AK-951 contained/critic, AK-952
   cross_cutting/lead_agent (forced by coupling), AK-953
   irreversible/human (p5's AK-9301 spelling). Policy references
   distinct per tier (`tier-*-real-v1`); fixture-local policies under
   `.gauntlet/policies/` (doctor check 5 needs the full set).
6. **Kill boundaries**: observed durable facts only — first settlement
   event (`test_support::settlement_committed_events`), artifact bytes
   in the store with production rows short, then SIGKILL. Assertions
   hold regardless of which side of the commit the kill lands on.
7. **Post-crash resume path**: models the DOCUMENTED recovery — typed
   stale-writer-lock refusal asserted, then the operator's manual lock
   removal, then the resume (COR-R3-3's own shape).
8. **Budget leg calibration**: observed and pinned the §11.5 row-4
   escalated terminal (silent unit → stall-cancel → retry-once →
   second timeout → escalate) with the reserve provably intact from
   the final checkpoint; forcing `budget_exhausted` artificially would
   have lied about the machinery's real order.
9. **Evidence needles**: artifact-unique token `"kind":"child-work-`
   `record"` and the child-computed `confinement=confined` verdict —
   the unit-id needles match the frozen reference projection pinned at
   freeze (an early-true race this session caught and fixed).
10. **Scanner-clean harness**: scripts derived via read_dir
    (`sole_script_of`), durations as named consts, reaping through the
    fixture child's own `reap` mode driven by the DURABLE HANDLE FILES
    (no command literals; the repo's literals/denylist scanners stay
    green).

## Deviations (all disclosed; none silent)

1. **compose_teeth.rs leg 4 re-pinned** — it pinned the task-20
   deferral the charter orders retired. The invariant (never unwrapped)
   is pinned stronger: the routed dispatch carries the admitted policy
   on the durable record. This is a settled-surface edit; the
   charter's own retirement instruction forces it.
2. **Plan step 4 "budget exhaustion"** → pinned the observed §11.5
   row-4 escalated terminal (reserve intact) instead of the literal
   token; recorded in e2e-real.md.
3. **AK-066 "every blocking Unit settles"** → the frozen §5.3.2
   coupling + §16.1 condition 3 make a cross_cutting unit un-settleable
   unattended; the E2E pins the honest routed-to-operator behavior.
   Ruler-owner reconciliation needed (recorded in e2e-real.md).
4. **`improve_cmd.rs`** gained a `None` seam argument (compile
   requirement of the seam; semantics: the improve surface fronts no
   live posture).

## Open findings for the critic/orchestrator

1. **R16-F2 leak**: the composed transient disposition (§11.5 row 2)
   never cancels the runner invocation — hostile (or any) children
   OUTLIVE the run; p8's explicit-cancel retirement is unenforced on
   this path. Proven operationally: leaked chains inherited the
   compile.lock fd and wedged the next flock (task20/23 lesson shape).
   The leg reaps via durable handles; the production fix is the
   orchestrator's call.
2. **`RecordingAgentRunner::admitted` hardcodes
   `tier: BlastRadius::Contained`** in the durable role_invocation
   record (`crates/gauntlet-app/src/compare/recording.rs`) — latent
   until this task; now misstates the tier for cross_cutting
   dispatches (policy ref + gate admission evidence are accurate).
   Needs a tier channel; not fixed here (scope).
3. **AK-066 wording** vs the authority ladder (see deviation 3) and
   the already-recorded locator divergence — reference-candidates
   material at settlement.
4. **UNObservability note**: the run report's category basis text says
   "never attempted on record" for units that WERE dispatched but
   whose latest checkpoint names them open (observed for AK-952) — a
   minor report-wording inaccuracy worth a look, not fixed here.

## Probe hygiene proof (executed at completion)

- `ls -la .gauntlet/policies/`: exactly `.gitkeep contained.json
  cross_cutting.json local.json`, mtimes 2026-08-28 (untouched; the
  legs' policy mutations live ONLY in per-test scratch worlds).
- `find tests/fixtures/target-real -type f`: exactly the 13 intended
  fixture files, no planted debris.
- `git status tests/fixtures/ .wayfinder/ .gauntlet/ GAUNTLET.md`:
  only the new untracked `tests/fixtures/target-real/`; every
  never-touch path unmodified.
- `ps` + `/tmp` sweep: no stray child processes, no leftover scratch
  worlds (the reapers + World drops cleaned everything).
- Nothing staged; nothing committed.

## Resume state

None — the slice is complete. If anything fails at review, the
checkpoints in `b1-progress.md` reconstruct the session.
