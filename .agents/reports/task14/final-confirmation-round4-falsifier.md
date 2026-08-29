# Task 14 — fresh blind confirmation round 4 (executing falsifier)

Blindness held: no `.agents/reports/**` (other than this file), no
`.agents/research/**`, no `CURRENT_STATUS.yml`, no `.agents/lessons/**`, no
`.agents/prompts/**`, no commit messages (`git log --format=%H` only).
Read: `.wayfinder/answer-key.md` AK-043..AK-051 verbatim; Core-spec §6.1/6.2,
§8.5/8.6, §11.2, §17.1–17.5, §21.3, §25; `gauntlet-impl.md` Task 14 only; the
tree; live execution.

## Baseline (green before any mutation)

| gate | result |
|---|---|
| `cargo nextest run --test p5_hard_gate` | 4 passed |
| `cargo nextest run --test g7_tier_downgrade` | 3 passed |
| `cargo nextest run -p gauntlet-app govern:: doctor::` | 44 passed |
| `cargo nextest run -p gauntlet-adapter-sandbox-{bwrap,none} -p gauntlet-adapter-policy-fs -p gauntlet-ports` | 160 passed |
| `gauntlet doctor` (report dir redirected to scratch) | exit 0 |

The live bwrap legs really ran: `--no-capture` shows no
"the process-isolation backend is absent — the live leg is skipped" line, and a
bwrap spawn costs ~9 ms here, consistent with the 0.111 s wall of
`containment_wraps_from_process_start_and_confines_the_session`.

---

## Attacks executed (10 mutation rounds + 8 live probes)

### A1 — teeth: the bwrap anchoring lint — HAS TEETH
`relocatable_ancestor`, sha256 `a4f324a3…`: `.skip(1)` → `.skip(9)` (no strict
ancestor is ever examined). RED: `a_nested_denial_survives_a_rename_between_sessions`,
`only_a_relocatable_ancestor_is_a_finding`, `the_shared_contract_suite_is_clean`.
Restored byte-identically; sha256 re-verified; re-ran GREEN.

I independently reproduced the underlying attack against real bubblewrap:
`--bind R R --tmpfs R/sub/inner`, then `mv R/sub R/sub2` inside the sandbox →
`RENAME_OK`, and the host directory really moved. The lint is load-bearing.

### A2 — teeth: AK-044 conjunct 1 — HAS TEETH
`govern/gate.rs` sha256 `9c986d1f…`; `if false && facts.tier == BlastRadius::Irreversible`.
RED: `p5_hard_gate::irreversible_never_dispatched` (the ruler's own frozen
locator) and `govern::gate::tests::irreversible_is_refused_before_every_other_conjunct`.
Restored; sha256 re-verified.

### A3 — teeth: AK-047 "escalate, never fall back" — HAS TEETH
Replaced the `validate(…)?` escalation with a silent fallback to a no-policy
`NoBackendContained` admission — the literal unsandboxed-fallback defect.
RED: `p5_hard_gate::a_sandbox_validation_failure_escalates_without_falling_back`
and `govern::gate::tests::a_validation_failure_escalates_and_never_falls_back`.
Restored; sha256 re-verified.

### A4 — teeth: AK-050 capability rung — HAS TEETH
`posture::required_isolation(Local|CrossCutting)` → `Isolation::None`.
RED: `govern::gate::tests::a_none_isolation_runner_is_refused_for_any_tier_above_contained`,
`posture::tests::the_rung_table_is_the_spec_posture_and_monotone_with_blast_rank`,
`p5_hard_gate::a_high_quality_dispatch_violating_one_hard_requirement_is_refused_not_averaged`.
The plan's own "Done when" teeth clause reproduces. Restored; sha256 re-verified.

### A5 — teeth: AK-046 lattice review — HAS TEETH
`review_lattice_movement` made movement-blind (both `!=` comparisons → `false`).
RED: `g7_tier_downgrade::an_agent_role_cannot_move_a_units_lattice_in_any_direction`,
`g7_tier_downgrade::a_dispatch_fact_below_the_recorded_facts_is_refused_as_a_downgrade`,
plus three `govern::` unit tests. Restored; sha256 re-verified.

### A6 — teeth: AK-049 doctor scan — HAS TEETH
`forbidden_entries_in`: `eq_ignore_ascii_case("irreversible")` → `"irreversible-x"`.
RED: `a_present_top_tier_policy_fails_check_five_and_the_run`,
`forbidden_tier_policies_under_any_spelling_or_link_fail_check_five`,
`a_forbidden_policy_in_the_default_dir_fails_even_when_policy_dir_is_overridden`
— and the LIVE binary flipped from exit 1 to exit 0 with a forbidden file
present. Restored; live exit 1 re-verified.

### A7 — teeth: AK-048 process start — HAS TEETH
bwrap `render` made an identity wrap (`command.to_owned()`), the exact AK-048
defect. RED: 13 of 20 adapter tests including `the_shared_contract_suite_is_clean`
(the shared `wraps_from_process_start` case). Restored byte-identically
(`a4f324a3…`); 20/20 GREEN.

### A8 — AK-049 live, end to end — HOLDS
Probe written ONLY to a scratch dir via `GAUNTLET_SANDBOX_POLICY_DIR`; nothing
was ever written under the repository's `.gauntlet/policies`.
Clean dir → exit 0. `irreversible.json` planted → exit 1,
`exit: FAIL (check 5: sandbox policies)`, evidence names the file. The scan
covers BOTH the overridden dir and the default `.gauntlet/policies`, so an
override cannot hide a forbidden file in the default location.

### A9 — MAJOR: the sandbox shares the host PID and IPC namespaces
`BASELINE_ARGS` (`crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs:69`)
is `--ro-bind / / --dev /dev --proc /proc --unshare-net --die-with-parent`.
It unshares only the NETWORK namespace. Executed, not argued:

- `ls /proc` inside the wrap shows **88 host PIDs**;
- `tr '\0' ' ' < /proc/<hostpid>/cmdline` reads a host process's command line;
- `kill -TERM <pid of a process started OUTSIDE the sandbox>` →
  **"HOST HELPER WAS KILLED FROM INSIDE THE SANDBOX"**;
- `kill -STOP <host pid>` → `ps` then reports state `T` (host process suspended);
- `ipcs -m` inside the sandbox lists the host's SysV shared-memory segments
  owned by the same user.

Mitigation exists on this exact host: re-running with `--unshare-pid` leaves 4
visible PIDs and the same `kill` fails ("No such process"); `--unshare-ipc`
closes the shm channel. This is §17.3's named failure mode verbatim —
"host-mutating fallbacks explicitly refused rather than left at the backend's
default consent" — and §17.5.1's "run only inside their assigned workspace path".
`grep` confirms **no test anywhere in `crates/` asserts process-space
confinement**; the only `unshare` occurrences are `--unshare-net`.

Not credited as defenses (blocked by environment, not by the adapter):
`/proc/<pid>/root` traversal and `/proc/<pid>/environ` reads are refused only
because bubblewrap's user namespace defeats cross-userns ptrace; TIOCSTI
terminal injection is refused only because this kernel has
`dev.tty.legacy_tiocsti = 0` — `--new-session` is absent from the baseline.

### A10 — BLOCKING: an irreversible-tier policy is COMMITTED at HEAD
```
$ git ls-tree HEAD .gauntlet/policies/
100644 blob 0967ef42…  .gauntlet/policies/irreversible.json
$ git show HEAD:.gauntlet/policies/irreversible.json
{}
```
The working tree carries an UNSTAGED deletion (` D`), so the landed commit still
ships the forbidden artifact. Proven end to end without touching the tree:
```
$ git archive HEAD .gauntlet/policies | tar -x -C <scratch>
$ GAUNTLET_SANDBOX_POLICY_DIR=<scratch>/.gauntlet/policies gauntlet doctor
EXIT=1 ;  exit: FAIL (check 5: sandbox policies)
"a policy file exists for the top tier ('irreversible.json' …) — its presence
 is a configuration error (AK-049, spec 17.3)"
```
A clean checkout of HEAD fails the repository's own required preflight, and the
Task-14 goal sentence "no `irreversible` policy" is false of the landed tree.
§17.3: "The absence of an `irreversible` policy file is a *requirement*, not an
omission: writing one implies a path that must not exist."

### A11 — frozen locators: six of seven have teeth on a real name; one does not
| unit | frozen locator leaf | source files carrying the name |
|---|---|---|
| AK-044 | `irreversible_never_dispatched` | 1 |
| AK-045 | `p5_hard_gate` | 3 |
| AK-046 | `g7_tier_downgrade` | 1 |
| AK-047 | `no_unsandboxed_fallback` | 2 |
| AK-048 | `wraps_from_process_start` | 3 |
| AK-050 | `capability_mismatch` | 4 |
| **AK-049** | **`irreversible_policy_is_error`** | **0** |
`grep -r` over the whole tree (excluding `.git`/`.agents`/`target`) finds the
string only inside `.wayfinder/answer-key.md` itself.

### A12 — the §17.3 rung table is duplicated with no cross-check
`govern::posture::required_isolation` and `doctor::checks::tier_posture`
independently encode contained→none / local→process / cross_cutting→process.
Executed: with `required_isolation` mutated to `Isolation::None` for local and
cross_cutting (A4's mutation), **all 32 `doctor::` tests stayed GREEN**. A drift
in one is invisible to the other's suite.

### A13 — `PolicyValidation` is assembled, not earned
`gauntlet_ports::sandbox::PolicyValidation` and `SandboxPolicy` are public
structs with public fields and no private constructor, so any caller can forge a
clean, process-rung validation — unlike `GateAdmission`, which is deliberately
earned-only. Both adapters compensate at `wrap` by re-deriving
`policy_findings` and pinning the rung, but they do NOT re-derive the DRY RUN,
so a forged validation lets a policy that `validate` would have refused reach
`wrap`. Bounded: fail-closed in practice — the dry run fails exactly when the
real spawn would, so the rendered command aborts at startup rather than running
under-contained. Related: `wrap` links validation to policy by
`SandboxPolicyRef` string equality only; `PolicyFs` does not enforce that the
`reference` field is unique across tier artifacts, so two tier policies sharing
a `reference` would cross-authorize each other's wraps. Unreachable through
`AuthorityGate`, which always passes the matched pair from one admission.

### A14 — attacks that FAILED to break the surface (bounded)
- **Nested-root shadowing.** Policy `writable_roots: [R/sub, R]` (inner first)
  so the later `--bind R R` shadows the inner mount, then `mv R/sub R/sub2`
  → **EBUSY**. Mount-point protection is a dentry property and both binds share
  the underlying filesystem, so the shadowed mount still protects the name.
  Attack failed; the anchoring rule is not defeated this way.
- **Symlinked writable root → `/`, and → a directory outside the anchor.**
  bubblewrap refuses both binds at startup, so `dry_run_policy` converts them
  into validation findings; `PolicyFs::contain_within_anchor` additionally
  refuses them at resolution via `existing_canonical`. The module's claim that
  symlinks are "answered by the dry run" holds on this host.
- **Nested user-namespace escape from the read-only root.** `bwrap --bind / /`
  and `unshare -Urm; mount -o remount,rw /` from inside → "permission denied";
  no write outside the writable root succeeded.
- **Network.** `--unshare-net` verified live: loopback only, DNS denied.
- **Direct filesystem escape.** Writes outside the writable root fail
  read-only; masked credential paths are unreadable.
- **Policy text as command structure.** `shell_word`'s conservative allowlist
  plus the single-quote dance survived the smuggled-flag and spaced-path cases.

### A15 — composed reachability of the wrap (informational)
`AuthorityGate::wrap_builder_command` has **no production caller**:
`control::controller::dispatch` calls `gate.admit`, records the admission, sets
`spec.sandbox_policy = admission.policy_ref()`, and invokes the runner —
it never wraps. Above `contained`, containment in the composed loop is delivered
by REFUSING dispatch unless the runner already reports `isolation ≥ process`;
the wrap itself is exercised only in tests. That is consistent with Tasks 15–16
being 4- and 5-line stubs (no real runner exists yet), and with the plan's
ordering, but it bounds what "wraps from process start" currently proves in the
composed system.

Related: `gauntlet-adapter-sandbox-none`'s `BACKEND_TOKEN` is `"none"`, which is
exactly `config::defaults::SANDBOX_BACKEND`, so `GovernDeps::with_backend("none", …)`
always returns `NoneBackendWithInstances` — the none adapter can never be
composed into the gate. Harmless today (the conservative posture needs no
sandbox instance at `contained`), but the adapter is unreachable by construction.

### A16 — doctor forbidden-scan spelling robustness (informational)
`forbidden_entries_in` matches on `Path::file_stem`, so `irreversible.json.bak`
(stem `irreversible.json`) is NOT matched, although the code comment claims
"whatever its extension or lack of one". No such file is one the runtime would
read (`PolicyFs` reads `<tier>.json` only), so the gap is cosmetic against
AK-049's claim but the comment overstates the guarantee.

---

## Residual findings, ranked

| id | severity | claim | locus | executed |
|---|---|---|---|---|
| F1 | **BLOCKING** | `.gauntlet/policies/irreversible.json` (blob `0967ef42`, content `{}`) is committed at HEAD; a clean checkout fails `gauntlet doctor` (exit 1, check 5). The Task-14 goal "no `irreversible` policy" is false of the landed tree. The tree's deletion is UNSTAGED, so a plain commit would not fix it. | `git ls-tree HEAD .gauntlet/policies/` | yes |
| F2 | **MAJOR** | The bwrap baseline unshares only the network. A contained process signalled, `SIGTERM`-killed and `SIGSTOP`-suspended host processes outside the sandbox, read host `/proc/<pid>/cmdline`, and shares the host SysV IPC namespace. `--unshare-pid` / `--unshare-ipc` close it on this host. No test in `crates/` asserts process-space confinement. | `gauntlet-adapter-sandbox-bwrap/src/lib.rs:69` `BASELINE_ARGS` | yes |
| F3 | MINOR | AK-049's frozen locator leaf `irreversible_policy_is_error` appears nowhere in the source tree — the only one of AK-044..050 without a materialized cargo-target or test/case name. Evidence keyed on the locator would find nothing. | `.wayfinder/answer-key.md:346` vs `crates/**` | yes |
| F4 | MINOR | The §17.3 rung table is duplicated in `doctor::checks::tier_posture` and `govern::posture::required_isolation` with no cross-check; mutating the gate's table left all 32 doctor tests green. | `gauntlet-app/src/doctor/checks.rs:742` | yes |
| F5 | MINOR | `PolicyValidation`/`SandboxPolicy` are freely constructible (public fields, no earned constructor), so `wrap`'s "validation precedes every use is structural" rests on the adapters' re-derivation, which omits the dry run. `wrap` links validation↔policy by `SandboxPolicyRef` string equality, and `PolicyFs` does not enforce `reference` uniqueness across tiers. Fail-closed in practice; unreachable through `AuthorityGate`. | `gauntlet-ports/src/sandbox.rs:95-121` | partial (read + reasoning; not exploited) |
| F6 | INFORMATIONAL | `wrap_builder_command` has no production caller; the composed loop delivers above-contained containment by refusing dispatch on the runner's rung, not by wrapping. Bounded by Tasks 15–16 being stubs. | `control/controller.rs:1612-1750` | yes (grep + read) |
| F7 | INFORMATIONAL | `gauntlet-adapter-sandbox-none`'s token collides with the conservative sentinel, so it can never be composed into `GovernDeps`. | `sandbox-none/src/lib.rs:28` vs `config/defaults.rs:56` | yes |
| F8 | INFORMATIONAL | The doctor's forbidden-tier scan misses compound extensions (`irreversible.json.bak`) despite the comment's "whatever its extension". | `doctor/checks.rs:1034` | yes |

## Tree restoration

```
$ sha256sum -c baseline.sha256
crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs: OK
crates/adapters/gauntlet-adapter-sandbox-none/src/lib.rs: OK
crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs: OK
crates/gauntlet-app/src/govern/gate.rs: OK
crates/gauntlet-app/src/govern/posture.rs: OK
crates/gauntlet-app/src/doctor/checks.rs: OK
crates/gauntlet-ports/src/contract/sandbox.rs: OK
crates/gauntlet-ports/src/sandbox.rs: OK
crates/gauntlet-cli/tests/p5_hard_gate.rs: OK
crates/gauntlet-cli/tests/g7_tier_downgrade.rs: OK
```
All ten mutations reverted; every gate re-run GREEN after restoration
(7 + 44 + 160 tests).

No scratch cargo target, test file, or fixture was added anywhere in the repo.
Every probe (policy artifacts, sandbox scratch trees, the HEAD policy export,
the doctor report dir) lived under the session scratchpad; `GAUNTLET_REPORT_DIR`
was redirected so `.agents/reports/doctor.json` was never rewritten. The one
SysV shm segment created for the IPC probe was removed (`ipcrm -m 0`).

`ls -la .gauntlet/policies/` — no probe file of mine, and the only residue is
the HEAD-committed `irreversible.json` of finding F1, whose working-tree
deletion I found already present and did not touch:
```
.gitkeep   contained.json   cross_cutting.json   local.json
```
`git status --short` differs from the state I inherited only by this report
file.

## Bounded claims

- "I attacked X and failed to break it" is not "X is proven." Specifically:
  the nested-root shadowing attack, the symlinked-root attacks, and the nested
  user-namespace remount attack all failed **on this kernel and this bubblewrap
  0.11.1**; I did not prove they fail everywhere.
- `/proc/<pid>/root`, `/proc/<pid>/environ` and TIOCSTI were blocked by the
  environment (user namespace; `dev.tty.legacy_tiocsti = 0`), not by the
  adapter. Those are not passes. `--new-session` is absent.
- The `local` and `cross_cutting` shipped policies deny exactly
  `.git`, `~/.ssh`, `~/.aws`, `~/.gnupg`. I did not adjudicate whether that
  enumerates "credential paths" for this host; the whole root remains readable.
- F5 is reasoned from the type surface and confirmed by reading both adapters'
  `wrap`; I did not build an exploit for it.
- Cross_cutting dispatch is unreachable today under the default
  `exercisable_authority: Critic` (AK-007 forbids critic authority there), so
  the cross_cutting tier's containment posture is untested by any live path.
- The mutation battery covered the mechanisms AK-044..AK-050 name. It does not
  bound defects in mechanisms no unit names.

VERDICT: NOT CONFIRM
