# Task 14 — correction round 1

Charter: `.agents/agents/builder.agent.md` (adopted verbatim).
Contract: fix COR-1 (falsifier F1), COR-2 (F2), COR-3 (F4). Record-only: F3, F5-F11.

STATUS: IN PROGRESS — this file is updated after every correction, never only at the end.

## Reproductions read from the tree first

- `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs` `render()` (~220-244):
  `words.join(" ")` with `root.clone()` / `denied.clone()` pushed RAW; only the
  `sh -c` payload passes through `single_quote_escape`.
- `crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs` `resolve_anchored_path`
  (~186-238): token prefix substitution + `is_absolute()`; no canonicalization,
  no containment re-check.
- `crates/gauntlet-app/src/govern/gate.rs`: `GateAdmission` all-`pub` fields
  (~216-232); `wrap_builder_command` `let ... else { return Ok(None) }` (~541-547).

(Sections below are appended as each correction lands.)

---

## COR-1 — policy strings must not become shell words (falsifier F1, MAJOR)

**Status: FIXED.**

### Regression home (written FIRST)

New case in the SHARED PORT CONTRACT SUITE — every sandbox adapter must
resist this, not just bwrap:

- `crates/gauntlet-ports/src/contract/sandbox.rs`
  - `policy_paths_are_not_shell_words` (registered in `sandbox_suite`, so it
    runs for every adapter that runs the suite).
  - `shell_words()` — a POSIX word splitter (single quotes literal;
    backslash escapes inside double quotes and unquoted; unterminated quote
    or dangling backslash is a typed `Err`, not a silent split). The case
    needs WORD semantics, not substring semantics: `'/a b'` contains the
    bytes of `/a` but is ONE argument.
  - `policy_with_paths()` fixture constructor; `policy_for_tier` now routes
    through it (no behavior change to existing cases).
  - unit test `shell_words_reads_quoting_the_way_a_shell_does` pins the
    splitter itself (it is the case's instrument).

Both legs the correction order required are in the one case:

- **(a) smuggling:** a `writable_roots`/`denied_paths` entry of the shape
  `<abs path> --suite-smuggled-flag` must never put `--suite-smuggled-flag`
  into the rendered command as a shell word of its own. A backend is free to
  REFUSE the hostile policy (a refusal executes nothing); what it may not do
  is render it as structure.
- **(b) legitimacy:** a well-formed absolute path that CONTAINS a space must
  still validate clean, still wrap, and arrive as ONE word. This is what
  forecloses the wrong fix — a backend cannot buy leg (a) by rejecting
  spaces at validation time, because leg (b) then fails.

### Teeth on the case itself (the suite must be failable)

- `crates/gauntlet-ports/src/contract/stubs.rs`: `RawInterpolationSandbox` /
  `SandboxBrokenShellWords` — a process-rung backend whose validation gating
  and tier posture delegate to the conforming `VerbatimProcessSandbox`, with
  exactly ONE defect: it joins policy paths into its rendered command raw.
- `crates/gauntlet-ports/src/contract/suite_runs.rs`:
  `sandbox_shell_words_broken_is_caught`, matching the existing
  `sandbox_fallback_broken_is_caught` / `sandbox_process_start_broken_is_caught`
  pattern (`assert_caught` also pins the CASE NAME, so an incidental catch
  does not count).

### RED, for the right reason

`cargo nextest run -p gauntlet-adapter-sandbox-bwrap` before the fix:
12 tests, 11 passed, **1 failed** — `the_shared_contract_suite_is_clean`,
reporting `policy_paths_are_not_shell_words` three times. Verbatim locus
from the failure text:

    /usr/bin/bwrap ... --unshare-net --die-with-parent
      --tmpfs /suite-denied-path --suite-smuggled-flag
      --bind /suite-writable-root --suite-smuggled-flag /suite-writable-root --suite-smuggled-flag
      -- sh -c 'run the probe inside the workspace'

`--suite-smuggled-flag` stands alone as a word AFTER `--unshare-net` — the
demonstrated attack, reproduced by the suite. Leg (b) failed in the same run
(`"/suite-writable-root suite-spaced-tail"` split across words). Not a
compile error: the suite compiled and ran, and reported the contract
violation.

### Fix (smallest coherent change)

`crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs`:

- new `shell_word(word)` — a word of only `is_shell_safe` bytes is emitted
  unchanged (the baseline flags and ordinary paths stay readable); anything
  else goes through the existing `single_quote_escape` dance. `is_shell_safe`
  is a conservative ALLOWLIST (ASCII alphanumeric plus `_-./:,+=@`), so a
  metacharacter cannot be admitted by omission — every byte above ASCII takes
  the quoted path too.
- `render()` now assembles every word RAW and maps the whole vector through
  `shell_word` at the end; the `sh -c` payload keeps its own unconditional
  `'...'` wrapper (its documented shape) and is appended after the mapping.
  Quoting once at the end is what makes it structural: a future push cannot
  forget the quoting, because no push does the quoting.
- `policy_findings()` is UNCHANGED. Paths containing spaces are deliberately
  NOT rejected — that would trade a security defect for a correctness defect.

Also added, at the adapter level, `a_policy_path_cannot_smuggle_a_backend_flag_and_a_spaced_path_still_binds`,
pinning the exact rendering of both legs (`--tmpfs '/ws/root --share-net'`,
`--bind '/home/some user/ws' '/home/some user/ws'`).

### GREEN

- `cargo nextest run -p gauntlet-adapter-sandbox-bwrap`: 13 tests, 13 passed.
- `cargo nextest run -p gauntlet-ports sandbox`: 6 tests, 6 passed
  (green stub clean; all four broken stubs caught on their intended cases).
- `cargo nextest run -p gauntlet-adapter-sandbox-none`: 12 tests, 12 passed
  (the none-rung backend satisfies the new case: it interpolates nothing, and
  it does not refuse a path containing a space).

### Teeth on the FIX

sha256 of `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs` before:
`0e6bab501c37d445ffc87095d1d56866163f2a46c62bccd7f470ef7df9e508da`.

Mutation: added `' '` to the `is_shell_safe` allowlist (one character class —
a space becomes "safe", so a path containing one is emitted unquoted).

Result with the mutation (`--no-fail-fast`): 13 tests, **2 failed** —
`a_policy_path_cannot_smuggle_a_backend_flag_and_a_spaced_path_still_binds`
AND `the_shared_contract_suite_is_clean`. Both the adapter-level pin and the
shared port-contract case have teeth on this fix.

Restored from the pre-mutation copy; sha256 after restore:
`0e6bab501c37d445ffc87095d1d56866163f2a46c62bccd7f470ef7df9e508da` —
byte-identical. Re-run: 13 tests, 13 passed.

---

## COR-2 — anchored paths must not escape their anchor (falsifier F2, MAJOR)

**Status: FIXED.**

Spec §17.5.2, read from `.agents/specs/Core-spec.md:1089` verbatim:

> Workspace paths stay inside `workspace.root`, verified by absolute-path
> prefix check after canonicalization (symlink escape is checked, not
> assumed absent).

### Regression tests (written FIRST)

`crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs`, tests module —
anchor resolution is this adapter's own concern (the `PolicyProvider` port
knows nothing about anchors), so the adapter's suite is the regression home:

- `an_anchored_path_may_not_escape_its_anchor` — `$WORKSPACE_ROOT/../outside`,
  `$PROJECT_ROOT/..`, `$HOME/../../etc`, `$WORKSPACE_ROOT/inside/../../outside`.
  All three tokens, and the finding must name the FILE, the ENTRY, and the
  offense.
- `an_anchor_sibling_sharing_a_byte_prefix_is_not_contained` — the trap the
  order named: `/run/workspaces-evil` must not count as inside
  `/run/workspaces`.
- `interior_dot_segments_normalize_inside_the_anchor` — `..` is not banned,
  containment is enforced: `$WORKSPACE_ROOT/a/../b/./c` resolves to
  `/run/workspaces/b/c`.
- `a_path_that_does_not_exist_yet_still_resolves` — the other trap: a policy
  path that does not exist yet must still resolve.
- `a_symlink_out_of_the_anchor_is_an_escape` (`#[cfg(unix)]`) — a real
  scratch tree: `<anchor>/escape` is a symlink to a sibling outside the
  anchor and must be refused, while `<anchor>/real` still resolves.

RED before the fix: `cargo nextest run --no-fail-fast -p gauntlet-adapter-policy-fs`
= 16 tests, 12 passed, **4 failed** — the three escape/sibling/symlink tests
returned `Ok`, e.g.

    got Ok(SandboxPolicy { ... writable_roots: ["/run/workspaces/../outside"] ... })

and the normalization test reported `left: ["/run/workspaces/a/../b/./c"]`.
Compiled and ran; failed on the assertions, not on the build.
`a_path_that_does_not_exist_yet_still_resolves` passed before AND after — it
is the guard on the fix, not a demonstration of the defect.

### Fix

`resolve_anchored_path` now takes the artifact's file name (so findings name
the file, matching `decode`'s existing `{name} …` finding style) and routes
every ANCHORED entry through the new `contain_within_anchor`, which does
exactly what §17.5.2 says, in order:

1. **Lexical normalization** (`normalize_lexically`): `.` dropped, `..` pops
   the preceding segment; a `..` with nothing left to pop returns `None` and
   is refused (it walks above the filesystem root).
2. **Prefix check by COMPONENTS** — `Path::starts_with`, which compares
   components, never bytes. `/run/workspaces-evil` is not inside
   `/run/workspaces`.
3. **Symlink check** — the same component prefix check repeated on
   canonicalized forms, because a lexical check cannot see a link.

**Trap 1 — `canonicalize()` fails on paths that do not exist yet. Approach
taken, stated explicitly:** neither refuse nor skip. Refusing would reject
honest policies (a workspace directory is created later); skipping would let
the symlink check be bypassed by simply naming a deeper path. Instead
`existing_canonical` canonicalizes the **deepest existing ancestor** and
re-attaches the not-yet-existing remainder. Every link that exists is
resolved; the part that does not exist cannot be a link, because it is not
anything. The walk terminates: each step drops one component and the
filesystem root canonicalizes.

**Trap 2 — component comparison** is `Path::starts_with` in both checks, and
`an_anchor_sibling_sharing_a_byte_prefix_is_not_contained` covers it.

**What is RETURNED** is the lexically normalized path, not the canonicalized
one: canonicalization is how containment is VERIFIED, not a rewrite of what
the operator wrote. Once the check passes the two name the same location, and
the existing expectations (`$HOME/.ssh` → `<home>/.ssh`) stay exact.

**Deliberately unchanged:** an UNANCHORED absolute entry (e.g. `/etc/shadow`)
is still carried verbatim. It names no anchor, so there is no anchor for it
to escape; F2 is an anchor-escape finding and widening this into a general
absolute-path policy would be scope the order did not authorize. Noted in the
code as such.

### GREEN

`cargo nextest run --no-fail-fast -p gauntlet-adapter-policy-fs`: 16 tests,
16 passed. That includes the pre-existing
`the_shipped_repo_policy_artifacts_resolve_for_every_dispatchable_tier`,
which resolves the real `.gauntlet/policies/*.json` against real anchors —
the shipped artifacts still resolve unchanged. No policy JSON was touched.

### Teeth on the FIX

sha256 of `crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs` before:
`642e0f586d7fe145ded0a05286d79298194b7a6db2a82c21a410f8f0de7092e6`.

- **Mutation M1a** (recorded because it is informative, not because it
  passed): turning ONLY the lexical prefix check into a byte-string
  `starts_with` left all 16 tests green — the canonicalized second check is
  also component-wise and caught the sibling on its own. A single-site
  mutation is masked here, so the honest mutation is the discipline itself.
- **Mutation M1b:** both prefix checks turned into byte-string `starts_with`
  → 16 tests, **1 failed**: `an_anchor_sibling_sharing_a_byte_prefix_is_not_contained`.
  Restored; sha256 back to `642e0f58…de7092e6`.
- **Mutation M2:** the symlink re-check deleted → 16 tests, **1 failed**:
  `a_symlink_out_of_the_anchor_is_an_escape`. Restored; sha256 back to
  `642e0f586d7fe145ded0a05286d79298194b7a6db2a82c21a410f8f0de7092e6`,
  byte-identical, and the re-run is 16 passed.

Both halves of the fix are load-bearing under test.

---

## COR-3 — a gate admission must be unforgeable (falsifier F4)

**Status: FIXED.**

Precedent followed (as instructed, not reinvented): Task 13's earned
admission — `crates/gauntlet-ports/src/producer_authority.rs`
`AdmittedSettlement` (private field, accessor, minted only by
`SettlementCommitAuthority::admit`, typed refusal enum). `GateAdmission` now
has the same shape.

### Regression test (written FIRST)

`crates/gauntlet-app/src/govern/gate.rs`,
`an_admission_the_posture_did_not_earn_is_a_typed_refusal`. It was written
against the API AS IT STOOD (asserting `is_err()`), so it COMPILED and RAN
before the fix — a compile error would not have been a red test:

    FAIL govern::gate::tests::an_admission_the_posture_did_not_earn_is_a_typed_refusal
    a validated admission under a sandbox-less posture must be a typed
    refusal, not a silent no-wrap: Ok(None)

`Ok(None)` — the exact F4 shape. After the fix the assertions were
STRENGTHENED to match the specific typed variants (`is_err()` →
`matches!(.., Err(WrapRefusal::PostureLostItsSandbox { .. }))` and
`AdmissionPostureMismatch`), and a third leg was added pinning that the
legitimate no-wrap answer stays distinguishable. Assertions only got
stronger; none was relaxed.

### Fix

1. **Unforgeable mint.** `GateAdmission`'s fields are now PRIVATE
   (`unit`, `tier`, `containment`), with accessors `unit()`, `tier()`,
   `policy()`, `validation()`, `policy_ref()`. There is no constructor
   outside `govern::gate`, so the only way to hold one is to have passed
   `AuthorityGate::admit`'s six conjuncts — and a holder cannot mutate one.

2. **The malformed shape is now UNREPRESENTABLE, not merely rejected.** The
   two parallel `Option`s are replaced by one private enum:

   ```rust
   enum Containment {
       NoBackendContained,
       Validated { policy: SandboxPolicy, validation: PolicyValidation },
   }
   ```

   "policy `Some`, validation `None`" — the state the falsifier pointed at —
   cannot be built at all. `Containment` is private, so a public variant
   cannot serve as a back-door constructor for the admission's contents.

3. **No empty success.** `wrap_builder_command` returns
   `Result<BuilderCommand, WrapRefusal>` instead of
   `Result<Option<SandboxedCommand>, SandboxError>`:

   - `BuilderCommand::Sandboxed(cmd)` — the wrapped invocation;
   - `BuilderCommand::Unwrapped { command }` — the §8.6 contained/no-backend
     posture, which legitimately has no policy (§5.3.5), handing the command
     BACK rather than returning an absence;
   - `Err(WrapRefusal::…)` — typed refusal.

   Both success arms are a command, so the `if let Some(c) = wrap(..) {
   run(c) } else { run(raw) }` shape the order warned about no longer type-
   checks: there is no arm that means "nothing for you, use what you had".
   The legitimate no-wrap answer stays fully distinguishable — it is its own
   variant, not an error and not an empty option.

4. **Typed refusals** (`WrapRefusal`, with `unit()`, `Display`, and
   `std::error::Error`, matching `GateRefusal`'s style):
   - `PostureLostItsSandbox` — a validated admission presented to a posture
     holding no sandbox (the F4 case: earned elsewhere, or a degraded
     composition);
   - `AdmissionPostureMismatch` — a no-backend admission presented to a
     real-backend posture (cross-wiring that would DOWNGRADE a backed
     dispatch to unwrapped);
   - `Backend` — the port's own typed wrap error, carried with the unit.

### Call sites updated (no assertion weakened)

- `crates/gauntlet-app/src/govern/mod.rs` — exports `BuilderCommand` and
  `WrapRefusal`.
- `crates/gauntlet-app/src/control/controller.rs` `record_gate_admission` —
  reads through the accessors; its `unreachable!` arm is kept total and its
  comment now says the mixed shape is unrepresentable by the enum.
- `crates/gauntlet-cli/tests/p5_hard_gate.rs` — three live wrap sites now
  destructure `BuilderCommand::Sandboxed` and PANIC on the unwrapped arm
  ("a real-backend posture wraps; it never hands back an unwrapped
  command") — a stronger assertion than the previous `.expect("a backend
  posture wraps")` on an `Option`.

### GREEN

- `cargo nextest run --no-fail-fast -p gauntlet-app govern::`: 12 tests,
  12 passed.
- `cargo nextest run --no-fail-fast -p gauntlet-cli --test p5_hard_gate
  --test g7_tier_downgrade`: 7 tests, 7 passed (including the live
  `containment_wraps_from_process_start_and_confines_the_session`).

### Teeth on the FIX

sha256 of `crates/gauntlet-app/src/govern/gate.rs` before:
`0638bdfd51a457d8c43863b8aef94f1894e77118368c82e4df008f16fae2c366`.

Mutation: the `(Containment::Validated { .. }, None)` arm changed from
`Err(WrapRefusal::PostureLostItsSandbox { .. })` back to a success
(`Ok(BuilderCommand::Unwrapped { .. })`) — the old silent-fallback
behaviour, in the new type.

Result: 12 tests, **1 failed** —
`an_admission_the_posture_did_not_earn_is_a_typed_refusal`.

Restored; sha256 after restore:
`0638bdfd51a457d8c43863b8aef94f1894e77118368c82e4df008f16fae2c366` —
byte-identical. Re-run: 12 passed.

---

## Post-correction adjustments (recorded, not hidden)

Two changes landed AFTER the three correction sections above were written,
both forced by repository gates, neither touching the fixes' logic:

1. **`just lint`** rejected `Containment` under `clippy::large_enum_variant`
   (`Validated` ≈ 208 bytes vs a 0-byte sibling). The variant's payload moved
   into a private `ValidatedContainment` struct behind a `Box`. The type is
   still private to `govern::gate`, so it is no more a constructor for an
   admission than the enum was. No `#[allow]` was added.
2. **`just boundaries`** (`hardcoded-timeout`, §8.1) flagged the numeric `2`
   in a multi-line `facts(2, …)` call that the literals scanner joined with a
   following `far_deadline()` line. The facts were hoisted into their own
   `let` statement — the same discipline `far_deadline`'s existing comment
   documents. No test, assertion, or scanner rule was changed.

Both were re-verified: teeth re-run against the FINAL bytes of every file.

- `crates/adapters/gauntlet-adapter-sandbox-bwrap/src/lib.rs`
  `0e6bab501c37d445ffc87095d1d56866163f2a46c62bccd7f470ef7df9e508da`
  (unchanged since its teeth run).
- `crates/adapters/gauntlet-adapter-policy-fs/src/lib.rs`
  `bcc3cc28499bb61c5512a2bf3c864261e3d88769b34848c7f00d62c234498cf4` —
  M1b re-applied → 1 failed (`an_anchor_sibling_sharing_a_byte_prefix_is_not_contained`);
  M2 re-applied → 1 failed (`a_symlink_out_of_the_anchor_is_an_escape`);
  restored byte-identically, 16/16.
- `crates/gauntlet-app/src/govern/gate.rs`
  `9c986d1f057a1b48416a52be7c46e6bbecb498c8ed951ab374b4565cd58e653d` —
  mutation re-applied → 1 failed
  (`an_admission_the_posture_did_not_earn_is_a_typed_refusal`);
  restored byte-identically, 12/12.

## GATES — real numbers, run sequentially at the end

| Gate | Result |
|---|---|
| `just check` (workspace, all targets) | PASS |
| `cargo nextest run -p gauntlet-ports sandbox` | 6 run, 6 passed, 103 skipped |
| `cargo nextest run -p gauntlet-adapter-sandbox-none` | 12 run, 12 passed |
| `cargo nextest run -p gauntlet-adapter-sandbox-bwrap` | 13 run, 13 passed |
| `cargo nextest run -p gauntlet-adapter-policy-fs` | 16 run, 16 passed |
| `cargo nextest run -p gauntlet-app govern:: doctor::` | 44 run, 44 passed, 302 skipped |
| `cargo nextest run -p gauntlet-cli --test p5_hard_gate --test g7_tier_downgrade` | 7 run, 7 passed |
| `just lint` (clippy `-D warnings` + fmt) | PASS, 0 warnings |
| `just fmt-check` | PASS |
| `just boundaries` | 215 run, 215 passed |
| `just ruler` | PASS — digests match the frozen lock |

Beyond the required list, run last:

| Extra | Result |
|---|---|
| `cargo nextest run -p gauntlet-cli` (whole crate) | 61 run, 61 passed |
| `cargo nextest run --workspace` | **1065 run, 1065 passed, 0 skipped** |

## Scope discipline

- Falsifier findings F3, F5, F6, F7, F8, F9, F10, F11 were NOT touched.
- `.gauntlet/policies/*.json`, `.wayfinder/**`, `.agents/research/**`,
  `.agents/CURRENT_STATUS.yml`, and every `.agents/reports/**` file other
  than this one were NOT edited.
- No git command was run; nothing was committed.
- No correction forced a change touching a record-only finding.
- No assertion was weakened, no gate relaxed, no test skipped or deleted, no
  acceptance criterion changed. Two pre-existing test call sites
  (`p5_hard_gate.rs`, `gate.rs`) were adapted to COR-3's new return type and
  came out STRONGER (a `panic!` on the unwrapped arm replacing an
  `Option::expect`).

STATUS: COMPLETE — COR-1, COR-2, COR-3 all fixed, each with a red-first
regression test and a demonstrated mutation.
