# Task 14 — Correction round 3

Fixes for the two residual defects the round-3 fresh blind executing falsifier
returned NOT CONFIRM on, after executing 40 attacks. The six corrections landed
in rounds 1 and 2 (COR-1..COR-6) were re-confirmed by that round and are NOT
revisited here.

Round-3 F4/F5/F6/F7/F8 are dispositioned by the orchestrator and are out of
scope for this report.

Build discipline: sole compiler, sequential package-scoped runs, `jobs=1`.

Host: bubblewrap 0.11.1 at `/usr/bin/bwrap`, WSL2.

---

## COR-7 (round-3 F2, MAJOR) — a nested denial was defeated by a rename, and the next wrap failed OPEN

**Status: FIXED**

### The defect, reproduced first

The falsifier's attack, replayed on this host before any code change. A denied
path NESTED inside a writable root, with one ordinary directory between them.

Session 1 — the contained process performs a write the policy EXPLICITLY
PERMITS (a rename inside a declared writable root):

```
$ bwrap --ro-bind / / --dev /dev --proc /proc --unshare-net --die-with-parent \
        --bind $W $W --tmpfs $W/proj/.git \
        -- sh -c 'mv $W/proj $W/proj2; echo rc=$?; ls -a $W/proj2'
rc=0
.  ..  .git  secret.txt        <-- the denied subtree moved, intact, with the sandbox
```

Session 2 — the IDENTICAL policy, re-validated and re-wrapped. `validate()`
returned CLEAN (it never inspected the denied-path/writable-root relationship,
nor whether the mask target existed), `wrap()` re-derived the same empty
findings, and the mask target `$W/proj/.git` was now missing — but it sat
INSIDE a writable root, so bubblewrap created the mount point and mounted an
empty tmpfs over it. The sandbox STARTED:

```
$ bwrap ... --bind $W $W --tmpfs $W/proj/.git \
        -- sh -c 'echo SANDBOX_STARTED; cat $W/proj2/.git/config;
                  echo PWNED > $W/proj2/.git/config'
SANDBOX_STARTED
known-secret-payload      <-- the denied payload READ
$ cat $W/proj2/.git/config
PWNED                     <-- the denied payload OVERWRITTEN on the host
```

Reproduced for a denied FILE and for a denied `.git` DIRECTORY. The SIBLING
case (a denied path outside every writable root) fails CLOSED, as claimed:

```
bwrap: Can't mkdir $S/gone: Read-only file system     (rc=1, nothing runs)
```

What it falsified, verbatim: the module doc's "there is no ordering in which a
missing mask lets the process run", and the port's `denied_paths` contract,
"denied even if otherwise writable".

### Direction chosen, and why

The work order offered (a) enforce the guarantee so it survives a rename, or
(b) refuse such a policy. **The fix does both, split along the line where the
guarantee is actually deliverable** — and that line was established by
measurement, not by assertion.

A mask is a MOUNT POINT, and the kernel refuses to rename, remove, or clobber
a mount point. Probed on this host, inside the writable root, against a mask
that is a DIRECT CHILD of that root:

```
mv $W/secret $W/moved   -> Device or resource busy
rm -f $W/secret         -> Device or resource busy
mv $W/other $W/secret   -> Device or resource busy   (clobber)
ln $W/secret $W/hard    -> Invalid cross-device link (EXDEV)
mv $W/gitd $W/gitd2     -> Device or resource busy   (dir mask)
rmdir $W/gitd           -> Device or resource busy
```

And a writable root that is itself bound inside another writable root is a
mount point too, so it is equally unrenameable:

```
roots [$W, $W/sub], denied $W/sub/x:  mv $W/sub $W/sub2 -> Device or resource busy
```

So the guarantee IS deliverable — exactly when every directory between the
denied path and the writable root enclosing it is itself a mount point. In a
rendered command the mount points are precisely the bound writable roots and
the masks. That yields the rule:

> An ancestor of a denied path is RELOCATABLE when it lies strictly inside
> some writable root (which makes it writable, hence renameable) without
> being a declared writable root itself. A policy with such an ancestor is
> refused.

The two admitted shapes — a denial outside every writable root (held by the
read-only baseline) and a denial whose every intervening directory is a bound
root — are enforced. Everything else is refused with a typed validation
finding naming the offending directory, per "do not fake containment". No
guarantee silently degrades.

Note what this does NOT do: it does not reject nesting. `$W/.git` under
writable root `$W` — the ordinary shape, and the one COR-4 exists to protect —
stays admitted and stays proven.

### The fix

`crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs`:

- new `relocatable_ancestor(denied, writable_roots) -> Option<PathBuf>` — walks
  the denied path's strict ancestors nearest-first and returns the innermost
  one that is inside a writable root without being one. Comparison is over
  `normalize_lexically`, a new helper that resolves `.` and `..` without a
  filesystem lookup, so `$W/x/../proj/.git` cannot dodge the rule by spelling.
- `policy_findings` gained the check, so BOTH `validate` and `wrap` enforce it
  (`wrap` re-derives findings — a forged clean validation cannot bootstrap it).
  The finding names the denied path, the intervening directory, and the two
  ways to fix the policy.
- doc comments corrected. The module doc's "there is no ordering in which a
  missing mask lets the process run" is gone; the module doc and `render`'s doc
  now state the anchoring rule alongside the ordering rule and say plainly why
  a missing mask target inside a writable root is safe *only* under it. The
  lexical comparison's blindness to symlinks is stated, not hidden.

The rule is lexical and cheap: no new filesystem IO on the validate path.

### Regression home

Shared port contract suite,
`crates/gauntlet-ports/src/contract/sandbox.rs` — new case
`a_relocatable_nested_denial_is_refused`, following
`policy_paths_are_not_shell_words` and
`a_nested_denial_survives_the_writable_bind`, and added to `sandbox_suite`. It
states the DECISION, not a mechanism: a backend may hold the denial by an
anchor of its own or refuse the policy, but it may not validate clean and then
serve it as a bare path mask. Read off the rendered command's shell WORDS, so
an identity wrap at the none rung is vacuously outside it — the same shape the
neighbouring cases use.

Teeth: new broken stub `PathMaskOnlySandbox` / `SandboxBrokenRelocatableDenial`
in `stubs.rs`. It is the mask-order fake with its one defect FIXED — correct
quoting, masks after binds — leaving exactly one defect: it accepts every
nested denial and expresses it as nothing but a path mask. Caught by
`sandbox_relocatable_denial_broken_is_caught` in `suite_runs.rs`, asserted on
the intended case by name.

LIVE bwrap leg:
`tests::a_nested_denial_survives_a_rename_between_sessions` in the adapter. Two
sessions of the identical policy with the permitted write in between. Half one
(admitted shape): all four relocation attacks against a direct-child mask are
executed and must fail; the host payloads must survive; session two must still
not read or overwrite them. Half two (refused shape): the relocatable policy is
refused at `validate` AND at `wrap` under a forged clean validation, with the
refusal naming the intervening directory; and a policy whose intervening
directory IS a declared writable root is still admitted.

Lexical teeth: `tests::only_a_relocatable_ancestor_is_a_finding` pins the rule
over paths alone, including the dotted-spelling dodge in both directions and
the nested-writable-root admission.

### RED before GREEN

RED, before the fix:

```
FAIL tests::a_nested_denial_survives_a_rename_between_sessions
  a relocatable nested denial must be refused, got Ok(PolicyValidation { ... findings: [] })
FAIL tests::the_shared_contract_suite_is_clean
  [sandbox::a_relocatable_nested_denial_is_refused] ... validated CLEAN and wrapped
  into a command naming the root as a writable bind and the denied path as a bare
  path mask: "/usr/bin/bwrap ... --bind /suite-writable-root /suite-writable-root
  --tmpfs /suite-writable-root/suite-relocatable-parent/suite-nested-denied-path ..."
```

The teeth discriminated on the first run: `sandbox_green_is_clean` passed and
`sandbox_relocatable_denial_broken_is_caught` caught the new stub, while the
real adapter failed — green, broken, and the adapter each landing where they
should.

GREEN, after the fix:

- `cargo nextest run -p gauntlet-adapter-sandbox-bwrap` — **18/18 passed**
- `cargo nextest run -p gauntlet-ports` — **111/111 passed**
- `cargo nextest run -p gauntlet-adapter-sandbox-none -p gauntlet-adapter-policy-fs`
  — **29/29 passed**
- `cargo nextest run -p gauntlet-cli --test p5_hard_gate --test g7_tier_downgrade`
  — **7/7 passed**

### COR-4 not regressed, sibling case not broken

`every_mask_is_rendered_after_the_last_writable_bind` and
`live_confinement_denies_a_path_nested_inside_the_writable_root` (the COR-4
ordering fix and its live confinement legs) both still pass, unmodified. The
sibling case — which correctly fails closed at backend startup — is untouched:
its ancestors lie outside every writable root, so `relocatable_ancestor`
returns `None` and no finding is raised.

---

## COR-8 (round-3 F3 / round-1 F3, MINOR — flagged by two independent blind rounds) — `validate()` was not a dry run of the POLICY

**Status: FIXED**

### The defect, reproduced first

`validate()` probed the BACKEND (`bwrap BASELINE_ARGS true`) and linted the
policy's FIELDS, but never exercised the policy's own binds and masks. Spec
§6.2 calls `validate` a dry run. Three shapes validated CLEAN and wrapped `Ok`,
then hard-aborted bubblewrap at process start — fail-closed, but surfacing as a
raw spawn error AFTER the gate had durably recorded an admission, instead of as
`GateRefusal::ValidationEscalated`. All three reproduced on this host first:

```
(1) denied path IS a symlink, inside the writable root
    bwrap: Can't create file at $W/link-denied: No such file or directory   rc=1
(2) nonexistent denied path OUTSIDE every writable root
    bwrap: Can't mkdir $S/nope: Read-only file system                       rc=1
(3) writable root IS a symlink
    bwrap: Can't bind mount ... on ...: No such file or directory           rc=1
```

### The fix

`crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs`:

- `probe_backend`'s bounded spawn/wait/kill loop is factored into
  `run_bounded(args, deadline) -> BoundedRun`, which now CAPTURES stderr. An IO
  failure (unspawnable binary, broken wait, expired deadline) stays
  `BackendUnavailable` whatever the caller was asking — it says nothing about
  the policy. What the backend DECIDED comes back for the caller to interpret,
  so the same machinery serves a backend fact and a policy fact.
- new `dry_run_policy(policy, deadline) -> Option<String>` — spawns the backend
  on the EXACT argument vector `render` will emit, with the same `true` payload
  idiom. A nonzero exit or a signal death becomes a validation finding carrying
  bubblewrap's own complaint.
- `render`'s bind/mask assembly is factored into `policy_args(policy)`, shared
  by `render` and `dry_run_policy`. A dry run of a shape the wrap does not emit
  proves nothing about the wrap, so they cannot drift.
- `validate` now runs, in order: the backend probe (unchanged), the field lints,
  and — only if the lints are clean — the dry run. Lints first because they are
  free and dry-running an already-ineligible policy spends a spawn to learn
  nothing.

Cost: one extra `true` under the real mounts, milliseconds, on the same
deadline. Measured: the whole adapter suite, live legs included, runs in 0.86s.

### The false refusal that had to be fixed with it, and why it is not a weakening

Shape (2) — a nonexistent denied path outside every writable root — turned out
NOT to be a policy defect at all. `~/.aws` and `~/.gnupg` do not exist on this
host, so the SHIPPED `local.json` and `cross_cutting.json` carry exactly that
shape. A naive dry run would have made both shipped artifacts unusable here.

`policy_args` therefore renders NO mask for a target that does not exist AND
cannot be created — one outside every writable root, where the baseline
`--ro-bind / /` rules. That is not a gap:

- nothing is at the path to read, and
- the contained process cannot bring anything into existence there — proven in
  the test, which runs `mkdir -p` on it inside the sandbox and asserts both the
  failure and the absence of the directory afterwards.

The denial is fully enforced; emitting `--tmpfs` for it merely aborted the
backend. A target that is missing but INSIDE a writable root still gets its
mask, because there the process COULD create and use it.

### What the dry run cannot pre-check — stated, not hidden

It proves the policy STARTS, not that every mask covers what the author meant: a
denied path that is a symlink RESOLVING INSIDE a writable root can mount
successfully while masking the link rather than the target. And the tree can
change between the dry run and the wrap — `render` re-reads the shapes it
depends on at wrap time, but no path-based backend can close that window. Both
limits are in the `dry_run_policy` doc comment.

`wrap` re-derives the cheap field lints (the forged-validation defense) but not
the dry run: the `PolicyValidation` token exists precisely so the expensive
check is not repeated per dispatch.

### A design correction the boundary gate forced, and which was right

Making the dry run real meant the shared suite's fixtures had to become real
paths — an invented `/suite-writable-root` cannot be bound, and a dry-running
backend is RIGHT to refuse it. The first attempt created that tree inside
`crates/gauntlet-ports/src/contract/sandbox.rs`, and `just boundaries` failed:

```
single-writer: 'crates/gauntlet-ports/src/contract/sandbox.rs:115'
  new production write-API token 'fs::create_dir_all' in a core crate
```

That gate was right, and it was NOT silenced or inventoried around. `contract/
sandbox.rs` is `pub mod` — production text — and `gauntlet-ports` is a pure
contract crate that writes nothing. The fix followed the precedent already in
the tree (`WorkspaceProviderFactory::root`): `SandboxFactory` gained
`fn fixture_dir(&self, segment: &str) -> PathBuf`. The suite decides the LAYOUT;
the factory decides WHERE and does the creating. Every filesystem write moved to
the two adapter crates (outside the core scan) and to `contract/stubs.rs`
(`#[cfg(test)]`-only, blanked from the production scan). `WRITE_API_INVENTORY`
is untouched.

### Regression home

- `tests::validate_dry_runs_the_policys_own_binds_and_masks` (adapter, live) —
  shapes (1) and (3) plus a nonexistent writable root must each be a typed
  `ValidationFailed` carrying the backend's own complaint; shape (2) must
  validate clean, actually START, and still deny (the `mkdir` leg above).
- `tests::a_mask_target_that_cannot_exist_renders_no_mask_and_one_that_can_still_does`
  (adapter) — pins both halves of the render decision.
- `crates/gauntlet-cli/tests/p5_hard_gate.rs`, shipped-artifact leg —
  strengthened: each of the three SHIPPED artifacts must now not only RESOLVE
  through the real provider but DRY-RUN CLEAN on this host's real backend under
  this host's real anchors, at `Isolation::Process`. Resolving was never enough:
  a shipped policy that resolves but cannot start would abort at spawn after the
  gate had already admitted the dispatch.

### Existing escalation tests

`p5_hard_gate::a_sandbox_validation_failure_escalates_without_falling_back` and
the rest of the p5/g7 gates stay green, unmodified.

### Two adapter tests updated (fixtures, not assertions)

`the_rendered_command_starts_with_the_backend_and_carries_every_root` and
`a_policy_path_cannot_smuggle_a_backend_flag_and_a_spaced_path_still_binds` used
invented paths (`/ws/denied`, `/ws/root --share-net`) and relied on the old
unconditional `--tmpfs`. They now build the same shapes as REAL scratch
directories — including a real directory literally named `root --share-net` —
so every assertion they made still fires, quoting included. No assertion was
weakened or removed; COR-1's quoting teeth still bite on a path that renders.

---

## GATES — all run sequentially, package-scoped, sole compiler

| Gate | Result |
| --- | --- |
| `just check` | pass |
| `cargo nextest run -p gauntlet-ports` | 111/111 passed |
| `-p gauntlet-adapter-sandbox-bwrap -p …-sandbox-none -p …-policy-fs` | 49/49 passed |
| `cargo nextest run -p gauntlet-app govern:: doctor::` | 44/44 passed, 302 skipped |
| `cargo nextest run -p gauntlet-cli --test p5_hard_gate --test g7_tier_downgrade` | 7/7 passed |
| `just lint` | pass (clippy `-D warnings`) |
| `just fmt-check` | pass |
| `just boundaries` | 215/215 passed |
| `just doctor` | `exit: OK` |
| `just ruler` | PASS — digests match the frozen lock |

The bwrap adapter alone is 20 tests, live bubblewrap legs included.

## Out of scope, untouched

Round-3 F4, F5, F6, F7, F8. No edits to `.gauntlet/policies/*.json`,
`.wayfinder/**`, `.agents/research/**`, `.agents/CURRENT_STATUS.yml`, or any
report but this one. No git operations, no commit. The stray
`.gauntlet/policies/irreversible.json` was not recreated.
