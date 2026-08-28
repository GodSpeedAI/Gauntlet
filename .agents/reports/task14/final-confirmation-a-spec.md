# Task 14 final confirmation — critic A (spec track)

Critic: independent blind confirmation round, fresh context.
Envelope: frozen ruler `.wayfinder/answer-key.md` (AK-044–AK-050), `.agents/specs/Core-spec.md`
(§17, §6.1–6.2, §8.5–8.6, §11.2, §5.3.5, §21.3, §22.2, §25), plan Task 14 (lines 471–495 only),
the landed tree (`git diff HEAD`), and gates/teeth run by this critic. No builder reports,
preregistrations, outcomes, status files, or session notes were read.

## What was run (all by this critic, sequentially, one compiler at a time)

| Gate | Result |
|---|---|
| `cargo nextest run -p gauntlet-cli --test p5_hard_gate` | PASS 4/4 |
| `cargo nextest run -p gauntlet-app govern::` | PASS 11/11 |
| `cargo nextest run -p gauntlet-cli --test g7_tier_downgrade` | PASS 3/3 |
| `cargo nextest run -p gauntlet-ports sandbox` (incl. broken-stub teeth) | PASS 4/4 |
| `cargo nextest run -p gauntlet-adapter-sandbox-none` | PASS 12/12 |
| `cargo nextest run -p gauntlet-adapter-sandbox-bwrap` (incl. live confinement) | PASS 12/12 |
| `cargo nextest run -p gauntlet-adapter-policy-fs` (incl. shipped-artifact resolution) | PASS 11/11 |
| `just doctor` | exit OK |
| `just boundaries` | PASS 215/215 |
| `just ruler` | PASS (frozen; untouched; re-verified after teeth mutations) |

Full workspace `just test` was not run (memory-constrained host; the task's minimum gate set
plus six targeted suites were run instead — scoped disclosure, not an absence claim about the
whole workspace).

## Teeth verified by mutation (RED confirmed, then byte-identical restore, then GREEN)

Baselines recorded by sha256 before mutation and re-verified `OK` after restore
(`/tmp/critic-teeth-baselines.txt`); `git status` shows both files byte-identical to the
landed tree.

1. **Capability conjunct** (`crates/gauntlet-app/src/govern/gate.rs:439`,
   `if facts.runner_isolation < required` → `if false && …`):
   `p5_hard_gate::a_high_quality_dispatch_violating_one_hard_requirement_is_refused_not_averaged`
   FAILED (the refusal no longer names the capability fact — the none-isolation dispatch
   proceeds past the conjunct), and
   `govern::gate::tests::a_none_isolation_runner_is_refused_for_any_tier_above_contained`
   FAILED. Restored → both GREEN. This is the plan's own teeth claim: deleting the capability
   check lets a none-isolation runner take an above-contained unit and fails the proof.
2. **Gate irreversible conjunct** (`gate.rs:404` → `if false && …`):
   `govern::gate::tests::irreversible_is_refused_before_every_other_conjunct` FAILED. The
   loop-composed `irreversible_never_dispatched` stayed green — via the scheduler's own
   §11.2 conjunct (the designed second line), not via a missing assertion.
3. **Scheduler irreversible conjunct** (`crates/gauntlet-app/src/control/scheduler.rs:522`
   → `if false && …`): loop-composed `p5_hard_gate::irreversible_never_dispatched` FAILED.
   (A third layer — per-tier capacity, "irreversible has no concurrency entry" — still blocks
   the dispatch, but the proof demands the §17.1 refusal specifically, so it discriminates the
   removed check rather than passing on the residual block.) Restored → GREEN.
4. **Contract stub teeth** (no mutation needed — they are landed tests):
   `sandbox_fallback_broken_is_caught` (case `no_unsandboxed_fallback`) and
   `sandbox_process_start_broken_is_caught` (case `wraps_from_process_start`) each fail their
   one-defect fake on the named case; the green stub runs the full suite clean; both real
   adapters (`none`, `bwrap`) run the shared suite clean.

## Claim-by-claim comparison (AK-044–AK-050)

**AK-044 — irreversible never an automatic dispatch candidate.** Three layered refusals:
scheduler eligibility conjunct 3 (`scheduler.rs:522`), the slice-membership guard
(`scheduler.rs:632`), and the gate's first conjunct (`gate.rs:404`); plus per-tier capacity
admitting no entry for the tier. Loop-composed proof: zero durable `role_invocations`,
`Blocked` terminal, ticket naming §17.1's first line. Teeth verified at both the gate layer
and the scheduler layer (above). LOCATOR materialization (`tests/proofs/p5_hard_gate::
irreversible_never_dispatched` as cargo test target `p5_hard_gate` with the exact test name)
follows the repo's recorded frozen-locator precedent, documented in the proof header. No
residual.

**AK-045 — one hard-requirement violation rejects, never averages.** `AuthorityGate::admit`
is strictly conjunctive — every conjunct short-circuits to a typed `GateRefusal`; no scoring
or averaging path exists in the module (checked by reading the whole of `gate.rs`). The
loop-composed proof's control leg shows the SAME runner and unit dispatching at `contained`
while the `local` variant is refused — isolating the capability conjunct alone. Teeth
verified by mutation (item 1). No residual.

**AK-046 — agent roles cannot lower blast radius/authority; only an operator raises.**
`posture::review_lattice_movement` refuses agent-originated movement in EVERY direction
(the second clause read strictly) and operator decreases; `gate` conjunct 3 compares dispatch
facts against the answer-key facts as last parsed; the g7 proof adds the composed
frozen-ruler defense (mid-run tier downgrade in the answer key → resume re-freeze-verifies
first → `Aborted` with `ReferenceBreach::Mutated`, zero invocations). Refusals are recorded
as governance events (`gate_refusal` pinned evidence + escalation ticket + terminal record).
The `recorded` lookup keys on `planned.units.first()`, but `dispatchable_member`
(`controller.rs:3385`) makes any multi-member plan a typed `SliceNotDispatchable` error, so
the lookup is exact for every dispatchable plan — guarded, not latent. No residual.

**AK-047 — validation failure escalates, never falls back.** Gate conjunct 6 maps
`ValidationFailed`/`BackendUnavailable`/dirty validation to escalations; no admission arm
can carry a failed validation. Both adapters refuse a findings-carrying validation, a
foreign validation, and a rung-downgraded validation at `wrap`, and re-derive policy
eligibility at wrap so a forged clean validation bootstraps nothing (unit-pinned in both
adapters). Contract case `no_unsandboxed_fallback` forges failure-shaped validations and
catches the fake that launders them. §8.6 arm (`NoBackendForTier`) proven at gate level and
loop-composed. No residual.

**AK-048 — containment wraps from process start; checks precede the builder.** The
controller records the durable `gate_admission` evidence BEFORE the invocation exists
(`controller.rs`, `record_gate_admission` called before spec construction and
`runner.invoke`), so the evidence order itself proves check-before-dispatch. The admission
carries the resolved policy and its clean validation; `wrap_builder_command` renders a
command whose first process is the backend; the proof EXECUTES the wrapped command under the
real backend and demonstrates confinement (in-root write succeeds, out-of-root write denied
+ no artifact, denied path masked, host originals untouched) and asserts the backend binary
is the first word. The shipped repo policy artifacts resolve through the real provider at
the composition root for every dispatchable tier (composed reachability, not mechanism
existence). Contract case `wraps_from_process_start` pins the first-process shape for every
adapter. `SandboxedCommand` escaping is adversarially tested (flag payloads, chained
destruction, quote round-trips). No residual.

**AK-049 — irreversible policy file fails doctor.** Still holds: `just doctor` check 5
passes with "no policy file for the top tier in policy dir '.gauntlet/policies' (AK-049)";
`.gauntlet/policies/` contains only `contained.json`, `local.json`, `cross_cutting.json`
(+ `.gitkeep`) — no `irreversible*` entry; `PolicyFs::policy_for` refuses the tier
structurally before any file read, even when one exists (unit-pinned). No residual.

**AK-050 — isolation-none runner refused above contained.** The rung table
(`posture::required_isolation`: contained→None, local/cross_cutting→Process, monotone with
`blast_rank`) feeds gate conjunct 4; loop-composed refusal for the mock runner's honest
§22.2 `isolation: none`; contract case `capability_mismatch` pins that a none-rung backend
never clean-validates an above-contained tier and that EVERY backend refuses an
irreversible-tier policy at `validate`; the `none` adapter is honest (refuses above-contained
tiers, allowlist postures, foreign backends, empty writable roots). Doctor check 4's new
tier-rung half fails honestly on an unfronted, unmeasured, or mistokened backend
(unit-pinned, three legs). Teeth verified by mutation (item 1). No residual.

## Fixture-tier moves in p4/p6 (evaluated as instructed)

`p4_corrective_loop.rs` and `p6_budget_control.rs` moved their proof-local scratch unit from
`blast_radius: local` to `contained`, with an in-place reasoning note; g10/g11/p7 only add
the conservative posture field. Verdict: **justified, not a weakening.** Both proofs'
subjects (defect→residual→routed correction→closure; non-converging exhaustion with the
recovery reserve intact) are tier-agnostic, and their scratch answer keys are synthetic
proof-local units, not frozen ruler units — the ruler's locators for those proofs do not
condition on the fixture tier. Keeping `local` would have made the runs governance-block on
the mock runner's honest `isolation: none` (the gate working exactly as AK-050 requires),
destroying the proofs' subjects while proving nothing new. The above-contained posture is
carried by `p5_hard_gate` against the real backend.

## Structural review notes (all minor/informational — none blocking or major)

- **OBS-1 (minor).** Doctor check 4's overall status remains `unavailable` because the
  runner-resolution half lands with Tasks 15–16 (same disclosed convention as check 3);
  the Task-14 tier-rung half is now measured and discriminating. Locus:
  `crates/gauntlet-app/src/doctor/checks.rs:781–796`. Pre-existing convention, improved by
  this task; no ruler unit requires the runner half yet.
- **OBS-2 (informational).** In the composed loop with a real-backend posture, the
  controller's deterministic clock makes the wall-clock-reading policy resolution refuse on
  deadline, so above-contained admission through the LOOP with a live backend is not
  exercisable yet; the proof states this in-test and proves the validation/escalation legs
  at gate level with live bounds, and the capability leg loop-composed. Honest and
  discriminating (mutation RED confirms); resolves naturally when Tasks 15–16 wire process
  runners.
- **OBS-3 (informational).** Ruler locators spell repo-root targets (`tests/proofs/…`,
  `tests/contract/sandbox::…`) that do not exist as paths; the repo's recorded precedent
  maps them to cargo test-target names / contract-suite case names, cited in each proof
  header. Pre-existing convention, not introduced by this task.

## Bounded-claims discipline

"Issue-free" above is scoped to: the landed diff files listed by `git diff HEAD` (govern
module, controller wiring, doctor, three adapters, three policy artifacts, ports contract
suite + stubs, CLI doctor measurement, p5/g7 proofs, fixture-tier moves), read in full;
every `validate`/`wrap`/`admit` implementation on the landed surface reviewed for a
fallback path (none found within that scope); ten gate/suite runs and four mutation-teeth
legs executed by this critic with byte-identical restoration verified by sha256. Not
claimed: whole-workspace test status, tasks not yet landed (15–16 runner wiring), or hosts
without the bwrap backend (live legs skip honestly there by design).

VERDICT: CONFIRM
