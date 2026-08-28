# Task 14 — Fresh blind confirmation round 3 (EXECUTING falsifier)

Blind: `.agents/reports/**`, `.agents/research/**`, `.agents/CURRENT_STATUS.yml`,
`.agents/lessons/**`, `.agents/prompts/**`, and git commit messages were NOT read.
Ruler read: `.wayfinder/answer-key.md` AK-044..AK-050. Spec §6.1/6.2, §8.5/8.6, §11.2,
§17.1-17.5, §21.3, §22.2, §25. Plan Task 14 section only.

Host: WSL2, bubblewrap 0.11.1 at `/usr/bin/bwrap`. `cargo` jobs=1 (repo `.cargo/config.toml`).

---

## Surface under attack

- `crates/gauntlet-ports/src/contract/sandbox.rs` (shared `Sandbox` contract suite)
- `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs` (real backend)
- `crates/adapters/gauntlet-adapter-sandbox-none/src/lib.rs`
- `crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs` (`PolicyProvider`)
- `crates/gauntlet-app/src/govern/{gate,posture}.rs`
- `crates/gauntlet-app/src/doctor/checks.rs` check 5
- `crates/gauntlet-cli/tests/p5_hard_gate.rs`, `crates/gauntlet-cli/tests/g7_tier_downgrade.rs`
- `.gauntlet/policies/{contained,local,cross_cutting}.json`

---

## A1 — EXECUTED — the shipped `local`/`cross_cutting` policies cannot start the backend on a host that lacks any denied credential path

**Claim attacked:** AK-047 "Sandbox policy validation precedes execution". The port doc
calls `validate` a *dry run*; the adapter doc calls a missing mask target "fail-closed:
the contained process never runs with a mask missing."

**Mechanism.** `BwrapSandbox::render` (adapter L253-268) emits `--tmpfs <denied>` for every
denied path that is not an existing *file*. The shipped `local.json` and
`cross_cutting.json` deny `$HOME/.ssh`, `$HOME/.aws`, `$HOME/.gnupg`,
`$PROJECT_ROOT/.git`. `BwrapSandbox::policy_findings` (L166-213) never looks at whether a
mask target exists, so `validate()` returns **clean**, and `wrap()` renders successfully.

**Executed evidence (real bwrap, this host):**

```
$ ls -d ~/.ssh ~/.aws ~/.gnupg
/home/sprime01/.ssh
ls: cannot access '/home/sprime01/.aws': No such file or directory
ls: cannot access '/home/sprime01/.gnupg': No such file or directory
```

Exact render of the shipped `local.json` under the anchors the shipped-artifact test itself
uses (`workspace_root = <repo>/target/workspaces`, `project_root = <repo>`, `home = $HOME`):

```
$ /usr/bin/bwrap --ro-bind / / --dev /dev --proc /proc --unshare-net --die-with-parent \
    --bind <repo>/target/workspaces <repo>/target/workspaces \
    --tmpfs <repo>/.git --tmpfs /home/sprime01/.ssh \
    --tmpfs /home/sprime01/.aws --tmpfs /home/sprime01/.gnupg -- sh -c 'echo RAN-INSIDE'
bwrap: Can't mkdir /home/sprime01/.aws: Read-only file system
exit=1
```

Dropping only the two *missing* denies makes the same command start and run:

```
$ /usr/bin/bwrap ... --tmpfs <repo>/.git --tmpfs /home/sprime01/.ssh -- sh -c 'echo RAN-INSIDE; ...'
RAN-INSIDE
exit=0
```

Isolated minimal repro (`--tmpfs` on a nonexistent path outside every writable root):

```
bwrap: Can't mkdir /home/sprime01/.aws: Read-only file system   # exit 1
```

vs. the same shape *inside* a writable root, which mounts fine (exit 0).

**Consequence.** On this host the `local` and `cross_cutting` tiers are **undispatchable**:
the gate admits (validation clean), the wrap renders, and the builder process never starts.
The direction of failure is safe (no unsandboxed execution, AK-047's safety half survives),
but the *dry run* does not detect a policy whose wrap cannot execute — which is precisely
what a dry run is for (§6.2 `validate(policy) -> PolicyValidation (dry run)`; §8.5 preflight
is supposed to catch config problems "before any run starts").

**Why no test catches it.** No test validates + wraps + **executes** the shipped artifacts.
`p5_hard_gate.rs:1176-1207` and `policy-fs` `the_shipped_repo_policy_artifacts_resolve_for_every_dispatchable_tier`
only *decode* them. Every executing test (`live_confinement_*`, p5's
`containment_wraps_from_process_start_and_confines_the_session`) builds its own scratch
policy with denied paths that **exist**, and a scratch `$HOME`. The gap is exactly the
difference between a fixture and the deliverable.

Severity: **MAJOR** (functional, not a containment breach — fail-closed).

---

## A2 — EXECUTED — the tmpfs/`/dev/null` mask survives an in-sandbox unmount attempt

Attempted to undo a nested denial from inside the sandbox.

```
$ bwrap <baseline> --bind <ws> <ws> --tmpfs <ws>/.git --ro-bind /dev/null <ws>/nested-secret.txt \
    -- sh -c 'id; umount <ws>/.git; cat <ws>/.git/config'
uid=1000(sprime01) ...
umount: ...: must be superuser to unmount.   umount-exit=32
cat: ...: No such file or directory
```

Nested user+mount namespace escalation also fails (the propagated mounts are locked):

```
$ ... -- sh -c "unshare -U -m -r sh -c 'umount <ws>/.git; cat <ws>/.git/config'"
umount: ...: not mounted.
cat: ...: No such file or directory
```

Capabilities inside are empty and `NoNewPrivs: 1`:

```
CapInh/CapPrm/CapEff/CapBnd: 0000000000000000    NoNewPrivs: 1
```

**Attack failed.** Bounded: single-host (WSL2 kernel 6.18, bwrap 0.11.1), `umount(2)` and
nested-userns routes only; I did not attempt `pivot_root`, `open_by_handle_at`, ptrace of a
sibling, or kernel-level namespace escapes.

---

*(work in progress — further attacks below as they run)*
