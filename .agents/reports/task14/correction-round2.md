# Task 14 — Correction round 2

Fixes for the three NEW defects found by the fresh blind executing falsifier
(round 2). COR-1/COR-2/COR-3 from round 1 were re-confirmed by that same blind
round (10 quoting payloads, 8 anchor escape shapes, neither broken) and are NOT
revisited here.

Build discipline: sole compiler, sequential package-scoped runs.

---

## COR-4 (round-2 F1, MAJOR) — a denied path nested in a writable root was re-exposed

**Status: FIXED**

### The defect

`BwrapSandbox::render` emitted, in order: the baseline (`--ro-bind / /` …), then
the denied-path MASKS, then the writable-root `--bind`s. bubblewrap applies mount
operations in ARGUMENT ORDER, so `--bind <root> <root>` arriving after
`--tmpfs <root>/<denied>` re-mounted the host subtree on top of the mask and
undid it. The adapter's own doc comment stated the correct rule ("a later bind
re-exposes what an earlier mask covered, so the order is load-bearing") and then
implemented its inverse.

Reproduced on this host (bubblewrap 0.11.1, `/usr/bin/bwrap`) before any code
change, mask-before-bind order:

```
$ bwrap --ro-bind / / --dev /dev --proc /proc --unshare-net --die-with-parent \
        --tmpfs $WS/.git --bind $WS $WS -- sh -c 'cat $WS/.git/config; echo PWNED > $WS/.git/config'
SECRET-GIT-CONFIG        <-- denied path READ
$ cat $WS/.git/config
PWNED                    <-- denied path WRITTEN, host file mutated
```

Same shapes after the fix (bind-before-mask): read fails
(`No such file or directory` for the dir mask, `Permission denied` for the
`/dev/null` file mask), the write lands in the overlay, and the host originals
are byte-unchanged.

### Locus of the fix

`crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs`, `BwrapSandbox::render`
— the writable-root `--bind` loop now precedes the denied-path mask loop. That is
the entire behavioural change; the mask SHAPES are untouched (`--tmpfs` for a
directory, `--ro-bind /dev/null` for a file).

Two doc corrections travelled with it, because a comment stating the opposite of
the code was part of the defect:
- the module header ("Denied paths are masked before writable roots are bound
  back read-write") now states the implemented and correct order;
- `render`'s doc now states the order, why it is load-bearing, and the honest
  fail-closed story for a mask target that does not exist: OUTSIDE every writable
  root the `--tmpfs` still fails backend startup on the read-only root; INSIDE a
  writable root the mount point is now creatable, so the mask mounts and the path
  is an empty tmpfs — denied either way. There is no ordering in which a missing
  mask lets the contained process run. (Verified live, both branches.)

### Regression tests (RED first, for the right reason)

1. **Shared port contract suite** —
   `crates/gauntlet-ports/src/contract/sandbox.rs`, new case
   `a_nested_denial_survives_the_writable_bind`, registered in `sandbox_suite`,
   with a new fixture segment `SUITE_NESTED_DENIED_SEGMENT`. Every sandbox
   adapter must now satisfy it. It states two halves: (i) a nested denial is a
   LEGITIMATE policy — refusing it at validate or at wrap is a defect, and that
   half binds every backend including the none rung; (ii) if the rendered command
   carries both the writable root and the nested denied path as shell WORDS, the
   last occurrence of the denied path must come AFTER the last occurrence of the
   enclosing root. Stated over `shell_words`, not substrings, following the style
   of `policy_paths_are_not_shell_words`.
2. **Broken stub** — `crates/gauntlet-ports/src/contract/stubs.rs`,
   `MaskBeforeBindSandbox` / `SandboxBrokenMaskOrder`: a process-rung backend
   that quotes every interpolated path CORRECTLY (so it does not trip the
   interpolation case) but emits masks before binds. Exactly one defect: the
   order.
3. **Catch test** — `crates/gauntlet-ports/src/contract/suite_runs.rs`,
   `sandbox_mask_order_broken_is_caught`, asserting the intended case by name.
4. **Adapter rendering pin** —
   `every_mask_is_rendered_after_the_last_writable_bind`: nested dir mask, nested
   file mask, and a SIBLING mask, all required after the last writable bind.
5. **Adapter LIVE confinement leg** —
   `live_confinement_denies_a_path_nested_inside_the_writable_root`: real
   `/usr/bin/bwrap`, real wrap, real execution. A write inside the writable root
   but outside any denied path still succeeds (the fix must not close the
   writable root); the nested denied dir-file and the nested denied file are
   neither readable (`known-secret-payload` never reaches stdout) nor writable
   (the host originals still read `known-secret-payload` afterwards).

RED evidence before the fix (3 tests, right reason, not compile errors):
- `every_mask_is_rendered_after_the_last_writable_bind` — panicked naming the
  rendered string with `--tmpfs …/ws/.git` at a word index before
  `--bind …/ws …/ws`.
- `live_confinement_denies_a_path_nested_inside_the_writable_root` — panicked on
  "a denied path nested in a writable root must never be readable:
  …/ws/.git/config" — i.e. the secret was actually read out of the live sandbox.
- `the_shared_contract_suite_is_clean` (bwrap) — the new shared case fired
  against the real adapter, naming word 11 (denied) before word 14 (root).

### GREEN after the fix

- `cargo nextest run -p gauntlet-adapter-sandbox-bwrap` — **15/15 passed**
  (includes both live legs).
- `cargo nextest run -p gauntlet-ports sandbox` — **7/7 passed**
  (`sandbox_green_is_clean` still clean; `sandbox_mask_order_broken_is_caught`
  passes).
- `cargo nextest run -p gauntlet-adapter-sandbox-none -p gauntlet-adapter-policy-fs`
  — **28/28 passed** (the none-rung adapter satisfies the new shared case).
- `cargo nextest run -p gauntlet-cli --test p5_hard_gate --test g7_tier_downgrade`
  — **7/7 passed**.

### Teeth on the fix

sha256 of `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs` before:
`da4bc37b035b2e1d0e68ce2580251fd67499d1a1c575883ca5b9c35bcbea59c3`

Minimal mutation: the two loops in `render` swapped back (masks before binds),
nothing else touched.

Result: `15 tests run: 12 passed, 3 failed` — the three failures being exactly
`every_mask_is_rendered_after_the_last_writable_bind`,
`live_confinement_denies_a_path_nested_inside_the_writable_root`, and
`the_shared_contract_suite_is_clean`.

Restored; sha256 after:
`da4bc37b035b2e1d0e68ce2580251fd67499d1a1c575883ca5b9c35bcbea59c3` — byte
identical. Re-run: **15/15 passed**.

The shared case has its own independent teeth via `SandboxBrokenMaskOrder`, so
the suite is proven able to fail an adapter on this rule, not only the one
adapter that had the defect.

### Nothing weakened

No assertion relaxed, no test skipped or deleted. The pre-existing sibling-denial
live leg (`live_confinement_holds_the_writable_root_and_masks_denied_paths`) and
the interpolation/quoting cases from round 1 all still pass unchanged.

---

## COR-5 (round-2 F2, MINOR but a real hole) — an unanchored absolute path was unbounded

**Status: FIXED**

### The defect

`resolve_anchored_path` refused three shapes — an unknown token, a token outside
prefix position, and a bare relative path — but a bare ABSOLUTE path is none of
them. It fell through every refusal into a passthrough branch that carried the
entry VERBATIM with no upper bound:

```rust
if Path::new(entry).is_absolute() {
    // An unanchored absolute path names no anchor, so there is no
    // anchor for it to escape: containment does not apply and the
    // entry is carried verbatim, exactly as before.
    return Ok(entry.to_owned());
}
```

Executed before the change: an artifact with `"writable_roots": ["/"]` resolved
to `SandboxPolicy { writable_roots: ["/"], .. }` (`Ok`, no findings), the bwrap
adapter validated it CLEAN at the process rung, and `render` emitted
`--bind / /` — the host root filesystem writable, under a validation reporting
no findings.

The module's own doc header already claimed the bound ("policy files never
hardcode operator-machine paths"), and `AnchorRoots` already claimed it ("an
artifact cannot widen its own anchor"). The code did not enforce either.

### The bound, decided and stated

**EVERY path entry must be anchored.** That is the module's documented discipline
made real, and it is the only shape that IS a bound: an anchored entry names a
root it must stay inside (already enforced by `contain_within_anchor`), while an
unanchored one names no root at all, so nothing limits how far it reaches. There
is no weaker second admission beside it.

Two loci, two lines of defense:

1. `crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs`,
   `resolve_anchored_path`: the unanchored-absolute passthrough is gone. Both
   remaining unanchored shapes now produce a typed `PolicyProviderError::Invalid`
   naming the FILE, the FIELD, the ENTRY, and the OFFENSE — "an unanchored
   absolute path hardcodes an operator-machine path and is bounded by nothing"
   for the absolute shape, "bare relative paths are refused" for the relative
   one. The module header and the function doc now state the enforced rule
   instead of a rule the code did not keep.
2. `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs`,
   `policy_findings`: the backend now bounds writable roots on its own. A
   writable root that resolves LEXICALLY to the filesystem root is a validation
   finding — `--bind <root> <root>` over the baseline's read-only `--ro-bind / /`
   would re-mount the whole host read-write and leave the sandbox nothing to
   contain. New helper `names_filesystem_root` resolves `.` and `..` lexically
   (so `/`, `/..`, and `/tmp/..` are all caught) and deliberately does NOT touch
   the filesystem: the entry is judged as written, so a nonexistent path cannot
   dodge the bound by failing to canonicalize. This line holds for any
   `SandboxPolicy` reaching the port however it was built, not only for one that
   came through `PolicyFs`.

### Regression tests (RED first, for the right reason)

- `gauntlet-adapter-policy-fs`:
  `an_unanchored_absolute_path_is_invalid_naming_the_entry` — `/`, `/etc`, and
  `/run/workspaces` (the last one deliberately EQUAL to the injected workspace
  anchor: reaching the right place by the wrong route is still unanchored) as
  `writable_roots`, plus `/etc/shadow` as a `denied_paths` entry. Each must be
  `Invalid` naming the entry, the file, and the offense.
  RED before the fix: `must be Invalid, got Ok(SandboxPolicy { … writable_roots:
  ["/"] … })`.
- `gauntlet-adapter-sandbox-bwrap`:
  `the_filesystem_root_as_a_writable_root_is_a_finding` — `/` and the dotted
  spelling `/tmp/..`, both required to be `ValidationFailed` naming the
  filesystem root.
  RED before the fix: `must be refused, got Ok(PolicyValidation { … isolation:
  Process, findings: [] })`.

### The shipped artifacts — VERIFIED, not assumed

`.gauntlet/policies/{contained,local,cross_cutting}.json` were read and left
untouched. Every path entry in all three is anchored (`$WORKSPACE_ROOT`,
`$PROJECT_ROOT/.git`, `$HOME/.ssh`, `$HOME/.aws`, `$HOME/.gnupg`).
`the_shipped_repo_policy_artifacts_resolve_for_every_dispatchable_tier` — the
test that reads the real repo artifacts — passes after the change.

### One pre-existing test went red; diagnosed, not silenced

`every_non_irreversible_tier_resolves_from_its_artifact` failed on its own
SCRATCH FIXTURE, which carried `"denied_paths": ["/etc/shadow"]` — an unanchored
absolute entry, i.e. the fixture was exercising precisely the admission this
correction removes. That test's claim is "each dispatchable tier resolves from
its artifact", not "an unanchored path resolves". The fixture entry was changed
to the anchored `$HOME/.gnupg` (the vocabulary the shipped artifacts use) and its
assertion updated to the resolved `/home/operator/.gnupg`. Nothing about the
test's claim was weakened, and the disposition of `/etc/shadow` is now pinned
explicitly — as a REFUSAL — by the new test. No assertion relaxed, no test
skipped or deleted.

### GREEN after the fix

- `cargo nextest run -p gauntlet-adapter-policy-fs` — **17/17 passed**.
- `cargo nextest run -p gauntlet-adapter-sandbox-bwrap` — **16/16 passed**.
- `cargo nextest run -p gauntlet-cli --test p5_hard_gate --test g7_tier_downgrade`
  — **7/7 passed**.
- `cargo nextest run -p gauntlet-app govern:: doctor::` — **44/44 passed**.

### Teeth on the fix (both loci, one mutation each)

sha256 before:
- policy-fs `lib.rs`: `665f60a52e38de7121857965429fbe2112b32b08c8bd6a6753b35a25465fce9a`
- bwrap `lib.rs`: `67eb4aa66f6af169aeeadb5cbb524a966acacc1425dc52881f3ed21107cd97ea`

1. **policy-fs.** Mutation: restore the passthrough (`if
   Path::new(entry).is_absolute() { return Ok(entry.to_owned()); }`) ahead of the
   refusal. Result: `17 tests run: 16 passed, 1 failed` — the failure being
   `an_unanchored_absolute_path_is_invalid_naming_the_entry`. Restored; sha256
   after: `665f60a52e38de7121857965429fbe2112b32b08c8bd6a6753b35a25465fce9a` —
   byte identical.
2. **bwrap.** Mutation: `else if false && names_filesystem_root(root)`. Result:
   `16 tests run: 15 passed, 1 failed` — the failure being
   `the_filesystem_root_as_a_writable_root_is_a_finding`. Restored; sha256 after:
   `67eb4aa66f6af169aeeadb5cbb524a966acacc1425dc52881f3ed21107cd97ea` — byte
   identical.

Re-run of both crates after restore: **33/33 passed**.

---

## COR-6 (round-2 F3 / round-1 F7) — AK-044's frozen locator now has teeth on the gate's own conjunct

**Status: FIXED**

### The defect

AK-044's frozen evidence locator is
`tests/proofs/p5_hard_gate::irreversible_never_dispatched`, materialized as
`crates/gauntlet-cli/tests/p5_hard_gate.rs::irreversible_never_dispatched`. §17.1
states TWO lines against an irreversible dispatch: the scheduler's §11.2
eligibility conjunct, and the gate's dispatch-time conjunct 1
(`crates/gauntlet-app/src/govern/gate.rs`,
`facts.tier == BlastRadius::Irreversible`). The second line exists precisely so a
forged or stale plan input cannot smuggle the tier past the first.

The locator ran the composed loop only. The scheduler refuses the unit before the
gate is ever consulted, so every assertion in the test — `TerminalState::Blocked`,
zero durable invocations, the ticket text — was carried by the scheduler ALONE.
Disabling the gate's conjunct left the locator fully GREEN: the ruler's named
proof had no teeth on the dispatch-time line, which was pinned only by an
in-crate unit test (`govern::gate::tests::irreversible_is_refused_before_every_other_conjunct`).

### The fix — coverage ADDED, the scheduler leg untouched

`crates/gauntlet-cli/tests/p5_hard_gate.rs::irreversible_never_dispatched` keeps
every existing assertion, byte for byte, and gains a second-line section that
calls the REAL `AuthorityGate::admit` directly with irreversible facts.

The discrimination is the refusal VARIANT, not merely that some refusal occurred
— conjunct 1 is unconditional and evaluated FIRST, "independent of what any
prompt says" and independent of backend and policy state. The facts are built so
no other conjunct can be the cause: authority `Critic` (within the ceiling),
reported isolation `Process` (at or above every required rung), and `recorded:
None` (the lattice conjunct is skipped).

Two postures are exercised:
- the §8.6 conservative no-backend posture (`GovernDeps::none_workspace()`);
- a LIVE posture over the real process-isolation backend and the real fs policy
  provider with real policy artifacts (`live_govern`), where every containment
  conjunct could otherwise be satisfied — the refusal must still be conjunct 1's,
  reached before the policy is ever resolved. Skipped honestly (with a printed
  note) if the backend is absent; on this host it runs.

### Acceptance evidence — the demonstration

sha256 of `crates/gauntlet-app/src/govern/gate.rs` before:
`9c986d1f057a1b48416a52be7c46e6bbecb498c8ed951ab374b4565cd58e653d`

**1. Disable the GATE conjunct alone.** Mutation:
`if false && facts.tier == BlastRadius::Irreversible {` — conjunct 1 only,
nothing else touched, the scheduler untouched.

Result: `4 tests run: 3 passed, 1 failed`, the failure being
`p5_hard_gate::irreversible_never_dispatched`:

```
under the conservative posture the gate must refuse an irreversible dispatch on
its OWN conjunct 1, not on a downstream one:
Err(CapabilityMismatch { unit: UnitId { raw: "AK-9344" }, tier: Irreversible,
                         runner_isolation: Process, required: Vm })
```

That failure text is itself the point: without conjunct 1 the top tier's refusal
becomes CONTINGENT on a downstream capability check rather than unconditional.

**2. Restore byte-identically.** sha256 after:
`9c986d1f057a1b48416a52be7c46e6bbecb498c8ed951ab374b4565cd58e653d` — identical.
Re-run: `p5_hard_gate` **4/4 passed**.

**3. The scheduler leg still has its own independent teeth** (proof that coverage
was added, not moved). sha256 of `crates/gauntlet-app/src/control/scheduler.rs`
before: `0524c92dca8d4141ae8eba72a09cd87d5a4048bbfab51a3503d65e6d46739765`.
Mutation: `if false && unit.blast_radius() == BlastRadius::Irreversible {` — the
§11.2 eligibility conjunct alone, the GATE left intact. Result:
`irreversible_never_dispatched` FAILS on the pre-existing ticket assertion:

```
the blocked ticket names the §17.1 first line: run blocked: Unit AK-9301's tier
irreversible has no concurrency entry — no policy admits it (spec 11.2, 17.3)
```

Restored; sha256 after:
`0524c92dca8d4141ae8eba72a09cd87d5a4048bbfab51a3503d65e6d46739765` — identical.
Re-run: **4/4 passed**.

The locator now fails when EITHER line of §17.1 is removed, which is what AK-044
claims and what it did not previously prove.

---

## Addendum — teeth re-run on the FINAL tree, and the gate results

`cargo fmt --all` ran after COR-6 (it rewrapped one `match` call in
`p5_hard_gate.rs` and one signature in `contract/sandbox.rs`), which changed two
of the file hashes quoted in the sections above. Every teeth cycle was therefore
RE-RUN against the final, formatted tree. The hashes below are the reproducible
ones; the in-section hashes were correct at the moment each demonstration ran.

| locus | sha256 (final tree, before AND after its mutation) | mutation | result |
| --- | --- | --- | --- |
| `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs` | `447868fbb8c4c8d20ffb1b991872c45173ae6409eef87234001d4e47bfa2e4e0` | COR-4: swap the bind/mask loops in `render` | 16 run, **13 passed, 3 failed** (`every_mask_is_rendered_after_the_last_writable_bind`, `live_confinement_denies_a_path_nested_inside_the_writable_root`, `the_shared_contract_suite_is_clean`) |
| `crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs` | `665f60a52e38de7121857965429fbe2112b32b08c8bd6a6753b35a25465fce9a` | COR-5: restore the unanchored-absolute passthrough | 17 run, **16 passed, 1 failed** (`an_unanchored_absolute_path_is_invalid_naming_the_entry`) |
| `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs` | `447868fbb8c4c8d20ffb1b991872c45173ae6409eef87234001d4e47bfa2e4e0` | COR-5: `else if false && names_filesystem_root(root)` | 16 run, **15 passed, 1 failed** (`the_filesystem_root_as_a_writable_root_is_a_finding`) |
| `crates/gauntlet-app/src/govern/gate.rs` | `9c986d1f057a1b48416a52be7c46e6bbecb498c8ed951ab374b4565cd58e653d` | COR-6: `if false && facts.tier == BlastRadius::Irreversible` | 4 run, **3 passed, 1 failed** (`irreversible_never_dispatched`) |
| `crates/gauntlet-app/src/control/scheduler.rs` | `0524c92dca8d4141ae8eba72a09cd87d5a4048bbfab51a3503d65e6d46739765` | COR-6 control: `if false && unit.blast_radius() == BlastRadius::Irreversible` | 1 run, **0 passed, 1 failed** (`irreversible_never_dispatched`, on the pre-existing scheduler assertion) |

Every restore was byte-identical (the "after" hash equals the "before" hash in
every row), and every crate was re-run green after its restore.

### Gates (sequential, final tree, real numbers)

| gate | result |
| --- | --- |
| `just check` | PASS — workspace type-checks, `Finished dev profile in 11.89s` |
| `cargo nextest run -p gauntlet-ports sandbox` | **7 run, 7 passed**, 103 skipped |
| `cargo nextest run -p gauntlet-adapter-sandbox-bwrap -p gauntlet-adapter-sandbox-none -p gauntlet-adapter-policy-fs` | **45 run, 45 passed**, 0 skipped |
| `cargo nextest run -p gauntlet-app govern:: doctor::` | **44 run, 44 passed**, 302 skipped |
| `cargo nextest run -p gauntlet-cli --test p5_hard_gate --test g7_tier_downgrade` | **7 run, 7 passed**, 0 skipped |
| `just lint` | PASS (clippy `-D warnings` + `cargo fmt --check`) |
| `just fmt-check` | PASS |
| `just boundaries` | **215 run, 215 passed**, 0 skipped |
| `just ruler` | PASS — "answer key and manifest digests match the frozen lock" |

`just lint` failed on its first invocation on FORMATTING only (two rewraps, no
clippy findings); `cargo fmt --all` was run and `just lint` then passed. That is
recorded rather than hidden.

### Out of scope, untouched, as instructed

Round-2 F4 (earned-type discipline at the port boundary) is DEFERRED and was not
fixed. Round-2 F5/F6/F7 are informational residuals and were not fixed.
`.gauntlet/policies/*.json` were READ (to verify COR-5 does not break them) and
NOT edited. `.wayfinder/**`, `.agents/research/**`, `.agents/CURRENT_STATUS.yml`,
and every report other than this one were not touched. No git operation was run.

### Files changed

- `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs` (COR-4 order + docs, COR-5 root bound, tests)
- `crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs` (COR-5 anchoring bound + docs, tests)
- `crates/gauntlet-ports/src/contract/sandbox.rs` (COR-4 shared contract case)
- `crates/gauntlet-ports/src/contract/stubs.rs` (COR-4 broken stub)
- `crates/gauntlet-ports/src/contract/suite_runs.rs` (COR-4 catch test)
- `crates/gauntlet-cli/tests/p5_hard_gate.rs` (COR-6 second-line coverage)
- `.agents/reports/task14/correction-round2.md` (this report)

`crates/gauntlet-app/src/govern/gate.rs` and
`crates/gauntlet-app/src/control/scheduler.rs` were MUTATED AND RESTORED for the
COR-6 demonstration only; both are byte-identical to their pre-demonstration
state.
