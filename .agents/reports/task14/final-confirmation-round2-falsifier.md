# Task 14 — fresh blind confirmation round 2 (falsifier, executing)

Status: COMPLETE. 5 attacks + 6 teeth checks executed; tree restored.

## Surface established from the tree (round 2, independent)
- `crates/gauntlet-ports/src/sandbox.rs` — `Sandbox` port.
- `crates/gauntlet-ports/src/policy_provider.rs` — `PolicyProvider` port.
- `crates/gauntlet-ports/src/contract/sandbox.rs` — shared contract suite:
  `validation_precedes_every_use`, `wrap_demands_its_own_validation`,
  `no_unsandboxed_fallback`, `wraps_from_process_start`,
  `capability_mismatch`, `policy_paths_are_not_shell_words`.
- `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs` — real backend.
- `crates/adapters/gauntlet-adapter-sandbox-none/src/lib.rs` — none backend.
- `crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs` — policy provider.
- `crates/gauntlet-app/src/govern/{gate,posture,mod}.rs` — AuthorityGate.
- `crates/gauntlet-app/src/control/controller.rs` — gate at dispatch.
- `crates/gauntlet-app/src/doctor/checks.rs` — checks 4 and 5.
- `crates/gauntlet-cli/tests/{p5_hard_gate,g7_tier_downgrade}.rs`.
- `.gauntlet/policies/{local,contained,cross_cutting}.json` (no irreversible).

## Attack A1 — denied_paths inside a writable root are silently re-exposed
EXECUTED at the backend level (raw bubblewrap 0.11.1, this host).

`BwrapSandbox::render` emits, in order: baseline `--ro-bind / /`, then the
denied-path masks, then the writable-root `--bind`s. bubblewrap applies mount
operations in argument order, so a `--bind <root> <root>` that comes AFTER a
`--tmpfs <root>/<denied>` re-exposes the masked subtree.

Observed (scratchpad `ordering/ws/secretdir/creds`):

    masks BEFORE binds (the adapter's order):  cat -> "TOP-SECRET", exit=0
    binds BEFORE masks (the correct order):    cat -> No such file, exit=1

The adapter's own doc comment states the correct rule
("a later bind re-exposes what an earlier mask covered, so the order is
load-bearing") and then implements its inverse.


### A1 — CONFIRMED BROKEN through the composed real path
Scratch attack target `crates/gauntlet-cli/tests/zz_falsifier_r2_scratch.rs`
(deleted afterwards) composed the REAL `PolicyFs` + REAL
`ProcessIsolationBackend` + REAL `AuthorityGate`, with the policy artifact

    { "tier":"contained", "backend":"bwrap", "network":"deny_by_default",
      "writable_roots":["$WORKSPACE_ROOT"],
      "denied_paths":["$WORKSPACE_ROOT/.git"] }

Observed:

    A1 validation clean = true
    A1 rendered: /usr/bin/bwrap --ro-bind / / --dev /dev --proc /proc \
      --unshare-net --die-with-parent --tmpfs <ws>/.git --bind <ws> <ws> \
      -- sh -c 'cat <ws>/.git/config'
    A1 read  status=exit 0  stdout="SECRET-GIT-CONFIG"
    A1 write status=exit 0
    A1 host file after write attempt = Ok("PWNED\n")

The gate ADMITTED, the validation reported CLEAN, and the denied path was
both readable and writable from inside the sandbox — the host file was
overwritten. `denied_paths` is documented in the port as "paths explicitly
denied EVEN IF otherwise writable"; the nested case is the only case in
which that phrase has content, and it is exactly the case that fails.
No existing test covers it: every denied path in
`gauntlet-adapter-sandbox-bwrap` tests and in `p5_hard_gate` is a SIBLING of
the writable root, never nested inside one.

## Attack A2 — anchored path escape (policy provider)
EXECUTED. Refused: `$WORKSPACE_ROOT/../outside`,
`$WORKSPACE_ROOT/a/b/../../../outside`, `$WORKSPACE_ROOT/link/loot` and
`$WORKSPACE_ROOT/link` (symlink out of anchor), `$WORKSPACE_ROOTevil`
(component-wise prefix, not bytes), `/tmp/$HOME/x` (token off prefix
position), `$WORKSPACE_ROOT//../outside`, `$WORKSPACE_ROOT/./../outside`.
Accepted (by documented design): a not-yet-existing path inside the anchor;
an UNANCHORED absolute path — including `"/"` — see finding F2.
I attacked the anchoring and failed to break it.

## Attack A3 — `irreversible` reachability below the scheduler
EXECUTED. `PolicyFs::policy_for(Irreversible)` -> `NoPolicyForTier` even with
`irreversible.json` present on disk. `BwrapSandbox::validate` on a hand-built
irreversible policy -> `ValidationFailed`. `BwrapSandbox::wrap` with a
HAND-FORGED clean `PolicyValidation` -> `WrapFailed` (wrap re-derives
findings). I attacked three routes and failed to reach an irreversible wrap.

## Attack A4 — policy artifact string -> backend command structure
EXECUTED, 10 payloads: `--share-net` after a space, `-- sh -c id`, `;`,
`$( )`, backticks, newline, embedded single quotes, RTL unicode override,
a 5000-char path, tab. In every case the bare token never appeared as its
own shell word, no `/tmp/pwn-*` marker was created, and bwrap fail-closed
("Can't find source path ..."). I attacked the quoting and failed to break it.

## Teeth verification (sha256 -> mutate -> RED -> restore -> sha256 -> GREEN)
Baseline digests (recorded before any mutation):

    9c986d1f057a1b48416a52be7c46e6bbecb498c8ed951ab374b4565cd58e653d  crates/gauntlet-app/src/govern/gate.rs
    04fd5d99394aa2c58ff84d24380f2e5fb060dcc6750230509f38a256660edc90  crates/gauntlet-app/src/govern/posture.rs
    0e6bab501c37d445ffc87095d1d56866163f2a46c62bccd7f470ef7df9e508da  crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs
    bcc3cc28499bb61c5512a2bf3c864261e3d88769b34848c7f00d62c234498cf4  crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs
    2452d5793643bba8bc5d35a6f5b08c84b1f1f11f417550d7404e2fdf1a54f75d  crates/gauntlet-app/src/doctor/checks.rs

**T1 — gate conjunct 1 (irreversible), gate.rs.**
`if facts.tier == BlastRadius::Irreversible` -> `if false && ...`.
- `p5_hard_gate::irreversible_never_dispatched` **STAYED GREEN** (see F3).
- `govern::gate::tests::irreversible_is_refused_before_every_other_conjunct`
  went RED (`assertion failed: matches!(refusal, GateRefusal::Irreversible{..})`).
Restored; sha256 back to `9c986d1f…`; both green again.

**T2 — gate conjunct 4 (runner isolation), gate.rs.**
`if facts.runner_isolation < required` -> `if false && ...`.
RED on `govern::gate::tests::a_none_isolation_runner_is_refused_for_any_tier_above_contained`
AND on `p5_hard_gate::a_high_quality_dispatch_violating_one_hard_requirement_is_refused_not_averaged`
— the plan's own declared teeth ("deleting the capability check lets a
`none`-isolation runner take a `cross_cutting` unit and fails the test").
Restored; sha256 `9c986d1f…`; 7/7 green.

**T3 — bwrap `wrap` clean-validation gate.**
`if !validation.is_clean()` -> `if false && …`.
RED on `tests::wrap_gates_on_its_own_clean_process_rung_validation` and on
`tests::the_shared_contract_suite_is_clean` (case `no_unsandboxed_fallback`,
AK-047). Restored; sha256 `0e6bab50…`; 13/13 green.

**T4 — bwrap `render` process-start.**
First word `self.binary…` -> `["env", self.binary…]` (a non-sandbox process
first). RED on `the_rendered_command_starts_with_the_backend_and_carries_every_root`
and on the shared suite (case `wraps_from_process_start`, AK-048).
Restored; sha256 `0e6bab50…`; 13/13 green.

**T5 — posture `review_lattice_movement`.**
`if blast_movement.is_none() && authority_movement.is_none()` ->
`if true || (…)`. RED on `g7_tier_downgrade::a_dispatch_fact_below_the_recorded_facts_is_refused_as_a_downgrade`
and `g7_tier_downgrade::an_agent_role_cannot_move_a_units_lattice_in_any_direction`;
`a_mid_run_tier_downgrade_attempt_never_dispatches_at_the_lowered_tier` stayed
green (it is carried by the scheduler/answer-key leg, not the lattice review).
Restored; sha256 `04fd5d99…`; 3/3 green.

**T6 — AK-049 live, no source mutation.**
Wrote `.gauntlet/policies/irreversible.json`; `gauntlet doctor` -> check 5
`fail`: "a policy file exists for the top tier ('irreversible.json' …) — its
presence is a configuration error (AK-049, spec 17.3)", process exit 1.
Casing/extension variants `Irreversible.json`, `IRREVERSIBLE.yaml`, and the
extensionless `irreversible` are all caught (exit 1). `irreversible.json.bak`
and a nested `sub/irreversible.json` are NOT caught (see F5 — neither is a
file `PolicyFs` would ever resolve). All files removed; `.gauntlet/policies`
back to `.gitkeep, contained.json, cross_cutting.json, local.json`;
`.agents/reports/doctor.json` restored byte-identically to
`9486ec5ad268efcc8edbcb2e815caf2e0a9809b3df2da94f252bc936b7fe7f9e`.

## Other executed probes
- Network posture under the rendered baseline: loopback only, DNS blocked
  (`--unshare-net` is real, not decorative).
- A mask target that does not exist fails backend STARTUP
  ("Can't mkdir … Read-only file system", exit 1) — fail-closed as documented.
- A denied FILE nested inside a writable root is re-exposed the same way a
  directory is: `--ro-bind /dev/null <file> … --bind <root> <root>` still
  prints `FILE-SECRET` (A1 generalises to both mask shapes).
- `GateAdmission` is exported but has only private fields and no public
  constructor; `Containment`/`ValidatedContainment` are module-private. I
  could not mint or mutate an admission from outside `govern::gate`.
- Wide test surface re-run clean: `gauntlet-ports` + `gauntlet-arch-tests`
  324/324; sandbox/policy adapters 41/41; `p5_hard_gate` + `g7_tier_downgrade`
  7/7; `gauntlet-app govern::` 12/12.

## Attack A5 — forging / replaying an admission
EXECUTED (scratch target `crates/gauntlet-cli/tests/zz_falsifier_r2_forge.rs`,
deleted afterwards).
- `GateAdmission { unit, tier, … }` from outside `govern::gate` →
  `error: cannot construct 'GateAdmission' with struct literal syntax due to
  private fields`. I could not mint an admission.
- BUT `SandboxedCommand { command: "rm -rf /", policy, validation }` with a
  hand-built `PolicyValidation { isolation: Vm, findings: [] }` COMPILES and
  runs — printed `A5(b) forged SandboxedCommand compiles and claims rung vm`.
- `BuilderCommand::Unwrapped { command: "rm -rf /" }` is a public variant and
  was minted outside the gate.
See finding F4.

## Residual findings, ranked

| id | severity | claim | locus |
|----|----------|-------|-------|
| F1 | MAJOR | A `denied_paths` entry nested inside a `writable_roots` entry is silently re-exposed read **and write** inside the sandbox; validation still reports CLEAN. The port documents `denied_paths` as "paths explicitly denied **even if otherwise writable**" — the nested case is the only case where that phrase has content, and it is the case that fails. `render` pushes the masks BEFORE the writable `--bind`s; bubblewrap applies mounts in argument order, so the later bind covers the earlier mask. The adapter's own doc comment states the correct rule and implements its inverse. Both mask shapes fail (`--tmpfs` for a directory, `--ro-bind /dev/null` for a file). Executed end-to-end through PolicyFs + AuthorityGate + real bubblewrap, with the host file overwritten (`Ok("PWNED\n")`). | `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs` — `BwrapSandbox::render` (denied-path loop precedes the writable-root loop) |
| F2 | MINOR | `PolicyFs` carries an UNANCHORED absolute path verbatim with no upper bound, and nothing downstream bounds a writable root: `writable_roots: ["/"]` resolves, validates clean, and renders `--bind / /` — the host root filesystem writable under a validation that reports no findings. (The resolution was executed; the `--bind / /` consequence was read from `render`, not separately executed.) | `crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs` — `resolve_anchored_path`, unanchored-absolute branch; no writable-root bound in `BwrapSandbox::policy_findings` |
| F3 | MINOR | AK-044's frozen evidence locator has no teeth on the gate's own conjunct 1: with `if facts.tier == BlastRadius::Irreversible` disabled, `p5_hard_gate::irreversible_never_dispatched` stayed GREEN (the scheduler's §11.2 eligibility carries it alone). The dispatch-time second line is pinned only by an in-crate unit test, not by the frozen locator. | `crates/gauntlet-cli/tests/p5_hard_gate.rs::irreversible_never_dispatched` vs `crates/gauntlet-app/src/govern/gate.rs` conjunct 1 |
| F4 | MINOR | The earned-type discipline stops at the app boundary. `SandboxedCommand` and `PolicyValidation` have all-public fields, so a `SandboxedCommand` carrying an UNWRAPPED command while claiming `isolation: Vm` compiles; `BuilderCommand::Unwrapped` is a public variant mintable outside the gate. "The wrapped command carries its validation so the executing layer can prove validation preceded use" is therefore not provable at the port boundary the Task-15/16 runners will consume. | `crates/gauntlet-ports/src/sandbox.rs` (`SandboxedCommand`, `PolicyValidation`); `crates/gauntlet-app/src/govern/gate.rs` (`BuilderCommand::Unwrapped`) |
| F5 | INFORMATIONAL | The doctor's forbidden-tier scan is stem-exact and non-recursive: `irreversible.json.bak` and `sub/irreversible.json` are not flagged (exit 0). Neither is a file `PolicyFs` would resolve, so no irreversible policy becomes reachable — the gap is in the configuration-hygiene report, not in containment. | `crates/gauntlet-app/src/doctor/checks.rs` — `forbidden_entries_in` |
| F6 | INFORMATIONAL | `wrap_builder_command` has no composed consumer. `RoleInvocationSpec` carries no command; the controller passes only `sandbox_policy: admission.policy_ref()` to `runner.invoke`. AK-048 is delivered at the gate/adapter level and executed live in `p5_hard_gate`, but nothing in the landed tree forces a runner adapter to execute the wrapped command. Relatedly, `GovernDeps` has no production composition — the CLI builds no `RunController`; every `ControllerDeps.govern` in the tree is a test's `GovernDeps::none_workspace()`. Honest staging for Tasks 15-16, recorded so it is not mistaken for composed enforcement. | `crates/gauntlet-app/src/control/controller.rs::dispatch`; `crates/gauntlet-cli/src/main.rs` |
| F7 | INFORMATIONAL | The gate never cross-checks `deps.backend_token()` against `sandbox.backend_identity()`; a posture composed with a mismatched token still functions and its refusal text names an unverified token. The dangerous shape (`is_none_posture()` true while a real sandbox is held) IS closed by `GovernDeps::with_backend`'s `NoneBackendWithInstances`. | `crates/gauntlet-app/src/govern/gate.rs` — `GovernDeps::with_backend` / `is_none_posture` |

## Tree restoration
Every mutation restored byte-identically; every scratch attack target deleted.

    $ sha256sum -c baseline.sha256
    crates/gauntlet-app/src/govern/gate.rs: OK
    crates/gauntlet-app/src/govern/posture.rs: OK
    crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs: OK
    crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs: OK
    crates/gauntlet-app/src/doctor/checks.rs: OK
    $ sha256sum .agents/reports/doctor.json
    9486ec5ad268efcc8edbcb2e815caf2e0a9809b3df2da94f252bc936b7fe7f9e  .agents/reports/doctor.json
    $ ./scripts/verify-ruler.sh
    PASS — answer key and manifest digests match the frozen lock

Scratch attack targets I ADDED and then DELETED:
`crates/gauntlet-cli/tests/zz_falsifier_r2_scratch.rs`,
`crates/gauntlet-cli/tests/zz_falsifier_r2_forge.rs`.
Temporary policy files I created and removed:
`.gauntlet/policies/{irreversible.json, Irreversible.json, IRREVERSIBLE.yaml,
irreversible, irreversible.json.bak, sub/irreversible.json}` and the `sub/`
directory. `.gauntlet/policies` now holds exactly
`.gitkeep, contained.json, cross_cutting.json, local.json`.

    $ git status --short
     M .agents/reports/doctor.json
     M .agents/reports/reference-candidates.md
     M .jolli/jollimemory/cursors.json
     M .jolli/jollimemory/debug.log
     M .jolli/jollimemory/discovery-cursors.json
     M .jolli/jollimemory/ingest-cooldown.json
     M .jolli/jollimemory/ingest-runs.json
     M .jolli/jollimemory/sessions.json
     M Cargo.lock
     M Cargo.toml
     M crates/gauntlet-app/src/control/controller.rs
     M crates/gauntlet-app/src/doctor/checks.rs
     M crates/gauntlet-app/src/doctor/mod.rs
     M crates/gauntlet-app/src/lib.rs
     M crates/gauntlet-arch-tests/src/denylist.rs
     M crates/gauntlet-cli/Cargo.toml
     M crates/gauntlet-cli/src/main.rs
     M crates/gauntlet-cli/tests/g10_builder_writes_answer_key.rs
     M crates/gauntlet-cli/tests/p11_critic_is_not_verifier.rs
     M crates/gauntlet-cli/tests/p4_corrective_loop.rs
     M crates/gauntlet-cli/tests/p6_budget_control.rs
     M crates/gauntlet-cli/tests/p7_recovery.rs
     M crates/gauntlet-ports/src/contract/sandbox.rs
     M crates/gauntlet-ports/src/contract/stubs.rs
     M crates/gauntlet-ports/src/contract/suite_runs.rs
    ?? .agents/reports/task14/
    ?? .agents/research/preregistrations/task-14.prereg.sha256
    ?? .agents/research/preregistrations/task-14.prereg.yaml
    ?? .gauntlet/policies/contained.json
    ?? .gauntlet/policies/cross_cutting.json
    ?? .gauntlet/policies/local.json
    ?? .jolli/jollimemory/capture-progress/207520f2d147963f51bdee493b7f96bb5f57a80c.ndjson
    ?? crates/adapters/gauntlet-adapter-policy-fs/
    ?? crates/adapters/gauntlet-adapter-sandbox-bwrap/
    ?? crates/adapters/gauntlet-adapter-sandbox-none/
    ?? crates/gauntlet-app/src/govern/
    ?? crates/gauntlet-cli/tests/g7_tier_downgrade.rs
    ?? crates/gauntlet-cli/tests/p5_hard_gate.rs

Four extra `M` lines relative to the session-start snapshot
(`reference-candidates.md`, `cursors.json`, `discovery-cursors.json`,
`sessions.json`) are not mine: their mtimes are 13:08–13:10 and 15:26 from a
background memory daemon; my first write was 15:25 and I touched none of
them. The only file I added to the tree is this report.

## Bounded claims
- **Scope.** I read only the surfaces named in my charter plus the tree. I did
  not read `.agents/reports/**` (other than writing this file),
  `.agents/research/**`, `CURRENT_STATUS.yml`, lessons, prompts, or commit
  messages. Round 1's findings are unknown to me; overlap is coincidence.
- **Threat model exercised.** Policy-artifact text as an injection channel;
  anchored-path escape (`..`, symlink, non-existent, prefix-sharing sibling,
  token off prefix position); admission forgery and cross-posture
  presentation; `irreversible` reachability at provider / validate / wrap /
  doctor; runner-isolation downgrade; lattice movement by an agent role. NOT
  exercised: concurrency and TOCTOU between policy resolution and process
  start (the anchoring checks are resolution-time only); kernel-level
  bubblewrap escapes; a hostile runner adapter (none exists yet); mutation
  testing beyond the six hand-built inverting mutations below.
- **Procedure.** Six teeth checks (T1–T6), each sha256 → minimal inverting
  mutation → named test → observed RED (except T1, where I observed GREEN and
  report that as F3) → byte-identical restore → sha256 re-verified → re-run →
  GREEN. Two scratch attack binaries, both deleted.
- **Honest wording.** For A2 (anchoring), A3 (irreversible), A4 (shell
  injection), and the `GateAdmission` half of A5: **I attacked these and
  failed to break them.** That is not the same as their being proven. My
  attacks were finite and hand-built; a wider generator or a different host
  (a bubblewrap other than 0.11.1, a non-WSL2 kernel) could differ.
- **Environment.** Every executed attack ran; none was blocked by the
  environment. bubblewrap 0.11.1 present at `/usr/bin/bwrap`; all live legs
  ran rather than skipping.
- **What A1 does NOT show.** With the SHIPPED artifacts and the default
  configuration (`workspace.root` = `/tmp/gauntlet/workspaces`,
  `$PROJECT_ROOT/.git` and `$HOME/.ssh|.aws|.gnupg` denied), no denied path is
  nested inside a writable root, so the defect is dormant there. It becomes
  live the moment an operator points `workspace.root` at a tree containing a
  denied path, or a policy artifact says "this root is writable except X" —
  the ordinary way to express the spec's own `local`-tier rule.

VERDICT: NOT CONFIRM
