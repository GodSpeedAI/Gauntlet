# Task 14 — closure record

**Task 14 — Authority gate, sandbox port, tier policies** *(phase 4 · proves P5)*.
Units: AK-044, AK-045, AK-046, AK-047, AK-048, AK-049, AK-050.

**Status: SETTLED (CONFIRM), with bounded claims.** Six blind confirmation tracks
(one spec critic, five executing falsifier rounds) and four correction rounds
(COR-1..COR-10). The fifth blind round found no BLOCKING and no MAJOR against
AK-044..AK-050 or P5 and returned CONFIRM, carrying ten MINOR/INFORMATIONAL
residuals. Every residual below has an explicit trigger.

Provisional, not settled: the ruler's owner has not settled any of the
locator-materialization candidates this task touches. Unit status here is
"supported by teeth-verified evidence", not "settled by the ruler's owner".

---

## 1. What Task 14 delivered, per AK unit

Every locator below was verified to exist by reading the source at closure. Where
the ruler's frozen locator does not materialize, the real one is named and the
divergence is stated.

### AK-044 — an irreversible unit is never an automatic dispatch candidate

Six independent, non-overlapping refusals (round 5 enumerated them and exercised
three):

1. `crates/gauntlet-app/src/control/scheduler.rs:522` — §11.2 eligibility conjunct 3.
2. `crates/gauntlet-app/src/control/scheduler.rs:632` — slice-member guard.
3. `crates/gauntlet-app/src/govern/gate.rs:538` — dispatch-time conjunct 1.
4. `adapter-policy-fs` `policy_for` — structural `NoPolicyForTier` before any file is read.
5. Both sandbox adapters' `policy_findings` — a finding at the backend, re-derived
   inside `wrap`, so a hand-forged clean validation cannot bootstrap it.
6. `doctor` check 5 — the file's PRESENCE fails the check.

Plus a further structural layer: the tier has no per-tier concurrency entry.

- **Frozen locator:** `tests/proofs/p5_hard_gate::irreversible_never_dispatched`.
- **Materializes as:** `crates/gauntlet-cli/tests/p5_hard_gate.rs:815`
  `fn irreversible_never_dispatched()` (cargo test target `p5_hard_gate`) — VERIFIED.
- **Teeth:** round-5 T2 neutered gate conjunct 1 (`gate.rs:538`, sha256
  `9c986d1f…`) → exactly that named test RED, the other three legs green; restored
  byte-identically → 4/4 PASS. Gate-level unit teeth:
  `govern::gate::tests::irreversible_is_refused_before_every_other_conjunct`
  (`gate.rs:904`).
- **Note:** as first landed, the frozen locator had teeth only on the SCHEDULER
  conjunct — found independently by round 1 (F7) and round 2 (F3), fixed by COR-6
  (coverage ADDED, scheduler leg kept byte for byte, refusal VARIANT asserted).

### AK-045 — one hard-requirement violation rejects, never averages

`AuthorityGate::admit` (`gate.rs:530`) is strictly conjunctive: six conjuncts,
each short-circuiting to a typed `GateRefusal`. There is no scoring or averaging
path in the module. The composed proof maximizes every other merit dimension and
fails exactly one hard conjunct; the control leg dispatches the SAME runner and
unit at `contained`, isolating the capability conjunct alone.

- **Frozen locator:** `tests/proofs/p5_hard_gate`.
- **Materializes as:** `crates/gauntlet-cli/tests/p5_hard_gate.rs:897`
  `fn a_high_quality_dispatch_violating_one_hard_requirement_is_refused_not_averaged()`
  — VERIFIED.
- **Teeth:** the plan's own Done-when condition (see §2).

### AK-046 — an agent role cannot lower a Unit's blast radius or authority

`govern::posture::review_lattice_movement` (`posture.rs:124`) refuses
agent-originated movement in EVERY direction and refuses operator decreases; gate
conjunct 3 (`gate.rs:554`) compares dispatch facts against the answer-key facts
as last parsed and reviews the movement as agent-originated — through the runtime
a DISPATCH can never be the operator's editing surface.

- **Frozen locator:** `tests/antigaming/g7_tier_downgrade`.
- **Materializes as:** `crates/gauntlet-cli/tests/g7_tier_downgrade.rs` (cargo test
  target `g7_tier_downgrade`), three legs — `an_agent_role_cannot_move_a_units_lattice_in_any_direction`
  (:561), `a_dispatch_fact_below_the_recorded_facts_is_refused_as_a_downgrade` (:651),
  `a_mid_run_tier_downgrade_attempt_never_dispatches_at_the_lowered_tier` (:712) —
  VERIFIED.
- **Refusals are governance events:** pinned `gate_refusal` evidence + escalation
  ticket on first refusal per unit + terminal record.
- **Teeth:** round-3 M8 mutated `review_lattice_movement`'s agent arm — RED.

### AK-047 — validation precedes execution; a failure escalates, never falls back

Gate conjunct 6 resolves the tier policy and demands a CLEAN validation, mapping
`ValidationFailed` / `BackendUnavailable` / dirty validation to escalations. No
`GateAdmission` arm can carry a failed validation. Both adapters refuse a
findings-carrying validation, a foreign validation, and a rung-downgraded
validation at `wrap`, and re-derive policy eligibility at `wrap`.
`wrap_builder_command` returns `Result<BuilderCommand, WrapRefusal>` with three
outcomes and no fourth — deliberately no `Option`, so a caller cannot write the
`else { run(raw) }` branch (COR-3).

- **Frozen locator:** `tests/contract/sandbox::no_unsandboxed_fallback`.
- **Materializes as:** `crates/gauntlet-ports/src/contract/sandbox.rs:449`
  `fn no_unsandboxed_fallback(...)`, case const `"no_unsandboxed_fallback"` —
  VERIFIED. Composed leg: `p5_hard_gate.rs:981`
  `fn a_sandbox_validation_failure_escalates_without_falling_back()` — VERIFIED.
- **Teeth:** the shared suite's broken stub `SandboxBrokenFallback` fails on that
  named case (`suite_runs.rs::sandbox_fallback_broken_is_caught`); round-3 M3
  re-verified the case has teeth on the mechanism it names.

### AK-048 — containment wraps the builder session from process start

`GateAdmission` carries the resolved policy and its clean validation;
`wrap_builder_command` renders a command whose first process is the backend
binary. The controller records the durable `gate_admission` evidence BEFORE the
invocation exists (`controller.rs:1651` `record_gate_admission`, ahead of spec
construction and `controller.rs:1749` `runner.invoke`), so the evidence ORDER is
itself the check-before-dispatch proof.

- **Frozen locator:** `tests/contract/sandbox::wraps_from_process_start`.
- **Materializes as:** `crates/gauntlet-ports/src/contract/sandbox.rs:536`
  `fn wraps_from_process_start(...)` — VERIFIED. Composed + LIVE leg:
  `p5_hard_gate.rs:1074` `fn containment_wraps_from_process_start_and_confines_the_session()`
  — VERIFIED (executes real bwrap: in-root write succeeds, out-of-root write
  denied with no artifact, denied path masked, host originals untouched, backend
  binary is the first word).
- **BOUNDED CLAIM — read this before citing AK-048.** `wrap_builder_command` has
  **no production caller**. `Controller::dispatch` runs the gate, records the
  admission durably, builds the `RoleInvocationSpec` with the admitted
  `sandbox_policy` ref, and calls `runner.invoke` — it never wraps. Likewise no
  production code calls `GovernDeps::with_backend`. AK-048 is delivered **at gate
  and adapter level, with LIVE bwrap confinement executed**, and NOT through the
  composed loop. This is consistent rather than broken (§17.5.4 requires the wrap
  only above `contained`, and an above-contained dispatch is admitted only when
  the runner itself reports `isolation ≥ process`), but it must never be stated
  as composed enforcement. Five independent blind rounds recorded this
  (round1-F6, round2-F6, round3-F5, round4-F6, round5-F8).

### AK-049 — an irreversible-tier policy file causes doctor to fail

Delivered since Task 5; re-verified this task. Check 5 scans BOTH the effective
and the default policy dir, is spelling- and symlink-robust, and its "no top-tier
policy" clause is UNCONDITIONAL — unlike the "tier policies exist" clause it is
never skipped for the no-backend posture. `DoctorReport::exit_ok()` is
`required_failure().is_none()`, so a check-5 failure is a non-zero exit.
`.gauntlet/policies/` ships `contained.json`, `local.json`, `cross_cutting.json`
and no `irreversible*` entry — absence by design.

- **Frozen locator:** `tests/e2e/doctor::irreversible_policy_is_error`.
- **DOES NOT MATERIALIZE.** Verified at closure: no file named `doctor` under any
  `tests/` directory and no function named `irreversible_policy_is_error` anywhere
  in `crates/**` (grep returns only the answer key and the spec).
- **Materializes as** three in-crate tests in `crates/gauntlet-app/src/doctor/mod.rs`:
  `a_present_top_tier_policy_fails_check_five_and_the_run` (:518),
  `forbidden_tier_policies_under_any_spelling_or_link_fail_check_five` (:688),
  `a_forbidden_policy_in_the_default_dir_fails_even_when_policy_dir_is_overridden`
  (:757) — all VERIFIED.
- **Reference-candidates status — checked, not assumed:** the existing
  2026-08-17 entry (`.agents/reports/reference-candidates.md:9`) already names
  those exact three tests as the proposed locator re-declaration, and already
  records the cargo-target-path arch rule as the reason the frozen spelling cannot
  be materialized. **The existing entry covers this task's sharpening; no new
  entry is owed.** What Task 14 adds is corroboration, not a new class: three
  independent blind rounds (3-F4, 4-F3, 5-F5) rediscovered it from the tree alone,
  and round 5 sharpened the observation that six of seven Task-14 locators satisfy
  the repo's `tests/proofs/<stem>::<fn>` / `tests/contract/sandbox::<case>`
  convention on both halves while AK-049 satisfies neither. That sharpening is
  recorded here rather than as a duplicate entry.
- **Teeth:** round-5 T4 mutated `forbidden_hit = true` → `false` in
  `doctor/checks.rs` (sha256 `78725fbc…`) → 3 RED; restored → 32/32 PASS.
  Live end-to-end: round 5 ran the real `gauntlet-cli doctor` against a scratch
  project root — clean → pass; `irreversible.json`, `IRREVERSIBLE.json`,
  `irreversible.yaml`, extensionless `irreversible`, `Irreversible.toml`, and a
  broken symlink each → fail; a forbidden file in the DEFAULT dir fails even with
  `sandbox.policy_dir` pointed elsewhere.

### AK-050 — a runner reporting isolation none is refused above contained

`govern::posture::required_isolation` (`posture.rs:31`) is the ONE §17.3 rung table
(contained → None; local / cross_cutting → Process; monotone with `blast_rank`);
gate conjunct 4 (`gate.rs:573`) consumes it against the runner's honest §22.2
reported isolation. After COR-10, `doctor::checks` reads the same table through
`posture::dispatchable_posture` rather than restating it.

- **Frozen locator:** `tests/contract/sandbox::capability_mismatch`.
- **Materializes as:** `crates/gauntlet-ports/src/contract/sandbox.rs:649`
  `fn capability_mismatch(...)` — VERIFIED. Unit leg:
  `govern::gate::tests::a_none_isolation_runner_is_refused_for_any_tier_above_contained`
  (`gate.rs:955`) — VERIFIED. Composed leg: the AK-045 proof above.
- **Teeth:** round-5 T3 (and the orchestrator's own re-run) neutered conjunct 4 →
  2 RED; `the_conservative_posture_admits_contained_critic_and_refuses_every_above_contained_tier`
  (`gate.rs:864`) stayed green because conjunct 5 catches that case independently —
  the layering working, not a missing assertion.

### Supporting surface (not a unit, but load-bearing)

| File | What it carries |
|---|---|
| `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs` | The real backend. Live binary probe precedes every policy judgment; `Isolation::Process` reported honestly; allowlist network is a FINDING, never a downgrade; `BASELINE_ARGS` (15 args) unshares user/pid/ipc/uts/cgroup/net plus `--new-session`, `--die-with-parent`, minimal `/dev`, new `/proc`; writable binds emitted BEFORE masks; every rendered word quoted; a relocatable nested denial is refused rather than served. `live_confinement_holds_the_host_process_space` (:1266) and `the_shared_contract_suite_is_clean` (:2001). |
| `crates/adapters/gauntlet-adapter-sandbox-none/src/lib.rs` | The §8.6 default no-backend `Sandbox`: contained-tier only, allowlist-is-a-finding, foreign-backend cross-wiring refused, honest `Isolation::None`. |
| `crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs` | `<policy_dir>/<tier>.json` strict minimal-schema decode; `$WORKSPACE_ROOT`/`$PROJECT_ROOT`/`$HOME` anchoring with §17.5.2 lexical normalization → component-wise containment → canonicalized symlink re-check; irreversible structurally refused before any read; `permissions_for` honestly `Unavailable` (no artifact invented). |
| `.gauntlet/policies/{contained,local,cross_cutting}.json` | Versioned in-repo tier policies. No `irreversible.json` — by design. |
| `crates/gauntlet-ports/src/contract/{sandbox,stubs,suite_runs}.rs` | +4 shared suite cases (`no_unsandboxed_fallback`, `wraps_from_process_start`, `capability_mismatch`, and COR-9's `the_process_space_is_confined` at :1156) with green + broken-stub teeth. |
| `crates/gauntlet-app/src/govern/{mod,posture,gate}.rs` | The governance module (I9: authority = whether, containment = how contained; spec defect D2 corrected). |
| `crates/gauntlet-app/src/control/controller.rs` | Gate before reservation/spec/invoke; `record_gate_admission` before the invocation exists; `record_gate_refusal` every time with a first-refusal ticket; `DispatchAdmission::GovernanceRefused`; `classify_no_admission` extended so an all-gate-refused run ends `blocked`, never mislabeled `budget_exhausted`; `spec.sandbox_policy` = the admitted ref (§5.3.5's backend half now real); exercisable authority now ONE source (the L1 deferral closed). |
| `crates/gauntlet-app/src/doctor/checks.rs` + `mod.rs` | Check 4 real: composition-root MEASURED `BackendDescriptor`, tier-rung table read from `posture`, `sandbox_capability_mismatch` on no-descriptor / mistokened / unmeasured / rung-too-low. |
| `crates/gauntlet-cli/src/main.rs` | The FIRST production adapter wiring in the composition root: locate the process-isolation backend, accept it only on identity match with the configured token, VALIDATE a live policy — the rung is measured, never declared. |

---

## 2. Gates — and who ran them

**Run personally by the orchestrator on the final tree, 2026-08-29.** These are
the authoritative numbers; the finisher did not compile anything.

| Gate | Result |
|---|---|
| `just test` (workspace) | **1077 run, 1077 passed, 0 skipped** |
| `just lint` | PASS (clippy `-D warnings` + fmt) |
| `just fmt-check` | PASS |
| `just boundaries` | **215/215** |
| `just deny` | advisories ok, bans ok, licenses ok, sources ok |
| `just doctor` | exit OK (check 5 pass) |
| `just ruler` | PASS — digests match the frozen lock |
| `just prereg-check 14` | PASS — matches frozen hash `0ba5ce79…` |

**TEETH — the plan's own Done-when condition, verified personally by the
orchestrator:** disabling the capability conjunct (`govern/gate.rs:573`,
`if false && facts.runner_isolation < required`) turns
`p5_hard_gate::a_high_quality_dispatch_violating_one_hard_requirement_is_refused_not_averaged`
RED; restored byte-identically (sha256 `9c986d1f…` before and after); 7/7 GREEN.

Independently run by the confirmation tracks (not orchestrator numbers): critic A
ran ten gates and four teeth legs; round 5 ran 15 attacks, four teeth mutations,
and a final green re-verification (adapters 50/50, `p5_hard_gate` + `g7` 7/7,
`govern::` + `doctor::` 44/44). Critic A explicitly did NOT run the whole
workspace battery (memory-constrained host) and disclosed that as a scope bound.

**Tree restoration was verified by the orchestrator, not on any falsifier's
say-so:** an independent sha256 baseline of all 181 `crates/**/*.rs` files plus
the three policy artifacts and root `Cargo.toml` was cut before each round and
re-cut after each correction round.

---

## 3. Confirmation tracks and correction rounds

### Six confirmation tracks

| # | Track | Verdict | What it found |
|---|---|---|---|
| 1 | Critic A — spec track (`final-confirmation-a-spec.md`) | **CONFIRM** | Ten gates re-run, four teeth legs mutation-verified, claim-by-claim AK-044..AK-050 with "no residual" on every unit; three informational observations (OBS-1/2/3). Adjudicated the p4/p6 fixture moves as justified. |
| 2 | Blind executing falsifier, round 1 (`final-confirmation-b-falsifier.md`) | **NOT CONFIRM** | 2 executed MAJORs. F1: policy strings become bwrap flags (`--share-net` defeats the declared deny-by-default posture while `validate()` reports CLEAN, proven end-to-end). F2: `$WORKSPACE_ROOT/../outside` escapes its anchor, falsifying the module's own documented invariant. Plus F3-F11. |
| 3 | Blind executing falsifier, round 2 (`final-confirmation-round2-falsifier.md`) | **NOT CONFIRM** | Re-attacked COR-1/COR-2 with 10 quoting payloads and 8 anchor-escape shapes and **broke neither** — then found a MAJOR round 1 had missed: a `denied_paths` entry NESTED inside a `writable_roots` entry was silently re-exposed read AND write, executed end-to-end with the host file overwritten (`PWNED`). Masks were emitted BEFORE the writable binds; bubblewrap applies mounts in argument order. |
| 4 | Blind executing falsifier, round 3 (`final-confirmation-round3-falsifier.md`) | **NOT CONFIRM** | F1 BLOCKING: a committed stray probe file `.gauntlet/policies/irreversible.json` made `just doctor` exit 1 on `main`. F2 MAJOR: the COR-4 ordering fix was defeated by a RENAME — `mv W/a W/b` using only policy-permitted writes, after which the next wrap under the IDENTICAL policy validated CLEAN and the payload was read and overwritten at its new location. |
| 5 | Blind executing falsifier, round 4 (`final-confirmation-round4-falsifier.md`) | **NOT CONFIRM** | F2 MAJOR: the baseline unshared only the network — a contained process signalled, `SIGTERM`-killed and `SIGSTOP`-suspended HOST processes and read host `/proc/<pid>/cmdline`. No test in `crates/` asserted process-space confinement. F1 re-raised the committed probe file as BLOCKING. F4: the §17.3 rung table was duplicated with no cross-check. |
| 6 | Blind executing falsifier, round 5 (`final-confirmation-round5-falsifier.md`) | **CONFIRM** | 15 attacks executed, four teeth mutations. No BLOCKING, no MAJOR against AK-044..AK-050 or P5. Ten MINOR/INFORMATIONAL residuals (F1-F10). |

### Four correction rounds

| Round | Corrections | Substance |
|---|---|---|
| 1 (`correction-round1.md`) | COR-1, COR-2, COR-3 | COR-1: every rendered word quoted at the END of assembly (so a future `push` cannot forget), conservative allowlist, `policy_findings()` deliberately unchanged so spaced paths stay legal — explicitly NOT fixed by rejecting spaces in paths. COR-2: §17.5.2 implemented in order — lexical normalization → component-wise containment (`/ws-evil` is not inside `/ws`) → canonicalized re-check for symlink escape, canonicalizing the deepest EXISTING ancestor. COR-3: the malformed admission made UNREPRESENTABLE (private `Containment` enum, private fields), return type `Result<BuilderCommand, WrapRefusal>` — Task 13's `AdmittedSettlement` earned-capability precedent. |
| 2 (`correction-round2.md`) | COR-4, COR-5, COR-6 | COR-4: writable binds emitted BEFORE masks so masks outrank the binds enclosing them; both inverted doc comments corrected; live bwrap legs prove the nested denial is neither readable nor writable and the host original survives. COR-5: the unanchored-absolute passthrough removed entirely, plus a second line of defense in bwrap `policy_findings` (`names_filesystem_root`, deliberately lexical so a nonexistent path cannot dodge the bound); shipped artifacts VERIFIED to still resolve, not assumed. COR-6: AK-044's frozen locator given teeth on the gate's own conjunct — coverage ADDED, scheduler leg byte-identical, with a CONTROL demonstration proving the added coverage did not displace the old. |
| 3 (`correction-round3.md`) | COR-7, COR-8 | COR-7: a mask is trustworthy exactly when it is a MOUNT POINT directly inside a bound root; any other nesting is refused as a validation finding rather than served and silently degraded. COR-8: `validate` became a real dry run OF THE POLICY (its own binds and masks), so a policy that would abort bubblewrap at process start escalates as `ValidationEscalated` instead of surfacing later as a spawn error. Flagged independently by two blind rounds (1-F3 and 3-F3). |
| 4 (`correction-round4.md`) | COR-9, COR-10 | COR-9: process/IPC/uts/cgroup/user namespace confinement plus `--new-session`, every flag verified by EXECUTION on this host before being written, with the regression home landed as the shared-suite case `the_process_space_is_confined`. COR-10: the §17.3 rung table made impossible to duplicate — `doctor::checks` now reads `posture::dispatchable_posture` — so one mutation fails a test where before it left all 32 doctor tests green. |

Round 5's own teeth (T1) confirms COR-9 has behavioural, not textual, teeth:
swapping `--unshare-pid` for `--unshare-ipc` produced a real `kill -0` landing on
the test process ("process 894221 — a process OUTSIDE the sandbox, and the very
process that built it").

---

## 4. Residuals carried forward

Every entry names a trigger. A residual with no trigger is a leak.

### Deferred with a HARD trigger — must not be quietly forgotten

| Id | Residual | Trigger |
|---|---|---|
| **R-1 (round5-F3)** | `PolicyProvider::policy_for` **discards its `workspace` argument** (`adapter-policy-fs/src/lib.rs:392` binds it `_workspace`; verified at closure). With the shipped artifacts every builder gets read-write over the **SHARED workspaces root** — every other run's and every other unit's workspace. MINOR against this ruler (no AK line names workspace scoping); the falsifier records it as **MAJOR under §17.5.1 invariant 1** ("Builder and critic processes run only inside their assigned workspace path"). Latent only because no composed runtime executes a wrapped builder today. | **Tasks 15/16, BEFORE any real runner executes a builder.** |
| **R-2** | Port-level forgeability of `PolicyValidation` / `SandboxedCommand` / `SandboxPolicy` (public fields, no earned constructor; `BuilderCommand::Unwrapped` a public variant). Fail-closed today: both adapters re-derive full policy eligibility at `wrap`, and nothing consumes a `SandboxedCommand` in production. Raised by rounds 2 (F4), 3 (F8), 4 (F5), 5 (A9). | **The ports revision at Tasks 15/16, before any real runner consumes a wrapped command.** |
| **R-3 (AK-048 composed half)** | `wrap_builder_command` and `GovernDeps::with_backend` have no production caller. AK-048 is a **BOUNDED CLAIM** — gate + adapter level with live confinement executed, never composed enforcement. | **Tasks 15/16 (process runners) and Task 20 (the `run` command).** |

### Spec-conformant but escalated

| Id | Residual | Trigger |
|---|---|---|
| **R-4 (round1-F5 / round5-F7)** | The whole host filesystem is readable inside the sandbox via `--ro-bind / /`, including credential files (`~/.claude/.credentials.json`, `~/.config/gh/hosts.yml`) and `.wayfinder/answer-key.md`. **Literally spec-conformant** — §17.3 places credential denial at `local` and above, and §17.5.5 and §8.5 check 7 bound only WRITABLE paths, both of which hold — so NOT a Task-14 defect, and deliberately NOT patched mid-correction. Carried as a residual **and** as a candidate spec CandidateRevision, with the reasoning stated plainly: **a builder that can READ the frozen ruler can target its assertions without ever modifying it.** | **Escalated to the operator; the spec revision process.** |

### Other residuals from round 5 (the CONFIRM round)

| Id | Residual | Trigger |
|---|---|---|
| round5-F1 | The parent environment is inherited wholesale into the sandbox: no `--clearenv`, and `SandboxPolicy` has **no environment field at all**, so a policy author cannot express an environment restriction. §17.3's credential-path denial is nominal for any credential in an env var; §17.2's `credential_scope` has no consumer at its top enforcement rung (`permissions_for` is an honest `Unavailable`). Undocumented as well as unclosed. | The ports revision at Tasks 15/16, or any sandbox-policy schema touch. |
| round5-F2 | A repo-owned artifact can declare `writable_roots: ["$HOME"]` at the `contained` tier; it resolves, validates clean, wraps, and writes outside the workspace. No `writable_roots ⊆ workspace` rule at either layer. Operator-authoring hazard, not an agent escalation path; the shipped artifacts all name `$WORKSPACE_ROOT`. | The same policy-schema touch as R-1. |
| round5-F4 | `gauntlet doctor` ignores every Annex E environment variable when `GAUNTLET.md` is absent — `doctor/mod.rs::assemble`'s `(None, None)` arm returns `EffectiveConfig::from_defaults()` without calling `resolve()`, so `apply_env_layer` never runs. Fail-CLOSED direction; AK-049's clause is unconditional and unaffected. Ownership ambiguous between the config task and this one. | Next doctor or config-resolution touch. |
| round5-F5 | AK-049's frozen locator does not materialize (see §1). **Covered by the existing 2026-08-17 reference-candidates entry**; no new entry owed. | Settlement by the ruler's owner. |
| round5-F6 | No rlimit or cgroup budget inside the sandbox (cpu/address space unlimited, 31724 processes, 1048576 fds). No §17.3/§17.5 line requires rlimits; §17.4's budget governs the orchestrator, not the contained process. **Not executed** — inferred from `/proc/self/limits`, not demonstrated. | Any §17 containment-hardening revision. |
| round5-F9 (also 2-F5, 4-F8) | The doctor's forbidden-tier scan is stem-exact and non-recursive (`sub/irreversible.json`, `irreversible.json.bak` unflagged). Not a bypass — `PolicyFs` resolves only `<dir>/<tier>.json`. | Next doctor touch. |
| round5-F10 | A `GateAdmission` is not bound to the command it authorizes and is `Clone`, so an in-process caller could wrap unit B's command under unit A's admission. Unreachable today (the only caller is the p5 proof). | The task that composes builder execution (Tasks 15/16). |

### Carried from earlier rounds, not corrected

| Id | Residual | Trigger |
|---|---|---|
| round2-F7 / round4-F7 | The gate never cross-checks `deps.backend_token()` against `sandbox.backend_identity()`; the `none` adapter's token collides with the conservative sentinel so `NoneSandbox` can never be composed into a `GovernDeps`. The dangerous shape (`is_none_posture()` true while a real sandbox is held) IS closed by `NoneBackendWithInstances`. | The ports revision at Tasks 15/16. |
| round1-F8 | An ungated `RoleId::Verifier` runner invocation exists at the composition root (`verify/ladder.rs:490-511`) — outside Task 14's gate. Read, not executed. | Verification-ladder or Task 20 touch. |
| OBS-2 (critic A) | With a live backend, the controller's deterministic clock makes the wall-clock-reading policy resolution refuse on deadline, so above-contained admission through the LOOP with a live backend is not exercisable yet. Stated in-test. | Tasks 15/16 wire process runners. |
| OBS-1 (critic A) | Doctor check 4's overall status stays `unavailable` because the runner-resolution half lands with Tasks 15/16; the Task-14 tier-rung half is measured and discriminating. | Tasks 15/16. |
| P5-routing | The policy-file schema gap and the missing `permissions_for` / environment representation were recorded in reports and here, but never filed as reference-candidate or fog entries. | The ports revision at Tasks 15/16. |
| Prereg instrument gap | No prediction bounded the number of critique rounds or defects, so the task's most striking observation could not be graded against anything. | The next preregistration should predict a round/defect count, or state explicitly that it declines to. |

### Dispositions recorded as decided (not re-litigated here)

- **UNK-001** (which containment backend `cross_cutting` runs under in production):
  **PARTIALLY RESOLVED, not closed.** Mechanism half established durably with
  evidence; production half open on three residues (no production target is named
  anywhere in the tree; the Annex-F weakening — this composition delivers
  `Isolation::Process` where Annex F's typical `cross_cutting` backend is
  microVM/container grade — is unstated; no composed dispatch path exists until
  Task 20). Evidence anchor:
  `.agents/reports/task14/structural-review-unk001.md`. **Terminal disposition
  belongs to `docs/CONFORMANCE.md` at Task 27.** Explicitly rejected: filing it in
  `reference-candidates.md` (UNK-001 is not a Unit and nothing about it is wrong).
- **`docs/CONFORMANCE.md` is NOT owed by Task 14.** The plan's redesign trigger is
  conditional on "no real sandbox backend exists on this host"; bwrap exists and is
  wired, so the trigger did not fire. Task 27 owns the file. Task 14 *does* make
  `cross_cutting` refuse dispatch under the default posture, but for a **different
  reason** than the trigger contemplated — a fail-closed default, not a missing
  backend. Same observable, different cause.
- **The denylist gained `none` as spec vocabulary** (`SPEC_VOCABULARY_SEGMENTS`):
  reviewed personally by the orchestrator. Justified (§6.1 isolation token / §8.2
  backend enum / §8.6 default — pre-existing literals, not adapter selection),
  precedented (Task 11 did the same for `digest`/`check`), pinned by `assert_eq!`,
  and the distinguishing compound spellings stay denied. **NOT a gate weakened to
  land the task.**

---

## 5. Deviations from the plan's Task-14 steps

Stated plainly, including the ones that reflect badly.

1. **One-task-one-commit was overtaken by events.** The operator committed the
   in-flight Task-14 work mid-task as `967731d` ("first push") and `d05bc2e`. This
   is the same class of deviation Task 13 recorded for `62d1a5f`. The task did not
   land as a single commit.
2. **That commit captured a falsifier's transient probe file
   `.gauntlet/policies/irreversible.json` into history**, which made `just doctor`
   exit 1 on `main` — i.e. the task's own declared gate was RED on the default
   branch. Blind rounds 3 and 4 each independently raised it as **BLOCKING**,
   reading only the tree and `git ls-tree HEAD`. The orchestrator deleted the file
   and staged the deletion. Worth naming: the defect was introduced by the
   confirmation process itself and caught by the confirmation process itself, twice,
   from opposite directions.
3. **Plan step 1 says "`AuthorityGate` in the scheduler"; it landed as a separate
   `govern` module** consulted from `controller::dispatch`. The scheduler keeps its
   own §11.2 eligibility conjunct; the gate is the dispatch-time second line. This
   is the prereg's distinction D1 made concrete — a forged or stale plan input
   bypassing the scheduler is still refused at the gate.
4. **Plan step 4 ("wrap the builder invocation from process start") is delivered as
   a surface, not as a composed behaviour** — see R-3. The plan's own gate command
   and Done-when condition pass, but the composed wrap does not exist.
5. **Fixture-tier moves.** `p4_corrective_loop.rs` and `p6_budget_control.rs` moved
   their proof-local scratch unit from `blast_radius: local` to `contained`, with an
   inline recorded reason. The recon and the preregistration both predicted `p12`
   would need the same move; **it did not, and was not touched** — p12 is a
   verification-ladder proof that never composes the dispatch loop, so its `local`
   fixtures never reach the gate. Critic A independently adjudicated the two moves
   as justified and not a weakening: both proofs' subjects are tier-agnostic, their
   scratch answer keys are synthetic proof-local units, and keeping `local` would
   have made the runs governance-block on the mock runner's honest `isolation: none`
   — the gate working exactly as AK-050 requires — destroying the proofs' subjects
   while proving nothing new.
6. **Slices B1, B2 and B3 were all recovered rather than delivered by their
   builders.** B1's builder died to the usage window after landing code but before
   writing its report; B2 and B3 were implemented by the orchestrator personally
   under the standing heal+continue instruction. Interruption instances 21-25 fall
   inside this task. The role charters in `.agents/agents/` remain non-invocable as
   typed sub-agents; every role ran as a verbatim-charter prompt on general-purpose
   transport.
7. **Critic A did not run the whole workspace battery** (memory-constrained host);
   it ran the task's minimum gate set plus six targeted suites and disclosed that as
   a scope bound rather than an absence claim. The full battery is the
   orchestrator's, run separately (§2).

---

## 6. Bounded claims — what is NOT claimed

1. **AK-048's composed half is not demonstrated.** No production code path wraps a
   builder command. Cite AK-048 as "gate-and-adapter level with live confinement
   executed", never as composed enforcement. (R-3.)
2. **"No issue found" is scoped to each round's declared attack surface.** Round 5
   judged the seven ruler lines AK-044..AK-050 and P5; findings bearing on §17.3 and
   §17.5 sentences that no AK line names were recorded but not allowed to decide the
   verdict. It did NOT model a compromised bubblewrap binary, a kernel bug in user
   namespaces, a malicious host `PATH` ahead of `locate()`, or an attacker with
   write access to the repo's own source.
3. **Every behavioural containment result rests on ONE host:** WSL2, kernel
   6.18.35.2-microsoft, bubblewrap 0.11.1, unprivileged uid 1000, cargo `jobs = 1`.
   Namespace results are host facts, not portable guarantees; the
   `--unshare-cgroup` strictness in particular means the backend probe would fail
   (honestly) on a host without cgroup namespaces, and no such host was tested.
4. **The surface is not exhausted.** Round 5's own conclusion: four blind rounds
   preceded it and each found real defects in code that had passed a full green
   battery; it then found four more breaks in areas none of the prior coverage
   names. "The honest conclusion is that this surface still yields to fresh attack
   angles, not that it is now exhausted."
5. **Round5-F6 (no resource bounds) was not executed** — no fork bomb or memory
   bomb was run against the operator's host. The absence of limits is read off
   `/proc/self/limits` inside a live sandbox; the exploitability claim is an
   inference from that reading.
6. **Unit status is provisional, not settled.** The ruler's owner has settled none
   of the locator-materialization candidates. AK-049's frozen locator still names a
   test that does not exist.
7. **The green standing battery is not independent evidence about containment.**
   Every MAJOR found in rounds 1-4 was in code that had just passed a full green
   workspace battery.

---

## 7. The process lesson

**A spec critic that ran ten gates and mutation-verified four teeth legs returned
CONFIRM on a tree that contained three MAJOR containment defects.** Critic A read
every `validate` / `wrap` / `admit` implementation on the landed surface looking
for a fallback path, found none, wrote "no residual" against all seven units, and
was wrong about containment three times over — nested-mask re-exposure, the rename
bypass, and the process-namespace escape all survived that reading.

**Four successive EXECUTING falsifier rounds each found real defects the previous
rounds missed, in different dimensions:**

| Round | Dimension | Defect |
|---|---|---|
| 1 | Shell-word quoting + path resolution | Policy strings become backend flags; `../` escapes its anchor |
| 2 | Filesystem mount ordering | A nested mask is covered by the later bind — argument order is semantics |
| 3 | Path identity over time | A rename moves content out from under its own mask; the next wrap fails OPEN |
| 4 | Kernel namespaces | The baseline unshared only the network; a contained process could SIGTERM host processes |
| 5 | Environment, workspace scoping, policy widening, doctor's env layer | Four more breaks, none MAJOR against the ruler |

Three things follow, and they are the lesson to carry:

1. **Reading cannot confirm containment.** Containment claims are claims about
   kernel behaviour. An argv is not kernel behaviour, and a reviewer who reads the
   argv is checking a proposition adjacent to the one that matters. This is the
   **second consecutive task** in which an executing falsifier was decisive after a
   static CONFIRM — the Task-13 round-1 lesson, reproduced exactly and in the same
   order. Keep the executing role in every confirmation round.
2. **A green battery is silent about the case the suite never constructed.**
   Round-2's MAJOR was invisible to every existing test for a structural reason:
   every denied path in the suite was a SIBLING of the writable root, and *nested is
   the only case where "denied even if otherwise writable" has content*. When a
   guarantee has one case in which it has content, that is the case the suite must
   construct.
3. **Fixing a defect is not the same as closing its class.** COR-4 fixed the mount
   ORDER and round 3 defeated the fix with a RENAME; only COR-7 — which changed what
   a mask *is* (a mount point directly inside a bound root, or a refusal) — closed
   the class. Each correction that fixes a mechanism without changing the
   representation invites the next round to find the same class wearing different
   clothes.

A fourth, smaller lesson: **the corrections held up under re-attack.** Round 2
threw ten quoting payloads and eight anchor-escape shapes at COR-1 and COR-2 and
broke neither; rounds 3, 4 and 5 re-attacked each prior fix and broke none of
them. The corrections were not fragile — the surface was simply larger than any
one round could see.

---

## Sources

`.agents/reports/task14/`: `recon.md`, `b1-progress.md`, `builder-b1.md`,
`builder-b2.md`, `builder-b3.md`, `final-confirmation-a-spec.md`,
`final-confirmation-b-falsifier.md`, `final-confirmation-round2-falsifier.md`,
`final-confirmation-round3-falsifier.md`, `final-confirmation-round4-falsifier.md`,
`final-confirmation-round5-falsifier.md`, `correction-round1.md`,
`correction-round2.md`, `correction-round3.md`, `correction-round4.md`,
`structural-review-unk001.md`, `IN-FLIGHT.md`.
Also `.agents/reports/reference-candidates.md` and
`.agents/research/preregistrations/task-14.outcome.yaml`.
