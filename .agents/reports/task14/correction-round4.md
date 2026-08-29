# Task 14 — correction round 4 (builder)

Round-4 fresh blind executing falsifier findings dispositioned to the builder:
F2 (MAJOR) as COR-9 and F4 (MINOR) as COR-10. F1, F3, F5–F8 are the
orchestrator's; nothing in this round touches them.

Build-lock honoured: every cargo invocation below was sequential, `-j 1`.

---

## COR-9 (round-4 F2, MAJOR) — containment stopped at the network namespace

**Locus:** `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs`,
`BASELINE_ARGS`.

### 1. Reproduced first, on this host (bubblewrap 0.11.1, `/usr/bin/bwrap`)

The shipped baseline was
`--ro-bind / / --dev /dev --proc /proc --unshare-net --die-with-parent` — the
network namespace and nothing else. Executed against a victim process I spawned
myself, OUTSIDE the sandbox:

| probe, run inside the "contained" sandbox | old baseline | new baseline |
|---|---|---|
| host PIDs visible in `/proc` | 92 | only the sandbox's own |
| `tr '\0' ' ' < /proc/<victim>/cmdline` | `sh -c trap … sleep 30` | `No such file` |
| `kill -TERM <victim>` | sent; **victim killed** | `No such process` |
| `ipcs -m` sees a host SysV shm segment | yes (`grep -c` → 1) | no (`grep -c` → 0) |

The kill was real: the victim I spawned outside the sandbox died from a signal
sent inside it. That is §17.3's named failure mode ("host-mutating fallbacks
explicitly refused rather than left at the backend's default consent") against
an adapter that reports `Isolation::Process`.

### 2. RED before the fix

Both new regressions failed against the unfixed baseline:

- `gauntlet-adapter-sandbox-bwrap tests::the_shared_contract_suite_is_clean` —
  `[sandbox::the_process_space_is_confined] … the payload of a wrap whose
  validation claims the process rung reached process 857880 — a process OUTSIDE
  the sandbox, and the very process that built it.`
- `gauntlet-adapter-sandbox-bwrap tests::live_confinement_holds_the_host_process_space`
  — `a contained process must not signal a process outside the sandbox (pid
  858868): "SIGNAL_LANDED\n"`.

### 3. The fix — every flag verified by execution, none pasted

`BASELINE_ARGS` is now, in this order:

```
--ro-bind / /
--unshare-user --unshare-pid --unshare-ipc --unshare-uts --unshare-cgroup --unshare-net
--new-session
--dev /dev  --proc /proc
--die-with-parent
```

Why each, and what was checked live before it was written down:

- **`--unshare-pid`** — the finding itself. Closes signalling, `/proc`
  enumeration and host `cmdline` reads.
- **`--unshare-ipc`** — verified against a real `ipcmk -M` segment: visible
  without it, invisible with it. The segment was removed (`ipcrm`).
- **`--unshare-uts`** — hostname/domainname are host state a contained process
  must not change.
- **`--unshare-cgroup`** — taken STRICT, not bubblewrap's `-try` variant. A
  `-try` flag that silently does nothing on a host without cgroup namespaces
  would leave the backend claiming a rung it did not achieve; strict means such
  a host fails the backend PROBE and gets a typed `BackendUnavailable`. Verified
  available here.
- **`--unshare-user`** — bubblewrap takes one implicitly when not setuid
  (`/usr/bin/bwrap` here is `-rwxr-xr-x`, not setuid); naming it makes the
  requirement explicit instead of inherited from the install.
- **`--new-session`** — no controlling terminal, so `TIOCSTI` input-queue
  injection cannot be attempted. The round-4 falsifier noted the sandbox was
  relying on this host's `dev.tty.legacy_tiocsti = 0` sysctl for that; the
  backend now holds it itself.
- **`--unshare-net`** — unchanged, still deny-by-default (§17.3).

**Ordering.** The namespaces are listed BEFORE the mounts that depend on them,
`--proc` in particular: under `--unshare-pid` it must be a procfs for the NEW
namespace. Executed both orderings on 0.11.1 — it sets every namespace up before
applying any mount, so on THIS bubblewrap the order is not what makes it work.
It is written this way so the dependency is legible and stays correct if a later
bubblewrap ever applied the two in argument order. `--die-with-parent` stays
last: it is the lifetime guarantee, not a containment flag.

**Nothing else was allowed to break — each checked by execution:**

| leg | result |
|---|---|
| exit-status propagation through the new pid ns | `true`→0, `false`→1, `exit 42`→42 |
| `--die-with-parent` lifetime | parent exits → sandbox dies, no leak |
| deadline/kill path (`run_bounded` SIGKILLs the bwrap pid) | payload AND its grandchild die; `--unshare-pid` makes this *stronger* than before, since killing the sandbox now takes the whole pid namespace |
| writable bind + nested tmpfs/`/dev/null` mask | write OK, mask OK |
| network still denied | loopback only |
| uid inside the sandbox | 1000 (unchanged) |
| stdin/stdout/stderr under `--new-session` | all three pass through |
| COR-8 dry run (`validate_dry_runs_the_policys_own_binds_and_masks`) | PASS |
| COR-2/COR-7 anchoring (`a_nested_denial_survives_a_rename_between_sessions`, `only_a_relocatable_ancestor_is_a_finding`) | PASS |

**No namespace was found unavailable on this host**, so there is no honest gap
to record: user, pid, ipc, uts, cgroup and net all unshare, each verified
individually before being adopted.

### 4. Regression home

**Shared port contract suite** —
`crates/gauntlet-ports/src/contract/sandbox.rs`, new case
`the_process_space_is_confined`, registered in `sandbox_suite`, so it binds
EVERY adapter that claims a process rung or above and skips (vacuously) any
backend below it.

The case had to be behavioural, and that is the one design decision here worth
stating. Every earlier case reads shell WORDS off the rendered command, which
works because the words in question are the policy's own paths — the suite
supplied them. A process namespace has no such word: the flag that unshares it
is backend vocabulary the port must not know (§24.3), and its ABSENCE is
indistinguishable from a backend that confines the process space by other
means. So the case runs the wrapped command and asks the payload what it could
reach. The running is delegated to a new `SandboxFactory::run_rendered`, for
exactly the reason `fixture_dir` delegates the creating: `gauntlet-ports` is a
pure contract crate that spawns nothing.

The "process outside the sandbox" is the SUITE'S OWN process, so the case needs
no victim and can never name a process it did not create. Its probe is `kill -0`
— the permission-and-existence check every signal performs, delivering nothing —
so a successful probe proves `SIGTERM` would have landed without sending one.

**Broken stub** — `stubs.rs`, `HostProcessSpaceSandbox` /
`SandboxBrokenProcessSpace`: claims the process rung, gates its validations
exactly like the existing process-rung stub, and really does wrap (its own
invocation is the first process, the payload inside it, so
`wraps_from_process_start` is satisfied) — but confines nothing about the
process space. Its identity is a REAL invocation, because a fake whose command
cannot execute would be caught for the wrong reason.

**Catch test** — `suite_runs.rs`,
`sandbox_process_space_broken_is_caught`, asserting the intended case.

**LIVE bwrap leg** — `live_confinement_holds_the_host_process_space`. It spawns
its OWN victim (`Victim`, a struct whose `Drop` kills and reaps it, so a failing
assertion cannot leak it), never names a host PID it did not create, and sends
the DESTRUCTIVE signal (`SIGTERM`) rather than a permission check standing in
for it. It asserts: the payload can still read its OWN `/proc` (the fix confines
the process space, it does not remove the mount); the signal is refused; the
victim is still alive afterwards; and the victim's `/proc/<pid>/cmdline` is
unreadable from inside.

### 5. GREEN, then teeth

`cargo nextest run -p gauntlet-adapter-sandbox-bwrap` — **21/21 passed**
(19 before this round + the live leg + the new shared case inside
`the_shared_contract_suite_is_clean`).
`-p gauntlet-ports -p gauntlet-adapter-sandbox-none -p gauntlet-adapter-policy-fs`
— **141/141 passed**.

Teeth, each mutation applied to the tree, run, then restored and the restoration
verified byte-identically against a recorded sha256:

| mutation | expected | observed |
|---|---|---|
| `BASELINE_ARGS`: `"--unshare-pid"` → `"--unshare-net"` (a duplicate, so the array shape is untouched and only the pid namespace is lost) | the process-space regressions go RED | **RED**: `the_shared_contract_suite_is_clean`, `live_confinement_holds_the_host_process_space`, and `the_rendered_command_starts_with_the_backend_and_carries_every_root` — 3 failed, 18 passed |
| the shared case's rung guard `< Isolation::Process` → `<= Isolation::Process` (the case skips exactly the backends it governs) | the catch test goes RED | **RED**: `sandbox_process_space_broken_is_caught` |

sha256 of both mutated files verified `OK` against the pre-mutation digests
after restoration, and both suites re-ran GREEN.

One mutation deliberately did NOT go red and is worth recording: blinding only
the REACHED branch (`if false && stdout.contains(…)`) left the catch test GREEN,
because the second branch — "the wrapped command ran but printed neither probe
marker" — reports on the same case. That is the case being robust rather than
the case being toothless; the rung-guard mutation above is the single point that
neuters it, and it goes red.

### 6. What this case does NOT claim

It observes the process space, not the kernel. The adapter's rung is still
honestly `Isolation::Process`; a user-namespace mount sandbox wraps process
trees, not kernels, and nothing in this round changes that. The case also does
not attest to namespaces it cannot see the effect of from a shell payload —
`--unshare-uts`, `--unshare-cgroup` and `--unshare-user` are justified above and
were each verified live, but only the pid-space legs are pinned by a test.

---

## COR-10 (round-4 F4, MINOR) — the §17.3 rung table was written twice

**Locus:** `crates/gauntlet-app/src/doctor/checks.rs` (`tier_posture`) and
`crates/gauntlet-app/src/govern/posture.rs` (`required_isolation`).

### 1. The defect, restated from the falsifier's evidence

The doctor carried its own `NONE_POSTURE` / `LIVE_POSTURE` constants encoding
contained→none, local→process, cross_cutting→process; the gate derived the same
mapping from `required_isolation`. Nothing tied them. The falsifier mutated the
GATE's table (`Local | CrossCutting => Isolation::None`) and **all 32 `doctor::`
tests stayed GREEN** — so the doctor could report a posture the gate would not
enforce, which is a green preflight for a dispatch that then refuses.

### 2. The fix — the duplicate is impossible, not cross-checked

The work order prefers making the duplicate impossible over adding a test that
compares two copies, and that is what this is: **the doctor's table is deleted.**
`govern::posture` gains

```rust
pub fn dispatchable_posture(backend_live: bool) -> Vec<(BlastRadius, Isolation)>
```

which pairs each dispatchable tier with `required_isolation(tier)`. Check 4 now
calls it. There is no second representation left to drift, and therefore no
cross-check to forget.

The §8.6 conservative posture is DERIVED rather than listed: the tiers a
no-backend configuration can serve are exactly the tiers whose required rung is
`Isolation::None`. That is §8.6 stated in §17.3's vocabulary instead of stated
a second time — so the no-backend posture also moves with the table rather than
beside it.

`DISPATCHABLE_TIERS` holds the tier SET (contained, local, cross_cutting;
`irreversible` absent by §17.1 line 1 — it is never dispatched). That is a
different fact from the rung table, and `required_isolation` remains total over
all four tiers.

### 3. Proof: mutating the mapping in ONE place now fails a test

sha256 of both files recorded, then the falsifier's own A12/A4 mutation applied
to the single source:

```
-  BlastRadius::Local | BlastRadius::CrossCutting => Isolation::Process,
+  BlastRadius::Local | BlastRadius::CrossCutting => Isolation::None,
```

`cargo nextest run -p gauntlet-app doctor::` — **the doctor suite alone, the one
the falsifier proved was blind**:

```
FAIL doctor::tests::check_four_covers_the_dispatchable_tiers_with_the_measured_rung
Summary  32 tests run: 31 passed, 1 failed
```

Before this correction that mutation left 32/32 green. It is now RED in the
doctor's own suite, in addition to the `govern::posture::` and `p5_hard_gate`
tests it already tripped.

Restored:

```
$ sha256sum -c cor10.sha256
crates/gauntlet-app/src/govern/posture.rs: OK
crates/gauntlet-app/src/doctor/checks.rs: OK
```

`cargo nextest run -p gauntlet-app govern:: doctor::` — **44/44 passed** after
restoration.

### 4. Bounded — what was NOT folded in

`check_capabilities`'s sibling in check 5 keeps its own
`["contained", "local", "cross_cutting"]` list of tier POLICY-FILE names. That
is a tier-set spelling, not the rung table, and folding it in is outside COR-10's
locus; it is recorded here rather than changed.

---

## Gate run (end, sequential, `-j 1`)

| gate | result |
|---|---|
| `just check` | OK |
| `cargo nextest run -p gauntlet-ports` | **112 passed** |
| `-p gauntlet-adapter-sandbox-bwrap -p gauntlet-adapter-sandbox-none -p gauntlet-adapter-policy-fs` | **50 passed** |
| `cargo nextest run -p gauntlet-app govern:: doctor::` | **44 passed** |
| `cargo nextest run -p gauntlet-cli --test p5_hard_gate --test g7_tier_downgrade` | **7 passed** |
| `just lint` | OK (clippy `-D warnings` + fmt check) |
| `just fmt-check` | OK |
| `just boundaries` | **215 passed** |
| `just doctor` | `exit: OK` |
| `just ruler` | PASS — digests match the frozen lock |

### One gate caught something, and the gate won

`just boundaries` failed on the FIRST attempt, and it was right to:

```
hardcoded-command:        crates/gauntlet-ports/src/contract/stubs.rs:1882: 'sh'
hardcoded-tool-identity:  crates/gauntlet-ports/src/contract/stubs.rs:1882: 'sh'
hardcoded-tool-identity:  crates/gauntlet-ports/src/contract/stubs.rs:2497: 'sh'
hardcoded-tool-identity:  crates/gauntlet-ports/src/contract/stubs.rs:2503: 'sh'
```

COR-9's stubs had named a shell in a CORE crate, and §8.1 says commands are
configuration, not code. The gate was not touched, exempted, or narrowed. The
stubs were changed instead:

- `stub_run_rendered` READS the interpreter from the environment
  (`STUB_SHELL_ENV`) rather than naming one, and reports "the environment names
  no SHELL" as its OWN failure to run — which the process-space case surfaces on
  the same case, so the catch test's teeth do not depend on the environment,
  only their detail does.
- the broken stub's backend identity is that interpreter's own NAME, derived at
  runtime, with a non-runnable sentinel when the environment names none.
- the shell's command-string flag is a named constant
  (`STUB_SHELL_COMMAND_FLAG`), so the spawn statement carries no literal at all
  — the gate's `Command::new(…)` clause reads the first literal in the
  statement as the command, and the honest answer is that there isn't one.

The bwrap and none ADAPTERS keep their own `run_rendered`; adapters are outside
this gate's core-crate scope, and naming the shell is exactly what an adapter is
for.

Re-run after the change: **215/215 passed.**

### Tree

Changed: `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs`,
`crates/adapters/gauntlet-adapter-sandbox-none/src/lib.rs`,
`crates/gauntlet-ports/src/contract/{sandbox,stubs,suite_runs}.rs`,
`crates/gauntlet-app/src/govern/{posture,mod}.rs`,
`crates/gauntlet-app/src/doctor/checks.rs`, and this report.

`.agents/reports/doctor.json` was rewritten by the `just doctor` gate itself,
which the work order requires; nothing else under `.agents/reports/` was
touched. No git command was run. `.gauntlet/policies/*`, `.wayfinder/**`,
`.agents/research/**` and `CURRENT_STATUS.yml` are untouched, and the staged
deletion of `.gauntlet/policies/irreversible.json` was left exactly as
inherited — not recreated, not restaged.

Every scratch artifact (probe scripts, victim processes, the SysV shm segment
created to verify `--unshare-ipc`, the sha256 baselines) lived in the session
scratchpad or was removed; `ipcrm` cleaned the one segment.
