# Task 14 — blind confirmation round, track B (executing falsifier)

Charter: `.agents/agents/falsifier.agent.md`. Blind to `.agents/reports/**`,
`.agents/research/**`, `.agents/CURRENT_STATUS.yml`, `.agents/lessons/**`,
`.agents/prompts/**`, and git commit messages. Ruler read: `.wayfinder/answer-key.md`
AK-044..AK-050. Spec read: §6.1/6.2, §8.5/8.6, §11.2, §17.1-17.5, §21.3, §22.2, §25.
Plan read: Task 14 section only.

STATUS: COMPLETE — 11 attacks executed; this file was updated after every one.

## Surface verified from the tree (not from any summary)

- `crates/gauntlet-app/src/govern/{mod,gate,posture}.rs` (17 / 989 / 265 lines)
- `crates/gauntlet-app/src/control/controller.rs` — gate wired into `dispatch`
- `crates/gauntlet-app/src/doctor/{mod,checks}.rs` — check 4 measured; check 5 pre-existing
- `crates/adapters/gauntlet-adapter-sandbox-none` (413), `-sandbox-bwrap` (846),
  `-policy-fs` (759)
- `.gauntlet/policies/{contained,local,cross_cutting}.json`; no `irreversible.json`
- `crates/gauntlet-ports/src/contract/sandbox.rs` (547) — extended contract suite
- `crates/gauntlet-cli/tests/p5_hard_gate.rs` (1260), `g7_tier_downgrade.rs` (765)

(Attacks follow.)

---

## Baseline (before any mutation)

```
$ cargo nextest run -p gauntlet-cli --test p5_hard_gate --test g7_tier_downgrade
Summary [0.084s] 7 tests run: 7 passed, 0 skipped
```
Host: WSL2, bubblewrap 0.11.1 at `/usr/bin/bwrap` (a real backend IS present, so no
attack below is blocked by a missing backend).

Attack probes were written to a throwaway target
`crates/gauntlet-cli/tests/zz_falsifier_attacks.rs` (an ADDED file, never a mutation of
landed source) and deleted at the end of the run.

---

## Attack 3b — policy-driven bwrap flag smuggling — **BROKE IT (executed)**

**What I tried.** `gauntlet-adapter-sandbox-bwrap::BwrapSandbox::render` builds the wrapped
command as `words.join(" ")` where `words` contains `policy.writable_roots` and
`policy.denied_paths` pushed RAW (`root.clone()`, `denied.clone()`) with no quoting at all.
`policy_findings` only checks `Path::new(root).is_absolute()`. An absolute path may contain
spaces. So a path entry of the form `/real/path --share-net` renders as two shell words and
becomes a real bwrap flag.

**Command.**
`cargo nextest run -p gauntlet-cli --test zz_falsifier_attacks --no-capture --no-fail-fast`
(test `attack_policy_smuggles_share_net`)

**Observed result.** Validation passed clean, wrap succeeded, and the rendered command was:

```
/usr/bin/bwrap --ro-bind / / --dev /dev --proc /proc --unshare-net --die-with-parent \
  --bind /tmp/.../ws --share-net /tmp/.../ws --share-net -- sh -c 'true'
```

`--share-net` now appears TWICE as a real bwrap argument after `--unshare-net`. bubblewrap
applies namespace flags in order, so the last one wins: the deny-by-default network posture
the policy DECLARES (`"network":"deny_by_default"`, and `PolicyValidation` reports clean) is
silently defeated by a `writable_roots` string.

**My reading.** The `single_quote_escape` helper protects the *command* payload only. Every
other word — including all operator/policy-supplied paths — is interpolated unquoted into a
string that is documented and used as "ONE shell-invocable string" (the p5 proof itself
executes it via `sh -c $wrapped.command`). Containment-widening flags are reachable from a
policy artifact. Two realistic triggers, neither requiring a malicious actor:

- a workspace root or `$HOME` containing a space (`/home/some user`) — the SHIPPED
  `["$WORKSPACE_ROOT"]` artifacts then render broken/injectable arguments;
- any future surface that lets a policy file be authored outside the repo's own review.

**Severity: MAJOR.** It is not BLOCKING today only because no production caller executes a
`SandboxedCommand` yet (see Attack 1 below) — but the defect is in the landed adapter, the
proof suite does not cover it, and Task 14's own claim is "no unsandboxed fallback".

## Attack 3f — a repo policy artifact DEFEATS the declared network posture — **BROKE IT (executed, decisive)**

**What I tried.** Same unquoted-word-join defect as 3b, but driven end-to-end through the
REAL `PolicyFs` reading a REAL policy JSON file, using `denied_paths` (which renders as
`--tmpfs <path>`, a one-argument flag, so a trailing zero-argument flag lands cleanly in
bwrap's grammar). Policy A and policy B are byte-identical except that B's denied path has
` --share-net` appended. Both are read by `PolicyFs::policy_for`, validated by the real
backend, wrapped, and executed with `sh -c`. Probe: `cat /proc/net/dev` (`/proc` is mounted
fresh by `--proc /proc`, so it reflects the sandbox's own network namespace).

**Command.**
`cargo nextest run -p gauntlet-cli --test zz_falsifier_attacks --no-capture --no-fail-fast -E 'test(attack_policy_file_defeats_network_containment)'`

**Observed result.**

```
CLEAN  : ... --unshare-net --die-with-parent --tmpfs /tmp/.../masked --bind WS WS -- sh -c 'cat /proc/net/dev'
         -> only `lo`  (37 whitespace tokens)
HOSTILE: ... --unshare-net --die-with-parent --tmpfs /tmp/.../masked --share-net --bind WS WS -- sh -c '...'
         -> lo, eth0, eth1, loopback0, eth2, eth3, eth4  (139 tokens) — the HOST namespace
hostile validation clean = true
```

**My reading.** A string inside a `.gauntlet/policies/*.json` artifact silently turns off the
network namespace, and `Sandbox::validate` still reports `is_clean() == true` and
`isolation: Process`. The gate's conjunct 6 ("the tier's policy resolves and validates
CLEAN") therefore admits a dispatch whose containment does not match its declared posture.
`PolicyValidation` — the artifact AK-047/AK-048 rest on to prove "validation preceded use" —
is a FALSE CLEAN here. The whole class: `BwrapSandbox::render` does
`words.join(" ")` with `policy.writable_roots` / `policy.denied_paths` pushed raw
(`root.clone()` / `denied.clone()`); the only guard is `Path::new(x).is_absolute()`, which is
true for any string starting with `/`, spaces and all. `single_quote_escape` protects the
COMMAND payload only.

Accidental (non-adversarial) trigger of the same class: an operator whose `$HOME` or
workspace root contains a space. The SHIPPED `local.json` / `cross_cutting.json` deny
`$HOME/.ssh`, `$HOME/.aws`, `$HOME/.gnupg`; under `/home/some user` those render as
mis-split bwrap arguments — silently broken containment, not a refusal.

**Severity: MAJOR.** Locus: `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs`
`render()` (lines ~209-245) and `policy_findings()` (~163-201).
Mitigating (stated, not excusing): no production caller executes a `SandboxedCommand` yet —
the executing runners land in Tasks 15/16. Aggravating: the p5 proof itself executes
`wrapped.command` through `sh -c`, so the string is already treated as an executable command
line by this task's own evidence.

## Attack 3e — `..` traversal escapes the `$WORKSPACE_ROOT` anchor — **BROKE IT (executed)**

**What I tried.** `writable_roots: ["$WORKSPACE_ROOT/../outside"]` through the real `PolicyFs`.

**Command.** same target, test `attack_traversal_escapes_anchor_for_real`.

**Observed result.**
```
[attack3e] resolved = ["/tmp/.../ws/../outside"]
[attack3e] canonicalized = /tmp/.../outside
POLICY WRITABLE ROOT ESCAPED ITS $WORKSPACE_ROOT ANCHOR
```

**My reading.** `resolve_anchored_path` (policy-fs lib.rs ~183-238) does token-prefix
substitution and an `is_absolute()` check, and nothing else — no normalization, no
`canonicalize`, no prefix re-check, no `..` rejection, no symlink check. Spec §17.5.2 states
the rule this locus does not apply: "Workspace paths stay inside `workspace.root`, verified
by absolute-path prefix check **after canonicalization** (symlink escape is checked, not
assumed absent)." A `$WORKSPACE_ROOT`-anchored writable root therefore only *looks* anchored.
Same for `$PROJECT_ROOT` and `$HOME`.

**Severity: MAJOR** (containment boundary is policy-declared and the declaration is not
enforced). Locus: `crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs:183-238`.

## Attack 2 — forged `GateAdmission` yields a silent no-wrap — **BROKE IT (executed)**

**What I tried.** `GateAdmission` is a `pub struct` with all-`pub` fields. Under a REAL-backend
posture I took a genuinely admitted admission and set `validation: None` while keeping
`policy: Some(..)`, then called `AuthorityGate::wrap_builder_command`.

**Observed result.** `Ok(None)` — no error, no refusal.
`[attack2] forged-admission wrap result is_none = true`

**My reading.** `wrap_builder_command`'s `let ... else { return Ok(None) }` collapses two very
different facts into one value: "this posture legitimately needs no wrap (contained/none)" and
"the admission I was handed is malformed". A Task-15/16 caller writing the obvious
`if let Some(c) = wrap_builder_command(..) { run(c) } else { run(raw) }` gets an unsandboxed
fallback for free. The gate's own `admit` never mints the mixed shape, and `ControllerDeps`
forces a `GovernDeps` at every construction site — so this is not exploitable through the
composed controller TODAY.

**Severity: MINOR today / a latent MAJOR at the Task-15/16 boundary.** Locus:
`crates/gauntlet-app/src/govern/gate.rs` `wrap_builder_command` (~520-540) plus the all-`pub`
`GateAdmission` fields (~215-232). I am reporting it as MINOR because I could not reach it
from a production path; I am not claiming it is safe.

## Attack 4 — lattice forgery on the isolation ladder — **failed to break**

`Isolation` derives `PartialOrd, Ord` with declaration order `None < Process < Container < Vm`
(`crates/gauntlet-ports/src/agent_runner.rs:205-211`) — I asserted each adjacent pair
directly. `required_isolation` is `Contained→None`, `Local|CrossCutting→Process`,
`Irreversible→Vm`, and the gate compares `facts.runner_isolation < required`. I swept the
full 4x4 tier x isolation cross-product through `AuthorityGate::admit` under the conservative
posture: every above-contained tier refused at every rung (by conjunct 1 or 5), contained
admitted at every rung. No manual/hand-rolled isolation comparison exists anywhere
(`grep -rn` for isolation comparisons found only the two `<` sites in gate.rs and checks.rs).

## Attack 1 — bypass the gate at dispatch — **failed to break (bounded)**

**Search procedure (this is the scope of the "none found").** Whole-workspace `grep -rn` for
`RoleInvocationSpec {` and `role: RoleId::` across `crates/*/src` (production source only,
adapters excluded as they receive specs rather than mint them), plus `grep -rn "\.invoke("`
across all crates.

**Findings.**
- The ONLY `role: RoleId::Builder` `RoleInvocationSpec` construction in non-test production
  source is `crates/gauntlet-app/src/control/controller.rs:1721`, inside `dispatch`, AFTER
  `self.gate.admit(...)`. `crates/gauntlet-app/src/control/scheduler.rs:559,1225` are
  `RoleId::Builder` mentions inside the scheduler's own test module.
  `crates/gauntlet-app/src/compare/review.rs:1147` (a `RoleId::Critic` forged spec) sits at
  line 1147, below the file's `#[cfg(test)]` at line 850 — test-gated.
- `self.dispatch(` has exactly ONE call site (controller.rs:1411). `self.runner.invoke` has
  exactly ONE call site in the controller (1749), inside `dispatch`, after the gate and after
  `record_gate_admission` pins the admission record — so the evidence ORDER (admission record
  before invocation record) is what carries AK-048's claim.
- `dispatchable_member` refuses any multi-unit slice with `SliceNotDispatchable`, so the
  `recorded` lookup's `.first()` cannot silently ignore a second member with a different tier.
- `ControllerDeps` has a required non-optional `govern: GovernDeps` field, no `Default` impl,
  and `GovernDeps` has no `Default` impl. `GovernDeps::with_backend` refuses the three
  inconsistent postures (default token + instances; real token + missing sandbox; real token
  + missing policies) — I read all three arms and the `trim().is_empty()` guard.

**Residual (INFORMATIONAL, out of AK-044..050's scope but recorded).**
`crates/gauntlet-app/src/verify/ladder.rs:490-511` constructs a `RoleId::Verifier`
`RoleInvocationSpec` with `sandbox_policy: None` and calls `runner.invoke` WITHOUT traversing
the authority gate. `ModelRung::new`/`rung()` are `#[cfg(any(test, feature = "composition"))]`
and the CLI enables `composition` (`gauntlet-cli/Cargo.toml:48`), so this is reachable at the
composition root. Spec §17.5.4 scopes the wrap rule to "every BUILDER invocation above
contained" and §17.5.1 to "builder and critic processes", so this verifier rung is outside
the letter of AK-048 — I am not scoring it as a Task-14 defect, but it is a second
ungoverned door to a role process and Tasks 15/16 will make it a real one.

**Bound.** This covers Rust source in `crates/` reachable without `#[cfg(test)]`. It does NOT
cover: a future `gauntlet run` CLI subcommand (none exists — `main.rs` dispatches only
`doctor` and `reference`), FFI, or build scripts.

## Attack 5 — irreversible smuggling — **failed to break**

**5a — through the gate.** `AuthorityGate::admit` conjunct 1 refuses
`BlastRadius::Irreversible` before anything else; my 4x4 sweep (Attack 4) confirmed refusal at
every isolation rung and both postures.

**5b — through the policy provider.** `PolicyFs::policy_for` returns
`NoPolicyForTier { tier: Irreversible }` BEFORE reading any file, even with a hostile
`irreversible.json` planted in the directory. Executed: `attack_irreversible_routes` printed
`Err(NoPolicyForTier { tier: Irreversible })`.

**5c — through the backend.** A hand-built `SandboxPolicy { tier: Irreversible, .. }` is
refused by `BwrapSandbox::validate` (`policy_findings` first rule) — executed, `is_err()`.
`wrap` re-derives `policy_findings`, so a hand-forged clean `PolicyValidation` does not
bootstrap it either (read, not separately executed).

**5d — doctor fails on a planted irreversible policy (AK-049).**
```
$ printf '{...}' > .gauntlet/policies/irreversible.json
$ just doctor
  5. sandbox policies         fail
      - no top-tier policy   fail
          a policy file exists for the top tier ('irreversible.json' in policy dir
          '.gauntlet/policies') — its presence is a configuration error (AK-049, spec 17.3)
exit: FAIL (check 5: sandbox policies)
error: recipe `doctor` failed on line 49 with exit code 1     [process exit code 1]
```
File removed afterwards; `git status --short .gauntlet/` re-verified as the three expected
untracked artifacts only, and `.agents/reports/doctor.json` restored to its pre-attack
sha256 `9486ec5a…7bfe7f9e`.

**5e — evasion sweep on check 5's stem rule.** Detected (doctor fails): `IRREVERSIBLE.JSON`,
`irreversible` (no extension), `irreversible.yaml`, `irreversible.txt`. NOT detected:
`irreversible.json.bak`, `irreversible .json`. **INFORMATIONAL, not a defect** — the rule is
`file_stem().eq_ignore_ascii_case("irreversible")` and those two names have a different stem,
but neither is a file any loader would ever read (`PolicyFs` reads exactly
`<tier.wire()>.json` and structurally refuses the top tier before opening anything). They are
inert files, not "a sandbox policy file for the irreversible tier".

## Attack 6 — do the shipped `.gauntlet/policies/*.json` reach a PRODUCTION composition path? — **partly no (INFORMATIONAL, scope fact)**

**What I tried.** Traced every production reader of the policy directory.

**Observed result.**
- `crates/gauntlet-cli/src/main.rs` dispatches exactly two subcommands (`doctor`, `reference`,
  lines 21-24). There is NO `run` subcommand — so no production path dispatches a Unit at all
  yet, and therefore none resolves a tier policy through `PolicyFs`.
- `check_policies` (checks.rs:881-1000) reads the directory but only (a) scans for a
  forbidden `irreversible`-stem entry and (b) tests `file.exists()` for the three tier files
  when a backend is configured. It NEVER decodes a policy. Spec §8.5 check 5 asks literally
  for existence, so this is conformant — but the shipped artifacts' CONTENT is validated by
  nothing in production.
- `measure_backend` (main.rs:90-146) builds a synthetic `doctor-probe` policy over `/tmp`; it
  does not read the shipped files.
- The only place the real `PolicyFs` decodes the real `.gauntlet/policies/*.json` is the p5
  proof (`p5_hard_gate.rs:1124-1150`).

**Executed check of the production composition root actually measuring the real backend:**
```
$ GAUNTLET_SANDBOX_BACKEND=bwrap just doctor
  4. capabilities             unavailable
      - tier rungs           pass
          tier contained requires rung none; backend achieves process
          tier local requires rung process; backend achieves process
          tier cross_cutting requires rung process; backend achieves process
  5. sandbox policies         pass
      - tier policies exist  pass   (contained / local / cross_cutting present)
exit: OK
```
So the composition root DOES reach the real adapter and MEASURES the real rung. It does not
reach the real policy *content*.

**My reading.** "The shipped artifacts compose through the real provider" is true only in the
proof. A corrupt or over-permissive shipped policy would pass `just doctor` today. That is
inside spec §8.5 check 5's letter, so I score it INFORMATIONAL, not a defect — but it is why
Attack 3f's finding is not caught anywhere: nothing in the production preflight ever parses
those files.

---

## Attack 7 — TEETH RE-VERIFICATION BY MUTATION

Protocol per mutation: `sha256sum` before → minimal inverting edit → run the named test →
observe FAIL → restore from a byte copy → re-verify sha256 → re-run → observe GREEN.
Pre-mutation digests recorded in scratch (`pre-mutation.sha256`).

| # | Locus | Mutation | Named test | Result |
|---|---|---|---|---|
| M1 | `govern/gate.rs` conjunct 4 | delete the `facts.runner_isolation < required` capability check (the plan's OWN declared teeth mutation) | `gauntlet-app govern::gate::tests::a_none_isolation_runner_is_refused_for_any_tier_above_contained` AND `gauntlet-cli::p5_hard_gate::a_high_quality_dispatch_violating_one_hard_requirement_is_refused_not_averaged` | **FAILED as required** |
| M3 | `govern/gate.rs` conjunct 1 | delete the `tier == Irreversible` refusal | `govern::gate::tests::irreversible_is_refused_before_every_other_conjunct` | **FAILED as required** |
| M3 | (same mutation) | | `p5_hard_gate::irreversible_never_dispatched` | **STILL PASSED — see note** |
| M3b | `control/scheduler.rs:522` | delete the scheduler's §11.2 irreversible eligibility conjunct | `p5_hard_gate::irreversible_never_dispatched` | **FAILED as required** |
| M2 | `govern/posture.rs` | honor an agent-originated DECREASE (`return Allowed`) | `g7_tier_downgrade::an_agent_role_cannot_move_a_units_lattice_in_any_direction` + `::a_dispatch_fact_below_the_recorded_facts_is_refused_as_a_downgrade` | **FAILED as required** (the third g7 test still passed) |
| M4 | `govern/gate.rs` conjunct 6 | swallow the `Sandbox::validate` error and return a policy-less admission (an unsandboxed fallback) | `p5_hard_gate::a_sandbox_validation_failure_escalates_without_falling_back` | **FAILED as required** |
| M5 | `gauntlet-adapter-sandbox-bwrap` `wrap` | delete the `!validation.is_clean()` guard | `gauntlet-adapter-sandbox-bwrap tests::the_shared_contract_suite_is_clean` reporting `[sandbox::no_unsandboxed_fallback]` (AK-047's frozen locator) + `tests::wrap_gates_on_its_own_clean_process_rung_validation` | **FAILED as required** |
| M6 | `doctor/checks.rs` forbidden-stem scan | `&& false` on the stem match | `doctor::tests::a_present_top_tier_policy_fails_check_five_and_the_run`, `::forbidden_tier_policies_under_any_spelling_or_link_fail_check_five`, `::a_forbidden_policy_in_the_default_dir_fails_even_when_policy_dir_is_overridden` (AK-049) | **FAILED as required** |

**Note on M3 (the one interesting teeth result).** With the gate's irreversible conjunct
deleted, `p5_hard_gate::irreversible_never_dispatched` — AK-044's own frozen locator — still
PASSES. That leg's teeth belong entirely to the SCHEDULER's §11.2 conjunct (M3b proves it),
not to the Task-14 gate. This is not a mis-scoped test: AK-044 says the refusal is "at the
scheduler level", and the gate's conjunct 1 is documented in gate.rs as a "dispatch-time
second line". But it does mean the Task-14 gate's own irreversible conjunct is covered ONLY
by a `gauntlet-app` unit test, never by the P5 proof target the ruler names. I record it as
INFORMATIONAL, with the observation that M3+M3b together still leave a third line standing
("tier irreversible has no concurrency entry") — the defense is genuinely layered.

**Restoration verified.** All six mutated files re-hashed byte-identical to their pre-mutation
sha256 (gate.rs `0c41d045…b28b`, posture.rs `04fd5d99…dc90`, bwrap lib.rs `044fe322…1d6d`,
controller.rs `51cbdc6f…9748`, checks.rs `2452d579…4f75d`, scheduler.rs `0524c92d…9765`), then
re-run:
```
p5_hard_gate + g7_tier_downgrade : 7 tests run: 7 passed
gauntlet-app govern:: + doctor:: : 43 tests run: 43 passed
sandbox-bwrap + sandbox-none + policy-fs : 35 tests run: 35 passed
```

---

## Attack 8 — `Sandbox::validate` is not a dry run OF THE POLICY — **BROKE IT (executed)**

**What I tried.** `BwrapSandbox::probe_backend` spawns
`<binary> <BASELINE_ARGS> true` — the baseline shape ONLY. It never spawns the policy's own
rendered mounts. `policy_findings` then judges the policy purely syntactically
(`is_absolute()`, tier, backend token, network form). So I asked: can a policy that
CANNOT START validate clean? The adapter's own doc claims the missing-mount case is
"fail-closed: the contained process never runs with a mask missing" — true, but the failure
lands at process start, AFTER validation reported clean and AFTER the gate admitted.

**Command (direct, no tree mutation) — the SHIPPED `local.json` rendered with this host's real
anchors:**
```
$ bwrap --ro-bind / / --dev /dev --proc /proc --unshare-net --die-with-parent \
    --tmpfs /home/.../projects/gauntlet/.git --tmpfs /home/.../.ssh \
    --tmpfs /home/.../.aws --tmpfs /home/.../.gnupg \
    --bind /tmp/gauntlet/workspaces /tmp/gauntlet/workspaces -- sh -c 'echo BUILDER-RAN'
bwrap: Can't mkdir /home/.../.aws: Read-only file system
exit=1
$ ls -d ~/.aws ~/.gnupg
ls: cannot access '/home/.../.aws': No such file or directory
ls: cannot access '/home/.../.gnupg': No such file or directory
```

**My reading.** On THIS host — the host Task 0 inventoried and Task 14 targeted — the shipped
`local.json` and `cross_cutting.json` produce a sandbox that cannot start, because they deny
`$HOME/.aws` and `$HOME/.gnupg` which do not exist here. `validate()` still returns
`is_clean() == true, isolation: Process`; the gate admits; `record_gate_admission` pins an
admission record; and only then does the process die. AK-047's "Sandbox policy validation
precedes execution" is satisfied only in the shallow sense — the validation exercises the
BACKEND, not the POLICY. Spec §6.2 calls it "`validate(policy) -> PolicyValidation` (dry run)".

**Severity: MINOR.** The failure mode is fail-closed (refuse to run, not run unsandboxed), so
it is not a containment hole. It is scored MINOR rather than higher for that reason, and
because Attack 3f is the same file's more serious sibling. Locus:
`gauntlet-adapter-sandbox-bwrap/src/lib.rs` `probe_backend` (~97-160) and `validate` (~265-287).

## Attack 9 — what the SHIPPED `contained.json` actually contains — **observations (MINOR / INFORMATIONAL)**

Executed directly against the shipped `contained.json` shape
(`--ro-bind / / … --bind $WORKSPACE_ROOT $WORKSPACE_ROOT`, `denied_paths: []`):

```
$ bwrap … -- sh -c 'head -c 120 <repo>/.wayfinder/answer-key.md'
## AK-001
claim: The domain layer has zero compile-time dependencies on ports, adapters, …

$ bwrap … -- sh -c 'ls -a ~/.ssh'
.  ..  agent  id_ed25519  id_ed25519.pub
```

**My reading.** Two facts, both scoped honestly:

1. **MINOR** — the `contained` tier's sandbox exposes the operator's SSH private key
   read-only. Spec §17.3 puts "explicit deny of credential paths" at the `local` tier and
   above, and `contained` at "read-write limited to the workspace" — so `denied_paths: []` at
   contained is literally conformant. But `--ro-bind / /` is a choice this task made, and it
   means the *weakest* tier reads every credential on the host. I report it as a posture
   finding, not a spec violation.
2. **INFORMATIONAL** — the frozen answer key is READABLE from inside a contained sandbox.
   §17.5.5 constrains only WRITABLE paths, and G10/AK-049 are about writing, so this violates
   no unit I am measuring against. It is nonetheless the kind of read a blind-envelope regime
   would want denied, and I record it so the decision is deliberate.
3. **INFORMATIONAL** — under the §8.6 shipped default (`sandbox.backend = none`), a
   `contained` dispatch runs with NO sandbox: no ephemeral workspace, no network denial, no
   write limiting, i.e. none of §17.3's contained posture. That is the spec's own default and
   the gate records it honestly in the admission evidence (`"policy":null,"note":"contained
   under the conservative no-backend posture"`), and every higher tier refuses — so it is a
   declared posture, not a fallback. Recorded because "no unsandboxed fallback" and "the
   default configuration runs builders unsandboxed" are both true at once.

## Attack 10 — did Task 14 weaken a gate to land? — **partly, narrowly, and pinned**

`crates/gauntlet-arch-tests/src/denylist.rs` adds `"none"` to `SPEC_VOCABULARY_SEGMENTS`,
which stops the boundary scan from deriving the bare word `none` as a denied adapter segment.
I checked the blast radius: the pinned test still derives and denies
`gauntlet-adapter-sandbox-none`, `sandbox-none`, `sandbox_none`, `none-sandbox`,
`none_sandbox`, and the `assert_eq!(SPEC_VOCABULARY_SEGMENTS, &["model","digest","check","none"])`
pin means growth is a deliberate edit. `none` genuinely is spec vocabulary (§6.1
`isolation: none`, §8.2 `sandbox.backend`, §8.6's default value). I attacked the relaxation by
writing a scratch integration test that spells `bwrap` as a bare string literal in
`crates/gauntlet-cli/tests/` — the suite CAUGHT it with 15 violations across
`denylist::repo::core_crates_contain_no_denied_identifiers`,
`literals::repo::core_crates_contain_no_hardcoded_literals`, and
`architecture_boundaries::denylist`. After deleting the scratch file:
`just boundaries` → `215 tests run: 215 passed`. `just ruler` → PASS.

**Verdict on this attack: the relaxation is narrow, argued, pinned, and the suite still has
teeth. INFORMATIONAL.**

## Attack 11 — is the WRAP actually wired into a composed dispatch? — **no (MINOR, scope)**

**Search procedure.** `grep -rn "wrap_builder_command" crates/`.

**Observed result.** Five hits: the definition (`govern/gate.rs:535`), one `gauntlet-app` unit
test (gate.rs:935), and three in `p5_hard_gate.rs` (1060/1079/1096). **Zero production
callers.** `RunController::dispatch` never calls it: it sets
`sandbox_policy: admission.policy_ref()` (a NAME, `SandboxPolicyRef`) on the
`RoleInvocationSpec` and calls `runner.invoke(spec)`. No composed dispatch ever produces or
consumes a `SandboxedCommand`.

I also confirmed the shape of the composed proof legs: the three `live_govern` (real backend +
real `PolicyFs`) legs at p5:851/931/1028 drive the CONTROLLER only for REFUSAL outcomes
(capability mismatch, validation escalation); the AK-048 leg at 1028 calls `gate.admit` and
`gate.wrap_builder_command` DIRECTLY and executes the wrapped string with `sh -c`. So the
composed controller never reaches a successful above-contained admission — additionally
because `PolicyFs::policy_for` and `BwrapSandbox::probe_backend` read the REAL wall clock
while the controller's `operation_bound()` comes from an injected (deterministic) `Clock`; I
saw this directly in the M1 mutation run, where the controller's policy resolution failed
with "the resolve deadline by 1700003600s has no time left".

**My reading.** AK-048's "containment wraps the builder session from process start" is
demonstrated at the GATE API with a real backend and a real executed command — that is
genuine, and I executed it myself in the baseline. It is NOT demonstrated through a composed
dispatch, and cannot be until a process-spawning runner exists (Tasks 15/16); the in-process
mock runner has no process to wrap. That is honest scoping, but it means the claim
"containment wraps the builder session" is true of the mechanism, not of any dispatch this
repository can currently perform. Combined with Attack 2, the Task-15 integration point is
where the residual risk sits: an `Option<SandboxedCommand>` whose `None` arm is ambiguous,
handed to a caller that does not exist yet.

**Severity: MINOR** (scope/coverage, not a hole).

---

## What survived my attacks (stated as attacks that failed, never as proof)

- **The authority gate itself.** Conjuncts 1-6 refused everything I threw at them: the full
  4x4 tier x isolation cross-product, every authority above the critic ceiling, both postures,
  and every irreversible route (gate, provider, backend). `GovernDeps::with_backend` refused
  all three inconsistent postures. `ControllerDeps.govern` is mandatory with no `Default`
  anywhere in the chain. I could not find a second builder-dispatch door.
- **The lattice.** `Isolation`'s derived `Ord` matches declaration order; `required_isolation`
  is monotone with `blast_rank`; no hand-rolled isolation comparison exists in the workspace.
  I could not construct a comparison saying a weaker rung satisfies a stronger tier.
- **Teeth.** Every Task-14 assertion I mutated failed as required (7 of 8 mutation/test pairs
  in the table; the eighth, M3-vs-p5, is the documented scheduler-vs-gate note).
- **AK-049.** Doctor fails on a planted `irreversible.json`, in four spellings, with a non-zero
  exit code, and the check has teeth (M6).
- **The boundary suite.** Caught my own scratch file with 15 violations; `just boundaries`
  215/215 and `just ruler` PASS after cleanup.

## Bounded claims

Every "none found" below is bounded exactly as written; nothing here is an unbounded
all-clear.

1. **Gate-bypass search.** Scope: Rust source under `crates/**/src` and `crates/**/tests`,
   non-`#[cfg(test)]` paths, searched by `grep -rn` for `RoleInvocationSpec {`,
   `role: RoleId::`, `.invoke(`, `wrap_builder_command`, and by reading every `self.dispatch`
   / `self.runner.invoke` call site in `controller.rs`. Threat model: a caller inside this
   workspace reaching a BUILDER process start. NOT covered: a future `gauntlet run` CLI
   subcommand (none exists), FFI, build scripts, procedural macros, or any dispatch a
   Task-15/16 runner will introduce. Found: one gated builder door; one UNGATED `RoleId::
   Verifier` door (`verify/ladder.rs:490-511`, reachable at the composition root through the
   `composition` feature) which is outside AK-044..050's letter.
2. **Injection search.** Scope: `BwrapSandbox::render`, `policy_findings`,
   `single_quote_escape`, and `resolve_anchored_path`, read line by line, plus executed
   probes for embedded whitespace, `..` traversal, unknown/mid-string anchor tokens, and
   irreversible tiers. NOT covered: newline/NUL in path entries (I did not execute those),
   symlinked writable roots, the `sandbox-none` adapter's own render path (I read it, I did
   not attack it), or `NetworkPosture::Allowlist` (refused as a finding, so unreachable).
3. **Environment-blocked attacks — none.** bubblewrap 0.11.1 is present at `/usr/bin/bwrap`
   and every live leg ran. No attack below was blocked by a missing backend. The ONE
   environment-dependent result I am flagging as such: Attack 8's shipped-policy start
   failure depends on `~/.aws` / `~/.gnupg` not existing on this host — the defect
   (validation not exercising the policy) is host-independent, the specific failure is not.
4. **Not attacked at all.** `cargo mutants` (charter says ask first; not run), the
   `sandbox-none` adapter's contract behaviour beyond running its suite, concurrency/TOCTOU
   between validate and wrap, the `escape_json_text` governance-record encoder, and the
   `permissions_for` half of `PolicyProvider` (honestly out of scope by its own doc).
5. **Blindness held.** I read `.wayfinder/answer-key.md` (AK-044..050), `Core-spec.md`
   §6.1/6.2/8.5/8.6/11.2/17.1-17.5/21.3/22.2, and the plan's Task 14 section. I did not open
   `.agents/reports/**` (including the stub at this path, which I overwrote without reading),
   `.agents/research/**`, `.agents/CURRENT_STATUS.yml`, `.agents/lessons/**`,
   `.agents/prompts/**`, or any commit message. No `git log` was run.

## Residual findings, ranked

| ID | Severity | Claim | Locus | Executed |
|---|---|---|---|---|
| F1 | MAJOR | A policy artifact string smuggles extra backend flags (executed: `--share-net` defeats the declared deny-by-default network posture) while `validate()` reports CLEAN | `gauntlet-adapter-sandbox-bwrap/src/lib.rs` `render`/`policy_findings` | yes |
| F2 | MAJOR | `$WORKSPACE_ROOT/../…` escapes its anchor; no canonicalization or prefix re-check anywhere, contradicting the module's own "an artifact cannot widen its own anchor" and spec §17.5.2 | `gauntlet-adapter-policy-fs/src/lib.rs:183-238` | yes |
| F3 | MINOR | `validate()` dry-runs the BACKEND, never the POLICY; the shipped `local.json`/`cross_cutting.json` validate clean and cannot start a sandbox on this host | `…-sandbox-bwrap` `probe_backend`/`validate` | yes |
| F4 | MINOR | `wrap_builder_command` returns `Ok(None)` for a malformed admission, indistinguishable from "no wrap needed"; `GateAdmission` fields are all `pub` | `govern/gate.rs` ~215-232, ~535 | yes |
| F5 | MINOR | The `contained` tier's sandbox exposes `~/.ssh/id_ed25519` (and the frozen answer key) read-only via `--ro-bind / /` with `denied_paths: []` | `.gauntlet/policies/contained.json` + adapter baseline | yes |
| F6 | MINOR | No production caller wraps anything: `wrap_builder_command` has zero non-test callers; the composed controller never produces a `SandboxedCommand` | `control/controller.rs` `dispatch` | yes |
| F7 | INFORMATIONAL | `p5_hard_gate::irreversible_never_dispatched` (AK-044's frozen locator) has teeth against the SCHEDULER conjunct, not the Task-14 gate conjunct | `p5_hard_gate.rs` / `scheduler.rs:522` | yes |
| F8 | INFORMATIONAL | An ungated `RoleId::Verifier` runner invocation exists at the composition root | `verify/ladder.rs:490-511` | no (read) |
| F9 | INFORMATIONAL | The shipped policy artifacts' CONTENT is decoded by no production path; doctor check 5 tests existence only | `doctor/checks.rs:881-1000`, `main.rs` | yes |
| F10 | INFORMATIONAL | Under the §8.6 default posture a `contained` dispatch runs with none of §17.3's contained containment (spec-conformant, recorded honestly in evidence) | `govern/gate.rs` none-posture arm | yes |
| F11 | INFORMATIONAL | `SPEC_VOCABULARY_SEGMENTS` gained `"none"`, narrowing the denylist; compound spellings still denied and the list is pinned | `arch-tests/src/denylist.rs` | yes |

## Tree restoration

Six files were mutated and restored byte-identically; each re-hashed against its
pre-mutation `sha256sum` and re-run green:

```
0c41d0459861eb870b7eaa0c5a032fa0bf08c796d9c65e919489617cfb91b28b  crates/gauntlet-app/src/govern/gate.rs
04fd5d99394aa2c58ff84d24380f2e5fb060dcc6750230509f38a256660edc90  crates/gauntlet-app/src/govern/posture.rs
044fe322b5bc95c4cf4f4bc48e4f334a93517307bc8763ee30ea15fbe82c1d6d  crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs
51cbdc6f41da5ca688ed5c5d119dcb0a33fa453d98e09837c4c6223ed8b9d748  crates/gauntlet-app/src/control/controller.rs
2452d5793643bba8bc5d35a6f5b08c84b1f1f11f417550d7404e2fdf1a54f75d  crates/gauntlet-app/src/doctor/checks.rs
0524c92dca8d4141ae8eba72a09cd87d5a4048bbfab51a3503d65e6d46739765  crates/gauntlet-app/src/control/scheduler.rs
```
The added scratch target `crates/gauntlet-cli/tests/zz_falsifier_attacks.rs` was deleted.
`.agents/reports/doctor.json` was restored to its pre-attack sha256
`9486ec5ad268efcc8edbcb2e815caf2e0a9809b3df2da94f252bc936b7fe7f9e`, and the four
`irreversible*` probe files planted in `.gauntlet/policies/` were removed
(`ls` re-verified: only `.gitkeep`, `contained.json`, `cross_cutting.json`, `local.json`).

**I ran `git status --short` at the end.** No source file is left modified by my mutations:
the modified/untracked set is byte-for-byte the Task-14 set I started with, plus
`.agents/reports/task14/` (this report) and background writes by other agents/tooling
(`.agents/reports/reference-candidates.md`, `.jolli/jollimemory/*`) that I did not touch and
am blind to. Final gates: `just check` clean, `just boundaries` 215/215, `just ruler` PASS,
`p5_hard_gate` + `g7_tier_downgrade` 7/7, `gauntlet-app govern::`+`doctor::` 43/43,
sandbox/policy adapters 35/35.

## Verdict rationale

The GOVERNANCE half of Task 14 — authority decides whether, containment decides how contained,
both before the runner is invoked, irreversible never dispatched, no `irreversible` policy,
validation failure escalates rather than falls back — survived every attack I mounted, and its
assertions have demonstrated teeth. The CONTAINMENT-ADAPTER half does not hold: a policy
artifact can silently widen containment past its own declaration while validation reports
clean (F1), and an anchored path can escape the anchor the adapter documents as inescapable
(F2). Both are executed, both are in landed Task-14 code, both are reachable without an
adversary, and neither is caught by any test in the repository.

VERDICT: NOT CONFIRM
