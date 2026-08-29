# Task 14 — fresh blind confirmation, round 3 (executing falsifier)

Charter: `.agents/agents/falsifier.agent.md`, adopted verbatim.
Blindness: `.agents/reports/**`, `.agents/research/**`, `.agents/CURRENT_STATUS.yml`,
`.agents/lessons/**`, `.agents/prompts/**`, and commit messages were not read as sources.
**Disclosed leak (see "Blindness incidents" at the end): one repo-wide `grep -rn` printed
matching LINES from files under `.agents/reports/**` before I could exclude the directory.**

Ruler read verbatim: `.wayfinder/answer-key.md` AK-044..AK-050.

Surface established from the tree (not from any report):
- `crates/gauntlet-ports/src/sandbox.rs` — the `Sandbox` port, `SandboxPolicy`,
  `PolicyValidation`, `SandboxedCommand`, `SandboxError`.
- `crates/gauntlet-ports/src/contract/sandbox.rs` — the shared contract suite.
- `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs` — bubblewrap backend.
- `crates/adapters/gauntlet-adapter-sandbox-none/src/lib.rs` — the none backend.
- `crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs` — `PolicyProvider` over
  `<policy_dir>/<tier>.json`.
- `crates/gauntlet-app/src/govern/{mod,gate,posture}.rs` — the §17.1 gate, the tier→rung
  table, the lattice-movement review.
- `crates/gauntlet-app/src/doctor/checks.rs` check 5 + `doctor/mod.rs` — AK-049.
- `crates/gauntlet-app/src/control/controller.rs` `dispatch()` — the composed call site.
- `crates/gauntlet-cli/tests/p5_hard_gate.rs`, `crates/gauntlet-cli/tests/g7_tier_downgrade.rs`.
- `.gauntlet/policies/{contained,local,cross_cutting,irreversible}.json`.

---

## A1 — Does the shipped tree pass its own §8.5 preflight? (AK-049)

**Tried.** Before mutating anything I enumerated the policy directory the default resolves to
(`defaults::policy_dir_at` = `<project_root>/.gauntlet/policies`, pinned by its own unit test at
`crates/gauntlet-app/src/config/defaults.rs:356`) and ran the shipped doctor against the shipped tree.

**Commands.**
```
$ for f in .gauntlet/policies/*.json; do echo "== $f"; cat "$f"; done
$ git ls-files .gauntlet/policies/
$ git show HEAD:.gauntlet/policies/irreversible.json
$ cargo build -p gauntlet-cli --bin gauntlet-cli
$ ./target/debug/gauntlet-cli doctor ; echo "exit=$?"
```

**Observed.**
- `.gauntlet/policies/irreversible.json` EXISTS, is TRACKED, is COMMITTED AT HEAD, and its
  content is `{}`. `git status --short` reports it unmodified, so this is the landed tree, not
  a working-tree artifact of mine.
- Doctor output, check 5:
  ```
    5. sandbox policies         fail
        - no top-tier policy   fail
            a policy file exists for the top tier ('irreversible.json' in policy dir
            '.gauntlet/policies') — its presence is a configuration error (AK-049, spec 17.3)
  ```
- `exit: FAIL (check 5: sandbox policies)`, process exit code **1**.

**Reading.** Two facts, opposite signs.
1. The AK-049 *mechanism* fires on a real file in the real default dir with the real binary —
   that is evidence of teeth, not of absence (teeth confirmed separately in A2).
2. The *landed tree* ships the artifact its own preflight declares a configuration error. The
   §8.5 preflight is red on `main` as committed. Task 14 owns both the check and the
   `.gauntlet/policies/*` artifact set, so this is a Task-14 delivery defect, not an unrelated
   environment condition. Per charter I did **not** repair it.

Side effect to disclose: running `doctor` rewrites `.agents/reports/doctor.json` (a generated
report). It was already modified relative to HEAD when this session started; see
"Tree restoration".

---
## A2 — Mount SEMANTICS: does the sandbox deny what the policy says it denies?

**Tried.** Eleven mount shapes the crate's own live tests do NOT cover, each probed for READ
and WRITE separately, executing the REAL `BwrapSandbox::validate` + `wrap` and running the
rendered string through `sh -c` against real bubblewrap 0.11.1.

**Scratch attack target (added, then deleted — see "Tree restoration"):**
`crates/adapters/gauntlet-adapter-sandbox-bwrap/tests/zz_falsifier_r3_mounts.rs`

**Command.**
```
$ cargo test -p gauntlet-adapter-sandbox-bwrap --test zz_falsifier_r3_mounts -- --nocapture
```

**Observed (verbatim highlights).**

| shape | read | write | verdict |
|---|---|---|---|
| S1 denied path IS the writable root | `No such file or directory` | into the tmpfs; host unchanged | contained |
| S2 nested writable roots, inner listed BEFORE outer, denied file in inner | `Permission denied` | `Permission denied` | contained |
| S3 denied child whose parent is also denied | `Permission denied` | `Permission denied` | contained |
| S4 symlink created inside the sandbox from writable root → denied path | `Permission denied` | — | contained |
| S5 the denied path is itself a symlink to an outside file | `bwrap: Can't create file at …` — sandbox never starts | same | fail-closed |
| S6 nonexistent denied path OUTSIDE every writable root | `bwrap: Can't mkdir …: Read-only file system` — never starts | same | fail-closed (matches the module doc) |
| S7 nonexistent denied path INSIDE a writable root | sandbox starts | — | starts; **and the mount point is created ON THE HOST** |
| S8 rename the masked file's parent from inside | `Permission denied` at the new path (the mount follows) | — | contained within the session |
| S9 `rm` / `umount` / `mv`-over the masked file | `Device or resource busy`, `must be superuser to unmount` | refused | contained |
| S10 writable root that is a symlink | `bwrap: Can't bind mount …` — never starts | same | fail-closed |
| S11 network posture | only `lo` present (`ip -o link`) | — | `--unshare-net` is real |

**Reading.** Ten of eleven shapes held, several of them fail-closed exactly as the module doc
claims. S7's host-side `mkdir` is a real but benign side effect (the created path is inside a
declared writable root) — INFORMATIONAL. **S8 opened a crack that S12/S13 then broke open.**

---

## A3 — FINDING: a nested mask is defeated by a rename, and the next wrap silently no-ops

**Tried.** S8 showed the contained process may rename the masked path's PARENT (the rename is
a write inside a declared writable root, so the policy permits it) and that within THAT session
the mount follows the rename. I then asked the next question: what does the NEXT wrap under the
IDENTICAL, UNCHANGED policy do, now that the mask target no longer exists?

**Command.**
```
$ cargo test -p gauntlet-adapter-sandbox-bwrap --test zz_falsifier_r3_mounts -- \
      --nocapture --test-threads=1 attack_nested
```

**Observed — verbatim.**
```
S12 phase0 leaked=false
S12 phase1 mv stdout="" stderr="" host_a=false host_b=true
S12 phase2 host payload now at "/tmp/gr3-s12-605694/ws/b/secret.txt" = Some("known-secret-payload")
S12 phase2 sandbox_started=true LEAKED=true stdout="known-secret-payload" stderr=""
S12 phase2 WRITE exit_ok=true host_now=Some("PWNED\n") stderr=""

S13 phase0 leaked=false
S13 phase1 stderr="" host_b=true
S13 phase2 sandbox_started=true LEAKED=true stdout="known-secret-payload" stderr=""
```
S13 is the same attack with the denied path shaped as a `.git` DIRECTORY — the §17.3 asset.

**Reading.** For the policy shape `writable_roots: [W]`, `denied_paths: [W/a/secret]`:

1. Session 1 runs `mv W/a W/b` — permitted; it is a write inside a declared writable root.
2. `validate()` still returns CLEAN for the unchanged policy (it inspects only absoluteness,
   filesystem-root-ness, tier, backend, network — never the relationship between the denied
   paths and the writable roots, and never whether the mask target still exists).
3. `wrap()` re-derives the same findings, finds none, and renders `--tmpfs W/a/secret`. The
   target is missing but INSIDE a writable root, so bubblewrap creates it and mounts an empty
   tmpfs over the newly created directory — the sandbox **starts**.
4. The payload now sits at `W/b/secret`, which no mask names. Session 2 **reads it and
   overwrites it** (`host_now=Some("PWNED\n")`).

This falsifies the adapter module doc's own sentence verbatim:

> "Inside a writable root the mount point is creatable, so the mask mounts and the path is an
> empty tmpfs — denied either way; there is no ordering in which a missing mask lets the process
> run."

There IS such an ordering. The doc reasons about the PATH; the asset is the CONTENT, and the
content is movable by the contained process using only permitted writes. It also falsifies the
port's field contract for `denied_paths` — "Paths explicitly denied even if otherwise writable" —
for the nested shape, which is precisely the shape the adapter's own
`live_confinement_denies_a_path_nested_inside_the_writable_root` test exists to cover.

Note the asymmetry that makes this silent rather than loud: the SIBLING shape fails closed (S6,
`Can't mkdir …: Read-only file system` — the sandbox never starts), while the NESTED shape —
the only shape where "denied even if otherwise writable" has content — fails OPEN.

**Reachability bound.** Under the four shipped artifacts and the default anchors
(`$WORKSPACE_ROOT` = `/tmp/gauntlet/workspaces`, `$PROJECT_ROOT` = the repo, `$HOME` =
`/home/…`), the denied paths are SIBLINGS of the writable root, not nested inside it, so the
shipped configuration takes the fail-closed S6 branch and is NOT exploitable as configured. The
defect is in the adapter's guarantee for a supported and documented policy shape, reachable by
any operator whose `workspace.root` encloses a denied path — and the multi-attempt corrective
loop (§11.5) is exactly the multi-session structure the attack needs. I did not find a shipped
config that makes it live; I did not search exhaustively for one.

**Severity: MAJOR.**

---
## A4 — Teeth of the ruler's named contract cases (mutation, one line at a time)

Baseline `sha256(crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs)` =
`447868fbb8c4c8d20ffb1b991872c45173ae6409eef87234001d4e47bfa2e4e0`.
Baseline: `cargo test -p gauntlet-adapter-sandbox-bwrap --lib` → 16 passed.

### M3 — does AK-047's named case `no_unsandboxed_fallback` have teeth on the mechanism it names?

Mutation: in `Sandbox::wrap`, `if !validation.is_clean() {` → `if false {` (one token; the
`policy_findings` re-derivation and the rung check were left intact, so only the is_clean
conjunct is removed).

```
$ cargo test -p gauntlet-adapter-sandbox-bwrap --lib the_shared_contract_suite_is_clean
test tests::the_shared_contract_suite_is_clean ... FAILED
  [sandbox::no_unsandboxed_fallback] … wrap accepted a findings-carrying validation —
  a failed validation must escalate, never authorize execution
```
**RED, attributed to the frozen case name itself.** Restored; sha256 back to
`447868f…`; re-ran → `1 passed`. **Teeth confirmed.**

### M4 — does the mask-after-bind ORDER (the whole nesting guarantee) have semantic teeth?

Mutation: the `denied_paths` loop in `render` swapped ahead of the `writable_roots` loop
(the two loops exchanged, nothing else edited).

```
$ cargo test -p gauntlet-adapter-sandbox-bwrap --lib
failures:
    tests::every_mask_is_rendered_after_the_last_writable_bind      (string shape)
    tests::live_confinement_denies_a_path_nested_inside_the_writable_root  (REAL bwrap read)
    tests::the_shared_contract_suite_is_clean
      [sandbox::a_nested_denial_survives_the_writable_bind] … the denied path … is named at
      word 11, BEFORE the enclosing writable root … at word 14
```
Three independent detectors, one of them a live confinement read, not a rendered string.
Restored; sha256 back to `447868f…`; re-ran → `16 passed`. **Teeth confirmed, semantically.**

---
## A5 — Teeth of AK-044 and AK-046 in the app layer (two more mutations)

Baselines: `sha256(govern/gate.rs)=9c986d1f…`, `sha256(govern/posture.rs)=04fd5d99…`,
`sha256(control/scheduler.rs)=0524c92d…`. Baseline run: `cargo test -p gauntlet-cli --test
p5_hard_gate --test g7_tier_downgrade` → 4 + 3 passed.

### M6 — gate conjunct 1 (AK-044's dispatch-time line)
Mutation: `if facts.tier == BlastRadius::Irreversible {` → `if false {` in
`AuthorityGate::admit`.
```
test irreversible_never_dispatched ... FAILED
  under the conservative posture the gate must refuse an irreversible dispatch on its OWN
  conjunct 1, not on a downstream one: Err(CapabilityMismatch { … tier: Irreversible … })
```
**RED.** Note the assertion is not merely "refused" — it pins WHICH conjunct refuses, so a
downstream conjunct cannot silently carry AK-044. Restored, sha256 verified, re-ran green.

### M7 — the SCHEDULER-level line (AK-044 says "at the scheduler level")
Mutation: `if unit.blast_radius() == BlastRadius::Irreversible {` → `if false {` in
`control/scheduler.rs` eligibility conjunct 3.
```
test irreversible_never_dispatched ... FAILED
  the blocked ticket names the §17.1 first line: run blocked: Unit AK-9301's tier irreversible
  has no concurrency entry — no policy admits it (spec 11.2, 17.3)
```
**RED.** The frozen locator covers BOTH legs the ruler names, not just the gate.
Restored, sha256 verified (`0524c92d…`), re-ran green.

### M8 — `review_lattice_movement`'s agent arm (AK-046)
Mutation: `ChangeOrigin::AgentRole(_) => { "an agent role cannot move …" }` →
`ChangeOrigin::AgentRole(_) => return LatticeMovement::Allowed,`
```
test an_agent_role_cannot_move_a_units_lattice_in_any_direction ... FAILED
  an agent blast decrease must be refused, got Allowed
test a_dispatch_fact_below_the_recorded_facts_is_refused_as_a_downgrade ... FAILED
  a downgraded dispatch is refused: GateAdmission { … tier: Contained … }
```
**RED on two of three legs.** The third leg
(`a_mid_run_tier_downgrade_attempt_never_dispatches_at_the_lowered_tier`) stayed GREEN, so that
leg is carried by the scheduler/selector, not by this function — an honest reading of the
locator's coverage map, not a defect. Restored; `sha256sum -c` on all four baselines → all OK;
re-ran → 3 + 4 passed.

---
## A6 — Anchoring: can an anchored path escape its anchor, or an unanchored one evade bounding?

**Tried.** 16 hostile path entries through the REAL `PolicyFs::policy_for`, plus the
irreversible-resolution legs.

**Scratch attack target (added, then deleted):**
`crates/adapters/gauntlet-adapter-policy-fs/tests/zz_falsifier_r3_anchor.rs`

**Command.**
```
$ cargo test -p gauntlet-adapter-policy-fs --test zz_falsifier_r3_anchor -- --nocapture --test-threads=1
```

**Observed.** REFUSED, each naming the file, the field, the entry and the offense:
`$WORKSPACE_ROOT/../project`; `$WORKSPACE_ROOT/a/../../project`; `/` (bare absolute — "bounded
by nothing"); `$WORKSPACE_ROOT-evil` (byte-prefix sibling — token not at prefix position);
`/x/$WORKSPACE_ROOT`; `$ROOT/x`; empty `writable_roots`; `$HOME/../../etc` in `denied_paths`;
and all three symlink shapes — a link OUT of the anchor, a link to a SIBLING of the anchor, and
a link at an INTERMEDIATE component with a not-yet-existing tail (`$WORKSPACE_ROOT/mid/deeper`,
caught by `existing_canonical`'s deepest-existing-ancestor walk).
ADMITTED and correct: `$WORKSPACE_ROOT`, `$WORKSPACE_ROOT/.`, and the two hostile-TEXT entries
`$WORKSPACE_ROOT/x --share-net` and `$WORKSPACE_ROOT/x\ny` — both admitted as anchored paths and
then rendered as a single shell WORD by the backend (A2's S-series ran real bwrap on such
shapes; the crate's own `a_policy_path_cannot_smuggle_a_backend_flag…` test pins the rendering).

Irreversible legs: `policy_for(Irreversible)` refuses structurally **with a well-formed
`irreversible.json` sitting in the directory** — the file is never read. A `contained.json`
whose body claims `"tier":"irreversible"` is refused on the tier mismatch.

**Reading.** I attacked the anchoring with 16 shapes and failed to break it. That is not a proof
of containment; my search was hand-enumerated, single-process, and did not include a concurrent
attacker racing symlink creation between `policy_for` and `wrap` — the one TOCTOU window I did
not close by experiment (I argue below, without executing it, that a writable root the sandboxed
process cannot reach cannot be swapped by that process; I did not test a *host*-side racer).

---

## A7 — Does `validate()` validate what it claims?

**Tried.** A2's S5, S6 and S10 already answer this, from their own output.

**Observed.** For all three shapes — a denied path that is a symlink, a nonexistent denied path
outside every writable root, a writable root that is a symlink — `validate()` returned **CLEAN**
and `wrap()` returned **Ok**; bubblewrap then ABORTED at process start
(`bwrap: Can't create file at …`, `Can't mkdir …: Read-only file system`,
`Can't bind mount …`). The stderr in each case is a runtime bwrap message, which is only
reachable if `validate` and `wrap` both succeeded (my harness prefixes validate/wrap failures
with `VALIDATE-REFUSED` / `WRAP-REFUSED`; none appeared).

**Reading.** `BwrapSandbox::validate` is a backend PROBE (it spawns `bwrap BASELINE_ARGS true`)
plus static arithmetic over the policy's fields. It never dry-runs the policy's own binds and
masks. §6.2 calls `validate(policy)` a "dry run"; for the policy this is a *lint*, not a dry run.
The consequence is not an unsandboxed execution — every case failed CLOSED, the contained
process never ran — but the escalation lands in the wrong place and at the wrong time: the gate
records an ADMISSION for a policy that cannot start, and the failure surfaces later as a spawn
error rather than as `GateRefusal::ValidationEscalated`. **MINOR.**

---

## A8 — The plan's own "Done when" teeth condition (M9)

Plan Task 14 "Done when": *"deleting the capability check lets a `none`-isolation runner take a
`cross_cutting` unit and fails the test (teeth; AK-044–AK-050)."* Executed literally.

Mutation: `if facts.runner_isolation < required {` → `if false {` in `AuthorityGate::admit`.
```
$ cargo test -p gauntlet-cli --test p5_hard_gate
test a_high_quality_dispatch_violating_one_hard_requirement_is_refused_not_averaged ... FAILED
  the governance event names the capability fact: … PolicyUnresolved …
$ cargo test -p gauntlet-app --lib govern::
test govern::gate::tests::a_none_isolation_runner_is_refused_for_any_tier_above_contained ... FAILED
  a none-isolation runner is refused above contained: GateAdmission { … tier: Local … }
```
**RED in both halves of the task's own gate command.** Restored; `sha256sum -c` → all OK;
re-ran → `4 passed` / `12 passed`. **Teeth confirmed.**

---

## A9 — The task's declared gate command, run as written

```
$ just doctor
just doctor exit=1
```
Plan Task 14's Gate is `cargo nextest run --test p5_hard_gate && cargo nextest run -p
gauntlet-app govern:: && just doctor`. The first two are green (verified above, and via
`cargo test` rather than `nextest`, which is not installed here — a bounded substitution). The
third is **RED on the landed tree** for the reason in A1: plan Task 14 step 3 says the
`irreversible` file is **"absent by design"**, and the landed tree commits it.

---

## A10 — Forging, replaying and downgrading an admission

**Tried.** Read the whole public surface for anything that mints containment authority.

**Observed.**
- `GateAdmission` — public struct, ALL fields private; `Containment` is a private enum whose
  `ValidatedContainment` payload is private. No public constructor, no public mutator, no
  `Default`, no `From`. Not forgeable outside `govern::gate`.
- `PolicyValidation` and `SandboxedCommand` — public structs with ALL fields public. A caller
  CAN mint a clean-looking `PolicyValidation` and a `SandboxedCommand` naming any command
  string. Both adapters defend `wrap` against the first (policy-ref equality, `is_clean`,
  achieved-rung equality, and a re-derivation of the full eligibility findings — M3 proved the
  `is_clean` conjunct has teeth). Nothing defends the second, because nothing CONSUMES a
  `SandboxedCommand` in production (A11).
- `wrap_builder_command` does not bind the admission to the command it wraps, nor re-check the
  admission's unit — an admission earned for unit A wraps any string for any unit at that
  admission's tier. No composed caller exists, so this is latent, not live. **INFORMATIONAL.**
- Downgrade route: `admit`'s conjunct 3 always reviews as `ChangeOrigin::AgentRole(Builder)`, so
  no dispatch can be the operator's editing surface; M8 proved that arm has teeth. The residual
  route is editing the answer key itself — both sides of the comparison then move together and
  the conjunct is vacuous. That is by design (the gate's own doc says the conjunct guards drift
  and forgery between plan assembly and dispatch), and it is AK-043/G10's territory, not this
  gate's. **INFORMATIONAL, bounded.**

---

## A11 — Is `irreversible` reachable, and is the wrap actually in the composed path?

**Tried.** Enumerated every `BlastRadius::Irreversible` site outside tests, and every caller of
`wrap_builder_command`.

**Observed.** Four independent refusals, three of which I mutated:
`control/scheduler.rs:522` (eligibility conjunct 3 — M7 RED), `control/scheduler.rs:632`
(slice-member check), `control/select.rs:392` (selection), `govern/gate.rs` conjunct 1 (M6 RED);
plus `PolicyFs::policy_for` refusing the tier before reading any file (A6), plus
`BwrapSandbox::policy_findings` and `NoneSandbox::policy_findings` flagging the tier, plus
doctor check 5. I found no route through.

`wrap_builder_command`: **every call site is a test.** `crates/gauntlet-app/src/control/
controller.rs::dispatch` calls `gate.admit(...)`, records the admission, and then builds a
`RoleInvocationSpec` carrying only `sandbox_policy: admission.policy_ref()` — a bare
`SandboxPolicyRef`. It never wraps. Doctor reports "runner adapters land with plan Tasks 15–16",
so no builder process is spawned in production at all today.

**Reading.** AK-048's first clause ("no builder command executes before the authority and
containment checks") is delivered in the composed path by evidence ORDER. Its second clause
("Containment wraps the builder session from process start") is delivered as a *callable
surface* and proven at the adapter/contract level and in `p5_hard_gate::
containment_wraps_from_process_start_and_confines_the_session`, but is NOT exercised by the
composed dispatch. Note also that what crosses the port boundary to the runner is a bare policy
REF, which carries no validation — the structural "validation precedes use" chain
(`PolicyValidation` threaded into `wrap`) stops at the app layer and does not reach the runner.
Given that Tasks 15–16 own the process runners this is a scope boundary rather than a defect;
I record it as **MINOR** because plan Task 14 step 4 says "Wrap the builder invocation from
process start" without deferring it, and the composed path does not.

---
## Residual findings, ranked

| # | Severity | Finding | Locus | Executed |
|---|---|---|---|---|
| F1 | **BLOCKING** | The landed tree commits `.gauntlet/policies/irreversible.json` (`{}`), which plan Task 14 step 3 says is "absent by design" and which the Task-14 doctor check declares a configuration error. `gauntlet doctor` exits **1** at check 5, so the task's own declared gate (`just doctor`) is RED on `main` as committed. | `.gauntlet/policies/irreversible.json`; `crates/gauntlet-app/src/doctor/checks.rs:877-1002` | yes |
| F2 | **MAJOR** | A denied path NESTED inside a writable root can be moved out from under its own mask by the contained process using only policy-permitted writes (`mv W/a W/b`). The next `validate`+`wrap` under the IDENTICAL policy still reports CLEAN, the mask target is missing but inside a writable root so bubblewrap creates it and mounts an empty tmpfs, the sandbox STARTS, and the payload is READ and OVERWRITTEN at its new location. Falsifies the adapter's own "there is no ordering in which a missing mask lets the process run" and the port's "denied even if otherwise writable". The SIBLING shape fails closed; only the nested shape — the one where the guarantee has content — fails open. | `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs` `render` + `policy_findings` | yes (S12, S13) |
| F3 | MINOR | `BwrapSandbox::validate` probes the BACKEND (`bwrap BASELINE_ARGS true`) and lints the policy's fields, but never dry-runs the policy's own binds and masks. Policies that hard-abort bubblewrap at process start validate CLEAN and wrap Ok (S5, S6, S10). Fail-closed, but the escalation lands as a spawn error instead of `GateRefusal::ValidationEscalated`. §6.2 calls `validate` a dry run. | same file, `probe_backend`/`validate` | yes |
| F4 | MINOR | AK-049's frozen locator is `tests/e2e/doctor::irreversible_policy_is_error`. No test of that name exists anywhere in the repo, and `tests/e2e/` holds only `.gitkeep`. The behaviour lives in `crates/gauntlet-app/src/doctor/mod.rs::a_present_top_tier_policy_fails_check_five_and_the_run` — an in-crate unit test, not an e2e one. The other four Task-14 `::case` locators (AK-044/047/048/050) all resolve exactly, which is what makes this one's absence legible. | `.wayfinder/answer-key.md` AK-049 vs the tree | yes (search) |
| F5 | MINOR | `AuthorityGate::wrap_builder_command` has NO production caller: `controller::dispatch` admits, records, and then hands the runner only a bare `SandboxPolicyRef`. Plan Task 14 step 4's "wrap the builder invocation from process start" is delivered as a surface, not as a composed behaviour, and the `PolicyValidation`→`wrap` structural chain does not cross the port boundary. | `crates/gauntlet-app/src/control/controller.rs` `dispatch`; `govern/gate.rs:680` | yes (call-site enumeration) |
| F6 | INFORMATIONAL | A nonexistent denied path INSIDE a writable root causes bubblewrap to CREATE that directory on the HOST at wrap time (S7). Benign (it is inside a declared writable root) but undocumented, and it can turn a would-be file path into a directory. | bwrap adapter `render` | yes |
| F7 | INFORMATIONAL | `NoneSandbox` can never be composed into a `GovernDeps`: `with_backend` rejects the `none` token with `NoneBackendWithInstances`, and `none_backend()` carries no instances. The adapter is reachable only from the contract suite. Consistent with §8.6, but it means the shipped `none` posture is an absence of a sandbox, not the `none` ADAPTER. | `govern/gate.rs` `with_backend`; `gauntlet-adapter-sandbox-none` | yes (read) |
| F8 | INFORMATIONAL | `PolicyValidation` and `SandboxedCommand` have fully public fields, so both are forgeable outside the adapters; `wrap_builder_command` binds an admission to neither the unit nor the command. Both `wrap` implementations re-derive full policy eligibility, and nothing consumes a `SandboxedCommand` in production, so this is latent. | `crates/gauntlet-ports/src/sandbox.rs`; `govern/gate.rs` | yes (read + M3) |

---

## Tree restoration

Baseline sha256 captured BEFORE any mutation, and re-verified after every restore:

```
447868fbb8c4c8d20ffb1b991872c45173ae6409eef87234001d4e47bfa2e4e0  crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs
9c986d1f057a1b48416a52be7c46e6bbecb498c8ed951ab374b4565cd58e653d  crates/gauntlet-app/src/govern/gate.rs
04fd5d99394aa2c58ff84d24380f2e5fb060dcc6750230509f38a256660edc90  crates/gauntlet-app/src/govern/posture.rs
2707bd8c80da2c96af6e8f0e15fa78c799e669122ee5cd86c8c5fc17d2be8a82  crates/gauntlet-ports/src/contract/sandbox.rs
0524c92dca8d4141ae8eba72a09cd87d5a4048bbfab51a3503d65e6d46739765  crates/gauntlet-app/src/control/scheduler.rs
```

Final verification:
```
$ sha256sum -c baseline.sha256
crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs: OK
crates/gauntlet-app/src/govern/gate.rs: OK
crates/gauntlet-app/src/govern/posture.rs: OK
crates/gauntlet-ports/src/contract/sandbox.rs: OK
$ sha256sum crates/gauntlet-app/src/control/scheduler.rs
0524c92dca8d4141ae8eba72a09cd87d5a4048bbfab51a3503d65e6d46739765  …/scheduler.rs
```

Five mutations were applied and each restored byte-identically before the next
(M3, M4 → bwrap adapter; M6, M9 → `govern/gate.rs`; M7 → `control/scheduler.rs`;
M8 → `govern/posture.rs`). No mutation was repaired by editing anything else. No assertion
was weakened. Nothing was fixed.

**Scratch attack targets added and DELETED** (both files removed, and both freshly-created
`tests/` directories removed with them — verified by `ls` showing only `Cargo.toml` and `src`):
- `crates/adapters/gauntlet-adapter-sandbox-bwrap/tests/zz_falsifier_r3_mounts.rs`
- `crates/adapters/gauntlet-adapter-policy-fs/tests/zz_falsifier_r3_anchor.rs`

Post-attack green re-runs: `gauntlet-adapter-sandbox-bwrap --lib` 16 passed ·
`gauntlet-adapter-policy-fs --lib` 17 passed · `gauntlet-cli --test p5_hard_gate` 4 passed ·
`gauntlet-cli --test g7_tier_downgrade` 3 passed · `gauntlet-app --lib govern::` 12 passed.

`git status --short` at close:
```
 M .agents/reports/doctor.json
 M .agents/reports/task14/IN-FLIGHT.md
 M .agents/reports/task14/final-confirmation-round3-falsifier.md
 D .jolli/jollimemory/capture-progress/1467f8c132e6ed15ffee6dfed9c6410f261731bf.ndjson
 D .jolli/jollimemory/capture-progress/1fcfdd0682877eba264592b9feb91971c794eec0.ndjson
 D .jolli/jollimemory/capture-progress/207520f2d147963f51bdee493b7f96bb5f57a80c.ndjson
 M .jolli/jollimemory/cursors.json
 M .jolli/jollimemory/debug.log
 M .jolli/jollimemory/discovery-cursors.json
 M .jolli/jollimemory/ingest-cooldown.json
 M .jolli/jollimemory/ingest-runs.json
 M .jolli/jollimemory/sessions.json
?? .jolli/jollimemory/capture-progress/967731d6d0906e26ed42b93e1c470ed45a888aef.ndjson
?? .jolli/jollimemory/capture-progress/d05bc2e60da4b7cf5e20671bee4b0d8987cc1b5b.ndjson
?? .jolli/jollimemory/push-pending.json
```
No file under `crates/`, `.gauntlet/`, `.wayfinder/` or `tests/` is modified.
Two disclosed side effects, neither a source change: `.agents/reports/doctor.json` is
REWRITTEN by every `gauntlet doctor` invocation (it was already modified vs HEAD when this
session opened), and `.agents/reports/task14/final-confirmation-round3-falsifier.md` is this
report. `.agents/reports/task14/IN-FLIGHT.md` shows modified; **I did not write it** — it
changed outside my tool calls and I did not read it.

---

## Bounded claims

- "I attacked X and failed to break it" is never "X is proven". Everything below is the former.
- **Mount semantics.** I executed 13 shapes against real bubblewrap 0.11.1 on this WSL2 host,
  single-user, no concurrent attacker, no seccomp/LSM variation, and no container/VM backend
  (none exists here). Ten shapes held; S12/S13 broke (F2). I did not test: a host-side racer
  swapping a path between `policy_for` and `wrap`; mount propagation from outside the sandbox;
  bind-mount loops; `--proc`/`--dev` interaction with a writable root inside `/proc`;
  non-UTF-8 path bytes; or any kernel other than 6.18.35.2-microsoft-standard-WSL2.
- **Anchoring.** 16 hand-enumerated hostile entries, all refused or correctly admitted. This is
  a hand search, not exhaustive; I did not fuzz.
- **Teeth.** Five single-line mutations, each RED then restored and GREEN. A mutation that does
  not go red proves nothing about the other 99% of the file; I mutated only the five lines the
  ruler's own claims name.
- **Composed path.** The CLI has no `run` subcommand and no runner adapter today (doctor:
  "runner adapters land with plan Tasks 15–16"), so AK-048's "no builder command executes
  before the checks" is currently vacuously true in production — no builder command executes at
  all. My evidence for it is the app-layer ORDER in `controller::dispatch` plus the
  contract-suite case, not an observed process start.
- **Nothing blocked by the environment** except: `cargo nextest` is not installed, so the plan's
  gate commands were run with `cargo test` instead (same test bodies, different runner); and I
  ran no full-workspace mutation suite (charter: ask first, expensive on this host).
- **F2's reachability** is bounded: NOT exploitable through the four shipped policy artifacts
  under the default anchors, because their denied paths are siblings of the writable root and
  the sibling shape fails closed. I did not exhaustively search the configuration space for a
  layout that makes it live.

## Blindness incidents (disclosed)

One repo-wide `grep -rn "irreversible.json" --include=*.md .` printed matching LINES from
`.agents/reports/**` before I had excluded that directory. I saw a handful of one-line
fragments from prior-round reports. I did not open any of those files, and every finding above
was reached from the tree and from executed commands BEFORE that grep ran (F1 was already
established by `cat`, `git ls-files`, `git show HEAD:` and a live `gauntlet doctor` run). All
later greps excluded `.agents/`. `.agents/research/**`, `.agents/CURRENT_STATUS.yml`,
`.agents/lessons/**`, `.agents/prompts/**` and commit messages were not consulted; two commit
subject lines were incidentally visible in a `git log --oneline -1` used to check tracking state
and were not used.

VERDICT: NOT CONFIRM
