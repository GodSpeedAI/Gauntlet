# Task 14 — fresh blind confirmation, round 5 (executing falsifier)

Charter: `.agents/agents/falsifier.agent.md`, adopted verbatim.

**Blindness held.** Nothing was read under `.agents/reports/**` (including every prior
falsifier report and every builder correction account), `.agents/research/**`,
`.agents/lessons/**`, `.agents/prompts/**`, `.agents/CURRENT_STATUS.yml`; no git commit
message was read. Read: `.wayfinder/answer-key.md` AK-044..AK-050 verbatim,
`.agents/specs/Core-spec.md` §8.5/§8.6/§17.3/§17.4/§17.5/§20.1, `.agents/plans/gauntlet-impl.md`
Task 14 section only, the tree, and executables.

**Claim under test.** The landed Task-14 surface delivers AK-044..AK-050 and proof P5:
authority decides *whether*, containment decides *how contained*, both before builder
process start, no unsandboxed fallback, no `irreversible` policy.

15 attacks executed. Verdict at the end.

---

## 1. Attacks that found nothing (the surface held)

### A3 — other namespaces and kernel surfaces (EXECUTED, held)
Ran the adapter's own `BASELINE_ARGS` shape by hand and interrogated the payload's view:
- `/proc` is the NEW pid namespace: 4 pids, `/proc/1/comm == bwrap`. Host process table
  invisible.
- `/sys` present but read-only (`echo 0 > /sys/kernel/profiling` → EROFS).
- `/dev` is bwrap's minimal set (`core fd full null ptmx pts random shm std* tty urandom
  zero`) — no `kmsg`, no disks, no loop devices. `mknod` → EROFS.
- Inherited file descriptors are exactly 0/1/2. No stray fd crosses the boundary.
- uid is the unprivileged host uid mapped through the user namespace; no capability over
  host objects.
- `--unshare-net` also carries the abstract-unix-socket namespace, so that IPC channel is
  closed with the network. `--unshare-uts` blocks hostname mutation. No time namespace, but
  `settimeofday` needs CAP_SYS_TIME in the *initial* user namespace, which the payload does
  not hold.
No break found.

### A5 — lifetime: daemonize / double-fork out of the sandbox (EXECUTED, held)
Payload: `(setsid sh -c 'while true; do echo tick >> $W/ticks; sleep 0.2; done' &)` then the
payload exits immediately. The daemon wrote 5 ticks while the payload lived and **stopped
the instant the payload exited** (tick count frozen at 5 across the next 4 seconds; nothing
survived on the host). `--unshare-pid` makes the sandbox pid 1 of its namespace, so the
kernel reaps the whole namespace when it goes; `--die-with-parent` closes the parent-death
half. A setsid'd, double-forked grandchild cannot outlive the wrap. The same mechanism is
what makes `BwrapSandbox::run_bounded`'s deadline `child.kill()` a real teardown rather than
an orphan factory.

### A8 — is `irreversible` reachable by ANY route? (EXECUTED, held)
Six independent, non-overlapping refusals, each read in source and three exercised:
1. `control/scheduler.rs:522` — §11.2 eligibility refuses the unit.
2. `control/scheduler.rs:632` — slice-member guard refuses it inside a plan.
3. `govern/gate.rs:538` — dispatch-time conjunct 1 (exercised by T2).
4. `adapter-policy-fs` `policy_for` — structural refusal *before any file is read*, "even
   if one is (mistakenly or maliciously) present".
5. `adapter-sandbox-bwrap` / `adapter-sandbox-none` `policy_findings` — a finding at the
   backend, re-derived inside `wrap` so a hand-forged clean validation cannot bootstrap it.
6. `doctor` check 5 — the file's PRESENCE fails the check (exercised by A6/T4).
No route reached a dispatch.

### A6 — doctor discrimination for AK-049 (EXECUTED, held)
Built a scratch project root and ran the real `gauntlet-cli doctor` binary against it.
Clean tree → check 5 `pass`. Then, one at a time (each removed after):
`irreversible.json` → **fail**; `IRREVERSIBLE.json` → fail; `irreversible.yaml` → fail;
`irreversible` (no extension) → fail; `Irreversible.toml` → fail; a **broken symlink** named
`irreversible.json` → fail. With `sandbox.policy_dir` pointed elsewhere, a forbidden file in
the DEFAULT `.gauntlet/policies` still fails. `DoctorReport::exit_ok()` is
`required_failure().is_none()`, and `required_failure` matches any `CheckStatus::Fail`, so a
check-5 failure is a non-zero exit (§8.5, §20.1). The "no top-tier policy" clause is
UNCONDITIONAL — unlike the "tier policies exist" clause it is never skipped for the
no-backend posture. AK-049 holds.
One bounded gap: the scan is non-recursive, so `.gauntlet/policies/sub/irreversible.json`
is not reported (F9). `PolicyFs` reads only `<dir>/<tier>.json`, so a nested file is not a
resolvable policy — defense in depth, not a bypass.

### A9 — admission forgery / downgrade (source review, held)
`GateAdmission` has private fields, a private `Containment` enum, and no constructor
outside `govern::gate`; the only way to hold one is to have passed all six conjuncts.
`wrap_builder_command` has three outcomes and no fourth — deliberately no `Option`, so a
caller cannot write the `else { run(raw) }` branch. Cross-wiring both ways is a typed
refusal (`PostureLostItsSandbox`, `AdmissionPostureMismatch`). Both adapters' `wrap`
re-derive `policy_findings` from scratch, so a hand-built `PolicyValidation { findings: [],
isolation: … }` (the struct IS publicly constructible) buys nothing: the wrap re-checks the
rung, the policy identity, and every eligibility rule.

---

## 2. Attacks that found something

### F1 — the parent environment is inherited wholesale into the sandbox (EXECUTED, BREAK)
Two independent demonstrations.

*By hand:* `FAKE_SECRET_TOKEN=sk-ATTACK-CANARY-12345 bwrap <BASELINE_ARGS> -- sh -c 'echo
$FAKE_SECRET_TOKEN'` printed the canary. 62 environment variables reach the payload; on
this host two are credential-shaped (`STARSHIP_SESSION_KEY`, `CLAUDE_CODE_MESSAGING_TOKEN`).

*Through the real port chain* (probe 3, since deleted): `PolicyFs` → `BwrapSandbox::validate`
→ `wrap` → run the rendered string with `GAUNTLET_FALSIFIER_CANARY` set; the contained
payload printed `canary-reached-the-sandbox`.

`BASELINE_ARGS` carries no `--clearenv` and no `--setenv`/`--unsetenv` handling, and
`SandboxPolicy` has **no environment field at all**, so a policy author cannot express an
environment restriction even if they wanted to. §17.3 gives `local` and `cross_cutting`
"explicit deny of credential paths"; for any credential that lives in an environment
variable rather than a file, that denial is nominal. §17.2's `PermissionSet.credential_scope`
exists and its enforcement ladder puts "sandbox policy" at the top rung — nothing at that
rung consumes it (`permissions_for` is an honest `Unavailable`).
The adapter's `BASELINE_ARGS` doc comment enumerates "each flag, and why it is not optional"
for six namespaces plus session/dev/proc/lifetime and never mentions the environment;
`docs/CONFORMANCE.md` does not exist. The gap is undocumented as well as unclosed.

### F2 — a repo-owned policy artifact can widen the `contained` tier past its workspace (EXECUTED, BREAK)
Probe 1 (since deleted): a `contained.json` declaring `writable_roots: ["$HOME"]`
- **resolved** through the real `PolicyFs` (the entry is anchored, and `contain_within_anchor`
  is satisfied because the anchor *is* `$HOME`);
- **validated CLEAN** through the real `BwrapSandbox` (`policy_findings` refuses only
  relative roots and the filesystem root — `names_filesystem_root` is false at depth ≥ 1);
- **wrapped**; and the contained payload then wrote and read back a file outside the
  workspace root: stdout `"hi"`, marker present on the host afterwards.

§17.3's `contained` posture is "read-write limited to the workspace". Nothing in the
resolution or the validation path enforces `writable_roots ⊆ workspace`; anchoring bounds an
entry to *some* declared root, never to the *workspace* root.
Bound: this requires an operator to author a wide artifact. A contained builder cannot reach
`.gauntlet/policies/` (its writable root is the workspace, outside the repo by default), so
this is an operator-authoring hazard, not an agent escalation path. The shipped artifacts
all name `$WORKSPACE_ROOT`.

### F3 — `PolicyProvider::policy_for`'s `workspace` argument is discarded (EXECUTED, BREAK)
Probe 2 (since deleted): `policy_for(Contained, "run-a-unit-1")` and
`policy_for(Contained, "run-b-unit-2")` returned the **identical** policy, writable root
`/tmp/…/workspaces` — the SHARED root under which all run workspaces live, not the assigned
one. `PolicyFs::policy_for` binds the parameter as `_workspace`.
Consequence with the **shipped** artifacts (`writable_roots: ["$WORKSPACE_ROOT"]`, default
`workspace.root = <tmp>/gauntlet/workspaces`): every wrapped builder gets read-write over
every other run's and every other unit's workspace, at every tier. §17.5.1 — "Builder and
critic processes run only inside their assigned workspace path" — is not enforced by the
containment mechanism.
Bound: no composed runtime executes a wrapped builder today (see F8), so the exposure is
latent; and §17.5.1's enforcement point may be intended to be the workspace provider rather
than the sandbox. Under §17.5.1's own scope this would be MAJOR; under this round's claim
(AK-044..AK-050 + P5) no line names workspace scoping.

### F4 — `gauntlet doctor` ignores every Annex E environment variable when `GAUNTLET.md` is absent (EXECUTED, BREAK)
In a scratch project with no `GAUNTLET.md`, `GAUNTLET_SANDBOX_BACKEND=bwrap` in the process
environment had **no effect**: check 4 reported "sandbox backend resolves to 'none' (the 8.6
conservative default)" and check 5's "tier policies exist" clause was **skipped**. Adding a
two-line `GAUNTLET.md` to the same tree, with the same environment, flipped both: check 4
measured the real backend ("tier local requires rung process; backend achieves process") and
check 5 verified all three tier artifacts.
Cause, verified in source: `doctor/mod.rs::assemble` has a `(None, None)` arm (no read
failure, no front matter) that returns `EffectiveConfig::from_defaults()` **without calling
`resolve()`**, so `config::resolve::apply_env_layer` — the only consumer of
`STATIC_ENV_KEYS`, which includes `GAUNTLET_SANDBOX_BACKEND` and
`GAUNTLET_SANDBOX_POLICY_DIR` — never runs. Reproduced identically via a `.env` file.
Bound: the direction of the error is fail-CLOSED for containment (doctor under-reports the
posture; the gate under the same resolution would refuse above-contained dispatches too), and
AK-049's clause is unconditional so it is unaffected. Ownership is ambiguous between the
config task and this one; `doctor/mod.rs` is in this task's diff.

### F5 — AK-049's frozen locator names a test that does not exist (source review)
The ruler requires `{kind: test_report, locator: tests/e2e/doctor::irreversible_policy_is_error}`.
No file named `doctor` exists under any `tests/` directory in the workspace and no function
named `irreversible_policy_is_error` exists anywhere (`grep -rn` over the tree returns only
the answer key and the spec). The mechanism is implemented and has teeth (T4), but it is
carried by `crates/gauntlet-app/src/doctor/mod.rs`'s
`forbidden_tier_policies_under_any_spelling_or_link_fail_check_five`,
`a_forbidden_policy_in_the_default_dir_fails_even_when_policy_dir_is_overridden`, and
`a_present_top_tier_policy_fails_check_five_and_the_run`.
Bound: none of the seven locators is a real filesystem path in this repo — the convention is
`tests/proofs/<stem>::<fn>` → `crates/gauntlet-cli/tests/<stem>.rs::<fn>` and
`tests/contract/sandbox::<case>` → the suite case names in
`crates/gauntlet-ports/src/contract/sandbox.rs`. Six of seven satisfy that convention on both
halves. AK-049 is the one that satisfies neither half. This is a traceability defect, not a
mechanism defect.

### F6 — no resource bound of any kind (EXECUTED, observation)
`/proc/self/limits` inside the sandbox is the host's: cpu unlimited, address space unlimited,
`Max processes 31724`, `Max open files 1048576`. `--unshare-cgroup` unshares the cgroup
*namespace view*, not any budget. A contained payload can exhaust host CPU, RAM and pids, and
fill the disk inside its writable root. Not attempted (a fork bomb against the operator's host
is out of bounds). No spec line in §17.3/§17.5 requires rlimits; §17.4's budget governs the
orchestrator, not the contained process.

### F7 — the whole host filesystem is readable from inside (EXECUTED, observation)
`--ro-bind / /` is the baseline, so containment is "write only where declared", never "read
only where declared". Under the shipped `local`/`cross_cutting` shape the payload could read
`$HOME/.config/gh/hosts.yml`, `$HOME/.claude/.credentials.json`, `$HOME/.claude.json`, and
`$PROJECT_ROOT/.wayfinder/answer-key.md`. `/etc/shadow` was refused by file mode, not by the
sandbox. The shipped denied set is `$PROJECT_ROOT/.git`, `$HOME/.ssh`, `$HOME/.aws`,
`$HOME/.gnupg`.
Bound, and this is why it is INFORMATIONAL rather than a break: both spec lines that govern
the answer key bound only WRITABILITY — §17.5.5 ("outside every builder-writable path") and
§8.5 check 7 ("answer key path **not** inside any builder-writable workspace"). Both hold. A
*read*-blindness requirement for the builder is not stated in any section this round is
allowed to read, so calling this a defect would be moving the ruler. Recorded so a later
round with a wider spec grant can adjudicate it.

### F8 — `wrap_builder_command` is not on the composed dispatch path (source review)
`grep` over `crates/**` finds `wrap_builder_command` and `BuilderCommand::` used only in
`crates/gauntlet-cli/tests/p5_hard_gate.rs`. `Controller::dispatch` runs the gate, records
the admission durably, then builds the `RoleInvocationSpec` and calls `runner.invoke` — it
never wraps. Likewise no production code calls `GovernDeps::with_backend`; every composed
posture is `none_backend`.
This is consistent rather than broken: §17.5.4 requires the wrap only *above* `contained`,
and an above-contained dispatch is admitted only when the runner itself reports
`isolation ≥ process` (§22.2's honesty rule), i.e. when the runner is already contained. It
does mean AK-048's end-to-end guarantee currently rests on the port + the composed method +
the p5 live leg, and the runner-side half lands with Tasks 15-16. Recorded as a bound on the
confirmation, not as a defect.

### F9 — the doctor's forbidden-tier scan is non-recursive (EXECUTED, observation)
`.gauntlet/policies/sub/irreversible.json` produced zero forbidden-hit lines. `PolicyFs`
resolves only `<dir>/<tier>.json`, so a nested file is never a policy. Cosmetic.

### F10 — an admission is not bound to the command it authorizes (source review)
`GateAdmission` carries `unit`, `tier`, and the containment pair; `wrap_builder_command`
never relates its `command` argument to `admission.unit`. `GateAdmission` is `Clone`, so an
in-process caller could wrap unit B's command under unit A's admission. Not reachable today
(the only caller is the p5 proof, and the controller derives both from one dispatch), and the
tier-policy is per-tier, not per-unit, so the containment delivered would be correct for the
tier either way. Recorded for the task that composes builder execution.

---

## 3. Teeth (sha256 → minimal inverting mutation → named test RED → byte-identical restore → GREEN)

Every mutation was a single line. Every file was restored and re-verified by `sha256sum -c`
before the next mutation. No mutation was left in the tree; nothing I broke was repaired.

### T1 — the process namespace (AK-050's rung half, §17.3 "isolation ≥ process")
File `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs`,
sha256 `3f510d95c6f08838ca015304a73a090785613db6b062df59b6b0eec1a40f6ad0`.
Mutation: `"--unshare-pid",` → `"--unshare-ipc",` (array length preserved; nothing else moved).
`cargo nextest run -p gauntlet-adapter-sandbox-bwrap --no-fail-fast` → **3 RED**:
- `live_confinement_holds_the_host_process_space` — "a contained process must not signal a
  process outside the sandbox (pid 893784)"
- `the_shared_contract_suite_is_clean` → suite case `sandbox::the_process_space_is_confined`,
  which reported reaching "process 894221 — a process OUTSIDE the sandbox, and the very
  process that built it. The permission check every signal performs succeeded"
- `the_rendered_command_starts_with_the_backend_and_carries_every_root`
The failure is behavioural (a real `kill -0` landing on the test process), not a string
comparison. Restored; `sha256sum -c` OK; 21/21 PASS.

### T2 — gate conjunct 1, irreversible never dispatches (AK-044)
File `crates/gauntlet-app/src/govern/gate.rs`,
sha256 `9c986d1f057a1b48416a52be7c46e6bbecb498c8ed951ab374b4565cd58e653d`.
Mutation: `if facts.tier == BlastRadius::Irreversible {` → `if false && …`.
`cargo nextest run -p gauntlet-cli --test p5_hard_gate --no-fail-fast` → **1 RED**, exactly the
ruler's named test `p5_hard_gate::irreversible_never_dispatched`; the other three legs stayed
green. Restored; `sha256sum -c` OK; 4/4 PASS.

### T3 — gate conjunct 4, the runner-capability check (AK-050, AK-045)
Same file. Mutation: `if facts.runner_isolation < required {` → `if false && …`.
This is the plan's own "Done when" teeth clause. → **2 RED**:
- `p5_hard_gate::a_high_quality_dispatch_violating_one_hard_requirement_is_refused_not_averaged`
- `gauntlet-app govern::gate::tests::a_none_isolation_runner_is_refused_for_any_tier_above_contained`
`the_conservative_posture_admits_contained_critic_and_refuses_every_above_contained_tier`
stayed green — conjunct 5 catches that case independently, which is the layering working.
Restored; `sha256sum -c` OK; 7/7 and 12/12 PASS.

### T4 — the doctor's forbidden-tier scan (AK-049)
File `crates/gauntlet-app/src/doctor/checks.rs`,
sha256 `78725fbcec573d06eab031eeeb05715b00f9b13a9a9c9d9ba2ce2880eb7e4e5d`.
Mutation: `forbidden_hit = true;` → `forbidden_hit = false;`. → **3 RED**:
`a_forbidden_policy_in_the_default_dir_fails_even_when_policy_dir_is_overridden`,
`a_present_top_tier_policy_fails_check_five_and_the_run`,
`forbidden_tier_policies_under_any_spelling_or_link_fail_check_five`.
Restored; `sha256sum -c` OK; 32/32 PASS.

---

## Residual findings, ranked

Severity is stated **relative to this round's claim** (AK-044..AK-050 + P5). Where a finding
would rank higher under a different spec scope, the report says so in its own section.

| # | Severity | Finding | Locus |
|---|---|---|---|
| F1 | MINOR | The parent environment is inherited wholesale into the sandbox: no `--clearenv`, and `SandboxPolicy` has no environment field, so env-borne credentials defeat §17.3's credential-path denial and §17.2's `credential_scope` has no enforcement at its top rung. Undocumented. | `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs` `BASELINE_ARGS` (l.131); `crates/gauntlet-ports/src/sandbox.rs` `SandboxPolicy` |
| F3 | MINOR (MAJOR under §17.5.1) | `PolicyProvider::policy_for`'s `workspace` argument is discarded; with the shipped artifacts every builder gets read-write over the SHARED workspaces root, i.e. every other run's workspace. | `crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs:389` (`_workspace`); `.gauntlet/policies/*.json` |
| F2 | MINOR | A repo-owned artifact can declare `writable_roots: ["$HOME"]` at the `contained` tier and it resolves, validates clean, wraps, and writes outside the workspace. No `writable_roots ⊆ workspace` rule exists at either layer. | `adapter-policy-fs` `resolve_anchored_path`/`contain_within_anchor`; `adapter-sandbox-bwrap` `policy_findings` |
| F4 | MINOR | `gauntlet doctor` silently ignores every Annex E environment variable (incl. `GAUNTLET_SANDBOX_BACKEND`, `GAUNTLET_SANDBOX_POLICY_DIR`) when `GAUNTLET.md` is absent; check 5's existence clause is then skipped. Fail-closed direction. | `crates/gauntlet-app/src/doctor/mod.rs::assemble`, the `(None, None)` arm |
| F5 | MINOR | AK-049's frozen locator `tests/e2e/doctor::irreversible_policy_is_error` matches no file and no function; the mechanism is carried by differently-named unit tests. Six of seven locators satisfy the repo's naming convention; this one satisfies neither half. | `.wayfinder/answer-key.md` AK-049 vs `crates/gauntlet-app/src/doctor/mod.rs` |
| F6 | INFORMATIONAL | No rlimit or cgroup budget: cpu/address space unlimited, 31724 processes, 1048576 fds inside the sandbox. Host DoS channel. | `BASELINE_ARGS` |
| F7 | INFORMATIONAL | The whole host filesystem is readable inside the sandbox (`--ro-bind / /`), including the answer key and several credential files outside the shipped denied set. Both governing spec lines bound only writability, and both hold. | `BASELINE_ARGS`; `.gauntlet/policies/{local,cross_cutting}.json` |
| F8 | INFORMATIONAL | `wrap_builder_command` and `GovernDeps::with_backend` appear on no composed production path; AK-048's end-to-end half lands with Tasks 15-16. | `crates/gauntlet-app/src/control/controller.rs::dispatch` |
| F9 | INFORMATIONAL | The doctor's forbidden-tier scan is non-recursive. Not a bypass — `PolicyFs` reads only `<dir>/<tier>.json`. | `crates/gauntlet-app/src/doctor/checks.rs::forbidden_entries_in` |
| F10 | INFORMATIONAL | A `GateAdmission` is not bound to the command it authorizes and is `Clone`. Unreachable today. | `crates/gauntlet-app/src/govern/gate.rs::wrap_builder_command` |

No BLOCKING and no MAJOR residual survives against AK-044..AK-050 or P5.

## Tree restoration

Three files were mutated and each was restored byte-identically before the next attack.
Final verification:

```
3f510d95c6f08838ca015304a73a090785613db6b062df59b6b0eec1a40f6ad0  crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs
9c986d1f057a1b48416a52be7c46e6bbecb498c8ed951ab374b4565cd58e653d  crates/gauntlet-app/src/govern/gate.rs
78725fbcec573d06eab031eeeb05715b00f9b13a9a9c9d9ba2ce2880eb7e4e5d  crates/gauntlet-app/src/doctor/checks.rs
```
All three match the pre-mutation hashes recorded at the top of each teeth section.

Scratch target added and removed: `crates/gauntlet-cli/tests/zz_falsifier_probe_round5.rs`
(probes 1-3). Deleted; `ls crates/gauntlet-cli/tests/` shows the 13 pre-existing files and
nothing else.

`git status --short` differs from the baseline snapshot taken at the start of this round by
exactly one line — this report:

```
25a26
> ?? .agents/reports/task14/final-confirmation-round5-falsifier.md
```

Policy directory — no probe was ever written here; every doctor probe used a scratch project
root under the session scratchpad:

```
$ ls -la .gauntlet/policies/
-rw-r--r-- 1 sprime01 sprime01    0 Aug 14 22:57 .gitkeep
-rw-r--r-- 1 sprime01 sprime01  179 Aug 28 10:15 contained.json
-rw-r--r-- 1 sprime01 sprime01  271 Aug 28 10:15 cross_cutting.json
-rw-r--r-- 1 sprime01 sprime01  255 Aug 28 10:15 local.json
```

Pre-existing and untouched by this round, but worth naming since it is exactly the hazard the
brief warns about: the index still carries `D  .gauntlet/policies/irreversible.json` — a
staged deletion of a probe file some earlier round committed into this repo's history. The
working tree is clean; the index entry is not mine to resolve.

Processes: `pgrep -c bwrap` → 0. Every sandbox I spawned was a foreground child that exited,
and the lifetime probe verified its own teardown. No host process was signalled.

Final green re-verification after all restores:
- `gauntlet-adapter-sandbox-bwrap` + `-sandbox-none` + `-policy-fs`: 50/50 PASS
- `gauntlet-cli --test p5_hard_gate --test g7_tier_downgrade`: 7/7 PASS
- `gauntlet-app govern:: doctor::`: 44/44 PASS

## Bounded claims

Everything below is "I attacked X and did not break it", never "X is proven".

1. **Scope.** I judged the seven ruler lines AK-044..AK-050 and P5. Findings F2/F3/F6/F7
   touch §17.3 and §17.5 sentences that no AK line names; I did not let them decide the
   verdict, and I recorded each with the spec sentence it bears on so a wider-scoped round
   can rule differently.
2. **Threat model.** My adversary was (a) a contained payload trying to reach host state,
   (b) a caller inside the process trying to forge, replay, or downgrade an admission, and
   (c) an operator or agent authoring a hostile policy artifact. I did NOT model a
   compromised bubblewrap binary, a kernel bug in user namespaces, a malicious host `PATH`
   ahead of `locate()`, or an attacker with write access to the repo's own source.
3. **Procedure.** Behavioural probes ran on ONE host: WSL2, kernel 6.18.35.2-microsoft,
   bubblewrap 0.11.1, unprivileged uid 1000, cargo `jobs = 1`. Namespace results are host
   facts, not portable guarantees — the `--unshare-cgroup` strictness in particular means the
   backend PROBE would fail (honestly) on a host without cgroup namespaces, and I did not
   test such a host.
4. **What "no unsandboxed fallback" was tested against.** I traced every arm of
   `SandboxError`, `GateRefusal`, and `WrapRefusal` in source and found no arm convertible to
   "run it unwrapped", and T3 shows the capability conjunct has teeth. I did NOT execute a
   composed run that reaches a real backend, because no such composition exists yet (F8) —
   AK-047's escalation path is exercised by `p5_hard_gate::a_sandbox_validation_failure_escalates_without_falling_back`
   and by the contract suite's `no_unsandboxed_fallback` case, not by a live end-to-end run.
5. **AK-048.** "Wraps from process start" is verified at the rendering level (the first word
   of the rendered string is the backend binary) and behaviourally through the live p5 leg
   and the contract suite. It is NOT verified on a composed builder dispatch, because the
   composed path does not wrap today (F8).
6. **F6 was not executed.** I did not run a fork bomb or a memory bomb against the operator's
   host. The absence of limits is read off `/proc/self/limits` inside a live sandbox; the
   exploitability claim is an inference from that reading, not a demonstration.
7. **Coverage.** Four blind rounds preceded me and each found real defects in code that had
   passed a full green battery. I found four more breaks in areas none of the listed prior
   coverage names (environment, workspace scoping, policy widening, doctor's env layer). The
   honest conclusion is that this surface still yields to fresh attack angles, not that it is
   now exhausted.

VERDICT: CONFIRM
